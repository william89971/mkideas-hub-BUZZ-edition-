//! Machine-readable reconciliation summaries.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable counters emitted by apply, verify, and reconcile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeCounts {
    /// Planned units.
    pub planned: u64,
    /// Newly accepted units.
    pub created: u64,
    /// Identical units already present.
    pub already_present: u64,
    /// Explicit links to pre-existing entities.
    pub linked: u64,
    /// Explicit exclusions.
    pub skipped: u64,
    /// Warning-bearing units.
    pub warned: u64,
    /// Failed units.
    pub failed: u64,
}

/// Complete local report. It contains identifiers and hashes, never source or
/// destination credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    /// Report schema version.
    pub schema_version: u32,
    /// Deterministic migration batch.
    pub batch_id: Uuid,
    /// Source dataset hash.
    pub dataset_sha256: String,
    /// Target community.
    pub community: String,
    /// Source table row counts.
    pub source_rows: BTreeMap<String, u64>,
    /// Destination event counts by kind.
    pub destination_events: BTreeMap<u32, u64>,
    /// Aggregate result counters.
    pub outcomes: OutcomeCounts,
    /// Sorted warning codes/messages.
    pub warnings: Vec<String>,
    /// Sorted errors.
    pub errors: Vec<String>,
    /// Source fields deliberately excluded.
    pub exclusions: Vec<String>,
}

impl MigrationReport {
    /// Whether reconciliation is complete enough for a final receipt/cutover.
    pub fn is_clean(&self) -> bool {
        self.outcomes.failed == 0
            && self.errors.is_empty()
            && self.outcomes.planned
                == self.outcomes.created
                    + self.outcomes.already_present
                    + self.outcomes.linked
                    + self.outcomes.skipped
    }

    /// Sort free-form collections before canonical serialization.
    pub fn normalize(&mut self) {
        self.warnings.sort();
        self.warnings.dedup();
        self.errors.sort();
        self.errors.dedup();
        self.exclusions.sort();
        self.exclusions.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_requires_every_planned_unit_to_be_accounted_for() {
        let report = MigrationReport {
            schema_version: 1,
            batch_id: Uuid::nil(),
            dataset_sha256: "a".repeat(64),
            community: "hub.mkideas.org".to_string(),
            source_rows: BTreeMap::new(),
            destination_events: BTreeMap::new(),
            outcomes: OutcomeCounts {
                planned: 3,
                created: 2,
                already_present: 1,
                ..OutcomeCounts::default()
            },
            warnings: Vec::new(),
            errors: Vec::new(),
            exclusions: Vec::new(),
        };
        assert!(report.is_clean());
    }
}
