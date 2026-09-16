use nostr::{EventBuilder, JsonUtil, Keys, Kind, Tag, ToBech32};
use sha2::{Digest, Sha256};

use crate::secret_store::SecretStore;
use serde::{Deserialize, Serialize};

const DEVICE_KEY_PREFIX: &str = "mkideas-device-v1";

#[derive(Serialize, Deserialize)]
struct SessionBinding {
    scope: String,
    community_id: String,
    grant_id: String,
    human_pubkey: String,
    relay_url: String,
}

fn binding_key(human: &str, relay: &str) -> String {
    format!(
        "mkideas-session-v1:{}",
        hex::encode(Sha256::digest(
            format!("{human}:{}", relay.trim_end_matches('/')).as_bytes()
        ))
    )
}

/// Persist the public enrollment receipt alongside the device key.
#[tauri::command]
pub fn remember_mkideas_device_grant(
    scope: String,
    community_id: String,
    grant_id: String,
    relay_url: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let human_pubkey = state.signing_keys()?.public_key().to_hex();
    if !scope.starts_with(&format!("{human_pubkey}:")) {
        return Err(
            "identity changed during device enrollment; retry for the active identity".into(),
        );
    }
    uuid::Uuid::parse_str(&community_id).map_err(|e| e.to_string())?;
    uuid::Uuid::parse_str(&grant_id).map_err(|e| e.to_string())?;
    let binding = SessionBinding {
        scope,
        community_id,
        grant_id,
        human_pubkey,
        relay_url,
    };
    let value = serde_json::to_string(&binding).map_err(|e| e.to_string())?;
    let store = SecretStore::shared("buzz-desktop");
    let key = binding_key(&binding.human_pubkey, &binding.relay_url);
    store.store(&key, &value)?;
    if !store.verify_stored_raw(&key, &value)? {
        return Err("device enrollment receipt could not be verified in secure storage".into());
    }
    Ok(())
}

/// Attach an enrolled device proof, without creating a replacement key on login.
pub(crate) fn session_tag(
    human: &str,
    relay: &str,
    challenge: &str,
) -> Result<Option<Tag>, String> {
    let store = SecretStore::shared("buzz-desktop");
    let Some(raw) = store.load(&binding_key(human, relay))? else {
        return Ok(None);
    };
    let binding: SessionBinding = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if binding.human_pubkey != human
        || binding.relay_url.trim_end_matches('/') != relay.trim_end_matches('/')
    {
        return Err("device enrollment does not match this identity and relay".into());
    }
    let raw_key = store
        .load(&storage_key(&binding.scope)?)?
        .ok_or("enrolled device key is unavailable; restore access before signing in")?;
    let keys = Keys::parse(raw_key.trim()).map_err(|e| e.to_string())?;
    build_session_tag(binding, &keys, human, challenge).map(Some)
}

fn build_session_tag(
    binding: SessionBinding,
    keys: &Keys,
    human: &str,
    challenge: &str,
) -> Result<Tag, String> {
    let payload = serde_json::json!({
        "version": 1,
        "community_id": binding.community_id,
        "human_pubkey": human,
        "device_pubkey": keys.public_key().to_hex(),
        "challenge_hash": hex::encode(Sha256::digest(challenge.as_bytes())),
        "relay_url": binding.relay_url,
    });
    let proof = EventBuilder::new(Kind::Authentication, payload.to_string())
        .sign_with_keys(keys)
        .map_err(|e| e.to_string())?;
    Tag::parse(vec![
        "mk-device".to_string(),
        binding.grant_id,
        keys.public_key().to_hex(),
        proof.as_json(),
    ])
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod session_tests {
    use super::*;

    #[test]
    fn session_proof_is_signed_by_device_and_bound_to_login() {
        let device = Keys::generate();
        let human = Keys::generate().public_key().to_hex();
        let binding = SessionBinding {
            scope: "test".into(),
            community_id: uuid::Uuid::new_v4().to_string(),
            grant_id: uuid::Uuid::new_v4().to_string(),
            human_pubkey: human.clone(),
            relay_url: "wss://hub.test".into(),
        };
        let grant = binding.grant_id.clone();
        let community = binding.community_id.clone();
        let tag = build_session_tag(binding, &device, &human, "fresh-challenge").expect("proof");
        let fields = tag.as_slice();
        assert_eq!(
            &fields[..3],
            &["mk-device".to_string(), grant, device.public_key().to_hex()]
        );
        let proof: nostr::Event = serde_json::from_str(&fields[3]).expect("signed event");
        assert!(proof.verify().is_ok());
        assert_eq!(proof.pubkey, device.public_key());
        assert_eq!(proof.kind, Kind::Authentication);
        let payload: serde_json::Value = serde_json::from_str(&proof.content).expect("payload");
        assert_eq!(payload["human_pubkey"], human);
        assert_eq!(payload["community_id"], community);
        assert_eq!(payload["relay_url"], "wss://hub.test");
        assert_eq!(
            payload["challenge_hash"],
            hex::encode(Sha256::digest(b"fresh-challenge"))
        );
    }

    #[test]
    fn receipt_storage_is_scoped_by_human_and_relay() {
        assert_eq!(
            binding_key("alice", "wss://a.test"),
            binding_key("alice", "wss://a.test/")
        );
        assert_ne!(
            binding_key("alice", "wss://a.test"),
            binding_key("bob", "wss://a.test")
        );
        assert_ne!(
            binding_key("alice", "wss://a.test"),
            binding_key("alice", "wss://b.test")
        );
    }
}

fn storage_key(scope: &str) -> Result<String, String> {
    let scope = scope.trim().to_ascii_lowercase();
    if scope.is_empty() || scope.len() > 512 {
        return Err("device key scope must be 1-512 characters".into());
    }
    Ok(format!(
        "{DEVICE_KEY_PREFIX}:{}",
        hex::encode(Sha256::digest(scope.as_bytes()))
    ))
}
fn load_or_create(scope: &str) -> Result<Keys, String> {
    let key = storage_key(scope)?;
    let store = SecretStore::shared("buzz-desktop");
    if let Some(nsec) = store.load(&key)? {
        return Keys::parse(nsec.trim())
            .map_err(|error| format!("stored device key is invalid: {error}"));
    }
    let keys = Keys::generate();
    let nsec = keys
        .secret_key()
        .to_bech32()
        .map_err(|error| format!("encode device key: {error}"))?;
    store.store(&key, &nsec)?;
    if !store.verify_stored_raw(&key, &nsec)? {
        let _ = store.delete(&key);
        return Err("device key keyring read-back verification failed".into());
    }
    Ok(keys)
}

/// Ensure this human/community scope has an independent key in the OS keyring.
#[tauri::command]
pub fn ensure_mkideas_device_identity(scope: String) -> Result<String, String> {
    Ok(load_or_create(&scope)?.public_key().to_hex())
}

/// Sign a device proof without exposing the device private key to the webview.
#[tauri::command]
pub async fn sign_mkideas_device_event(
    scope: String,
    kind: u16,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<String, String> {
    let keys = load_or_create(&scope)?;
    tauri::async_runtime::spawn_blocking(move || {
        let tags = tags
            .into_iter()
            .map(|tag| Tag::parse(tag).map_err(|error| format!("invalid tag: {error}")))
            .collect::<Result<Vec<_>, _>>()?;
        EventBuilder::new(Kind::Custom(kind), content)
            .tags(tags)
            .sign_with_keys(&keys)
            .map(|event| event.as_json())
            .map_err(|error| format!("sign device proof: {error}"))
    })
    .await
    .map_err(|error| format!("sign device proof task failed: {error}"))?
}
