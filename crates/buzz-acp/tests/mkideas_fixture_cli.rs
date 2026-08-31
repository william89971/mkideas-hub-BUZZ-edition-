use std::{collections::BTreeSet, process::Command};

use buzz_core::mkideas::{validate_agent_proposal, validate_agent_service_event};
use nostr::Event;
use serde_json::Value;

#[test]
fn cli_emits_five_offline_signed_fixture_executions() {
    let output = Command::new(env!("CARGO_BIN_EXE_mkideas-agent-fixture"))
        .args(["--persona", "all", "--scenario", "success"])
        .output()
        .expect("fixture CLI must start");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("CLI stdout must be JSON");
    assert_eq!(payload["schema_version"], 2);
    assert_eq!(payload["fixture_only"], true);
    assert_eq!(payload["network_calls"], 0);
    assert_eq!(payload["scenario"], "success");
    let executions = payload["executions"].as_array().expect("executions array");
    assert_eq!(executions.len(), 5);
    let personas = executions
        .iter()
        .map(|execution| {
            execution["lifecycle"][0]["job"]["persona"]
                .as_str()
                .expect("serialized persona")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        personas,
        BTreeSet::from([
            "content-clip-copilot",
            "guest-researcher",
            "interview-producer",
            "operations-briefing-assistant",
            "outreach-drafter",
        ])
    );
    for execution in executions {
        let signed = execution["signed_events"]
            .as_array()
            .expect("signed event array");
        assert!(signed.len() >= 3);
        assert!(signed
            .iter()
            .all(|event| { matches!(event["kind"].as_u64(), Some(48_201 | 48_203 | 48_204)) }));
        for event in signed {
            let signed_event: Event =
                serde_json::from_value(event.clone()).expect("canonical signed Nostr event");
            let content: Value = serde_json::from_str(
                event["content"]
                    .as_str()
                    .expect("signed event content string"),
            )
            .expect("signed event content JSON");
            assert_eq!(content["schema_version"], 2);
            assert!(content.get("job").is_none());
            assert!(content.get("persona").is_some());
            assert!(content.get("run_id").is_some());
            match event["kind"].as_u64() {
                Some(48_201) => {
                    validate_agent_proposal(&signed_event)
                        .expect("proposal fixture must pass the relay's core contract");
                    assert!(content.get("proposal_id").is_some());
                    assert!(content.get("target_event_id").is_some());
                    assert!(content["provenance"][0].get("source_id").is_some());
                    assert_eq!(content["review"]["human_action_required"], true);
                }
                Some(48_203) => {
                    validate_agent_service_event(&signed_event)
                        .expect("summary fixture must pass the relay's core contract");
                    assert!(content.get("summary").is_some());
                    assert_eq!(content["status"], "informational");
                }
                Some(48_204) => {
                    validate_agent_service_event(&signed_event)
                        .expect("lifecycle fixture must pass the relay's core contract");
                    assert_eq!(content["activity_type"], "agent_run");
                }
                _ => unreachable!("event kind was checked above"),
            }
        }
    }
}

#[test]
fn cli_exposes_retry_failure_cancellation_and_timeout_without_network() {
    let scenarios = [
        ("retry-once", "retry_once", true),
        ("failure", "failure", false),
        ("cancelled", "cancelled", false),
        ("timed-out", "timed_out", false),
    ];
    for (argument, serialized, has_result) in scenarios {
        let output = Command::new(env!("CARGO_BIN_EXE_mkideas-agent-fixture"))
            .args(["--persona", "guest-researcher", "--scenario", argument])
            .output()
            .expect("fixture CLI must start");
        assert!(
            output.status.success(),
            "scenario {argument} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let payload: Value =
            serde_json::from_slice(&output.stdout).expect("CLI stdout must be JSON");
        assert_eq!(payload["scenario"], serialized);
        assert_eq!(payload["network_calls"], 0);
        assert_eq!(!payload["executions"][0]["result"].is_null(), has_result);
    }
}

#[test]
fn cli_stale_summary_fails_visibly_without_protected_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_mkideas-agent-fixture"))
        .args([
            "--persona",
            "operations-briefing-assistant",
            "--scenario",
            "stale",
        ])
        .output()
        .expect("fixture CLI must start");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("CLI stdout must be JSON");
    let execution = &payload["executions"][0];
    assert!(execution["result"].is_null());
    assert_eq!(execution["stale"], true);
    assert!(execution["signed_events"]
        .as_array()
        .expect("signed events")
        .iter()
        .all(|event| event["kind"] == 48_204));
}
