//! Community-scoped MK Ideas notification preferences and deduplication.

use buzz_core::mkideas_notification::{MkNotificationClass, MkNotificationPreferencesV1};
use buzz_core::CommunityId;
use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::{Db, DbError, Result};

impl Db {
    /// Store validated actionable-notification preferences.
    pub async fn upsert_mkideas_notification_preferences(
        &self,
        community: CommunityId,
        human_pubkey: &[u8],
        preferences: &MkNotificationPreferencesV1,
    ) -> Result<()> {
        preferences
            .validate()
            .map_err(|error| DbError::InvalidData(error.to_string()))?;
        let enabled = serde_json::to_value(&preferences.enabled)?;
        let schema_version = i16::try_from(preferences.version)
            .map_err(|_| DbError::InvalidData("notification schema version overflow".into()))?;
        sqlx::query(
            r#"
            INSERT INTO mk_notification_preferences (
                community_id, human_pubkey, schema_version, enabled_classes,
                quiet_start_minute, quiet_end_minute, timezone
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (community_id, human_pubkey) DO UPDATE SET
                schema_version = EXCLUDED.schema_version,
                enabled_classes = EXCLUDED.enabled_classes,
                quiet_start_minute = EXCLUDED.quiet_start_minute,
                quiet_end_minute = EXCLUDED.quiet_end_minute,
                timezone = EXCLUDED.timezone,
                updated_at = now()
            "#,
        )
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(schema_version)
        .bind(enabled)
        .bind(preferences.quiet_start_minute.map(i32::from))
        .bind(preferences.quiet_end_minute.map(i32::from))
        .bind(&preferences.timezone)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read preferences only for the exact human and community.
    pub async fn get_mkideas_notification_preferences(
        &self,
        community: CommunityId,
        human_pubkey: &[u8],
    ) -> Result<Option<MkNotificationPreferencesV1>> {
        let row = sqlx::query(
            r#"
            SELECT schema_version, enabled_classes, quiet_start_minute,
                   quiet_end_minute, timezone
            FROM mk_notification_preferences
            WHERE community_id = $1 AND human_pubkey = $2
            "#,
        )
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            let version: i16 = row.try_get("schema_version")?;
            let enabled: serde_json::Value = row.try_get("enabled_classes")?;
            let preferences = MkNotificationPreferencesV1 {
                version: u16::try_from(version).map_err(|_| {
                    DbError::InvalidData("negative notification schema version".into())
                })?,
                enabled: serde_json::from_value(enabled)?,
                quiet_start_minute: optional_minute(&row, "quiet_start_minute")?,
                quiet_end_minute: optional_minute(&row, "quiet_end_minute")?,
                timezone: row.try_get("timezone")?,
            };
            preferences
                .validate()
                .map_err(|error| DbError::InvalidData(error.to_string()))?;
            Ok(preferences)
        })
        .transpose()
    }

    /// Atomically claim one semantic delivery key.
    ///
    /// Returns false for an unexpired duplicate. An expired row may be safely
    /// reused without colliding across recipients or communities.
    pub async fn claim_mkideas_notification_delivery(
        &self,
        community: CommunityId,
        human_pubkey: &[u8],
        dedupe_key: &[u8],
        class: MkNotificationClass,
        expires_at: DateTime<Utc>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO mk_notification_delivery_dedupe (
                community_id, human_pubkey, dedupe_key,
                notification_class, expires_at
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (community_id, human_pubkey, dedupe_key) DO UPDATE SET
                notification_class = EXCLUDED.notification_class,
                claimed_at = now(),
                expires_at = EXCLUDED.expires_at
            WHERE mk_notification_delivery_dedupe.expires_at <= now()
            "#,
        )
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(dedupe_key)
        .bind(notification_class_name(class))
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Delete expired semantic delivery claims in bounded batches.
    pub async fn prune_mkideas_notification_dedupe(&self, limit: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM mk_notification_delivery_dedupe
            WHERE ctid IN (
                SELECT ctid FROM mk_notification_delivery_dedupe
                WHERE expires_at <= now()
                ORDER BY expires_at
                LIMIT $1
            )
            "#,
        )
        .bind(limit.clamp(1, 10_000))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

fn optional_minute(row: &sqlx::postgres::PgRow, column: &str) -> Result<Option<u16>> {
    let value: Option<i16> = row.try_get(column)?;
    value
        .map(|minute| {
            u16::try_from(minute)
                .map_err(|_| DbError::InvalidData(format!("invalid {column} value")))
        })
        .transpose()
}

const fn notification_class_name(class: MkNotificationClass) -> &'static str {
    match class {
        MkNotificationClass::Assignment => "assignment",
        MkNotificationClass::Approval => "approval",
        MkNotificationClass::Deadline => "deadline",
        MkNotificationClass::Mention => "mention",
        MkNotificationClass::AgentOutcome => "agent-outcome",
        MkNotificationClass::ImportantTransition => "important-transition",
    }
}
