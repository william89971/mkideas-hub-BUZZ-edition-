use super::*;
use chrono::TimeZone;
use serde_json::json;

fn at(second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 30, 16, 0, second)
        .single()
        .expect("fixture timestamp must be valid")
}

fn uuid(value: u128) -> Uuid {
    Uuid::from_u128(value | 0x0000_0000_0000_4000_8000_0000_0000_0000)
}

fn target(kind: u32, version: u64) -> MkEntityRevision {
    MkEntityRevision {
        kind,
        entity_id: uuid(kind as u128),
        event_id: format!("{:064x}", kind as u64 + version),
        version,
    }
}

fn job(
    persona: MkAgentPersona,
    purpose: MkAgentPurpose,
    target: Option<MkEntityRevision>,
) -> MkAgentJobEnvelope {
    let primary = target.clone().unwrap_or_else(|| MkEntityRevision {
        kind: 30_802,
        entity_id: uuid(900),
        event_id: format!("{:064x}", 900),
        version: 9,
    });
    MkAgentJobEnvelope {
        schema_version: MK_AGENT_CONTRACT_SCHEMA_VERSION,
        run_id: uuid(1),
        idempotency_key: uuid(2),
        persona,
        purpose,
        community_id: uuid(3),
        request_event_id: format!("{:064x}", 4),
        requester_pubkey: format!("{:064x}", 5),
        target,
        inputs: vec![MkAgentInput {
            kind: primary.kind,
            entity_id: primary.entity_id,
            event_id: primary.event_id,
            version: primary.version,
            sha256: format!("{:064x}", 6),
        }],
        input_hash: format!("{:064x}", 7),
        attempt: 1,
        retry: None,
        requested_at: at(0),
    }
}

fn provenance() -> Vec<MkAgentProvenance> {
    vec![MkAgentProvenance {
        source_id: "fixture:source".into(),
        source_type: "synthetic_document".into(),
        title: "Synthetic source".into(),
        locator: "fixture://mkideas/source".into(),
        retrieved_at: at(2),
        sha256: format!("{:064x}", 8),
    }]
}

fn result(job: MkAgentJobEnvelope) -> MkAgentResultEnvelope {
    let informational = job.purpose == MkAgentPurpose::OperationsBriefing;
    MkAgentResultEnvelope {
        job,
        result_id: uuid(10),
        result_version: 1,
        event_kind: if informational {
            MK_GENERATED_SUMMARY_EVENT_KIND
        } else {
            MK_AGENT_PROPOSAL_EVENT_KIND
        },
        status: if informational {
            MkAgentResultStatus::Informational
        } else {
            MkAgentResultStatus::Proposed
        },
        review_state: if informational {
            None
        } else {
            Some(MkAgentReviewState::Pending)
        },
        template_version: "fixture.v2".into(),
        provider: "fixture".into(),
        model: "deterministic-local-v1".into(),
        agent_pubkey: format!("{:064x}", 11),
        provenance: provenance(),
        started_at: at(1),
        completed_at: at(3),
        stale_input: None,
        usage: Some(MkAgentUsage {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            cost_microunits: 0,
            currency: "USD".into(),
        }),
        output: json!({"summary": "deterministic fixture output"}),
    }
}

#[test]
fn validates_all_five_persona_boundaries() {
    let fixtures = [
        (
            MkAgentPersona::GuestResearcher,
            MkAgentPurpose::GuestResearch,
            Some(target(KIND_MK_PERSON, 3)),
        ),
        (
            MkAgentPersona::OutreachDrafter,
            MkAgentPurpose::OutreachDraft,
            Some(target(KIND_MK_PERSON, 4)),
        ),
        (
            MkAgentPersona::InterviewProducer,
            MkAgentPurpose::InterviewQuestions,
            Some(target(KIND_MK_INTERVIEW, 2)),
        ),
        (
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips,
            Some(target(KIND_MK_CONTENT, 5)),
        ),
        (
            MkAgentPersona::OperationsBriefingAssistant,
            MkAgentPurpose::OperationsBriefing,
            None,
        ),
    ];

    for (persona, purpose, target) in fixtures {
        let job = job(persona, purpose, target);
        assert!(validate_mk_agent_job(&job).is_ok());
        assert!(validate_mk_agent_result(&result(job)).is_ok());
    }
}

#[test]
fn rejects_wrong_persona_purpose_and_target_kind() {
    let mismatch = job(
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::OutreachDraft,
        Some(target(KIND_MK_PERSON, 1)),
    );
    assert!(matches!(
        validate_mk_agent_job(&mismatch),
        Err(MkAgentContractError::CapabilityMismatch { .. })
    ));

    let wrong_target = job(
        MkAgentPersona::ContentClipCopilot,
        MkAgentPurpose::TimestampedClips,
        Some(target(KIND_MK_PERSON, 1)),
    );
    assert!(matches!(
        validate_mk_agent_job(&wrong_target),
        Err(MkAgentContractError::TargetKindNotAllowed { .. })
    ));
}

#[test]
fn requires_exact_target_revision_in_inputs() {
    let target = target(KIND_MK_PERSON, 3);
    let mut job = job(
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::GuestResearch,
        Some(target),
    );
    job.inputs[0].version = 2;
    assert!(validate_mk_agent_job(&job).is_err());
}

#[test]
fn validates_automatic_retry_and_human_rerun_identity_rules() {
    let mut automatic = job(
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::GuestResearch,
        Some(target(KIND_MK_PERSON, 3)),
    );
    automatic.attempt = 2;
    automatic.retry = Some(MkRetryRelation {
        mode: MkRetryMode::Automatic,
        prior_run_id: automatic.run_id,
        prior_idempotency_key: automatic.idempotency_key,
        prior_attempt: 1,
    });
    assert!(validate_mk_agent_job(&automatic).is_ok());

    let mut rerun = job(
        MkAgentPersona::InterviewProducer,
        MkAgentPurpose::InterviewBrief,
        Some(target(KIND_MK_INTERVIEW, 3)),
    );
    rerun.retry = Some(MkRetryRelation {
        mode: MkRetryMode::HumanRerun,
        prior_run_id: uuid(20),
        prior_idempotency_key: uuid(21),
        prior_attempt: 3,
    });
    assert!(validate_mk_agent_job(&rerun).is_ok());

    automatic.idempotency_key = uuid(22);
    assert!(validate_mk_agent_job(&automatic).is_err());
}

#[test]
fn validates_retry_failure_cancellation_and_timeout_visibility() {
    let base = job(
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::GuestResearch,
        Some(target(KIND_MK_PERSON, 3)),
    );
    let terminal = [
        (MkAgentRunStatus::Failed, "provider_error"),
        (MkAgentRunStatus::Cancelled, "cancelled_by_requester"),
        (MkAgentRunStatus::TimedOut, "hard_timeout"),
    ];
    for (status, code) in terminal {
        let lifecycle = MkAgentLifecycle {
            job: base.clone(),
            status,
            occurred_at: at(5),
            started_at: Some(at(1)),
            completed_at: Some(at(5)),
            result_id: None,
            error: Some(MkAgentRunError {
                code: code.into(),
                message: "Sanitized fixture failure".into(),
                retryable: false,
            }),
        };
        assert!(validate_mk_agent_lifecycle(&lifecycle).is_ok());
    }

    let mut retry_job = base;
    retry_job.attempt = 2;
    retry_job.retry = Some(MkRetryRelation {
        mode: MkRetryMode::Automatic,
        prior_run_id: retry_job.run_id,
        prior_idempotency_key: retry_job.idempotency_key,
        prior_attempt: 1,
    });
    let retrying = MkAgentLifecycle {
        job: retry_job,
        status: MkAgentRunStatus::Retrying,
        occurred_at: at(4),
        started_at: Some(at(1)),
        completed_at: None,
        result_id: None,
        error: Some(MkAgentRunError {
            code: "provider_unavailable".into(),
            message: "Retryable fixture failure".into(),
            retryable: true,
        }),
    };
    assert!(validate_mk_agent_lifecycle(&retrying).is_ok());
}

#[test]
fn validates_stale_input_evidence_and_rejects_inconsistent_staleness() {
    let job = job(
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::GuestResearch,
        Some(target(KIND_MK_PERSON, 7)),
    );
    let expected = job.inputs[0].clone();
    let mut result = result(job);
    result.review_state = Some(MkAgentReviewState::Stale);
    result.stale_input = Some(MkStaleInput {
        expected_event_id: expected.event_id,
        expected_version: expected.version,
        current_event_id: format!("{:064x}", 99),
        current_version: expected.version + 1,
        detected_at: at(4),
    });
    assert!(validate_mk_agent_result(&result).is_ok());

    result
        .stale_input
        .as_mut()
        .expect("stale fixture")
        .current_version = expected.version;
    assert!(validate_mk_agent_result(&result).is_err());
}

#[test]
fn rejects_approval_and_protected_state_event_kinds() {
    let job = job(
        MkAgentPersona::GuestResearcher,
        MkAgentPurpose::GuestResearch,
        Some(target(KIND_MK_PERSON, 3)),
    );
    let mut approval = result(job.clone());
    approval.event_kind = 48_200;
    assert!(matches!(
        validate_mk_agent_result(&approval),
        Err(MkAgentContractError::ForbiddenEventKind(48_200))
    ));

    let mut protected_state = result(job);
    protected_state.event_kind = KIND_MK_PERSON;
    assert!(matches!(
        validate_mk_agent_result(&protected_state),
        Err(MkAgentContractError::ForbiddenEventKind(KIND_MK_PERSON))
    ));
}

#[test]
fn rejects_protected_actions_and_chain_of_thought_anywhere_in_output() {
    let job = job(
        MkAgentPersona::OutreachDrafter,
        MkAgentPurpose::OutreachDraft,
        Some(target(KIND_MK_PERSON, 4)),
    );
    let mut result = result(job);
    result.output = json!({
        "draft": "Unsent fixture",
        "nested": {"chain_of_thought": "hidden reasoning must never persist"}
    });
    assert_eq!(
        validate_mk_agent_result(&result),
        Err(MkAgentContractError::ForbiddenOutputKey(
            "chain_of_thought".into()
        ))
    );

    result.output = json!({"draft": "Unsent fixture", "decision": "approved"});
    assert_eq!(
        validate_mk_agent_result(&result),
        Err(MkAgentContractError::ForbiddenOutputKey("decision".into()))
    );
}

#[test]
fn informational_brief_cannot_be_published_as_a_proposal() {
    let job = job(
        MkAgentPersona::OperationsBriefingAssistant,
        MkAgentPurpose::OperationsBriefing,
        None,
    );
    let mut briefing = result(job);
    briefing.status = MkAgentResultStatus::Proposed;
    briefing.event_kind = MK_AGENT_PROPOSAL_EVENT_KIND;
    briefing.review_state = Some(MkAgentReviewState::Pending);
    assert!(validate_mk_agent_result(&briefing).is_err());
}

#[test]
fn serde_rejects_human_only_review_states_and_unknown_result_fields() {
    let payload = json!({
        "job": job(
            MkAgentPersona::GuestResearcher,
            MkAgentPurpose::GuestResearch,
            Some(target(KIND_MK_PERSON, 3))
        ),
        "result_id": uuid(10),
        "result_version": 1,
        "event_kind": MK_AGENT_PROPOSAL_EVENT_KIND,
        "status": "proposed",
        "review_state": "approved",
        "template_version": "fixture.v2",
        "provider": "fixture",
        "model": "deterministic-local-v1",
        "agent_pubkey": format!("{:064x}", 11),
        "provenance": provenance(),
        "started_at": at(1),
        "completed_at": at(3),
        "stale_input": null,
        "usage": null,
        "output": {"summary": "fixture"}
    });
    assert!(serde_json::from_value::<MkAgentResultEnvelope>(payload).is_err());
}
