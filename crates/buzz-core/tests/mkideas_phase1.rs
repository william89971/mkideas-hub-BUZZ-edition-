//! Pure contract tests for the NIP-MK v2 state and media schemas.

use buzz_core::{
    kind::{
        KIND_MK_APPROVAL, KIND_MK_CONTENT, KIND_MK_DECISION, KIND_MK_GOAL, KIND_MK_INTERVIEW,
        KIND_MK_KNOWLEDGE, KIND_MK_MEETING, KIND_MK_OPERATIONAL_PROJECT, KIND_MK_PERSON,
        KIND_MK_TASK,
    },
    mkideas::{validate_state_event, validate_state_transition},
};
use nostr::{Event, EventBuilder, Keys, Kind, Tag};
use serde_json::{json, Map, Value};
use uuid::Uuid;

fn tag(values: impl IntoIterator<Item = String>) -> Tag {
    Tag::parse(values).expect("valid test tag")
}

fn merge(mut base: Value, extra: Value) -> Value {
    let base = base.as_object_mut().expect("base object");
    for (key, value) in extra.as_object().expect("extra object") {
        base.insert(key.clone(), value.clone());
    }
    Value::Object(std::mem::take(base))
}

fn record_type(kind: u32) -> &'static str {
    match kind {
        KIND_MK_GOAL => "goal",
        KIND_MK_OPERATIONAL_PROJECT => "operational_project",
        KIND_MK_TASK => "task",
        KIND_MK_PERSON => "person",
        KIND_MK_INTERVIEW => "interview",
        KIND_MK_CONTENT => "content",
        KIND_MK_MEETING => "meeting",
        KIND_MK_DECISION => "decision",
        KIND_MK_KNOWLEDGE => "knowledge",
        KIND_MK_APPROVAL => "approval",
        _ => panic!("unsupported fixture kind {kind}"),
    }
}

fn v2_event(
    kind: u32,
    entity_id: Uuid,
    version: i64,
    previous: Option<&Event>,
    status: &str,
    extra: Value,
    extra_tags: impl IntoIterator<Item = Vec<String>>,
) -> Event {
    let content = merge(
        json!({
            "schema_version": 2,
            "record_type": record_type(kind),
            "entity_id": entity_id,
            "version": version,
            "status": status,
            "source": "phase1-contract-test",
            "provenance": {"type": "test", "ref": entity_id}
        }),
        extra,
    );
    let mut tags = vec![
        vec!["d".to_string(), entity_id.to_string()],
        vec!["h".to_string(), "phase1.mkideas.test".to_string()],
        vec!["version".to_string(), version.to_string()],
        vec!["status".to_string(), status.to_string()],
    ];
    if let Some(previous) = previous {
        tags.push(vec!["prev".to_string(), previous.id.to_hex()]);
    }
    tags.extend(extra_tags);
    EventBuilder::new(Kind::from(kind as u16), content.to_string())
        .tags(tags.into_iter().map(tag))
        .sign_with_keys(&Keys::generate())
        .expect("sign v2 state event")
}

fn entity_fixture(
    kind: u32,
    entity_id: Uuid,
    version: i64,
    previous: Option<&Event>,
    status: &str,
) -> Event {
    match kind {
        KIND_MK_GOAL => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"title": "Goal"}),
            [],
        ),
        KIND_MK_OPERATIONAL_PROJECT => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"title": "Project"}),
            [],
        ),
        KIND_MK_TASK => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"title": "Task"}),
            [],
        ),
        KIND_MK_PERSON => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"name": "Guest", "do_not_contact": false}),
            [],
        ),
        KIND_MK_INTERVIEW => {
            let guest_id = Uuid::from_u128(44);
            v2_event(
                kind,
                entity_id,
                version,
                previous,
                status,
                json!({"title": "Interview", "guest_id": guest_id}),
                [vec!["guest".to_string(), guest_id.to_string()]],
            )
        }
        KIND_MK_CONTENT => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"title": "Content"}),
            [],
        ),
        KIND_MK_MEETING => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"title": "Meeting"}),
            [],
        ),
        KIND_MK_DECISION => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"title": "Decision"}),
            [],
        ),
        KIND_MK_KNOWLEDGE => v2_event(
            kind,
            entity_id,
            version,
            previous,
            status,
            json!({"title": "Knowledge"}),
            [],
        ),
        _ => panic!("unsupported fixture kind {kind}"),
    }
}

fn approval_fixture(
    entity_id: Uuid,
    version: i64,
    previous: Option<&Event>,
    status: &str,
) -> Event {
    let target_id = Uuid::from_u128(100);
    let proposal_id = Uuid::from_u128(101);
    let target_event_id = "11".repeat(32);
    let proposal_event_id = "22".repeat(32);
    v2_event(
        KIND_MK_APPROVAL,
        entity_id,
        version,
        previous,
        status,
        json!({
            "target_id": target_id,
            "target_kind": KIND_MK_CONTENT,
            "target_event_id": target_event_id,
            "target_version": 1,
            "proposal_id": proposal_id,
            "proposal_event_id": proposal_event_id
        }),
        [
            vec![
                "target".to_string(),
                KIND_MK_CONTENT.to_string(),
                target_id.to_string(),
                target_event_id,
            ],
            vec![
                "proposal".to_string(),
                proposal_id.to_string(),
                proposal_event_id,
            ],
        ],
    )
}

fn assert_transition(kind: u32, from: &str, to: &str, legal: bool) {
    let entity_id = Uuid::new_v4();
    let previous = entity_fixture(kind, entity_id, 1, None, from);
    validate_state_event(&previous).expect("valid previous fixture");
    let next = entity_fixture(kind, entity_id, 2, Some(&previous), to);
    let envelope = validate_state_event(&next).expect("valid next fixture");
    assert_eq!(
        validate_state_transition(kind, Some(&previous.content), &envelope).is_ok(),
        legal,
        "unexpected {from} -> {to} result for kind {kind}"
    );
}

#[test]
fn every_v2_entity_transition_table_accepts_forward_flow_and_rejects_rewind() {
    for (kind, forward_from, forward_to, rewind_from, rewind_to) in [
        (KIND_MK_GOAL, "draft", "active", "active", "draft"),
        (
            KIND_MK_OPERATIONAL_PROJECT,
            "planned",
            "active",
            "active",
            "planned",
        ),
        (KIND_MK_TASK, "backlog", "to-do", "done", "in-progress"),
        (
            KIND_MK_PERSON,
            "prospect",
            "researching",
            "researching",
            "prospect",
        ),
        (KIND_MK_INTERVIEW, "idea", "planning", "planning", "idea"),
        (KIND_MK_CONTENT, "idea", "draft", "draft", "idea"),
        (
            KIND_MK_MEETING,
            "planned",
            "completed",
            "completed",
            "planned",
        ),
        (
            KIND_MK_DECISION,
            "proposed",
            "decided",
            "decided",
            "proposed",
        ),
        (KIND_MK_KNOWLEDGE, "draft", "verified", "archived", "draft"),
    ] {
        assert_transition(kind, forward_from, forward_to, true);
        assert_transition(kind, rewind_from, rewind_to, false);
    }

    let approval_id = Uuid::new_v4();
    let pending = approval_fixture(approval_id, 1, None, "pending");
    let approved = approval_fixture(approval_id, 2, Some(&pending), "approved");
    let approved_envelope = validate_state_event(&approved).expect("valid approved state");
    assert!(validate_state_transition(
        KIND_MK_APPROVAL,
        Some(&pending.content),
        &approved_envelope,
    )
    .is_ok());
    let reopened = approval_fixture(approval_id, 3, Some(&approved), "pending");
    let reopened_envelope = validate_state_event(&reopened).expect("valid pending schema");
    assert!(validate_state_transition(
        KIND_MK_APPROVAL,
        Some(&approved.content),
        &reopened_envelope,
    )
    .is_err());
}

#[test]
fn interview_transcript_requires_an_immutable_private_media_descriptor() {
    let interview_id = Uuid::new_v4();
    let guest_id = Uuid::new_v4();
    let raw_transcript = v2_event(
        KIND_MK_INTERVIEW,
        interview_id,
        1,
        None,
        "recorded",
        json!({
            "title": "Interview",
            "guest_id": guest_id,
            "transcript_text": "Large transcript text must not live in a Nostr event."
        }),
        [vec!["guest".to_string(), guest_id.to_string()]],
    );
    assert!(validate_state_event(&raw_transcript).is_err());

    let uploader = Keys::generate().public_key().to_hex();
    let descriptor = json!({
        "media_id": Uuid::new_v4(),
        "sha256": "ab".repeat(32),
        "mime_type": "text/vtt",
        "size": 8192,
        "filename": "interview.vtt",
        "uploaded_by": uploader,
        "uploaded_at": "2026-08-30T00:00:00Z",
        "source": "private-media",
        "format": "webvtt",
        "duration": "00:42:10",
        "language": "en",
        "version": 1
    });
    let referenced_transcript = v2_event(
        KIND_MK_INTERVIEW,
        interview_id,
        1,
        None,
        "recorded",
        json!({
            "title": "Interview",
            "guest_id": guest_id,
            "transcript": descriptor
        }),
        [vec!["guest".to_string(), guest_id.to_string()]],
    );
    validate_state_event(&referenced_transcript).expect("valid private transcript descriptor");

    let mut invalid_descriptor = serde_json::from_str::<Value>(&referenced_transcript.content)
        .expect("parse fixture content");
    invalid_descriptor["transcript"]["sha256"] = Value::String("not-a-digest".to_string());
    let invalid_digest = EventBuilder::new(
        Kind::from(KIND_MK_INTERVIEW as u16),
        invalid_descriptor.to_string(),
    )
    .tags(referenced_transcript.tags.clone())
    .sign_with_keys(&Keys::generate())
    .expect("sign invalid digest event");
    assert!(validate_state_event(&invalid_digest).is_err());
}

#[test]
fn schema_v1_is_readable_but_new_v1_only_entity_kinds_are_rejected() {
    let entity_id = Uuid::new_v4();
    let v1_person = EventBuilder::new(
        Kind::from(KIND_MK_PERSON as u16),
        json!({
            "schema_version": 1,
            "record_type": "person",
            "entity_id": entity_id,
            "version": 1,
            "status": "potential",
            "name": "Legacy guest",
            "do_not_contact": false
        })
        .to_string(),
    )
    .tags([
        tag(vec!["d".to_string(), entity_id.to_string()]),
        tag(vec!["h".to_string(), "phase1.mkideas.test".to_string()]),
        tag(vec!["version".to_string(), "1".to_string()]),
        tag(vec!["status".to_string(), "potential".to_string()]),
    ])
    .sign_with_keys(&Keys::generate())
    .expect("sign legacy person");
    assert_eq!(
        validate_state_event(&v1_person)
            .expect("schema v1 person remains readable")
            .schema_version,
        1
    );

    let mut task_content = Map::new();
    task_content.insert("schema_version".to_string(), json!(1));
    task_content.insert("record_type".to_string(), json!("task"));
    task_content.insert("entity_id".to_string(), json!(entity_id));
    task_content.insert("version".to_string(), json!(1));
    task_content.insert("status".to_string(), json!("backlog"));
    task_content.insert("title".to_string(), json!("Legacy task"));
    let v1_task = EventBuilder::new(
        Kind::from(KIND_MK_TASK as u16),
        Value::Object(task_content).to_string(),
    )
    .tags(v1_person.tags.clone())
    .sign_with_keys(&Keys::generate())
    .expect("sign unsupported legacy task");
    assert!(validate_state_event(&v1_task).is_err());
}
