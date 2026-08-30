//! MK Ideas community-wide entity-head persistence.
//!
//! The accepted state event is never re-signed. This transaction serializes
//! all authorized human authors at `(community, kind, d)` and leaves exactly
//! one visible event for ordinary relay queries and search.

use buzz_core::{event::StoredEvent, kind::event_kind_i32, mkideas::MkStateEnvelope, CommunityId};
use nostr::Event;
use sqlx::Row;

use crate::{event, Db, DbError, Result};

impl Db {
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
        let kind = event_kind_i32(event);
        let d_tag = envelope.entity_id.to_string();
        let incoming_id = event.id.as_bytes().as_slice();
        let incoming_pubkey = event.pubkey.to_bytes();
        let mut tx = self.pool.begin().await?;

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
        .execute(&mut *tx)
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
        .fetch_one(&mut *tx)
        .await?;

        let current_event_id: Option<Vec<u8>> = row.try_get("current_event_id")?;
        let current_version: i64 = row.try_get("current_version")?;
        if current_event_id
            .as_deref()
            .is_some_and(|current| current == incoming_id)
        {
            let (stored, _) =
                event::insert_event_in_transaction(&mut tx, community, event, None).await?;
            tx.commit().await?;
            return Ok((stored, false));
        }

        let expected_previous = current_event_id.as_deref();
        let provided_previous = envelope
            .previous_event_id
            .as_ref()
            .map(<[u8; 32]>::as_slice);
        if envelope.version != current_version + 1 || provided_previous != expected_previous {
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
                    WHERE community_id = $1 AND id = $2 AND deleted_at IS NULL
                    LIMIT 1
                    "#,
                )
                .bind(community.as_uuid())
                .bind(current)
                .fetch_optional(&mut *tx)
                .await?;
                Some(content.ok_or_else(|| {
                    DbError::InvalidData("MK Ideas head points to a missing live event".to_string())
                })?)
            }
            None => None,
        };

        buzz_core::mkideas::validate_state_transition(
            u32::from(event.kind.as_u16()),
            previous_content.as_deref(),
            envelope,
        )
        .map_err(|error| DbError::MkIdeasValidation(error.to_string()))?;

        let (stored, inserted) =
            event::insert_event_in_transaction(&mut tx, community, event, None).await?;
        if !inserted {
            return Err(DbError::InvalidData(
                "MK Ideas event id already exists outside the authoritative head".to_string(),
            ));
        }

        if let Some(current) = current_event_id.as_deref() {
            sqlx::query(
                "UPDATE events SET deleted_at = now() WHERE community_id = $1 AND id = $2 AND deleted_at IS NULL",
            )
            .bind(community.as_uuid())
            .bind(current)
            .execute(&mut *tx)
            .await?;
        }

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
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok((stored, true))
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

        sqlx::query("DELETE FROM communities WHERE id = $1")
            .bind(community)
            .execute(&pool)
            .await
            .expect("clean community");
    }
}
