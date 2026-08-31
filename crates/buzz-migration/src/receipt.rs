//! Idempotency coordinates shared by plans, checkpoints, and relay receipts.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Immutable source identity for one migration unit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceCoordinate {
    /// Stable source-system identifier.
    pub system: String,
    /// Source workspace UUID.
    pub workspace_id: Uuid,
    /// Aggregated source unit kind, such as `person` or `content-version`.
    pub unit_kind: String,
    /// Source primary key or deterministic aggregate key.
    pub unit_id: String,
    /// Source revision. Use `1` for immutable/unversioned rows.
    pub revision: u64,
}

impl SourceCoordinate {
    /// Canonical tag value carried by every migration-authored event.
    pub fn tag_value(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.system,
            self.workspace_id.hyphenated(),
            self.unit_kind,
            self.unit_id,
            self.revision
        )
    }
}

impl fmt::Display for SourceCoordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.tag_value())
    }
}

/// Destination identity recorded by the relay's migration projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinationReceipt {
    /// Accepted event kind.
    pub kind: u32,
    /// Addressable `d` tag when applicable.
    pub d_tag: Option<Uuid>,
    /// Accepted signed event ID.
    pub event_id: String,
}

/// Durable per-unit receipt used for idempotent replay and reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationItemReceipt {
    /// Source unit coordinate.
    pub source: SourceCoordinate,
    /// Hash of the canonical source unit.
    pub source_sha256: String,
    /// Destination event identity.
    pub destination: DestinationReceipt,
    /// Deterministic dataset batch.
    pub batch_id: Uuid,
    /// Relay acceptance time in RFC 3339 form.
    pub accepted_at: String,
}
