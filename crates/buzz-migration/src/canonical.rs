//! Canonical JSON and hashing helpers.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{MigrationError, Result};

/// Serialize a value with recursively lexicographically ordered object keys.
///
/// Migration contracts intentionally avoid non-integer JSON numbers. Sorting
/// at every object boundary keeps hashes stable even if `serde_json` is later
/// compiled with its insertion-order map feature.
pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)
        .map_err(|error| MigrationError::Serialization(error.to_string()))?;
    let normalized = canonicalize_value(value);
    serde_json::to_vec(&normalized)
        .map_err(|error| MigrationError::Serialization(error.to_string()))
}

/// Return the lowercase SHA-256 of a canonical JSON value.
pub fn canonical_sha256<T: Serialize>(value: &T) -> Result<String> {
    let bytes = canonical_json_bytes(value)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .into_iter()
                .map(|(key, value)| (key, canonicalize_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_value).collect()),
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn canonical_json_sorts_nested_keys() {
        let value = json!({"z": 1, "a": {"z": 2, "a": 3}});
        let bytes = canonical_json_bytes(&value).expect("canonical JSON");
        assert_eq!(
            String::from_utf8(bytes).expect("UTF-8"),
            r#"{"a":{"a":3,"z":2},"z":1}"#
        );
    }

    #[test]
    fn hash_is_independent_of_input_key_order() {
        let left = json!({"b": 2, "a": 1});
        let right = json!({"a": 1, "b": 2});
        assert_eq!(
            canonical_sha256(&left).expect("left hash"),
            canonical_sha256(&right).expect("right hash")
        );
    }
}
