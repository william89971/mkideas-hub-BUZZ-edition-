//! Deny-list protection for source exports, plans, reports, and checkpoints.

use serde_json::Value;

use crate::{MigrationError, Result};

const EXACT_FORBIDDEN_KEYS: &[&str] = &[
    "access_token",
    "auth_user_id",
    "client_secret",
    "database_url",
    "invitation_token",
    "oauth_token",
    "password",
    "private_key",
    "refresh_token",
    "secret_key",
    "secret",
    "session",
    "session_id",
    "session_token",
    "token",
    "token_hash",
];

/// Find forbidden field paths in arbitrary source JSON.
pub fn forbidden_field_paths(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    visit(value, "$", &mut found);
    found.sort();
    found.dedup();
    found
}

/// Reject a value containing any forbidden source field.
pub fn validate_no_secrets(value: &Value) -> Result<()> {
    let fields = forbidden_field_paths(value);
    if fields.is_empty() {
        Ok(())
    } else {
        Err(MigrationError::ForbiddenFields(fields.join(", ")))
    }
}

fn visit(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let normalized = normalize_key(key);
                let child_path = format!("{path}.{key}");
                if is_forbidden_key(&normalized) {
                    found.push(child_path.clone());
                }
                visit(child, &child_path, found);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                visit(item, &format!("{path}[{index}]"), found);
            }
        }
        _ => {}
    }
}

fn is_forbidden_key(key: &str) -> bool {
    EXACT_FORBIDDEN_KEYS.contains(&key)
        || key.contains("oauth")
        || key.ends_with("_password")
        || key.ends_with("_private_key")
        || key.ends_with("_secret")
        || key.ends_with("_token")
}

fn normalize_key(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let characters = value.chars().collect::<Vec<_>>();
    for (index, character) in characters.iter().copied().enumerate() {
        if character.is_ascii_uppercase() {
            let previous_is_word = index > 0
                && (characters[index - 1].is_ascii_lowercase()
                    || characters[index - 1].is_ascii_digit());
            if previous_is_word && !normalized.ends_with('_') {
                normalized.push('_');
            }
            normalized.push(character.to_ascii_lowercase());
        } else if character == '-' || character == ' ' {
            normalized.push('_');
        } else {
            normalized.push(character.to_ascii_lowercase());
        }
    }
    while normalized.contains("__") {
        normalized = normalized.replace("__", "_");
    }
    normalized.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn catches_legacy_camel_case_identity_and_invitation_fields() {
        let value = json!({
            "users": [{"authUserId": "provider-user"}],
            "invite": {"tokenHash": "hash"}
        });
        let paths = forbidden_field_paths(&value);
        assert_eq!(paths, vec!["$.invite.tokenHash", "$.users[0].authUserId"]);
    }

    #[test]
    fn permits_operational_token_usage_and_provider_message_ids() {
        let value = json!({
            "tokenUsage": 123,
            "providerMessageId": "message-1",
            "sourceCount": 3
        });
        validate_no_secrets(&value).expect("operational metadata is safe");
    }

    #[test]
    fn catches_uppercase_environment_style_keys() {
        let value = json!({"DATABASE_URL": "postgres://secret"});
        assert_eq!(forbidden_field_paths(&value), vec!["$.DATABASE_URL"]);
    }
}
