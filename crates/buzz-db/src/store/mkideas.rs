//! MK Ideas community-wide entity-head persistence.
//!
//! The accepted state event is never re-signed. This transaction serializes
//! all authorized human authors at `(community, kind, d)` and leaves exactly
//! one visible event for ordinary relay queries and search.

use buzz_core::{event::StoredEvent, kind::event_kind_i32, mkideas::MkStateEnvelope, CommunityId};
use nostr::Event;
use sqlx::{QueryBuilder, Row};

use crate::{event, Db, DbError, Result};

/// Stable cursor for current-state traversal. It is independent of update
/// timestamps, so a head changing while the client pages cannot duplicate a
/// different entity coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MkIdeasHeadCursor {
    /// Last kind returned by the previous page.
    pub kind: u32,
    /// Last stable entity UUID returned by the previous page.
    pub entity_id: uuid::Uuid,
}

async fn insert_revision_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    community: CommunityId,
    event: &Event,
    envelope: &MkStateEnvelope,
) -> Result<()> {
    let signer_pubkey = event.pubkey.to_bytes();
    sqlx::query(
        r#"
        INSERT INTO mk_entity_revisions (
            community_id, kind, d_tag, version, event_id,
            previous_event_id, signer_pubkey, schema_version
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (community_id, event_id) DO NOTHING
        "#,
    )
    .bind(community.as_uuid())
    .bind(event_kind_i32(event))
    .bind(envelope.entity_id.to_string())
    .bind(envelope.version)
    .bind(event.id.as_bytes().as_slice())
    .bind(
        envelope
            .previous_event_id
            .as_ref()
            .map(<[u8; 32]>::as_slice),
    )
    .bind(signer_pubkey.as_slice())
    .bind(envelope.schema_version as i64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn advance_head_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    community: CommunityId,
    event: &Event,
    envelope: &MkStateEnvelope,
    allow_sparse_initial_version: bool,
) -> Result<(StoredEvent, bool)> {
    let kind = event_kind_i32(event);
    let d_tag = envelope.entity_id.to_string();
    let incoming_id = event.id.as_bytes().as_slice();
    let incoming_pubkey = event.pubkey.to_bytes();

    sqlx::query(
        r#"
        INSERT INTO mk_entity_heads (community_id, kind, d_tag)
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(community.as_uuid())
    .bind(kind)
    .bind(&d_tag)
    .execute(&mut **tx)
    .await?;

    let row = sqlx::query(
        r#"
        SELECT current_event_id, current_version
        FROM mk_entity_heads
        WHERE community_id = $1 AND kind = $2 AND d_tag = $3
        FOR UPDATE
        "#,
    )
    .bind(community.as_uuid())
    .bind(kind)
    .bind(&d_tag)
    .fetch_one(&mut **tx)
    .await?;

    let current_event_id: Option<Vec<u8>> = row.try_get("current_event_id")?;
    let current_version: i64 = row.try_get("current_version")?;
    if current_event_id
        .as_deref()
        .is_some_and(|current| current == incoming_id)
    {
        let (stored, _) = event::insert_event_in_transaction(tx, community, event, None).await?;
        insert_revision_in_transaction(tx, community, event, envelope).await?;
        crate::insert_mentions_in_transaction(tx, community, event, None).await?;
        return Ok((stored, false));
    }

    let expected_previous = current_event_id.as_deref();
    let provided_previous = envelope
        .previous_event_id
        .as_ref()
        .map(<[u8; 32]>::as_slice);
    let valid_version = envelope.version == current_version + 1
        || (allow_sparse_initial_version && current_version == 0 && envelope.version >= 1);
    if !valid_version || provided_previous != expected_previous {
        return Err(DbError::MkIdeasConflict {
            current_version,
            current_event_id: current_event_id.as_deref().map(hex::encode),
        });
    }

    let previous_content: Option<String> = match current_event_id.as_deref() {
        Some(current) => {
            let content = sqlx::query_scalar(
                r#"
                SELECT content
                FROM events
                WHERE community_id = $1 AND id = $2
                LIMIT 1
                "#,
            )
            .bind(community.as_uuid())
            .bind(current)
            .fetch_optional(&mut **tx)
            .await?;
            Some(content.ok_or_else(|| {
                DbError::InvalidData("MK Ideas head points to a missing event".to_string())
            })?)
        }
        None => None,
    };

    let allow_dnc_clear = if kind == buzz_core::kind::KIND_MK_PERSON as i32 {
        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM relay_members
                WHERE community_id = $1
                  AND pubkey = $2
                  AND role = 'owner'
            )
            "#,
        )
        .bind(community.as_uuid())
        .bind(event.pubkey.to_hex())
        .fetch_one(&mut **tx)
        .await?
    } else {
        false
    };

    buzz_core::mkideas::validate_state_transition_with_policy(
        u32::from(event.kind.as_u16()),
        previous_content.as_deref(),
        envelope,
        allow_dnc_clear,
    )
    .map_err(|error| DbError::MkIdeasValidation(error.to_string()))?;

    let (stored, inserted) = event::insert_event_in_transaction(tx, community, event, None).await?;
    if !inserted {
        return Err(DbError::InvalidData(
            "MK Ideas event id already exists outside the authoritative head".to_string(),
        ));
    }

    insert_revision_in_transaction(tx, community, event, envelope).await?;
    crate::insert_mentions_in_transaction(tx, community, event, None).await?;

    sqlx::query(
        r#"
        UPDATE mk_entity_heads
        SET current_event_id = $4,
            current_version = $5,
            current_pubkey = $6,
            updated_at = now()
        WHERE community_id = $1 AND kind = $2 AND d_tag = $3
        "#,
    )
    .bind(community.as_uuid())
    .bind(kind)
    .bind(&d_tag)
    .bind(incoming_id)
    .bind(envelope.version)
    .bind(incoming_pubkey.as_slice())
    .execute(&mut **tx)
    .await?;

    Ok((stored, true))
}

fn approval_uuid(content: &serde_json::Value, field: &str) -> Result<uuid::Uuid> {
    content
        .get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or_else(|| DbError::MkIdeasValidation(format!("`{field}` must be a UUID")))
}

fn approval_event_id(content: &serde_json::Value, field: &str) -> Result<Vec<u8>> {
    content
        .get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(|value| hex::decode(value).ok())
        .filter(|value| value.len() == 32)
        .ok_or_else(|| DbError::MkIdeasValidation(format!("`{field}` must be a 32-byte event id")))
}

impl Db {
    /// Return whether an active, owner-associated service grant permits this
    /// exact event and target kind. This is a capability check, not a role
    /// fallback: absent or expired grants fail closed.
    #[allow(clippy::too_many_arguments)]
    pub async fn mkideas_service_grant_allows(
        &self,
        community: CommunityId,
        service_pubkey: &[u8],
        purpose: &str,
        persona: Option<&str>,
        event_kind: u32,
        target_kind: Option<u32>,
        dataset_sha256: Option<&str>,
    ) -> Result<bool> {
        let allowed: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
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
                  AND service_grant.purpose = $3
                  AND service_grant.persona IS NOT DISTINCT FROM $4::TEXT
                  AND $5 = ANY(service_grant.allowed_event_kinds)
                  AND ($6::INT IS NULL OR $6 = ANY(service_grant.allowed_target_kinds))
                  AND service_grant.dataset_sha256 IS NOT DISTINCT FROM $7::TEXT
                  AND service_grant.revoked_at IS NULL
                  AND service_grant.expires_at > now()
            )
            "#,
        )
        .bind(community.as_uuid())
        .bind(service_pubkey)
        .bind(purpose)
        .bind(persona)
        .bind(event_kind as i32)
        .bind(target_kind.map(|kind| kind as i32))
        .bind(dataset_sha256)
        .fetch_one(&self.pool)
        .await?;
        Ok(allowed)
    }

    /// Load the current accepted MK Ideas event for a community-wide entity.
    pub async fn get_mkideas_entity_head(
        &self,
        community: CommunityId,
        kind: u32,
        entity_id: uuid::Uuid,
    ) -> Result<Option<StoredEvent>> {
        let current_event_id: Option<Vec<u8>> = sqlx::query_scalar(
            r#"
            SELECT current_event_id
            FROM mk_entity_heads
            WHERE community_id = $1 AND kind = $2 AND d_tag = $3
            "#,
        )
        .bind(community.as_uuid())
        .bind(kind as i32)
        .bind(entity_id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        match current_event_id {
            Some(event_id) => self.get_event_by_id(community, &event_id).await,
            None => Ok(None),
        }
    }

    /// Page authoritative MK Ideas heads in stable `(kind, d)` order.
    pub async fn query_mkideas_heads(
        &self,
        community: CommunityId,
        kinds: &[u32],
        after: Option<&MkIdeasHeadCursor>,
        limit: u32,
    ) -> Result<(Vec<StoredEvent>, Option<MkIdeasHeadCursor>)> {
        if kinds.is_empty() {
            return Ok((Vec::new(), None));
        }
        let page_size = limit.clamp(1, 200) as i64;
        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT e.id, e.pubkey, e.created_at, e.kind, e.tags, e.content, \
             e.sig, e.received_at, e.channel_id \
             FROM mk_entity_heads h \
             JOIN events e ON e.community_id = h.community_id \
                          AND e.id = h.current_event_id \
             WHERE h.community_id = ",
        );
        qb.push_bind(community.as_uuid());
        qb.push(" AND h.kind = ANY(")
            .push_bind(kinds.iter().map(|kind| *kind as i32).collect::<Vec<_>>())
            .push(")");
        if let Some(cursor) = after {
            qb.push(" AND (h.kind > ")
                .push_bind(cursor.kind as i32)
                .push(" OR (h.kind = ")
                .push_bind(cursor.kind as i32)
                .push(" AND h.d_tag > ")
                .push_bind(cursor.entity_id.to_string())
                .push("))");
        }
        qb.push(" ORDER BY h.kind ASC, h.d_tag ASC LIMIT ")
            .push_bind(page_size + 1);

        let rows = qb.build().fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > page_size;
        let mut events = Vec::with_capacity(rows.len().min(page_size as usize));
        for row in rows.into_iter().take(page_size as usize) {
            if let Some(stored) = event::row_to_stored_event(row)? {
                events.push(stored);
            }
        }
        let next = if has_more {
            events.last().and_then(|stored| {
                let kind = u32::from(stored.event.kind.as_u16());
                let entity_id = stored.event.tags.iter().find_map(|tag| {
                    let values = tag.as_slice();
                    (values.first().map(String::as_str) == Some("d"))
                        .then(|| values.get(1))
                        .flatten()
                        .and_then(|value| uuid::Uuid::parse_str(value).ok())
                })?;
                Some(MkIdeasHeadCursor { kind, entity_id })
            })
        } else {
            None
        };
        Ok((events, next))
    }

    /// Page every accepted revision for one entity, newest version first.
    /// `before_version` is exclusive and therefore safe to reuse as a cursor.
    pub async fn query_mkideas_history(
        &self,
        community: CommunityId,
        kind: u32,
        entity_id: uuid::Uuid,
        before_version: Option<i64>,
        limit: u32,
    ) -> Result<(Vec<StoredEvent>, Option<i64>)> {
        let page_size = limit.clamp(1, 200) as i64;
        let rows = sqlx::query(
            r#"
            SELECT e.id, e.pubkey, e.created_at, e.kind, e.tags, e.content,
                   e.sig, e.received_at, e.channel_id, r.version AS mk_version
            FROM mk_entity_revisions r
            JOIN events e ON e.community_id = r.community_id AND e.id = r.event_id
            WHERE r.community_id = $1
              AND r.kind = $2
              AND r.d_tag = $3
              AND ($4::BIGINT IS NULL OR r.version < $4)
            ORDER BY r.version DESC, r.event_id ASC
            LIMIT $5
            "#,
        )
        .bind(community.as_uuid())
        .bind(kind as i32)
        .bind(entity_id.to_string())
        .bind(before_version)
        .bind(page_size + 1)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() as i64 > page_size;
        let next = if has_more {
            rows.get(page_size as usize - 1)
                .map(|row| row.try_get::<i64, _>("mk_version"))
                .transpose()?
        } else {
            None
        };
        let mut events = Vec::with_capacity(rows.len().min(page_size as usize));
        for row in rows.into_iter().take(page_size as usize) {
            if let Some(stored) = event::row_to_stored_event(row)? {
                events.push(stored);
            }
        }
        Ok((events, next))
    }

    /// Store a schema-v2 approval action and its optional human-signed result
    /// as one transaction. Exact request, proposal, target event, and target
    /// version bindings are rechecked while their projection rows are locked.
    pub async fn apply_mkideas_approval_action(
        &self,
        community: CommunityId,
        action: &Event,
    ) -> Result<(StoredEvent, Option<StoredEvent>, bool)> {
        let content: serde_json::Value = serde_json::from_str(&action.content)?;
        if content
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(2)
        {
            return Err(DbError::MkIdeasValidation(
                "atomic approval requires schema version 2".to_string(),
            ));
        }
        let approval_id = approval_uuid(&content, "approval_id")?;
        let approval_request_event_id = approval_event_id(&content, "approval_event_id")?;
        let target_id = approval_uuid(&content, "target_id")?;
        let target_event_id = approval_event_id(&content, "target_event_id")?;
        let target_version = content
            .get("target_version")
            .and_then(serde_json::Value::as_i64)
            .filter(|version| *version >= 1)
            .ok_or_else(|| {
                DbError::MkIdeasValidation("`target_version` must be positive".to_string())
            })?;
        let target_kind = content
            .get("target_kind")
            .and_then(serde_json::Value::as_u64)
            .filter(|kind| (30800..=30808).contains(kind))
            .ok_or_else(|| {
                DbError::MkIdeasValidation("`target_kind` must be 30800-30808".to_string())
            })? as i32;
        let proposal_id = approval_uuid(&content, "proposal_id")?;
        let proposal_event_id = approval_event_id(&content, "proposal_event_id")?;
        let decision = content
            .get("decision")
            .and_then(serde_json::Value::as_str)
            .filter(|value| matches!(*value, "approved" | "rejected"))
            .ok_or_else(|| {
                DbError::MkIdeasValidation("decision must be approved or rejected".to_string())
            })?;
        let reason = content
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty() && value.len() <= 2_000)
            .ok_or_else(|| DbError::MkIdeasValidation("approval reason is required".to_string()))?;

        let result = content
            .get("result_event")
            .map(|value| {
                serde_json::from_value::<Event>(value.clone()).map_err(|error| {
                    DbError::MkIdeasValidation(format!("invalid result event: {error}"))
                })
            })
            .transpose()?;
        if result.is_some() && decision != "approved" {
            return Err(DbError::MkIdeasValidation(
                "a rejected action cannot carry a resulting state event".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await?;
        let already_decided: Option<Vec<u8>> = sqlx::query_scalar(
            r#"
            SELECT action_event_id
            FROM mk_approval_decisions
            WHERE community_id = $1 AND approval_id = $2
            FOR UPDATE
            "#,
        )
        .bind(community.as_uuid())
        .bind(approval_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing_action) = already_decided {
            if existing_action.as_slice() != action.id.as_bytes() {
                return Err(DbError::MkIdeasConflict {
                    current_version: 1,
                    current_event_id: Some(hex::encode(existing_action)),
                });
            }
            let (stored, _) =
                event::insert_event_in_transaction(&mut tx, community, action, None).await?;
            tx.commit().await?;
            return Ok((stored, None, false));
        }

        let approval_row = sqlx::query(
            r#"
            SELECT h.current_event_id, e.content
            FROM mk_entity_heads h
            JOIN events e ON e.community_id = h.community_id AND e.id = h.current_event_id
            WHERE h.community_id = $1 AND h.kind = 30809 AND h.d_tag = $2
            FOR UPDATE OF h
            "#,
        )
        .bind(community.as_uuid())
        .bind(approval_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| DbError::MkIdeasValidation("approval request not found".to_string()))?;
        let current_approval_event: Vec<u8> = approval_row.try_get("current_event_id")?;
        if current_approval_event != approval_request_event_id {
            return Err(DbError::MkIdeasValidation(
                "approval request changed and is stale".to_string(),
            ));
        }
        let approval_content: serde_json::Value =
            serde_json::from_str(approval_row.try_get::<String, _>("content")?.as_str())?;
        if approval_content
            .get("status")
            .and_then(serde_json::Value::as_str)
            != Some("pending")
            || approval_content.get("target_id") != content.get("target_id")
            || approval_content.get("target_kind") != content.get("target_kind")
            || approval_content.get("target_event_id") != content.get("target_event_id")
            || approval_content.get("target_version") != content.get("target_version")
            || approval_content.get("proposal_id") != content.get("proposal_id")
            || approval_content.get("proposal_event_id") != content.get("proposal_event_id")
        {
            return Err(DbError::MkIdeasValidation(
                "approval action does not match the pending request".to_string(),
            ));
        }

        let target_row = sqlx::query(
            r#"
            SELECT current_event_id, current_version
            FROM mk_entity_heads
            WHERE community_id = $1 AND kind = $2 AND d_tag = $3
            FOR UPDATE
            "#,
        )
        .bind(community.as_uuid())
        .bind(target_kind)
        .bind(target_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| DbError::MkIdeasValidation("approval target not found".to_string()))?;
        let current_target_event: Vec<u8> = target_row.try_get("current_event_id")?;
        let current_target_version: i64 = target_row.try_get("current_version")?;
        if current_target_event != target_event_id || current_target_version != target_version {
            return Err(DbError::MkIdeasValidation(
                "approval target changed and the request is stale".to_string(),
            ));
        }

        let proposal_content: Option<String> = sqlx::query_scalar(
            "SELECT content FROM events WHERE community_id = $1 AND id = $2 AND kind = 48201 LIMIT 1",
        )
        .bind(community.as_uuid())
        .bind(&proposal_event_id)
        .fetch_optional(&mut *tx)
        .await?;
        let proposal_content: serde_json::Value = serde_json::from_str(
            proposal_content
                .ok_or_else(|| DbError::MkIdeasValidation("proposal not found".to_string()))?
                .as_str(),
        )?;
        let proposal_target = buzz_core::mkideas::parse_agent_proposal_target(&proposal_content)
            .map_err(|error| DbError::MkIdeasValidation(error.to_string()))?;
        if proposal_content
            .get("proposal_id")
            .and_then(serde_json::Value::as_str)
            != Some(proposal_id.to_string().as_str())
            || proposal_target.entity_id != target_id
            || proposal_target.kind as i32 != target_kind
            || proposal_target.event_id.as_ref().map(|id| id.as_slice())
                != Some(target_event_id.as_slice())
            || proposal_target.version != Some(target_version)
        {
            return Err(DbError::MkIdeasValidation(
                "proposal does not match the exact approval target revision".to_string(),
            ));
        }

        let (stored_action, inserted) =
            event::insert_event_in_transaction(&mut tx, community, action, None).await?;
        if !inserted {
            return Err(DbError::InvalidData(
                "approval action exists without its decision projection".to_string(),
            ));
        }
        crate::insert_mentions_in_transaction(&mut tx, community, action, None).await?;

        let stored_result = if let Some(result) = result.as_ref() {
            if result.pubkey != action.pubkey || !result.verify_id() || !result.verify_signature() {
                return Err(DbError::MkIdeasValidation(
                    "result event must be valid and signed by the approving human".to_string(),
                ));
            }
            if i32::from(result.kind.as_u16()) != target_kind {
                return Err(DbError::MkIdeasValidation(
                    "result event kind does not match the approval target".to_string(),
                ));
            }
            let envelope = buzz_core::mkideas::validate_state_event(result)
                .map_err(|error| DbError::MkIdeasValidation(error.to_string()))?;
            let community_host: String =
                sqlx::query_scalar("SELECT host FROM communities WHERE id = $1")
                    .bind(community.as_uuid())
                    .fetch_one(&mut *tx)
                    .await?;
            if envelope.community != community_host
                || envelope.entity_id != target_id
                || envelope.version != target_version + 1
                || envelope
                    .previous_event_id
                    .as_ref()
                    .map(<[u8; 32]>::as_slice)
                    != Some(target_event_id.as_slice())
            {
                return Err(DbError::MkIdeasValidation(
                    "result event does not advance the exact approved target".to_string(),
                ));
            }
            Some(
                advance_head_in_transaction(&mut tx, community, result, &envelope, false)
                    .await?
                    .0,
            )
        } else {
            None
        };

        let signer = action.pubkey.to_bytes();
        sqlx::query(
            r#"
            INSERT INTO mk_approval_decisions (
                community_id, approval_id, approval_event_id,
                target_kind, target_d_tag, target_event_id, target_version,
                proposal_id, proposal_event_id, action_event_id, result_event_id,
                decision, decided_by, reason
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            "#,
        )
        .bind(community.as_uuid())
        .bind(approval_id)
        .bind(&approval_request_event_id)
        .bind(target_kind)
        .bind(target_id.to_string())
        .bind(&target_event_id)
        .bind(target_version)
        .bind(proposal_id)
        .bind(&proposal_event_id)
        .bind(action.id.as_bytes().as_slice())
        .bind(
            stored_result
                .as_ref()
                .map(|stored| stored.event.id.as_bytes().as_slice()),
        )
        .bind(decision)
        .bind(signer.as_slice())
        .bind(reason)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok((stored_action, stored_result, true))
    }

    /// Atomically validate and advance one MK Ideas authoritative entity head.
    ///
    /// Returns `(event, inserted)`. Replaying the current event id is
    /// idempotent and returns `inserted = false`; stale versions return
    /// [`DbError::MkIdeasConflict`].
    pub async fn replace_mkideas_entity_head(
        &self,
        community: CommunityId,
        event: &Event,
        envelope: &MkStateEnvelope,
    ) -> Result<(StoredEvent, bool)> {
        let mut tx = self.pool.begin().await?;
        let result =
            advance_head_in_transaction(&mut tx, community, event, envelope, false).await?;
        tx.commit().await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buzz_core::kind::KIND_MK_PERSON;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    const TEST_DB_URL: &str = "postgres://buzz:buzz_dev@localhost:5432/buzz"; // sadscan:disable np.postgres.1 -- local test-only credentials

    async fn setup_pool() -> PgPool {
        let database_url = std::env::var("BUZZ_TEST_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| TEST_DB_URL.to_owned());
        PgPool::connect(&database_url)
            .await
            .expect("connect to test DB")
    }

    fn person_event(
        keys: &Keys,
        entity_id: Uuid,
        version: i64,
        previous: Option<&Event>,
        name: &str,
    ) -> Event {
        let mut tags = vec![
            Tag::parse(["d", entity_id.to_string().as_str()]).expect("d"),
            Tag::parse(["h", "mkideas-concurrency.example"]).expect("h"),
            Tag::parse(["version", version.to_string().as_str()]).expect("version"),
            Tag::parse([
                "status",
                if version == 1 {
                    "potential"
                } else {
                    "researching"
                },
            ])
            .expect("status"),
        ];
        if let Some(previous) = previous {
            tags.push(Tag::parse(["prev", previous.id.to_hex().as_str()]).expect("prev"));
        }
        EventBuilder::new(
            Kind::from(KIND_MK_PERSON as u16),
            json!({
                "schema_version": 1,
                "record_type": "person",
                "entity_id": entity_id,
                "version": version,
                "status": if version == 1 { "potential" } else { "researching" },
                "name": name,
                "do_not_contact": false
            })
            .to_string(),
        )
        .tags(tags)
        .sign_with_keys(keys)
        .expect("signed person")
    }

    #[tokio::test]
    async fn two_human_signers_cannot_both_advance_one_head() {
        let pool = setup_pool().await;
        let community = Uuid::new_v4();
        sqlx::query("INSERT INTO communities (id, host) VALUES ($1, $2)")
            .bind(community)
            .bind(format!(
                "mkideas-concurrency-{}.example",
                community.simple()
            ))
            .execute(&pool)
            .await
            .expect("insert community");
        let db = Db::from_pool(pool.clone());
        let entity_id = Uuid::new_v4();
        let original = person_event(&Keys::generate(), entity_id, 1, None, "Avery Stone");
        let original_envelope = buzz_core::mkideas::validate_state_event(&original).expect("v1");
        db.replace_mkideas_entity_head(
            CommunityId::from_uuid(community),
            &original,
            &original_envelope,
        )
        .await
        .expect("insert v1");

        let first = person_event(
            &Keys::generate(),
            entity_id,
            2,
            Some(&original),
            "Avery Stone — first edit",
        );
        let second = person_event(
            &Keys::generate(),
            entity_id,
            2,
            Some(&original),
            "Avery Stone — second edit",
        );
        let first_envelope = buzz_core::mkideas::validate_state_event(&first).expect("first v2");
        let second_envelope = buzz_core::mkideas::validate_state_event(&second).expect("second v2");
        let (first_result, second_result) = tokio::join!(
            db.replace_mkideas_entity_head(
                CommunityId::from_uuid(community),
                &first,
                &first_envelope,
            ),
            db.replace_mkideas_entity_head(
                CommunityId::from_uuid(community),
                &second,
                &second_envelope,
            )
        );
        let results = [first_result, second_result];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(DbError::MkIdeasConflict { .. })))
                .count(),
            1
        );

        let head = db
            .get_mkideas_entity_head(CommunityId::from_uuid(community), KIND_MK_PERSON, entity_id)
            .await
            .expect("load head")
            .expect("head exists");
        assert!(head.event.id == first.id || head.event.id == second.id);
        assert!(head.event.pubkey == first.pubkey || head.event.pubkey == second.pubkey);

        sqlx::query("DELETE FROM event_mentions WHERE community_id = $1")
            .bind(community)
            .execute(&pool)
            .await
            .expect("clean mentions");
        sqlx::query("DELETE FROM events WHERE community_id = $1")
            .bind(community)
            .execute(&pool)
            .await
            .expect("clean events");
        sqlx::query("DELETE FROM communities WHERE id = $1")
            .bind(community)
            .execute(&pool)
            .await
            .expect("clean community");
    }
}
