//! Typed MK Ideas agent job, lifecycle, and result contracts.
//!
//! This module is intentionally independent from relay ingest and MK state
//! validation. It gives ACP runners one reusable boundary for carrying exact
//! human-signed inputs through retries and for rejecting outputs that attempt
//! to cross a human approval gate.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

/// Current schema version for the ACP-side MK agent contract.
pub const MK_AGENT_CONTRACT_SCHEMA_VERSION: u16 = 2;

const KIND_MK_PERSON: u32 = 30_803;
const KIND_MK_INTERVIEW: u32 = 30_804;
const KIND_MK_CONTENT: u32 = 30_805;

/// MK Ideas service event kind for a human-gated agent proposal/result.
pub const MK_AGENT_PROPOSAL_EVENT_KIND: u32 = 48_201;

/// MK Ideas service event kind for an informational generated summary.
pub const MK_GENERATED_SUMMARY_EVENT_KIND: u32 = 48_203;

/// MK Ideas service event kind for durable agent lifecycle activity.
pub const MK_AGENT_ACTIVITY_EVENT_KIND: u32 = 48_204;

const FORBIDDEN_OUTPUT_KEYS: &[&str] = &[
    "approval",
    "approval_action",
    "approved",
    "chain_of_thought",
    "clear_dnc",
    "decision",
    "hidden_reasoning",
    "internal_reasoning",
    "mutation",
    "publish",
    "reasoning",
    "scratchpad",
    "send_external_communication",
    "state_event",
    "state_patch",
    "status_transition",
    "thoughts",
];

/// A validation failure in an MK agent envelope.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MkAgentContractError {
    /// The payload uses a schema version this runner does not understand.
    #[error("unsupported MK agent schema version {0}")]
    UnsupportedSchemaVersion(u16),

    /// A named field violates a structural invariant.
    #[error("invalid {field}: {reason}")]
    InvalidField {
        /// Stable field path suitable for tests and diagnostics.
        field: &'static str,
        /// Human-readable reason that is safe to expose in runner logs.
        reason: String,
    },

    /// The assigned persona is not permitted to perform the requested purpose.
    #[error("persona {persona:?} is not allowed to perform {purpose:?}")]
    CapabilityMismatch {
        /// Relay-resolved persona associated with the service identity.
        persona: MkAgentPersona,
        /// Requested capability purpose.
        purpose: MkAgentPurpose,
    },

    /// The job target is incompatible with its persona or purpose.
    #[error("target kind {kind:?} is not allowed for {persona:?} / {purpose:?}")]
    TargetKindNotAllowed {
        /// Relay-resolved persona.
        persona: MkAgentPersona,
        /// Requested capability purpose.
        purpose: MkAgentPurpose,
        /// Supplied target kind, or no target for a community summary.
        kind: Option<u32>,
    },

    /// The output attempts to use a protected event kind.
    #[error("event kind {0} is not an agent-writable MK output")]
    ForbiddenEventKind(u32),

    /// The output contains protected action or hidden-reasoning material.
    #[error("agent output contains forbidden key `{0}`")]
    ForbiddenOutputKey(String),

    /// Lifecycle timestamps or state-specific fields are inconsistent.
    #[error("invalid lifecycle: {0}")]
    InvalidLifecycle(String),
}

/// One of the five owner-assigned MK Ideas personas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MkAgentPersona {
    /// Researches prospective guests using sourced public information.
    GuestResearcher,
    /// Produces unsent outreach drafts.
    OutreachDrafter,
    /// Produces interview briefs, questions, and run-of-show drafts.
    InterviewProducer,
    /// Produces transcript-grounded clips and content drafts.
    ContentClipCopilot,
    /// Produces informational operational briefings.
    OperationsBriefingAssistant,
}

/// A narrow capability purpose requested from an MK Ideas persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MkAgentPurpose {
    /// Sourced guest research.
    GuestResearch,
    /// Initial outreach draft.
    OutreachDraft,
    /// Follow-up outreach draft.
    OutreachFollowUp,
    /// Interview briefing draft.
    InterviewBrief,
    /// Interview question draft.
    InterviewQuestions,
    /// Interview run-of-show draft.
    InterviewRunOfShow,
    /// Timestamped transcript clip proposals.
    TimestampedClips,
    /// Caption draft.
    CaptionDraft,
    /// Alternative content variants.
    ContentVariants,
    /// Informational community operations briefing.
    OperationsBriefing,
}

/// Exact revision of an addressable MK entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkEntityRevision {
    /// MK state event kind.
    pub kind: u32,
    /// Stable entity UUID from the event's `d` tag.
    pub entity_id: Uuid,
    /// Exact signed event ID used by the run.
    pub event_id: String,
    /// Monotonic entity version used by the run.
    pub version: u64,
}

/// One immutable, versioned event input consumed by a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkAgentInput {
    /// Event kind of the input revision.
    pub kind: u32,
    /// Stable UUID of the input record or proposal.
    pub entity_id: Uuid,
    /// Exact signed event ID consumed by the run.
    pub event_id: String,
    /// Exact record or result version consumed by the run.
    pub version: u64,
    /// SHA-256 of the normalized input made available to the agent.
    pub sha256: String,
}

/// Relationship between this attempt and a prior attempt or human rerun.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkRetryRelation {
    /// Whether the runner retried automatically or a human requested a new run.
    pub mode: MkRetryMode,
    /// Logical run identifier of the prior attempt/run.
    pub prior_run_id: Uuid,
    /// Idempotency identifier of the prior attempt/run.
    pub prior_idempotency_key: Uuid,
    /// Attempt number that preceded this job.
    pub prior_attempt: u32,
}

/// Retry mode used by an MK agent job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MkRetryMode {
    /// The runner reuses the logical run and idempotency IDs.
    Automatic,
    /// A human starts a fresh run linked to the prior logical run.
    HumanRerun,
}

/// Immutable job envelope handed from Buzz to an ACP agent runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkAgentJobEnvelope {
    /// Contract schema version.
    pub schema_version: u16,
    /// Logical run identifier, stable across automatic retries.
    pub run_id: Uuid,
    /// Idempotency identifier, stable across automatic retries.
    pub idempotency_key: Uuid,
    /// Relay-resolved, owner-controlled persona.
    pub persona: MkAgentPersona,
    /// Narrow capability requested from the persona.
    pub purpose: MkAgentPurpose,
    /// Community boundary for every input and output.
    pub community_id: Uuid,
    /// Human-signed Team request event that initiated the run.
    pub request_event_id: String,
    /// Human requester pubkey.
    pub requester_pubkey: String,
    /// Exact target revision; absent only for a community briefing.
    pub target: Option<MkEntityRevision>,
    /// Complete versioned event inputs supplied to the run.
    pub inputs: Vec<MkAgentInput>,
    /// SHA-256 of the canonical ordered input envelope.
    pub input_hash: String,
    /// One-based attempt number.
    pub attempt: u32,
    /// Retry relationship when this is not an initial run.
    pub retry: Option<MkRetryRelation>,
    /// Time the human-signed request was accepted for execution.
    pub requested_at: DateTime<Utc>,
}

/// Durable lifecycle state for one MK agent run attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MkAgentRunStatus {
    /// Accepted and waiting for runner capacity.
    Queued,
    /// Currently executing.
    Running,
    /// A retryable failure occurred and another attempt is scheduled.
    Retrying,
    /// Completed with a proposal or informational summary.
    Succeeded,
    /// Completed without an output because of an error.
    Failed,
    /// Explicitly cancelled by an authorized human or runner policy.
    Cancelled,
    /// Exceeded an idle or hard timeout.
    TimedOut,
}

/// Sanitized lifecycle error visible in Today and record timelines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkAgentRunError {
    /// Stable machine-readable error code.
    pub code: String,
    /// Concise message with secrets and raw provider responses removed.
    pub message: String,
    /// Whether automatic retry is permitted.
    pub retryable: bool,
}

/// Lifecycle event derived by the runner from one immutable job envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkAgentLifecycle {
    /// Immutable job and exact inputs associated with the state change.
    pub job: MkAgentJobEnvelope,
    /// Current run state.
    pub status: MkAgentRunStatus,
    /// Time this lifecycle state became true.
    pub occurred_at: DateTime<Utc>,
    /// Start time once execution begins.
    pub started_at: Option<DateTime<Utc>>,
    /// Completion time for terminal states.
    pub completed_at: Option<DateTime<Utc>>,
    /// Result identifier for a successful terminal state.
    pub result_id: Option<Uuid>,
    /// Sanitized error for retrying or unsuccessful terminal states.
    pub error: Option<MkAgentRunError>,
}

/// Structured provenance for one source supporting an agent result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkAgentProvenance {
    /// Stable source identifier.
    pub source_id: String,
    /// Source category, such as `mk_event`, `private_transcript`, or `web_page`.
    pub source_type: String,
    /// Human-readable source title.
    pub title: String,
    /// Stable locator or URL shown to reviewers.
    pub locator: String,
    /// Time the runner retrieved or read the source.
    pub retrieved_at: DateTime<Utc>,
    /// SHA-256 of the exact source representation used by the run.
    pub sha256: String,
}

/// Agent-writable result status; intentionally excludes approval states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MkAgentResultStatus {
    /// A draft/proposal awaiting human review.
    Proposed,
    /// An informational summary that cannot mutate state.
    Informational,
}

/// Agent-writable review state; approval and rejection remain human-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MkAgentReviewState {
    /// Available for a human to review.
    Pending,
    /// Retained for history but no longer approvable because an input advanced.
    Stale,
}

/// Exact evidence that an input advanced after a result was produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkStaleInput {
    /// Input event used by the run.
    pub expected_event_id: String,
    /// Input version used by the run.
    pub expected_version: u64,
    /// Current head event observed after the run.
    pub current_event_id: String,
    /// Current head version observed after the run.
    pub current_version: u64,
    /// Time staleness was detected.
    pub detected_at: DateTime<Utc>,
}

/// Optional token and cost metadata supplied by a provider adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkAgentUsage {
    /// Provider-reported input tokens.
    pub input_tokens: u64,
    /// Provider-reported output tokens.
    pub output_tokens: u64,
    /// Provider-reported total tokens.
    pub total_tokens: u64,
    /// Cost in currency microunits, or zero for a deterministic local provider.
    pub cost_microunits: u64,
    /// ISO-style currency code used for cost microunits.
    pub currency: String,
}

/// Successful proposal or informational output returned by an MK agent runner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MkAgentResultEnvelope {
    /// Immutable job and exact inputs used to produce the result.
    pub job: MkAgentJobEnvelope,
    /// Stable result/proposal UUID.
    pub result_id: Uuid,
    /// One-based result version.
    pub result_version: u32,
    /// Service event kind requested for publication.
    pub event_kind: u32,
    /// Agent-writable proposal or summary status.
    pub status: MkAgentResultStatus,
    /// Review state for proposals; absent for informational summaries.
    pub review_state: Option<MkAgentReviewState>,
    /// Persona template version used for this result.
    pub template_version: String,
    /// Provider identifier, including `fixture` or `local`.
    pub provider: String,
    /// Model identifier when available.
    pub model: String,
    /// Service identity that will sign the result event.
    pub agent_pubkey: String,
    /// Sources shown to human reviewers.
    pub provenance: Vec<MkAgentProvenance>,
    /// Start time for result production.
    pub started_at: DateTime<Utc>,
    /// Completion time for result production.
    pub completed_at: DateTime<Utc>,
    /// Staleness evidence when the review state is stale.
    pub stale_input: Option<MkStaleInput>,
    /// Optional provider usage and cost metadata.
    pub usage: Option<MkAgentUsage>,
    /// Typed persona output represented as JSON until relay schemas land.
    pub output: Value,
}

/// Validate an immutable MK agent job before it enters the ACP queue.
pub fn validate_mk_agent_job(job: &MkAgentJobEnvelope) -> Result<(), MkAgentContractError> {
    validate_schema(job.schema_version)?;
    validate_uuid(job.run_id, "run_id")?;
    validate_uuid(job.idempotency_key, "idempotency_key")?;
    validate_hex_64(&job.request_event_id, "request_event_id")?;
    validate_hex_64(&job.requester_pubkey, "requester_pubkey")?;
    validate_hex_64(&job.input_hash, "input_hash")?;

    if !persona_allows_purpose(job.persona, job.purpose) {
        return Err(MkAgentContractError::CapabilityMismatch {
            persona: job.persona,
            purpose: job.purpose,
        });
    }

    validate_target(job)?;
    validate_inputs(job)?;
    validate_retry(job)?;
    Ok(())
}

/// Validate state-specific lifecycle fields and timings for an MK agent run.
pub fn validate_mk_agent_lifecycle(
    lifecycle: &MkAgentLifecycle,
) -> Result<(), MkAgentContractError> {
    validate_mk_agent_job(&lifecycle.job)?;
    if lifecycle.occurred_at < lifecycle.job.requested_at {
        return Err(MkAgentContractError::InvalidLifecycle(
            "occurred_at precedes requested_at".into(),
        ));
    }

    if let Some(started_at) = lifecycle.started_at {
        if started_at < lifecycle.job.requested_at {
            return Err(MkAgentContractError::InvalidLifecycle(
                "started_at precedes requested_at".into(),
            ));
        }
    }
    if let Some(completed_at) = lifecycle.completed_at {
        let Some(started_at) = lifecycle.started_at else {
            return Err(MkAgentContractError::InvalidLifecycle(
                "completed_at requires started_at".into(),
            ));
        };
        if completed_at < started_at || lifecycle.occurred_at < completed_at {
            return Err(MkAgentContractError::InvalidLifecycle(
                "completion timestamps are out of order".into(),
            ));
        }
    }

    match lifecycle.status {
        MkAgentRunStatus::Queued => require_lifecycle_shape(lifecycle, false, false, false, false),
        MkAgentRunStatus::Running => require_lifecycle_shape(lifecycle, true, false, false, false),
        MkAgentRunStatus::Retrying => {
            require_lifecycle_shape(lifecycle, true, false, false, true)?;
            let error = lifecycle.error.as_ref().ok_or_else(|| {
                MkAgentContractError::InvalidLifecycle("retrying requires an error".into())
            })?;
            if !error.retryable || lifecycle.job.retry.is_none() {
                return Err(MkAgentContractError::InvalidLifecycle(
                    "retrying requires a retryable error and retry relation".into(),
                ));
            }
            Ok(())
        }
        MkAgentRunStatus::Succeeded => require_lifecycle_shape(lifecycle, true, true, true, false),
        MkAgentRunStatus::Failed => require_lifecycle_shape(lifecycle, true, true, false, true),
        MkAgentRunStatus::Cancelled | MkAgentRunStatus::TimedOut => {
            require_lifecycle_shape(lifecycle, true, true, false, true)?;
            if lifecycle
                .error
                .as_ref()
                .is_some_and(|error| error.retryable)
            {
                return Err(MkAgentContractError::InvalidLifecycle(
                    "cancelled and timed-out terminal states cannot be retryable".into(),
                ));
            }
            Ok(())
        }
    }
}

/// Validate a successful MK proposal or informational summary.
pub fn validate_mk_agent_result(
    result: &MkAgentResultEnvelope,
) -> Result<(), MkAgentContractError> {
    validate_mk_agent_job(&result.job)?;
    validate_uuid(result.result_id, "result_id")?;
    if result.result_version == 0 {
        return invalid("result_version", "must be greater than zero");
    }
    validate_nonempty(&result.template_version, "template_version")?;
    validate_nonempty(&result.provider, "provider")?;
    validate_nonempty(&result.model, "model")?;
    validate_hex_64(&result.agent_pubkey, "agent_pubkey")?;
    validate_result_times(result)?;
    validate_provenance(&result.provenance)?;
    validate_usage(result.usage.as_ref())?;
    validate_output_value(&result.output)?;

    match result.status {
        MkAgentResultStatus::Proposed => {
            if result.event_kind != MK_AGENT_PROPOSAL_EVENT_KIND {
                return Err(MkAgentContractError::ForbiddenEventKind(result.event_kind));
            }
            if result.job.purpose == MkAgentPurpose::OperationsBriefing {
                return Err(MkAgentContractError::CapabilityMismatch {
                    persona: result.job.persona,
                    purpose: result.job.purpose,
                });
            }
            let review_state =
                result
                    .review_state
                    .ok_or_else(|| MkAgentContractError::InvalidField {
                        field: "review_state",
                        reason: "proposals require pending or stale review state".into(),
                    })?;
            validate_stale_state(result, review_state)
        }
        MkAgentResultStatus::Informational => {
            if result.event_kind != MK_GENERATED_SUMMARY_EVENT_KIND {
                return Err(MkAgentContractError::ForbiddenEventKind(result.event_kind));
            }
            if result.job.purpose != MkAgentPurpose::OperationsBriefing
                || result.review_state.is_some()
                || result.stale_input.is_some()
            {
                return Err(MkAgentContractError::InvalidField {
                    field: "status",
                    reason: "only operations briefings may be informational and they have no review state"
                        .into(),
                });
            }
            Ok(())
        }
    }
}

fn validate_schema(schema_version: u16) -> Result<(), MkAgentContractError> {
    if schema_version != MK_AGENT_CONTRACT_SCHEMA_VERSION {
        return Err(MkAgentContractError::UnsupportedSchemaVersion(
            schema_version,
        ));
    }
    Ok(())
}

fn validate_uuid(value: Uuid, field: &'static str) -> Result<(), MkAgentContractError> {
    if value.is_nil() {
        return invalid(field, "must not be nil");
    }
    Ok(())
}

fn validate_hex_64(value: &str, field: &'static str) -> Result<(), MkAgentContractError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return invalid(field, "must be 64 lowercase hexadecimal characters");
    }
    Ok(())
}

fn validate_nonempty(value: &str, field: &'static str) -> Result<(), MkAgentContractError> {
    if value.trim().is_empty() {
        return invalid(field, "must not be empty");
    }
    Ok(())
}

fn validate_target(job: &MkAgentJobEnvelope) -> Result<(), MkAgentContractError> {
    let allowed = target_kinds(job.persona, job.purpose);
    match (&job.target, allowed) {
        (None, &[]) => Ok(()),
        (Some(target), allowed) if allowed.contains(&target.kind) => {
            validate_uuid(target.entity_id, "target.entity_id")?;
            validate_hex_64(&target.event_id, "target.event_id")?;
            if target.version == 0 {
                return invalid("target.version", "must be greater than zero");
            }
            Ok(())
        }
        (target, _) => Err(MkAgentContractError::TargetKindNotAllowed {
            persona: job.persona,
            purpose: job.purpose,
            kind: target.as_ref().map(|target| target.kind),
        }),
    }
}

fn validate_inputs(job: &MkAgentJobEnvelope) -> Result<(), MkAgentContractError> {
    if job.inputs.is_empty() {
        return invalid("inputs", "at least one exact versioned input is required");
    }
    let mut event_ids = HashSet::with_capacity(job.inputs.len());
    for input in &job.inputs {
        validate_uuid(input.entity_id, "inputs.entity_id")?;
        validate_hex_64(&input.event_id, "inputs.event_id")?;
        validate_hex_64(&input.sha256, "inputs.sha256")?;
        if input.version == 0 {
            return invalid("inputs.version", "must be greater than zero");
        }
        if !event_ids.insert(&input.event_id) {
            return invalid("inputs.event_id", "duplicate input event ID");
        }
    }

    if let Some(target) = &job.target {
        let includes_exact_target = job.inputs.iter().any(|input| {
            input.kind == target.kind
                && input.entity_id == target.entity_id
                && input.event_id == target.event_id
                && input.version == target.version
        });
        if !includes_exact_target {
            return invalid(
                "inputs",
                "must include the exact target event ID and version",
            );
        }
    }
    Ok(())
}

fn validate_retry(job: &MkAgentJobEnvelope) -> Result<(), MkAgentContractError> {
    match &job.retry {
        None if job.attempt == 1 => Ok(()),
        None => invalid("retry", "attempts above one require a retry relation"),
        Some(retry) if retry.prior_attempt == 0 => {
            invalid("retry.prior_attempt", "must be greater than zero")
        }
        Some(retry) => match retry.mode {
            MkRetryMode::Automatic => {
                if job.attempt != retry.prior_attempt + 1
                    || job.run_id != retry.prior_run_id
                    || job.idempotency_key != retry.prior_idempotency_key
                {
                    return invalid(
                        "retry",
                        "automatic retry must retain run/idempotency IDs and increment attempt",
                    );
                }
                Ok(())
            }
            MkRetryMode::HumanRerun => {
                if job.attempt != 1
                    || job.run_id == retry.prior_run_id
                    || job.idempotency_key == retry.prior_idempotency_key
                {
                    return invalid(
                        "retry",
                        "human rerun must start at attempt one with fresh run/idempotency IDs",
                    );
                }
                Ok(())
            }
        },
    }
}

fn require_lifecycle_shape(
    lifecycle: &MkAgentLifecycle,
    requires_started: bool,
    requires_completed: bool,
    requires_result: bool,
    requires_error: bool,
) -> Result<(), MkAgentContractError> {
    if lifecycle.started_at.is_some() != requires_started
        || lifecycle.completed_at.is_some() != requires_completed
        || lifecycle.result_id.is_some() != requires_result
        || lifecycle.error.is_some() != requires_error
    {
        return Err(MkAgentContractError::InvalidLifecycle(format!(
            "{:?} carries fields for a different lifecycle state",
            lifecycle.status
        )));
    }
    if let Some(result_id) = lifecycle.result_id {
        validate_uuid(result_id, "result_id")?;
    }
    if let Some(error) = &lifecycle.error {
        validate_nonempty(&error.code, "error.code")?;
        validate_nonempty(&error.message, "error.message")?;
        if error.message.len() > 2_048 {
            return invalid("error.message", "must not exceed 2048 bytes");
        }
    }
    Ok(())
}

fn validate_result_times(result: &MkAgentResultEnvelope) -> Result<(), MkAgentContractError> {
    if result.started_at < result.job.requested_at {
        return invalid("started_at", "must not precede requested_at");
    }
    if result.completed_at < result.started_at {
        return invalid("completed_at", "must not precede started_at");
    }
    Ok(())
}

fn validate_provenance(provenance: &[MkAgentProvenance]) -> Result<(), MkAgentContractError> {
    if provenance.is_empty() {
        return invalid("provenance", "at least one source is required");
    }
    let mut source_ids = HashSet::with_capacity(provenance.len());
    for source in provenance {
        validate_nonempty(&source.source_id, "provenance.source_id")?;
        validate_nonempty(&source.source_type, "provenance.source_type")?;
        validate_nonempty(&source.title, "provenance.title")?;
        validate_nonempty(&source.locator, "provenance.locator")?;
        validate_hex_64(&source.sha256, "provenance.sha256")?;
        if !source_ids.insert(&source.source_id) {
            return invalid("provenance.source_id", "duplicate source ID");
        }
    }
    Ok(())
}

fn validate_usage(usage: Option<&MkAgentUsage>) -> Result<(), MkAgentContractError> {
    let Some(usage) = usage else {
        return Ok(());
    };
    if usage.total_tokens != usage.input_tokens.saturating_add(usage.output_tokens) {
        return invalid("usage.total_tokens", "must equal input plus output tokens");
    }
    validate_nonempty(&usage.currency, "usage.currency")
}

fn validate_output_value(output: &Value) -> Result<(), MkAgentContractError> {
    if !output.is_object() {
        return invalid("output", "must be a structured JSON object");
    }
    find_forbidden_output_key(output)
}

fn find_forbidden_output_key(value: &Value) -> Result<(), MkAgentContractError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if FORBIDDEN_OUTPUT_KEYS.contains(&key.as_str()) {
                    return Err(MkAgentContractError::ForbiddenOutputKey(key.clone()));
                }
                find_forbidden_output_key(child)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                find_forbidden_output_key(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_stale_state(
    result: &MkAgentResultEnvelope,
    review_state: MkAgentReviewState,
) -> Result<(), MkAgentContractError> {
    match (review_state, &result.stale_input) {
        (MkAgentReviewState::Pending, None) => Ok(()),
        (MkAgentReviewState::Stale, Some(stale)) => {
            validate_hex_64(&stale.expected_event_id, "stale_input.expected_event_id")?;
            validate_hex_64(&stale.current_event_id, "stale_input.current_event_id")?;
            let expected_input = result.job.inputs.iter().find(|input| {
                input.event_id == stale.expected_event_id && input.version == stale.expected_version
            });
            if expected_input.is_none()
                || stale.current_version <= stale.expected_version
                || stale.current_event_id == stale.expected_event_id
                || stale.detected_at < result.completed_at
            {
                return invalid(
                    "stale_input",
                    "must identify an exact consumed input and a newer distinct current revision",
                );
            }
            Ok(())
        }
        _ => invalid(
            "review_state",
            "pending must not carry staleness and stale must carry staleness evidence",
        ),
    }
}

fn persona_allows_purpose(persona: MkAgentPersona, purpose: MkAgentPurpose) -> bool {
    matches!(
        (persona, purpose),
        (
            MkAgentPersona::GuestResearcher,
            MkAgentPurpose::GuestResearch
        ) | (
            MkAgentPersona::OutreachDrafter,
            MkAgentPurpose::OutreachDraft | MkAgentPurpose::OutreachFollowUp
        ) | (
            MkAgentPersona::InterviewProducer,
            MkAgentPurpose::InterviewBrief
                | MkAgentPurpose::InterviewQuestions
                | MkAgentPurpose::InterviewRunOfShow
        ) | (
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips
                | MkAgentPurpose::CaptionDraft
                | MkAgentPurpose::ContentVariants
        ) | (
            MkAgentPersona::OperationsBriefingAssistant,
            MkAgentPurpose::OperationsBriefing
        )
    )
}

fn target_kinds(persona: MkAgentPersona, purpose: MkAgentPurpose) -> &'static [u32] {
    match (persona, purpose) {
        (MkAgentPersona::GuestResearcher, MkAgentPurpose::GuestResearch)
        | (
            MkAgentPersona::OutreachDrafter,
            MkAgentPurpose::OutreachDraft | MkAgentPurpose::OutreachFollowUp,
        ) => &[KIND_MK_PERSON],
        (
            MkAgentPersona::InterviewProducer,
            MkAgentPurpose::InterviewBrief
            | MkAgentPurpose::InterviewQuestions
            | MkAgentPurpose::InterviewRunOfShow,
        ) => &[KIND_MK_PERSON, KIND_MK_INTERVIEW],
        (
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips
            | MkAgentPurpose::CaptionDraft
            | MkAgentPurpose::ContentVariants,
        ) => &[KIND_MK_INTERVIEW, KIND_MK_CONTENT],
        (MkAgentPersona::OperationsBriefingAssistant, MkAgentPurpose::OperationsBriefing) => &[],
        _ => &[],
    }
}

fn invalid<T>(field: &'static str, reason: impl Into<String>) -> Result<T, MkAgentContractError> {
    Err(MkAgentContractError::InvalidField {
        field,
        reason: reason.into(),
    })
}

#[cfg(test)]
#[path = "mkideas_tests.rs"]
mod tests;
