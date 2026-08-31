//! Stable identifiers for one-time Command Center imports.

use uuid::Uuid;

/// Fixed namespace for legacy MK Ideas Command Center identities.
///
/// Changing this value would duplicate every imported entity, so it is part of
/// the persisted migration contract.
pub const MKCC_NAMESPACE: Uuid = Uuid::from_u128(0x4d4b_4944_4541_5342_555a_5a4d_4947_0001);

/// Derive a destination `d` tag for one source entity.
pub fn destination_id(workspace_id: Uuid, destination_type: &str, source_key: &str) -> Uuid {
    let name = format!(
        "mkcc/{}/{}/{}",
        workspace_id.hyphenated(),
        normalize_component(destination_type),
        normalize_component(source_key)
    );
    Uuid::new_v5(&MKCC_NAMESPACE, name.as_bytes())
}

/// Derive a repeatable batch ID from a source workspace and dataset hash.
pub fn batch_id(workspace_id: Uuid, dataset_sha256: &str) -> Uuid {
    let name = format!(
        "mkcc/{}/batch/{}",
        workspace_id.hyphenated(),
        dataset_sha256.trim().to_ascii_lowercase()
    );
    Uuid::new_v5(&MKCC_NAMESPACE, name.as_bytes())
}

fn normalize_component(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE: Uuid = Uuid::from_u128(0x1111_1111_1111_4111_8111_1111_1111_1111);

    #[test]
    fn destination_ids_are_stable_and_type_scoped() {
        let first = destination_id(WORKSPACE, "Person", "ABC-123");
        let second = destination_id(WORKSPACE, " person ", "abc-123");
        let interview = destination_id(WORKSPACE, "interview", "abc-123");
        assert_eq!(first, second);
        assert_ne!(first, interview);
        assert_eq!(first.get_version_num(), 5);
    }

    #[test]
    fn batch_id_normalizes_hash_case() {
        assert_eq!(batch_id(WORKSPACE, "AABBCC"), batch_id(WORKSPACE, "aabbcc"));
    }
}
