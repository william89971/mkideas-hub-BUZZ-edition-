//! Local, secret-free resume checkpoint models.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::receipt::{DestinationReceipt, SourceCoordinate};
use crate::{MigrationError, Result};

/// State of one immutable planned unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitCheckpointStatus {
    /// No destination operation has been attempted.
    Pending,
    /// Destination confirmed the exact event/unit.
    Accepted,
    /// Destination already held the same source coordinate and hash.
    AlreadyPresent,
    /// An operator explicitly linked this source unit to existing state.
    Linked,
    /// An operator explicitly excluded this source unit with a reason.
    Skipped,
    /// A definitive failure occurred.
    Failed,
    /// Delivery outcome is unknown and must be resolved by event/source query.
    DeliveryUnknown,
}

/// Resume information for one plan unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitCheckpoint {
    /// Immutable source content hash.
    pub source_sha256: String,
    /// Current application state.
    pub status: UnitCheckpointStatus,
    /// Accepted or linked destination, when known.
    pub destination: Option<DestinationReceipt>,
    /// Non-secret diagnostic or explicit skip reason.
    pub detail: Option<String>,
}

/// Whole-run checkpoint. It intentionally stores only key fingerprints, never
/// private keys, database URLs, or authentication tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationCheckpoint {
    /// Checkpoint schema version.
    pub schema_version: u32,
    /// Hash of the immutable export dataset.
    pub dataset_sha256: String,
    /// Hash of the immutable migration plan.
    pub plan_sha256: String,
    /// Target community host.
    pub community: String,
    /// Public key of the migration signer.
    pub migration_pubkey: String,
    /// Deterministic batch ID.
    pub batch_id: Uuid,
    /// Fixed batch size used to construct chunk receipts.
    pub batch_size: u32,
    /// Unit progress in stable source-coordinate order.
    pub units: BTreeMap<SourceCoordinate, UnitCheckpoint>,
}

impl MigrationCheckpoint {
    /// Ensure a resume invocation has exactly the same immutable identity as
    /// the checkpoint. An operator must create a new plan for any difference.
    pub fn validate_resume_identity(
        &self,
        dataset_sha256: &str,
        plan_sha256: &str,
        community: &str,
        migration_pubkey: &str,
        batch_size: u32,
    ) -> Result<()> {
        let checks = [
            ("dataset", self.dataset_sha256.as_str(), dataset_sha256),
            ("plan", self.plan_sha256.as_str(), plan_sha256),
            ("community", self.community.as_str(), community),
            (
                "migration pubkey",
                self.migration_pubkey.as_str(),
                migration_pubkey,
            ),
        ];
        for (label, existing, incoming) in checks {
            if existing != incoming {
                return Err(MigrationError::IdentityConflict(format!(
                    "{label} changed from {existing} to {incoming}"
                )));
            }
        }
        if self.batch_size != batch_size {
            return Err(MigrationError::IdentityConflict(format!(
                "batch size changed from {} to {batch_size}",
                self.batch_size
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint() -> MigrationCheckpoint {
        MigrationCheckpoint {
            schema_version: 1,
            dataset_sha256: "a".repeat(64),
            plan_sha256: "b".repeat(64),
            community: "hub.mkideas.org".to_string(),
            migration_pubkey: "c".repeat(64),
            batch_id: Uuid::nil(),
            batch_size: 100,
            units: BTreeMap::new(),
        }
    }

    #[test]
    fn resume_rejects_changed_identity() {
        let checkpoint = checkpoint();
        let result = checkpoint.validate_resume_identity(
            &"d".repeat(64),
            &"b".repeat(64),
            "hub.mkideas.org",
            &"c".repeat(64),
            100,
        );
        assert!(matches!(result, Err(MigrationError::IdentityConflict(_))));
    }
}
