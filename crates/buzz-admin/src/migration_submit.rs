//! Explicit-credential, relay-only submission of offline migration artifacts.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use buzz_core::mkideas_migration::{mkcc_destination_id, MkMigrationReceipt, MkMigrationSource};
use buzz_core::tenant::relay_url_authority;
use buzz_migration::bundle::MediaIntegrity;
use buzz_migration::dry_run::{
    OfflineDryRun, OfflinePlanningCheckpoint, PlannedMedia, PlannedRecord, PlannedRecordClass,
};
use buzz_migration::manifest::ExportManifest;
use buzz_migration::secrets::validate_no_secrets;
use chrono::{DateTime, Utc};
use nostr::{Event, EventBuilder, JsonUtil, Keys, Kind, Tag, Timestamp};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

pub(crate) struct DestinationReconcileOptions<'a> {
    pub(crate) relay: &'a str,
    pub(crate) service_key_file: &'a Path,
    pub(crate) auth_tag_file: Option<&'a Path>,
}

#[derive(Debug)]
pub(crate) struct DestinationReconcileResult {
    pub(crate) receipt_count: usize,
    pub(crate) mismatches: Vec<String>,
}

pub(crate) struct SubmitOptions {
    pub(crate) artifact: PathBuf,
    pub(crate) bundle: PathBuf,
    pub(crate) checkpoint: Option<PathBuf>,
    pub(crate) relay: String,
    pub(crate) service_key_file: PathBuf,
    pub(crate) auth_tag_file: Option<PathBuf>,
    pub(crate) checkpoint_out: PathBuf,
}

pub(crate) async fn submit_dry_run(options: SubmitOptions) -> Result<i32> {
    if options.checkpoint_out == options.artifact
        || options.checkpoint_out == options.service_key_file
        || options
            .auth_tag_file
            .as_ref()
            .is_some_and(|path| path == &options.checkpoint_out)
    {
        return Err(anyhow!(
            "checkpoint output must not overwrite the artifact or credential files"
        ));
    }
    let artifact: OfflineDryRun = read_secret_free_json(&options.artifact, "dry-run artifact")?;
    artifact
        .plan
        .ordered_units()
        .context("dry-run artifact plan is invalid")?;
    if artifact.records.len() != artifact.plan.units.len() {
        return Err(anyhow!(
            "dry-run artifact record count does not match plan unit count"
        ));
    }
    validate_bundle_identity(&options.bundle, &artifact)?;
    let relay = explicit_ws_url(&options.relay)?;
    if relay_url_authority(&relay) != artifact.plan.community {
        return Err(anyhow!(
            "explicit relay host does not match dry-run community {}",
            artifact.plan.community
        ));
    }
    let keys = read_service_keys(&options.service_key_file)?;
    let auth_tag = options
        .auth_tag_file
        .as_deref()
        .map(read_auth_tag)
        .transpose()?;
    let start = if let Some(path) = options.checkpoint.as_deref() {
        let checkpoint: OfflinePlanningCheckpoint = read_secret_free_json(path, "checkpoint")?;
        artifact
            .validate_checkpoint(&checkpoint)
            .context("checkpoint does not belong to this artifact")?;
        usize::try_from(checkpoint.next_unit_index)
            .context("checkpoint index does not fit this platform")?
    } else {
        0
    };

    let mut connection =
        buzz_ws_client::NostrWsConnection::connect_authenticated(&relay, &keys, auth_tag.as_ref())
            .await
            .context("failed to authenticate the explicit migration service")?;
    let mut signed_by_source = BTreeMap::<String, Event>::new();
    for record in artifact.records.iter().take(start) {
        if record.action == buzz_migration::plan::PlannedAction::Create
            && record.class != PlannedRecordClass::MediaReceipt
        {
            let imported = sign_imported_record(record, &keys, &signed_by_source)?;
            signed_by_source.insert(record.source.tag_value(), imported);
        }
    }

    let mut submitted = 0_u64;
    let mut skipped = 0_u64;
    for (index, record) in artifact.records.iter().enumerate().skip(start) {
        if record.action == buzz_migration::plan::PlannedAction::Create {
            let prepared = if record.class == PlannedRecordClass::MediaReceipt {
                let media = artifact
                    .media
                    .iter()
                    .find(|media| media.media_id == record.record_id)
                    .ok_or_else(|| {
                        anyhow!("media record {} has no planned media", record.source)
                    })?;
                let descriptor =
                    upload_planned_media(&options.bundle, media, &relay, &keys, auth_tag.as_ref())
                        .await
                        .with_context(|| format!("media upload failed for {}", record.source))?;
                with_uploaded_media(record, &descriptor)?
            } else {
                record.clone()
            };
            let imported = sign_imported_record(&prepared, &keys, &signed_by_source)?;
            let receipt = sign_receipt(&artifact, &prepared, imported.clone(), &keys)?;
            let response = connection
                .send_event(receipt)
                .await
                .with_context(|| format!("relay submission failed for {}", record.source))?;
            if !response.accepted {
                return Err(anyhow!(
                    "relay rejected {}: {}",
                    record.source,
                    response.message
                ));
            }
            if record.class != PlannedRecordClass::MediaReceipt {
                signed_by_source.insert(record.source.tag_value(), imported);
            }
            submitted = submitted.saturating_add(1);
        } else {
            skipped = skipped.saturating_add(1);
        }
        write_checkpoint(&options.checkpoint_out, &artifact.checkpoint_at(index + 1)?)?;
    }
    if let Err(error) = connection.disconnect().await {
        eprintln!("warning: migration relay disconnect failed: {error}");
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "operation": if start == 0 { "apply" } else { "resume" },
            "valid": true,
            "relay": relay,
            "community": artifact.plan.community,
            "dataset_sha256": artifact.plan.dataset_sha256,
            "batch_id": artifact.plan.batch_id,
            "starting_index": start,
            "next_unit_index": artifact.plan.units.len(),
            "submitted_units": submitted,
            "non_create_units": skipped,
            "checkpoint": options.checkpoint_out,
            "source_contacted": false
        }))?
    );
    Ok(0)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MigrationBlobDescriptor {
    url: String,
    sha256: String,
    size: u64,
    #[serde(rename = "type")]
    mime_type: String,
}

fn validate_bundle_identity(bundle: &Path, artifact: &OfflineDryRun) -> Result<()> {
    let manifest: ExportManifest =
        read_secret_free_json(&bundle.join("manifest.json"), "export manifest")?;
    manifest.validate().context("export manifest is invalid")?;
    let workspace_matches = artifact
        .records
        .iter()
        .all(|record| record.source.workspace_id == manifest.source_workspace_id);
    if manifest.dataset_sha256 != artifact.plan.dataset_sha256 || !workspace_matches {
        return Err(anyhow!(
            "bundle identity does not match the dry-run artifact"
        ));
    }
    Ok(())
}

async fn upload_planned_media(
    bundle: &Path,
    media: &PlannedMedia,
    relay: &str,
    keys: &Keys,
    auth_tag: Option<&Tag>,
) -> Result<MigrationBlobDescriptor> {
    if media.integrity != MediaIntegrity::Ready {
        return Err(anyhow!("planned media is not marked ready"));
    }
    let path = safe_bundle_media_path(bundle, &media.file)?;
    let metadata = fs::metadata(&path).with_context(|| format!("read media metadata {path:?}"))?;
    if !metadata.is_file() || metadata.len() != media.size {
        return Err(anyhow!(
            "media size mismatch: expected {}, found {}",
            media.size,
            metadata.len()
        ));
    }
    let bytes = fs::read(&path).with_context(|| format!("read migration media {path:?}"))?;
    let actual_sha256 = hex::encode(Sha256::digest(&bytes));
    if actual_sha256 != media.sha256.to_ascii_lowercase() {
        return Err(anyhow!(
            "media SHA-256 mismatch: expected {}, found {actual_sha256}",
            media.sha256
        ));
    }

    let base = explicit_http_url(relay)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("build migration media client")?;
    let primary = format!("{base}/upload");
    let mut response =
        send_media_upload(&client, &primary, &base, &bytes, media, keys, auth_tag).await?;
    if matches!(
        response.status(),
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::METHOD_NOT_ALLOWED
    ) {
        let legacy = format!("{base}/media/upload");
        response =
            send_media_upload(&client, &legacy, &base, &bytes, media, keys, auth_tag).await?;
    }
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("relay media upload returned {status}: {body}"));
    }
    let descriptor: MigrationBlobDescriptor = response
        .json()
        .await
        .context("relay returned an invalid media descriptor")?;
    validate_uploaded_descriptor(media, &descriptor)?;
    Ok(descriptor)
}

async fn send_media_upload(
    client: &reqwest::Client,
    url: &str,
    relay_http_url: &str,
    bytes: &[u8],
    media: &PlannedMedia,
    keys: &Keys,
    auth_tag: Option<&Tag>,
) -> Result<reqwest::Response> {
    let authorization = sign_blossom_upload(keys, &media.sha256, relay_http_url)?;
    let mut request = client
        .put(url)
        .header("Authorization", authorization)
        .header("Content-Type", &media.mime_type)
        .header("X-SHA-256", &media.sha256)
        .body(bytes.to_vec());
    if let Some(tag) = auth_tag {
        request = request.header("x-auth-tag", serde_json::to_string(tag)?);
    }
    request.send().await.context("send migration media upload")
}

fn sign_blossom_upload(keys: &Keys, sha256: &str, relay_url: &str) -> Result<String> {
    let expiration = Timestamp::now().as_secs().saturating_add(3600).to_string();
    let mut tags = vec![
        Tag::parse(["t", "upload"])?,
        Tag::parse(["x", sha256])?,
        Tag::parse(["expiration", expiration.as_str()])?,
    ];
    let authority = relay_url_authority(relay_url);
    if !authority.is_empty() {
        tags.push(Tag::parse(["server", authority.as_str()])?);
    }
    let event = EventBuilder::new(Kind::Custom(24242), "Upload migration media")
        .tags(tags)
        .sign_with_keys(keys)
        .context("sign Blossom upload authorization")?;
    Ok(format!(
        "Nostr {}",
        URL_SAFE_NO_PAD.encode(event.as_json().as_bytes())
    ))
}

fn safe_bundle_media_path(bundle: &Path, relative: &str) -> Result<PathBuf> {
    buzz_migration::manifest::validate_relative_path(relative)
        .context("migration media path is unsafe")?;
    let canonical_bundle = fs::canonicalize(bundle).context("resolve migration bundle")?;
    let candidate = canonical_bundle.join(relative);
    let canonical_candidate = fs::canonicalize(&candidate)
        .with_context(|| format!("resolve migration media {candidate:?}"))?;
    if !canonical_candidate.starts_with(&canonical_bundle) {
        return Err(anyhow!("migration media resolves outside the bundle"));
    }
    Ok(canonical_candidate)
}

fn validate_uploaded_descriptor(
    media: &PlannedMedia,
    descriptor: &MigrationBlobDescriptor,
) -> Result<()> {
    if !descriptor.sha256.eq_ignore_ascii_case(&media.sha256)
        || descriptor.size != media.size
        || descriptor.mime_type != media.mime_type
    {
        return Err(anyhow!(
            "relay media descriptor does not match the planned hash, size, and MIME type"
        ));
    }
    let parsed = url::Url::parse(&descriptor.url).context("invalid relay media URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(anyhow!("relay media descriptor URL must use HTTP or HTTPS"));
    }
    Ok(())
}

fn with_uploaded_media(
    record: &PlannedRecord,
    descriptor: &MigrationBlobDescriptor,
) -> Result<PlannedRecord> {
    let mut prepared = record.clone();
    let object = prepared
        .content
        .as_object_mut()
        .ok_or_else(|| anyhow!("media record content must be a JSON object"))?;
    object.insert("blob".to_string(), serde_json::to_value(descriptor)?);
    let migration = object
        .get_mut("migration")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| anyhow!("media record is missing migration provenance"))?;
    migration.insert("dry_run_only".to_string(), serde_json::Value::Bool(false));
    Ok(prepared)
}

fn explicit_http_url(raw: &str) -> Result<String> {
    let mut parsed = url::Url::parse(raw).context("invalid explicit relay URL")?;
    let scheme = match parsed.scheme() {
        "http" | "https" => parsed.scheme().to_string(),
        "ws" => "http".to_string(),
        "wss" => "https".to_string(),
        _ => return Err(anyhow!("relay URL must use ws, wss, http, or https")),
    };
    parsed
        .set_scheme(&scheme)
        .map_err(|_| anyhow!("could not normalize relay HTTP URL scheme"))?;
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

pub(crate) async fn reconcile_destination(
    plan: &buzz_migration::plan::MigrationPlan,
    options: DestinationReconcileOptions<'_>,
) -> Result<DestinationReconcileResult> {
    let keys = read_service_keys(options.service_key_file)?;
    let auth_tag = options.auth_tag_file.map(read_auth_tag).transpose()?;
    let base = explicit_http_url(options.relay)?;
    if relay_url_authority(&base) != plan.community {
        return Err(anyhow!(
            "explicit relay host does not match migration community {}",
            plan.community
        ));
    }
    let query_url = format!("{base}/query");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("build destination reconciliation client")?;
    let batch = plan.batch_id.to_string();
    let mut cursor: Option<String> = None;
    let mut receipts = Vec::new();
    loop {
        let mut filter = json!({
            "kinds": [buzz_core::kind::KIND_MK_MIGRATION_RECEIPT],
            "#h": [plan.community],
            "#dataset": [plan.dataset_sha256],
            "#batch": [batch],
            "limit": 200,
            "mk_projection": "operations"
        });
        if let Some(value) = cursor.as_deref() {
            filter
                .as_object_mut()
                .ok_or_else(|| anyhow!("migration receipt filter is not an object"))?
                .insert("mk_cursor".to_string(), json!(value));
        }
        let body = serde_json::to_vec(&[filter])?;
        let authorization = sign_nip98(&keys, "POST", &query_url, &body)?;
        let mut request = client
            .post(&query_url)
            .header("Authorization", authorization)
            .header("Content-Type", "application/json")
            .body(body);
        if let Some(tag) = auth_tag.as_ref() {
            request = request.header("x-auth-tag", serde_json::to_string(tag)?);
        }
        let response = request
            .send()
            .await
            .context("query migration receipts from destination relay")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "destination receipt query returned {status}: {body}"
            ));
        }
        let page: ProjectionPage = response
            .json()
            .await
            .context("destination receipt query returned invalid JSON")?;
        receipts.extend(page.events);
        match page.next_cursor {
            Some(next) if cursor.as_deref() != Some(next.as_str()) => cursor = Some(next),
            Some(_) => return Err(anyhow!("destination receipt query cursor did not advance")),
            None => break,
        }
    }
    Ok(compare_destination_receipts(plan, &receipts))
}

#[derive(Debug, Deserialize)]
struct ProjectionPage {
    events: Vec<Event>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

fn sign_nip98(keys: &Keys, method: &str, url: &str, body: &[u8]) -> Result<String> {
    use base64::engine::general_purpose::STANDARD;

    let payload = hex::encode(Sha256::digest(body));
    let nonce = uuid::Uuid::new_v4().to_string();
    let event = EventBuilder::new(Kind::Custom(27235), "")
        .tags([
            Tag::parse(["u", url])?,
            Tag::parse(["method", method])?,
            Tag::parse(["nonce", nonce.as_str()])?,
            Tag::parse(["payload", payload.as_str()])?,
        ])
        .sign_with_keys(keys)
        .context("sign migration query authorization")?;
    Ok(format!("Nostr {}", STANDARD.encode(event.as_json())))
}

fn compare_destination_receipts(
    plan: &buzz_migration::plan::MigrationPlan,
    events: &[Event],
) -> DestinationReconcileResult {
    let expected = plan
        .units
        .iter()
        .filter(|unit| unit.action == buzz_migration::plan::PlannedAction::Create)
        .map(|unit| (unit.source.tag_value(), unit))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::<String, &MkMigrationReceipt>::new();
    let mut parsed = Vec::<MkMigrationReceipt>::new();
    let mut mismatches = Vec::new();

    for event in events {
        match buzz_core::mkideas_migration::validate_migration_receipt(event) {
            Ok(envelope) => parsed.push(envelope.receipt),
            Err(error) => mismatches.push(format!(
                "invalid destination receipt {}: {error}",
                event.id.to_hex()
            )),
        }
    }
    for receipt in &parsed {
        let source = receipt.source.tag_value();
        if actual.insert(source.clone(), receipt).is_some() {
            mismatches.push(format!("duplicate destination receipt for {source}"));
        }
    }
    for (source, unit) in &expected {
        let Some(receipt) = actual.get(source) else {
            mismatches.push(format!("missing destination receipt for {source}"));
            continue;
        };
        let destination = unit.destination.as_ref();
        if receipt.dataset_sha256 != plan.dataset_sha256
            || receipt.batch_id != plan.batch_id
            || receipt.source_sha256 != unit.source_sha256
            || destination.is_none_or(|value| {
                receipt.target_kind != value.kind || receipt.destination_d != value.d_tag
            })
        {
            mismatches.push(format!(
                "destination receipt does not match plan for {source}"
            ));
        }
    }
    for source in actual.keys() {
        if !expected.contains_key(source) {
            mismatches.push(format!("unexpected destination receipt for {source}"));
        }
    }
    mismatches.sort();
    DestinationReconcileResult {
        receipt_count: events.len(),
        mismatches,
    }
}

fn sign_imported_record(
    record: &PlannedRecord,
    keys: &Keys,
    signed_by_source: &BTreeMap<String, Event>,
) -> Result<Event> {
    let destination = record
        .destination
        .as_ref()
        .ok_or_else(|| anyhow!("create record {} has no destination", record.source))?;
    let mut tags = Vec::new();
    for raw in &record.tags {
        if raw.first().map(String::as_str) == Some("prev_source") {
            let source = raw
                .get(1)
                .ok_or_else(|| anyhow!("prev_source tag has no coordinate"))?;
            let previous = signed_by_source.get(source).ok_or_else(|| {
                anyhow!(
                    "previous source {source} was not signed before {}",
                    record.source
                )
            })?;
            let previous_id = previous.id.to_hex();
            tags.push(Tag::parse(["prev", previous_id.as_str()])?);
        } else {
            tags.push(Tag::parse(raw.clone())?);
        }
    }
    EventBuilder::new(
        Kind::Custom(u16::try_from(destination.kind).context("event kind exceeds u16")?),
        serde_json::to_string(&record.content)?,
    )
    .tags(tags)
    .custom_created_at(deterministic_timestamp(record))
    .sign_with_keys(keys)
    .context("failed to sign imported event")
}

fn sign_receipt(
    artifact: &OfflineDryRun,
    record: &PlannedRecord,
    imported_event: Event,
    keys: &Keys,
) -> Result<Event> {
    let destination = record
        .destination
        .as_ref()
        .ok_or_else(|| anyhow!("create record {} has no destination", record.source))?;
    let source = MkMigrationSource {
        system: record.source.system.clone(),
        workspace_id: record.source.workspace_id,
        unit_kind: record.source.unit_kind.clone(),
        unit_id: record.source.unit_id.clone(),
        revision: record.source.revision,
    };
    let source_tag = source.tag_value();
    let receipt = MkMigrationReceipt {
        schema_version: 2,
        community: artifact.plan.community.clone(),
        dataset_sha256: artifact.plan.dataset_sha256.clone(),
        batch_id: artifact.plan.batch_id,
        receipt_id: mkcc_destination_id(source.workspace_id, "receipt", &source_tag),
        source,
        source_sha256: record.source_sha256.clone(),
        record_id: record.record_id,
        target_kind: destination.kind,
        destination_d: destination.d_tag,
        imported_event,
    };
    let batch = receipt.batch_id.to_string();
    let imported_id = receipt.imported_event.id.to_hex();
    let tags = [
        Tag::parse(["h", receipt.community.as_str()])?,
        Tag::parse(["dataset", receipt.dataset_sha256.as_str()])?,
        Tag::parse(["batch", batch.as_str()])?,
        Tag::parse(["source", source_tag.as_str()])?,
        Tag::parse(["e", imported_id.as_str()])?,
    ];
    EventBuilder::new(
        Kind::Custom(buzz_core::kind::KIND_MK_MIGRATION_RECEIPT as u16),
        serde_json::to_string(&receipt)?,
    )
    .tags(tags)
    .custom_created_at(Timestamp::from(
        deterministic_timestamp(record).as_secs().saturating_add(1),
    ))
    .sign_with_keys(keys)
    .context("failed to sign migration receipt")
}

fn deterministic_timestamp(record: &PlannedRecord) -> Timestamp {
    if let Some(timestamp) = record.source_created_at.as_deref().and_then(|value| {
        DateTime::parse_from_rfc3339(value)
            .ok()
            .and_then(|parsed| u64::try_from(parsed.with_timezone(&Utc).timestamp()).ok())
    }) {
        return Timestamp::from(timestamp);
    }
    let digest = Sha256::digest(record.source_sha256.as_bytes());
    let offset = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as u64;
    Timestamp::from(1_577_836_800_u64.saturating_add(offset % 315_532_800))
}

fn explicit_ws_url(raw: &str) -> Result<String> {
    let mut parsed = url::Url::parse(raw).context("invalid explicit relay URL")?;
    let scheme = match parsed.scheme() {
        "ws" | "wss" => parsed.scheme().to_string(),
        "http" => "ws".to_string(),
        "https" => "wss".to_string(),
        _ => return Err(anyhow!("relay URL must use ws, wss, http, or https")),
    };
    parsed
        .set_scheme(&scheme)
        .map_err(|_| anyhow!("could not normalize relay URL scheme"))?;
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

fn read_service_keys(path: &Path) -> Result<Keys> {
    let metadata = fs::metadata(path).with_context(|| format!("read key file {path:?}"))?;
    if metadata.len() == 0 || metadata.len() > 512 {
        return Err(anyhow!("service key file must contain 1-512 bytes"));
    }
    let secret = fs::read_to_string(path).with_context(|| format!("read key file {path:?}"))?;
    Keys::parse(secret.trim()).context("service key file is not a valid Nostr secret key")
}

fn read_auth_tag(path: &Path) -> Result<Tag> {
    let metadata = fs::metadata(path).with_context(|| format!("read auth tag {path:?}"))?;
    if metadata.len() == 0 || metadata.len() > 16 * 1024 {
        return Err(anyhow!("auth tag file must contain 1-16384 bytes"));
    }
    serde_json::from_slice(&fs::read(path)?).context("auth tag file is not a valid Nostr tag")
}

fn write_checkpoint(path: &Path, checkpoint: &OfflinePlanningCheckpoint) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(checkpoint)?;
    fs::write(path, bytes).with_context(|| format!("write checkpoint {path:?}"))
}

fn read_secret_free_json<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {label} {path:?}"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {label} {path:?}"))?;
    validate_no_secrets(&value).with_context(|| format!("{label} contains a secret-like field"))?;
    serde_json::from_value(value).with_context(|| format!("invalid {label} schema"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use buzz_migration::dry_run::build_offline_dry_run;
    use buzz_migration::mapping::MigrationMapping;

    #[test]
    fn synthetic_dry_run_signs_as_valid_atomic_receipts() {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/mkideas-v0/migration");
        let mapping: MigrationMapping =
            serde_json::from_slice(&fs::read(root.join("mapping.example.json")).expect("mapping"))
                .expect("mapping schema");
        let artifact = build_offline_dry_run(&root, &mapping).expect("synthetic dry-run");
        let keys = Keys::generate();
        let mut signed = BTreeMap::<String, Event>::new();
        let mut receipts = Vec::new();
        for record in &artifact.records {
            if record.action != buzz_migration::plan::PlannedAction::Create {
                continue;
            }
            let imported = sign_imported_record(record, &keys, &signed).expect("import event");
            let receipt = sign_receipt(&artifact, record, imported.clone(), &keys)
                .expect("migration receipt");
            buzz_core::mkideas_migration::validate_migration_receipt(&receipt)
                .expect("valid migration pair");
            receipts.push(receipt);
            signed.insert(record.source.tag_value(), imported);
        }
        assert_eq!(signed.len(), 39);
        let reconciled = compare_destination_receipts(&artifact.plan, &receipts);
        assert_eq!(reconciled.receipt_count, 39);
        assert!(
            reconciled.mismatches.is_empty(),
            "{:?}",
            reconciled.mismatches
        );
    }

    #[test]
    fn explicit_relay_does_not_read_an_environment_fallback() {
        assert_eq!(
            explicit_ws_url("https://hub.mkideas.test/").expect("relay URL"),
            "wss://hub.mkideas.test"
        );
        assert!(explicit_ws_url("file:///tmp/relay").is_err());
    }

    #[test]
    fn media_upload_uses_the_same_explicit_relay_origin() {
        assert_eq!(
            explicit_http_url("wss://hub.mkideas.test/").expect("HTTP relay URL"),
            "https://hub.mkideas.test"
        );
        assert_eq!(
            explicit_http_url("ws://127.0.0.1:3000").expect("HTTP relay URL"),
            "http://127.0.0.1:3000"
        );
        assert!(explicit_http_url("file:///tmp/relay").is_err());
    }

    #[test]
    fn relay_media_descriptor_must_match_the_planned_bytes() {
        let media = PlannedMedia {
            media_id: uuid::Uuid::nil(),
            source: "guest:fixture".to_string(),
            related: None,
            file: "attachments/fixture.txt".to_string(),
            sha256: "a".repeat(64),
            mime_type: "text/plain".to_string(),
            size: 12,
            integrity: MediaIntegrity::Ready,
        };
        let valid = MigrationBlobDescriptor {
            url: format!("https://hub.mkideas.test/media/{}.txt", "a".repeat(64)),
            sha256: "a".repeat(64),
            size: 12,
            mime_type: "text/plain".to_string(),
        };
        assert!(validate_uploaded_descriptor(&media, &valid).is_ok());
        let mut substituted = valid;
        substituted.sha256 = "b".repeat(64);
        assert!(validate_uploaded_descriptor(&media, &substituted).is_err());
    }
}
