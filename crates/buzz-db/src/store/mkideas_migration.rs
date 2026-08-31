//! Atomic storage for one migration receipt and its one imported event.

use buzz_core::kind::{is_mkideas_state_kind, KIND_MK_MIGRATION_RECEIPT};
use buzz_core::mkideas::MkStateEnvelope;
use buzz_core::mkideas_migration::MkMigrationEnvelope;
use buzz_core::{CommunityId, StoredEvent};
use nostr::Event;
use sqlx::Row;
use uuid::Uuid;

use crate::{event, Db, DbError, Result};

/// Result of an atomic migration item apply.
#[derive(Clone, Debug)]
pub struct MkMigrationApplyResult {
    /// Stored service-signed receipt.
    pub receipt: StoredEvent,
    /// Stored imported event.
    pub imported: StoredEvent,
    /// True only when this source coordinate was first accepted.
    pub inserted: bool,
}

impl Db {
    /// Atomically apply one validated migration receipt/import pair.
    ///
    /// The active dataset-scoped service grant is rechecked and held inside
    /// this transaction. A replay of the exact pair is a no-op; any reuse of a
    /// source coordinate with different hashes or event ids fails closed.
    pub async fn apply_mkideas_migration_item(
        &self,
        community: CommunityId,
        receipt_event: &Event,
        envelope: &MkMigrationEnvelope,
    ) -> Result<MkMigrationApplyResult> {
        let receipt = &envelope.receipt;
        let imported = &receipt.imported_event;
        let mut tx = self.pool.begin().await?;

        let grant_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT service_grant.id
            FROM mk_service_grants service_grant
            JOIN users service
              ON service.community_id = service_grant.community_id
             AND service.pubkey = service_grant.service_pubkey
             AND service.agent_owner_pubkey = service_grant.owner_pubkey
            JOIN users owner
              ON owner.community_id = service_grant.community_id
             AND owner.pubkey = service_grant.owner_pubkey
            WHERE service_grant.community_id = $1
              AND service_grant.service_pubkey = $2
              AND service_grant.purpose = 'migration'
              AND service_grant.persona IS NULL
              AND $3 = ANY(service_grant.allowed_event_kinds)
              AND $4 = ANY(service_grant.allowed_target_kinds)
              AND service_grant.dataset_sha256 = $5
              AND service_grant.revoked_at IS NULL
              AND service_grant.expires_at > now()
            FOR SHARE OF service_grant
            "#,
        )
        .bind(community.as_uuid())
        .bind(receipt_event.pubkey.as_bytes())
        .bind(KIND_MK_MIGRATION_RECEIPT as i32)
        .bind(receipt.target_kind as i32)
        .bind(&receipt.dataset_sha256)
        .fetch_optional(&mut *tx)
        .await?;
        if grant_id.is_none() {
            return Err(DbError::AccessDenied(
                "no active dataset-scoped migration grant permits this target kind".to_string(),
            ));
        }

        let existing = sqlx::query(
            r#"
            SELECT source_sha256, record_id, destination_kind,
                   destination_d_tag, imported_event_id, receipt_event_id,
                   service_pubkey
            FROM mk_migration_items
            WHERE community_id = $1 AND dataset_sha256 = $2 AND batch_id = $3
              AND source_system = $4 AND source_workspace_id = $5
              AND source_type = $6 AND source_id = $7 AND source_revision = $8
            FOR UPDATE
            "#,
        )
        .bind(community.as_uuid())
        .bind(&receipt.dataset_sha256)
        .bind(receipt.batch_id)
        .bind(&receipt.source.system)
        .bind(receipt.source.workspace_id)
        .bind(&receipt.source.unit_kind)
        .bind(&receipt.source.unit_id)
        .bind(i64::try_from(receipt.source.revision).map_err(|_| {
            DbError::MkIdeasValidation("source revision exceeds PostgreSQL BIGINT".to_string())
        })?)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = existing {
            ensure_exact_replay(&row, receipt_event, envelope)?;
            let stored_receipt_id: Vec<u8> = row.try_get("receipt_event_id")?;
            let stored_imported_id: Vec<u8> = row.try_get("imported_event_id")?;
            let stored_receipt =
                load_event_in_transaction(&mut tx, community, &stored_receipt_id).await?;
            let stored_imported =
                load_event_in_transaction(&mut tx, community, &stored_imported_id).await?;
            tx.commit().await?;
            return Ok(MkMigrationApplyResult {
                receipt: stored_receipt,
                imported: stored_imported,
                inserted: false,
            });
        }

        let channel_id = imported_channel_id(receipt.target_kind, imported)?;
        if let Some(channel_id) = channel_id {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM channels WHERE community_id = $1 AND id = $2)",
            )
            .bind(community.as_uuid())
            .bind(channel_id)
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                return Err(DbError::MkIdeasValidation(
                    "Team history target channel does not exist in this community".to_string(),
                ));
            }
        }
        let (stored_imported, imported_inserted) = if is_mkideas_state_kind(receipt.target_kind) {
            let entity_id = receipt.destination_d.ok_or_else(|| {
                DbError::MkIdeasValidation("state import is missing destination d".to_string())
            })?;
            let version = imported_tag(imported, "version")?
                .parse::<i64>()
                .map_err(|_| DbError::MkIdeasValidation("invalid imported version".to_string()))?;
            let previous_event_id = imported_optional_event_id(imported, "prev")?;
            let status = imported_tag(imported, "status")?.to_string();
            let state = MkStateEnvelope {
                entity_id,
                community: envelope.receipt.community.clone(),
                version,
                previous_event_id,
                status,
                content: envelope.imported_content.clone(),
                schema_version: 2,
            };
            super::mkideas::advance_head_in_transaction(&mut tx, community, imported, &state, true)
                .await?
        } else {
            let stored =
                event::insert_event_in_transaction(&mut tx, community, imported, channel_id)
                    .await?;
            if stored.1 {
                crate::insert_mentions_in_transaction(&mut tx, community, imported, channel_id)
                    .await?;
            }
            stored
        };
        if !imported_inserted {
            return Err(DbError::MkIdeasValidation(
                "imported event already exists without this migration item".to_string(),
            ));
        }

        let (stored_receipt, receipt_inserted) =
            event::insert_event_in_transaction(&mut tx, community, receipt_event, None).await?;
        if !receipt_inserted {
            return Err(DbError::MkIdeasValidation(
                "receipt event already exists without this migration item".to_string(),
            ));
        }
        crate::insert_mentions_in_transaction(&mut tx, community, receipt_event, None).await?;

        sqlx::query(
            r#"
            INSERT INTO mk_migration_items (
                community_id, dataset_sha256, batch_id, source_system,
                source_workspace_id, source_type, source_id, source_revision,
                source_sha256, record_id, destination_kind, destination_d_tag,
                imported_event_id, receipt_event_id, service_pubkey
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
            "#,
        )
        .bind(community.as_uuid())
        .bind(&receipt.dataset_sha256)
        .bind(receipt.batch_id)
        .bind(&receipt.source.system)
        .bind(receipt.source.workspace_id)
        .bind(&receipt.source.unit_kind)
        .bind(&receipt.source.unit_id)
        .bind(i64::try_from(receipt.source.revision).map_err(|_| {
            DbError::MkIdeasValidation("source revision exceeds PostgreSQL BIGINT".to_string())
        })?)
        .bind(&receipt.source_sha256)
        .bind(receipt.record_id)
        .bind(receipt.target_kind as i32)
        .bind(receipt.destination_d.map(|id| id.to_string()))
        .bind(imported.id.as_bytes().as_slice())
        .bind(receipt_event.id.as_bytes().as_slice())
        .bind(receipt_event.pubkey.as_bytes())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(MkMigrationApplyResult {
            receipt: stored_receipt,
            imported: stored_imported,
            inserted: true,
        })
    }
}

fn ensure_exact_replay(
    row: &sqlx::postgres::PgRow,
    receipt_event: &Event,
    envelope: &MkMigrationEnvelope,
) -> Result<()> {
    let receipt = &envelope.receipt;
    let imported = &receipt.imported_event;
    let exact = row.try_get::<String, _>("source_sha256")? == receipt.source_sha256
        && row.try_get::<Uuid, _>("record_id")? == receipt.record_id
        && row.try_get::<i32, _>("destination_kind")? == receipt.target_kind as i32
        && row.try_get::<Option<String>, _>("destination_d_tag")?
            == receipt.destination_d.map(|id| id.to_string())
        && row.try_get::<Vec<u8>, _>("imported_event_id")?.as_slice()
            == imported.id.as_bytes().as_slice()
        && row.try_get::<Vec<u8>, _>("service_pubkey")?.as_slice()
            == receipt_event.pubkey.as_bytes().as_slice();
    if exact {
        Ok(())
    } else {
        Err(DbError::MkIdeasValidation(
            "source coordinate was already imported with different immutable values".to_string(),
        ))
    }
}

async fn load_event_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    community: CommunityId,
    event_id: &[u8],
) -> Result<StoredEvent> {
    let row = sqlx::query(
        "SELECT id, pubkey, created_at, kind, tags, content, sig, received_at, channel_id \
         FROM events WHERE community_id = $1 AND id = $2 LIMIT 1",
    )
    .bind(community.as_uuid())
    .bind(event_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| DbError::InvalidData("migration item points to a missing event".to_string()))?;
    event::row_to_stored_event(row)?.ok_or_else(|| {
        DbError::InvalidData("migration item event could not be decoded".to_string())
    })
}

fn imported_channel_id(kind: u32, event: &Event) -> Result<Option<Uuid>> {
    if kind != 9 {
        return Ok(None);
    }
    imported_tag(event, "h")?
        .parse::<Uuid>()
        .map(Some)
        .map_err(|_| {
            DbError::MkIdeasValidation("Team history h tag must be a channel UUID".to_string())
        })
}

fn imported_tag<'a>(event: &'a Event, name: &str) -> Result<&'a str> {
    let values = event
        .tags
        .iter()
        .filter_map(|tag| {
            let parts = tag.as_slice();
            (parts.first().map(String::as_str) == Some(name))
                .then(|| parts.get(1).map(String::as_str))
                .flatten()
        })
        .collect::<Vec<_>>();
    match values.as_slice() {
        [value] => Ok(*value),
        _ => Err(DbError::MkIdeasValidation(format!(
            "imported event requires exactly one {name} tag"
        ))),
    }
}

fn imported_optional_event_id(event: &Event, name: &str) -> Result<Option<[u8; 32]>> {
    let values = event
        .tags
        .iter()
        .filter_map(|tag| {
            let parts = tag.as_slice();
            (parts.first().map(String::as_str) == Some(name))
                .then(|| parts.get(1).cloned())
                .flatten()
        })
        .collect::<Vec<_>>();
    match values.as_slice() {
        [] => Ok(None),
        [value] => hex::decode(value)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .map(Some)
            .ok_or_else(|| {
                DbError::MkIdeasValidation("invalid imported prev event id".to_string())
            }),
        _ => Err(DbError::MkIdeasValidation(
            "imported event has duplicate prev tags".to_string(),
        )),
    }
}
