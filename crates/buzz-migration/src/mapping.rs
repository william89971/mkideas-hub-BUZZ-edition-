//! Explicit human identity and entity-collision decisions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{MigrationError, Result};

/// Operational role assigned to a mapped legacy membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappedRole {
    /// Relay/community owner.
    Owner,
    /// Operational administrator.
    Admin,
}

/// One old membership mapped to an existing Buzz identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberMapping {
    /// Lowercase 32-byte Nostr public key in hex.
    pub pubkey: String,
    /// Owner or admin role.
    pub role: MappedRole,
    /// Human-readable provenance label.
    pub display_name: String,
}

/// Operator-selected collision strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideStrategy {
    /// Create the normal deterministic destination.
    Create,
    /// Link to an existing destination without fuzzy auto-merge.
    Link,
    /// Explicitly exclude the source entity.
    Skip,
}

/// Explicit resolution for a duplicate candidate or exclusion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityOverride {
    /// Legacy aggregate type.
    pub source_type: String,
    /// Legacy primary key.
    pub source_id: String,
    /// Operator decision.
    pub strategy: OverrideStrategy,
    /// Existing destination for `link`.
    pub destination_d: Option<Uuid>,
    /// Required rationale for `link` and `skip`.
    pub reason: Option<String>,
}

/// Complete non-secret operator mapping input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationMapping {
    /// Legacy workspace to import.
    pub workspace_id: Uuid,
    /// Target Buzz community host.
    pub community: String,
    /// Legacy membership UUID to existing Buzz identity.
    pub memberships: BTreeMap<Uuid, MemberMapping>,
    /// Explicit entity collision/exclusion decisions.
    #[serde(default)]
    pub entity_overrides: Vec<EntityOverride>,
    /// Existing Team channel used for imported historical comments.
    pub team_history_channel: Option<Uuid>,
}

impl MigrationMapping {
    /// Validate identity and collision decisions without contacting either system.
    pub fn validate(&self) -> Result<()> {
        if self.community.trim().is_empty() {
            return Err(MigrationError::IdentityConflict(
                "target community is empty".to_string(),
            ));
        }
        let owner_count = self
            .memberships
            .values()
            .filter(|member| member.role == MappedRole::Owner)
            .count();
        if owner_count != 1 {
            return Err(MigrationError::IdentityConflict(format!(
                "expected exactly one owner mapping, found {owner_count}"
            )));
        }
        for (membership_id, member) in &self.memberships {
            if member.display_name.trim().is_empty() {
                return Err(MigrationError::IdentityConflict(format!(
                    "membership {membership_id} has an empty display name"
                )));
            }
            if member.pubkey.len() != 64
                || !member.pubkey.bytes().all(|byte| byte.is_ascii_hexdigit())
                || member.pubkey != member.pubkey.to_ascii_lowercase()
            {
                return Err(MigrationError::IdentityConflict(format!(
                    "membership {membership_id} has an invalid Nostr pubkey"
                )));
            }
        }
        for resolution in &self.entity_overrides {
            let has_reason = resolution
                .reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty());
            match resolution.strategy {
                OverrideStrategy::Create if resolution.destination_d.is_some() => {
                    return Err(MigrationError::IdentityConflict(format!(
                        "create override {}:{} must not set destination_d",
                        resolution.source_type, resolution.source_id
                    )));
                }
                OverrideStrategy::Link if resolution.destination_d.is_none() || !has_reason => {
                    return Err(MigrationError::IdentityConflict(format!(
                        "link override {}:{} requires destination_d and reason",
                        resolution.source_type, resolution.source_id
                    )));
                }
                OverrideStrategy::Skip if resolution.destination_d.is_some() || !has_reason => {
                    return Err(MigrationError::IdentityConflict(format!(
                        "skip override {}:{} requires a reason and no destination_d",
                        resolution.source_type, resolution.source_id
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_requires_exactly_one_owner() {
        let mapping = MigrationMapping {
            workspace_id: Uuid::nil(),
            community: "hub.mkideas.org".to_string(),
            memberships: BTreeMap::new(),
            entity_overrides: Vec::new(),
            team_history_channel: None,
        };
        assert!(matches!(
            mapping.validate(),
            Err(MigrationError::IdentityConflict(_))
        ));
    }
}
