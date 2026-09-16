use nostr::{EventBuilder, JsonUtil, Keys, Kind, Tag, ToBech32};
use sha2::{Digest, Sha256};

use crate::secret_store::SecretStore;

const DEVICE_KEY_PREFIX: &str = "mkideas-device-v1";

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
