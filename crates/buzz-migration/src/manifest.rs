//! Canonical export-manifest models and validation.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::canonical::canonical_sha256;
use crate::{MigrationError, Result};

/// Current offline bundle schema.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Stable legacy-source identifier.
pub const SOURCE_SYSTEM: &str = "mkideas-command-center";

/// One source table exported as deterministic NDJSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableManifest {
    /// PostgreSQL table name.
    pub name: String,
    /// Bundle-relative NDJSON path.
    pub file: String,
    /// Number of rows in the file.
    pub row_count: u64,
    /// SHA-256 of the exact file bytes.
    pub sha256: String,
}

/// One generated or separately supplied migration media item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaManifest {
    /// Source-unit coordinate/tag value.
    pub source: String,
    /// Bundle-relative path.
    pub file: String,
    /// Expected bytes hash.
    pub sha256: String,
    /// Validated MIME type expected at upload.
    pub mime_type: String,
    /// Expected byte count.
    pub size: u64,
}

/// Complete immutable source snapshot description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportManifest {
    /// Bundle schema version.
    pub schema_version: u32,
    /// Must equal [`SOURCE_SYSTEM`].
    pub source_system: String,
    /// Source workspace UUID.
    pub source_workspace_id: Uuid,
    /// Hash of the inspected legacy table/column contract.
    pub source_schema_sha256: String,
    /// Informational export time. Excluded from the dataset hash.
    pub exported_at: String,
    /// Hash of this manifest with `exported_at` and this field cleared.
    pub dataset_sha256: String,
    /// Ordered source-table files.
    pub tables: Vec<TableManifest>,
    /// Generated or separately supplied media.
    #[serde(default)]
    pub media: Vec<MediaManifest>,
    /// Source fields intentionally excluded for security or semantic reasons.
    #[serde(default)]
    pub excluded_fields: Vec<String>,
}

impl ExportManifest {
    /// Compute the dataset hash while excluding nondeterministic export time and
    /// the self-referential hash field.
    pub fn computed_dataset_sha256(&self) -> Result<String> {
        let mut normalized = self.clone();
        normalized.exported_at.clear();
        normalized.dataset_sha256.clear();
        normalized
            .tables
            .sort_by(|left, right| left.name.cmp(&right.name));
        normalized
            .media
            .sort_by(|left, right| left.source.cmp(&right.source));
        normalized.excluded_fields.sort();
        canonical_sha256(&normalized)
    }

    /// Validate version, source identity, paths, duplicate table names, and
    /// dataset hash.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(MigrationError::InvalidManifest(format!(
                "unsupported schema_version {}",
                self.schema_version
            )));
        }
        if self.source_system != SOURCE_SYSTEM {
            return Err(MigrationError::InvalidManifest(format!(
                "unexpected source_system {}",
                self.source_system
            )));
        }

        let mut names = std::collections::BTreeSet::new();
        for table in &self.tables {
            validate_relative_path(&table.file)?;
            if !names.insert(table.name.as_str()) {
                return Err(MigrationError::InvalidManifest(format!(
                    "duplicate table {}",
                    table.name
                )));
            }
            validate_sha256(&table.sha256, &format!("table {}", table.name))?;
        }
        for media in &self.media {
            validate_relative_path(&media.file)?;
            validate_sha256(&media.sha256, &format!("media {}", media.source))?;
        }
        validate_sha256(&self.source_schema_sha256, "source schema")?;
        validate_sha256(&self.dataset_sha256, "dataset")?;
        let computed = self.computed_dataset_sha256()?;
        if computed != self.dataset_sha256.to_ascii_lowercase() {
            return Err(MigrationError::InvalidManifest(format!(
                "dataset hash mismatch: expected {}, computed {computed}",
                self.dataset_sha256
            )));
        }
        Ok(())
    }
}

/// Reject absolute paths, prefixes, roots, and parent traversal.
pub fn validate_relative_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    let portable = value.replace('\\', "/");
    let has_drive_prefix = portable.as_bytes().get(1).is_some_and(|byte| *byte == b':');
    let has_parent_segment = portable.split('/').any(|segment| segment == "..");
    if value.is_empty()
        || path.is_absolute()
        || portable.starts_with('/')
        || has_drive_prefix
        || has_parent_segment
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(MigrationError::UnsafePath(value.to_string()));
    }
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MigrationError::InvalidManifest(format!(
            "{label} SHA-256 must be 64 hex characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_paths_are_portably_confined() {
        validate_relative_path("tables/guests.ndjson").expect("safe path");
        for unsafe_path in [
            "../secret",
            "tables/../../secret",
            r"tables\..\secret",
            r"C:\secret",
            "/etc/passwd",
            r"\\server\share\secret",
        ] {
            assert!(matches!(
                validate_relative_path(unsafe_path),
                Err(MigrationError::UnsafePath(_))
            ));
        }
    }
}
