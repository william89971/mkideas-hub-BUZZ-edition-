//! Deterministic migration-unit dependency planning.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::canonical::canonical_sha256;
use crate::receipt::SourceCoordinate;
use crate::{MigrationError, Result};

/// How a source unit will be represented at the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedAction {
    /// Create a new deterministic destination event/entity.
    Create,
    /// Link provenance to an existing destination entity selected by an operator.
    Link,
    /// Explicitly exclude the unit. A reason is mandatory.
    Skip,
}

/// Destination identity planned for a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedDestination {
    /// Nostr event kind.
    pub kind: u32,
    /// Stable addressable identity, when the event kind uses one.
    pub d_tag: Option<Uuid>,
}

/// One independently idempotent source aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedUnit {
    /// Immutable source coordinate.
    pub source: SourceCoordinate,
    /// Hash of the canonical aggregate input.
    pub source_sha256: String,
    /// Create, link, or explicit skip.
    pub action: PlannedAction,
    /// Destination identity for create/link.
    pub destination: Option<PlannedDestination>,
    /// Explanation required for link/skip decisions.
    pub reason: Option<String>,
    /// Units that must complete before this one is submitted.
    #[serde(default)]
    pub dependencies: Vec<SourceCoordinate>,
}

/// Immutable plan produced from a bundle and operator mapping decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationPlan {
    /// Plan schema version.
    pub schema_version: u32,
    /// Source dataset hash.
    pub dataset_sha256: String,
    /// Target community host.
    pub community: String,
    /// Deterministic batch ID.
    pub batch_id: Uuid,
    /// Unordered input units. [`ordered_units`](Self::ordered_units) produces
    /// the canonical apply order.
    pub units: Vec<PlannedUnit>,
}

impl MigrationPlan {
    /// Canonical hash of the immutable plan.
    pub fn sha256(&self) -> Result<String> {
        let mut normalized = self.clone();
        normalized
            .units
            .sort_by(|left, right| left.source.cmp(&right.source));
        for unit in &mut normalized.units {
            unit.dependencies.sort();
        }
        canonical_sha256(&normalized)
    }

    /// Validate operator decisions and return a deterministic topological order.
    pub fn ordered_units(&self) -> Result<Vec<&PlannedUnit>> {
        let mut units = BTreeMap::new();
        for unit in &self.units {
            validate_action(unit)?;
            if units.insert(unit.source.clone(), unit).is_some() {
                return Err(MigrationError::DuplicateUnit(unit.source.to_string()));
            }
        }

        let mut in_degree = units
            .keys()
            .cloned()
            .map(|coordinate| (coordinate, 0_usize))
            .collect::<BTreeMap<_, _>>();
        let mut children = BTreeMap::<SourceCoordinate, BTreeSet<SourceCoordinate>>::new();

        for unit in units.values() {
            let unique_dependencies = unit.dependencies.iter().cloned().collect::<BTreeSet<_>>();
            for dependency in unique_dependencies {
                if !units.contains_key(&dependency) {
                    return Err(MigrationError::MissingDependency {
                        unit: unit.source.to_string(),
                        dependency: dependency.to_string(),
                    });
                }
                if let Some(degree) = in_degree.get_mut(&unit.source) {
                    *degree += 1;
                }
                children
                    .entry(dependency)
                    .or_default()
                    .insert(unit.source.clone());
            }
        }

        let mut ready = in_degree
            .iter()
            .filter_map(|(coordinate, degree)| (*degree == 0).then_some(coordinate.clone()))
            .collect::<BTreeSet<_>>();
        let mut ordered = Vec::with_capacity(units.len());

        while let Some(coordinate) = ready.pop_first() {
            if let Some(unit) = units.get(&coordinate) {
                ordered.push(*unit);
            }
            if let Some(next) = children.get(&coordinate) {
                for child in next {
                    if let Some(degree) = in_degree.get_mut(child) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            ready.insert(child.clone());
                        }
                    }
                }
            }
        }

        if ordered.len() != units.len() {
            let blocked = in_degree
                .into_iter()
                .filter_map(|(coordinate, degree)| (degree > 0).then_some(coordinate.to_string()))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(MigrationError::DependencyCycle(blocked));
        }
        Ok(ordered)
    }
}

fn validate_action(unit: &PlannedUnit) -> Result<()> {
    match unit.action {
        PlannedAction::Create if unit.destination.is_none() => Err(
            MigrationError::IdentityConflict(format!("{} create has no destination", unit.source)),
        ),
        PlannedAction::Link if unit.destination.is_none() => Err(MigrationError::IdentityConflict(
            format!("{} link has no destination", unit.source),
        )),
        PlannedAction::Link | PlannedAction::Skip
            if unit.reason.as_deref().is_none_or(str::is_empty) =>
        {
            Err(MigrationError::IdentityConflict(format!(
                "{} {:?} requires a reason",
                unit.source, unit.action
            )))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coordinate(kind: &str) -> SourceCoordinate {
        SourceCoordinate {
            system: "mkideas-command-center".to_string(),
            workspace_id: Uuid::nil(),
            unit_kind: kind.to_string(),
            unit_id: "one".to_string(),
            revision: 1,
        }
    }

    fn unit(kind: &str, dependencies: Vec<SourceCoordinate>) -> PlannedUnit {
        PlannedUnit {
            source: coordinate(kind),
            source_sha256: "a".repeat(64),
            action: PlannedAction::Create,
            destination: Some(PlannedDestination {
                kind: 30_803,
                d_tag: Some(Uuid::nil()),
            }),
            reason: None,
            dependencies,
        }
    }

    #[test]
    fn topological_order_is_stable() {
        let person = coordinate("person");
        let plan = MigrationPlan {
            schema_version: 1,
            dataset_sha256: "b".repeat(64),
            community: "hub.mkideas.org".to_string(),
            batch_id: Uuid::nil(),
            units: vec![
                unit("content", vec![coordinate("interview")]),
                unit("interview", vec![person.clone()]),
                unit("person", Vec::new()),
            ],
        };
        let order = plan
            .ordered_units()
            .expect("valid order")
            .into_iter()
            .map(|unit| unit.source.unit_kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["person", "interview", "content"]);
    }

    #[test]
    fn cycle_is_rejected() {
        let mut person = unit("person", vec![coordinate("interview")]);
        person.destination = Some(PlannedDestination {
            kind: 30_803,
            d_tag: Some(Uuid::nil()),
        });
        let plan = MigrationPlan {
            schema_version: 1,
            dataset_sha256: "b".repeat(64),
            community: "hub.mkideas.org".to_string(),
            batch_id: Uuid::nil(),
            units: vec![person, unit("interview", vec![coordinate("person")])],
        };
        assert!(matches!(
            plan.ordered_units(),
            Err(MigrationError::DependencyCycle(_))
        ));
    }
}
