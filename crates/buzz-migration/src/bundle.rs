//! Read-only loading and integrity inspection for offline export bundles.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::canonical::canonical_sha256;
use crate::manifest::{ExportManifest, MediaManifest};
use crate::secrets::validate_no_secrets;
use crate::{MigrationError, Result};

const MAX_NDJSON_ROW_BYTES: usize = 4 * 1024 * 1024;

/// One source row with a deterministic source key and canonical content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRow {
    /// Legacy table name.
    pub table: String,
    /// Primary or deterministic composite key.
    pub key: String,
    /// SHA-256 of canonical row JSON.
    pub sha256: String,
    /// Secret-free source payload.
    pub value: Value,
}

/// Local integrity result for one declared media object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MediaIntegrity {
    /// The file exists and matches its declared bytes and size.
    Ready,
    /// The declared file is absent.
    Missing,
    /// The file exists but its bytes or size do not match the manifest.
    Corrupt {
        /// SHA-256 of the bytes found locally.
        actual_sha256: String,
        /// Number of bytes found locally.
        actual_size: u64,
    },
    /// The path exists but is not a regular readable file.
    Unreadable {
        /// Non-secret diagnostic code.
        reason: String,
    },
}

/// One declared attachment plus its local integrity result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectedMedia {
    /// Immutable manifest descriptor.
    pub manifest: MediaManifest,
    /// Read-only local integrity result.
    pub integrity: MediaIntegrity,
}

/// Complete secret-free bundle loaded without contacting either system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedBundle {
    /// Validated source manifest.
    pub manifest: ExportManifest,
    /// Source rows grouped by table in manifest order.
    pub tables: BTreeMap<String, Vec<SourceRow>>,
    /// Declared media with integrity outcomes.
    pub media: Vec<InspectedMedia>,
}

impl LoadedBundle {
    /// Return all rows in deterministic table/key order.
    pub fn ordered_rows(&self) -> Vec<&SourceRow> {
        self.tables.values().flat_map(|rows| rows.iter()).collect()
    }

    /// Find one row by legacy table and stable key.
    pub fn row(&self, table: &str, key: &str) -> Option<&SourceRow> {
        self.tables
            .get(table)
            .and_then(|rows| rows.iter().find(|row| row.key == key))
    }
}

/// Load and verify an export bundle using local read-only file operations.
///
/// Table integrity failures stop planning. Declared media failures are retained
/// as explicit outcomes so a dry run can produce an attachment exception
/// report without pretending the object is ready.
pub fn load_bundle(root: &Path) -> Result<LoadedBundle> {
    let root = canonical_directory(root)?;
    let manifest_path = confined_existing_file(&root, "manifest.json")?;
    let manifest_value = read_json(&manifest_path, "manifest")?;
    validate_no_secrets(&manifest_value)?;
    let manifest: ExportManifest = serde_json::from_value(manifest_value)
        .map_err(|error| MigrationError::InvalidManifest(error.to_string()))?;
    manifest.validate()?;

    let mut tables = BTreeMap::new();
    for table in &manifest.tables {
        let path = confined_existing_file(&root, &table.file)?;
        let actual_hash = hash_file(&path)?;
        if actual_hash != table.sha256.to_ascii_lowercase() {
            return Err(MigrationError::InvalidManifest(format!(
                "table {} hash mismatch: expected {}, computed {actual_hash}",
                table.name, table.sha256
            )));
        }

        let file = File::open(&path).map_err(bundle_io)?;
        let mut rows = Vec::new();
        let mut keys = BTreeSet::new();
        for (index, line) in BufReader::new(file).split(b'\n').enumerate() {
            let line = line.map_err(bundle_io)?;
            if line.len() > MAX_NDJSON_ROW_BYTES {
                return Err(MigrationError::InvalidSource(format!(
                    "{} logical line {} exceeds the {} byte safety limit",
                    table.name,
                    index + 1,
                    MAX_NDJSON_ROW_BYTES
                )));
            }
            let line = trim_ascii(&line);
            if line.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_slice(line).map_err(|error| {
                MigrationError::InvalidSource(format!(
                    "{} logical line {} is invalid JSON: {error}",
                    table.name,
                    index + 1
                ))
            })?;
            validate_no_secrets(&value)?;
            validate_workspace(&manifest, &table.name, &value)?;
            let key = source_key(&table.name, &value)?;
            if !keys.insert(key.clone()) {
                return Err(MigrationError::DuplicateSource {
                    table: table.name.clone(),
                    key,
                });
            }
            rows.push(SourceRow {
                table: table.name.clone(),
                key,
                sha256: canonical_sha256(&value)?,
                value,
            });
        }
        if u64::try_from(rows.len()).unwrap_or(u64::MAX) != table.row_count {
            return Err(MigrationError::InvalidManifest(format!(
                "table {} row count mismatch: manifest {}, file {}",
                table.name,
                table.row_count,
                rows.len()
            )));
        }
        rows.sort_by(|left, right| left.key.cmp(&right.key));
        tables.insert(table.name.clone(), rows);
    }

    let media = manifest
        .media
        .iter()
        .cloned()
        .map(|media| inspect_media(&root, media))
        .collect::<Vec<_>>();

    Ok(LoadedBundle {
        manifest,
        tables,
        media,
    })
}

/// Derive a stable source key for one supported legacy row.
pub fn source_key(table: &str, value: &Value) -> Result<String> {
    if let Some(id) = string_field(value, "id") {
        return Ok(id.to_string());
    }
    let fields: &[&str] = match table {
        "guest_tags" => &["guest_id", "tag"],
        "guest_topics" => &["guest_id", "topic"],
        "interview_members" => &["interview_id", "membership_id"],
        "content_platforms" => &["content_item_id", "platform"],
        "clip_transcript_segments" => &["clip_candidate_id", "transcript_segment_id"],
        _ => &[],
    };
    if !fields.is_empty() {
        let parts = fields
            .iter()
            .map(|field| {
                value.get(*field).and_then(Value::as_str).ok_or_else(|| {
                    MigrationError::InvalidSource(format!(
                        "{table} row is missing string field {field}"
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        return canonical_sha256(&parts);
    }
    Err(MigrationError::InvalidSource(format!(
        "{table} row has no supported primary or composite key"
    )))
}

fn inspect_media(root: &Path, manifest: MediaManifest) -> InspectedMedia {
    let path = match confined_optional_file(root, &manifest.file) {
        Ok(Some(path)) => path,
        Ok(None) => {
            return InspectedMedia {
                manifest,
                integrity: MediaIntegrity::Missing,
            };
        }
        Err(error) => {
            return InspectedMedia {
                manifest,
                integrity: MediaIntegrity::Unreadable {
                    reason: error.to_string(),
                },
            };
        }
    };
    let actual_size = match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => metadata.len(),
        Ok(_) => {
            return InspectedMedia {
                manifest,
                integrity: MediaIntegrity::Unreadable {
                    reason: "not_regular_file".to_string(),
                },
            };
        }
        Err(error) => {
            return InspectedMedia {
                manifest,
                integrity: MediaIntegrity::Unreadable {
                    reason: error.kind().to_string(),
                },
            };
        }
    };
    let actual_sha256 = match hash_file(&path) {
        Ok(hash) => hash,
        Err(error) => {
            return InspectedMedia {
                manifest,
                integrity: MediaIntegrity::Unreadable {
                    reason: error.to_string(),
                },
            };
        }
    };
    let integrity =
        if actual_sha256 == manifest.sha256.to_ascii_lowercase() && actual_size == manifest.size {
            MediaIntegrity::Ready
        } else {
            MediaIntegrity::Corrupt {
                actual_sha256,
                actual_size,
            }
        };
    InspectedMedia {
        manifest,
        integrity,
    }
}

fn validate_workspace(manifest: &ExportManifest, table: &str, value: &Value) -> Result<()> {
    let expected = manifest.source_workspace_id.to_string();
    let observed = if table == "workspaces" {
        string_field(value, "id")
    } else {
        string_field(value, "workspace_id")
    };
    if let Some(observed) = observed {
        if observed != expected {
            return Err(MigrationError::InvalidSource(format!(
                "{table} row belongs to workspace {observed}, expected {expected}"
            )));
        }
    }
    Ok(())
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn read_json(path: &Path, label: &str) -> Result<Value> {
    let bytes = fs::read(path).map_err(bundle_io)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| MigrationError::InvalidSource(format!("{label} JSON: {error}")))
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(bundle_io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(bundle_io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(bundle_io)?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(MigrationError::BundleIo(
            "bundle root is not a directory".to_string(),
        ))
    }
}

fn confined_existing_file(root: &Path, relative: &str) -> Result<PathBuf> {
    confined_optional_file(root, relative)?.ok_or_else(|| {
        MigrationError::BundleIo(format!("declared bundle file is missing: {relative}"))
    })
}

fn confined_optional_file(root: &Path, relative: &str) -> Result<Option<PathBuf>> {
    let candidate = root.join(relative);
    if !candidate.exists() {
        return Ok(None);
    }
    let canonical = fs::canonicalize(&candidate).map_err(bundle_io)?;
    if !canonical.starts_with(root) {
        return Err(MigrationError::UnsafePath(relative.to_string()));
    }
    Ok(Some(canonical))
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(|byte| byte.is_ascii_whitespace()) {
        value = &value[1..];
    }
    while value.last().is_some_and(|byte| byte.is_ascii_whitespace()) {
        value = &value[..value.len().saturating_sub(1)];
    }
    value
}

fn bundle_io(error: std::io::Error) -> MigrationError {
    MigrationError::BundleIo(error.kind().to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn supported_composite_keys_are_stable() {
        let value = json!({"guest_id": "guest", "tag": "editorial"});
        assert_eq!(
            source_key("guest_tags", &value).expect("composite key"),
            "09f70812ae72653758563353768ee8f434cc749f944f8744ea003cdf90da4ccf"
        );
    }
}
