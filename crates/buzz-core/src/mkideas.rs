//! MK Ideas operational event contract.
//!
//! Human state changes stay signed by the human who performed them. The relay
//! validates this envelope and advances a community-wide entity head keyed by
//! `(community, kind, d)` rather than re-signing the state with a service key.

use nostr::Event;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::kind::{
    is_mkideas_state_kind, KIND_MK_APPROVAL, KIND_MK_APPROVAL_ACTION, KIND_MK_CONTENT,
    KIND_MK_DECISION, KIND_MK_GOAL, KIND_MK_INTERVIEW, KIND_MK_KNOWLEDGE, KIND_MK_MEETING,
    KIND_MK_OPERATIONAL_PROJECT, KIND_MK_PERSON, KIND_MK_TASK,
};

/// Current MK Ideas JSON schema version.
pub const MK_SCHEMA_VERSION: u64 = 2;
/// Oldest state schema that remains readable during the V1 rollout.
pub const MK_MIN_READ_SCHEMA_VERSION: u64 = 1;

/// Parsed, validated envelope shared by every MK Ideas state event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MkStateEnvelope {
    /// Stable entity identifier, mirrored by the event's `d` tag.
    pub entity_id: Uuid,
    /// Host-derived community identifier, mirrored by the event's `h` tag.
    ///
    /// Buzz intentionally never accepts its internal database UUID from a
    /// client. The public community identifier is therefore the normalized
    /// relay host that both desktop and mobile already know.
    pub community: String,
    /// Monotonic application version.
    pub version: i64,
    /// Previous authoritative event id, absent only for version one.
    pub previous_event_id: Option<[u8; 32]>,
    /// Validated domain status.
    pub status: String,
    /// Parsed event content.
    pub content: Value,
    /// Version of the schema that validated this payload.
    pub schema_version: u64,
}

/// Exact operational record revision targeted by an agent proposal.
///
/// Schema-v2 producers use the structured `target` object. Flat fields remain
/// readable during the V1 rollout; when both shapes are present they must
/// describe the same signed revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MkAgentProposalTarget {
    /// Stable entity UUID.
    pub entity_id: Uuid,
    /// MK Ideas state event kind.
    pub kind: u32,
    /// Exact signed target event ID when supplied.
    pub event_id: Option<[u8; 32]>,
    /// Monotonic target version when supplied.
    pub version: Option<i64>,
}

/// Validation failure for an MK Ideas event.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MkValidationError {
    /// A required tag is absent, duplicated, or malformed.
    #[error("invalid MK Ideas tag: {0}")]
    Tag(String),
    /// The JSON payload is malformed or violates its typed contract.
    #[error("invalid MK Ideas content: {0}")]
    Content(String),
    /// A requested state transition is not legal.
    #[error("invalid MK Ideas transition: {0}")]
    Transition(String),
}

fn one_tag(event: &Event, name: &str) -> Result<String, MkValidationError> {
    let values: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let parts = tag.as_slice();
            (parts.first().map(String::as_str) == Some(name))
                .then(|| parts.get(1).cloned())
                .flatten()
        })
        .collect();
    match values.as_slice() {
        [value] => Ok(value.clone()),
        [] => Err(MkValidationError::Tag(format!("missing `{name}` tag"))),
        _ => Err(MkValidationError::Tag(format!(
            "expected one `{name}` tag, got {}",
            values.len()
        ))),
    }
}

fn optional_one_tag(event: &Event, name: &str) -> Result<Option<String>, MkValidationError> {
    let values: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let parts = tag.as_slice();
            (parts.first().map(String::as_str) == Some(name))
                .then(|| parts.get(1).cloned())
                .flatten()
        })
        .collect();
    match values.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(value.clone())),
        _ => Err(MkValidationError::Tag(format!(
            "expected at most one `{name}` tag, got {}",
            values.len()
        ))),
    }
}

fn parse_event_id(value: &str) -> Result<[u8; 32], MkValidationError> {
    let bytes = hex::decode(value)
        .map_err(|_| MkValidationError::Tag("`prev` must be lowercase hex".to_string()))?;
    bytes
        .try_into()
        .map_err(|_| MkValidationError::Tag("`prev` must be 32 bytes".to_string()))
}

fn required_string<'a>(content: &'a Value, key: &str) -> Result<&'a str, MkValidationError> {
    let value = content
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| MkValidationError::Content(format!("`{key}` is required")))?;
    if value.chars().count() > 500 {
        return Err(MkValidationError::Content(format!(
            "`{key}` exceeds 500 characters"
        )));
    }
    Ok(value)
}

fn require_uuid_field(content: &Value, key: &str) -> Result<Uuid, MkValidationError> {
    let value = required_string(content, key)?;
    Uuid::parse_str(value)
        .map_err(|_| MkValidationError::Content(format!("`{key}` must be a UUID")))
}

fn proposal_target_value<'a>(
    content: &'a Value,
    flat_key: &str,
    nested_key: &str,
) -> Result<Option<&'a Value>, MkValidationError> {
    let flat = content.get(flat_key);
    let nested = content
        .get("target")
        .and_then(|target| target.get(nested_key));
    if flat.is_some() && nested.is_some() && flat != nested {
        return Err(MkValidationError::Content(format!(
            "`{flat_key}` must match `target.{nested_key}`"
        )));
    }
    Ok(flat.or(nested))
}

/// Parse the exact entity revision bound to a schema-v1 or schema-v2 proposal.
pub fn parse_agent_proposal_target(
    content: &Value,
) -> Result<MkAgentProposalTarget, MkValidationError> {
    let entity_id = proposal_target_value(content, "target_id", "id")?
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| MkValidationError::Content("proposal target id must be a UUID".into()))?;
    let kind = proposal_target_value(content, "target_kind", "kind")?
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            MkValidationError::Content("proposal target kind must be an integer".into())
        })?;
    let event_id = proposal_target_value(content, "target_event_id", "event_id")?
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| {
                    MkValidationError::Content("proposal target event id must be a string".into())
                })
                .and_then(parse_event_id)
        })
        .transpose()?;
    let version = proposal_target_value(content, "target_version", "version")?
        .map(|value| {
            value
                .as_i64()
                .filter(|version| *version >= 1)
                .ok_or_else(|| {
                    MkValidationError::Content(
                        "proposal target version must be a positive integer".into(),
                    )
                })
        })
        .transpose()?;
    Ok(MkAgentProposalTarget {
        entity_id,
        kind,
        event_id,
        version,
    })
}

fn status_allowed(status: &str, allowed: &[&str]) -> Result<(), MkValidationError> {
    if allowed.contains(&status) {
        Ok(())
    } else {
        Err(MkValidationError::Content(format!(
            "unsupported status `{status}`"
        )))
    }
}

fn require_link_tag(
    event: &Event,
    name: &str,
    expected_values: &[String],
) -> Result<(), MkValidationError> {
    let matches = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some(name))
        .collect::<Vec<_>>();
    let valid = matches.len() == 1
        && matches[0]
            .as_slice()
            .get(1..)
            .is_some_and(|values| values == expected_values);
    if !valid {
        return Err(MkValidationError::Tag(format!(
            "`{name}` must appear once and match the typed relationship"
        )));
    }
    Ok(())
}

fn expected_record_type(kind: u32) -> Option<&'static str> {
    match kind {
        KIND_MK_GOAL => Some("goal"),
        KIND_MK_OPERATIONAL_PROJECT => Some("operational_project"),
        KIND_MK_TASK => Some("task"),
        KIND_MK_PERSON => Some("person"),
        KIND_MK_INTERVIEW => Some("interview"),
        KIND_MK_CONTENT => Some("content"),
        KIND_MK_MEETING => Some("meeting"),
        KIND_MK_DECISION => Some("decision"),
        KIND_MK_KNOWLEDGE => Some("knowledge"),
        KIND_MK_APPROVAL => Some("approval"),
        _ => None,
    }
}

fn validate_v2_metadata(content: &Value) -> Result<(), MkValidationError> {
    required_string(content, "source")?;
    let provenance = content
        .get("provenance")
        .and_then(Value::as_object)
        .ok_or_else(|| MkValidationError::Content("`provenance` must be an object".to_string()))?;
    if provenance.is_empty() {
        return Err(MkValidationError::Content(
            "`provenance` must not be empty".to_string(),
        ));
    }

    if let Some(assignees) = content.get("assignees") {
        let assignees = assignees.as_array().ok_or_else(|| {
            MkValidationError::Content("`assignees` must be an array".to_string())
        })?;
        if assignees.len() > 32
            || assignees.iter().any(|value| {
                value
                    .as_str()
                    .is_none_or(|pubkey| pubkey.len() != 64 || hex::decode(pubkey).is_err())
            })
        {
            return Err(MkValidationError::Content(
                "`assignees` must contain at most 32 hex pubkeys".to_string(),
            ));
        }
    }
    if let Some(deadline) = content.get("deadline") {
        if !deadline.is_null()
            && deadline
                .as_str()
                .is_none_or(|value| value.trim().is_empty() || value.len() > 64)
        {
            return Err(MkValidationError::Content(
                "`deadline` must be null or an ISO-8601 string".to_string(),
            ));
        }
    }
    if let Some(links) = content.get("links") {
        let links = links
            .as_array()
            .ok_or_else(|| MkValidationError::Content("`links` must be an array".to_string()))?;
        if links.len() > 128 {
            return Err(MkValidationError::Content(
                "`links` exceeds 128 entries".to_string(),
            ));
        }
        for link in links {
            let kind = link.get("kind").and_then(Value::as_u64).ok_or_else(|| {
                MkValidationError::Content("every link requires `kind`".to_string())
            })? as u32;
            if !is_mkideas_state_kind(kind) {
                return Err(MkValidationError::Content(
                    "links may target only MK Ideas state kinds".to_string(),
                ));
            }
            require_uuid_field(link, "d")?;
            required_string(link, "relationship")?;
        }
    }
    Ok(())
}

fn required_title(content: &Value) -> Result<&str, MkValidationError> {
    content
        .get("title")
        .or_else(|| content.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| MkValidationError::Content("`title` or `name` is required".to_string()))
}

fn validate_media_descriptor(media: &Value) -> Result<(), MkValidationError> {
    let object = media.as_object().ok_or_else(|| {
        MkValidationError::Content("media descriptor must be an object".to_string())
    })?;
    for field in [
        "media_id",
        "sha256",
        "mime_type",
        "filename",
        "uploaded_by",
        "uploaded_at",
        "source",
    ] {
        required_string(media, field)?;
    }
    let digest = object
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| MkValidationError::Content("`sha256` is required".to_string()))?;
    if digest.len() != 64 || hex::decode(digest).is_err() {
        return Err(MkValidationError::Content(
            "media `sha256` must be 32-byte hex".to_string(),
        ));
    }
    let size = object
        .get("size")
        .and_then(Value::as_u64)
        .ok_or_else(|| MkValidationError::Content("media `size` is required".to_string()))?;
    if size == 0 || size > 1_073_741_824 {
        return Err(MkValidationError::Content(
            "media `size` must be between 1 byte and 1 GiB".to_string(),
        ));
    }
    Ok(())
}

fn validate_v2_state_fields(
    event: &Event,
    kind: u32,
    content: &Value,
    status: &str,
) -> Result<(), MkValidationError> {
    match kind {
        KIND_MK_GOAL => {
            required_title(content)?;
            status_allowed(
                status,
                &["draft", "active", "on-hold", "completed", "archived"],
            )?;
        }
        KIND_MK_OPERATIONAL_PROJECT => {
            required_title(content)?;
            status_allowed(
                status,
                &["planned", "active", "blocked", "completed", "archived"],
            )?;
        }
        KIND_MK_TASK => {
            required_title(content)?;
            status_allowed(
                status,
                &[
                    "backlog",
                    "to-do",
                    "in-progress",
                    "blocked",
                    "review",
                    "done",
                    "cancelled",
                ],
            )?;
        }
        KIND_MK_PERSON => {
            required_string(content, "name")?;
            status_allowed(
                status,
                &[
                    "prospect",
                    "researching",
                    "ready-to-contact",
                    "contacted",
                    "responded",
                    "scheduled",
                    "interviewed",
                    "nurture",
                    "closed",
                    "archived",
                ],
            )?;
            if content
                .get("do_not_contact")
                .and_then(Value::as_bool)
                .is_none()
            {
                return Err(MkValidationError::Content(
                    "`do_not_contact` must be a boolean".to_string(),
                ));
            }
        }
        KIND_MK_INTERVIEW => {
            required_title(content)?;
            let guest_id = require_uuid_field(content, "guest_id")?;
            require_link_tag(event, "guest", &[guest_id.to_string()])?;
            status_allowed(
                status,
                &[
                    "idea",
                    "planning",
                    "scheduled",
                    "recorded",
                    "transcribing",
                    "reviewing",
                    "complete",
                    "cancelled",
                ],
            )?;
            if content.get("transcript_text").is_some() {
                return Err(MkValidationError::Content(
                    "schema v2 stores transcripts as private media descriptors, not event text"
                        .to_string(),
                ));
            }
            if let Some(transcript) = content.get("transcript") {
                validate_media_descriptor(transcript)?;
                for field in ["format", "language"] {
                    required_string(transcript, field)?;
                }
                if transcript.get("version").and_then(Value::as_u64).is_none() {
                    return Err(MkValidationError::Content(
                        "transcript `version` is required".to_string(),
                    ));
                }
            }
        }
        KIND_MK_CONTENT => {
            required_title(content)?;
            if content.get("interview_id").is_some() {
                let interview_id = require_uuid_field(content, "interview_id")?;
                require_link_tag(event, "interview", &[interview_id.to_string()])?;
            }
            status_allowed(
                status,
                &[
                    "idea",
                    "draft",
                    "in-review",
                    "approved",
                    "scheduled",
                    "published",
                    "archived",
                ],
            )?;
        }
        KIND_MK_MEETING => {
            required_title(content)?;
            status_allowed(status, &["planned", "completed", "cancelled"])?;
        }
        KIND_MK_DECISION => {
            required_title(content)?;
            status_allowed(status, &["proposed", "decided", "superseded", "archived"])?;
        }
        KIND_MK_KNOWLEDGE => {
            required_title(content)?;
            status_allowed(status, &["draft", "verified", "archived"])?;
        }
        KIND_MK_APPROVAL => {
            let target_id = require_uuid_field(content, "target_id")?;
            let target_kind = content
                .get("target_kind")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    MkValidationError::Content("`target_kind` is required".to_string())
                })? as u32;
            if !is_mkideas_state_kind(target_kind) || target_kind == KIND_MK_APPROVAL {
                return Err(MkValidationError::Content(
                    "approval target must be an operational state record".to_string(),
                ));
            }
            let target_event_id = required_string(content, "target_event_id")?;
            parse_event_id(target_event_id).map_err(|_| {
                MkValidationError::Content(
                    "`target_event_id` must be a 32-byte event id".to_string(),
                )
            })?;
            if content
                .get("target_version")
                .and_then(Value::as_u64)
                .is_none()
            {
                return Err(MkValidationError::Content(
                    "`target_version` is required".to_string(),
                ));
            }
            let proposal_id = require_uuid_field(content, "proposal_id")?;
            let proposal_event_id = required_string(content, "proposal_event_id")?;
            parse_event_id(proposal_event_id).map_err(|_| {
                MkValidationError::Content(
                    "`proposal_event_id` must be a 32-byte event id".to_string(),
                )
            })?;
            require_link_tag(
                event,
                "target",
                &[
                    target_kind.to_string(),
                    target_id.to_string(),
                    target_event_id.to_string(),
                ],
            )?;
            require_link_tag(
                event,
                "proposal",
                &[proposal_id.to_string(), proposal_event_id.to_string()],
            )?;
            status_allowed(
                status,
                &["pending", "approved", "rejected", "stale", "cancelled"],
            )?;
        }
        _ => {
            return Err(MkValidationError::Content(format!(
                "unsupported MK Ideas state kind {kind}"
            )));
        }
    }
    Ok(())
}

fn is_legal_v2_transition(kind: u32, previous: &str, next: &str) -> bool {
    if previous == next {
        return true;
    }
    match kind {
        KIND_MK_GOAL => matches!(
            (previous, next),
            ("draft", "active" | "on-hold" | "completed" | "archived")
                | ("active", "on-hold" | "completed" | "archived")
                | ("on-hold", "active" | "completed" | "archived")
                | ("completed", "archived")
        ),
        KIND_MK_OPERATIONAL_PROJECT => matches!(
            (previous, next),
            ("planned", "active" | "blocked" | "completed" | "archived")
                | ("active", "blocked" | "completed" | "archived")
                | ("blocked", "active" | "completed" | "archived")
                | ("completed", "archived")
        ),
        KIND_MK_TASK => matches!(
            (previous, next),
            ("backlog", "to-do" | "in-progress" | "cancelled")
                | ("to-do", "backlog" | "in-progress" | "blocked" | "cancelled")
                | (
                    "in-progress",
                    "to-do" | "blocked" | "review" | "done" | "cancelled"
                )
                | ("blocked", "to-do" | "in-progress" | "cancelled")
                | ("review", "in-progress" | "done" | "cancelled")
        ),
        KIND_MK_PERSON => matches!(
            (previous, next),
            (
                "prospect",
                "researching" | "ready-to-contact" | "closed" | "archived"
            ) | (
                "researching",
                "ready-to-contact" | "nurture" | "closed" | "archived"
            ) | (
                "ready-to-contact",
                "contacted" | "nurture" | "closed" | "archived"
            ) | ("contacted", "responded" | "nurture" | "closed" | "archived")
                | ("responded", "scheduled" | "nurture" | "closed" | "archived")
                | (
                    "scheduled",
                    "interviewed" | "nurture" | "closed" | "archived"
                )
                | ("interviewed", "nurture" | "closed" | "archived")
                | (
                    "nurture",
                    "researching"
                        | "ready-to-contact"
                        | "contacted"
                        | "scheduled"
                        | "closed"
                        | "archived"
                )
                | ("closed", "nurture" | "archived")
        ),
        KIND_MK_INTERVIEW => matches!(
            (previous, next),
            ("idea", "planning" | "cancelled")
                | ("planning", "scheduled" | "recorded" | "cancelled")
                | ("scheduled", "recorded" | "cancelled")
                | ("recorded", "transcribing" | "reviewing" | "complete")
                | ("transcribing", "reviewing" | "complete")
                | ("reviewing", "transcribing" | "complete")
        ),
        KIND_MK_CONTENT => matches!(
            (previous, next),
            ("idea", "draft" | "archived")
                | ("draft", "in-review" | "archived")
                | ("in-review", "draft" | "approved" | "archived")
                | ("approved", "draft" | "scheduled" | "published" | "archived")
                | ("scheduled", "approved" | "published" | "archived")
                | ("published", "archived")
        ),
        KIND_MK_MEETING => matches!((previous, next), ("planned", "completed" | "cancelled")),
        KIND_MK_DECISION => matches!(
            (previous, next),
            ("proposed", "decided" | "archived")
                | ("decided", "superseded" | "archived")
                | ("superseded", "archived")
        ),
        KIND_MK_KNOWLEDGE => matches!(
            (previous, next),
            ("draft", "verified" | "archived") | ("verified", "draft" | "archived")
        ),
        KIND_MK_APPROVAL => matches!(
            (previous, next),
            ("pending", "approved" | "rejected" | "stale" | "cancelled") | ("stale", "cancelled")
        ),
        _ => false,
    }
}

/// Validate an MK Ideas state event and return its canonical envelope.
///
/// Schema v1 remains readable for rolling upgrades. New product clients write
/// schema v2, which covers every V1 entity and carries explicit provenance.
pub fn validate_state_event(event: &Event) -> Result<MkStateEnvelope, MkValidationError> {
    let kind = u32::from(event.kind.as_u16());
    let expected_type = expected_record_type(kind).ok_or_else(|| {
        MkValidationError::Content(format!("kind {kind} is not an MK Ideas state kind"))
    })?;

    let d_tag = one_tag(event, "d")?;
    let entity_id = Uuid::parse_str(&d_tag)
        .map_err(|_| MkValidationError::Tag("`d` must be a UUID".to_string()))?;
    let community = one_tag(event, "h")?;
    if community.trim().is_empty() || community.chars().count() > 253 {
        return Err(MkValidationError::Tag(
            "`h` must be a non-empty community host".to_string(),
        ));
    }
    let version = one_tag(event, "version")?
        .parse::<i64>()
        .map_err(|_| MkValidationError::Tag("`version` must be an integer".to_string()))?;
    if version < 1 {
        return Err(MkValidationError::Tag(
            "`version` must be at least 1".to_string(),
        ));
    }
    let previous_event_id = match optional_one_tag(event, "prev")? {
        Some(value) if !value.is_empty() => Some(parse_event_id(&value)?),
        Some(_) | None if version == 1 => None,
        Some(_) | None => {
            return Err(MkValidationError::Tag(
                "`prev` is required after version 1".to_string(),
            ));
        }
    };

    let content: Value = serde_json::from_str(&event.content)
        .map_err(|error| MkValidationError::Content(error.to_string()))?;
    let object = content
        .as_object()
        .ok_or_else(|| MkValidationError::Content("payload must be an object".to_string()))?;
    let schema_version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| MkValidationError::Content("`schema_version` is required".to_string()))?;
    if !(MK_MIN_READ_SCHEMA_VERSION..=MK_SCHEMA_VERSION).contains(&schema_version) {
        return Err(MkValidationError::Content(
            "`schema_version` must be 1 or 2".to_string(),
        ));
    }
    if schema_version == MK_MIN_READ_SCHEMA_VERSION
        && !matches!(
            kind,
            KIND_MK_PERSON | KIND_MK_INTERVIEW | KIND_MK_CONTENT | KIND_MK_APPROVAL
        )
    {
        return Err(MkValidationError::Content(
            "schema v1 is supported only for the V0 entity kinds".to_string(),
        ));
    }
    if schema_version == MK_SCHEMA_VERSION {
        validate_v2_metadata(&content)?;
    }
    if object.get("entity_id").and_then(Value::as_str) != Some(d_tag.as_str()) {
        return Err(MkValidationError::Content(
            "`entity_id` must match the `d` tag".to_string(),
        ));
    }
    if object.get("version").and_then(Value::as_i64) != Some(version) {
        return Err(MkValidationError::Content(
            "payload `version` must match the tag".to_string(),
        ));
    }
    if object.get("record_type").and_then(Value::as_str) != Some(expected_type) {
        return Err(MkValidationError::Content(format!(
            "`record_type` must be `{expected_type}`"
        )));
    }
    let status = required_string(&content, "status")?.to_string();
    if one_tag(event, "status")? != status {
        return Err(MkValidationError::Tag(
            "`status` tag must match payload status".to_string(),
        ));
    }

    if schema_version == MK_MIN_READ_SCHEMA_VERSION {
        match kind {
            KIND_MK_PERSON => {
                required_string(&content, "name")?;
                status_allowed(
                    &status,
                    &[
                        "potential",
                        "researching",
                        "research_ready",
                        "interview_planning",
                        "scheduled",
                        "interviewed",
                        "content_processing",
                        "relationship_nurture",
                        "paused",
                        "declined",
                        "not_a_fit",
                        "do_not_contact",
                        "archived",
                    ],
                )?;
                let do_not_contact = content
                    .get("do_not_contact")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        MkValidationError::Content("`do_not_contact` must be a boolean".to_string())
                    })?;
                if do_not_contact != (status == "do_not_contact") {
                    return Err(MkValidationError::Content(
                        "status and `do_not_contact` must agree".to_string(),
                    ));
                }
            }
            KIND_MK_INTERVIEW => {
                required_string(&content, "title")?;
                let guest_id = require_uuid_field(&content, "guest_id")?;
                require_link_tag(event, "guest", &[guest_id.to_string()])?;
                status_allowed(
                    &status,
                    &[
                        "planning",
                        "research",
                        "questions_ready",
                        "awaiting_review",
                        "scheduled",
                        "recorded",
                        "transcript_processing",
                        "content_processing",
                        "review",
                        "complete",
                        "archived",
                    ],
                )?;
            }
            KIND_MK_CONTENT => {
                required_string(&content, "title")?;
                let interview_id = require_uuid_field(&content, "interview_id")?;
                require_link_tag(event, "interview", &[interview_id.to_string()])?;
                status_allowed(
                    &status,
                    &[
                        "idea",
                        "research",
                        "planned",
                        "recording",
                        "editing",
                        "internal_review",
                        "changes_requested",
                        "approved",
                        "scheduled",
                        "published",
                        "performance_review",
                        "archived",
                    ],
                )?;
            }
            KIND_MK_APPROVAL => {
                let target_id = require_uuid_field(&content, "target_id")?;
                let proposal_id = require_uuid_field(&content, "proposal_id")?;
                let proposal_event_id = required_string(&content, "proposal_event_id")?;
                parse_event_id(proposal_event_id).map_err(|_| {
                    MkValidationError::Content(
                        "`proposal_event_id` must be a 32-byte event id".to_string(),
                    )
                })?;
                let target_kind = content
                    .get("target_kind")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        MkValidationError::Content("`target_kind` is required".to_string())
                    })? as u32;
                if !matches!(
                    target_kind,
                    KIND_MK_PERSON | KIND_MK_INTERVIEW | KIND_MK_CONTENT
                ) {
                    return Err(MkValidationError::Content(
                        "approval target is not enabled in V0".to_string(),
                    ));
                }
                require_link_tag(
                    event,
                    "target",
                    &[target_kind.to_string(), target_id.to_string()],
                )?;
                require_link_tag(
                    event,
                    "proposal",
                    &[proposal_id.to_string(), proposal_event_id.to_string()],
                )?;
                status_allowed(&status, &["pending", "approved", "rejected", "cancelled"])?;
            }
            _ => unreachable!("schema v1 kind gate filtered unsupported kinds"),
        }
    } else {
        validate_v2_state_fields(event, kind, &content, &status)?;
    }

    Ok(MkStateEnvelope {
        entity_id,
        community,
        version,
        previous_event_id,
        status,
        content,
        schema_version,
    })
}

/// Validate an accepted update against the previous authoritative payload.
pub fn validate_state_transition(
    kind: u32,
    previous_content: Option<&str>,
    next: &MkStateEnvelope,
) -> Result<(), MkValidationError> {
    validate_state_transition_with_policy(kind, previous_content, next, false)
}

/// Validate an update with relay-authorized transition policy.
///
/// `allow_dnc_clear` must only be set after the relay has established that the
/// signer is the active community owner. The signed payload still carries the
/// required reason, preserving who cleared the protection and why.
pub fn validate_state_transition_with_policy(
    kind: u32,
    previous_content: Option<&str>,
    next: &MkStateEnvelope,
    allow_dnc_clear: bool,
) -> Result<(), MkValidationError> {
    let Some(previous_content) = previous_content else {
        if next.version != 1 || next.previous_event_id.is_some() {
            return Err(MkValidationError::Transition(
                "new records must start at version 1 without `prev`".to_string(),
            ));
        }
        return Ok(());
    };

    let previous: Value = serde_json::from_str(previous_content)
        .map_err(|error| MkValidationError::Transition(error.to_string()))?;
    let previous_version = previous
        .get("version")
        .and_then(Value::as_i64)
        .ok_or_else(|| MkValidationError::Transition("previous version missing".to_string()))?;
    if next.version != previous_version + 1 {
        return Err(MkValidationError::Transition(format!(
            "expected version {}, got {}",
            previous_version + 1,
            next.version
        )));
    }

    if kind == KIND_MK_PERSON {
        let was_do_not_contact = previous
            .get("do_not_contact")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_do_not_contact = next
            .content
            .get("do_not_contact")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if was_do_not_contact && !is_do_not_contact {
            let reason = next
                .content
                .get("dnc_override_reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty() && value.chars().count() <= 500);
            if next.schema_version != MK_SCHEMA_VERSION || !allow_dnc_clear || reason.is_none() {
                return Err(MkValidationError::Transition(
                    "do-not-contact may only be cleared by the owner with `dnc_override_reason`"
                        .to_string(),
                ));
            }
        }
    }

    let previous_status = previous
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if next.schema_version == MK_SCHEMA_VERSION {
        if !is_legal_v2_transition(kind, previous_status, &next.status) {
            return Err(MkValidationError::Transition(format!(
                "status cannot move from `{previous_status}` to `{}`",
                next.status
            )));
        }
        return Ok(());
    }
    if previous_status == "archived" && next.status != "archived" {
        return Err(MkValidationError::Transition(
            "archived records cannot be reopened in V0".to_string(),
        ));
    }
    if previous_status != next.status && next.status != "archived" {
        let legal = match kind {
            KIND_MK_PERSON => matches!(
                (previous_status, next.status.as_str()),
                (
                    "potential",
                    "researching"
                        | "research_ready"
                        | "interview_planning"
                        | "paused"
                        | "declined"
                        | "not_a_fit"
                        | "do_not_contact"
                ) | (
                    "researching",
                    "research_ready"
                        | "interview_planning"
                        | "paused"
                        | "declined"
                        | "not_a_fit"
                        | "do_not_contact"
                ) | (
                    "research_ready",
                    "interview_planning"
                        | "scheduled"
                        | "paused"
                        | "declined"
                        | "not_a_fit"
                        | "do_not_contact"
                ) | (
                    "interview_planning",
                    "scheduled" | "interviewed" | "paused" | "declined" | "do_not_contact"
                ) | (
                    "scheduled",
                    "interviewed" | "paused" | "declined" | "do_not_contact"
                ) | (
                    "interviewed",
                    "content_processing" | "relationship_nurture" | "paused" | "do_not_contact"
                ) | (
                    "content_processing",
                    "relationship_nurture" | "paused" | "do_not_contact"
                ) | (
                    "relationship_nurture",
                    "interview_planning" | "paused" | "do_not_contact"
                ) | (
                    "paused",
                    "researching" | "research_ready" | "interview_planning" | "do_not_contact"
                )
            ),
            KIND_MK_INTERVIEW => matches!(
                (previous_status, next.status.as_str()),
                (
                    "planning",
                    "research"
                        | "questions_ready"
                        | "awaiting_review"
                        | "scheduled"
                        | "recorded"
                        | "transcript_processing"
                        | "content_processing"
                ) | (
                    "research",
                    "questions_ready" | "awaiting_review" | "scheduled" | "content_processing"
                ) | (
                    "questions_ready",
                    "awaiting_review" | "scheduled" | "recorded" | "content_processing"
                ) | (
                    "awaiting_review",
                    "questions_ready" | "scheduled" | "content_processing"
                ) | (
                    "scheduled",
                    "recorded" | "transcript_processing" | "content_processing"
                ) | (
                    "recorded",
                    "transcript_processing" | "content_processing" | "review"
                ) | ("transcript_processing", "content_processing" | "review")
                    | ("content_processing", "review" | "complete")
                    | ("review", "content_processing" | "complete")
            ),
            KIND_MK_CONTENT => matches!(
                (previous_status, next.status.as_str()),
                (
                    "idea",
                    "research" | "planned" | "recording" | "editing" | "internal_review"
                ) | (
                    "research",
                    "planned" | "recording" | "editing" | "internal_review"
                ) | ("planned", "recording" | "editing" | "internal_review")
                    | ("recording", "editing" | "internal_review")
                    | ("editing", "internal_review")
                    | ("internal_review", "changes_requested" | "approved")
                    | ("changes_requested", "editing" | "internal_review")
                    | ("approved", "scheduled" | "published")
                    | ("scheduled", "published")
                    | ("published", "performance_review")
            ),
            KIND_MK_APPROVAL => matches!(
                (previous_status, next.status.as_str()),
                ("pending", "approved" | "rejected" | "cancelled")
            ),
            _ => false,
        };
        if !legal {
            return Err(MkValidationError::Transition(format!(
                "status cannot move from `{previous_status}` to `{}`",
                next.status
            )));
        }
    }
    Ok(())
}

/// Validate an append-only human approval action.
pub fn validate_approval_action(event: &Event) -> Result<(), MkValidationError> {
    if u32::from(event.kind.as_u16()) != KIND_MK_APPROVAL_ACTION {
        return Err(MkValidationError::Content(
            "not an MK approval action".to_string(),
        ));
    }
    let community = one_tag(event, "h")?;
    if community.trim().is_empty() || community.chars().count() > 253 {
        return Err(MkValidationError::Tag(
            "`h` must be a non-empty community host".to_string(),
        ));
    }
    let content: Value = serde_json::from_str(&event.content)
        .map_err(|error| MkValidationError::Content(error.to_string()))?;
    let schema_version = content
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| MkValidationError::Content("`schema_version` is required".to_string()))?;
    if !(MK_MIN_READ_SCHEMA_VERSION..=MK_SCHEMA_VERSION).contains(&schema_version) {
        return Err(MkValidationError::Content(
            "`schema_version` must be 1 or 2".to_string(),
        ));
    }
    require_uuid_field(&content, "approval_id")?;
    require_uuid_field(&content, "target_id")?;
    require_uuid_field(&content, "proposal_id")?;
    if schema_version == MK_SCHEMA_VERSION {
        require_uuid_field(&content, "action_id")?;
        for field in ["approval_event_id", "target_event_id", "proposal_event_id"] {
            let event_id = required_string(&content, field)?;
            parse_event_id(event_id).map_err(|_| {
                MkValidationError::Content(format!("`{field}` must be a 32-byte event id"))
            })?;
        }
        if content
            .get("target_version")
            .and_then(Value::as_u64)
            .is_none()
        {
            return Err(MkValidationError::Content(
                "`target_version` is required".to_string(),
            ));
        }
        required_string(&content, "reason")?;
        if content.get("approved").is_some() || content.get("auto_approved").is_some() {
            return Err(MkValidationError::Content(
                "approval actions express a human decision, never an auto-approval flag"
                    .to_string(),
            ));
        }
    } else {
        let proposal_event_id = one_tag(event, "e")?;
        parse_event_id(&proposal_event_id).map_err(|_| {
            MkValidationError::Tag("`e` must identify the proposal event".to_string())
        })?;
    }
    let decision = required_string(&content, "decision")?;
    if !matches!(decision, "approved" | "rejected") {
        return Err(MkValidationError::Content(
            "`decision` must be `approved` or `rejected`".to_string(),
        ));
    }
    Ok(())
}

/// Validate a service-authored agent proposal. Proposals are append-only and
/// deliberately carry no approval authority.
pub fn validate_agent_proposal(event: &Event) -> Result<(), MkValidationError> {
    use crate::kind::KIND_MK_AGENT_PROPOSAL;

    if u32::from(event.kind.as_u16()) != KIND_MK_AGENT_PROPOSAL {
        return Err(MkValidationError::Content(
            "not an MK agent proposal".to_string(),
        ));
    }
    let community = one_tag(event, "h")?;
    if community.trim().is_empty() || community.chars().count() > 253 {
        return Err(MkValidationError::Tag(
            "`h` must be a non-empty community host".to_string(),
        ));
    }
    let content: Value = serde_json::from_str(&event.content)
        .map_err(|error| MkValidationError::Content(error.to_string()))?;
    let schema_version = content
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| MkValidationError::Content("`schema_version` is required".to_string()))?;
    if !(MK_MIN_READ_SCHEMA_VERSION..=MK_SCHEMA_VERSION).contains(&schema_version) {
        return Err(MkValidationError::Content(
            "`schema_version` must be 1 or 2".to_string(),
        ));
    }
    require_uuid_field(&content, "proposal_id")?;
    if content.get("proposal_version").and_then(Value::as_u64) != Some(1) {
        return Err(MkValidationError::Content(
            "`proposal_version` must be 1".to_string(),
        ));
    }
    let target = parse_agent_proposal_target(&content)?;
    if !is_mkideas_state_kind(target.kind) || target.kind == KIND_MK_APPROVAL {
        return Err(MkValidationError::Content(
            "proposal target must be an operational MK Ideas record".to_string(),
        ));
    }
    require_link_tag(
        event,
        "target",
        &[target.kind.to_string(), target.entity_id.to_string()],
    )?;
    if required_string(&content, "agent").is_err()
        && required_string(&content, "agent_pubkey").is_err()
    {
        return Err(MkValidationError::Content(
            "`agent` or `agent_pubkey` is required".to_string(),
        ));
    }
    required_string(&content, "proposal_type")?;
    required_string(&content, "summary")?;
    if schema_version == MK_SCHEMA_VERSION {
        require_uuid_field(&content, "run_id")?;
        if required_string(&content, "persona").is_err()
            && required_string(&content, "persona_id").is_err()
        {
            return Err(MkValidationError::Content(
                "`persona` or `persona_id` is required".to_string(),
            ));
        }
        if target.event_id.is_none() {
            return Err(MkValidationError::Content(
                "`target_event_id` or `target.event_id` is required".to_string(),
            ));
        }
        if target.version.is_none() {
            return Err(MkValidationError::Content(
                "`target_version` or `target.version` is required".to_string(),
            ));
        }
        required_string(&content, "input_hash")?;
        required_string(&content, "template_version")?;
        let inputs = content
            .get("input_event_ids")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty())
            .ok_or_else(|| {
                MkValidationError::Content(
                    "`input_event_ids` must be a non-empty array".to_string(),
                )
            })?;
        if inputs.iter().any(|value| {
            value
                .as_str()
                .is_none_or(|event_id| parse_event_id(event_id).is_err())
        }) {
            return Err(MkValidationError::Content(
                "every input event id must be 32-byte hex".to_string(),
            ));
        }
    }
    let provenance = content
        .get("provenance")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            MkValidationError::Content("`provenance` must be a non-empty array".to_string())
        })?;
    if provenance
        .iter()
        .any(|item| !valid_agent_provenance_item(item, schema_version))
    {
        return Err(MkValidationError::Content(
            "every provenance entry must be a non-empty source string or structured v2 source"
                .to_string(),
        ));
    }
    if let Some(clips) = content.get("clips") {
        let clips = clips
            .as_array()
            .ok_or_else(|| MkValidationError::Content("`clips` must be an array".to_string()))?;
        for clip in clips {
            for field in ["start", "end", "title", "caption"] {
                required_string(clip, field)?;
            }
        }
    }
    if content.get("status").and_then(Value::as_str) != Some("proposed") {
        return Err(MkValidationError::Content(
            "agent output status must be `proposed`".to_string(),
        ));
    }
    if one_tag(event, "status")? != "proposed" {
        return Err(MkValidationError::Tag(
            "agent proposal `status` tag must be `proposed`".to_string(),
        ));
    }
    if content.get("decision").is_some() || content.get("approved").is_some() {
        return Err(MkValidationError::Content(
            "agent proposals cannot contain approval decisions".to_string(),
        ));
    }
    Ok(())
}

fn valid_agent_provenance_item(item: &Value, schema_version: u64) -> bool {
    if let Some(value) = item.as_str().map(str::trim) {
        return !value.is_empty() && value.chars().count() <= 2_000;
    }
    if schema_version != MK_SCHEMA_VERSION {
        return false;
    }
    let Some(source) = item.as_object() else {
        return false;
    };
    for field in [
        "source_id",
        "source_type",
        "title",
        "locator",
        "retrieved_at",
    ] {
        if source
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .is_none_or(|value| value.is_empty() || value.chars().count() > 2_000)
        {
            return false;
        }
    }
    source
        .get("sha256")
        .and_then(Value::as_str)
        .is_some_and(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

/// Validate a schema-v2 generated summary or agent lifecycle activity payload.
pub fn validate_agent_service_event(event: &Event) -> Result<(), MkValidationError> {
    use crate::kind::{KIND_MK_GENERATED_SUMMARY, KIND_MK_SYSTEM_ACTIVITY};

    let kind = u32::from(event.kind.as_u16());
    if !matches!(kind, KIND_MK_GENERATED_SUMMARY | KIND_MK_SYSTEM_ACTIVITY) {
        return Err(MkValidationError::Content(
            "not an MK agent summary or lifecycle event".to_string(),
        ));
    }
    one_tag(event, "h")?;
    let content: Value = serde_json::from_str(&event.content)
        .map_err(|error| MkValidationError::Content(error.to_string()))?;
    if content.get("schema_version").and_then(Value::as_u64) != Some(MK_SCHEMA_VERSION) {
        return Err(MkValidationError::Content(
            "agent service event schema_version must be 2".to_string(),
        ));
    }
    require_uuid_field(&content, "run_id")?;
    required_string(&content, "persona_id")?;
    required_string(&content, "status")?;
    if kind == KIND_MK_GENERATED_SUMMARY {
        required_string(&content, "provider")?;
        required_string(&content, "model")?;
        let provenance = content
            .get("provenance")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                MkValidationError::Content("`provenance` must be an array".to_string())
            })?;
        if provenance
            .iter()
            .any(|item| !valid_agent_provenance_item(item, MK_SCHEMA_VERSION))
        {
            return Err(MkValidationError::Content(
                "agent service provenance contains an invalid source".to_string(),
            ));
        }
        required_string(&content, "summary")?;
        if content.get("status").and_then(Value::as_str) != Some("informational") {
            return Err(MkValidationError::Content(
                "generated summary status must be informational".to_string(),
            ));
        }
    } else {
        if content.get("activity_type").and_then(Value::as_str) != Some("agent_run") {
            return Err(MkValidationError::Content(
                "agent activity_type must be agent_run".to_string(),
            ));
        }
        if content.get("attempt").and_then(Value::as_u64).is_none() {
            return Err(MkValidationError::Content(
                "agent lifecycle attempt is required".to_string(),
            ));
        }
        required_string(&content, "input_hash")?;
        if content
            .get("input_event_ids")
            .and_then(Value::as_array)
            .is_none()
        {
            return Err(MkValidationError::Content(
                "agent lifecycle input_event_ids must be an array".to_string(),
            ));
        }
    }
    for forbidden in ["approval", "approval_action", "approved", "decision"] {
        if content.get(forbidden).is_some() {
            return Err(MkValidationError::Content(format!(
                "agent service events cannot contain `{forbidden}`"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use serde_json::json;

    fn state_event(kind: u32, content: Value, version: i64, prev: Option<&str>) -> Event {
        let entity_id = content["entity_id"].as_str().expect("entity id");
        let version_string = version.to_string();
        let mut tags = vec![
            Tag::parse(["d", entity_id]).expect("d"),
            Tag::parse(["h", "hub.mkideas.org"]).expect("h"),
            Tag::parse(["version", version_string.as_str()]).expect("version"),
            Tag::parse(["status", content["status"].as_str().expect("status")])
                .expect("status tag"),
        ];
        if let Some(previous) = prev {
            tags.push(Tag::parse(["prev", previous]).expect("prev"));
        }
        EventBuilder::new(Kind::from(kind as u16), content.to_string())
            .tags(tags)
            .sign_with_keys(&Keys::generate())
            .expect("signed")
    }

    #[test]
    fn validates_guest_create() {
        let entity_id = Uuid::new_v4().to_string();
        let event = state_event(
            KIND_MK_PERSON,
            json!({
                "schema_version": 1,
                "record_type": "person",
                "entity_id": entity_id,
                "version": 1,
                "status": "potential",
                "name": "Elena Marquez",
                "organization": "Vista Media",
                "do_not_contact": false
            }),
            1,
            None,
        );
        let envelope = validate_state_event(&event).expect("valid guest");
        assert_eq!(envelope.version, 1);
        assert_eq!(envelope.status, "potential");
    }

    #[test]
    fn rejects_agent_style_self_approval_decision() {
        let event = EventBuilder::new(
            Kind::from(KIND_MK_APPROVAL_ACTION as u16),
            json!({
                "schema_version": 1,
                "approval_id": Uuid::new_v4(),
                "target_id": Uuid::new_v4(),
                "decision": "auto_approved"
            })
            .to_string(),
        )
        .tags([Tag::parse(["h", "hub.mkideas.org"]).expect("h")])
        .sign_with_keys(&Keys::generate())
        .expect("signed");
        assert!(validate_approval_action(&event).is_err());
    }

    #[test]
    fn do_not_contact_is_sticky() {
        let entity_id = Uuid::new_v4().to_string();
        let previous = json!({
            "schema_version": 1,
            "record_type": "person",
            "entity_id": entity_id,
            "version": 1,
            "status": "do_not_contact",
            "name": "Protected Guest",
            "do_not_contact": true
        });
        let event = state_event(
            KIND_MK_PERSON,
            json!({
                "schema_version": 1,
                "record_type": "person",
                "entity_id": entity_id,
                "version": 2,
                "status": "potential",
                "name": "Protected Guest",
                "do_not_contact": false
            }),
            2,
            Some(&"11".repeat(32)),
        );
        let next = validate_state_event(&event).expect("valid envelope");
        assert!(
            validate_state_transition(KIND_MK_PERSON, Some(&previous.to_string()), &next).is_err()
        );
    }

    #[test]
    fn owner_policy_can_clear_do_not_contact_with_signed_reason() {
        let entity_id = Uuid::new_v4().to_string();
        let previous = json!({
            "schema_version": 2,
            "record_type": "person",
            "entity_id": entity_id,
            "version": 1,
            "status": "prospect",
            "name": "Protected Guest",
            "do_not_contact": true,
            "source": "manual",
            "provenance": {"type": "human", "ref": "initial-import"}
        });
        let event = state_event(
            KIND_MK_PERSON,
            json!({
                "schema_version": 2,
                "record_type": "person",
                "entity_id": entity_id,
                "version": 2,
                "status": "prospect",
                "name": "Protected Guest",
                "do_not_contact": false,
                "dnc_override_reason": "Guest asked the owner to resume contact.",
                "source": "manual",
                "provenance": {"type": "human", "ref": "owner-review"}
            }),
            2,
            Some(&"33".repeat(32)),
        );
        let next = validate_state_event(&event).expect("valid envelope");
        assert!(
            validate_state_transition(KIND_MK_PERSON, Some(&previous.to_string()), &next).is_err()
        );
        assert!(validate_state_transition_with_policy(
            KIND_MK_PERSON,
            Some(&previous.to_string()),
            &next,
            true,
        )
        .is_ok());
    }

    #[test]
    fn rejects_illegal_person_transition() {
        let entity_id = Uuid::new_v4().to_string();
        let previous = json!({
            "schema_version": 1,
            "record_type": "person",
            "entity_id": entity_id,
            "version": 1,
            "status": "potential",
            "name": "Elena Marquez",
            "do_not_contact": false
        });
        let event = state_event(
            KIND_MK_PERSON,
            json!({
                "schema_version": 1,
                "record_type": "person",
                "entity_id": entity_id,
                "version": 2,
                "status": "interviewed",
                "name": "Elena Marquez",
                "do_not_contact": false
            }),
            2,
            Some(&"22".repeat(32)),
        );
        let next = validate_state_event(&event).expect("valid envelope");
        assert!(
            validate_state_transition(KIND_MK_PERSON, Some(&previous.to_string()), &next).is_err()
        );
    }

    #[test]
    fn agent_proposal_requires_provenance_and_has_no_approval_authority() {
        let target_id = Uuid::new_v4().to_string();
        let proposal_id = Uuid::new_v4();
        let event = EventBuilder::new(
            Kind::from(crate::kind::KIND_MK_AGENT_PROPOSAL as u16),
            json!({
                "schema_version": 1,
                "proposal_id": proposal_id,
                "proposal_version": 1,
                "target_id": target_id,
                "target_kind": KIND_MK_PERSON,
                "agent": "Guest Researcher",
                "proposal_type": "guest_research",
                "summary": "A sourced research brief",
                "provenance": [],
                "status": "proposed",
                "approved": true
            })
            .to_string(),
        )
        .tags([
            Tag::parse(["h", "hub.mkideas.org"]).expect("h"),
            Tag::parse(["status", "proposed"]).expect("status"),
            Tag::parse(["target", &KIND_MK_PERSON.to_string(), &target_id]).expect("target"),
        ])
        .sign_with_keys(&Keys::generate())
        .expect("signed proposal");
        assert!(validate_agent_proposal(&event).is_err());
    }

    #[test]
    fn schema_v2_agent_proposal_accepts_structured_target_and_provenance() {
        let target_id = Uuid::new_v4();
        let target_event_id = "a1".repeat(32);
        let proposal = EventBuilder::new(
            Kind::from(crate::kind::KIND_MK_AGENT_PROPOSAL as u16),
            json!({
                "schema_version": 2,
                "proposal_id": Uuid::new_v4(),
                "proposal_version": 1,
                "run_id": Uuid::new_v4(),
                "persona_id": "guest-researcher",
                "agent_pubkey": "11".repeat(32),
                "target": {
                    "id": target_id,
                    "kind": KIND_MK_PERSON,
                    "event_id": target_event_id,
                    "version": 3
                },
                "proposal_type": "guest_research",
                "summary": "A sourced research brief",
                "input_event_ids": [target_event_id],
                "input_hash": "b2".repeat(32),
                "template_version": "guest-research.v2",
                "provenance": [{
                    "source_id": "event:guest",
                    "source_type": "mk_event",
                    "title": "Guest profile",
                    "locator": "buzz://mkideas/entity",
                    "retrieved_at": "2026-08-30T00:00:00Z",
                    "sha256": "c3".repeat(32)
                }],
                "status": "proposed"
            })
            .to_string(),
        )
        .tags([
            Tag::parse(["h", "hub.mkideas.org"]).expect("h"),
            Tag::parse(["status", "proposed"]).expect("status"),
            Tag::parse([
                "target",
                &KIND_MK_PERSON.to_string(),
                &target_id.to_string(),
            ])
            .expect("target"),
        ])
        .sign_with_keys(&Keys::generate())
        .expect("signed proposal");
        validate_agent_proposal(&proposal).expect("structured v2 proposal");

        let mut content: Value = serde_json::from_str(&proposal.content).expect("proposal JSON");
        content["target_id"] = json!(Uuid::new_v4());
        let mismatched = EventBuilder::new(proposal.kind, content.to_string())
            .tags(proposal.tags.clone())
            .sign_with_keys(&Keys::generate())
            .expect("signed mismatch");
        assert!(validate_agent_proposal(&mismatched).is_err());
    }

    #[test]
    fn schema_v2_agent_summary_is_informational_only() {
        let event = EventBuilder::new(
            Kind::from(crate::kind::KIND_MK_GENERATED_SUMMARY as u16),
            json!({
                "schema_version": 2,
                "run_id": Uuid::new_v4(),
                "persona_id": "operations-briefing-assistant",
                "status": "informational",
                "provider": "fixture",
                "model": "deterministic-local-v1",
                "summary": "Three operational items need human attention.",
                "provenance": []
            })
            .to_string(),
        )
        .tags([Tag::parse(["h", "hub.mkideas.org"]).expect("h")])
        .sign_with_keys(&Keys::generate())
        .expect("signed summary");
        validate_agent_service_event(&event).expect("informational summary");

        let mut content: Value = serde_json::from_str(&event.content).expect("summary JSON");
        content["approved"] = json!(true);
        let forbidden = EventBuilder::new(event.kind, content.to_string())
            .tags(event.tags.clone())
            .sign_with_keys(&Keys::generate())
            .expect("signed forbidden summary");
        assert!(validate_agent_service_event(&forbidden).is_err());
    }
}
