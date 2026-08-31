//! Offline CLI harness for deterministic, signed MK Ideas agent fixtures.

use anyhow::{Context, Result};
use buzz_acp::mkideas::{
    MkAgentInput, MkAgentJobEnvelope, MkAgentPersona, MkAgentPurpose, MkEntityRevision,
    MK_AGENT_CONTRACT_SCHEMA_VERSION,
};
use buzz_acp::mkideas_fake::{
    MkDeterministicRequest, MkDeterministicRunner, MkDeterministicScenario,
};
use chrono::{TimeZone, Utc};
use clap::{Parser, ValueEnum};
use nostr::Keys;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "mkideas-agent-fixture",
    about = "Generate signed, deterministic MK Ideas agent lifecycle/result events without network access"
)]
struct Args {
    /// Persona to exercise, or all five in product order.
    #[arg(long, value_enum, default_value = "all")]
    persona: PersonaArg,

    /// Deterministic outcome to exercise.
    #[arg(long, value_enum, default_value = "success")]
    scenario: ScenarioArg,

    /// Pretty-print the JSON fixture bundle.
    #[arg(long)]
    pretty: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PersonaArg {
    All,
    GuestResearcher,
    OutreachDrafter,
    InterviewProducer,
    ContentClipCopilot,
    OperationsBriefingAssistant,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScenarioArg {
    Success,
    RetryOnce,
    Failure,
    Cancelled,
    TimedOut,
    Stale,
}

impl ScenarioArg {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::RetryOnce => "retry_once",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Stale => "stale",
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let keys = deterministic_fixture_keys()?;
    let service_pubkey = keys.public_key().to_hex();
    let mut runner = MkDeterministicRunner::new(keys);
    let mut executions = Vec::new();
    for mut request in fixture_requests(args.persona)? {
        request.scenario = match args.scenario {
            ScenarioArg::Success | ScenarioArg::Stale => MkDeterministicScenario::Success,
            ScenarioArg::RetryOnce => MkDeterministicScenario::RetryOnce,
            ScenarioArg::Failure => MkDeterministicScenario::Failure,
            ScenarioArg::Cancelled => MkDeterministicScenario::Cancelled,
            ScenarioArg::TimedOut => MkDeterministicScenario::TimedOut,
        };
        if matches!(args.scenario, ScenarioArg::Stale) {
            let observed = request
                .observed_heads
                .first_mut()
                .context("fixture request must include an observed head")?;
            observed.version = observed
                .version
                .checked_add(1)
                .context("fixture version overflow")?;
            observed.event_id = format!("{:064x}", 90_000 + u64::from(observed.kind));
        }
        executions.push(runner.execute(request)?);
    }

    let bundle = json!({
        "schema_version": MK_AGENT_CONTRACT_SCHEMA_VERSION,
        "fixture_only": true,
        "network_calls": 0,
        "service_pubkey": service_pubkey,
        "scenario": args.scenario.as_str(),
        "executions": executions,
    });
    let rendered = if args.pretty {
        serde_json::to_string_pretty(&bundle)?
    } else {
        serde_json::to_string(&bundle)?
    };
    println!("{rendered}");
    Ok(())
}

fn deterministic_fixture_keys() -> Result<Keys> {
    Keys::parse(&format!("{:064x}", 1)).context("deterministic fixture signing key is invalid")
}

fn fixture_requests(persona: PersonaArg) -> Result<Vec<MkDeterministicRequest>> {
    let cases = [
        (
            MkAgentPersona::GuestResearcher,
            MkAgentPurpose::GuestResearch,
            Some(30_803),
        ),
        (
            MkAgentPersona::OutreachDrafter,
            MkAgentPurpose::OutreachDraft,
            Some(30_803),
        ),
        (
            MkAgentPersona::InterviewProducer,
            MkAgentPurpose::InterviewQuestions,
            Some(30_804),
        ),
        (
            MkAgentPersona::ContentClipCopilot,
            MkAgentPurpose::TimestampedClips,
            Some(30_805),
        ),
        (
            MkAgentPersona::OperationsBriefingAssistant,
            MkAgentPurpose::OperationsBriefing,
            None,
        ),
    ];
    let selected = cases
        .into_iter()
        .enumerate()
        .filter(|(_, (candidate, _, _))| persona.matches(*candidate))
        .map(|(index, (candidate, purpose, target_kind))| {
            fixture_request(index as u64 + 1, candidate, purpose, target_kind)
        })
        .collect::<Result<Vec<_>>>()?;
    if selected.is_empty() {
        anyhow::bail!("no fixture persona matched the request");
    }
    Ok(selected)
}

impl PersonaArg {
    fn matches(self, persona: MkAgentPersona) -> bool {
        match self {
            Self::All => true,
            Self::GuestResearcher => persona == MkAgentPersona::GuestResearcher,
            Self::OutreachDrafter => persona == MkAgentPersona::OutreachDrafter,
            Self::InterviewProducer => persona == MkAgentPersona::InterviewProducer,
            Self::ContentClipCopilot => persona == MkAgentPersona::ContentClipCopilot,
            Self::OperationsBriefingAssistant => {
                persona == MkAgentPersona::OperationsBriefingAssistant
            }
        }
    }
}

fn fixture_request(
    index: u64,
    persona: MkAgentPersona,
    purpose: MkAgentPurpose,
    target_kind: Option<u32>,
) -> Result<MkDeterministicRequest> {
    let entity_id = fixture_uuid(100 + u128::from(index));
    let event_id = format!("{:064x}", 10_000 + index);
    let version = index + 1;
    let input_kind = target_kind.unwrap_or(30_802);
    let target = target_kind.map(|kind| MkEntityRevision {
        kind,
        entity_id,
        event_id: event_id.clone(),
        version,
    });
    let requested_at = Utc
        .with_ymd_and_hms(2026, 8, 30, 18, 0, index as u32)
        .single()
        .context("fixture timestamp is invalid")?;
    Ok(MkDeterministicRequest {
        job: MkAgentJobEnvelope {
            schema_version: MK_AGENT_CONTRACT_SCHEMA_VERSION,
            run_id: fixture_uuid(200 + u128::from(index)),
            idempotency_key: fixture_uuid(300 + u128::from(index)),
            persona,
            purpose,
            community_id: fixture_uuid(400),
            request_event_id: format!("{:064x}", 20_000 + index),
            requester_pubkey: format!("{:064x}", 30_000 + index),
            target,
            inputs: vec![MkAgentInput {
                kind: input_kind,
                entity_id,
                event_id: event_id.clone(),
                version,
                sha256: format!("{:064x}", 40_000 + index),
            }],
            input_hash: format!("{:064x}", 50_000 + index),
            attempt: 1,
            retry: None,
            requested_at,
        },
        observed_heads: vec![MkEntityRevision {
            kind: input_kind,
            entity_id,
            event_id,
            version,
        }],
        scenario: MkDeterministicScenario::Success,
    })
}

fn fixture_uuid(value: u128) -> Uuid {
    Uuid::from_u128(value | 0x0000_0000_0000_4000_8000_0000_0000_0000)
}
