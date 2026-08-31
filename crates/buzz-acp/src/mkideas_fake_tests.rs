use super::*;
use chrono::TimeZone;

const KIND_PERSON: u32 = 30_803;
const KIND_INTERVIEW: u32 = 30_804;
const KIND_CONTENT: u32 = 30_805;

fn at(second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 30, 17, 0, second)
        .single()
        .expect("fixture timestamp must be valid")
}

fn uuid(value: u128) -> Uuid {
    Uuid::from_u128(value | 0x0000_0000_0000_4000_8000_0000_0000_0000)
}

fn fixture_keys() -> Keys {
    Keys::parse(&format!("{:064x}", 1)).expect("deterministic fixture key must be valid")
}

fn persona_case(
    index: u64,
    persona: MkAgentPersona,
    purpose: MkAgentPurpose,
    target_kind: Option<u32>,
) -> MkDeterministicRequest {
    let entity_id = uuid(100 + u128::from(index));
    let event_id = format!("{:064x}", 1_000 + index);
    let version = 2 + index;
    let target = target_kind.map(|kind| MkEntityRevision {
        kind,
        entity_id,
        event_id: event_id.clone(),
        version,
    });
    let input_kind = target_kind.unwrap_or(30_802);
    let input = MkAgentInput {
        kind: input_kind,
        entity_id,
        event_id: event_id.clone(),
        version,
        sha256: format!("{:064x}", 2_000 + index),
    };
    MkDeterministicRequest {
        job: MkAgentJobEnvelope {
            schema_version: 2,
            run_id: uuid(10 + u128::from(index)),
            idempotency_key: uuid(20 + u128::from(index)),
            persona,
            purpose,
            community_id: uuid(30),
            request_event_id: format!("{:064x}", 3_000 + index),
            requester_pubkey: format!("{:064x}", 4_000 + index),
            target,
            inputs: vec![input],
            input_hash: format!("{:064x}", 5_000 + index),
            attempt: 1,
            retry: None,
            requested_at: at(index as u32),
        },
        observed_heads: vec![MkEntityRevision {
            kind: input_kind,
            entity_id,
            event_id,
            version,
        }],
        scenario: MkDeterministicScenario::Success,
    }
}

fn all_persona_cases() -> Vec<MkDeterministicRequest> {
    vec![
        persona_case(
            1,
            MkAgentPersona::GuestResearcher,
            MkAgentPurpose::GuestResearch,
            Some(KIND_PERSON),
        ),
        persona_case(
            2,
            MkAgentPersona::OutreachDrafter,
            MkAgentPurpose::OutreachDraft,
            Some(KIND_PERSON),
        ),
        persona_case(
            3,
            MkAgentPersona::InterviewProducer,
            MkAgentPurpose::InterviewQuestions,
            Some(KIND_INTERVIEW),
        ),
        persona_case(
            4,
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips,
            Some(KIND_CONTENT),
        ),
        persona_case(
            5,
            MkAgentPersona::OperationsBriefingAssistant,
            MkAgentPurpose::OperationsBriefing,
            None,
        ),
    ]
}

fn event_content(event: &Event) -> Value {
    serde_json::from_str(&event.content).expect("signed fixture content must be JSON")
}

fn assert_required_top_level(content: &Value, fields: &[&str]) {
    for field in fields {
        assert!(
            content.get(field).is_some(),
            "missing canonical top-level field `{field}` in {content}"
        );
    }
    assert!(content.get("job").is_none());
    assert!(content.get("lifecycle").is_none());
}

fn assert_no_protected_wire_fields(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "approval",
        "approval_action",
        "approved",
        "chain_of_thought",
        "decision",
        "mutation",
        "publish",
        "send_external_communication",
        "state_event",
        "state_patch",
        "status_transition",
    ];
    match value {
        Value::Object(fields) => {
            for (key, nested) in fields {
                assert!(
                    !FORBIDDEN.contains(&key.as_str()),
                    "forbidden wire key {key}"
                );
                assert_no_protected_wire_fields(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_no_protected_wire_fields(item);
            }
        }
        _ => {}
    }
}

#[test]
fn produces_signed_results_for_all_five_personas_without_credentials_or_network() {
    let keys = fixture_keys();
    let expected_pubkey = keys.public_key();
    let mut runner = MkDeterministicRunner::new(keys);

    for request in all_persona_cases() {
        let persona = request.job.persona;
        let execution = runner
            .execute(request)
            .expect("fixture execution must pass");
        let result = execution.result.expect("success must produce a result");
        let expected_kind = if persona == MkAgentPersona::OperationsBriefingAssistant {
            MK_GENERATED_SUMMARY_EVENT_KIND
        } else {
            MK_AGENT_PROPOSAL_EVENT_KIND
        };
        assert_eq!(result.event_kind, expected_kind);
        assert_eq!(result.provider, MK_DETERMINISTIC_PROVIDER);
        assert_eq!(result.model, MK_DETERMINISTIC_MODEL);
        assert_eq!(result.usage.as_ref().expect("usage").cost_microunits, 0);
        assert!(!result.provenance.is_empty());
        assert!(result.output.get("fixture_only").is_some());
        assert!(!result.output.to_string().contains("chain_of_thought"));

        for event in &execution.signed_events {
            event.verify().expect("signed fixture event must verify");
            assert_eq!(event.pubkey, expected_pubkey);
            assert!(matches!(event.kind.as_u16(), 48_201 | 48_203 | 48_204));
        }
        assert_eq!(
            execution
                .signed_events
                .iter()
                .filter(|event| event.kind.as_u16() == expected_kind as u16)
                .count(),
            1
        );
    }
}

#[test]
fn signed_events_use_canonical_flat_v2_wire_payloads() {
    let mut runner = MkDeterministicRunner::new(fixture_keys());

    for request in all_persona_cases() {
        let expected_persona = enum_name(&request.job.persona).expect("persona name");
        let expected_purpose = enum_name(&request.job.purpose).expect("purpose name");
        let expected_target = request.job.target.clone();
        let execution = runner.execute(request).expect("fixture execution");

        for event in &execution.signed_events {
            let content = event_content(event);
            assert_eq!(content["schema_version"], 2);
            assert_eq!(content["persona"], expected_persona);
            assert_eq!(content["persona_id"], expected_persona);
            assert_eq!(content["purpose"], expected_purpose);
            assert_no_protected_wire_fields(&content);

            match u32::from(event.kind.as_u16()) {
                MK_AGENT_ACTIVITY_EVENT_KIND => {
                    assert_required_top_level(
                        &content,
                        &[
                            "activity_type",
                            "run_id",
                            "idempotency_key",
                            "persona",
                            "persona_id",
                            "agent",
                            "agent_pubkey",
                            "target",
                            "status",
                            "attempt",
                            "requested_at",
                            "occurred_at",
                            "timestamps",
                            "result_id",
                            "result",
                            "error",
                        ],
                    );
                    assert_eq!(content["activity_type"], "agent_run");
                    assert!(content.get("output").is_none());
                }
                MK_AGENT_PROPOSAL_EVENT_KIND => {
                    assert_required_top_level(
                        &content,
                        &[
                            "proposal_id",
                            "proposal_version",
                            "run_id",
                            "idempotency_key",
                            "persona",
                            "persona_id",
                            "agent",
                            "agent_pubkey",
                            "purpose",
                            "proposal_type",
                            "target_id",
                            "target_kind",
                            "target_event_id",
                            "target_version",
                            "target",
                            "input_event_ids",
                            "input_hash",
                            "template_version",
                            "provider",
                            "model",
                            "provenance",
                            "summary",
                            "output",
                            "status",
                            "review_state",
                            "review",
                            "timestamps",
                            "usage",
                            "stale",
                            "stale_input",
                        ],
                    );
                    assert_eq!(content["proposal_version"], 1);
                    assert_eq!(content["proposal_type"], expected_purpose);
                    assert_eq!(content["status"], "proposed");
                    assert_eq!(content["review"]["human_action_required"], true);
                    assert!(content["provenance"][0].get("source_id").is_some());
                    assert!(content["input_event_ids"]
                        .as_array()
                        .is_some_and(|ids| !ids.is_empty()));
                    let target = expected_target.as_ref().expect("proposal target");
                    assert_eq!(content["target_id"], target.entity_id.to_string());
                    assert_eq!(content["target"]["event_id"], target.event_id);
                    let target_tag = event
                        .tags
                        .iter()
                        .find(|tag| tag.as_slice().first().map(String::as_str) == Some("target"))
                        .expect("canonical target tag");
                    assert_eq!(
                        target_tag.as_slice(),
                        &[
                            "target".to_string(),
                            target.kind.to_string(),
                            target.entity_id.to_string(),
                        ]
                    );
                }
                MK_GENERATED_SUMMARY_EVENT_KIND => {
                    assert_required_top_level(
                        &content,
                        &[
                            "summary_id",
                            "summary_version",
                            "run_id",
                            "idempotency_key",
                            "persona",
                            "persona_id",
                            "summary",
                            "output",
                            "provenance",
                            "status",
                            "timestamps",
                            "usage",
                            "stale",
                        ],
                    );
                    assert_eq!(content["status"], "informational");
                    assert!(content["provenance"][0].get("source_id").is_some());
                    assert!(content.get("proposal_id").is_none());
                    assert!(content.get("review").is_none());
                }
                kind => panic!("unexpected signed event kind {kind}"),
            }
        }
    }
}

#[test]
fn retry_once_preserves_run_and_idempotency_ids_and_increments_attempt() {
    let mut request = persona_case(
        6,
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::GuestResearch,
        Some(KIND_PERSON),
    );
    request.scenario = MkDeterministicScenario::RetryOnce;
    let original_run_id = request.job.run_id;
    let original_idempotency = request.job.idempotency_key;
    let mut runner = MkDeterministicRunner::new(fixture_keys());
    let execution = runner.execute(request).expect("retry fixture must pass");

    assert_eq!(
        execution
            .lifecycle
            .iter()
            .map(|item| item.status.clone())
            .collect::<Vec<_>>(),
        vec![
            MkAgentRunStatus::Queued,
            MkAgentRunStatus::Running,
            MkAgentRunStatus::Retrying,
            MkAgentRunStatus::Running,
            MkAgentRunStatus::Succeeded,
        ]
    );
    let retrying = &execution.lifecycle[2].job;
    assert_eq!(retrying.attempt, 2);
    assert_eq!(retrying.run_id, original_run_id);
    assert_eq!(retrying.idempotency_key, original_idempotency);
    assert_eq!(execution.result.expect("retry result").job.attempt, 2);
}

#[test]
fn idempotent_replay_returns_identical_signed_events_and_conflicts_fail_closed() {
    let request = persona_case(
        7,
        MkAgentPersona::InterviewProducer,
        MkAgentPurpose::InterviewBrief,
        Some(KIND_INTERVIEW),
    );
    let mut runner = MkDeterministicRunner::new(fixture_keys());
    let first = runner
        .execute(request.clone())
        .expect("initial fixture execution");
    let replay = runner.execute(request.clone()).expect("idempotent replay");
    assert_eq!(first.request_fingerprint, replay.request_fingerprint);
    assert_eq!(
        first
            .signed_events
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>(),
        replay
            .signed_events
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>()
    );

    let mut conflicting = request;
    conflicting.job.input_hash = format!("{:064x}", 9_999);
    assert!(matches!(
        runner.execute(conflicting),
        Err(MkDeterministicError::IdempotencyConflict(_))
    ));
}

#[test]
fn newer_observed_head_marks_proposal_stale_and_prevents_operations_summary() {
    let mut proposal_request = persona_case(
        8,
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::GuestResearch,
        Some(KIND_PERSON),
    );
    proposal_request.observed_heads[0].version += 1;
    proposal_request.observed_heads[0].event_id = format!("{:064x}", 8_888);
    let mut runner = MkDeterministicRunner::new(fixture_keys());
    let proposal = runner
        .execute(proposal_request)
        .expect("stale proposal fixture");
    assert!(proposal.stale);
    let result = proposal.result.expect("stale proposal remains visible");
    assert_eq!(result.review_state, Some(MkAgentReviewState::Stale));
    assert!(result.stale_input.is_some());

    let mut summary_request = persona_case(
        9,
        MkAgentPersona::OperationsBriefingAssistant,
        MkAgentPurpose::OperationsBriefing,
        None,
    );
    summary_request.observed_heads[0].version += 1;
    summary_request.observed_heads[0].event_id = format!("{:064x}", 9_999);
    let summary = runner
        .execute(summary_request)
        .expect("stale summary fixture must fail visibly");
    assert!(summary.stale);
    assert!(summary.result.is_none());
    assert_eq!(
        summary.lifecycle.last().expect("terminal lifecycle").status,
        MkAgentRunStatus::Failed
    );
}

#[test]
fn cancellation_timeout_and_failure_emit_lifecycle_only() {
    let scenarios = [
        (MkDeterministicScenario::Failure, MkAgentRunStatus::Failed),
        (
            MkDeterministicScenario::Cancelled,
            MkAgentRunStatus::Cancelled,
        ),
        (
            MkDeterministicScenario::TimedOut,
            MkAgentRunStatus::TimedOut,
        ),
    ];
    let mut runner = MkDeterministicRunner::new(fixture_keys());
    for (index, (scenario, status)) in scenarios.into_iter().enumerate() {
        let mut request = persona_case(
            20 + index as u64,
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips,
            Some(KIND_INTERVIEW),
        );
        request.scenario = scenario;
        let execution = runner.execute(request).expect("terminal fixture");
        assert!(execution.result.is_none());
        assert_eq!(
            execution.lifecycle.last().expect("terminal state").status,
            status
        );
        assert!(execution
            .signed_events
            .iter()
            .all(|event| event.kind.as_u16() == MK_AGENT_ACTIVITY_EVENT_KIND as u16));
    }
}

#[test]
fn missing_or_conflicting_heads_fail_before_generation() {
    let mut missing = persona_case(
        30,
        MkAgentPersona::OutreachDrafter,
        MkAgentPurpose::OutreachDraft,
        Some(KIND_PERSON),
    );
    missing.observed_heads.clear();
    let mut runner = MkDeterministicRunner::new(fixture_keys());
    assert!(matches!(
        runner.execute(missing),
        Err(MkDeterministicError::MissingObservedHead { .. })
    ));

    let mut conflict = persona_case(
        31,
        MkAgentPersona::OutreachDrafter,
        MkAgentPurpose::OutreachDraft,
        Some(KIND_PERSON),
    );
    conflict.observed_heads[0].event_id = format!("{:064x}", 31_313);
    assert!(matches!(
        runner.execute(conflict),
        Err(MkDeterministicError::ObservedHeadConflict { .. })
    ));
}
