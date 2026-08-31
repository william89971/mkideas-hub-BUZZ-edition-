//! Reconciliation reporting for deterministic offline migration plans.

use std::collections::BTreeMap;

use crate::bundle::{LoadedBundle, MediaIntegrity};
use crate::dry_run_model::{PlannedMedia, PlannedRecord};
use crate::plan::{MigrationPlan, PlannedAction};
use crate::report::{MigrationReport, OutcomeCounts};
use crate::{MigrationError, Result};

pub(crate) fn build_report(
    bundle: &LoadedBundle,
    plan: &MigrationPlan,
    records: &[PlannedRecord],
    warnings: Vec<String>,
    errors: Vec<String>,
) -> Result<MigrationReport> {
    let source_rows = bundle
        .manifest
        .tables
        .iter()
        .map(|table| (table.name.clone(), table.row_count))
        .collect::<BTreeMap<_, _>>();
    let mut destination_events = BTreeMap::new();
    let mut linked = 0_u64;
    let mut skipped = 0_u64;
    for record in records {
        match record.action {
            PlannedAction::Create => {
                if let Some(destination) = &record.destination {
                    *destination_events.entry(destination.kind).or_insert(0) += 1;
                }
            }
            PlannedAction::Link => linked += 1,
            PlannedAction::Skip => skipped += 1,
        }
    }
    let planned = u64::try_from(plan.units.len()).map_err(|_| {
        MigrationError::InvalidSource("plan unit count does not fit u64".to_string())
    })?;
    let mut report = MigrationReport {
        schema_version: 1,
        batch_id: plan.batch_id,
        dataset_sha256: plan.dataset_sha256.clone(),
        community: plan.community.clone(),
        source_rows,
        destination_events,
        outcomes: OutcomeCounts {
            planned,
            linked,
            skipped,
            warned: u64::try_from(warnings.len())
                .unwrap_or(u64::MAX)
                .min(planned),
            failed: 0,
            ..OutcomeCounts::default()
        },
        warnings,
        errors,
        exclusions: bundle.manifest.excluded_fields.clone(),
    };
    report.normalize();
    Ok(report)
}

pub(crate) fn media_exception(media: &PlannedMedia) -> Option<String> {
    match &media.integrity {
        MediaIntegrity::Ready if media.related.is_some() => None,
        MediaIntegrity::Ready => Some(format!("media_unresolved:{}", media.source)),
        MediaIntegrity::Missing => Some(format!("media_missing:{}", media.source)),
        MediaIntegrity::Corrupt { .. } => Some(format!("media_corrupt:{}", media.source)),
        MediaIntegrity::Unreadable { reason } => {
            Some(format!("media_unreadable:{}:{reason}", media.source))
        }
    }
}
