use buzz_core::kind::{KIND_MK_AGENT_PROPOSAL, KIND_MK_CONTENT, KIND_MK_INTERVIEW, KIND_MK_PERSON};
use nostr::{EventBuilder, Kind, Tag};
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;

use crate::client::{normalize_write_response, BuzzClient};
use crate::error::CliError;
use crate::MkIdeasCmd;

pub async fn dispatch(cmd: MkIdeasCmd, client: &BuzzClient) -> Result<(), CliError> {
    match cmd {
        MkIdeasCmd::SeedDemo => seed_demo(client).await,
        MkIdeasCmd::Propose {
            target,
            target_kind,
            agent,
            proposal_type,
            summary,
            provenance,
            draft,
            clips_json,
        } => {
            let clips = parse_clips(clips_json.as_deref())?;
            cmd_propose(
                client,
                ProposalInput {
                    target: &target,
                    target_kind,
                    agent: &agent,
                    proposal_type: &proposal_type,
                    summary: &summary,
                    provenance: &provenance,
                    draft: draft.as_deref(),
                    clips,
                },
            )
            .await
        }
    }
}

const DEMO_GUEST_ID: &str = "7f8d9c9f-2c1b-4f87-a4fe-cb4b639418b1";
const DEMO_INTERVIEW_ID: &str = "53566e42-c908-48c3-8e69-eb63acdb904c";
const DEMO_CONTENT_ID: &str = "ebaaef77-2b1d-4f85-8a4f-c319458176ae";

async fn seed_demo(client: &BuzzClient) -> Result<(), CliError> {
    let community = community_host(client.relay_url())?;
    let records = [
        SeedRecord {
            kind: KIND_MK_PERSON,
            entity_id: DEMO_GUEST_ID,
            record_type: "person",
            status: "research_ready",
            fields: json!({
                "name": "Maya Chen (synthetic)",
                "organization": "Signal & Craft Studio",
                "why_now": "A fictional guest used only to prove the V0 workflow.",
                "do_not_contact": false
            }),
        },
        SeedRecord {
            kind: KIND_MK_INTERVIEW,
            entity_id: DEMO_INTERVIEW_ID,
            record_type: "interview",
            status: "content_processing",
            fields: json!({
                "title": "Interview with Maya Chen (synthetic)",
                "guest_id": DEMO_GUEST_ID,
                "transcript_name": "mkideas-v0-synthetic-interview.vtt",
                "transcript_text": "00:00:04.000 --> 00:00:18.000\nThe most useful ideas start as conversations that somebody cared enough to remember.\n\n00:00:22.000 --> 00:00:39.000\nA small team moves faster when the decision and the discussion stay attached to the work."
            }),
        },
        SeedRecord {
            kind: KIND_MK_CONTENT,
            entity_id: DEMO_CONTENT_ID,
            record_type: "content",
            status: "internal_review",
            fields: json!({
                "title": "Maya Chen — conversation as operating memory",
                "interview_id": DEMO_INTERVIEW_ID
            }),
        },
    ];

    let mut created = Vec::new();
    let mut skipped = Vec::new();
    for record in records {
        if seed_record_exists(client, &community, record.kind, record.entity_id).await? {
            skipped.push(record.entity_id);
            continue;
        }
        publish_seed_record(client, &community, &record).await?;
        created.push(record.entity_id);
    }
    println!(
        "{}",
        json!({
            "seed": "mkideas-v0-demo-v1",
            "created": created,
            "skipped_existing": skipped,
            "synthetic": true
        })
    );
    Ok(())
}

#[derive(Clone)]
struct SeedRecord {
    kind: u32,
    entity_id: &'static str,
    record_type: &'static str,
    status: &'static str,
    fields: Value,
}

async fn seed_record_exists(
    client: &BuzzClient,
    community: &str,
    kind: u32,
    entity_id: &str,
) -> Result<bool, CliError> {
    let filter = json!({
        "kinds": [kind],
        "#h": [community],
        "#d": [entity_id],
        "limit": 1
    });
    let raw = client.query(&filter).await?;
    let events: Vec<Value> = serde_json::from_str(&raw)
        .map_err(|error| CliError::Other(format!("invalid seed query response: {error}")))?;
    Ok(!events.is_empty())
}

async fn publish_seed_record(
    client: &BuzzClient,
    community: &str,
    record: &SeedRecord,
) -> Result<(), CliError> {
    let mut content = record.fields.clone();
    let object = content
        .as_object_mut()
        .ok_or_else(|| CliError::Other("seed fields must be an object".into()))?;
    object.insert("schema_version".into(), json!(1));
    object.insert("record_type".into(), json!(record.record_type));
    object.insert("entity_id".into(), json!(record.entity_id));
    object.insert("version".into(), json!(1));
    object.insert("status".into(), json!(record.status));
    object.insert(
        "source_id".into(),
        json!(format!("mkideas-v0-demo-v1:{}", record.entity_id)),
    );
    object.insert("source_system".into(), json!("mkideas-v0-synthetic-seed"));
    object.insert("synthetic".into(), json!(true));

    let mut tags = vec![
        Tag::parse(["d", record.entity_id])
            .map_err(|error| CliError::Other(format!("invalid seed d tag: {error}")))?,
        Tag::parse(["h", community])
            .map_err(|error| CliError::Other(format!("invalid seed h tag: {error}")))?,
        Tag::parse(["version", "1"])
            .map_err(|error| CliError::Other(format!("invalid seed version tag: {error}")))?,
        Tag::parse(["status", record.status])
            .map_err(|error| CliError::Other(format!("invalid seed status tag: {error}")))?,
    ];
    if record.kind == KIND_MK_INTERVIEW {
        let guest_id = content
            .get("guest_id")
            .and_then(Value::as_str)
            .ok_or_else(|| CliError::Other("interview seed requires guest_id".into()))?;
        tags.push(
            Tag::parse(["guest", guest_id])
                .map_err(|error| CliError::Other(format!("invalid guest tag: {error}")))?,
        );
    }
    if record.kind == KIND_MK_CONTENT {
        let interview_id = content
            .get("interview_id")
            .and_then(Value::as_str)
            .ok_or_else(|| CliError::Other("content seed requires interview_id".into()))?;
        tags.push(
            Tag::parse(["interview", interview_id])
                .map_err(|error| CliError::Other(format!("invalid interview tag: {error}")))?,
        );
    }
    let builder =
        EventBuilder::new(Kind::Custom(record.kind as u16), content.to_string()).tags(tags);
    let event = client.sign_event(builder)?;
    let response = client.submit_event(event).await?;
    let normalized = normalize_write_response(&response);
    let value: Value = serde_json::from_str(&normalized)
        .map_err(|error| CliError::Other(format!("invalid seed write response: {error}")))?;
    if value.get("accepted").and_then(Value::as_bool) != Some(true) {
        return Err(CliError::Other(format!(
            "relay rejected synthetic seed {}: {}",
            record.entity_id,
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        )));
    }
    Ok(())
}

struct ProposalInput<'a> {
    target: &'a str,
    target_kind: u32,
    agent: &'a str,
    proposal_type: &'a str,
    summary: &'a str,
    provenance: &'a [String],
    draft: Option<&'a str>,
    clips: Vec<Value>,
}

async fn cmd_propose(client: &BuzzClient, input: ProposalInput<'_>) -> Result<(), CliError> {
    let target = Uuid::parse_str(input.target)
        .map_err(|_| CliError::Usage("--target must be a UUID".into()))?;
    if !matches!(
        input.target_kind,
        KIND_MK_PERSON | KIND_MK_INTERVIEW | KIND_MK_CONTENT
    ) {
        return Err(CliError::Usage(
            "--target-kind must be 30803, 30804, or 30805".into(),
        ));
    }
    for (name, value) in [
        ("--agent", input.agent),
        ("--proposal-type", input.proposal_type),
        ("--summary", input.summary),
    ] {
        if value.trim().is_empty() {
            return Err(CliError::Usage(format!("{name} must not be empty")));
        }
    }
    if input.provenance.iter().any(|item| item.trim().is_empty()) {
        return Err(CliError::Usage(
            "--provenance values must not be empty".into(),
        ));
    }

    let proposal_id = Uuid::new_v4();
    let community = community_host(client.relay_url())?;
    let mut content = json!({
        "schema_version": 1,
        "proposal_id": proposal_id,
        "proposal_version": 1,
        "target_id": target,
        "target_kind": input.target_kind,
        "agent": input.agent.trim(),
        "proposal_type": input.proposal_type.trim(),
        "summary": input.summary.trim(),
        "provenance": input.provenance,
        "status": "proposed",
    });
    if let Some(draft) = input.draft.filter(|value| !value.trim().is_empty()) {
        content["draft"] = Value::String(draft.to_owned());
    }
    if !input.clips.is_empty() {
        content["clips"] = Value::Array(input.clips);
    }

    let tags = [
        Tag::parse(["h", community.as_str()])
            .map_err(|error| CliError::Other(format!("invalid community tag: {error}")))?,
        Tag::parse(["target", &input.target_kind.to_string(), input.target])
            .map_err(|error| CliError::Other(format!("invalid target tag: {error}")))?,
        Tag::parse(["status", "proposed"])
            .map_err(|error| CliError::Other(format!("invalid status tag: {error}")))?,
    ];
    let builder = EventBuilder::new(
        Kind::Custom(KIND_MK_AGENT_PROPOSAL as u16),
        content.to_string(),
    )
    .tags(tags);
    let event = client.sign_event(builder)?;
    let response = client.submit_event(event).await?;
    println!("{}", normalize_write_response(&response));
    Ok(())
}

fn parse_clips(raw: Option<&str>) -> Result<Vec<Value>, CliError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let clips: Vec<Value> = serde_json::from_str(raw)
        .map_err(|error| CliError::Usage(format!("invalid --clips-json: {error}")))?;
    for clip in &clips {
        let Some(object) = clip.as_object() else {
            return Err(CliError::Usage("every clip must be a JSON object".into()));
        };
        for key in ["start", "end", "title", "caption"] {
            if object.get(key).and_then(Value::as_str).is_none() {
                return Err(CliError::Usage(format!(
                    "every clip requires a string {key} field"
                )));
            }
        }
    }
    Ok(clips)
}

fn community_host(relay_url: &str) -> Result<String, CliError> {
    let url = Url::parse(relay_url)
        .map_err(|error| CliError::Usage(format!("invalid relay URL: {error}")))?;
    let host = url
        .host_str()
        .ok_or_else(|| CliError::Usage("relay URL is missing a host".into()))?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let default_port = matches!(
        (url.scheme(), url.port()),
        ("http", Some(80)) | ("https", Some(443))
    );
    Ok(match url.port() {
        Some(port) if !default_port => format!("{host}:{port}"),
        _ => host,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_require_reviewable_timestamp_fields() {
        assert!(parse_clips(Some(
            r#"[{"start":"00:10","end":"00:30","title":"Hook","caption":"A caption"}]"#
        ))
        .is_ok());
        assert!(parse_clips(Some(r#"[{"title":"Missing timestamps"}]"#)).is_err());
    }

    #[test]
    fn community_host_preserves_non_default_port() {
        assert_eq!(
            community_host("https://Hub.MKIdeas.org/").unwrap(),
            "hub.mkideas.org"
        );
        assert_eq!(
            community_host("http://localhost:3000").unwrap(),
            "localhost:3000"
        );
    }
}
