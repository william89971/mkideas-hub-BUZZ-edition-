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
    KIND_MK_APPROVAL, KIND_MK_APPROVAL_ACTION, KIND_MK_CONTENT, KIND_MK_INTERVIEW, KIND_MK_PERSON,
};

/// Current MK Ideas JSON schema version.
pub const MK_SCHEMA_VERSION: u64 = 1;

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
        KIND_MK_PERSON => Some("person"),
        KIND_MK_INTERVIEW => Some("interview"),
        KIND_MK_CONTENT => Some("content"),
        KIND_MK_APPROVAL => Some("approval"),
        _ => None,
    }
}

/// Validate a V0 MK Ideas state event and return its canonical envelope.
pub fn validate_state_event(event: &Event) -> Result<MkStateEnvelope, MkValidationError> {
    let kind = u32::from(event.kind.as_u16());
    let expected_type = expected_record_type(kind).ok_or_else(|| {
        MkValidationError::Content(format!(
            "kind {kind} is reserved for V1 and not enabled in V0"
        ))
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
    if object.get("schema_version").and_then(Value::as_u64) != Some(MK_SCHEMA_VERSION) {
        return Err(MkValidationError::Content(
            "`schema_version` must be 1".to_string(),
        ));
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
        _ => unreachable!("expected_record_type filtered V0 kinds"),
    }

    Ok(MkStateEnvelope {
        entity_id,
        community,
        version,
        previous_event_id,
        status,
        content,
    })
}

/// Validate an accepted update against the previous authoritative payload.
pub fn validate_state_transition(
    kind: u32,
    previous_content: Option<&str>,
    next: &MkStateEnvelope,
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
            return Err(MkValidationError::Transition(
                "do-not-contact cannot be cleared by an ordinary state update".to_string(),
            ));
        }
    }

    let previous_status = previous
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
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
    if content.get("schema_version").and_then(Value::as_u64) != Some(MK_SCHEMA_VERSION) {
        return Err(MkValidationError::Content(
            "`schema_version` must be 1".to_string(),
        ));
    }
    require_uuid_field(&content, "approval_id")?;
    require_uuid_field(&content, "target_id")?;
    require_uuid_field(&content, "proposal_id")?;
    let proposal_event_id = one_tag(event, "e")?;
    parse_event_id(&proposal_event_id)
        .map_err(|_| MkValidationError::Tag("`e` must identify the proposal event".to_string()))?;
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
    if content.get("schema_version").and_then(Value::as_u64) != Some(MK_SCHEMA_VERSION) {
        return Err(MkValidationError::Content(
            "`schema_version` must be 1".to_string(),
        ));
    }
    require_uuid_field(&content, "proposal_id")?;
    if content.get("proposal_version").and_then(Value::as_u64) != Some(1) {
        return Err(MkValidationError::Content(
            "`proposal_version` must be 1".to_string(),
        ));
    }
    require_uuid_field(&content, "target_id")?;
    let target_kind = content
        .get("target_kind")
        .and_then(Value::as_u64)
        .ok_or_else(|| MkValidationError::Content("`target_kind` is required".to_string()))?
        as u32;
    if !matches!(
        target_kind,
        KIND_MK_PERSON | KIND_MK_INTERVIEW | KIND_MK_CONTENT
    ) {
        return Err(MkValidationError::Content(
            "proposal target is not enabled in V0".to_string(),
        ));
    }
    let target_id = required_string(&content, "target_id")?.to_string();
    require_link_tag(event, "target", &[target_kind.to_string(), target_id])?;
    required_string(&content, "agent")?;
    required_string(&content, "proposal_type")?;
    required_string(&content, "summary")?;
    let provenance = content
        .get("provenance")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            MkValidationError::Content("`provenance` must be a non-empty array".to_string())
        })?;
    if provenance.iter().any(|item| {
        item.as_str()
            .map(str::trim)
            .is_none_or(|value| value.is_empty() || value.chars().count() > 2_000)
    }) {
        return Err(MkValidationError::Content(
            "every provenance entry must be a non-empty string under 2000 characters".to_string(),
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
}
