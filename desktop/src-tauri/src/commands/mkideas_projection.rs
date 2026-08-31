//! Typed native bridge for the NIP-MK current-head and revision projections.

use std::collections::HashSet;
use std::time::Duration;

use buzz_core_pkg::kind::is_mkideas_state_kind;
use nostr::Event;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

use crate::app_state::AppState;

const DEFAULT_PAGE_LIMIT: u32 = 100;
const MAX_PAGE_LIMIT: u32 = 200;
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Projection {
    Heads,
    History,
}

impl Projection {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "heads" => Ok(Self::Heads),
            "history" => Ok(Self::History),
            _ => Err("projection must be `heads` or `history`".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Heads => "heads",
            Self::History => "history",
        }
    }
}

#[derive(Debug)]
struct ProjectionArgs {
    projection: Projection,
    kinds: Vec<u32>,
    community: String,
    entity_id: Option<Uuid>,
    cursor: Option<String>,
    limit: u32,
}

/// Typed input for an MK Ideas current-head or immutable-history query.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MkIdeasProjectionInput {
    /// Projection to query: `heads` or `history`.
    projection: String,
    /// Registered MK Ideas addressable-state kinds to include.
    kinds: Vec<u32>,
    /// Host-derived community identifier.
    community: String,
    /// Stable entity UUID, required only for history queries.
    entity_id: Option<String>,
    /// Opaque cursor returned by the previous page.
    cursor: Option<String>,
    /// Page size from 1 through 200.
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectionWireResponse {
    events: Vec<Event>,
    next_cursor: Option<String>,
}

/// Signed MK Ideas events plus the opaque cursor for the next page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MkIdeasProjectionResponse {
    /// Relay-signed or human-signed events returned by the projection.
    events: Vec<Event>,
    /// Cursor to pass unchanged to the next request, when another page exists.
    next_cursor: Option<String>,
}

fn validate_args(
    projection: &str,
    kinds: Vec<u32>,
    community: String,
    entity_id: Option<String>,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<ProjectionArgs, String> {
    let projection = Projection::parse(projection)?;
    if kinds.is_empty() {
        return Err("kinds must contain at least one MK Ideas state kind".to_string());
    }
    if kinds.iter().any(|kind| !is_mkideas_state_kind(*kind)) {
        return Err("every kind must be a registered MK Ideas state kind".to_string());
    }
    let unique = kinds.iter().copied().collect::<HashSet<_>>();
    if unique.len() != kinds.len() {
        return Err("kinds must not contain duplicates".to_string());
    }

    let community = community.trim().to_string();
    if community.is_empty() || community.chars().count() > 253 {
        return Err("community must be a non-empty host under 254 characters".to_string());
    }
    if community.chars().any(char::is_whitespace)
        || community.contains('/')
        || community.contains("//")
    {
        return Err("community must be a relay host, not a URL".to_string());
    }

    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(format!("limit must be between 1 and {MAX_PAGE_LIMIT}"));
    }

    let entity_id = match (projection, entity_id) {
        (Projection::Heads, None) => None,
        (Projection::Heads, Some(_)) => {
            return Err("entityId is only valid for history queries".to_string());
        }
        (Projection::History, Some(value)) => Some(
            Uuid::parse_str(value.trim())
                .map_err(|_| "entityId must be a UUID for history queries".to_string())?,
        ),
        (Projection::History, None) => {
            return Err("entityId is required for history queries".to_string());
        }
    };

    if projection == Projection::History && kinds.len() != 1 {
        return Err("history queries require exactly one kind".to_string());
    }
    if let Some(value) = cursor.as_deref() {
        validate_cursor(projection, &kinds, value)?;
    }

    Ok(ProjectionArgs {
        projection,
        kinds,
        community,
        entity_id,
        cursor,
        limit,
    })
}

fn validate_cursor(projection: Projection, kinds: &[u32], cursor: &str) -> Result<(), String> {
    match projection {
        Projection::Heads => {
            let (kind, entity_id) = cursor
                .split_once(':')
                .ok_or_else(|| "heads cursor must be `<kind>:<uuid>`".to_string())?;
            let kind = kind
                .parse::<u32>()
                .map_err(|_| "heads cursor kind must be an integer".to_string())?;
            if !kinds.contains(&kind) {
                return Err("heads cursor kind must be present in kinds".to_string());
            }
            Uuid::parse_str(entity_id)
                .map_err(|_| "heads cursor entity must be a UUID".to_string())?;
        }
        Projection::History => {
            let version = cursor
                .parse::<i64>()
                .map_err(|_| "history cursor must be a version number".to_string())?;
            if version < 1 {
                return Err("history cursor must be a positive version number".to_string());
            }
        }
    }
    Ok(())
}

fn request_body(args: &ProjectionArgs) -> Result<Vec<u8>, String> {
    let mut filter = json!({
        "mk_projection": args.projection.as_str(),
        "kinds": &args.kinds,
        "#h": [&args.community],
        "limit": args.limit,
    });
    if let Some(entity_id) = args.entity_id {
        filter["#d"] = json!([entity_id]);
    }
    if let Some(cursor) = args.cursor.as_deref() {
        filter["mk_cursor"] = json!(cursor);
    }
    serde_json::to_vec(&[filter]).map_err(|error| format!("query serialization failed: {error}"))
}

fn parse_response(
    value: Value,
    args: &ProjectionArgs,
) -> Result<MkIdeasProjectionResponse, String> {
    let response: ProjectionWireResponse = serde_json::from_value(value)
        .map_err(|error| format!("invalid MK Ideas projection response: {error}"))?;
    if let Some(cursor) = response.next_cursor.as_deref() {
        validate_cursor(args.projection, &args.kinds, cursor)
            .map_err(|error| format!("invalid nextCursor: {error}"))?;
    }

    for event in &response.events {
        if !event.verify_id() || !event.verify_signature() {
            return Err("MK Ideas projection returned an invalid event signature".to_string());
        }
        let kind = u32::from(event.kind.as_u16());
        if !args.kinds.contains(&kind) {
            return Err(format!(
                "MK Ideas projection returned unexpected kind {kind}"
            ));
        }
        let envelope = buzz_core_pkg::mkideas::validate_state_event(event)
            .map_err(|error| format!("MK Ideas projection returned invalid state: {error}"))?;
        if envelope.community != args.community {
            return Err("MK Ideas projection crossed the requested community".to_string());
        }
        if args
            .entity_id
            .is_some_and(|entity_id| envelope.entity_id != entity_id)
        {
            return Err("MK Ideas history returned a different entity".to_string());
        }
    }

    Ok(MkIdeasProjectionResponse {
        events: response.events,
        next_cursor: response.next_cursor,
    })
}

/// Query the relay's NIP-MK current-head or immutable-history projection.
#[tauri::command]
pub async fn query_mkideas_projection(
    state: State<'_, AppState>,
    input: MkIdeasProjectionInput,
) -> Result<MkIdeasProjectionResponse, String> {
    let args = validate_args(
        &input.projection,
        input.kinds,
        input.community,
        input.entity_id,
        input.cursor,
        input.limit,
    )?;
    let body = request_body(&args)?;
    crate::relay_admission::wait_for_rate_limit().await;
    let url = format!(
        "{}/query",
        crate::relay::relay_api_base_url_with_override(&state)
    );
    let auth = crate::relay::build_nip98_auth_header(&Method::POST, &url, &body, &state)?;
    let response = state
        .http_client
        .post(&url)
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .timeout(QUERY_TIMEOUT)
        .body(body)
        .send()
        .await
        .map_err(|error| crate::relay::classify_request_error(&error))?;
    if !response.status().is_success() {
        return Err(crate::relay::relay_error_message(response).await);
    }
    let value = crate::relay::parse_json_response(response).await?;
    parse_response(value, &args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use buzz_core_pkg::kind::{KIND_MK_CONTENT, KIND_MK_PERSON};
    use nostr::{EventBuilder, Keys, Kind, Tag};

    fn signed_person(community: &str, entity_id: Uuid) -> Event {
        let entity_id_tag = entity_id.to_string();
        EventBuilder::new(
            Kind::from(KIND_MK_PERSON as u16),
            json!({
                "schema_version": 2,
                "record_type": "person",
                "entity_id": entity_id,
                "version": 1,
                "status": "prospect",
                "name": "Native bridge fixture",
                "do_not_contact": false,
                "source": "tauri-test",
                "provenance": {"type": "test", "ref": entity_id}
            })
            .to_string(),
        )
        .tags([
            Tag::parse(["d", entity_id_tag.as_str()]).expect("d tag"),
            Tag::parse(["h", community]).expect("h tag"),
            Tag::parse(["version", "1"]).expect("version tag"),
            Tag::parse(["status", "prospect"]).expect("status tag"),
        ])
        .sign_with_keys(&Keys::generate())
        .expect("signed person")
    }

    #[test]
    fn heads_request_uses_one_extended_filter_and_default_limit() {
        let args = validate_args(
            "heads",
            vec![KIND_MK_PERSON, KIND_MK_CONTENT],
            "hub.mkideas.org".to_string(),
            None,
            Some(format!("{KIND_MK_PERSON}:{}", Uuid::nil())),
            None,
        )
        .expect("valid heads args");
        let body: Value = serde_json::from_slice(&request_body(&args).expect("request body"))
            .expect("request JSON");
        assert_eq!(body.as_array().map(Vec::len), Some(1));
        assert_eq!(body[0]["mk_projection"], "heads");
        assert_eq!(body[0]["#h"], json!(["hub.mkideas.org"]));
        assert_eq!(body[0]["limit"], DEFAULT_PAGE_LIMIT);
        assert!(body[0].get("#d").is_none());
    }

    #[test]
    fn history_request_and_projection_constraints_are_validated_locally() {
        let entity_id = Uuid::new_v4();
        let args = validate_args(
            "history",
            vec![KIND_MK_PERSON],
            "hub.mkideas.org".to_string(),
            Some(entity_id.to_string()),
            Some("9".to_string()),
            Some(25),
        )
        .expect("valid history args");
        let body: Value = serde_json::from_slice(&request_body(&args).expect("request body"))
            .expect("request JSON");
        assert_eq!(body[0]["#d"], json!([entity_id]));
        assert_eq!(body[0]["mk_cursor"], "9");

        assert!(validate_args(
            "history",
            vec![KIND_MK_PERSON],
            "hub.mkideas.org".to_string(),
            None,
            None,
            None,
        )
        .is_err());
        assert!(validate_args(
            "history",
            vec![KIND_MK_PERSON, KIND_MK_CONTENT],
            "hub.mkideas.org".to_string(),
            Some(entity_id.to_string()),
            None,
            None,
        )
        .is_err());
        assert!(validate_args(
            "heads",
            vec![KIND_MK_PERSON],
            "https://hub.mkideas.org".to_string(),
            None,
            None,
            Some(0),
        )
        .is_err());
        assert!(validate_args(
            "heads",
            vec![99],
            "hub.mkideas.org".to_string(),
            None,
            None,
            None,
        )
        .is_err());

        let input: MkIdeasProjectionInput = serde_json::from_value(json!({
            "projection": "history",
            "kinds": [KIND_MK_PERSON],
            "community": "hub.mkideas.org",
            "entityId": entity_id,
            "cursor": "9",
            "limit": 25
        }))
        .expect("camelCase input");
        assert_eq!(
            input.entity_id.as_deref(),
            Some(entity_id.to_string().as_str())
        );
        assert!(serde_json::from_value::<MkIdeasProjectionInput>(json!({
            "projection": "heads",
            "kinds": [KIND_MK_PERSON],
            "community": "hub.mkideas.org",
            "unexpected": true
        }))
        .is_err());
    }

    #[test]
    fn response_parser_returns_verified_signed_events_and_cursor() {
        let entity_id = Uuid::new_v4();
        let args = validate_args(
            "history",
            vec![KIND_MK_PERSON],
            "hub.mkideas.org".to_string(),
            Some(entity_id.to_string()),
            None,
            Some(1),
        )
        .expect("valid args");
        let event = signed_person("hub.mkideas.org", entity_id);
        let parsed = parse_response(json!({"events": [&event], "nextCursor": "1"}), &args)
            .expect("valid projection response");
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.next_cursor.as_deref(), Some("1"));

        let wrong_community = signed_person("other.mkideas.org", entity_id);
        assert!(parse_response(
            json!({"events": [wrong_community], "nextCursor": null}),
            &args,
        )
        .is_err());
        assert!(parse_response(json!([&event]), &args).is_err());
    }
}
