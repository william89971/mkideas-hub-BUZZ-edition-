use std::fs;
use std::path::PathBuf;

use buzz_migration::manifest::ExportManifest;
use buzz_migration::mapping::MigrationMapping;
use sha2::{Digest, Sha256};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/mkideas-v0/migration")
}

#[test]
fn synthetic_bundle_manifest_and_files_are_consistent() {
    let root = fixture_root();
    let manifest_bytes = fs::read(root.join("manifest.json")).expect("read fixture manifest");
    let manifest: ExportManifest =
        serde_json::from_slice(&manifest_bytes).expect("parse fixture manifest");

    for table in &manifest.tables {
        let bytes = fs::read(root.join(&table.file)).expect("read fixture table");
        assert_eq!(hex::encode(Sha256::digest(&bytes)), table.sha256);
        let rows = std::str::from_utf8(&bytes)
            .expect("UTF-8 NDJSON")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count() as u64;
        assert_eq!(rows, table.row_count, "{} row count", table.name);

        for line in std::str::from_utf8(&bytes).expect("UTF-8 NDJSON").lines() {
            if !line.trim().is_empty() {
                let value: serde_json::Value =
                    serde_json::from_str(line).expect("valid NDJSON row");
                buzz_migration::secrets::validate_no_secrets(&value)
                    .expect("fixture row contains no secret fields");
            }
        }
    }

    let computed = manifest
        .computed_dataset_sha256()
        .expect("compute dataset hash");
    assert_eq!(
        manifest.dataset_sha256, computed,
        "dataset_sha256={computed}"
    );
    manifest.validate().expect("valid fixture manifest");

    let mapping_bytes = fs::read(root.join("mapping.example.json")).expect("read fixture mapping");
    let mapping: MigrationMapping =
        serde_json::from_slice(&mapping_bytes).expect("parse fixture mapping");
    mapping.validate().expect("valid fixture mapping");
}
