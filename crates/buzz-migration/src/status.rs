//! Explicit legacy-to-NIP-MK status conversion.

use serde::{Deserialize, Serialize};

use crate::{MigrationError, Result};

/// A converted status plus optional legacy disposition that would otherwise be
/// lost when several old states converge on one NIP-MK state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusMapping {
    /// NIP-MK v2 status.
    pub status: String,
    /// Original status, retained whenever the conversion is not one-to-one.
    pub source_status: String,
}

/// Convert a legacy guest status.
pub fn guest_status(source: &str) -> Result<StatusMapping> {
    let target = match source {
        "potential" => "prospect",
        "researching" => "researching",
        "research_ready" | "outreach_drafted" | "awaiting_approval" => "ready-to-contact",
        "contacted" | "follow_up_due" => "contacted",
        "interested" => "responded",
        "scheduling" | "scheduled" => "scheduled",
        "interviewed" | "content_processing" => "interviewed",
        "relationship_nurture" | "paused" => "nurture",
        "declined" | "no_response" | "not_a_fit" => "closed",
        "do_not_contact" => "archived",
        other => return unsupported("guest", other),
    };
    Ok(mapping(target, source))
}

/// Convert a legacy interview status.
pub fn interview_status(source: &str) -> Result<StatusMapping> {
    let target = match source {
        "planning" | "research" | "questions_ready" | "awaiting_review" => "planning",
        "scheduled" => "scheduled",
        "recorded" => "recorded",
        "transcript_processing" => "transcribing",
        "content_processing" | "review" => "reviewing",
        "complete" => "complete",
        other => return unsupported("interview", other),
    };
    Ok(mapping(target, source))
}

/// Convert a legacy content status.
pub fn content_status(source: &str) -> Result<StatusMapping> {
    let target = match source {
        "idea" => "idea",
        "research" | "planned" | "recording" | "editing" => "draft",
        "internal_review" | "changes_requested" | "performance_review" => "in-review",
        "approved" => "approved",
        "scheduled" => "scheduled",
        "published" => "published",
        "archived" => "archived",
        other => return unsupported("content", other),
    };
    Ok(mapping(target, source))
}

/// Convert a legacy task status.
pub fn task_status(source: &str) -> Result<StatusMapping> {
    let target = match source {
        "open" => "to-do",
        "in_progress" => "in-progress",
        "blocked" | "waiting" => "blocked",
        "complete" => "done",
        "cancelled" => "cancelled",
        other => return unsupported("task", other),
    };
    Ok(mapping(target, source))
}

/// Convert a legacy approval state. The caller still constructs a pending
/// `30809` request followed by a `48200` action for approved/rejected results.
pub fn approval_status(source: &str) -> Result<StatusMapping> {
    let target = match source {
        "pending" => "pending",
        "approved" => "approved",
        "rejected" | "changes_requested" => "rejected",
        "not_requested" => "cancelled",
        other => return unsupported("approval", other),
    };
    Ok(mapping(target, source))
}

fn mapping(target: &str, source: &str) -> StatusMapping {
    StatusMapping {
        status: target.to_string(),
        source_status: source.to_string(),
    }
}

fn unsupported<T>(domain: &'static str, status: &str) -> Result<T> {
    Err(MigrationError::UnsupportedStatus {
        domain,
        status: status.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnc_remains_explicit_after_archival_mapping() {
        let result = guest_status("do_not_contact").expect("known status");
        assert_eq!(result.status, "archived");
        assert_eq!(result.source_status, "do_not_contact");
    }

    #[test]
    fn every_legacy_domain_rejects_unknown_values() {
        for result in [
            guest_status("future").map(|_| ()),
            interview_status("future").map(|_| ()),
            content_status("future").map(|_| ()),
            task_status("future").map(|_| ()),
            approval_status("future").map(|_| ()),
        ] {
            assert!(matches!(
                result,
                Err(MigrationError::UnsupportedStatus { .. })
            ));
        }
    }
}
