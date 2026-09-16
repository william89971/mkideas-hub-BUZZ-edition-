//! MK Ideas device enrollment, inventory, and revocation control plane.
//!
//! These endpoints require tenant-bound, replay-protected NIP-98. Enrollment
//! additionally requires an independently signed device proof, so possession
//! of the human identity alone cannot silently authorize a device key.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use base64::Engine;
use chrono::{Duration, Utc};
use nostr::{Event, Kind, PublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use buzz_core::TenantContext;
use buzz_db::device_grants::{MkDeviceEnrollmentInput, MkDeviceGrantRecord};

use crate::state::AppState;

use super::{api_error, bridge, internal_error};

const CHALLENGE_PATH: &str = "/api/devices/enrollment-challenges";
const ENROLL_PATH: &str = "/api/devices/enroll";
const INVENTORY_PATH: &str = "/api/devices";
const REVOKE_PATH: &str = "/api/devices/revoke";

#[derive(Debug, Deserialize)]
/// Request to create a short-lived enrollment challenge for one device key.
pub struct ChallengeRequest {
    device_pubkey: String,
}

#[derive(Debug, Serialize)]
/// Server-issued challenge and its complete tenant and identity binding.
pub struct ChallengeResponse {
    challenge_id: Uuid,
    challenge: String,
    expires_at: chrono::DateTime<Utc>,
    community_id: Uuid,
    human_pubkey: String,
    device_pubkey: String,
    relay_url: String,
}

#[derive(Debug, Deserialize)]
/// Human-authorized enrollment request carrying the independent device proof.
pub struct EnrollRequest {
    challenge_id: Uuid,
    challenge: String,
    device_name: String,
    platform: String,
    device_proof: Event,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct EnrollmentProofV1 {
    version: u16,
    action: String,
    community_id: Uuid,
    human_pubkey: String,
    device_pubkey: String,
    challenge_id: Uuid,
    challenge_hash: String,
    relay_url: String,
}

#[derive(Debug, Deserialize)]
/// Request to revoke one device grant owned by the authenticated human.
pub struct RevokeRequest {
    grant_id: Uuid,
    reason: String,
}

#[derive(Debug, Serialize)]
struct DeviceRecordResponse {
    id: Uuid,
    human_pubkey: String,
    device_pubkey: String,
    device_name: String,
    platform: String,
    enrollment_method: String,
    issued_at: chrono::DateTime<Utc>,
    expires_at: Option<chrono::DateTime<Utc>>,
    last_seen_at: Option<chrono::DateTime<Utc>>,
    revoked_at: Option<chrono::DateTime<Utc>>,
    revocation_reason: Option<String>,
    auth_epoch: i64,
}

impl From<MkDeviceGrantRecord> for DeviceRecordResponse {
    fn from(record: MkDeviceGrantRecord) -> Self {
        Self {
            id: record.id,
            human_pubkey: hex::encode(record.human_pubkey),
            device_pubkey: hex::encode(record.device_pubkey),
            device_name: record.device_name,
            platform: record.platform,
            enrollment_method: record.enrollment_method,
            issued_at: record.issued_at,
            expires_at: record.expires_at,
            last_seen_at: record.last_seen_at,
            revoked_at: record.revoked_at,
            revocation_reason: record.revocation_reason,
            auth_epoch: record.auth_epoch,
        }
    }
}

async fn authenticate(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
) -> Result<(TenantContext, PublicKey, [u8; 32]), (StatusCode, Json<Value>)> {
    let raw_host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let tenant = crate::tenant::bind_community(&state.db, raw_host)
        .await
        .map_err(|_| {
            api_error(
                StatusCode::NOT_FOUND,
                "relay: no community is configured for this host",
            )
        })?;
    let url = bridge::nip98_expected_url(&state.config.relay_url, &tenant, path);
    let (pubkey, event_id) =
        bridge::verify_bridge_auth_with_options(headers, method, &url, body, true, body.is_some())?;
    bridge::check_nip98_replay(state, &tenant, event_id).await?;
    bridge::enforce_http_admission(state, &tenant, &pubkey).await?;
    super::relay_members::enforce_relay_membership(
        state,
        tenant.community(),
        pubkey.as_bytes(),
        headers
            .get("x-buzz-auth-tag")
            .and_then(|value| value.to_str().ok()),
    )
    .await?;
    Ok((tenant, pubkey, event_id))
}

fn parse_device_pubkey(value: &str) -> Result<PublicKey, (StatusCode, Json<Value>)> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "device_pubkey must be canonical lowercase hex",
        ));
    }
    PublicKey::from_hex(value)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid device_pubkey"))
}

fn validate_label(value: &str, field: &str) -> Result<String, (StatusCode, Json<Value>)> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 100 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            &format!("{field} must be 1-100 characters"),
        ));
    }
    Ok(trimmed.to_string())
}

/// Create a ten-minute, single-use challenge bound to a human/device pair.
pub async fn create_challenge(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<ChallengeResponse>, (StatusCode, Json<Value>)> {
    let (tenant, human, _) =
        authenticate(&state, &headers, "POST", CHALLENGE_PATH, Some(&body)).await?;
    let request: ChallengeRequest = serde_json::from_slice(&body)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid challenge request JSON"))?;
    let device = parse_device_pubkey(&request.device_pubkey)?;
    if device == human {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "device key must differ from the human identity key",
        ));
    }
    let challenge_bytes = rand::random::<[u8; 32]>();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge_bytes);
    let challenge_hash = Sha256::digest(challenge.as_bytes());
    let challenge_id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::minutes(10);
    state
        .db
        .create_mkideas_device_enrollment_challenge(
            tenant.community(),
            challenge_id,
            human.as_bytes(),
            device.as_bytes(),
            human.as_bytes(),
            &challenge_hash,
            expires_at,
        )
        .await
        .map_err(|error| internal_error(&format!("create device challenge: {error}")))?;
    Ok(Json(ChallengeResponse {
        challenge_id,
        challenge,
        expires_at,
        community_id: *tenant.community().as_uuid(),
        human_pubkey: human.to_hex(),
        device_pubkey: device.to_hex(),
        relay_url: bridge::nip42_expected_relay_url(&state.config.relay_url, &tenant),
    }))
}

/// Consume a challenge after validating the independent device signature.
pub async fn enroll(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (tenant, human, human_proof_id) =
        authenticate(&state, &headers, "POST", ENROLL_PATH, Some(&body)).await?;
    let request: EnrollRequest = serde_json::from_slice(&body)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid enrollment request JSON"))?;
    if request.device_proof.kind != Kind::Authentication
        || !request.device_proof.verify_id()
        || !request.device_proof.verify_signature()
    {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "invalid device proof signature",
        ));
    }
    let device = request.device_proof.pubkey;
    if device == human {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "device key must differ from the human identity key",
        ));
    }
    let challenge_hash = Sha256::digest(request.challenge.as_bytes());
    let expected = EnrollmentProofV1 {
        version: 1,
        action: "enroll".into(),
        community_id: *tenant.community().as_uuid(),
        human_pubkey: human.to_hex(),
        device_pubkey: device.to_hex(),
        challenge_id: request.challenge_id,
        challenge_hash: hex::encode(challenge_hash),
        relay_url: bridge::nip42_expected_relay_url(&state.config.relay_url, &tenant),
    };
    let actual: EnrollmentProofV1 = serde_json::from_str(&request.device_proof.content)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid device proof payload"))?;
    if actual != expected {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "device proof binding mismatch",
        ));
    }
    let device_name = validate_label(&request.device_name, "device_name")?;
    let platform = validate_label(&request.platform, "platform")?;
    let device_proof_id = request.device_proof.id.to_bytes();
    let grant_id = state
        .db
        .enroll_mkideas_device(
            tenant.community(),
            MkDeviceEnrollmentInput {
                challenge_id: request.challenge_id,
                challenge_hash: &challenge_hash,
                human_pubkey: human.as_bytes(),
                device_pubkey: device.as_bytes(),
                device_name: &device_name,
                platform: &platform,
                human_proof_event_id: &human_proof_id,
                device_proof_event_id: &device_proof_id,
            },
        )
        .await
        .map_err(|error| match error {
            buzz_db::DbError::AccessDenied(message) => api_error(StatusCode::CONFLICT, &message),
            buzz_db::DbError::InvalidData(message) => api_error(StatusCode::BAD_REQUEST, &message),
            error => internal_error(&format!("enroll device: {error}")),
        })?;
    Ok(Json(
        serde_json::json!({ "grant_id": grant_id, "status": "active" }),
    ))
}

/// List the caller's current and revoked device grant history.
pub async fn inventory(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (tenant, human, _) = authenticate(&state, &headers, "GET", INVENTORY_PATH, None).await?;
    let devices: Vec<DeviceRecordResponse> = state
        .db
        .list_mkideas_device_grants(tenant.community(), human.as_bytes())
        .await
        .map_err(|error| internal_error(&format!("list devices: {error}")))?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(Json(serde_json::json!({ "devices": devices })))
}

/// Revoke one grant and disconnect only that device's live sessions.
pub async fn revoke(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (tenant, actor, _) =
        authenticate(&state, &headers, "POST", REVOKE_PATH, Some(&body)).await?;
    let request: RevokeRequest = serde_json::from_slice(&body)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid revocation request JSON"))?;
    let disconnected = crate::device_security::revoke_device_and_disconnect_sessions(
        &state,
        &tenant,
        request.grant_id,
        actor.as_bytes(),
        &request.reason,
    )
    .await
    .map_err(|error| match error {
        buzz_db::DbError::InvalidData(message) => api_error(StatusCode::BAD_REQUEST, &message),
        error => internal_error(&format!("revoke device: {error}")),
    })?;
    Ok(Json(serde_json::json!({
        "grant_id": request.grant_id,
        "status": "revoked",
        "sessions_disconnected": disconnected
    })))
}
