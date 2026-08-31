//! Explicit-credential, relay-only submission of offline migration artifacts.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use buzz_core::mkideas_migration::{mkcc_destination_id, MkMigrationReceipt, MkMigrationSource};
use buzz_core::tenant::relay_url_authority;
use buzz_migration::dry_run::{OfflineDryRun, OfflinePlanningCheckpoint, PlannedRecord};
use buzz_migration::secrets::validate_no_secrets;
use chrono::{DateTime, Utc};
use nostr::{Event, EventBuilder, Keys, Kind, Tag, Timestamp};
use serde::de::DeserializeOwned;
use serde_json::json;
use sha2::{Digest, Sha256};

pub(crate) struct SubmitOptions {
    pub(crate) artifact: PathBuf,
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
        if record.action == buzz_migration::plan::PlannedAction::Create {
            let imported = sign_imported_record(record, &keys, &signed_by_source)?;
            signed_by_source.insert(record.source.tag_value(), imported);
        }
    }

    let mut submitted = 0_u64;
    let mut skipped = 0_u64;
    for (index, record) in artifact.records.iter().enumerate().skip(start) {
        if record.action == buzz_migration::plan::PlannedAction::Create {
            let imported = sign_imported_record(record, &keys, &signed_by_source)?;
            let receipt = sign_receipt(&artifact, record, imported.clone(), &keys)?;
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
            signed_by_source.insert(record.source.tag_value(), imported);
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
        for record in &artifact.records {
            if record.action != buzz_migration::plan::PlannedAction::Create {
                continue;
            }
            let imported = sign_imported_record(record, &keys, &signed).expect("import event");
            let receipt = sign_receipt(&artifact, record, imported.clone(), &keys)
                .expect("migration receipt");
            buzz_core::mkideas_migration::validate_migration_receipt(&receipt)
                .expect("valid migration pair");
            signed.insert(record.source.tag_value(), imported);
        }
        assert_eq!(signed.len(), 39);
    }

    #[test]
    fn explicit_relay_does_not_read_an_environment_fallback() {
        assert_eq!(
            explicit_ws_url("https://hub.mkideas.test/").expect("relay URL"),
            "wss://hub.mkideas.test"
        );
        assert!(explicit_ws_url("file:///tmp/relay").is_err());
    }
}
