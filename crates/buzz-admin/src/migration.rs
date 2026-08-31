//! Operator controls for the one-time MK Ideas Command Center migration.
//!
//! Inspection and planning remain offline and secret-free. Apply/resume are
//! the only networked commands; they require explicit artifact, relay, key-file,
//! and checkpoint arguments and never read the source Command Center.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use buzz_migration::dry_run::{build_offline_dry_run, OfflinePlanningCheckpoint};
use buzz_migration::ids::batch_id;
use buzz_migration::manifest::ExportManifest;
use buzz_migration::mapping::MigrationMapping;
use buzz_migration::plan::MigrationPlan;
use buzz_migration::report::MigrationReport;
use buzz_migration::secrets::validate_no_secrets;
use clap::Subcommand;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::migration_submit::{
    reconcile_destination, submit_dry_run, DestinationReconcileOptions, SubmitOptions,
};

const ARTIFACT_SCHEMA_VERSION: u32 = 1;
const RECONCILIATION_MISMATCH_EXIT_CODE: i32 = 4;
const MAX_NDJSON_ROW_BYTES: usize = 4 * 1024 * 1024;

/// Offline-safe MK Ideas migration operator commands.
#[derive(Subcommand)]
pub enum MigrationCommand {
    /// Validate every table and media object in an existing export bundle.
    Inspect {
        /// Directory containing manifest.json and its declared files.
        #[arg(long)]
        bundle: PathBuf,
    },
    /// Export a legacy Command Center snapshot through an explicit read-only configuration.
    Export {
        /// JSON file containing the source URL, workspace UUID, and optional media paths.
        #[arg(long)]
        source_config: PathBuf,
        /// New directory that will receive manifest.json, tables, and attachments.
        #[arg(long)]
        output: PathBuf,
    },
    /// Validate an existing bundle, identity mapping, and immutable plan.
    Plan {
        /// Directory containing manifest.json and its declared files.
        #[arg(long)]
        bundle: PathBuf,
        /// Explicit legacy-membership and collision mapping JSON.
        #[arg(long)]
        mapping: PathBuf,
        /// Existing immutable migration plan JSON.
        #[arg(long)]
        plan: PathBuf,
    },
    /// Build a complete deterministic local plan and reconciliation report.
    DryRun {
        /// Directory containing manifest.json and its declared files.
        #[arg(long)]
        bundle: PathBuf,
        /// Explicit legacy-membership and collision mapping JSON.
        #[arg(long)]
        mapping: PathBuf,
        /// Optional interruption checkpoint to validate before resuming.
        #[arg(long)]
        checkpoint: Option<PathBuf>,
    },
    /// Apply an offline dry-run artifact to one explicitly named relay.
    Apply {
        /// Complete JSON emitted by `migration dry-run`.
        #[arg(long)]
        artifact: PathBuf,
        /// Inspected export bundle containing media declared by the artifact.
        #[arg(long)]
        bundle: PathBuf,
        /// Explicit relay WebSocket URL; no environment fallback is used.
        #[arg(long)]
        relay: String,
        /// File containing only the granted migration service secret key.
        #[arg(long)]
        service_key_file: PathBuf,
        /// Optional JSON NIP-OA authorization tag for the service identity.
        #[arg(long)]
        auth_tag_file: Option<PathBuf>,
        /// Non-secret checkpoint updated after each accepted item.
        #[arg(long)]
        checkpoint_out: PathBuf,
    },
    /// Resume an interrupted, idempotent relay submission.
    Resume {
        /// Complete JSON emitted by `migration dry-run`.
        #[arg(long)]
        artifact: PathBuf,
        /// Inspected export bundle containing media declared by the artifact.
        #[arg(long)]
        bundle: PathBuf,
        /// Existing non-secret checkpoint from `apply` or `resume`.
        #[arg(long)]
        checkpoint: PathBuf,
        /// Explicit relay WebSocket URL; no environment fallback is used.
        #[arg(long)]
        relay: String,
        /// File containing only the granted migration service secret key.
        #[arg(long)]
        service_key_file: PathBuf,
        /// Optional JSON NIP-OA authorization tag for the service identity.
        #[arg(long)]
        auth_tag_file: Option<PathBuf>,
        /// Non-secret checkpoint updated after each accepted item.
        #[arg(long)]
        checkpoint_out: PathBuf,
    },
    /// Verify an existing report against its bundle, mapping, and plan.
    Verify {
        /// Directory containing manifest.json and its declared files.
        #[arg(long)]
        bundle: PathBuf,
        /// Explicit legacy-membership and collision mapping JSON.
        #[arg(long)]
        mapping: PathBuf,
        /// Existing immutable migration plan JSON.
        #[arg(long)]
        plan: PathBuf,
        /// Existing migration report JSON.
        #[arg(long)]
        report: PathBuf,
    },
    /// Reconcile source counts and plan outcomes using local artifacts only.
    Reconcile {
        /// Directory containing manifest.json and its declared files.
        #[arg(long)]
        bundle: PathBuf,
        /// Explicit legacy-membership and collision mapping JSON.
        #[arg(long)]
        mapping: PathBuf,
        /// Existing immutable migration plan JSON.
        #[arg(long)]
        plan: PathBuf,
        /// Existing migration report JSON.
        #[arg(long)]
        report: PathBuf,
        /// Optional explicit relay URL for read-only destination receipt verification.
        #[arg(long, requires = "service_key_file")]
        relay: Option<String>,
        /// Migration service secret-key file used only for relay authentication.
        #[arg(long, requires = "relay")]
        service_key_file: Option<PathBuf>,
        /// Optional NIP-OA authorization tag for the migration service.
        #[arg(long, requires = "relay")]
        auth_tag_file: Option<PathBuf>,
    },
    /// Validate and summarize an existing migration report.
    Report {
        /// Existing migration report JSON.
        #[arg(long)]
        report: PathBuf,
    },
}

/// Run one migration command.
///
/// Export reads only its explicit source configuration and opens a read-only
/// source transaction. Apply/resume read only their explicit relay credential
/// files. No command discovers credentials from the environment, and no
/// command writes to the legacy Command Center.
pub async fn run(command: MigrationCommand) -> Result<i32> {
    match command {
        MigrationCommand::Inspect { bundle } => {
            let inspected = inspect_bundle(&bundle)?;
            print_json(json!({
                "operation": "inspect",
                "valid": true,
                "source_workspace_id": inspected.manifest.source_workspace_id,
                "dataset_sha256": inspected.manifest.dataset_sha256,
                "table_count": inspected.manifest.tables.len(),
                "source_row_count": inspected.source_row_count,
                "media_count": inspected.manifest.media.len(),
                "media_bytes": inspected.media_bytes,
                "read_only": true
            }))?;
            Ok(0)
        }
        MigrationCommand::Export {
            source_config,
            output,
        } => crate::migration_export::export_legacy_snapshot(&source_config, &output).await,
        MigrationCommand::Plan {
            bundle,
            mapping,
            plan,
        } => {
            let validated = validate_plan_artifacts(&bundle, &mapping, &plan)?;
            print_plan_summary("plan", &validated)?;
            Ok(0)
        }
        MigrationCommand::DryRun {
            bundle,
            mapping,
            checkpoint,
        } => {
            let mapping: MigrationMapping = read_secret_free_json(&mapping, "mapping")?;
            let output = build_offline_dry_run(&bundle, &mapping)
                .context("offline migration dry-run failed")?;
            if let Some(checkpoint) = checkpoint {
                let checkpoint: OfflinePlanningCheckpoint =
                    read_secret_free_json(&checkpoint, "planning checkpoint")?;
                output
                    .validate_checkpoint(&checkpoint)
                    .context("planning checkpoint validation failed")?;
            }
            print_serialized(&output)?;
            Ok(0)
        }
        MigrationCommand::Apply {
            artifact,
            bundle,
            relay,
            service_key_file,
            auth_tag_file,
            checkpoint_out,
        } => {
            submit_dry_run(SubmitOptions {
                artifact,
                bundle,
                checkpoint: None,
                relay,
                service_key_file,
                auth_tag_file,
                checkpoint_out,
            })
            .await
        }
        MigrationCommand::Resume {
            artifact,
            bundle,
            checkpoint,
            relay,
            service_key_file,
            auth_tag_file,
            checkpoint_out,
        } => {
            submit_dry_run(SubmitOptions {
                artifact,
                bundle,
                checkpoint: Some(checkpoint),
                relay,
                service_key_file,
                auth_tag_file,
                checkpoint_out,
            })
            .await
        }
        MigrationCommand::Verify {
            bundle,
            mapping,
            plan,
            report,
        } => {
            let validated = validate_all_artifacts(&bundle, &mapping, &plan, &report)?;
            print_json(json!({
                "operation": "verify",
                "valid": true,
                "offline_only": true,
                "relay_checked": false,
                "batch_id": validated.plan.plan.batch_id,
                "dataset_sha256": validated.plan.inspected.manifest.dataset_sha256,
                "plan_sha256": validated.plan.plan_sha256,
                "planned_units": validated.plan.plan.units.len(),
                "source_row_count": validated.plan.inspected.source_row_count,
                "report_clean": validated.report.is_clean(),
                "reported_failures": validated.report.outcomes.failed,
                "reported_warnings": validated.report.warnings.len()
            }))?;
            Ok(0)
        }
        MigrationCommand::Reconcile {
            bundle,
            mapping,
            plan,
            report,
            relay,
            service_key_file,
            auth_tag_file,
        } => {
            let validated = validate_all_artifacts(&bundle, &mapping, &plan, &report)?;
            let mut mismatches = reconciliation_mismatches(&validated);
            let destination = match (relay.as_deref(), service_key_file.as_deref()) {
                (Some(relay), Some(service_key_file)) => {
                    let result = reconcile_destination(
                        &validated.plan.plan,
                        DestinationReconcileOptions {
                            relay,
                            service_key_file,
                            auth_tag_file: auth_tag_file.as_deref(),
                        },
                    )
                    .await?;
                    mismatches.extend(result.mismatches);
                    Some(json!({
                        "relay": relay,
                        "receipt_count": result.receipt_count,
                        "checked": true
                    }))
                }
                (None, None) => None,
                _ => {
                    return Err(anyhow!(
                        "relay and service key file must be supplied together"
                    ))
                }
            };
            mismatches.sort();
            mismatches.dedup();
            let is_clean = mismatches.is_empty();
            print_json(json!({
                "operation": "reconcile",
                "valid": is_clean,
                "offline_only": destination.is_none(),
                "relay_checked": destination.is_some(),
                "destination": destination,
                "batch_id": validated.plan.plan.batch_id,
                "dataset_sha256": validated.plan.inspected.manifest.dataset_sha256,
                "mismatches": mismatches
            }))?;
            if is_clean {
                Ok(0)
            } else {
                Ok(RECONCILIATION_MISMATCH_EXIT_CODE)
            }
        }
        MigrationCommand::Report { report } => {
            let report = read_report(&report)?;
            let source_row_count = checked_sum_counts(&report.source_rows, "source row")?;
            let destination_event_count =
                checked_sum_counts(&report.destination_events, "destination event")?;
            print_json(json!({
                "operation": "report",
                "valid": true,
                "batch_id": report.batch_id,
                "dataset_sha256": report.dataset_sha256,
                "community": report.community,
                "source_table_count": report.source_rows.len(),
                "source_row_count": source_row_count,
                "destination_event_count": destination_event_count,
                "planned_units": report.outcomes.planned,
                "created_units": report.outcomes.created,
                "already_present_units": report.outcomes.already_present,
                "linked_units": report.outcomes.linked,
                "skipped_units": report.outcomes.skipped,
                "failed_units": report.outcomes.failed,
                "warning_count": report.warnings.len(),
                "error_count": report.errors.len(),
                "clean": report.is_clean(),
                "read_only": true
            }))?;
            Ok(0)
        }
    }
}

#[derive(Debug)]
struct InspectedBundle {
    manifest: ExportManifest,
    source_row_count: u64,
    media_bytes: u64,
}

#[derive(Debug)]
struct ValidatedPlanArtifacts {
    inspected: InspectedBundle,
    mapping: MigrationMapping,
    plan: MigrationPlan,
    plan_sha256: String,
}

#[derive(Debug)]
struct ValidatedAllArtifacts {
    plan: ValidatedPlanArtifacts,
    report: MigrationReport,
}

fn inspect_bundle(bundle: &Path) -> Result<InspectedBundle> {
    let bundle_root = canonical_directory(bundle, "bundle")?;
    let manifest_path = confined_bundle_file(&bundle_root, "manifest.json")?;
    let manifest: ExportManifest = read_secret_free_json(&manifest_path, "manifest")?;
    manifest.validate().context("manifest validation failed")?;
    validate_sha256(&manifest.source_schema_sha256, "manifest source schema")?;
    validate_sha256(&manifest.dataset_sha256, "manifest dataset")?;

    let mut source_row_count = 0_u64;
    for table in &manifest.tables {
        let table_path = confined_bundle_file(&bundle_root, &table.file)?;
        validate_file_hash(&table_path, &table.sha256, &format!("table {}", table.name))?;

        let mut actual_rows = 0_u64;
        let table_file = File::open(&table_path)
            .with_context(|| format!("failed to open table file {}", table.file))?;
        for (index, line) in BufReader::new(table_file).split(b'\n').enumerate() {
            let line = line.with_context(|| format!("failed to read table file {}", table.file))?;
            if line.len() > MAX_NDJSON_ROW_BYTES {
                return Err(anyhow!(
                    "table {} logical line {} exceeds the {} byte safety limit",
                    table.name,
                    index + 1,
                    MAX_NDJSON_ROW_BYTES
                ));
            }
            let line = trim_ascii_whitespace(&line);
            if line.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_slice(line).with_context(|| {
                format!(
                    "table {} contains invalid JSON on logical line {}",
                    table.name,
                    index + 1
                )
            })?;
            validate_no_secrets(&value)
                .with_context(|| format!("table {} failed secret-field validation", table.name))?;
            actual_rows = actual_rows
                .checked_add(1)
                .ok_or_else(|| anyhow!("table {} row count overflow", table.name))?;
        }
        if actual_rows != table.row_count {
            return Err(anyhow!(
                "table {} row count mismatch: manifest {}, file {}",
                table.name,
                table.row_count,
                actual_rows
            ));
        }
        source_row_count = source_row_count
            .checked_add(actual_rows)
            .ok_or_else(|| anyhow!("bundle source row count overflow"))?;
    }

    let mut media_bytes = 0_u64;
    for media in &manifest.media {
        let media_path = confined_bundle_file(&bundle_root, &media.file)?;
        validate_file_hash(
            &media_path,
            &media.sha256,
            &format!("media {}", media.source),
        )?;
        let actual_size = fs::metadata(&media_path)
            .with_context(|| format!("failed to inspect media file {}", media.file))?
            .len();
        if actual_size != media.size {
            return Err(anyhow!(
                "media {} size mismatch: manifest {}, file {}",
                media.source,
                media.size,
                actual_size
            ));
        }
        media_bytes = media_bytes
            .checked_add(actual_size)
            .ok_or_else(|| anyhow!("bundle media byte count overflow"))?;
    }

    Ok(InspectedBundle {
        manifest,
        source_row_count,
        media_bytes,
    })
}

fn validate_plan_artifacts(
    bundle: &Path,
    mapping_path: &Path,
    plan_path: &Path,
) -> Result<ValidatedPlanArtifacts> {
    let inspected = inspect_bundle(bundle)?;
    let mapping: MigrationMapping = read_secret_free_json(mapping_path, "mapping")?;
    mapping.validate().context("mapping validation failed")?;
    let plan: MigrationPlan = read_secret_free_json(plan_path, "plan")?;
    validate_plan(&inspected.manifest, &mapping, &plan)?;
    let plan_sha256 = plan.sha256().context("failed to hash migration plan")?;

    Ok(ValidatedPlanArtifacts {
        inspected,
        mapping,
        plan,
        plan_sha256,
    })
}

fn validate_plan(
    manifest: &ExportManifest,
    mapping: &MigrationMapping,
    plan: &MigrationPlan,
) -> Result<()> {
    if plan.schema_version != ARTIFACT_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported plan schema_version {}",
            plan.schema_version
        ));
    }
    validate_sha256(&plan.dataset_sha256, "plan dataset")?;
    if plan.dataset_sha256 != manifest.dataset_sha256 {
        return Err(anyhow!("plan dataset hash does not match bundle manifest"));
    }
    if mapping.workspace_id != manifest.source_workspace_id {
        return Err(anyhow!("mapping workspace does not match bundle workspace"));
    }
    if mapping.community != plan.community {
        return Err(anyhow!("mapping community does not match plan community"));
    }
    if plan.community.trim().is_empty() {
        return Err(anyhow!("plan community is empty"));
    }
    let expected_batch = batch_id(manifest.source_workspace_id, &manifest.dataset_sha256);
    if plan.batch_id != expected_batch {
        return Err(anyhow!(
            "plan batch ID is not deterministic for this dataset"
        ));
    }

    for unit in &plan.units {
        if unit.source.system != manifest.source_system {
            return Err(anyhow!(
                "plan unit {} has an unexpected source system",
                unit.source
            ));
        }
        if unit.source.workspace_id != manifest.source_workspace_id {
            return Err(anyhow!(
                "plan unit {} belongs to a different workspace",
                unit.source
            ));
        }
        if unit.source.unit_kind.trim().is_empty()
            || unit.source.unit_id.trim().is_empty()
            || unit.source.revision == 0
        {
            return Err(anyhow!("plan unit has an incomplete source coordinate"));
        }
        validate_sha256(&unit.source_sha256, &format!("plan unit {}", unit.source))?;
        if unit
            .destination
            .as_ref()
            .is_some_and(|destination| destination.kind == 0)
        {
            return Err(anyhow!(
                "plan unit {} has destination kind zero",
                unit.source
            ));
        }
    }
    plan.ordered_units()
        .context("plan dependency validation failed")?;
    Ok(())
}

fn validate_all_artifacts(
    bundle: &Path,
    mapping: &Path,
    plan: &Path,
    report: &Path,
) -> Result<ValidatedAllArtifacts> {
    let plan = validate_plan_artifacts(bundle, mapping, plan)?;
    let report = read_report(report)?;
    if report.batch_id != plan.plan.batch_id {
        return Err(anyhow!("report batch ID does not match plan"));
    }
    if report.dataset_sha256 != plan.inspected.manifest.dataset_sha256 {
        return Err(anyhow!("report dataset hash does not match bundle"));
    }
    if report.community != plan.mapping.community {
        return Err(anyhow!("report community does not match mapping"));
    }
    Ok(ValidatedAllArtifacts { plan, report })
}

fn read_report(path: &Path) -> Result<MigrationReport> {
    let report: MigrationReport = read_secret_free_json(path, "report")?;
    validate_report(&report)?;
    Ok(report)
}

fn validate_report(report: &MigrationReport) -> Result<()> {
    if report.schema_version != ARTIFACT_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported report schema_version {}",
            report.schema_version
        ));
    }
    validate_sha256(&report.dataset_sha256, "report dataset")?;
    if report.community.trim().is_empty() {
        return Err(anyhow!("report community is empty"));
    }

    let accounted = report
        .outcomes
        .created
        .checked_add(report.outcomes.already_present)
        .and_then(|value| value.checked_add(report.outcomes.linked))
        .and_then(|value| value.checked_add(report.outcomes.skipped))
        .ok_or_else(|| anyhow!("report outcome count overflow"))?;
    let terminal = accounted
        .checked_add(report.outcomes.failed)
        .ok_or_else(|| anyhow!("report terminal outcome count overflow"))?;
    if terminal > report.outcomes.planned {
        return Err(anyhow!(
            "report accounts for {terminal} terminal outcomes but only {} were planned",
            report.outcomes.planned
        ));
    }
    if report.outcomes.warned > report.outcomes.planned {
        return Err(anyhow!("report warning count exceeds planned units"));
    }
    Ok(())
}

fn reconciliation_mismatches(validated: &ValidatedAllArtifacts) -> Vec<String> {
    let expected_source_rows = validated
        .plan
        .inspected
        .manifest
        .tables
        .iter()
        .map(|table| (table.name.clone(), table.row_count))
        .collect::<BTreeMap<_, _>>();
    let mut mismatches = Vec::new();
    for (table, expected) in &expected_source_rows {
        match validated.report.source_rows.get(table) {
            Some(actual) if actual == expected => {}
            Some(actual) => mismatches.push(format!(
                "source table {table}: bundle {expected}, report {actual}"
            )),
            None => mismatches.push(format!("source table {table}: missing from report")),
        }
    }
    for table in validated.report.source_rows.keys() {
        if !expected_source_rows.contains_key(table) {
            mismatches.push(format!("source table {table}: absent from bundle"));
        }
    }
    let planned = u64::try_from(validated.plan.plan.units.len()).unwrap_or(u64::MAX);
    if validated.report.outcomes.planned != planned {
        mismatches.push(format!(
            "planned units: plan {planned}, report {}",
            validated.report.outcomes.planned
        ));
    }
    mismatches.sort();
    mismatches
}

fn read_secret_free_json<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {label} file"))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("{label} file is not valid JSON"))?;
    validate_no_secrets(&value).with_context(|| format!("{label} contains forbidden fields"))?;
    serde_json::from_value(value).with_context(|| format!("{label} has an invalid schema"))
}

fn checked_sum_counts<Key: Ord>(counts: &BTreeMap<Key, u64>, label: &str) -> Result<u64> {
    counts.values().try_fold(0_u64, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| anyhow!("{label} count overflow"))
    })
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical =
        fs::canonicalize(path).with_context(|| format!("failed to resolve {label} directory"))?;
    if !canonical.is_dir() {
        return Err(anyhow!("{label} path is not a directory"));
    }
    Ok(canonical)
}

fn confined_bundle_file(bundle_root: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = bundle_root.join(relative);
    let canonical = fs::canonicalize(&candidate)
        .with_context(|| format!("failed to resolve bundle file {relative}"))?;
    if !canonical.starts_with(bundle_root) {
        return Err(anyhow!("bundle file escapes bundle root: {relative}"));
    }
    if !canonical.is_file() {
        return Err(anyhow!("bundle path is not a regular file: {relative}"));
    }
    Ok(canonical)
}

fn validate_file_hash(path: &Path, expected: &str, label: &str) -> Result<()> {
    validate_sha256(expected, label)?;
    let mut file = File::open(path).with_context(|| format!("failed to open {label}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to hash {label}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected.to_ascii_lowercase() {
        return Err(anyhow!(
            "{label} SHA-256 mismatch: expected {expected}, computed {actual}"
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value != value.to_ascii_lowercase()
    {
        return Err(anyhow!(
            "{label} SHA-256 must be 64 lowercase hex characters"
        ));
    }
    Ok(())
}

fn trim_ascii_whitespace(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(|byte| byte.is_ascii_whitespace()) {
        value = &value[1..];
    }
    while value.last().is_some_and(|byte| byte.is_ascii_whitespace()) {
        value = &value[..value.len().saturating_sub(1)];
    }
    value
}

fn print_plan_summary(operation: &str, validated: &ValidatedPlanArtifacts) -> Result<()> {
    print_json(json!({
        "operation": operation,
        "valid": true,
        "batch_id": validated.plan.batch_id,
        "dataset_sha256": validated.inspected.manifest.dataset_sha256,
        "plan_sha256": validated.plan_sha256,
        "planned_units": validated.plan.units.len(),
        "source_row_count": validated.inspected.source_row_count,
        "community": validated.plan.community,
        "read_only": true
    }))
}

fn print_json(value: Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn print_serialized<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use buzz_migration::report::OutcomeCounts;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn synthetic_bundle_passes_read_only_inspection() {
        let bundle =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/mkideas-v0/migration");
        let inspected = inspect_bundle(&bundle).expect("synthetic bundle should validate");
        assert_eq!(inspected.manifest.tables.len(), 32);
        assert_eq!(inspected.manifest.media.len(), 1);
        assert!(inspected.source_row_count > 0);
    }

    #[test]
    fn report_rejects_more_terminal_outcomes_than_planned() {
        let report = MigrationReport {
            schema_version: 1,
            batch_id: Uuid::nil(),
            dataset_sha256: "a".repeat(64),
            community: "hub.mkideas.org".to_string(),
            source_rows: BTreeMap::new(),
            destination_events: BTreeMap::new(),
            outcomes: OutcomeCounts {
                planned: 1,
                created: 1,
                failed: 1,
                ..OutcomeCounts::default()
            },
            warnings: Vec::new(),
            errors: Vec::new(),
            exclusions: Vec::new(),
        };
        assert!(validate_report(&report).is_err());
    }
}
