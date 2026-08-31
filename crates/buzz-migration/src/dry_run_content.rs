//! Event-safe legacy payload shaping for offline plans.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::bundle::SourceRow;

pub(crate) fn event_safe_legacy(row: &SourceRow) -> Value {
    let mut legacy = row.value.clone();
    let Some(object) = legacy.as_object_mut() else {
        return legacy;
    };
    if row.table == "transcript_segments" {
        if let Some(text) = object.remove("text") {
            if let Some(text) = text.as_str() {
                object.insert(
                    "text_sha256".to_string(),
                    Value::String(hex::encode(Sha256::digest(text.as_bytes()))),
                );
            }
        }
        object.insert(
            "storage".to_string(),
            Value::String("private_transcript_media".to_string()),
        );
    }
    if row.table == "audit_events" {
        object.remove("before");
        object.remove("after");
        object.insert(
            "archive_entry_sha256".to_string(),
            Value::String(row.sha256.clone()),
        );
        object.insert(
            "storage".to_string(),
            Value::String("immutable_audit_archive".to_string()),
        );
    }
    legacy
}
