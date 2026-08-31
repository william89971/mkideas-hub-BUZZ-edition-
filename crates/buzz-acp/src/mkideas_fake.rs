//! Deterministic, offline execution path for MK Ideas agent integration tests.
//!
//! The provider performs no network or external actions. It turns an already
//! validated [`MkAgentJobEnvelope`](crate::mkideas::MkAgentJobEnvelope) into
//! lifecycle records, a human-gated proposal or informational summary, and
//! signed Nostr fixture events suitable for relay integration tests.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use nostr::{Event, EventBuilder, Keys, Kind, Tag, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::mkideas::{
    validate_mk_agent_job, validate_mk_agent_lifecycle, validate_mk_agent_result,
    MkAgentContractError, MkAgentInput, MkAgentJobEnvelope, MkAgentLifecycle, MkAgentPersona,
    MkAgentProvenance, MkAgentPurpose, MkAgentResultEnvelope, MkAgentResultStatus,
    MkAgentReviewState, MkAgentRunError, MkAgentRunStatus, MkAgentUsage, MkEntityRevision,
    MkRetryMode, MkRetryRelation, MkStaleInput, MK_AGENT_ACTIVITY_EVENT_KIND,
    MK_AGENT_PROPOSAL_EVENT_KIND, MK_GENERATED_SUMMARY_EVENT_KIND,
};

/// Stable provider identifier used by the deterministic runner.
pub const MK_DETERMINISTIC_PROVIDER: &str = "fixture";

/// Stable model identifier used by the deterministic runner.
pub const MK_DETERMINISTIC_MODEL: &str = "mkideas-deterministic-v2";

/// An execution failure before a fixture can be safely signed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MkDeterministicError {
    /// The typed MK contract rejected a job, lifecycle event, or result.
    #[error(transparent)]
    Contract(#[from] MkAgentContractError),

    /// A required current head observation is absent.
    #[error("missing observed head for kind {kind} entity {entity_id}")]
    MissingObservedHead {
        /// Event kind of the missing input head.
        kind: u32,
        /// Stable entity UUID of the missing input head.
        entity_id: Uuid,
    },

    /// The observed revision conflicts with, rather than advances, an input.
    #[error("observed head conflicts with input for kind {kind} entity {entity_id}")]
    ObservedHeadConflict {
        /// Event kind of the conflicting input.
        kind: u32,
        /// Stable entity UUID of the conflicting input.
        entity_id: Uuid,
    },

    /// An idempotency key was reused with a different request payload.
    #[error("idempotency key {0} was reused with a different deterministic request")]
    IdempotencyConflict(Uuid),

    /// Fixture serialization or fingerprinting failed.
    #[error("fixture serialization failed: {0}")]
    Serialization(String),

    /// A signed fixture event could not be constructed.
    #[error("fixture event signing failed: {0}")]
    Signing(String),

    /// A deterministic timestamp could not be represented safely.
    #[error("fixture timestamp is out of range")]
    TimestampOutOfRange,
}

/// Deterministic execution branch used to exercise runner outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MkDeterministicScenario {
    /// Produce a proposal or informational summary immediately.
    Success,
    /// Emit a retryable failure, retry once, then produce a result.
    RetryOnce,
    /// End in a sanitized terminal provider failure.
    Failure,
    /// End in an explicit cancellation without a result.
    Cancelled,
    /// End in a hard timeout without a result.
    TimedOut,
}

/// Offline request supplied to the deterministic execution path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkDeterministicRequest {
    /// Validated human-requested job envelope.
    pub job: MkAgentJobEnvelope,
    /// Current entity heads observed immediately before execution.
    pub observed_heads: Vec<MkEntityRevision>,
    /// Deterministic lifecycle branch to exercise.
    pub scenario: MkDeterministicScenario,
}

/// Complete deterministic execution and its signed relay fixtures.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MkDeterministicExecution {
    /// Validated lifecycle sequence in display order.
    pub lifecycle: Vec<MkAgentLifecycle>,
    /// Proposal or informational summary, absent on unsuccessful runs.
    pub result: Option<MkAgentResultEnvelope>,
    /// Signed kind 48204 lifecycle events plus an optional 48201/48203 result.
    pub signed_events: Vec<Event>,
    /// True only when a result was marked stale against an observed newer head.
    pub stale: bool,
    /// SHA-256 fingerprint used to enforce deterministic idempotency.
    pub request_fingerprint: String,
}

#[derive(Debug, Clone)]
struct CachedExecution {
    fingerprint: String,
    execution: MkDeterministicExecution,
}

/// Credential-free, no-network MK Ideas fixture provider and event signer.
#[derive(Debug)]
pub struct MkDeterministicRunner {
    keys: Keys,
    cache: HashMap<Uuid, CachedExecution>,
}

impl MkDeterministicRunner {
    /// Create a runner using the service identity that will sign fixture events.
    pub fn new(keys: Keys) -> Self {
        Self {
            keys,
            cache: HashMap::new(),
        }
    }

    /// Return the public service identity used to sign fixture events.
    pub fn service_pubkey(&self) -> String {
        self.keys.public_key().to_hex()
    }

    /// Execute a deterministic request, returning the cached signed events on replay.
    pub fn execute(
        &mut self,
        request: MkDeterministicRequest,
    ) -> Result<MkDeterministicExecution, MkDeterministicError> {
        validate_mk_agent_job(&request.job)?;
        let fingerprint = request_fingerprint(&request)?;
        if let Some(cached) = self.cache.get(&request.job.idempotency_key) {
            if cached.fingerprint != fingerprint {
                return Err(MkDeterministicError::IdempotencyConflict(
                    request.job.idempotency_key,
                ));
            }
            return Ok(cached.execution.clone());
        }

        let execution = self.execute_uncached(&request, fingerprint.clone())?;
        self.cache.insert(
            request.job.idempotency_key,
            CachedExecution {
                fingerprint,
                execution: execution.clone(),
            },
        );
        Ok(execution)
    }

    fn execute_uncached(
        &self,
        request: &MkDeterministicRequest,
        fingerprint: String,
    ) -> Result<MkDeterministicExecution, MkDeterministicError> {
        let stale_input = detect_stale_input(&request.job, &request.observed_heads)?;
        let initial_job = request.job.clone();
        let queued_at = initial_job.requested_at;
        let first_started_at = plus_seconds(queued_at, 1)?;
        let first_finished_at = plus_seconds(queued_at, 2)?;

        let mut lifecycle = vec![
            make_lifecycle(
                initial_job.clone(),
                MkAgentRunStatus::Queued,
                queued_at,
                None,
                None,
                None,
                None,
            )?,
            make_lifecycle(
                initial_job.clone(),
                MkAgentRunStatus::Running,
                first_started_at,
                Some(first_started_at),
                None,
                None,
                None,
            )?,
        ];

        let (result, stale) = match request.scenario {
            MkDeterministicScenario::Success => {
                if stale_input.is_some()
                    && initial_job.purpose == MkAgentPurpose::OperationsBriefing
                {
                    lifecycle.push(make_terminal_error(
                        initial_job,
                        MkAgentRunStatus::Failed,
                        first_started_at,
                        first_finished_at,
                        "input_stale",
                        "The operations snapshot changed before the fixture summary completed.",
                    )?);
                    (None, true)
                } else {
                    let result = make_result(
                        &self.keys,
                        initial_job.clone(),
                        first_started_at,
                        first_finished_at,
                        stale_input,
                    )?;
                    lifecycle.push(make_lifecycle(
                        initial_job,
                        MkAgentRunStatus::Succeeded,
                        first_finished_at,
                        Some(first_started_at),
                        Some(first_finished_at),
                        Some(result.result_id),
                        None,
                    )?);
                    let stale = result.review_state == Some(MkAgentReviewState::Stale);
                    (Some(result), stale)
                }
            }
            MkDeterministicScenario::RetryOnce => {
                let retry_started_at = plus_seconds(queued_at, 3)?;
                let retry_finished_at = plus_seconds(queued_at, 4)?;
                let retry_job = automatic_retry_job(&initial_job)?;
                lifecycle.push(make_lifecycle(
                    retry_job.clone(),
                    MkAgentRunStatus::Retrying,
                    first_finished_at,
                    Some(first_started_at),
                    None,
                    None,
                    Some(MkAgentRunError {
                        code: "fixture_retryable_failure".into(),
                        message: "The deterministic provider requested one retry.".into(),
                        retryable: true,
                    }),
                )?);
                lifecycle.push(make_lifecycle(
                    retry_job.clone(),
                    MkAgentRunStatus::Running,
                    retry_started_at,
                    Some(retry_started_at),
                    None,
                    None,
                    None,
                )?);

                if stale_input.is_some() && retry_job.purpose == MkAgentPurpose::OperationsBriefing
                {
                    lifecycle.push(make_terminal_error(
                        retry_job,
                        MkAgentRunStatus::Failed,
                        retry_started_at,
                        retry_finished_at,
                        "input_stale",
                        "The operations snapshot changed before the retried fixture summary completed.",
                    )?);
                    (None, true)
                } else {
                    let result = make_result(
                        &self.keys,
                        retry_job.clone(),
                        retry_started_at,
                        retry_finished_at,
                        stale_input,
                    )?;
                    lifecycle.push(make_lifecycle(
                        retry_job,
                        MkAgentRunStatus::Succeeded,
                        retry_finished_at,
                        Some(retry_started_at),
                        Some(retry_finished_at),
                        Some(result.result_id),
                        None,
                    )?);
                    let stale = result.review_state == Some(MkAgentReviewState::Stale);
                    (Some(result), stale)
                }
            }
            MkDeterministicScenario::Failure => {
                lifecycle.push(make_terminal_error(
                    initial_job,
                    MkAgentRunStatus::Failed,
                    first_started_at,
                    first_finished_at,
                    "fixture_provider_failure",
                    "The deterministic provider returned a sanitized fixture failure.",
                )?);
                (None, stale_input.is_some())
            }
            MkDeterministicScenario::Cancelled => {
                lifecycle.push(make_terminal_error(
                    initial_job,
                    MkAgentRunStatus::Cancelled,
                    first_started_at,
                    first_finished_at,
                    "cancelled_by_requester",
                    "The requesting human cancelled the deterministic run.",
                )?);
                (None, stale_input.is_some())
            }
            MkDeterministicScenario::TimedOut => {
                lifecycle.push(make_terminal_error(
                    initial_job,
                    MkAgentRunStatus::TimedOut,
                    first_started_at,
                    first_finished_at,
                    "hard_timeout",
                    "The deterministic run exceeded its configured fixture timeout.",
                )?);
                (None, stale_input.is_some())
            }
        };

        let signed_events = sign_execution(&self.keys, &lifecycle, result.as_ref())?;
        Ok(MkDeterministicExecution {
            lifecycle,
            result,
            signed_events,
            stale,
            request_fingerprint: fingerprint,
        })
    }
}

fn request_fingerprint(request: &MkDeterministicRequest) -> Result<String, MkDeterministicError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| MkDeterministicError::Serialization(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn detect_stale_input(
    job: &MkAgentJobEnvelope,
    observed_heads: &[MkEntityRevision],
) -> Result<Option<MkStaleInput>, MkDeterministicError> {
    let mut first_stale = None;
    for input in &job.inputs {
        let observed = observed_heads
            .iter()
            .find(|head| head.kind == input.kind && head.entity_id == input.entity_id)
            .ok_or(MkDeterministicError::MissingObservedHead {
                kind: input.kind,
                entity_id: input.entity_id,
            })?;

        if observed.version < input.version
            || (observed.version == input.version && observed.event_id != input.event_id)
        {
            return Err(MkDeterministicError::ObservedHeadConflict {
                kind: input.kind,
                entity_id: input.entity_id,
            });
        }
        if observed.version > input.version && first_stale.is_none() {
            first_stale = Some(MkStaleInput {
                expected_event_id: input.event_id.clone(),
                expected_version: input.version,
                current_event_id: observed.event_id.clone(),
                current_version: observed.version,
                detected_at: plus_seconds(job.requested_at, 2)?,
            });
        }
    }
    Ok(first_stale)
}

fn automatic_retry_job(
    initial: &MkAgentJobEnvelope,
) -> Result<MkAgentJobEnvelope, MkDeterministicError> {
    let mut retry = initial.clone();
    retry.attempt = retry
        .attempt
        .checked_add(1)
        .ok_or(MkDeterministicError::TimestampOutOfRange)?;
    retry.retry = Some(MkRetryRelation {
        mode: MkRetryMode::Automatic,
        prior_run_id: initial.run_id,
        prior_idempotency_key: initial.idempotency_key,
        prior_attempt: initial.attempt,
    });
    validate_mk_agent_job(&retry)?;
    Ok(retry)
}

#[allow(clippy::too_many_arguments)]
fn make_lifecycle(
    job: MkAgentJobEnvelope,
    status: MkAgentRunStatus,
    occurred_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    result_id: Option<Uuid>,
    error: Option<MkAgentRunError>,
) -> Result<MkAgentLifecycle, MkDeterministicError> {
    let lifecycle = MkAgentLifecycle {
        job,
        status,
        occurred_at,
        started_at,
        completed_at,
        result_id,
        error,
    };
    validate_mk_agent_lifecycle(&lifecycle)?;
    Ok(lifecycle)
}

fn make_terminal_error(
    job: MkAgentJobEnvelope,
    status: MkAgentRunStatus,
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
    code: &str,
    message: &str,
) -> Result<MkAgentLifecycle, MkDeterministicError> {
    make_lifecycle(
        job,
        status,
        completed_at,
        Some(started_at),
        Some(completed_at),
        None,
        Some(MkAgentRunError {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }),
    )
}

fn make_result(
    keys: &Keys,
    job: MkAgentJobEnvelope,
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
    stale_input: Option<MkStaleInput>,
) -> Result<MkAgentResultEnvelope, MkDeterministicError> {
    let output = deterministic_output(job.persona, job.purpose);
    let output_bytes = serde_json::to_vec(&output)
        .map_err(|error| MkDeterministicError::Serialization(error.to_string()))?;
    let input_bytes = serde_json::to_vec(&job.inputs)
        .map_err(|error| MkDeterministicError::Serialization(error.to_string()))?;
    let input_tokens = token_estimate(input_bytes.len());
    let output_tokens = token_estimate(output_bytes.len());
    let informational = job.purpose == MkAgentPurpose::OperationsBriefing;
    let review_state = if informational {
        None
    } else if stale_input.is_some() {
        Some(MkAgentReviewState::Stale)
    } else {
        Some(MkAgentReviewState::Pending)
    };

    let result = MkAgentResultEnvelope {
        result_id: deterministic_result_id(job.run_id, job.attempt),
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
        review_state,
        template_version: template_version(job.purpose).into(),
        provider: MK_DETERMINISTIC_PROVIDER.into(),
        model: MK_DETERMINISTIC_MODEL.into(),
        agent_pubkey: keys.public_key().to_hex(),
        provenance: deterministic_provenance(&job, started_at),
        started_at,
        completed_at,
        stale_input,
        usage: Some(MkAgentUsage {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens.saturating_add(output_tokens),
            cost_microunits: 0,
            currency: "USD".into(),
        }),
        output,
        job,
    };
    validate_mk_agent_result(&result)?;
    Ok(result)
}

fn deterministic_result_id(run_id: Uuid, attempt: u32) -> Uuid {
    let mut bytes = *run_id.as_bytes();
    let attempt_bytes = attempt.to_be_bytes();
    for (index, byte) in attempt_bytes.iter().enumerate() {
        bytes[12 + index] ^= byte;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn token_estimate(bytes: usize) -> u64 {
    u64::try_from(bytes.div_ceil(4)).unwrap_or(u64::MAX)
}

fn deterministic_provenance(
    job: &MkAgentJobEnvelope,
    retrieved_at: DateTime<Utc>,
) -> Vec<MkAgentProvenance> {
    job.inputs
        .iter()
        .map(|input| input_provenance(job.community_id, input, retrieved_at))
        .collect()
}

fn input_provenance(
    community_id: Uuid,
    input: &MkAgentInput,
    retrieved_at: DateTime<Utc>,
) -> MkAgentProvenance {
    MkAgentProvenance {
        source_id: format!("event:{}", input.event_id),
        source_type: "mk_event".into(),
        title: format!("MK input kind {} version {}", input.kind, input.version),
        locator: format!(
            "buzz://mkideas/entity?community={community_id}&kind={}&d={}&event={}",
            input.kind, input.entity_id, input.event_id
        ),
        retrieved_at,
        sha256: input.sha256.clone(),
    }
}

fn template_version(purpose: MkAgentPurpose) -> &'static str {
    match purpose {
        MkAgentPurpose::GuestResearch => "guest-research.v2",
        MkAgentPurpose::OutreachDraft => "outreach-draft.v2",
        MkAgentPurpose::OutreachFollowUp => "outreach-follow-up.v2",
        MkAgentPurpose::InterviewBrief => "interview-brief.v2",
        MkAgentPurpose::InterviewQuestions => "interview-questions.v2",
        MkAgentPurpose::InterviewRunOfShow => "interview-run-of-show.v2",
        MkAgentPurpose::TimestampedClips => "timestamped-clips.v2",
        MkAgentPurpose::CaptionDraft => "caption-draft.v2",
        MkAgentPurpose::ContentVariants => "content-variants.v2",
        MkAgentPurpose::OperationsBriefing => "operations-briefing.v2",
    }
}

fn deterministic_output(persona: MkAgentPersona, purpose: MkAgentPurpose) -> Value {
    match (persona, purpose) {
        (MkAgentPersona::GuestResearcher, MkAgentPurpose::GuestResearch) => json!({
            "fixture_only": true,
            "why_now": "The synthetic guest is testing a human-owned way to preserve team context.",
            "verified_facts": ["The supplied fixture revision links the guest to the synthetic project."],
            "inferences": ["The topic may connect operational trust to editorial practice."],
            "angles": ["shared memory", "human review", "small-team operations"],
            "questions": [
                "What made the problem visible?",
                "Where did human judgment matter most?",
                "What remains unresolved?"
            ],
            "uncertainties": ["No external research was performed by this deterministic provider."]
        }),
        (
            MkAgentPersona::OutreachDrafter,
            MkAgentPurpose::OutreachDraft | MkAgentPurpose::OutreachFollowUp,
        ) => json!({
            "fixture_only": true,
            "channel": "email_draft",
            "subject": "A conversation about shared memory and human judgment",
            "body": "This is a synthetic outreach draft for partner review. It has not been sent.",
            "delivery_state": "not_sent",
            "dnc_checked": true,
            "dnc_active": false,
            "claims_requiring_verification": []
        }),
        (
            MkAgentPersona::InterviewProducer,
            MkAgentPurpose::InterviewBrief
            | MkAgentPurpose::InterviewQuestions
            | MkAgentPurpose::InterviewRunOfShow,
        ) => json!({
            "fixture_only": true,
            "opening_context": "Explore how a small team preserves context without automating away judgment.",
            "questions": [
                "When did this become a problem worth solving?",
                "Where should automation deliberately stop?",
                "What experiment comes next?"
            ],
            "optional_follow_ups": ["Can you give a specific synthetic example?"],
            "unresolved_facts": ["No external facts were fetched by this deterministic provider."]
        }),
        (
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips
            | MkAgentPurpose::CaptionDraft
            | MkAgentPurpose::ContentVariants,
        ) => json!({
            "fixture_only": true,
            "transcript_fixture": "mkideas-v0-synthetic-interview.vtt",
            "clips": [
                {
                    "start_ms": 4000,
                    "end_ms": 18000,
                    "title": "Ideas begin as remembered conversations",
                    "caption": "The most useful ideas start as conversations worth remembering."
                },
                {
                    "start_ms": 44000,
                    "end_ms": 58000,
                    "title": "Sources visible, humans responsible",
                    "caption": "Trust grows when sources stay visible and humans own the final call."
                }
            ],
            "rendered": false,
            "publication_state": "not_published"
        }),
        (MkAgentPersona::OperationsBriefingAssistant, MkAgentPurpose::OperationsBriefing) => {
            json!({
                "fixture_only": true,
                "approvals": ["One synthetic clip proposal awaits partner review."],
                "deadlines": ["One synthetic interview task is due tomorrow."],
                "blocked_work": ["A synthetic outreach draft needs contact-channel confirmation."],
                "agent_outcomes": ["Guest Researcher completed one sourced fixture draft."],
                "recommendations": ["A partner may review the pending clip proposal next."],
                "informational_only": true
            })
        }
        _ => json!({"fixture_only": true, "unsupported": true}),
    }
}

fn sign_execution(
    keys: &Keys,
    lifecycle: &[MkAgentLifecycle],
    result: Option<&MkAgentResultEnvelope>,
) -> Result<Vec<Event>, MkDeterministicError> {
    let mut events = Vec::with_capacity(lifecycle.len() + usize::from(result.is_some()));
    let mut succeeded = None;
    for item in lifecycle {
        if item.status == MkAgentRunStatus::Succeeded {
            succeeded = Some(item);
        } else {
            events.push(sign_lifecycle(keys, item)?);
        }
    }
    if let Some(result) = result {
        events.push(sign_result(keys, result)?);
    }
    if let Some(item) = succeeded {
        events.push(sign_lifecycle(keys, item)?);
    }
    Ok(events)
}

fn sign_lifecycle(
    keys: &Keys,
    lifecycle: &MkAgentLifecycle,
) -> Result<Event, MkDeterministicError> {
    let status = enum_name(&lifecycle.status)?;
    let payload = canonical_lifecycle_payload(keys, lifecycle)?;
    sign_payload(
        keys,
        MK_AGENT_ACTIVITY_EVENT_KIND,
        &payload,
        &lifecycle.job,
        &status,
        lifecycle.occurred_at,
    )
}

fn sign_result(keys: &Keys, result: &MkAgentResultEnvelope) -> Result<Event, MkDeterministicError> {
    if result.event_kind != MK_AGENT_PROPOSAL_EVENT_KIND
        && result.event_kind != MK_GENERATED_SUMMARY_EVENT_KIND
    {
        return Err(MkDeterministicError::Contract(
            MkAgentContractError::ForbiddenEventKind(result.event_kind),
        ));
    }
    let status = enum_name(&result.status)?;
    let payload = canonical_result_payload(result)?;
    sign_payload(
        keys,
        result.event_kind,
        &payload,
        &result.job,
        &status,
        result.completed_at,
    )
}

fn canonical_lifecycle_payload(
    keys: &Keys,
    lifecycle: &MkAgentLifecycle,
) -> Result<Value, MkDeterministicError> {
    let job = &lifecycle.job;
    let persona = enum_name(&job.persona)?;
    let purpose = enum_name(&job.purpose)?;
    let status = enum_name(&lifecycle.status)?;
    let target = job.target.as_ref().map(canonical_target);
    let input_event_ids = input_event_ids(job);
    let result = lifecycle.result_id.map(|result_id| {
        json!({
            "result_id": result_id,
            "event_kind": if job.purpose == MkAgentPurpose::OperationsBriefing {
                MK_GENERATED_SUMMARY_EVENT_KIND
            } else {
                MK_AGENT_PROPOSAL_EVENT_KIND
            }
        })
    });
    Ok(json!({
        "schema_version": job.schema_version,
        "activity_type": "agent_run",
        "run_id": job.run_id,
        "idempotency_key": job.idempotency_key,
        "persona": persona,
        "persona_id": persona,
        "agent": persona,
        "agent_pubkey": keys.public_key().to_hex(),
        "purpose": purpose,
        "community_id": job.community_id,
        "request_event_id": job.request_event_id,
        "requester_pubkey": job.requester_pubkey,
        "target_id": job.target.as_ref().map(|value| value.entity_id),
        "target_kind": job.target.as_ref().map(|value| value.kind),
        "target_event_id": job.target.as_ref().map(|value| value.event_id.as_str()),
        "target_version": job.target.as_ref().map(|value| value.version),
        "target": target,
        "input_event_ids": input_event_ids,
        "input_hash": job.input_hash,
        "status": status,
        "attempt": job.attempt,
        "retry": job.retry,
        "requested_at": job.requested_at,
        "occurred_at": lifecycle.occurred_at,
        "started_at": lifecycle.started_at,
        "completed_at": lifecycle.completed_at,
        "timestamps": {
            "requested_at": job.requested_at,
            "occurred_at": lifecycle.occurred_at,
            "started_at": lifecycle.started_at,
            "completed_at": lifecycle.completed_at
        },
        "result_id": lifecycle.result_id,
        "result": result,
        "error": lifecycle.error
    }))
}

fn canonical_result_payload(result: &MkAgentResultEnvelope) -> Result<Value, MkDeterministicError> {
    let job = &result.job;
    let persona = enum_name(&job.persona)?;
    let purpose = enum_name(&job.purpose)?;
    let status = enum_name(&result.status)?;
    let input_event_ids = input_event_ids(job);
    let summary = result_summary(job.persona, job.purpose);
    let stale = result.stale_input.is_some();
    let timestamps = json!({
        "started_at": result.started_at,
        "completed_at": result.completed_at
    });

    if result.event_kind == MK_GENERATED_SUMMARY_EVENT_KIND {
        return Ok(json!({
            "schema_version": job.schema_version,
            "summary_id": result.result_id,
            "summary_version": result.result_version,
            "summary_type": purpose,
            "run_id": job.run_id,
            "idempotency_key": job.idempotency_key,
            "persona": persona,
            "persona_id": persona,
            "agent": persona,
            "agent_pubkey": result.agent_pubkey,
            "purpose": purpose,
            "community_id": job.community_id,
            "request_event_id": job.request_event_id,
            "input_event_ids": input_event_ids,
            "input_hash": job.input_hash,
            "template_version": result.template_version,
            "provider": result.provider,
            "model": result.model,
            "summary": summary,
            "output": result.output,
            "provenance": result.provenance,
            "status": status,
            "started_at": result.started_at,
            "completed_at": result.completed_at,
            "timestamps": timestamps,
            "usage": result.usage,
            "stale": stale,
            "stale_input": result.stale_input
        }));
    }

    let target = job.target.as_ref().ok_or_else(|| {
        MkDeterministicError::Contract(MkAgentContractError::InvalidField {
            field: "target",
            reason: "proposal wire payload requires an exact target revision".into(),
        })
    })?;
    let review_state = result.review_state.as_ref().map(enum_name).transpose()?;
    Ok(json!({
        "schema_version": job.schema_version,
        "proposal_id": result.result_id,
        "proposal_version": result.result_version,
        "run_id": job.run_id,
        "idempotency_key": job.idempotency_key,
        "persona": persona,
        "persona_id": persona,
        "agent": persona,
        "agent_pubkey": result.agent_pubkey,
        "purpose": purpose,
        "proposal_type": purpose,
        "community_id": job.community_id,
        "request_event_id": job.request_event_id,
        "target_id": target.entity_id,
        "target_kind": target.kind,
        "target_event_id": target.event_id,
        "target_version": target.version,
        "target": canonical_target(target),
        "input_event_ids": input_event_ids,
        "input_hash": job.input_hash,
        "template_version": result.template_version,
        "provider": result.provider,
        "model": result.model,
        "summary": summary,
        "output": result.output,
        "provenance": result.provenance,
        "status": status,
        "review_state": review_state,
        "review": {
            "state": review_state,
            "human_action_required": true
        },
        "started_at": result.started_at,
        "completed_at": result.completed_at,
        "timestamps": timestamps,
        "usage": result.usage,
        "stale": stale,
        "stale_input": result.stale_input,
        "attempt": job.attempt,
        "retry": job.retry
    }))
}

fn canonical_target(target: &MkEntityRevision) -> Value {
    json!({
        "id": target.entity_id,
        "kind": target.kind,
        "event_id": target.event_id,
        "version": target.version
    })
}

fn input_event_ids(job: &MkAgentJobEnvelope) -> Vec<&str> {
    job.inputs
        .iter()
        .map(|input| input.event_id.as_str())
        .collect()
}

fn result_summary(persona: MkAgentPersona, purpose: MkAgentPurpose) -> &'static str {
    match (persona, purpose) {
        (MkAgentPersona::GuestResearcher, MkAgentPurpose::GuestResearch) => {
            "Synthetic sourced guest research draft awaiting human review."
        }
        (
            MkAgentPersona::OutreachDrafter,
            MkAgentPurpose::OutreachDraft | MkAgentPurpose::OutreachFollowUp,
        ) => "Synthetic unsent outreach draft awaiting human review.",
        (
            MkAgentPersona::InterviewProducer,
            MkAgentPurpose::InterviewBrief
            | MkAgentPurpose::InterviewQuestions
            | MkAgentPurpose::InterviewRunOfShow,
        ) => "Synthetic interview preparation draft awaiting human review.",
        (
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips
            | MkAgentPurpose::CaptionDraft
            | MkAgentPurpose::ContentVariants,
        ) => "Synthetic timestamped content proposal awaiting human review.",
        (MkAgentPersona::OperationsBriefingAssistant, MkAgentPurpose::OperationsBriefing) => {
            "Synthetic informational operations briefing; no state change was proposed."
        }
        _ => "Unsupported deterministic agent result.",
    }
}

fn sign_payload(
    keys: &Keys,
    event_kind: u32,
    payload: &Value,
    job: &MkAgentJobEnvelope,
    status: &str,
    created_at: DateTime<Utc>,
) -> Result<Event, MkDeterministicError> {
    let content = serde_json::to_string(payload)
        .map_err(|error| MkDeterministicError::Serialization(error.to_string()))?;
    let persona = enum_name(&job.persona)?;
    let purpose = enum_name(&job.purpose)?;
    let mut tags = vec![
        make_tag(vec!["h".into(), job.community_id.to_string()])?,
        make_tag(vec!["run".into(), job.run_id.to_string()])?,
        make_tag(vec!["idempotency".into(), job.idempotency_key.to_string()])?,
        make_tag(vec!["persona".into(), persona])?,
        make_tag(vec!["purpose".into(), purpose])?,
        make_tag(vec!["status".into(), status.into()])?,
    ];
    if let Some(target) = &job.target {
        tags.push(make_tag(vec![
            "target".into(),
            target.kind.to_string(),
            target.entity_id.to_string(),
        ])?);
    }
    let seconds = u64::try_from(created_at.timestamp())
        .map_err(|_| MkDeterministicError::TimestampOutOfRange)?;
    EventBuilder::new(Kind::Custom(event_kind as u16), content)
        .tags(tags)
        .custom_created_at(Timestamp::from(seconds))
        .sign_with_keys(keys)
        .map_err(|error| MkDeterministicError::Signing(error.to_string()))
}

fn make_tag(values: Vec<String>) -> Result<Tag, MkDeterministicError> {
    Tag::parse(values).map_err(|error| MkDeterministicError::Signing(error.to_string()))
}

fn enum_name<T: Serialize>(value: &T) -> Result<String, MkDeterministicError> {
    let serialized = serde_json::to_value(value)
        .map_err(|error| MkDeterministicError::Serialization(error.to_string()))?;
    serialized
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| MkDeterministicError::Serialization("enum did not serialize as text".into()))
}

fn plus_seconds(
    timestamp: DateTime<Utc>,
    seconds: i64,
) -> Result<DateTime<Utc>, MkDeterministicError> {
    timestamp
        .checked_add_signed(Duration::seconds(seconds))
        .ok_or(MkDeterministicError::TimestampOutOfRange)
}

#[cfg(test)]
#[path = "mkideas_fake_tests.rs"]
mod tests;
