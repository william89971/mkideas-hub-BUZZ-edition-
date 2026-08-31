//! Staged MK Ideas device-grant authentication and revocation.
//!
//! Enforcement defaults to [`DeviceGrantMode::Off`]. `Enforce` cannot be
//! selected accidentally: startup requires closed membership, a configured
//! owner, and an explicit enrollment-complete acknowledgement. This module
//! intentionally exposes no enrollment HTTP endpoint; clients first need the
//! versioned two-key proof flow in `buzz-core` and locally enrolled grants.

use nostr::{Event, Kind, PublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use buzz_core::{CommunityId, TenantContext};
use buzz_db::device_grants::MkDeviceGrantStatus;
use buzz_db::Db;

use crate::state::AppState;

/// Device-grant rollout mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceGrantMode {
    /// Ignore device claims and preserve current authentication behavior.
    Off,
    /// Validate and measure claims without denying existing clients.
    Audit,
    /// Require an active, independently signed device grant.
    Enforce,
}

/// Validated staged rollout configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceGrantConfig {
    /// Selected rollout mode.
    pub mode: DeviceGrantMode,
}

/// Security-sensitive device operation for community-scoped rate limiting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceSecurityOperation {
    /// Creating or consuming an enrollment challenge.
    Enrollment,
    /// Creating or deciding a surviving-device recovery request.
    Recovery,
    /// Registering or fetching recovery-bundle metadata.
    RecoveryBundle,
}

/// Conservative default attempts per hour for a device-security operation.
pub const fn device_security_attempts_per_hour(operation: DeviceSecurityOperation) -> u32 {
    match operation {
        DeviceSecurityOperation::Enrollment => 10,
        DeviceSecurityOperation::Recovery => 5,
        DeviceSecurityOperation::RecoveryBundle => 10,
    }
}

/// Build a non-secret, community-fenced Redis rate-limit key.
///
/// `subject` may contain a pubkey, grant UUID, or source address. Only its hash
/// is emitted so operational logs and Redis inspection do not expose it.
pub fn device_security_rate_limit_key(
    community: CommunityId,
    operation: DeviceSecurityOperation,
    subject: &[u8],
) -> String {
    let operation = match operation {
        DeviceSecurityOperation::Enrollment => "enrollment",
        DeviceSecurityOperation::Recovery => "recovery",
        DeviceSecurityOperation::RecoveryBundle => "bundle",
    };
    let digest = Sha256::digest(subject);
    format!(
        "mk-device-security:{}:{operation}:{}",
        community,
        hex::encode(&digest[..16])
    )
}

impl DeviceGrantConfig {
    /// Parse configuration through an injected environment lookup.
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
        require_relay_membership: bool,
        relay_owner_configured: bool,
    ) -> Result<Self, String> {
        let raw = lookup("BUZZ_MK_DEVICE_GRANTS").unwrap_or_else(|| "off".into());
        let mode = match raw.trim().to_ascii_lowercase().as_str() {
            "" | "off" | "false" | "0" => DeviceGrantMode::Off,
            "audit" => DeviceGrantMode::Audit,
            "enforce" => DeviceGrantMode::Enforce,
            _ => return Err("BUZZ_MK_DEVICE_GRANTS must be one of off, audit, or enforce".into()),
        };
        if mode == DeviceGrantMode::Enforce {
            let enrollment_complete = lookup("BUZZ_MK_DEVICE_GRANTS_ENROLLMENT_COMPLETE")
                .is_some_and(|value| matches!(value.trim(), "true" | "1" | "on"));
            if !enrollment_complete {
                return Err(
                    "BUZZ_MK_DEVICE_GRANTS=enforce requires BUZZ_MK_DEVICE_GRANTS_ENROLLMENT_COMPLETE=true"
                        .into(),
                );
            }
            if !require_relay_membership {
                return Err(
                    "device-grant enforcement requires BUZZ_REQUIRE_RELAY_MEMBERSHIP=true".into(),
                );
            }
            if !relay_owner_configured {
                return Err(
                    "device-grant enforcement requires RELAY_OWNER_PUBKEY for recovery authority"
                        .into(),
                );
            }
        }
        Ok(Self { mode })
    }

    /// Whether this mode may deny authentication.
    pub const fn is_enforcing(self) -> bool {
        matches!(self.mode, DeviceGrantMode::Enforce)
    }
}

/// Independently device-signed session proof payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceSessionPayloadV1 {
    version: u16,
    community_id: Uuid,
    human_pubkey: String,
    device_pubkey: String,
    challenge_hash: String,
    relay_url: String,
}

/// A verified device claim extracted from a human-signed NIP-42 event.
#[derive(Clone, Debug)]
pub struct DeviceSessionClaim {
    /// Durable grant identifier.
    pub grant_id: Uuid,
    /// Independent device public key.
    pub device_pubkey: PublicKey,
}

/// Outcome of staged device admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceAuthDecision {
    /// Rollout is disabled.
    Bypassed,
    /// Audit mode observed an absent or invalid claim and allowed auth.
    AuditAllowed,
    /// An active grant was validated.
    Active(PublicKey),
    /// Authentication must be denied.
    Denied(&'static str),
}

/// Extract and validate the device proof carried by a NIP-42 AUTH event.
///
/// The tag shape is `mk-device`, grant UUID, device hex pubkey, serialized
/// kind-22242 device proof. The proof signs the exact challenge hash, relay,
/// human key, device key, and server-resolved community.
pub fn extract_device_session_claim(
    human_auth_event: &Event,
    tenant: &TenantContext,
    challenge: &str,
    expected_relay_url: &str,
) -> Result<Option<DeviceSessionClaim>, &'static str> {
    let mut tags = human_auth_event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("mk-device"));
    let Some(tag) = tags.next() else {
        return Ok(None);
    };
    if tags.next().is_some() {
        return Err("duplicate device claim");
    }
    let parts = tag.as_slice();
    if parts.len() != 4 {
        return Err("invalid device claim shape");
    }
    let grant_id = Uuid::parse_str(&parts[1]).map_err(|_| "invalid device grant id")?;
    let device_pubkey = canonical_pubkey(&parts[2])?;
    if device_pubkey == human_auth_event.pubkey {
        return Err("device key must differ from human key");
    }
    let proof: Event = serde_json::from_str(&parts[3]).map_err(|_| "invalid device proof JSON")?;
    if proof.kind != Kind::Authentication
        || proof.pubkey != device_pubkey
        || !proof.verify_id()
        || !proof.verify_signature()
    {
        return Err("invalid device proof signature");
    }
    let payload: DeviceSessionPayloadV1 =
        serde_json::from_str(&proof.content).map_err(|_| "invalid device proof payload")?;
    let expected = DeviceSessionPayloadV1 {
        version: 1,
        community_id: *tenant.community().as_uuid(),
        human_pubkey: human_auth_event.pubkey.to_hex(),
        device_pubkey: device_pubkey.to_hex(),
        challenge_hash: hex::encode(Sha256::digest(challenge.as_bytes())),
        relay_url: expected_relay_url.to_string(),
    };
    if payload != expected {
        return Err("device proof binding mismatch");
    }
    Ok(Some(DeviceSessionClaim {
        grant_id,
        device_pubkey,
    }))
}

/// Apply the rollout policy to a parsed claim.
pub async fn evaluate_device_auth(
    db: &Db,
    config: DeviceGrantConfig,
    community: CommunityId,
    human_pubkey: &[u8],
    claim: Result<Option<DeviceSessionClaim>, &'static str>,
) -> DeviceAuthDecision {
    if config.mode == DeviceGrantMode::Off {
        return DeviceAuthDecision::Bypassed;
    }
    match db
        .mkideas_active_service_identity(community, human_pubkey)
        .await
    {
        Ok(true) => return DeviceAuthDecision::Bypassed,
        Err(_) if config.mode == DeviceGrantMode::Audit => {
            return DeviceAuthDecision::AuditAllowed;
        }
        Err(_) => return DeviceAuthDecision::Denied("service identity check unavailable"),
        Ok(false) => {}
    }
    let claim = match claim {
        Ok(Some(claim)) => claim,
        Ok(None) | Err(_) if config.mode == DeviceGrantMode::Audit => {
            return DeviceAuthDecision::AuditAllowed;
        }
        Ok(None) => return DeviceAuthDecision::Denied("device grant required"),
        Err(_) => return DeviceAuthDecision::Denied("invalid device proof"),
    };
    let status = db
        .mkideas_device_grant_status_by_id(
            community,
            claim.grant_id,
            human_pubkey,
            claim.device_pubkey.as_bytes(),
        )
        .await;
    match status {
        Ok(MkDeviceGrantStatus::Active) => {
            let _ = db
                .touch_mkideas_device_grant_by_id(
                    community,
                    claim.grant_id,
                    human_pubkey,
                    claim.device_pubkey.as_bytes(),
                )
                .await;
            DeviceAuthDecision::Active(claim.device_pubkey)
        }
        Ok(_) if config.mode == DeviceGrantMode::Audit => DeviceAuthDecision::AuditAllowed,
        Ok(MkDeviceGrantStatus::Missing) => DeviceAuthDecision::Denied("device grant missing"),
        Ok(MkDeviceGrantStatus::Revoked) => DeviceAuthDecision::Denied("device grant revoked"),
        Ok(MkDeviceGrantStatus::Expired) => DeviceAuthDecision::Denied("device grant expired"),
        Err(_) if config.mode == DeviceGrantMode::Audit => DeviceAuthDecision::AuditAllowed,
        Err(_) => DeviceAuthDecision::Denied("device grant check unavailable"),
    }
}

/// Revoke a grant and terminate the human's active sessions cluster-wide.
///
/// Until the connection registry records independent device keys, this safely
/// closes all of the human's sessions. Durable grant state still prevents only
/// the revoked device from authenticating again; unaffected devices reconnect.
pub async fn revoke_device_and_disconnect_sessions(
    state: &AppState,
    tenant: &TenantContext,
    grant_id: Uuid,
    revoked_by: &[u8],
    reason: &str,
) -> Result<usize, buzz_db::DbError> {
    if reason.trim().len() < 8 || reason.chars().count() > 500 {
        return Err(buzz_db::DbError::InvalidData(
            "device revocation reason must be 8-500 characters".into(),
        ));
    }
    let Some(human_pubkey) = state
        .db
        .revoke_mkideas_device_grant(tenant.community(), grant_id, revoked_by, reason.trim())
        .await?
    else {
        return Ok(0);
    };
    let event_id = "0".repeat(64);
    Ok(state.disconnect_pubkey_clusterwide(
        tenant,
        &human_pubkey,
        &event_id,
        "auth-required: a device grant was revoked",
    ))
}

fn canonical_pubkey(value: &str) -> Result<PublicKey, &'static str> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("invalid device public key");
    }
    let key = PublicKey::from_hex(value).map_err(|_| "invalid device public key")?;
    if key.to_hex() != value {
        return Err("non-canonical device public key");
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventBuilder, Keys, Tag};

    fn claim_fixture(community: Uuid, challenge: &str, relay: &str) -> (Event, TenantContext) {
        let human = Keys::generate();
        let device = Keys::generate();
        let payload = DeviceSessionPayloadV1 {
            version: 1,
            community_id: community,
            human_pubkey: human.public_key().to_hex(),
            device_pubkey: device.public_key().to_hex(),
            challenge_hash: hex::encode(Sha256::digest(challenge.as_bytes())),
            relay_url: relay.into(),
        };
        let device_proof = EventBuilder::new(
            Kind::Authentication,
            serde_json::to_string(&payload).expect("serialize proof"),
        )
        .tags([])
        .sign_with_keys(&device)
        .expect("sign device proof");
        let grant_id = Uuid::new_v4().to_string();
        let device_pubkey = device.public_key().to_hex();
        let device_proof_json = serde_json::to_string(&device_proof).expect("serialize event");
        let tag = Tag::parse([
            "mk-device",
            grant_id.as_str(),
            device_pubkey.as_str(),
            device_proof_json.as_str(),
        ])
        .expect("device tag");
        let human_event = EventBuilder::new(Kind::Authentication, "")
            .tags([tag])
            .sign_with_keys(&human)
            .expect("sign human auth");
        (
            human_event,
            TenantContext::resolved(CommunityId::from_uuid(community), "hub.test"),
        )
    }

    #[test]
    fn enforcement_requires_explicit_enrollment_ack() {
        let lookup = |name: &str| match name {
            "BUZZ_MK_DEVICE_GRANTS" => Some("enforce".into()),
            _ => None,
        };
        assert!(DeviceGrantConfig::from_lookup(lookup, true, true).is_err());
    }

    #[test]
    fn enforcement_requires_closed_membership_and_owner() {
        let lookup = |name: &str| match name {
            "BUZZ_MK_DEVICE_GRANTS" => Some("enforce".into()),
            "BUZZ_MK_DEVICE_GRANTS_ENROLLMENT_COMPLETE" => Some("true".into()),
            _ => None,
        };
        assert!(DeviceGrantConfig::from_lookup(lookup, false, true).is_err());
        assert!(DeviceGrantConfig::from_lookup(lookup, true, false).is_err());
        assert_eq!(
            DeviceGrantConfig::from_lookup(lookup, true, true),
            Ok(DeviceGrantConfig {
                mode: DeviceGrantMode::Enforce
            })
        );
    }

    #[test]
    fn default_mode_is_disabled() {
        let config =
            DeviceGrantConfig::from_lookup(|_| None, false, false).expect("off default is valid");
        assert_eq!(config.mode, DeviceGrantMode::Off);
    }

    #[test]
    fn session_claim_is_bound_to_server_resolved_community() {
        let community_a = Uuid::new_v4();
        let (event, tenant_a) = claim_fixture(community_a, "challenge", "wss://hub.test");
        assert!(
            extract_device_session_claim(&event, &tenant_a, "challenge", "wss://hub.test")
                .expect("valid claim")
                .is_some()
        );

        let tenant_b =
            TenantContext::resolved(CommunityId::from_uuid(Uuid::new_v4()), "other.test");
        assert_eq!(
            extract_device_session_claim(&event, &tenant_b, "challenge", "wss://hub.test")
                .expect_err("cross-community replay rejected"),
            "device proof binding mismatch"
        );
    }

    #[test]
    fn session_claim_is_bound_to_exact_challenge_and_relay() {
        let (event, tenant) = claim_fixture(Uuid::new_v4(), "challenge", "wss://hub.test");
        assert!(extract_device_session_claim(&event, &tenant, "other", "wss://hub.test").is_err());
        assert!(
            extract_device_session_claim(&event, &tenant, "challenge", "wss://evil.test").is_err()
        );
    }

    #[test]
    fn rate_limit_keys_are_redacted_and_community_fenced() {
        let secret_subject = b"private-device-or-ip-identifier";
        let key_a = device_security_rate_limit_key(
            CommunityId::from_uuid(Uuid::new_v4()),
            DeviceSecurityOperation::Recovery,
            secret_subject,
        );
        let key_b = device_security_rate_limit_key(
            CommunityId::from_uuid(Uuid::new_v4()),
            DeviceSecurityOperation::Recovery,
            secret_subject,
        );
        assert_ne!(key_a, key_b);
        assert!(!key_a.contains("private-device"));
        assert_eq!(
            device_security_attempts_per_hour(DeviceSecurityOperation::Recovery),
            5
        );
    }
}
