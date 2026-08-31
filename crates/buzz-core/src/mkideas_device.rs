//! Staged MK Ideas device-grant and recovery wire contracts.
//!
//! This module is deliberately I/O-free. It validates the two-key enrollment
//! proof and recovery metadata that relay and clients exchange, but it never
//! stores a private key, passphrase, or decrypted recovery bundle.

use chrono::{DateTime, Utc};
use nostr::{Event, Kind, PublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

/// Maximum lifetime of an enrollment or recovery challenge.
pub const MAX_CHALLENGE_LIFETIME_SECS: i64 = 10 * 60;
/// Maximum accepted encrypted NIP-49 value length.
pub const MAX_NIP49_ENCODED_BYTES: usize = 4096;

/// Device operating system reported at enrollment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevicePlatform {
    /// Windows desktop.
    Windows,
    /// macOS desktop.
    Macos,
    /// Apple iOS or iPadOS.
    Ios,
    /// Android.
    Android,
    /// Linux desktop.
    Linux,
    /// Browser client.
    Web,
}

/// Canonical payload signed independently by a human identity and a device key.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceEnrollmentPayloadV1 {
    /// Contract version. Must be `1`.
    pub version: u16,
    /// Server-resolved community UUID.
    pub community_id: Uuid,
    /// Human Nostr pubkey as lowercase hex.
    pub human_pubkey: String,
    /// Independent device Nostr pubkey as lowercase hex.
    pub device_pubkey: String,
    /// SHA-256 of the server challenge as lowercase hex.
    pub challenge_hash: String,
    /// Human-readable device name.
    pub device_name: String,
    /// Device platform.
    pub platform: DevicePlatform,
    /// Challenge issuance time.
    pub issued_at: DateTime<Utc>,
    /// Challenge expiry time.
    pub expires_at: DateTime<Utc>,
}

/// Two-key proof used to consume an enrollment challenge.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceEnrollmentProofV1 {
    /// Canonical payload repeated outside signed events for routing.
    pub payload: DeviceEnrollmentPayloadV1,
    /// Kind-22242 event signed by the human identity.
    pub human_proof: Event,
    /// Kind-22242 event signed by the independent device key.
    pub device_proof: Event,
}

/// Metadata for an encrypted offline recovery bundle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Nip49RecoveryBundleDescriptorV1 {
    /// Contract version. Must be `1`.
    pub version: u16,
    /// Opaque bundle UUID.
    pub bundle_id: Uuid,
    /// Human identity protected by the bundle.
    pub human_pubkey: String,
    /// Private-media object key. The relay stores no ciphertext in an event.
    pub object_key: String,
    /// SHA-256 of the exact `ncryptsec1...` text bytes.
    pub sha256: String,
    /// Ciphertext byte length.
    pub size_bytes: u32,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// Owner-authorized successor attribution when the old key is unrecoverable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SuccessorIdentityAttributionV1 {
    /// Contract version. Must be `1`.
    pub version: u16,
    /// Community containing both identities.
    pub community_id: Uuid,
    /// Unrecoverable predecessor public key.
    pub predecessor_pubkey: String,
    /// Replacement public key.
    pub successor_pubkey: String,
    /// Owner public key authorizing the attribution link.
    pub authorized_by: String,
    /// Non-secret incident/reference identifier.
    pub incident_id: Uuid,
    /// Required human-readable reason.
    pub reason: String,
    /// Authorization time.
    pub created_at: DateTime<Utc>,
}

/// Owner-signed proof for a successor attribution record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuccessorIdentityProofV1 {
    /// Canonical attribution payload.
    pub attribution: SuccessorIdentityAttributionV1,
    /// Kind-22242 event signed by the configured owner.
    pub owner_proof: Event,
}

/// Device-contract validation failure.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum DeviceContractError {
    /// A version is unsupported.
    #[error("unsupported contract version")]
    UnsupportedVersion,
    /// A public key is invalid or non-canonical.
    #[error("invalid public key: {0}")]
    InvalidPubkey(&'static str),
    /// A challenge hash is malformed.
    #[error("invalid challenge hash")]
    InvalidChallengeHash,
    /// Challenge timing is invalid.
    #[error("invalid challenge lifetime")]
    InvalidChallengeLifetime,
    /// Challenge is expired.
    #[error("challenge expired")]
    ChallengeExpired,
    /// A signed proof is invalid.
    #[error("invalid {0} proof")]
    InvalidProof(&'static str),
    /// Signed content differs from the routed payload.
    #[error("signed enrollment payload mismatch")]
    PayloadMismatch,
    /// Device metadata is invalid.
    #[error("invalid device metadata")]
    InvalidDeviceMetadata,
    /// Recovery descriptor is invalid.
    #[error("invalid recovery bundle descriptor")]
    InvalidRecoveryDescriptor,
    /// NIP-49 ciphertext is malformed or has the wrong digest.
    #[error("invalid NIP-49 ciphertext")]
    InvalidNip49Ciphertext,
    /// Successor attribution is invalid.
    #[error("invalid successor attribution")]
    InvalidSuccessorAttribution,
}

/// Hash an enrollment challenge without retaining it in persistence or logs.
pub fn challenge_sha256(challenge: &[u8]) -> String {
    hex::encode(Sha256::digest(challenge))
}

/// Validate a complete two-key enrollment proof at `now`.
pub fn validate_device_enrollment_proof(
    proof: &DeviceEnrollmentProofV1,
    expected_community: Uuid,
    expected_challenge_hash: &str,
    now: DateTime<Utc>,
) -> Result<(), DeviceContractError> {
    let payload = &proof.payload;
    if payload.version != 1 {
        return Err(DeviceContractError::UnsupportedVersion);
    }
    if payload.community_id != expected_community {
        return Err(DeviceContractError::PayloadMismatch);
    }
    validate_lower_hex(&payload.challenge_hash, 64)
        .map_err(|_| DeviceContractError::InvalidChallengeHash)?;
    if payload.challenge_hash != expected_challenge_hash {
        return Err(DeviceContractError::InvalidChallengeHash);
    }
    let lifetime = payload.expires_at - payload.issued_at;
    if lifetime.num_seconds() <= 0 || lifetime.num_seconds() > MAX_CHALLENGE_LIFETIME_SECS {
        return Err(DeviceContractError::InvalidChallengeLifetime);
    }
    if now > payload.expires_at {
        return Err(DeviceContractError::ChallengeExpired);
    }
    if payload.device_name.trim().is_empty() || payload.device_name.chars().count() > 120 {
        return Err(DeviceContractError::InvalidDeviceMetadata);
    }

    let human = canonical_pubkey(&payload.human_pubkey, "human")?;
    let device = canonical_pubkey(&payload.device_pubkey, "device")?;
    if human == device {
        return Err(DeviceContractError::InvalidDeviceMetadata);
    }
    let canonical =
        serde_json::to_string(payload).map_err(|_| DeviceContractError::PayloadMismatch)?;
    validate_proof_event(&proof.human_proof, human, &canonical, "human")?;
    validate_proof_event(&proof.device_proof, device, &canonical, "device")?;
    Ok(())
}

/// Validate the encrypted NIP-49 text fetched from private media.
///
/// Decryption and passphrase handling remain client-side. This helper only
/// verifies the Bech32 prefix, a conservative size bound, and the immutable
/// descriptor digest.
pub fn validate_nip49_ciphertext(
    descriptor: &Nip49RecoveryBundleDescriptorV1,
    encoded: &str,
) -> Result<(), DeviceContractError> {
    validate_recovery_descriptor(descriptor)?;
    let bytes = encoded.as_bytes();
    if !encoded.starts_with("ncryptsec1")
        || bytes.len() > MAX_NIP49_ENCODED_BYTES
        || bytes.len() != descriptor.size_bytes as usize
        || challenge_sha256(bytes) != descriptor.sha256
    {
        return Err(DeviceContractError::InvalidNip49Ciphertext);
    }
    Ok(())
}

/// Validate recovery-bundle metadata before persistence.
pub fn validate_recovery_descriptor(
    descriptor: &Nip49RecoveryBundleDescriptorV1,
) -> Result<(), DeviceContractError> {
    if descriptor.version != 1
        || descriptor.bundle_id.is_nil()
        || canonical_pubkey(&descriptor.human_pubkey, "human").is_err()
        || descriptor.object_key.trim().is_empty()
        || descriptor.object_key.len() > 512
        || descriptor.object_key.contains("..")
        || descriptor.object_key.starts_with('/')
        || descriptor.size_bytes == 0
        || descriptor.size_bytes as usize > MAX_NIP49_ENCODED_BYTES
        || validate_lower_hex(&descriptor.sha256, 64).is_err()
    {
        return Err(DeviceContractError::InvalidRecoveryDescriptor);
    }
    Ok(())
}

/// Validate an owner-created successor attribution record.
pub fn validate_successor_attribution(
    attribution: &SuccessorIdentityAttributionV1,
) -> Result<(), DeviceContractError> {
    if attribution.version != 1
        || attribution.community_id.is_nil()
        || attribution.incident_id.is_nil()
        || attribution.reason.trim().len() < 8
        || attribution.reason.chars().count() > 500
    {
        return Err(DeviceContractError::InvalidSuccessorAttribution);
    }
    let predecessor = canonical_pubkey(&attribution.predecessor_pubkey, "predecessor")?;
    let successor = canonical_pubkey(&attribution.successor_pubkey, "successor")?;
    canonical_pubkey(&attribution.authorized_by, "owner")?;
    if predecessor == successor {
        return Err(DeviceContractError::InvalidSuccessorAttribution);
    }
    Ok(())
}

/// Validate the configured owner's signature over successor attribution.
pub fn validate_successor_identity_proof(
    proof: &SuccessorIdentityProofV1,
) -> Result<(), DeviceContractError> {
    validate_successor_attribution(&proof.attribution)?;
    let owner = canonical_pubkey(&proof.attribution.authorized_by, "owner")?;
    let canonical = serde_json::to_string(&proof.attribution)
        .map_err(|_| DeviceContractError::InvalidSuccessorAttribution)?;
    validate_proof_event(&proof.owner_proof, owner, &canonical, "owner")
        .map_err(|_| DeviceContractError::InvalidSuccessorAttribution)
}

fn canonical_pubkey(value: &str, label: &'static str) -> Result<PublicKey, DeviceContractError> {
    validate_lower_hex(value, 64).map_err(|_| DeviceContractError::InvalidPubkey(label))?;
    let key = PublicKey::from_hex(value).map_err(|_| DeviceContractError::InvalidPubkey(label))?;
    if key.to_hex() != value {
        return Err(DeviceContractError::InvalidPubkey(label));
    }
    Ok(key)
}

fn validate_proof_event(
    event: &Event,
    expected_pubkey: PublicKey,
    canonical_payload: &str,
    label: &'static str,
) -> Result<(), DeviceContractError> {
    if event.kind != Kind::Authentication
        || event.pubkey != expected_pubkey
        || event.content != canonical_payload
        || !event.verify_id()
        || !event.verify_signature()
    {
        return Err(DeviceContractError::InvalidProof(label));
    }
    Ok(())
}

fn validate_lower_hex(value: &str, length: usize) -> Result<(), ()> {
    if value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventBuilder, Keys};

    fn enrollment_fixture() -> (DeviceEnrollmentProofV1, Uuid, DateTime<Utc>) {
        let human = Keys::generate();
        let device = Keys::generate();
        let community = Uuid::new_v4();
        let issued_at = Utc::now();
        let payload = DeviceEnrollmentPayloadV1 {
            version: 1,
            community_id: community,
            human_pubkey: human.public_key().to_hex(),
            device_pubkey: device.public_key().to_hex(),
            challenge_hash: challenge_sha256(b"one-time challenge"),
            device_name: "William's Windows PC".into(),
            platform: DevicePlatform::Windows,
            issued_at,
            expires_at: issued_at + chrono::Duration::minutes(5),
        };
        let content = serde_json::to_string(&payload).expect("serialize payload");
        let human_proof = EventBuilder::new(Kind::Authentication, content.as_str())
            .tags([])
            .sign_with_keys(&human)
            .expect("sign human proof");
        let device_proof = EventBuilder::new(Kind::Authentication, content.as_str())
            .tags([])
            .sign_with_keys(&device)
            .expect("sign device proof");
        (
            DeviceEnrollmentProofV1 {
                payload,
                human_proof,
                device_proof,
            },
            community,
            issued_at,
        )
    }

    #[test]
    fn two_key_enrollment_proof_is_accepted() {
        let (proof, community, now) = enrollment_fixture();
        assert_eq!(
            validate_device_enrollment_proof(&proof, community, &proof.payload.challenge_hash, now,),
            Ok(())
        );
    }

    #[test]
    fn cross_community_enrollment_is_rejected() {
        let (proof, _community, now) = enrollment_fixture();
        assert_eq!(
            validate_device_enrollment_proof(
                &proof,
                Uuid::new_v4(),
                &proof.payload.challenge_hash,
                now,
            ),
            Err(DeviceContractError::PayloadMismatch)
        );
    }

    #[test]
    fn proof_cannot_be_rebound_to_another_device() {
        let (mut proof, community, now) = enrollment_fixture();
        proof.payload.device_pubkey = Keys::generate().public_key().to_hex();
        assert!(matches!(
            validate_device_enrollment_proof(&proof, community, &proof.payload.challenge_hash, now,),
            Err(DeviceContractError::InvalidProof("human"))
        ));
    }

    #[test]
    fn nip49_descriptor_verifies_exact_ciphertext() {
        let encoded = format!("ncryptsec1{}", "a".repeat(80));
        let descriptor = Nip49RecoveryBundleDescriptorV1 {
            version: 1,
            bundle_id: Uuid::new_v4(),
            human_pubkey: Keys::generate().public_key().to_hex(),
            object_key: "private/recovery/bundle.ncryptsec".into(),
            sha256: challenge_sha256(encoded.as_bytes()),
            size_bytes: encoded.len() as u32,
            created_at: Utc::now(),
        };
        assert_eq!(validate_nip49_ciphertext(&descriptor, &encoded), Ok(()));
        assert_eq!(
            validate_nip49_ciphertext(&descriptor, &(encoded + "tampered")),
            Err(DeviceContractError::InvalidNip49Ciphertext)
        );
    }

    #[test]
    fn successor_cannot_alias_predecessor() {
        let key = Keys::generate().public_key().to_hex();
        let attribution = SuccessorIdentityAttributionV1 {
            version: 1,
            community_id: Uuid::new_v4(),
            predecessor_pubkey: key.clone(),
            successor_pubkey: key,
            authorized_by: Keys::generate().public_key().to_hex(),
            incident_id: Uuid::new_v4(),
            reason: "Device lost and recovery bundle unavailable".into(),
            created_at: Utc::now(),
        };
        assert_eq!(
            validate_successor_attribution(&attribution),
            Err(DeviceContractError::InvalidSuccessorAttribution)
        );
    }

    #[test]
    fn successor_attribution_requires_owner_signature() {
        let owner = Keys::generate();
        let attribution = SuccessorIdentityAttributionV1 {
            version: 1,
            community_id: Uuid::new_v4(),
            predecessor_pubkey: Keys::generate().public_key().to_hex(),
            successor_pubkey: Keys::generate().public_key().to_hex(),
            authorized_by: owner.public_key().to_hex(),
            incident_id: Uuid::new_v4(),
            reason: "Previous identity cannot be recovered".into(),
            created_at: Utc::now(),
        };
        let content = serde_json::to_string(&attribution).expect("serialize attribution");
        let owner_proof = EventBuilder::new(Kind::Authentication, content)
            .tags([])
            .sign_with_keys(&owner)
            .expect("sign attribution");
        let proof = SuccessorIdentityProofV1 {
            attribution,
            owner_proof,
        };
        assert_eq!(validate_successor_identity_proof(&proof), Ok(()));
    }
}
