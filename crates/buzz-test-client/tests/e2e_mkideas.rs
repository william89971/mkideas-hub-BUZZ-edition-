//! Relay-backed NIP-MK shared-state acceptance tests.
//!
//! Start a local relay with PostgreSQL and Redis, then run:
//!
//! ```text
//! cargo test -p buzz-test-client --test e2e_mkideas -- --ignored --nocapture
//! ```

use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use buzz_core::kind::{
    KIND_MK_AGENT_PROPOSAL, KIND_MK_APPROVAL, KIND_MK_APPROVAL_ACTION, KIND_MK_PERSON,
};
use buzz_test_client::{BuzzTestClient, RelayMessage};
use nostr::{Event, EventBuilder, Filter, Keys, Kind, Tag};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn relay_ws_url() -> String {
    std::env::var("RELAY_URL").unwrap_or_else(|_| "ws://localhost:3000".to_string())
}

fn relay_http_url() -> String {
    relay_ws_url()
        .replace("wss://", "https://")
        .replace("ws://", "http://")
        .trim_end_matches('/')
        .to_string()
}

fn relay_authority() -> String {
    let url = url::Url::parse(&relay_http_url()).expect("relay HTTP URL");
    url[url::Position::BeforeHost..url::Position::AfterPort].to_string()
}

async fn database_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://buzz:buzz_dev@localhost:5432/buzz".to_string());
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect to local Buzz database")
}

async fn seed_human(pool: &PgPool, keys: &Keys, role: &str) -> Uuid {
    let authority = relay_authority();
    let community_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO communities (id, host) VALUES ($1, $2) \
         ON CONFLICT (lower(host)) DO NOTHING",
    )
    .bind(community_id)
    .bind(&authority)
    .execute(pool)
    .await
    .expect("seed test community");
    let community_id: Uuid =
        sqlx::query("SELECT id FROM communities WHERE lower(host) = lower($1)")
            .bind(&authority)
            .fetch_one(pool)
            .await
            .expect("load test community")
            .get("id");
    sqlx::query(
        "INSERT INTO relay_members (community_id, pubkey, role, added_by) \
         VALUES ($1, $2, $3, NULL) \
         ON CONFLICT (community_id, pubkey) DO UPDATE SET role = $3, updated_at = now()",
    )
    .bind(community_id)
    .bind(keys.public_key().to_hex())
    .bind(role)
    .execute(pool)
    .await
    .expect("seed MK Ideas human");
    community_id
}

async fn seed_agent_service(pool: &PgPool, community_id: Uuid, owner: &Keys, service: &Keys) {
    let owner_pubkey = owner.public_key().to_bytes();
    let service_pubkey = service.public_key().to_bytes();
    sqlx::query(
        "INSERT INTO users (community_id, pubkey) VALUES ($1, $2) \
         ON CONFLICT (community_id, pubkey) DO NOTHING",
    )
    .bind(community_id)
    .bind(owner_pubkey.as_slice())
    .execute(pool)
    .await
    .expect("seed service owner identity");
    sqlx::query(
        "INSERT INTO users (community_id, pubkey, agent_owner_pubkey) VALUES ($1, $2, $3) \
         ON CONFLICT (community_id, pubkey) DO UPDATE SET agent_owner_pubkey = $3",
    )
    .bind(community_id)
    .bind(service_pubkey.as_slice())
    .bind(owner_pubkey.as_slice())
    .execute(pool)
    .await
    .expect("seed owned agent identity");
    sqlx::query(
        "INSERT INTO mk_service_grants (community_id, service_pubkey, owner_pubkey, purpose, persona, allowed_event_kinds, allowed_target_kinds, expires_at) \
         VALUES ($1, $2, $3, 'agent', 'guest-researcher', $4, $5, now() + interval '1 hour') \
         ON CONFLICT (community_id, service_pubkey, purpose, persona, dataset_sha256) \
         DO UPDATE SET allowed_event_kinds = $4, allowed_target_kinds = $5, expires_at = now() + interval '1 hour', revoked_at = NULL",
    )
    .bind(community_id)
    .bind(service_pubkey.as_slice())
    .bind(owner_pubkey.as_slice())
    .bind(vec![KIND_MK_AGENT_PROPOSAL as i32])
    .bind(vec![KIND_MK_PERSON as i32])
    .execute(pool)
    .await
    .expect("seed narrow guest-researcher capability");
}

fn guest_event(
    keys: &Keys,
    entity_id: Uuid,
    version: i64,
    previous: Option<&Event>,
    status: &str,
) -> Event {
    let mut tags = vec![
        Tag::parse(["d", &entity_id.to_string()]).expect("d tag"),
        Tag::parse(["h", &relay_authority()]).expect("h tag"),
        Tag::parse(["version", &version.to_string()]).expect("version tag"),
        Tag::parse(["status", status]).expect("status tag"),
    ];
    if let Some(previous) = previous {
        tags.push(Tag::parse(["prev", &previous.id.to_hex()]).expect("prev tag"));
    }
    EventBuilder::new(
        Kind::Custom(KIND_MK_PERSON as u16),
        json!({
            "schema_version": 2,
            "record_type": "person",
            "entity_id": entity_id,
            "version": version,
            "status": status,
            "name": "Relay Acceptance Guest",
            "do_not_contact": false,
            "source": "relay-acceptance",
            "provenance": {"type": "test", "ref": entity_id}
        })
        .to_string(),
    )
    .tags(tags)
    .sign_with_keys(keys)
    .expect("sign MK Ideas guest")
}

fn agent_proposal_event(service: &Keys, guest_id: Uuid, guest: &Event, proposal_id: Uuid) -> Event {
    let guest_event_id = guest.id.to_hex();
    EventBuilder::new(
        Kind::Custom(KIND_MK_AGENT_PROPOSAL as u16),
        json!({
            "schema_version": 2,
            "proposal_id": proposal_id,
            "proposal_version": 1,
            "run_id": Uuid::new_v4(),
            "persona": "guest-researcher",
            "persona_id": "guest-researcher",
            "agent": "guest-researcher",
            "proposal_type": "guest-research",
            "target_id": guest_id,
            "target_kind": KIND_MK_PERSON,
            "target_event_id": guest_event_id,
            "target_version": 2,
            "target": {
                "id": guest_id,
                "kind": KIND_MK_PERSON,
                "event_id": guest_event_id,
                "version": 2
            },
            "input_hash": "11".repeat(32),
            "input_event_ids": [guest_event_id],
            "template_version": "relay-acceptance-v1",
            "summary": "Synthetic sourced guest research draft awaiting human review.",
            "provenance": [{
                "source_id": Uuid::new_v4().to_string(),
                "source_type": "relay-acceptance-fixture",
                "title": "Synthetic guest research source",
                "locator": "fixture://guest-research",
                "retrieved_at": "2026-08-30T00:00:00Z",
                "sha256": "22".repeat(32)
            }],
            "review": {"state": "pending", "human_action_required": true},
            "status": "proposed"
        })
        .to_string(),
    )
    .tags(vec![
        Tag::parse(["h", &relay_authority()]).expect("h tag"),
        Tag::parse(["status", "proposed"]).expect("status tag"),
        Tag::parse(["target", &KIND_MK_PERSON.to_string(), &guest_id.to_string()])
            .expect("target tag"),
    ])
    .sign_with_keys(service)
    .expect("sign agent proposal")
}

fn approval_request_event(
    human: &Keys,
    guest_id: Uuid,
    guest: &Event,
    proposal_id: Uuid,
    proposal: &Event,
) -> (Uuid, Event) {
    let approval_id = Uuid::new_v4();
    let guest_event_id = guest.id.to_hex();
    let proposal_event_id = proposal.id.to_hex();
    let event = EventBuilder::new(
        Kind::Custom(KIND_MK_APPROVAL as u16),
        json!({
            "schema_version": 2,
            "record_type": "approval",
            "entity_id": approval_id,
            "version": 1,
            "status": "pending",
            "target_id": guest_id,
            "target_kind": KIND_MK_PERSON,
            "target_event_id": guest_event_id,
            "target_version": 2,
            "proposal_id": proposal_id,
            "proposal_event_id": proposal_event_id,
            "source": "relay-acceptance",
            "provenance": {"type": "test", "ref": proposal_event_id}
        })
        .to_string(),
    )
    .tags(vec![
        Tag::parse(["d", &approval_id.to_string()]).expect("d tag"),
        Tag::parse(["h", &relay_authority()]).expect("h tag"),
        Tag::parse(["version", "1"]).expect("version tag"),
        Tag::parse(["status", "pending"]).expect("status tag"),
        Tag::parse([
            "target",
            &KIND_MK_PERSON.to_string(),
            &guest_id.to_string(),
            &guest_event_id,
        ])
        .expect("target tag"),
        Tag::parse(["proposal", &proposal_id.to_string(), &proposal_event_id])
            .expect("proposal tag"),
    ])
    .sign_with_keys(human)
    .expect("sign approval request");
    (approval_id, event)
}

fn approval_action_event(
    human: &Keys,
    approval_id: Uuid,
    approval: &Event,
    guest: (Uuid, &Event),
    proposal: (Uuid, &Event),
    result: &Event,
) -> Event {
    let (guest_id, guest) = guest;
    let (proposal_id, proposal) = proposal;
    EventBuilder::new(
        Kind::Custom(KIND_MK_APPROVAL_ACTION as u16),
        json!({
            "schema_version": 2,
            "action_id": Uuid::new_v4(),
            "approval_id": approval_id,
            "approval_event_id": approval.id.to_hex(),
            "target_id": guest_id,
            "target_kind": KIND_MK_PERSON,
            "target_event_id": guest.id.to_hex(),
            "target_version": 2,
            "proposal_id": proposal_id,
            "proposal_event_id": proposal.id.to_hex(),
            "decision": "approved",
            "reason": "Human reviewed the sourced research and approved the proposed state change.",
            "result_event": result
        })
        .to_string(),
    )
    .tags(vec![Tag::parse(["h", &relay_authority()]).expect("h tag")])
    .sign_with_keys(human)
    .expect("sign approval action")
}

fn nip98_post_header(keys: &Keys, url: &str, body: &str) -> String {
    let digest = Sha256::digest(body.as_bytes());
    let event = EventBuilder::new(Kind::Custom(27_235), "")
        .tags(vec![
            Tag::parse(["u", url]).expect("u tag"),
            Tag::parse(["method", "POST"]).expect("method tag"),
            Tag::parse(["payload", &hex::encode(digest)]).expect("payload tag"),
            Tag::parse(["nonce", &Uuid::new_v4().to_string()]).expect("nonce tag"),
        ])
        .sign_with_keys(keys)
        .expect("sign NIP-98 request");
    format!(
        "Nostr {}",
        BASE64.encode(serde_json::to_string(&event).expect("serialize NIP-98 request"))
    )
}

async fn projection_query(keys: &Keys, filter: Value) -> Value {
    let endpoint = format!("{}/query", relay_http_url());
    let body = json!([filter]).to_string();
    let response = reqwest::Client::new()
        .post(&endpoint)
        .header("Authorization", nip98_post_header(keys, &endpoint, &body))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .expect("POST projection query");
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.expect("parse projection response")
}

#[tokio::test]
#[ignore]
async fn two_humans_share_one_head_with_live_updates_and_paginated_history() {
    let owner = Keys::generate();
    let admin = Keys::generate();
    let pool = database_pool().await;
    let owner_community = seed_human(&pool, &owner, "owner").await;
    let admin_community = seed_human(&pool, &admin, "admin").await;
    assert_eq!(owner_community, admin_community);

    let mut owner_client = BuzzTestClient::connect(&relay_ws_url(), &owner)
        .await
        .expect("owner connects");
    let mut admin_client = BuzzTestClient::connect(&relay_ws_url(), &admin)
        .await
        .expect("admin connects");

    let guest_id = Uuid::new_v4();
    let created = guest_event(&owner, guest_id, 1, None, "prospect");
    let create_result = owner_client
        .send_event(created.clone())
        .await
        .expect("owner submits guest");
    assert!(create_result.accepted, "{}", create_result.message);

    let subscription_id = format!("mk-live-{}", Uuid::new_v4());
    let authority = relay_authority();
    owner_client
        .subscribe(
            &subscription_id,
            vec![Filter::new()
                .kind(Kind::Custom(KIND_MK_PERSON as u16))
                .custom_tags(
                    nostr::SingleLetterTag::lowercase(nostr::Alphabet::H),
                    [authority.as_str()],
                )],
        )
        .await
        .expect("subscribe to MK guest updates");
    loop {
        match owner_client
            .recv_event(Duration::from_secs(5))
            .await
            .expect("subscription snapshot response")
        {
            RelayMessage::Eose {
                subscription_id: received,
            } if received == subscription_id => break,
            RelayMessage::Event {
                subscription_id: received,
                ..
            } if received == subscription_id => {}
            RelayMessage::Closed {
                subscription_id: received,
                message,
            } if received == subscription_id => {
                panic!("subscription closed before EOSE: {message}")
            }
            other => panic!("unexpected subscription snapshot response: {other:?}"),
        }
    }

    let stale = guest_event(&admin, guest_id, 1, None, "researching");
    let stale_result = admin_client
        .send_event(stale)
        .await
        .expect("relay returns stale-write result");
    assert!(
        !stale_result.accepted,
        "a second version-one head must not win"
    );

    let updated = guest_event(&admin, guest_id, 2, Some(&created), "researching");
    let update_result = admin_client
        .send_event(updated.clone())
        .await
        .expect("admin advances shared guest head");
    assert!(update_result.accepted, "{}", update_result.message);

    let live = owner_client
        .recv_event(Duration::from_secs(5))
        .await
        .expect("owner receives live cross-signer update");
    match live {
        RelayMessage::Event {
            subscription_id: received_subscription,
            event,
        } => {
            assert_eq!(received_subscription, subscription_id);
            assert_eq!(event.id, updated.id);
            assert_eq!(event.pubkey, admin.public_key());
        }
        other => panic!("expected live MK event, got {other:?}"),
    }

    let service = Keys::generate();
    seed_human(&pool, &service, "member").await;
    seed_agent_service(&pool, owner_community, &owner, &service).await;
    let mut service_client = BuzzTestClient::connect(&relay_ws_url(), &service)
        .await
        .expect("narrow agent service connects");
    let proposal_id = Uuid::new_v4();
    let proposal = agent_proposal_event(&service, guest_id, &updated, proposal_id);
    let proposal_result = service_client
        .send_event(proposal.clone())
        .await
        .expect("guest researcher submits proposal");
    assert!(proposal_result.accepted, "{}", proposal_result.message);

    let (approval_id, approval) =
        approval_request_event(&owner, guest_id, &updated, proposal_id, &proposal);
    let approval_result = owner_client
        .send_event(approval.clone())
        .await
        .expect("human submits approval request");
    assert!(approval_result.accepted, "{}", approval_result.message);

    let approved_guest = guest_event(&owner, guest_id, 3, Some(&updated), "ready-to-contact");
    let action = approval_action_event(
        &owner,
        approval_id,
        &approval,
        (guest_id, &updated),
        (proposal_id, &proposal),
        &approved_guest,
    );
    let agent_action = approval_action_event(
        &service,
        approval_id,
        &approval,
        (guest_id, &updated),
        (proposal_id, &proposal),
        &approved_guest,
    );
    let agent_approval_attempt = service_client
        .send_event(agent_action)
        .await
        .expect("relay returns agent approval rejection");
    assert!(
        !agent_approval_attempt.accepted,
        "agent identity must not cross the human approval gate"
    );
    let action_result = owner_client
        .send_event(action.clone())
        .await
        .expect("human submits atomic approval action");
    assert!(action_result.accepted, "{}", action_result.message);

    let decision = sqlx::query(
        "SELECT action_event_id, result_event_id, decision, decided_by \
         FROM mk_approval_decisions WHERE community_id = $1 AND approval_id = $2",
    )
    .bind(owner_community)
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .expect("atomic approval decision projection");
    assert_eq!(
        decision.get::<Vec<u8>, _>("action_event_id"),
        action.id.as_bytes().to_vec()
    );
    assert_eq!(
        decision.get::<Vec<u8>, _>("result_event_id"),
        approved_guest.id.as_bytes().to_vec()
    );
    assert_eq!(decision.get::<String, _>("decision"), "approved");
    assert_eq!(
        decision.get::<Vec<u8>, _>("decided_by"),
        owner.public_key().to_bytes().to_vec()
    );

    let heads = projection_query(
        &owner,
        json!({
            "mk_projection": "heads",
            "kinds": [KIND_MK_PERSON],
            "limit": 200
        }),
    )
    .await;
    let matching_heads = heads["events"]
        .as_array()
        .expect("head events")
        .iter()
        .filter(|event| event["id"] == approved_guest.id.to_hex())
        .count();
    assert_eq!(matching_heads, 1);

    let first_history = projection_query(
        &owner,
        json!({
            "mk_projection": "history",
            "kinds": [KIND_MK_PERSON],
            "#d": [guest_id],
            "limit": 1
        }),
    )
    .await;
    assert_eq!(first_history["events"][0]["id"], approved_guest.id.to_hex());
    assert_eq!(first_history["nextCursor"], "3");

    let second_history = projection_query(
        &owner,
        json!({
            "mk_projection": "history",
            "kinds": [KIND_MK_PERSON],
            "#d": [guest_id],
            "limit": 1,
            "mk_cursor": first_history["nextCursor"]
        }),
    )
    .await;
    assert_eq!(second_history["events"][0]["id"], updated.id.to_hex());

    owner_client
        .close_subscription(&subscription_id)
        .await
        .expect("close MK subscription");
    owner_client.disconnect().await.expect("owner disconnects");
    admin_client.disconnect().await.expect("admin disconnects");
    service_client
        .disconnect()
        .await
        .expect("service disconnects");
}
