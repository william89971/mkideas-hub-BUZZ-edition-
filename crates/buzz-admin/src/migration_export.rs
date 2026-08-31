//! Explicit, read-only legacy PostgreSQL export for the MK Ideas migration.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use buzz_migration::canonical::{canonical_json_bytes, canonical_sha256};
use buzz_migration::manifest::{
    ExportManifest, MediaManifest, TableManifest, MANIFEST_SCHEMA_VERSION, SOURCE_SYSTEM,
};
use buzz_migration::secrets::validate_no_secrets;
use chrono::{SecondsFormat, Utc};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::Row;
use uuid::Uuid;

const CONFIG_SCHEMA_VERSION: u32 = 1;
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_NDJSON_ROW_BYTES: usize = 4 * 1024 * 1024;
const MAX_MEDIA_BYTES: u64 = 8 * 1024 * 1024 * 1024;

const SUPPORTED_TABLES: &[&str] = &[
    "activity_events",
    "agent_runs",
    "approval_events",
    "approvals",
    "attention_items",
    "audit_events",
    "clip_candidates",
    "clip_transcript_segments",
    "comments",
    "content_items",
    "content_platforms",
    "content_qc_findings",
    "content_versions",
    "guest_communications",
    "guest_notes",
    "guest_research_briefs",
    "guest_research_facts",
    "guest_research_themes",
    "guest_tags",
    "guest_topics",
    "guests",
    "interview_members",
    "interview_questions",
    "interviews",
    "memberships",
    "notifications",
    "organizations",
    "outreach_draft_versions",
    "outreach_send_claims",
    "tasks",
    "transcript_segments",
    "workspaces",
];

fn default_schema() -> String {
    "public".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyExportConfig {
    schema_version: u32,
    database_url: String,
    source_workspace_id: Uuid,
    #[serde(default = "default_schema")]
    schema: String,
    #[serde(default)]
    media: Vec<LegacyMediaSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyMediaSource {
    source: String,
    path: PathBuf,
    mime_type: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct SchemaColumn {
    column_name: String,
    data_type: String,
    udt_name: String,
    ordinal_position: i32,
}

/// Export one authorized legacy workspace without writing to the source.
pub(crate) async fn export_legacy_snapshot(config_path: &Path, output: &Path) -> Result<i32> {
    let config = read_config(config_path)?;
    validate_config(&config)?;
    if output.exists() {
        return Err(anyhow!(
            "export output already exists; choose a new empty destination"
        ));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(anyhow!("export output parent does not exist"));
    }
    fs::create_dir(output).context("create export output directory")?;
    fs::create_dir(output.join("tables")).context("create export tables directory")?;

    let options = PgConnectOptions::from_str(&config.database_url)
        .context("source configuration contains an invalid PostgreSQL URL")?
        .application_name("buzz-admin-mkideas-read-only-export");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .context("could not connect to the explicitly configured legacy PostgreSQL source")?;
    let mut transaction = pool.begin().await.context("begin source transaction")?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *transaction)
        .await
        .context("source refused a repeatable-read, read-only transaction")?;
    let read_only: bool =
        sqlx::query_scalar("SELECT current_setting('transaction_read_only')::boolean")
            .fetch_one(&mut *transaction)
            .await
            .context("verify source transaction mode")?;
    if !read_only {
        return Err(anyhow!("source transaction is not read-only"));
    }

    let mut schema_contract = BTreeMap::<String, Vec<SchemaColumn>>::new();
    let mut tables = Vec::with_capacity(SUPPORTED_TABLES.len());
    let mut excluded_fields = BTreeSet::<String>::new();
    for table in SUPPORTED_TABLES {
        let columns = load_columns(&mut transaction, &config.schema, table).await?;
        if columns.is_empty() {
            return Err(anyhow!(
                "required legacy table {}.{} is missing",
                config.schema,
                table
            ));
        }
        let selected = permitted_columns(table, &columns, &mut excluded_fields)?;
        validate_workspace_filter_contract(table, &columns)?;
        let query = export_query(&config.schema, table, &selected);
        let file_path = output.join("tables").join(format!("{table}.ndjson"));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&file_path)
            .with_context(|| format!("create export table file {table}"))?;
        let mut writer = BufWriter::new(file);
        let mut hasher = Sha256::new();
        let mut row_count = 0_u64;
        {
            // SAFETY: `export_query` receives only a validated schema name, a
            // compile-time allowlisted table, and identifiers read from
            // information_schema that are escaped by `quote_identifier`.
            // Workspace identity remains a bind parameter.
            let mut rows = sqlx::query(sqlx::AssertSqlSafe(query))
                .bind(config.source_workspace_id.to_string())
                .fetch(&mut *transaction);
            while let Some(row) = rows
                .try_next()
                .await
                .with_context(|| format!("read legacy table {table}"))?
            {
                let value: Value = row
                    .try_get("row_json")
                    .with_context(|| format!("decode legacy table {table} row"))?;
                validate_no_secrets(&value)
                    .with_context(|| format!("legacy table {table} exposed a forbidden field"))?;
                let bytes = canonical_json_bytes(&value)
                    .with_context(|| format!("serialize legacy table {table} row"))?;
                if bytes.len() > MAX_NDJSON_ROW_BYTES {
                    return Err(anyhow!(
                        "legacy table {table} contains a row larger than {MAX_NDJSON_ROW_BYTES} bytes"
                    ));
                }
                writer
                    .write_all(&bytes)
                    .and_then(|_| writer.write_all(b"\n"))
                    .with_context(|| format!("write legacy table {table}"))?;
                hasher.update(&bytes);
                hasher.update(b"\n");
                row_count = row_count
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("legacy table {table} row count overflow"))?;
            }
        }
        writer
            .flush()
            .with_context(|| format!("flush legacy table {table}"))?;
        tables.push(TableManifest {
            name: (*table).to_string(),
            file: format!("tables/{table}.ndjson"),
            row_count,
            sha256: hex::encode(hasher.finalize()),
        });
        schema_contract.insert((*table).to_string(), columns);
    }

    transaction
        .rollback()
        .await
        .context("close read-only source transaction")?;
    pool.close().await;

    let media = export_media(output, &config.media)?;
    let source_schema_sha256 =
        canonical_sha256(&schema_contract).context("hash legacy schema contract")?;
    let mut manifest = ExportManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        source_system: SOURCE_SYSTEM.to_string(),
        source_workspace_id: config.source_workspace_id,
        source_schema_sha256,
        exported_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        dataset_sha256: String::new(),
        tables,
        media,
        excluded_fields: excluded_fields.into_iter().collect(),
    };
    manifest.dataset_sha256 = manifest
        .computed_dataset_sha256()
        .context("hash export manifest")?;
    manifest
        .validate()
        .context("validate generated export manifest")?;
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    write_new(&output.join("manifest.json"), &manifest_bytes)?;

    let source_row_count = manifest.tables.iter().try_fold(0_u64, |total, table| {
        total
            .checked_add(table.row_count)
            .ok_or_else(|| anyhow!("source row count overflow"))
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "operation": "export",
            "valid": true,
            "read_only": true,
            "source_workspace_id": manifest.source_workspace_id,
            "dataset_sha256": manifest.dataset_sha256,
            "table_count": manifest.tables.len(),
            "source_row_count": source_row_count,
            "media_count": manifest.media.len(),
            "output": output
        }))?
    );
    Ok(0)
}

fn read_config(path: &Path) -> Result<LegacyExportConfig> {
    let metadata = fs::metadata(path).context("read source configuration metadata")?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_CONFIG_BYTES {
        return Err(anyhow!(
            "source configuration must be a regular JSON file between 1 and {MAX_CONFIG_BYTES} bytes"
        ));
    }
    let bytes = fs::read(path).context("read source configuration")?;
    serde_json::from_slice(&bytes).context("parse source configuration JSON")
}

fn validate_config(config: &LegacyExportConfig) -> Result<()> {
    if config.schema_version != CONFIG_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported source configuration schema_version {}",
            config.schema_version
        ));
    }
    let url = url::Url::parse(&config.database_url).context("invalid source database URL")?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        return Err(anyhow!(
            "source database URL must use postgres or postgresql"
        ));
    }
    validate_identifier(&config.schema, "source schema")?;
    let mut sources = BTreeSet::new();
    for media in &config.media {
        if media.source.trim().is_empty() || !sources.insert(media.source.as_str()) {
            return Err(anyhow!(
                "media source identifiers must be non-empty and unique"
            ));
        }
        validate_mime_type(&media.mime_type)?;
    }
    Ok(())
}

async fn load_columns(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    schema: &str,
    table: &str,
) -> Result<Vec<SchemaColumn>> {
    sqlx::query_as::<_, SchemaColumn>(
        "SELECT column_name, data_type, udt_name, ordinal_position::int \
         FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = $2 \
         ORDER BY ordinal_position",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(&mut **transaction)
    .await
    .with_context(|| format!("inspect legacy table {schema}.{table}"))
}

fn permitted_columns(
    table: &str,
    columns: &[SchemaColumn],
    excluded_fields: &mut BTreeSet<String>,
) -> Result<Vec<String>> {
    let mut permitted = Vec::new();
    for column in columns {
        if validate_no_secrets(&json!({column.column_name.clone(): null})).is_ok() {
            permitted.push(column.column_name.clone());
        } else {
            excluded_fields.insert(format!("{table}.{}", column.column_name));
        }
    }
    if permitted.is_empty() {
        return Err(anyhow!("legacy table {table} has no exportable columns"));
    }
    Ok(permitted)
}

fn validate_workspace_filter_contract(table: &str, columns: &[SchemaColumn]) -> Result<()> {
    let names = columns
        .iter()
        .map(|column| column.column_name.as_str())
        .collect::<BTreeSet<_>>();
    let required = match table {
        "workspaces" => &["id"][..],
        "clip_transcript_segments" => &["clip_candidate_id"][..],
        _ => &["workspace_id"][..],
    };
    for name in required {
        if !names.contains(name) {
            return Err(anyhow!(
                "legacy table {table} is missing required tenant column {name}"
            ));
        }
    }
    Ok(())
}

fn export_query(schema: &str, table: &str, columns: &[String]) -> String {
    let schema = quote_identifier(schema);
    let table_name = quote_identifier(table);
    let selected = columns
        .iter()
        .map(|column| format!("source.{}", quote_identifier(column)))
        .collect::<Vec<_>>()
        .join(", ");
    let predicate = match table {
        "workspaces" => "source.\"id\"::text = $1".to_string(),
        "clip_transcript_segments" => format!(
            "EXISTS (SELECT 1 FROM {schema}.\"clip_candidates\" owner \
             WHERE owner.\"id\" = source.\"clip_candidate_id\" \
             AND owner.\"workspace_id\"::text = $1)"
        ),
        _ => "source.\"workspace_id\"::text = $1".to_string(),
    };
    format!(
        "SELECT to_jsonb(export_row) AS row_json \
         FROM (SELECT {selected} FROM {schema}.{table_name} source WHERE {predicate}) export_row \
         ORDER BY md5(to_jsonb(export_row)::text), to_jsonb(export_row)::text"
    )
}

fn export_media(output: &Path, sources: &[LegacyMediaSource]) -> Result<Vec<MediaManifest>> {
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    fs::create_dir(output.join("attachments")).context("create export attachments directory")?;
    let mut manifest = Vec::with_capacity(sources.len());
    for source in sources {
        let canonical = fs::canonicalize(&source.path)
            .with_context(|| format!("resolve media source {}", source.source))?;
        let metadata = fs::metadata(&canonical)
            .with_context(|| format!("inspect media source {}", source.source))?;
        if !metadata.is_file() || metadata.len() > MAX_MEDIA_BYTES {
            return Err(anyhow!(
                "media source {} must be a regular file no larger than {MAX_MEDIA_BYTES} bytes",
                source.source
            ));
        }
        let sha256 = hash_file(&canonical)?;
        let extension = canonical
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 16
                    && value
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
            })
            .map(|value| format!(".{}", value.to_ascii_lowercase()))
            .unwrap_or_default();
        let relative = format!("attachments/{sha256}{extension}");
        let destination = output.join(&relative);
        if destination.exists() {
            return Err(anyhow!(
                "duplicate media bytes in export: {}",
                source.source
            ));
        }
        fs::copy(&canonical, &destination)
            .with_context(|| format!("copy media source {}", source.source))?;
        if hash_file(&destination)? != sha256 {
            return Err(anyhow!("copied media hash mismatch for {}", source.source));
        }
        manifest.push(MediaManifest {
            source: source.source.clone(),
            file: relative,
            sha256,
            mime_type: source.mime_type.to_ascii_lowercase(),
            size: metadata.len(),
        });
    }
    manifest.sort_by(|left, right| left.source.cmp(&right.source));
    Ok(manifest)
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).context("open file for SHA-256")?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).context("read file for SHA-256")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .context("create export manifest")?;
    let mut writer = BufWriter::new(file);
    writer.write_all(bytes).context("write export manifest")?;
    writer.write_all(b"\n").context("finish export manifest")?;
    writer.flush().context("flush export manifest")
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    let mut characters = value.chars();
    let starts_safely = characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    if !starts_safely
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
        || value.len() > 63
    {
        return Err(anyhow!(
            "{label} must be a PostgreSQL identifier of at most 63 characters"
        ));
    }
    Ok(())
}

fn validate_mime_type(value: &str) -> Result<()> {
    let lower = value.to_ascii_lowercase();
    let Some((major, minor)) = lower.split_once('/') else {
        return Err(anyhow!("media MIME type must contain one slash"));
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part.len() <= 127
            && part.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(
                        character,
                        '!' | '#' | '$' | '&' | '^' | '_' | '.' | '+' | '-'
                    )
            })
    };
    if value != value.trim() || value.contains(';') || !valid(major) || !valid(minor) {
        return Err(anyhow!(
            "media MIME type must be a normalized type/subtype without parameters"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, ordinal_position: i32) -> SchemaColumn {
        SchemaColumn {
            column_name: name.to_string(),
            data_type: "text".to_string(),
            udt_name: "text".to_string(),
            ordinal_position,
        }
    }

    #[test]
    fn secret_columns_are_excluded_before_row_selection() {
        let mut excluded = BTreeSet::new();
        let selected = permitted_columns(
            "memberships",
            &[
                column("id", 1),
                column("workspace_id", 2),
                column("authUserId", 3),
                column("session_token", 4),
                column("display_name", 5),
            ],
            &mut excluded,
        )
        .expect("permitted columns");
        assert_eq!(selected, vec!["id", "workspace_id", "display_name"]);
        assert_eq!(
            excluded.into_iter().collect::<Vec<_>>(),
            vec!["memberships.authUserId", "memberships.session_token"]
        );
    }

    #[test]
    fn every_query_is_workspace_scoped_and_deterministically_ordered() {
        let ordinary = export_query(
            "public",
            "guests",
            &["id".to_string(), "workspace_id".to_string()],
        );
        assert!(ordinary.contains("source.\"workspace_id\"::text = $1"));
        assert!(ordinary.contains("ORDER BY md5(to_jsonb(export_row)::text)"));

        let join = export_query(
            "public",
            "clip_transcript_segments",
            &["clip_candidate_id".to_string()],
        );
        assert!(join.contains("\"clip_candidates\" owner"));
        assert!(join.contains("owner.\"workspace_id\"::text = $1"));

        let workspace = export_query("public", "workspaces", &["id".to_string()]);
        assert!(workspace.contains("source.\"id\"::text = $1"));
    }

    #[test]
    fn configuration_rejects_non_postgres_urls_and_ambiguous_media() {
        let mut config = LegacyExportConfig {
            schema_version: 1,
            database_url: "https://example.test".to_string(),
            source_workspace_id: Uuid::nil(),
            schema: "public".to_string(),
            media: Vec::new(),
        };
        assert!(validate_config(&config).is_err());
        config.database_url = "postgresql://operator@example.test/source".to_string();
        config.media = vec![
            LegacyMediaSource {
                source: "interview:1".to_string(),
                path: PathBuf::from("one.vtt"),
                mime_type: "text/vtt".to_string(),
            },
            LegacyMediaSource {
                source: "interview:1".to_string(),
                path: PathBuf::from("two.vtt"),
                mime_type: "text/vtt; charset=utf-8".to_string(),
            },
        ];
        assert!(validate_config(&config).is_err());
    }
}
