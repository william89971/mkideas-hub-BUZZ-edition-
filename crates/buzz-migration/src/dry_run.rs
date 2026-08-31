//! Deterministic, write-free planning of legacy Command Center bundles.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::bundle::{load_bundle, LoadedBundle, MediaIntegrity, SourceRow};
use crate::canonical::canonical_sha256;
use crate::dry_run_content::event_safe_legacy;
use crate::dry_run_model::DRY_RUN_SCHEMA_VERSION;
pub use crate::dry_run_model::{
    OfflineDryRun, OfflinePlanningCheckpoint, PlannedAuthorship, PlannedMedia, PlannedRecord,
    PlannedRecordClass,
};
use crate::dry_run_report::{build_report, media_exception};
use crate::dry_run_validation::{
    validate_membership_references, validate_source_versions, validate_team_history_target,
};
use crate::ids::{batch_id, destination_id};
use crate::mapping::{MigrationMapping, OverrideStrategy};
use crate::plan::{MigrationPlan, PlannedAction, PlannedDestination, PlannedUnit};
use crate::receipt::SourceCoordinate;
use crate::status::{
    approval_status, content_status, guest_status, interview_status, task_status, StatusMapping,
};
use crate::{MigrationError, Result};

/// NIP-MK schema written by migration plans.
pub const MK_SCHEMA_VERSION: u32 = 2;
const KIND_TEAM_HISTORY: u32 = 9;
const KIND_TASK: u32 = 30_802;
const KIND_PERSON: u32 = 30_803;
const KIND_INTERVIEW: u32 = 30_804;
const KIND_CONTENT: u32 = 30_805;
const KIND_KNOWLEDGE: u32 = 30_808;
const KIND_APPROVAL: u32 = 30_809;
const KIND_APPROVAL_ACTION: u32 = 48_200;
const KIND_AGENT_RESULT: u32 = 48_201;
const KIND_MIGRATION_RECEIPT: u32 = 48_202;
const KIND_SYSTEM_ACTIVITY: u32 = 48_204;
const KIND_COMMUNICATION: u32 = 48_205;

/// Load, map, order, and reconcile a bundle without source, relay, or media
/// writes and without reading any credential.
pub fn build_offline_dry_run(
    bundle_root: &Path,
    mapping: &MigrationMapping,
) -> Result<OfflineDryRun> {
    mapping.validate()?;
    let bundle = load_bundle(bundle_root)?;
    if bundle.manifest.source_workspace_id != mapping.workspace_id {
        return Err(MigrationError::IdentityConflict(
            "bundle workspace does not match mapping workspace".to_string(),
        ));
    }
    validate_source_versions(&bundle)?;
    validate_team_history_target(&bundle, mapping)?;
    validate_membership_references(&bundle, mapping)?;

    let coordinates = coordinate_index(&bundle);
    let mut warnings = Vec::new();
    let mut records = bundle
        .ordered_rows()
        .into_iter()
        .map(|row| plan_source_row(&bundle, mapping, &coordinates, row, &mut warnings))
        .collect::<Result<Vec<_>>>()?;

    let mut planned_media = Vec::new();
    let mut media_dependencies = Vec::new();
    for media in &bundle.media {
        let planned = plan_media(&bundle, mapping, &coordinates, media, &mut warnings)?;
        if let Some(related) = planned.1.related.clone() {
            media_dependencies.push((related, planned.0.source.clone()));
        }
        records.push(planned.0);
        planned_media.push(planned.1);
    }
    for record in &mut records {
        if record.source.unit_kind != "transcript_segments" {
            continue;
        }
        for (related, media_source) in &media_dependencies {
            if record.dependencies.contains(related) {
                record.dependencies.push(media_source.clone());
            }
        }
        record.dependencies.sort();
        record.dependencies.dedup();
    }

    let destination_by_source = records
        .iter()
        .filter_map(|record| {
            record
                .destination
                .clone()
                .map(|destination| (record.source.clone(), destination))
        })
        .collect::<BTreeMap<_, _>>();
    for record in &mut records {
        for dependency in &record.dependencies {
            if let Some(destination) = destination_by_source.get(dependency) {
                if let Some(d_tag) = destination.d_tag {
                    record.tags.push(vec![
                        "mk".to_string(),
                        mapping.community.clone(),
                        destination.kind.to_string(),
                        d_tag.to_string(),
                    ]);
                }
                if record.destination.as_ref().is_some_and(|current| {
                    current.kind == destination.kind && current.d_tag == destination.d_tag
                }) {
                    record
                        .tags
                        .push(vec!["prev_source".to_string(), dependency.tag_value()]);
                }
            }
        }
        record.tags.sort();
        record.tags.dedup();
    }

    let unordered_plan = MigrationPlan {
        schema_version: DRY_RUN_SCHEMA_VERSION,
        dataset_sha256: bundle.manifest.dataset_sha256.clone(),
        community: mapping.community.clone(),
        batch_id: batch_id(
            bundle.manifest.source_workspace_id,
            &bundle.manifest.dataset_sha256,
        ),
        units: records.iter().map(planned_unit).collect(),
    };
    let ordered_sources = unordered_plan
        .ordered_units()?
        .into_iter()
        .map(|unit| unit.source.clone())
        .collect::<Vec<_>>();
    let mut record_by_source = records
        .into_iter()
        .map(|record| (record.source.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let mut ordered_records = Vec::with_capacity(ordered_sources.len());
    for source in &ordered_sources {
        let record = record_by_source.remove(source).ok_or_else(|| {
            MigrationError::InvalidSource(format!("missing planned record for {source}"))
        })?;
        ordered_records.push(record);
    }
    let plan = MigrationPlan {
        units: ordered_records.iter().map(planned_unit).collect(),
        ..unordered_plan
    };

    warnings.sort();
    warnings.dedup();
    let mut media_errors = planned_media
        .iter()
        .filter_map(media_exception)
        .collect::<Vec<_>>();
    media_errors.sort();
    media_errors.dedup();
    let report = build_report(&bundle, &plan, &ordered_records, warnings, media_errors)?;
    let placeholder = OfflinePlanningCheckpoint {
        schema_version: DRY_RUN_SCHEMA_VERSION,
        dataset_sha256: plan.dataset_sha256.clone(),
        plan_sha256: plan.sha256()?,
        batch_id: plan.batch_id,
        next_unit_index: 0,
        processed_prefix_sha256: canonical_sha256(&Vec::<String>::new())?,
    };
    let mut output = OfflineDryRun {
        schema_version: DRY_RUN_SCHEMA_VERSION,
        plan,
        records: ordered_records,
        media: planned_media,
        checkpoint: placeholder,
        report,
    };
    output.checkpoint = output.checkpoint_at(output.plan.units.len())?;
    Ok(output)
}

fn coordinate_index(bundle: &LoadedBundle) -> BTreeMap<(String, String), SourceCoordinate> {
    bundle
        .ordered_rows()
        .into_iter()
        .map(|row| {
            (
                (row.table.clone(), row.key.clone()),
                coordinate(bundle, &row.table, &row.key, source_revision(row)),
            )
        })
        .collect()
}

fn coordinate(bundle: &LoadedBundle, table: &str, key: &str, revision: u64) -> SourceCoordinate {
    SourceCoordinate {
        system: bundle.manifest.source_system.clone(),
        workspace_id: bundle.manifest.source_workspace_id,
        unit_kind: table.to_string(),
        unit_id: key.to_string(),
        revision,
    }
}

fn plan_source_row(
    bundle: &LoadedBundle,
    mapping: &MigrationMapping,
    coordinates: &BTreeMap<(String, String), SourceCoordinate>,
    row: &SourceRow,
    warnings: &mut Vec<String>,
) -> Result<PlannedRecord> {
    let spec = destination_spec(bundle, row)?;
    let source = coordinates
        .get(&(row.table.clone(), row.key.clone()))
        .cloned()
        .ok_or_else(|| MigrationError::InvalidSource("source coordinate missing".to_string()))?;
    let dependencies = dependencies(bundle, coordinates, row, warnings)?;
    let status = normalized_status(bundle, row)?;
    let authorship = authorship(mapping, row)?;
    let version = event_version(row);
    let source_created_at = source_timestamp(row);
    if source_created_at.is_none() {
        warnings.push(format!(
            "missing_source_timestamp:{}:{}",
            row.table, row.key
        ));
    }
    let record_id = destination_id(bundle.manifest.source_workspace_id, &row.table, &row.key);
    let (action, destination, reason) = apply_override(mapping, row, spec.kind, spec.d_tag)?;
    let content = migration_content(
        bundle,
        mapping,
        row,
        record_id,
        status.as_ref(),
        &authorship,
        reason.as_deref(),
    );
    let mut tags = base_tags(
        mapping,
        &source,
        &row.sha256,
        version,
        status.as_ref(),
        spec.class,
    );
    if let Some(destination) = &destination {
        if let Some(d_tag) = destination.d_tag {
            tags.push(vec!["d".to_string(), d_tag.to_string()]);
        }
    }
    if row.table == "guests" && is_do_not_contact(row) {
        tags.push(vec!["dnc".to_string(), "1".to_string()]);
    }
    Ok(PlannedRecord {
        record_id,
        class: spec.class,
        source,
        source_sha256: row.sha256.clone(),
        action,
        destination,
        source_created_at,
        version,
        authorship,
        dependencies,
        tags,
        content,
    })
}

fn plan_media(
    bundle: &LoadedBundle,
    mapping: &MigrationMapping,
    coordinates: &BTreeMap<(String, String), SourceCoordinate>,
    media: &crate::bundle::InspectedMedia,
    warnings: &mut Vec<String>,
) -> Result<(PlannedRecord, PlannedMedia)> {
    let related = resolve_media_source(coordinates, &media.manifest.source);
    if related.is_none() {
        warnings.push(format!(
            "unresolved_media_association:{}",
            media.manifest.source
        ));
    }
    let source_key =
        canonical_sha256(&(media.manifest.source.as_str(), media.manifest.file.as_str()))?;
    let source = coordinate(bundle, "media", &source_key, 1);
    let media_id = destination_id(bundle.manifest.source_workspace_id, "media", &source_key);
    let ready = matches!(media.integrity, MediaIntegrity::Ready) && related.is_some();
    let action = if ready {
        PlannedAction::Create
    } else {
        PlannedAction::Skip
    };
    let destination = ready.then_some(PlannedDestination {
        kind: KIND_MIGRATION_RECEIPT,
        d_tag: None,
    });
    let reason = (!ready).then(|| "attachment_not_ready".to_string());
    let content = json!({
        "schema_version": MK_SCHEMA_VERSION,
        "record_id": media_id,
        "community": mapping.community,
        "version": 1,
        "media": media.manifest,
        "integrity": media.integrity,
        "related_source": related,
        "migration": {
            "source_system": bundle.manifest.source_system,
            "source_workspace_id": bundle.manifest.source_workspace_id,
            "dataset_sha256": bundle.manifest.dataset_sha256,
            "source_table": "media",
            "source_id": source_key,
            "source_sha256": media.manifest.sha256,
            "source_coordinate": source.tag_value(),
            "dry_run_only": true,
            "skip_reason": reason
        }
    });
    let mut tags = base_tags(
        mapping,
        &source,
        &media.manifest.sha256,
        1,
        None,
        PlannedRecordClass::MediaReceipt,
    );
    tags.push(vec![
        "media_sha256".to_string(),
        media.manifest.sha256.clone(),
    ]);
    let dependencies = related.clone().into_iter().collect::<Vec<_>>();
    let record = PlannedRecord {
        record_id: media_id,
        class: PlannedRecordClass::MediaReceipt,
        source,
        source_sha256: media.manifest.sha256.clone(),
        action,
        destination,
        source_created_at: None,
        version: 1,
        authorship: PlannedAuthorship {
            signer: "migration_service".to_string(),
            original_membership_id: None,
            original_pubkey: None,
            original_actor_kind: "migration_media".to_string(),
        },
        dependencies,
        tags,
        content,
    };
    let planned = PlannedMedia {
        media_id,
        source: media.manifest.source.clone(),
        related,
        file: media.manifest.file.clone(),
        sha256: media.manifest.sha256.clone(),
        mime_type: media.manifest.mime_type.clone(),
        size: media.manifest.size,
        integrity: media.integrity.clone(),
    };
    Ok((record, planned))
}

#[derive(Debug, Clone, Copy)]
struct DestinationSpec {
    kind: u32,
    d_tag: Option<Uuid>,
    class: PlannedRecordClass,
}

fn destination_spec(bundle: &LoadedBundle, row: &SourceRow) -> Result<DestinationSpec> {
    let workspace = bundle.manifest.source_workspace_id;
    let own_d = || destination_id(workspace, &row.table, &row.key);
    let state = |kind, d_tag| DestinationSpec {
        kind,
        d_tag: Some(d_tag),
        class: PlannedRecordClass::State,
    };
    let operation = |kind| DestinationSpec {
        kind,
        d_tag: None,
        class: PlannedRecordClass::Operation,
    };
    let spec = match row.table.as_str() {
        "workspaces" | "memberships" | "audit_events" => operation(KIND_MIGRATION_RECEIPT),
        "organizations"
        | "guest_topics"
        | "guest_tags"
        | "guest_notes"
        | "outreach_draft_versions"
        | "interview_members"
        | "interview_questions"
        | "content_platforms" => state(KIND_KNOWLEDGE, own_d()),
        "guests" => state(KIND_PERSON, destination_id(workspace, "person", &row.key)),
        "guest_research_briefs"
        | "guest_research_themes"
        | "guest_research_facts"
        | "agent_runs"
        | "clip_candidates"
        | "clip_transcript_segments"
        | "content_qc_findings" => operation(KIND_AGENT_RESULT),
        "guest_communications" | "outreach_send_claims" => operation(KIND_COMMUNICATION),
        "interviews" => state(
            KIND_INTERVIEW,
            destination_id(workspace, "interview", &row.key),
        ),
        "transcript_segments" => DestinationSpec {
            kind: KIND_MIGRATION_RECEIPT,
            d_tag: None,
            class: PlannedRecordClass::MediaReceipt,
        },
        "content_items" => state(KIND_CONTENT, destination_id(workspace, "content", &row.key)),
        "content_versions" => {
            let parent = required_string(row, "content_item_id")?;
            state(KIND_CONTENT, destination_id(workspace, "content", parent))
        }
        "approvals" => state(
            KIND_APPROVAL,
            destination_id(workspace, "approval", &row.key),
        ),
        "approval_events" => operation(KIND_APPROVAL_ACTION),
        "tasks" => state(KIND_TASK, destination_id(workspace, "task", &row.key)),
        "attention_items" | "notifications" | "activity_events" => operation(KIND_SYSTEM_ACTIVITY),
        "comments" => DestinationSpec {
            kind: KIND_TEAM_HISTORY,
            d_tag: None,
            class: PlannedRecordClass::TeamHistory,
        },
        other => {
            return Err(MigrationError::InvalidSource(format!(
                "unsupported source table {other}"
            )));
        }
    };
    Ok(spec)
}

fn dependencies(
    bundle: &LoadedBundle,
    coordinates: &BTreeMap<(String, String), SourceCoordinate>,
    row: &SourceRow,
    warnings: &mut Vec<String>,
) -> Result<Vec<SourceCoordinate>> {
    let mut references = Vec::<(&str, String)>::new();
    macro_rules! add {
        ($table:literal, $field:literal) => {
            add_reference(row, &mut references, $table, $field)
        };
    }
    match row.table.as_str() {
        "guests" => {
            add!("organizations", "organization_id");
            add!("memberships", "owner_membership_id");
        }
        "guest_topics"
        | "guest_tags"
        | "guest_notes"
        | "guest_research_briefs"
        | "outreach_draft_versions"
        | "guest_communications" => {
            add!("guests", "guest_id");
            add!("memberships", "author_membership_id");
            add!("memberships", "created_by_membership_id");
        }
        "guest_research_themes" | "guest_research_facts" => {
            add!("guest_research_briefs", "research_brief_id");
        }
        "outreach_send_claims" => {
            add!("guests", "guest_id");
            add!("outreach_draft_versions", "outreach_draft_version_id");
            add!("memberships", "claimed_by_membership_id");
        }
        "interviews" => {
            add!("guests", "guest_id");
            add!("memberships", "owner_membership_id");
        }
        "interview_members" => {
            add!("interviews", "interview_id");
            add!("memberships", "membership_id");
        }
        "interview_questions" | "transcript_segments" => add!("interviews", "interview_id"),
        "content_items" => {
            add!("interviews", "interview_id");
            add!("guests", "guest_id");
            add!("memberships", "owner_membership_id");
            if let Some(versions) = bundle.tables.get("content_versions") {
                if let Some(version) = versions
                    .iter()
                    .filter(|version| {
                        optional_string(version, "content_item_id") == Some(row.key.as_str())
                    })
                    .max_by_key(|version| source_revision(version))
                {
                    references.push(("content_versions", version.key.clone()));
                }
            }
        }
        "content_versions" => {
            add!("memberships", "edited_by_membership_id");
            let parent = required_string(row, "content_item_id")?;
            let revision = source_revision(row);
            if let Some(previous) = bundle.tables.get("content_versions").and_then(|versions| {
                versions
                    .iter()
                    .filter(|candidate| {
                        optional_string(candidate, "content_item_id") == Some(parent)
                            && source_revision(candidate) < revision
                    })
                    .max_by_key(|candidate| source_revision(candidate))
            }) {
                references.push(("content_versions", previous.key.clone()));
            }
        }
        "content_platforms" => add!("content_items", "content_item_id"),
        "clip_candidates" | "content_qc_findings" => {
            add!("content_versions", "content_version_id")
        }
        "clip_transcript_segments" => {
            add!("clip_candidates", "clip_candidate_id");
            add!("transcript_segments", "transcript_segment_id");
        }
        "agent_runs" => {
            add!("memberships", "triggered_by_membership_id");
            add!("memberships", "reviewer_membership_id");
            add_related(row, &mut references, warnings);
        }
        "approvals" => {
            add!("guest_research_briefs", "research_brief_id");
            add!("outreach_draft_versions", "outreach_draft_version_id");
            add!("content_versions", "content_version_id");
            add!("agent_runs", "requested_by_agent_run_id");
            add!("memberships", "assigned_reviewer_membership_id");
            add!("memberships", "decided_by_membership_id");
        }
        "approval_events" => {
            add!("approvals", "approval_id");
            add!("memberships", "actor_membership_id");
        }
        "attention_items" => {
            add!("memberships", "assigned_to_membership_id");
            add_related(row, &mut references, warnings);
        }
        "comments" => {
            add!("attention_items", "attention_item_id");
            add!("memberships", "author_membership_id");
        }
        "tasks" => {
            add!("memberships", "owner_membership_id");
            add_related(row, &mut references, warnings);
        }
        "notifications" => add!("memberships", "membership_id"),
        "activity_events" => {
            add!("memberships", "actor_membership_id");
            add_related(row, &mut references, warnings);
        }
        "audit_events" => {
            add!("memberships", "actor_membership_id");
            add_target(row, &mut references, warnings);
        }
        "workspaces" | "memberships" | "organizations" => {}
        other => {
            return Err(MigrationError::InvalidSource(format!(
                "dependency mapping missing for {other}"
            )));
        }
    }
    let mut result = Vec::new();
    for (table, key) in references {
        let coordinate = coordinates
            .get(&(table.to_string(), key.clone()))
            .cloned()
            .ok_or_else(|| {
                MigrationError::InvalidSource(format!(
                    "{}:{} references missing {table}:{key}",
                    row.table, row.key
                ))
            })?;
        result.push(coordinate);
    }
    result.sort();
    result.dedup();
    let current = coordinate(bundle, &row.table, &row.key, source_revision(row));
    if result.contains(&current) {
        return Err(MigrationError::InvalidSource(format!(
            "{}:{} depends on itself",
            row.table, row.key
        )));
    }
    Ok(result)
}

fn add_reference(
    row: &SourceRow,
    references: &mut Vec<(&'static str, String)>,
    table: &'static str,
    field: &str,
) {
    if let Some(value) = optional_string(row, field) {
        references.push((table, value.to_string()));
    }
}

fn add_related(
    row: &SourceRow,
    references: &mut Vec<(&'static str, String)>,
    warnings: &mut Vec<String>,
) {
    let Some(kind) = optional_string(row, "related_type") else {
        return;
    };
    let Some(id) = optional_string(row, "related_id") else {
        return;
    };
    match source_table_for_type(kind) {
        Some(table) => references.push((table, id.to_string())),
        None => warnings.push(format!(
            "unresolved_related_type:{}:{}:{kind}",
            row.table, row.key
        )),
    }
}

fn add_target(
    row: &SourceRow,
    references: &mut Vec<(&'static str, String)>,
    warnings: &mut Vec<String>,
) {
    let Some(kind) = optional_string(row, "target_type") else {
        return;
    };
    let Some(id) = optional_string(row, "target_id") else {
        return;
    };
    match source_table_for_type(kind) {
        Some(table) => references.push((table, id.to_string())),
        None => warnings.push(format!(
            "unresolved_target_type:{}:{}:{kind}",
            row.table, row.key
        )),
    }
}

fn source_table_for_type(kind: &str) -> Option<&'static str> {
    match kind {
        "workspace" => Some("workspaces"),
        "organization" => Some("organizations"),
        "guest" | "person" => Some("guests"),
        "interview" => Some("interviews"),
        "content" => Some("content_items"),
        "task" => Some("tasks"),
        "agent" => Some("agent_runs"),
        "approval" => Some("approvals"),
        "attention" => Some("attention_items"),
        _ => None,
    }
}

fn resolve_media_source(
    coordinates: &BTreeMap<(String, String), SourceCoordinate>,
    source: &str,
) -> Option<SourceCoordinate> {
    let (kind, id) = source.split_once(':')?;
    let table = source_table_for_type(kind)?;
    coordinates
        .get(&(table.to_string(), id.to_string()))
        .cloned()
}

fn normalized_status(bundle: &LoadedBundle, row: &SourceRow) -> Result<Option<StatusMapping>> {
    if matches!(
        row.table.as_str(),
        "organizations"
            | "guest_topics"
            | "guest_tags"
            | "guest_notes"
            | "interview_members"
            | "interview_questions"
            | "content_platforms"
    ) {
        return Ok(Some(StatusMapping {
            status: "verified".to_string(),
            source_status: "legacy_import".to_string(),
        }));
    }
    if row.table == "outreach_draft_versions" {
        return Ok(Some(StatusMapping {
            status: "draft".to_string(),
            source_status: "legacy_draft".to_string(),
        }));
    }
    if row.table == "content_versions" {
        let content_id = required_string(row, "content_item_id")?;
        let parent = bundle.row("content_items", content_id).ok_or_else(|| {
            MigrationError::InvalidSource(format!(
                "content version {} references missing content item {content_id}",
                row.key
            ))
        })?;
        let source = required_string(parent, "status")?;
        return content_status(source).map(Some);
    }
    let Some(source) = optional_string(
        row,
        match row.table.as_str() {
            "approvals" => "state",
            _ => "status",
        },
    ) else {
        return Ok(None);
    };
    let mut mapped = match row.table.as_str() {
        "guests" => guest_status(source)?,
        "interviews" => interview_status(source)?,
        "content_items" => content_status(source)?,
        "tasks" => task_status(source)?,
        "approvals" => approval_status(source)?,
        _ => return Ok(None),
    };
    if row.table == "guests" && is_do_not_contact(row) {
        mapped.status = "archived".to_string();
    }
    Ok(Some(mapped))
}

fn authorship(mapping: &MigrationMapping, row: &SourceRow) -> Result<PlannedAuthorship> {
    const ACTOR_FIELDS: &[&str] = &[
        "author_membership_id",
        "actor_membership_id",
        "edited_by_membership_id",
        "created_by_membership_id",
        "decided_by_membership_id",
        "triggered_by_membership_id",
        "claimed_by_membership_id",
    ];
    let original_membership_id = ACTOR_FIELDS
        .iter()
        .find_map(|field| optional_string(row, field))
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| {
            MigrationError::InvalidSource(format!(
                "{}:{} has an invalid actor membership",
                row.table, row.key
            ))
        })?;
    let original_pubkey = original_membership_id
        .and_then(|id| mapping.memberships.get(&id))
        .map(|member| member.pubkey.clone());
    let original_actor_kind = if original_membership_id.is_some() {
        "human".to_string()
    } else {
        optional_string(row, "actor_kind")
            .unwrap_or(match row.table.as_str() {
                "agent_runs"
                | "guest_research_briefs"
                | "guest_research_themes"
                | "guest_research_facts"
                | "clip_candidates"
                | "content_qc_findings" => "agent",
                _ => "unknown",
            })
            .to_string()
    };
    Ok(PlannedAuthorship {
        signer: "migration_service".to_string(),
        original_membership_id,
        original_pubkey,
        original_actor_kind,
    })
}

fn migration_content(
    bundle: &LoadedBundle,
    mapping: &MigrationMapping,
    row: &SourceRow,
    record_id: Uuid,
    status: Option<&StatusMapping>,
    authorship: &PlannedAuthorship,
    override_reason: Option<&str>,
) -> Value {
    let dnc_active = row.table == "guests" && is_do_not_contact(row);
    json!({
        "schema_version": MK_SCHEMA_VERSION,
        "record_id": record_id,
        "community": mapping.community,
        "version": event_version(row),
        "status": status.map(|value| value.status.as_str()),
        "source_status": status.map(|value| value.source_status.as_str()),
        "do_not_contact": dnc_active,
        "legacy": event_safe_legacy(row),
        "migration": {
            "source_system": bundle.manifest.source_system,
            "source_workspace_id": bundle.manifest.source_workspace_id,
            "dataset_sha256": bundle.manifest.dataset_sha256,
            "source_table": row.table,
            "source_id": row.key,
            "source_sha256": row.sha256,
            "source_created_at": source_timestamp(row),
            "original_membership_id": authorship.original_membership_id,
            "original_pubkey": authorship.original_pubkey,
            "original_actor_kind": authorship.original_actor_kind,
            "override_reason": override_reason
        }
    })
}

fn base_tags(
    mapping: &MigrationMapping,
    source: &SourceCoordinate,
    source_sha256: &str,
    version: u64,
    status: Option<&StatusMapping>,
    class: PlannedRecordClass,
) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["schema_version".to_string(), MK_SCHEMA_VERSION.to_string()],
        vec!["source".to_string(), source.tag_value()],
        vec!["source_sha256".to_string(), source_sha256.to_string()],
        vec!["version".to_string(), version.to_string()],
    ];
    if class == PlannedRecordClass::TeamHistory {
        if let Some(channel) = mapping.team_history_channel {
            tags.push(vec!["h".to_string(), channel.to_string()]);
        }
        tags.push(vec!["community".to_string(), mapping.community.clone()]);
    } else {
        tags.push(vec!["h".to_string(), mapping.community.clone()]);
    }
    if let Some(status) = status {
        tags.push(vec!["status".to_string(), status.status.clone()]);
        tags.push(vec![
            "source_status".to_string(),
            status.source_status.clone(),
        ]);
    }
    tags
}

fn apply_override(
    mapping: &MigrationMapping,
    row: &SourceRow,
    kind: u32,
    default_d: Option<Uuid>,
) -> Result<(PlannedAction, Option<PlannedDestination>, Option<String>)> {
    let override_match = mapping.entity_overrides.iter().find(|item| {
        (item.source_type == row.table || item.source_type == semantic_type(&row.table))
            && item.source_id == row.key
    });
    let Some(item) = override_match else {
        return Ok((
            PlannedAction::Create,
            Some(PlannedDestination {
                kind,
                d_tag: default_d,
            }),
            None,
        ));
    };
    match item.strategy {
        OverrideStrategy::Create => Ok((
            PlannedAction::Create,
            Some(PlannedDestination {
                kind,
                d_tag: default_d,
            }),
            None,
        )),
        OverrideStrategy::Link => Ok((
            PlannedAction::Link,
            Some(PlannedDestination {
                kind,
                d_tag: item.destination_d,
            }),
            item.reason.clone(),
        )),
        OverrideStrategy::Skip => Ok((PlannedAction::Skip, None, item.reason.clone())),
    }
}

fn semantic_type(table: &str) -> &str {
    match table {
        "guests" => "person",
        "interviews" => "interview",
        "content_items" | "content_versions" => "content",
        "tasks" => "task",
        "approvals" | "approval_events" => "approval",
        other => other,
    }
}

fn planned_unit(record: &PlannedRecord) -> PlannedUnit {
    PlannedUnit {
        source: record.source.clone(),
        source_sha256: record.source_sha256.clone(),
        action: record.action,
        destination: record.destination.clone(),
        reason: match record.action {
            PlannedAction::Link | PlannedAction::Skip => record
                .content
                .pointer("/migration/override_reason")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    (record.class == PlannedRecordClass::MediaReceipt
                        && record.action == PlannedAction::Skip)
                        .then(|| "attachment_not_ready".to_string())
                }),
            PlannedAction::Create => None,
        },
        dependencies: record.dependencies.clone(),
    }
}

fn source_timestamp(row: &SourceRow) -> Option<String> {
    [
        "created_at",
        "occurred_at",
        "generated_at",
        "started_at",
        "updated_at",
    ]
    .iter()
    .find_map(|field| optional_string(row, field))
    .map(str::to_string)
}

fn source_revision(row: &SourceRow) -> u64 {
    row.value
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or(1)
}

fn is_do_not_contact(row: &SourceRow) -> bool {
    row.value
        .get("do_not_contact")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || optional_string(row, "status") == Some("do_not_contact")
}

fn event_version(row: &SourceRow) -> u64 {
    if row.table == "content_items" {
        return row
            .value
            .get("current_version")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| source_revision(row))
            .saturating_add(1);
    }
    source_revision(row)
}

fn required_string<'a>(row: &'a SourceRow, field: &str) -> Result<&'a str> {
    optional_string(row, field).ok_or_else(|| {
        MigrationError::InvalidSource(format!(
            "{}:{} is missing required field {field}",
            row.table, row.key
        ))
    })
}

fn optional_string<'a>(row: &'a SourceRow, field: &str) -> Option<&'a str> {
    row.value.get(field).and_then(Value::as_str)
}
