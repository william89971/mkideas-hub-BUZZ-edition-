//! Serializable models and checkpoint validation for offline dry runs.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::bundle::MediaIntegrity;
use crate::canonical::canonical_sha256;
use crate::plan::{MigrationPlan, PlannedAction, PlannedDestination};
use crate::receipt::SourceCoordinate;
use crate::report::MigrationReport;
use crate::{MigrationError, Result};

pub(crate) const DRY_RUN_SCHEMA_VERSION: u32 = 1;

/// Migration record category used by the future relay/media adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedRecordClass {
    /// Addressable MK state.
    State,
    /// Immutable MK operation or system event.
    Operation,
    /// Service-authored historical Team message.
    TeamHistory,
    /// Media descriptor and receipt; bytes remain local during dry-run.
    MediaReceipt,
}

/// Historical authorship retained while the migration service remains signer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedAuthorship {
    /// Actual future signer class. No private key is read by dry-run.
    pub signer: String,
    /// Original legacy membership, when the source recorded one.
    pub original_membership_id: Option<Uuid>,
    /// Existing Buzz pubkey selected by the operator mapping.
    pub original_pubkey: Option<String>,
    /// Original human/system/agent actor classification.
    pub original_actor_kind: String,
}

/// One complete deterministic event-shaped record ready for a future signer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedRecord {
    /// Deterministic internal record identity.
    pub record_id: Uuid,
    /// State, operation, Team history, or media receipt.
    pub class: PlannedRecordClass,
    /// Exact source unit.
    pub source: SourceCoordinate,
    /// Canonical source hash.
    pub source_sha256: String,
    /// Create, link, or explicit skip.
    pub action: PlannedAction,
    /// Event kind and addressable identity, when applicable.
    pub destination: Option<PlannedDestination>,
    /// Source timestamp retained exactly when one existed.
    pub source_created_at: Option<String>,
    /// Source entity version, defaulting to one only when the source had none.
    pub version: u64,
    /// Historical author attribution.
    pub authorship: PlannedAuthorship,
    /// Deterministic dependency coordinates.
    pub dependencies: Vec<SourceCoordinate>,
    /// Event tags to be supplied unchanged to the future signing adapter.
    pub tags: Vec<Vec<String>>,
    /// Versioned event content with source provenance and normalized fields.
    pub content: Value,
}

/// Media association and integrity outcome included in the dry-run report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMedia {
    /// Deterministic media migration identity.
    pub media_id: Uuid,
    /// Manifest source association.
    pub source: String,
    /// Related source unit when the association resolved.
    pub related: Option<SourceCoordinate>,
    /// Bundle-relative path.
    pub file: String,
    /// Declared immutable bytes hash.
    pub sha256: String,
    /// Declared MIME type.
    pub mime_type: String,
    /// Declared byte count.
    pub size: u64,
    /// Ready, missing, corrupt, or unreadable.
    pub integrity: MediaIntegrity,
}

/// Secret-free checkpoint for deterministic interruption and resume validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflinePlanningCheckpoint {
    /// Checkpoint schema.
    pub schema_version: u32,
    /// Immutable dataset.
    pub dataset_sha256: String,
    /// Immutable plan.
    pub plan_sha256: String,
    /// Deterministic migration batch.
    pub batch_id: Uuid,
    /// Next topologically ordered unit to process.
    pub next_unit_index: u64,
    /// Canonical hash of the already processed source/hash prefix.
    pub processed_prefix_sha256: String,
}

/// Complete deterministic offline result. It contains no signed events,
/// credentials, uploaded media IDs, or relay acceptance claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineDryRun {
    /// Dry-run result schema.
    pub schema_version: u32,
    /// Immutable dependency-ordered plan.
    pub plan: MigrationPlan,
    /// Event-shaped records in the same dependency order.
    pub records: Vec<PlannedRecord>,
    /// Attachment associations and integrity outcomes.
    pub media: Vec<PlannedMedia>,
    /// Complete-planning checkpoint.
    pub checkpoint: OfflinePlanningCheckpoint,
    /// Source/destination count and exception reconciliation.
    pub report: MigrationReport,
}

impl OfflineDryRun {
    /// Canonical result hash used to prove repeatability.
    pub fn sha256(&self) -> Result<String> {
        canonical_sha256(self)
    }

    /// Construct a checkpoint after any prefix of the ordered plan.
    pub fn checkpoint_at(&self, next_unit_index: usize) -> Result<OfflinePlanningCheckpoint> {
        if next_unit_index > self.plan.units.len() {
            return Err(MigrationError::IdentityConflict(format!(
                "checkpoint index {next_unit_index} exceeds {} plan units",
                self.plan.units.len()
            )));
        }
        let prefix = self.plan.units[..next_unit_index]
            .iter()
            .map(|unit| (&unit.source, unit.source_sha256.as_str()))
            .collect::<Vec<_>>();
        Ok(OfflinePlanningCheckpoint {
            schema_version: DRY_RUN_SCHEMA_VERSION,
            dataset_sha256: self.plan.dataset_sha256.clone(),
            plan_sha256: self.plan.sha256()?,
            batch_id: self.plan.batch_id,
            next_unit_index: u64::try_from(next_unit_index).unwrap_or(u64::MAX),
            processed_prefix_sha256: canonical_sha256(&prefix)?,
        })
    }

    /// Validate that an interruption checkpoint belongs to this exact plan.
    pub fn validate_checkpoint(&self, checkpoint: &OfflinePlanningCheckpoint) -> Result<()> {
        if checkpoint.schema_version != DRY_RUN_SCHEMA_VERSION {
            return Err(MigrationError::IdentityConflict(format!(
                "unsupported planning checkpoint schema {}",
                checkpoint.schema_version
            )));
        }
        if checkpoint.dataset_sha256 != self.plan.dataset_sha256
            || checkpoint.plan_sha256 != self.plan.sha256()?
            || checkpoint.batch_id != self.plan.batch_id
        {
            return Err(MigrationError::IdentityConflict(
                "planning checkpoint identity does not match the dry-run plan".to_string(),
            ));
        }
        let next = usize::try_from(checkpoint.next_unit_index).map_err(|_| {
            MigrationError::IdentityConflict("checkpoint index does not fit usize".to_string())
        })?;
        let expected = self.checkpoint_at(next)?;
        if expected.processed_prefix_sha256 != checkpoint.processed_prefix_sha256 {
            return Err(MigrationError::IdentityConflict(
                "planning checkpoint prefix hash does not match".to_string(),
            ));
        }
        Ok(())
    }
}
