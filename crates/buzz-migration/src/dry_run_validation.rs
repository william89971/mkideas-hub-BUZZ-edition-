//! Cross-row validations for deterministic offline planning.

use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::bundle::{LoadedBundle, SourceRow};
use crate::mapping::MigrationMapping;
use crate::{MigrationError, Result};

pub(crate) fn validate_source_versions(bundle: &LoadedBundle) -> Result<()> {
    let mut content_revisions = BTreeMap::<(String, u64), String>::new();
    for row in bundle.ordered_rows() {
        let revision = source_revision(row);
        if revision == 0 {
            return Err(MigrationError::InvalidSource(format!(
                "{}:{} has invalid zero version",
                row.table, row.key
            )));
        }
        if row.table == "content_items" {
            let current = row
                .value
                .get("current_version")
                .and_then(Value::as_u64)
                .unwrap_or(revision);
            if current == u64::MAX {
                return Err(MigrationError::InvalidSource(format!(
                    "content_items:{} current_version cannot be advanced",
                    row.key
                )));
            }
        }
        if row.table == "content_versions" {
            let parent = required_string(row, "content_item_id")?.to_string();
            if let Some(existing) =
                content_revisions.insert((parent.clone(), revision), row.key.clone())
            {
                return Err(MigrationError::DuplicateSource {
                    table: "content_versions".to_string(),
                    key: format!("{parent}@{revision} ({existing}, {})", row.key),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_team_history_target(
    bundle: &LoadedBundle,
    mapping: &MigrationMapping,
) -> Result<()> {
    let has_comments = bundle
        .tables
        .get("comments")
        .is_some_and(|comments| !comments.is_empty());
    if has_comments && mapping.team_history_channel.is_none() {
        return Err(MigrationError::IdentityConflict(
            "team_history_channel is required when the bundle contains comments".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_membership_references(
    bundle: &LoadedBundle,
    mapping: &MigrationMapping,
) -> Result<()> {
    for row in bundle.ordered_rows() {
        let Some(object) = row.value.as_object() else {
            return Err(MigrationError::InvalidSource(format!(
                "{}:{} is not an object",
                row.table, row.key
            )));
        };
        for (field, value) in object {
            let is_reference = field == "membership_id"
                || field.ends_with("_membership_id")
                || (row.table == "memberships" && field == "id");
            if !is_reference || value.is_null() {
                continue;
            }
            let raw = value.as_str().ok_or_else(|| {
                MigrationError::InvalidSource(format!(
                    "{}:{} field {field} is not a membership UUID",
                    row.table, row.key
                ))
            })?;
            let id = Uuid::parse_str(raw).map_err(|_| {
                MigrationError::InvalidSource(format!(
                    "{}:{} field {field} is not a membership UUID",
                    row.table, row.key
                ))
            })?;
            if !mapping.memberships.contains_key(&id) {
                return Err(MigrationError::IdentityConflict(format!(
                    "{}:{} references unmapped membership {id}",
                    row.table, row.key
                )));
            }
        }
    }
    Ok(())
}

fn source_revision(row: &SourceRow) -> u64 {
    row.value
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or(1)
}

fn required_string<'a>(row: &'a SourceRow, field: &str) -> Result<&'a str> {
    row.value.get(field).and_then(Value::as_str).ok_or_else(|| {
        MigrationError::InvalidSource(format!(
            "{}:{} is missing required field {field}",
            row.table, row.key
        ))
    })
}
