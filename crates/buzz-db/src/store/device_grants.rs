//! Staged MK Ideas device-bound grant persistence.
//!
//! The tables and lookup seam land before enforcement so existing clients can
//! be enrolled without locking out the private team. Every operation is
//! community-scoped and recovery approvals verify the surviving grant inside
//! the same transaction.

use buzz_core::CommunityId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{Db, DbError, Result};

/// Durable device grant state returned by inventory queries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MkDeviceGrantRecord {
    /// Grant identifier.
    pub id: Uuid,
    /// Human identity public key.
    pub human_pubkey: Vec<u8>,
    /// Independent device public key.
    pub device_pubkey: Vec<u8>,
    /// Human-readable device name.
    pub device_name: String,
    /// Platform label.
    pub platform: String,
    /// Enrollment method.
    pub enrollment_method: String,
    /// Grant issue time.
    pub issued_at: DateTime<Utc>,
    /// Optional expiry.
    pub expires_at: Option<DateTime<Utc>>,
    /// Most recent authenticated use.
    pub last_seen_at: Option<DateTime<Utc>>,
    /// Revocation time.
    pub revoked_at: Option<DateTime<Utc>>,
    /// Human-readable revocation reason.
    pub revocation_reason: Option<String>,
    /// Monotonic authentication epoch.
    pub auth_epoch: i64,
}

/// Result of a durable grant lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MkDeviceGrantStatus {
    /// Grant exists and may authenticate.
    Active,
    /// No grant exists in this community for the exact human/device pair.
    Missing,
    /// Grant was revoked.
    Revoked,
    /// Grant expired.
    Expired,
}

/// Inputs for atomically consuming an enrollment challenge.
pub struct MkDeviceEnrollmentInput<'a> {
    /// Server-issued challenge identifier.
    pub challenge_id: Uuid,
    /// Expected human public key.
    pub human_pubkey: &'a [u8],
    /// Expected device public key.
    pub device_pubkey: &'a [u8],
    /// Human-readable device name.
    pub device_name: &'a str,
    /// Platform label.
    pub platform: &'a str,
    /// Human proof event id.
    pub human_proof_event_id: &'a [u8],
    /// Device proof event id.
    pub device_proof_event_id: &'a [u8],
}

impl Db {
    /// Return whether a principal is an active, owner-associated MK service.
    ///
    /// Service identities bypass the physical-device gate but remain bounded
    /// by the event-level capability check on every write.
    pub async fn mkideas_active_service_identity(
        &self,
        community: CommunityId,
        service_pubkey: &[u8],
    ) -> Result<bool> {
        let active: bool = sqlx::query_scalar(
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
                  AND service_grant.revoked_at IS NULL
                  AND service_grant.expires_at > now()
            )
            "#,
        )
        .bind(community.as_uuid())
        .bind(service_pubkey)
        .fetch_one(&self.pool)
        .await?;
        Ok(active)
    }

    /// Persist a hash-only, short-lived enrollment challenge.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_mkideas_device_enrollment_challenge(
        &self,
        community: CommunityId,
        challenge_id: Uuid,
        human_pubkey: &[u8],
        device_pubkey: &[u8],
        issued_by: &[u8],
        challenge_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let lifetime = expires_at - Utc::now();
        if lifetime.num_seconds() <= 0 || lifetime.num_seconds() > 10 * 60 {
            return Err(DbError::InvalidData(
                "device enrollment challenge must expire within ten minutes".into(),
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO mk_device_enrollment_challenges (
                id, community_id, human_pubkey, device_pubkey,
                issued_by, challenge_hash, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(challenge_id)
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(device_pubkey)
        .bind(issued_by)
        .bind(challenge_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Consume one valid challenge and create an active grant atomically.
    pub async fn enroll_mkideas_device(
        &self,
        community: CommunityId,
        input: MkDeviceEnrollmentInput<'_>,
    ) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;
        let challenge = sqlx::query(
            r#"
            SELECT human_pubkey, device_pubkey
            FROM mk_device_enrollment_challenges
            WHERE id = $1 AND community_id = $2
              AND consumed_at IS NULL AND expires_at > now()
            FOR UPDATE
            "#,
        )
        .bind(input.challenge_id)
        .bind(community.as_uuid())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| DbError::AccessDenied("invalid or expired device challenge".into()))?;
        let challenged_human: Vec<u8> = challenge.try_get("human_pubkey")?;
        let challenged_device: Vec<u8> = challenge.try_get("device_pubkey")?;
        if challenged_human.as_slice() != input.human_pubkey
            || challenged_device.as_slice() != input.device_pubkey
        {
            return Err(DbError::AccessDenied(
                "device challenge is bound to another identity".into(),
            ));
        }

        let grant_id = Uuid::new_v4();
        let inserted = sqlx::query(
            r#"
            INSERT INTO mk_device_grants (
                id, community_id, human_pubkey, device_pubkey, issued_by,
                device_name, platform, enrollment_method,
                human_proof_event_id, device_proof_event_id
            )
            SELECT $1, c.community_id, c.human_pubkey, c.device_pubkey,
                   c.issued_by, $3, $4, 'two-key-proof', $5, $6
            FROM mk_device_enrollment_challenges c
            WHERE c.id = $2 AND c.community_id = $7
            ON CONFLICT (community_id, device_pubkey) DO NOTHING
            "#,
        )
        .bind(grant_id)
        .bind(input.challenge_id)
        .bind(input.device_name)
        .bind(input.platform)
        .bind(input.human_proof_event_id)
        .bind(input.device_proof_event_id)
        .bind(community.as_uuid())
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() != 1 {
            return Err(DbError::AccessDenied(
                "device already has a grant and must be revoked explicitly".into(),
            ));
        }
        sqlx::query(
            r#"
            UPDATE mk_device_enrollment_challenges
            SET consumed_at = now(), consumed_by_grant_id = $3
            WHERE id = $1 AND community_id = $2 AND consumed_at IS NULL
            "#,
        )
        .bind(input.challenge_id)
        .bind(community.as_uuid())
        .bind(grant_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(grant_id)
    }

    /// Return the state of an exact human/device grant inside one community.
    pub async fn mkideas_device_grant_status(
        &self,
        community: CommunityId,
        human_pubkey: &[u8],
        device_pubkey: &[u8],
    ) -> Result<MkDeviceGrantStatus> {
        let row = sqlx::query(
            r#"
            SELECT revoked_at, expires_at
            FROM mk_device_grants
            WHERE community_id = $1 AND human_pubkey = $2 AND device_pubkey = $3
            "#,
        )
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(device_pubkey)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(MkDeviceGrantStatus::Missing);
        };
        let revoked_at: Option<DateTime<Utc>> = row.try_get("revoked_at")?;
        if revoked_at.is_some() {
            return Ok(MkDeviceGrantStatus::Revoked);
        }
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at")?;
        if expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Ok(MkDeviceGrantStatus::Expired);
        }
        Ok(MkDeviceGrantStatus::Active)
    }

    /// Return the status of one exact grant/human/device tuple.
    pub async fn mkideas_device_grant_status_by_id(
        &self,
        community: CommunityId,
        grant_id: Uuid,
        human_pubkey: &[u8],
        device_pubkey: &[u8],
    ) -> Result<MkDeviceGrantStatus> {
        let row = sqlx::query(
            r#"
            SELECT revoked_at, expires_at
            FROM mk_device_grants
            WHERE id = $1 AND community_id = $2
              AND human_pubkey = $3 AND device_pubkey = $4
            "#,
        )
        .bind(grant_id)
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(device_pubkey)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(MkDeviceGrantStatus::Missing);
        };
        let revoked_at: Option<DateTime<Utc>> = row.try_get("revoked_at")?;
        if revoked_at.is_some() {
            return Ok(MkDeviceGrantStatus::Revoked);
        }
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at")?;
        if expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Ok(MkDeviceGrantStatus::Expired);
        }
        Ok(MkDeviceGrantStatus::Active)
    }

    /// Return whether a device key currently has an unrevoked, unexpired grant.
    pub async fn mkideas_device_grant_active(
        &self,
        community: CommunityId,
        human_pubkey: &[u8],
        device_pubkey: &[u8],
    ) -> Result<bool> {
        Ok(matches!(
            self.mkideas_device_grant_status(community, human_pubkey, device_pubkey)
                .await?,
            MkDeviceGrantStatus::Active
        ))
    }

    /// Record successful use of an active device grant.
    pub async fn touch_mkideas_device_grant(
        &self,
        community: CommunityId,
        device_pubkey: &[u8],
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE mk_device_grants
            SET last_seen_at = now()
            WHERE community_id = $1 AND device_pubkey = $2
              AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())
            "#,
        )
        .bind(community.as_uuid())
        .bind(device_pubkey)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Record use only when the complete grant binding is still active.
    pub async fn touch_mkideas_device_grant_by_id(
        &self,
        community: CommunityId,
        grant_id: Uuid,
        human_pubkey: &[u8],
        device_pubkey: &[u8],
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE mk_device_grants
            SET last_seen_at = now()
            WHERE id = $1 AND community_id = $2
              AND human_pubkey = $3 AND device_pubkey = $4
              AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())
            "#,
        )
        .bind(grant_id)
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(device_pubkey)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// List a human's devices in one community, including revoked history.
    pub async fn list_mkideas_device_grants(
        &self,
        community: CommunityId,
        human_pubkey: &[u8],
    ) -> Result<Vec<MkDeviceGrantRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, human_pubkey, device_pubkey, device_name, platform,
                   enrollment_method, issued_at, expires_at, last_seen_at,
                   revoked_at, revocation_reason, auth_epoch
            FROM mk_device_grants
            WHERE community_id = $1 AND human_pubkey = $2
            ORDER BY issued_at DESC, id
            "#,
        )
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(MkDeviceGrantRecord {
                    id: row.try_get("id")?,
                    human_pubkey: row.try_get("human_pubkey")?,
                    device_pubkey: row.try_get("device_pubkey")?,
                    device_name: row.try_get("device_name")?,
                    platform: row.try_get("platform")?,
                    enrollment_method: row.try_get("enrollment_method")?,
                    issued_at: row.try_get("issued_at")?,
                    expires_at: row.try_get("expires_at")?,
                    last_seen_at: row.try_get("last_seen_at")?,
                    revoked_at: row.try_get("revoked_at")?,
                    revocation_reason: row.try_get("revocation_reason")?,
                    auth_epoch: row.try_get("auth_epoch")?,
                })
            })
            .collect()
    }

    /// Revoke one device and return its human and device identities for exact
    /// live-session eviction.
    pub async fn revoke_mkideas_device_grant(
        &self,
        community: CommunityId,
        grant_id: Uuid,
        revoked_by: &[u8],
        reason: &str,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        let revoker_hex = hex::encode(revoked_by);
        let row = sqlx::query(
            r#"
            UPDATE mk_device_grants
            SET revoked_at = now(), revoked_by = $3, revocation_reason = $4,
                auth_epoch = auth_epoch + 1
            WHERE id = $1 AND community_id = $2 AND revoked_at IS NULL
              AND (
                  human_pubkey = $3 OR EXISTS (
                      SELECT 1 FROM relay_members
                      WHERE community_id = $2 AND pubkey = $5
                        AND role IN ('owner', 'admin')
                  )
              )
            RETURNING human_pubkey, device_pubkey
            "#,
        )
        .bind(grant_id)
        .bind(community.as_uuid())
        .bind(revoked_by)
        .bind(reason)
        .bind(revoker_hex)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            std::result::Result::<(Vec<u8>, Vec<u8>), sqlx::Error>::Ok((
                row.try_get("human_pubkey")?,
                row.try_get("device_pubkey")?,
            ))
        })
        .transpose()
        .map_err(DbError::from)
    }

    /// Create a surviving-device recovery request after verifying its active grant.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_mkideas_device_recovery_request(
        &self,
        community: CommunityId,
        request_id: Uuid,
        human_pubkey: &[u8],
        surviving_grant_id: Uuid,
        requested_device_pubkey: &[u8],
        challenge_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let lifetime = expires_at - Utc::now();
        if lifetime.num_seconds() <= 0 || lifetime.num_seconds() > 10 * 60 {
            return Err(DbError::InvalidData(
                "device recovery challenge must expire within ten minutes".into(),
            ));
        }
        if human_pubkey == requested_device_pubkey {
            return Err(DbError::InvalidData(
                "recovery device key must differ from the human key".into(),
            ));
        }
        let result = sqlx::query(
            r#"
            INSERT INTO mk_device_recovery_requests (
                id, community_id, human_pubkey, surviving_grant_id,
                requested_device_pubkey, challenge_hash, expires_at
            )
            SELECT $1, g.community_id, g.human_pubkey, g.id, $5, $6, $7
            FROM mk_device_grants g
            WHERE g.id = $4 AND g.community_id = $2 AND g.human_pubkey = $3
              AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at > now())
            "#,
        )
        .bind(request_id)
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(surviving_grant_id)
        .bind(requested_device_pubkey)
        .bind(challenge_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(DbError::AccessDenied(
                "surviving device grant is inactive or belongs to another community".into(),
            ));
        }
        Ok(())
    }

    /// Approve a recovery from its exact active surviving device and create the
    /// replacement grant in one transaction.
    #[allow(clippy::too_many_arguments)]
    pub async fn approve_mkideas_device_recovery(
        &self,
        community: CommunityId,
        request_id: Uuid,
        approving_device_pubkey: &[u8],
        device_name: &str,
        platform: &str,
        human_proof_event_id: &[u8],
        device_proof_event_id: &[u8],
    ) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT r.human_pubkey, r.requested_device_pubkey
            FROM mk_device_recovery_requests r
            JOIN mk_device_grants g
              ON g.id = r.surviving_grant_id
             AND g.community_id = r.community_id
             AND g.human_pubkey = r.human_pubkey
            WHERE r.id = $1 AND r.community_id = $2
              AND r.state = 'pending' AND r.expires_at > now()
              AND g.device_pubkey = $3 AND g.revoked_at IS NULL
              AND (g.expires_at IS NULL OR g.expires_at > now())
            FOR UPDATE OF r, g
            "#,
        )
        .bind(request_id)
        .bind(community.as_uuid())
        .bind(approving_device_pubkey)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            DbError::AccessDenied(
                "recovery requires its exact active surviving device grant".into(),
            )
        })?;
        let human_pubkey: Vec<u8> = row.try_get("human_pubkey")?;
        let requested_device_pubkey: Vec<u8> = row.try_get("requested_device_pubkey")?;
        let grant_id = Uuid::new_v4();
        let inserted = sqlx::query(
            r#"
            INSERT INTO mk_device_grants (
                id, community_id, human_pubkey, device_pubkey, issued_by,
                device_name, platform, enrollment_method,
                human_proof_event_id, device_proof_event_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'surviving-device', $8, $9)
            ON CONFLICT (community_id, device_pubkey) DO NOTHING
            "#,
        )
        .bind(grant_id)
        .bind(community.as_uuid())
        .bind(&human_pubkey)
        .bind(&requested_device_pubkey)
        .bind(approving_device_pubkey)
        .bind(device_name)
        .bind(platform)
        .bind(human_proof_event_id)
        .bind(device_proof_event_id)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() != 1 {
            return Err(DbError::AccessDenied(
                "requested recovery device already has a grant".into(),
            ));
        }
        sqlx::query(
            r#"
            UPDATE mk_device_recovery_requests
            SET state = 'consumed', decided_at = now(), decided_by = $3,
                resulting_grant_id = $4, decision_reason = 'approved by surviving device'
            WHERE id = $1 AND community_id = $2 AND state = 'pending'
            "#,
        )
        .bind(request_id)
        .bind(community.as_uuid())
        .bind(approving_device_pubkey)
        .bind(grant_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(grant_id)
    }

    /// Reject a pending recovery from the exact surviving device or an owner/admin.
    pub async fn reject_mkideas_device_recovery(
        &self,
        community: CommunityId,
        request_id: Uuid,
        decided_by: &[u8],
        reason: &str,
    ) -> Result<bool> {
        let decider_hex = hex::encode(decided_by);
        let result = sqlx::query(
            r#"
            UPDATE mk_device_recovery_requests r
            SET state = 'rejected', decided_at = now(), decided_by = $3,
                decision_reason = $4
            WHERE r.id = $1 AND r.community_id = $2 AND r.state = 'pending'
              AND r.expires_at > now()
              AND (
                  EXISTS (
                      SELECT 1 FROM mk_device_grants g
                      WHERE g.id = r.surviving_grant_id
                        AND g.community_id = r.community_id
                        AND g.human_pubkey = r.human_pubkey
                        AND g.device_pubkey = $3 AND g.revoked_at IS NULL
                        AND (g.expires_at IS NULL OR g.expires_at > now())
                  ) OR EXISTS (
                      SELECT 1 FROM relay_members m
                      WHERE m.community_id = $2 AND m.pubkey = $5
                        AND m.role IN ('owner', 'admin')
                  )
              )
            "#,
        )
        .bind(request_id)
        .bind(community.as_uuid())
        .bind(decided_by)
        .bind(reason)
        .bind(decider_hex)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Register immutable metadata for a client-encrypted NIP-49 bundle.
    #[allow(clippy::too_many_arguments)]
    pub async fn register_mkideas_recovery_bundle(
        &self,
        community: CommunityId,
        bundle_id: Uuid,
        human_pubkey: &[u8],
        object_key: &str,
        sha256: &[u8],
        size_bytes: i32,
        created_by_grant_id: Uuid,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            INSERT INTO mk_nip49_recovery_bundles (
                id, community_id, human_pubkey, object_key, sha256,
                size_bytes, created_by_grant_id
            )
            SELECT $1, g.community_id, g.human_pubkey, $4, $5, $6, g.id
            FROM mk_device_grants g
            WHERE g.id = $7 AND g.community_id = $2 AND g.human_pubkey = $3
              AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at > now())
            "#,
        )
        .bind(bundle_id)
        .bind(community.as_uuid())
        .bind(human_pubkey)
        .bind(object_key)
        .bind(sha256)
        .bind(size_bytes)
        .bind(created_by_grant_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(DbError::AccessDenied(
                "recovery bundle requires an active same-community device grant".into(),
            ));
        }
        Ok(())
    }

    /// Record an owner-authorized successor identity attribution.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_mkideas_identity_successor(
        &self,
        community: CommunityId,
        record_id: Uuid,
        predecessor_pubkey: &[u8],
        successor_pubkey: &[u8],
        authorized_by: &[u8],
        authorization_event_id: &[u8],
        incident_id: Uuid,
        reason: &str,
    ) -> Result<()> {
        let owner_hex = hex::encode(authorized_by);
        let result = sqlx::query(
            r#"
            INSERT INTO mk_identity_successors (
                id, community_id, predecessor_pubkey, successor_pubkey,
                authorized_by, authorization_event_id, incident_id, reason
            )
            SELECT $1, $2, $3, $4, $5, $6, $7, $8
            WHERE EXISTS (
                SELECT 1 FROM relay_members
                WHERE community_id = $2 AND pubkey = $9 AND role = 'owner'
            )
            "#,
        )
        .bind(record_id)
        .bind(community.as_uuid())
        .bind(predecessor_pubkey)
        .bind(successor_pubkey)
        .bind(authorized_by)
        .bind(authorization_event_id)
        .bind(incident_id)
        .bind(reason)
        .bind(owner_hex)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(DbError::AccessDenied(
                "successor identity attribution requires the community owner".into(),
            ));
        }
        Ok(())
    }
}
