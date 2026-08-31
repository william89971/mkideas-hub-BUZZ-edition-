use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use buzz_migration::bundle::MediaIntegrity;
use buzz_migration::dry_run::{build_offline_dry_run, PlannedRecordClass};
use buzz_migration::manifest::ExportManifest;
use buzz_migration::mapping::MigrationMapping;
use buzz_migration::MigrationError;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const PARTNER_TWO: &str = "20000000-0000-4000-8000-000000000003";

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/mkideas-v0/migration")
}

fn mapping(root: &Path) -> MigrationMapping {
    serde_json::from_slice(&fs::read(root.join("mapping.example.json")).expect("read mapping"))
        .expect("parse mapping")
}

#[test]
fn dry_run_is_repeatable_and_preserves_representative_relationships() {
    let root = fixture_root();
    let mapping = mapping(&root);
    let first = build_offline_dry_run(&root, &mapping).expect("first dry-run");
    let second = build_offline_dry_run(&root, &mapping).expect("second dry-run");

    assert_eq!(
        first.sha256().expect("first hash"),
        second.sha256().expect("second hash")
    );
    assert_eq!(first.records.len(), 39);
    assert_eq!(first.plan.units.len(), first.records.len());
    assert_eq!(first.media.len(), 1);
    assert!(matches!(first.media[0].integrity, MediaIntegrity::Ready));
    assert_eq!(
        first.media[0]
            .related
            .as_ref()
            .map(|source| source.unit_kind.as_str()),
        Some("interviews")
    );

    let guest = record(&first.records, "guests");
    assert_eq!(
        guest.content.pointer("/status").and_then(Value::as_str),
        Some("archived")
    );
    assert_eq!(
        guest
            .content
            .pointer("/do_not_contact")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(guest.tags.iter().any(|tag| {
        tag.first().is_some_and(|name| name == "dnc")
            && tag.get(1).is_some_and(|value| value == "1")
    }));

    let content = record(&first.records, "content_items");
    let dependency_kinds = content
        .dependencies
        .iter()
        .map(|source| source.unit_kind.as_str())
        .collect::<Vec<_>>();
    assert!(dependency_kinds.contains(&"guests"));
    assert!(dependency_kinds.contains(&"interviews"));
    assert!(dependency_kinds.contains(&"memberships"));
    assert!(dependency_kinds.contains(&"content_versions"));
    assert_eq!(record(&first.records, "content_versions").version, 1);
    assert_eq!(content.version, 2);
    assert!(content.tags.iter().any(|tag| {
        tag.first().is_some_and(|name| name == "prev_source")
            && tag
                .get(1)
                .is_some_and(|value| value.contains("content_versions"))
    }));

    let transcript = record(&first.records, "transcript_segments");
    assert!(transcript.content.pointer("/legacy/text").is_none());
    assert!(transcript
        .content
        .pointer("/legacy/text_sha256")
        .and_then(Value::as_str)
        .is_some());
    assert_eq!(
        transcript
            .content
            .pointer("/legacy/storage")
            .and_then(Value::as_str),
        Some("private_transcript_media")
    );
    assert!(transcript
        .dependencies
        .iter()
        .any(|dependency| dependency.unit_kind == "media"));

    let audit = record(&first.records, "audit_events");
    assert!(audit.content.pointer("/legacy/before").is_none());
    assert!(audit.content.pointer("/legacy/after").is_none());
    assert!(audit
        .content
        .pointer("/legacy/archive_entry_sha256")
        .and_then(Value::as_str)
        .is_some());
    for state in first
        .records
        .iter()
        .filter(|record| record.class == PlannedRecordClass::State)
    {
        assert!(
            state
                .content
                .pointer("/status")
                .and_then(Value::as_str)
                .is_some(),
            "{} must have a typed status",
            state.source
        );
    }

    let positions = first
        .records
        .iter()
        .enumerate()
        .map(|(position, record)| (record.source.clone(), position))
        .collect::<BTreeMap<_, _>>();
    for record in &first.records {
        let current = positions[&record.source];
        for dependency in &record.dependencies {
            assert!(
                positions[dependency] < current,
                "{dependency} must precede {}",
                record.source
            );
        }
    }
    first
        .validate_checkpoint(&first.checkpoint)
        .expect("complete checkpoint");
    assert!(first.report.errors.is_empty());
}

#[test]
fn unmapped_author_is_rejected() {
    let root = fixture_root();
    let mut mapping = mapping(&root);
    mapping
        .memberships
        .remove(&Uuid::parse_str(PARTNER_TWO).expect("partner UUID"));
    assert!(matches!(
        build_offline_dry_run(&root, &mapping),
        Err(MigrationError::IdentityConflict(message))
            if message.contains("unmapped membership")
    ));
}

#[test]
fn unknown_status_is_rejected() {
    let temporary = TestBundle::copy();
    rewrite_table(&temporary.path, "guests", |row| {
        row["status"] = Value::String("future_status".to_string());
    });
    assert!(matches!(
        build_offline_dry_run(&temporary.path, &mapping(&temporary.path)),
        Err(MigrationError::UnsupportedStatus {
            domain: "guest",
            ..
        })
    ));
}

#[test]
fn duplicate_source_ids_are_rejected() {
    let temporary = TestBundle::copy();
    let table_path = temporary.path.join("tables/guests.ndjson");
    let original = fs::read_to_string(&table_path).expect("read guests");
    fs::write(&table_path, format!("{original}{original}")).expect("write duplicate guest");
    refresh_table_manifest(&temporary.path, "guests", 2);

    assert!(matches!(
        build_offline_dry_run(&temporary.path, &mapping(&temporary.path)),
        Err(MigrationError::DuplicateSource { table, .. }) if table == "guests"
    ));
}

#[test]
fn interrupted_checkpoint_validates_exact_prefix() {
    let root = fixture_root();
    let output = build_offline_dry_run(&root, &mapping(&root)).expect("dry-run");
    let partial = output.checkpoint_at(7).expect("partial checkpoint");
    output
        .validate_checkpoint(&partial)
        .expect("resume same plan");

    let mut corrupt = partial;
    corrupt.processed_prefix_sha256 = "f".repeat(64);
    assert!(matches!(
        output.validate_checkpoint(&corrupt),
        Err(MigrationError::IdentityConflict(_))
    ));
}

#[test]
fn missing_and_corrupt_media_become_explicit_exceptions() {
    let missing = TestBundle::copy();
    fs::remove_file(missing.path.join("attachments/synthetic-interview.vtt"))
        .expect("remove temporary attachment");
    let missing_output =
        build_offline_dry_run(&missing.path, &mapping(&missing.path)).expect("missing-media plan");
    assert!(matches!(
        missing_output.media[0].integrity,
        MediaIntegrity::Missing
    ));
    assert!(missing_output
        .report
        .errors
        .iter()
        .any(|error| error.starts_with("media_missing:")));

    let corrupt = TestBundle::copy();
    fs::write(
        corrupt.path.join("attachments/synthetic-interview.vtt"),
        b"corrupt synthetic attachment",
    )
    .expect("corrupt temporary attachment");
    let corrupt_output =
        build_offline_dry_run(&corrupt.path, &mapping(&corrupt.path)).expect("corrupt-media plan");
    assert!(matches!(
        corrupt_output.media[0].integrity,
        MediaIntegrity::Corrupt { .. }
    ));
    assert!(corrupt_output
        .report
        .errors
        .iter()
        .any(|error| error.starts_with("media_corrupt:")));
}

fn record<'a>(
    records: &'a [buzz_migration::dry_run::PlannedRecord],
    table: &str,
) -> &'a buzz_migration::dry_run::PlannedRecord {
    records
        .iter()
        .find(|record| record.source.unit_kind == table)
        .expect("representative record")
}

fn rewrite_table(root: &Path, table: &str, update: impl FnOnce(&mut Value)) {
    let path = root.join(format!("tables/{table}.ndjson"));
    let source = fs::read_to_string(&path).expect("read source table");
    let mut rows = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("parse row"))
        .collect::<Vec<_>>();
    update(rows.first_mut().expect("fixture row"));
    let output = rows
        .iter()
        .map(|row| serde_json::to_string(row).expect("serialize row"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(path, output).expect("write source table");
    refresh_table_manifest(root, table, u64::try_from(rows.len()).expect("row count"));
}

fn refresh_table_manifest(root: &Path, table: &str, row_count: u64) {
    let manifest_path = root.join("manifest.json");
    let mut manifest: ExportManifest =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    let table_manifest = manifest
        .tables
        .iter_mut()
        .find(|candidate| candidate.name == table)
        .expect("table manifest");
    let bytes = fs::read(root.join(&table_manifest.file)).expect("read changed table");
    table_manifest.sha256 = hex::encode(Sha256::digest(bytes));
    table_manifest.row_count = row_count;
    manifest.dataset_sha256 = manifest
        .computed_dataset_sha256()
        .expect("recompute dataset hash");
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
}

struct TestBundle {
    path: PathBuf,
}

impl TestBundle {
    fn copy() -> Self {
        let path = std::env::temp_dir().join(format!("buzz-migration-test-{}", Uuid::new_v4()));
        copy_directory(&fixture_root(), &path);
        Self { path }
    }
}

impl Drop for TestBundle {
    fn drop(&mut self) {
        let temporary_root = std::env::temp_dir();
        let safe_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("buzz-migration-test-"));
        if self.path.starts_with(&temporary_root) && safe_name {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create temporary fixture root");
    for entry in fs::read_dir(source).expect("read fixture directory") {
        let entry = entry.expect("fixture entry");
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().expect("fixture file type").is_dir() {
            copy_directory(&entry.path(), &destination_path);
        } else {
            fs::copy(entry.path(), destination_path).expect("copy fixture file");
        }
    }
}
