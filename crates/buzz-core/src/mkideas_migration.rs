//! Atomic MK Ideas migration-receipt wire contract.
//!
//! This is an intentionally narrow exception to normal human-authored state
//! ingest. A granted migration service signs both a schema-v2 receipt and the
//! one imported event embedded in it. Relay persistence treats the pair as one
//! transaction; neither event is accepted independently through this module.

use nostr::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::kind::{is_mkideas_operation_kind, is_mkideas_state_kind, KIND_MK_MIGRATION_RECEIPT};

/// Fixed namespace shared with the offline Command Center planner.
pub const MKCC_NAMESPACE: Uuid = Uuid::from_u128(0x4d4b_4944_4541_5342_555a_5a4d_4947_0001);

/// Immutable source identity bound into every receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MkMigrationSource {
    /// Legacy source-system name.
    pub system: String,
    /// Legacy workspace identifier.
    pub workspace_id: Uuid,
    /// Legacy table/aggregate name.
    pub unit_kind: String,
    /// Legacy primary or deterministic composite key.
    pub unit_id: String,
    /// Source revision, one for immutable rows.
    pub revision: u64,
}

impl MkMigrationSource {
    /// Stable textual idempotency coordinate.
    pub fn tag_value(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.system,
            self.workspace_id.hyphenated(),
            self.unit_kind,
            self.unit_id,
            self.revision
        )
    }
}

/// Parsed schema-v2 migration receipt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MkMigrationReceipt {
    /// Receipt schema; always two.
    pub schema_version: u64,
    /// Host-derived community name.
    pub community: String,
    /// Exact dataset digest granted to the service.
    pub dataset_sha256: String,
    /// Deterministic dataset batch.
    pub batch_id: Uuid,
    /// Deterministic per-source receipt id.
    pub receipt_id: Uuid,
    /// Exact source coordinate.
    pub source: MkMigrationSource,
    /// Canonical source-row digest.
    pub source_sha256: String,
    /// Deterministic planned record id.
    pub record_id: Uuid,
    /// Imported event kind.
    pub target_kind: u32,
    /// Addressable destination, only for MK state.
    pub destination_d: Option<Uuid>,
    /// Separately signed event imported by this receipt.
    pub imported_event: Event,
}

/// Fully validated receipt and imported event pair.
#[derive(Clone, Debug)]
pub struct MkMigrationEnvelope {
    /// Parsed receipt fields.
    pub receipt: MkMigrationReceipt,
    /// Parsed imported event content.
    pub imported_content: Value,
}

/// Migration receipt validation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MkMigrationError {
    /// A receipt or imported-event tag is absent or inconsistent.
    #[error("invalid migration tag: {0}")]
    Tag(String),
    /// Receipt or imported content violates the schema.
    #[error("invalid migration content: {0}")]
    Content(String),
    /// A signature, event id, or signer binding is invalid.
    #[error("invalid migration signature: {0}")]
    Signature(String),
}

/// Derive a deterministic destination/receipt identifier.
pub fn mkcc_destination_id(workspace_id: Uuid, destination_type: &str, source_key: &str) -> Uuid {
    let name = format!(
        "mkcc/{}/{}/{}",
        workspace_id.hyphenated(),
        destination_type.trim().to_ascii_lowercase(),
        source_key.trim().to_ascii_lowercase()
    );
    Uuid::new_v5(&MKCC_NAMESPACE, name.as_bytes())
}

/// Derive the batch bound to one workspace and immutable dataset hash.
pub fn mkcc_batch_id(workspace_id: Uuid, dataset_sha256: &str) -> Uuid {
    let name = format!(
        "mkcc/{}/batch/{}",
        workspace_id.hyphenated(),
        dataset_sha256.trim().to_ascii_lowercase()
    );
    Uuid::new_v5(&MKCC_NAMESPACE, name.as_bytes())
}

/// Validate a signed schema-v2 receipt and its one signed imported event.
pub fn validate_migration_receipt(event: &Event) -> Result<MkMigrationEnvelope, MkMigrationError> {
    if u32::from(event.kind.as_u16()) != KIND_MK_MIGRATION_RECEIPT {
        return Err(MkMigrationError::Content(
            "receipt kind must be 48202".to_string(),
        ));
    }
    let receipt: MkMigrationReceipt = serde_json::from_str(&event.content)
        .map_err(|error| MkMigrationError::Content(error.to_string()))?;
    validate_receipt_fields(event, &receipt)?;
    validate_imported_event(event, &receipt)
}

fn validate_receipt_fields(
    event: &Event,
    receipt: &MkMigrationReceipt,
) -> Result<(), MkMigrationError> {
    if receipt.schema_version != 2 {
        return Err(MkMigrationError::Content(
            "schema_version must be 2".to_string(),
        ));
    }
    if receipt.community.trim().is_empty() || receipt.community.chars().count() > 253 {
        return Err(MkMigrationError::Content(
            "community must be a non-empty host".to_string(),
        ));
    }
    require_hash(&receipt.dataset_sha256, "dataset_sha256")?;
    require_hash(&receipt.source_sha256, "source_sha256")?;
    if receipt.source.system.trim().is_empty()
        || receipt.source.unit_kind.trim().is_empty()
        || receipt.source.unit_id.trim().is_empty()
        || receipt.source.revision == 0
    {
        return Err(MkMigrationError::Content(
            "source coordinate fields and positive revision are required".to_string(),
        ));
    }
    if receipt.batch_id != mkcc_batch_id(receipt.source.workspace_id, &receipt.dataset_sha256) {
        return Err(MkMigrationError::Content(
            "batch_id does not match workspace and dataset".to_string(),
        ));
    }
    let source_tag = receipt.source.tag_value();
    if receipt.receipt_id
        != mkcc_destination_id(receipt.source.workspace_id, "receipt", &source_tag)
    {
        return Err(MkMigrationError::Content(
            "receipt_id is not deterministic for the source".to_string(),
        ));
    }
    if receipt.record_id
        != mkcc_destination_id(
            receipt.source.workspace_id,
            &receipt.source.unit_kind,
            &receipt.source.unit_id,
        )
    {
        return Err(MkMigrationError::Content(
            "record_id is not deterministic for the source".to_string(),
        ));
    }
    exact_tag(event, "h", &receipt.community)?;
    exact_tag(event, "dataset", &receipt.dataset_sha256)?;
    exact_tag(event, "batch", &receipt.batch_id.to_string())?;
    exact_tag(event, "source", &source_tag)?;
    exact_tag(event, "e", &receipt.imported_event.id.to_hex())?;
    Ok(())
}

fn validate_imported_event(
    receipt_event: &Event,
    receipt: &MkMigrationReceipt,
) -> Result<MkMigrationEnvelope, MkMigrationError> {
    let imported = &receipt.imported_event;
    if !imported.verify_id() || !imported.verify_signature() {
        return Err(MkMigrationError::Signature(
            "imported event id or signature is invalid".to_string(),
        ));
    }
    if imported.pubkey != receipt_event.pubkey {
        return Err(MkMigrationError::Signature(
            "receipt and imported event must have the same service signer".to_string(),
        ));
    }
    if u32::from(imported.kind.as_u16()) != receipt.target_kind
        || !is_allowed_import_kind(receipt.target_kind)
    {
        return Err(MkMigrationError::Content(
            "target_kind is not an allowed imported kind".to_string(),
        ));
    }
    let imported_content: Value = serde_json::from_str(&imported.content)
        .map_err(|error| MkMigrationError::Content(error.to_string()))?;
    if imported_content
        .get("schema_version")
        .and_then(Value::as_u64)
        != Some(2)
        || imported_content.get("record_id").and_then(Value::as_str)
            != Some(receipt.record_id.to_string().as_str())
        || imported_content.get("community").and_then(Value::as_str)
            != Some(receipt.community.as_str())
    {
        return Err(MkMigrationError::Content(
            "imported event schema_version and record_id must match the receipt".to_string(),
        ));
    }
    let migration = imported_content
        .get("migration")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            MkMigrationError::Content("imported migration provenance is required".to_string())
        })?;
    for (field, expected) in [
        ("dataset_sha256", receipt.dataset_sha256.as_str()),
        ("source_system", receipt.source.system.as_str()),
        ("source_table", receipt.source.unit_kind.as_str()),
        ("source_id", receipt.source.unit_id.as_str()),
        ("source_sha256", receipt.source_sha256.as_str()),
    ] {
        if migration.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(MkMigrationError::Content(format!(
                "imported migration {field} does not match the receipt"
            )));
        }
    }
    if migration.get("source_workspace_id").and_then(Value::as_str)
        != Some(receipt.source.workspace_id.to_string().as_str())
    {
        return Err(MkMigrationError::Content(
            "imported migration source_workspace_id does not match the receipt".to_string(),
        ));
    }
    exact_tag(imported, "source", &receipt.source.tag_value())?;
    exact_tag(imported, "source_sha256", &receipt.source_sha256)?;
    if is_mkideas_state_kind(receipt.target_kind) {
        let destination = receipt.destination_d.ok_or_else(|| {
            MkMigrationError::Content("state imports require destination_d".to_string())
        })?;
        let expected_destination = expected_state_destination(receipt, &imported_content)?;
        if destination != expected_destination {
            return Err(MkMigrationError::Content(
                "destination_d is not deterministic for the imported state".to_string(),
            ));
        }
        exact_tag(imported, "h", &receipt.community)?;
        exact_tag(imported, "d", &destination.to_string())?;
        let version = exact_tag_value(imported, "version")?
            .parse::<u64>()
            .map_err(|_| MkMigrationError::Tag("version must be positive".to_string()))?;
        if version == 0 || imported_content.get("version").and_then(Value::as_u64) != Some(version)
        {
            return Err(MkMigrationError::Content(
                "state version tag and content must match".to_string(),
            ));
        }
        let status = exact_tag_value(imported, "status")?;
        if imported_content.get("status").and_then(Value::as_str) != Some(status) {
            return Err(MkMigrationError::Content(
                "state status tag and content must match".to_string(),
            ));
        }
    } else {
        if receipt.destination_d.is_some() {
            return Err(MkMigrationError::Content(
                "operation imports cannot set destination_d".to_string(),
            ));
        }
        if is_mkideas_operation_kind(receipt.target_kind) {
            exact_tag(imported, "h", &receipt.community)?;
        }
    }
    Ok(MkMigrationEnvelope {
        receipt: receipt.clone(),
        imported_content,
    })
}

fn expected_state_destination(
    receipt: &MkMigrationReceipt,
    content: &Value,
) -> Result<Uuid, MkMigrationError> {
    let source_key = match receipt.source.unit_kind.as_str() {
        "content_versions" => content
            .pointer("/legacy/content_item_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                MkMigrationError::Content(
                    "content version import requires legacy content_item_id".to_string(),
                )
            })?,
        _ => receipt.source.unit_id.as_str(),
    };
    let destination_type = match receipt.target_kind {
        30_800 => "goal",
        30_801 => "operational_project",
        30_802 => "task",
        30_803 => "person",
        30_804 => "interview",
        30_805 => "content",
        30_806 => "meeting",
        30_807 => "decision",
        30_808 => receipt.source.unit_kind.as_str(),
        30_809 => "approval",
        _ => {
            return Err(MkMigrationError::Content(
                "unsupported imported state kind".to_string(),
            ));
        }
    };
    Ok(mkcc_destination_id(
        receipt.source.workspace_id,
        destination_type,
        source_key,
    ))
}

fn is_allowed_import_kind(kind: u32) -> bool {
    is_mkideas_state_kind(kind) || is_mkideas_operation_kind(kind) || kind == 9 // service-authored, clearly attributed Team migration history
}

fn require_hash(value: &str, field: &str) -> Result<(), MkMigrationError> {
    if value.len() != 64
        || value != value.to_ascii_lowercase()
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(MkMigrationError::Content(format!(
            "{field} must be lowercase 32-byte hex"
        )));
    }
    Ok(())
}

fn exact_tag(event: &Event, name: &str, expected: &str) -> Result<(), MkMigrationError> {
    if exact_tag_value(event, name)? != expected {
        return Err(MkMigrationError::Tag(format!(
            "{name} does not match the receipt"
        )));
    }
    Ok(())
}

fn exact_tag_value<'a>(event: &'a Event, name: &str) -> Result<&'a str, MkMigrationError> {
    let mut values = event.tags.iter().filter_map(|tag| {
        let parts = tag.as_slice();
        (parts.first().map(String::as_str) == Some(name))
            .then(|| parts.get(1).map(String::as_str))
            .flatten()
    });
    let value = values
        .next()
        .ok_or_else(|| MkMigrationError::Tag(format!("missing {name}")))?;
    if values.next().is_some() {
        return Err(MkMigrationError::Tag(format!("duplicate {name}")));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::KIND_MK_SYSTEM_ACTIVITY;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use serde_json::json;

    fn valid_receipt() -> (Event, MkMigrationReceipt) {
        let keys = Keys::generate();
        let workspace_id = Uuid::new_v4();
        let dataset_sha256 = "b".repeat(64);
        let source_sha256 = "a".repeat(64);
        let source = MkMigrationSource {
            system: "mkideas-command-center".to_string(),
            workspace_id,
            unit_kind: "activity_events".to_string(),
            unit_id: Uuid::new_v4().to_string(),
            revision: 1,
        };
        let source_tag = source.tag_value();
        let record_id = mkcc_destination_id(workspace_id, &source.unit_kind, &source.unit_id);
        let imported = EventBuilder::new(
            Kind::Custom(KIND_MK_SYSTEM_ACTIVITY as u16),
            json!({
                "schema_version": 2,
                "record_id": record_id,
                "community": "hub.mkideas.test",
                "version": 1,
                "migration": {
                    "dataset_sha256": dataset_sha256,
                    "source_system": source.system,
                    "source_workspace_id": workspace_id,
                    "source_table": source.unit_kind,
                    "source_id": source.unit_id,
                    "source_sha256": source_sha256
                }
            })
            .to_string(),
        )
        .tags([
            Tag::parse(["h", "hub.mkideas.test"]).expect("h"),
            Tag::parse(["source", source_tag.as_str()]).expect("source"),
            Tag::parse(["source_sha256", source_sha256.as_str()]).expect("hash"),
        ])
        .sign_with_keys(&keys)
        .expect("import event");
        let receipt = MkMigrationReceipt {
            schema_version: 2,
            community: "hub.mkideas.test".to_string(),
            dataset_sha256: dataset_sha256.clone(),
            batch_id: mkcc_batch_id(workspace_id, &dataset_sha256),
            receipt_id: mkcc_destination_id(workspace_id, "receipt", &source_tag),
            source,
            source_sha256,
            record_id,
            target_kind: KIND_MK_SYSTEM_ACTIVITY,
            destination_d: None,
            imported_event: imported,
        };
        let batch = receipt.batch_id.to_string();
        let imported_id = receipt.imported_event.id.to_hex();
        let event = EventBuilder::new(
            Kind::Custom(KIND_MK_MIGRATION_RECEIPT as u16),
            serde_json::to_string(&receipt).expect("receipt JSON"),
        )
        .tags([
            Tag::parse(["h", receipt.community.as_str()]).expect("h"),
            Tag::parse(["dataset", receipt.dataset_sha256.as_str()]).expect("dataset"),
            Tag::parse(["batch", batch.as_str()]).expect("batch"),
            Tag::parse(["source", source_tag.as_str()]).expect("source"),
            Tag::parse(["e", imported_id.as_str()]).expect("event"),
        ])
        .sign_with_keys(&keys)
        .expect("receipt event");
        (event, receipt)
    }

    #[test]
    fn validates_exact_signed_pair() {
        let (event, _) = valid_receipt();
        let envelope = validate_migration_receipt(&event).expect("valid receipt");
        assert_eq!(envelope.receipt.target_kind, KIND_MK_SYSTEM_ACTIVITY);
    }

    #[test]
    fn rejects_dataset_and_target_mismatches() {
        let (event, mut receipt) = valid_receipt();
        receipt.dataset_sha256 = "c".repeat(64);
        let changed = EventBuilder::new(
            Kind::Custom(KIND_MK_MIGRATION_RECEIPT as u16),
            serde_json::to_string(&receipt).expect("changed receipt"),
        )
        .tags(event.tags)
        .sign_with_keys(&Keys::generate())
        .expect("changed event");
        assert!(validate_migration_receipt(&changed).is_err());

        let (_, mut receipt) = valid_receipt();
        receipt.target_kind = 1;
        assert!(matches!(
            validate_imported_event(
                &EventBuilder::new(Kind::Custom(1), "{}")
                    .sign_with_keys(&Keys::generate())
                    .expect("wrong outer"),
                &receipt
            ),
            Err(MkMigrationError::Signature(_) | MkMigrationError::Content(_))
        ));
    }
}
