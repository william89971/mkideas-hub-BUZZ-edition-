//! PostgreSQL contract tests for the MK Ideas Phase 1 persistence boundary.
//!
//! These tests intentionally exercise the public `buzz-db` API rather than
//! duplicating its SQL. A local migrated PostgreSQL database is required.

use buzz_core::{
    kind::{
        KIND_MK_AGENT_PROPOSAL, KIND_MK_APPROVAL, KIND_MK_APPROVAL_ACTION, KIND_MK_CONTENT,
        KIND_MK_PERSON,
    },
    mkideas::{validate_agent_proposal, validate_approval_action, validate_state_event},
    CommunityId,
};
use buzz_db::{migration, Db, DbError};
use nostr::{Event, EventBuilder, Keys, Kind, Tag};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const TEST_DB_URL: &str = "postgres://buzz:buzz_dev@localhost:5432/buzz"; // sadscan:disable np.postgres.1 -- local test-only credentials

async fn setup_pool() -> PgPool {
    let database_url = std::env::var("BUZZ_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| TEST_DB_URL.to_owned());
    let pool = PgPool::connect(&database_url)
        .await
        .expect("connect to test PostgreSQL");
    migration::run_migrations(&pool)
        .await
        .expect("apply current migrations");
    pool
}

async fn create_community(pool: &PgPool, label: &str) -> (CommunityId, String) {
    let id = Uuid::new_v4();
    let host = format!("{label}-{}.mkideas.test", id.simple());
    sqlx::query("INSERT INTO communities (id, host) VALUES ($1, $2)")
        .bind(id)
        .bind(&host)
        .execute(pool)
        .await
        .expect("insert isolated test community");
    (CommunityId::from_uuid(id), host)
}

async fn delete_community(pool: &PgPool, community: CommunityId) {
    for (table, statement) in [
        (
            "mk_notification_delivery_dedupe",
            "DELETE FROM mk_notification_delivery_dedupe WHERE community_id = $1",
        ),
        (
            "mk_notification_preferences",
            "DELETE FROM mk_notification_preferences WHERE community_id = $1",
        ),
        (
            "mk_identity_successors",
            "DELETE FROM mk_identity_successors WHERE community_id = $1",
        ),
        (
            "mk_nip49_recovery_bundles",
            "DELETE FROM mk_nip49_recovery_bundles WHERE community_id = $1",
        ),
        (
            "mk_device_recovery_requests",
            "DELETE FROM mk_device_recovery_requests WHERE community_id = $1",
        ),
        (
            "mk_device_enrollment_challenges",
            "DELETE FROM mk_device_enrollment_challenges WHERE community_id = $1",
        ),
        (
            "mk_device_grants",
            "DELETE FROM mk_device_grants WHERE community_id = $1",
        ),
        (
            "mk_migration_items",
            "DELETE FROM mk_migration_items WHERE community_id = $1",
        ),
        (
            "mk_approval_decisions",
            "DELETE FROM mk_approval_decisions WHERE community_id = $1",
        ),
        (
            "mk_entity_heads",
            "DELETE FROM mk_entity_heads WHERE community_id = $1",
        ),
        (
            "mk_entity_revisions",
            "DELETE FROM mk_entity_revisions WHERE community_id = $1",
        ),
        (
            "mk_service_grants",
            "DELETE FROM mk_service_grants WHERE community_id = $1",
        ),
        ("events", "DELETE FROM events WHERE community_id = $1"),
        (
            "relay_members",
            "DELETE FROM relay_members WHERE community_id = $1",
        ),
        ("users", "DELETE FROM users WHERE community_id = $1"),
    ] {
        sqlx::query(statement)
            .bind(community.as_uuid())
            .execute(pool)
            .await
            .unwrap_or_else(|error| panic!("delete isolated rows from {table}: {error}"));
    }
    sqlx::query("DELETE FROM communities WHERE id = $1")
        .bind(community.as_uuid())
        .execute(pool)
        .await
        .expect("delete isolated test community");
}

fn tag(values: impl IntoIterator<Item = String>) -> Tag {
    Tag::parse(values).expect("valid test tag")
}

fn signed_event(
    keys: &Keys,
    kind: u32,
    content: Value,
    tags: impl IntoIterator<Item = Vec<String>>,
) -> Event {
    EventBuilder::new(Kind::from(kind as u16), content.to_string())
        .tags(tags.into_iter().map(tag))
        .sign_with_keys(keys)
        .expect("sign test event")
}

// Keeping the envelope fields explicit makes each database fixture auditable against NIP-MK.
#[allow(clippy::too_many_arguments)]
fn state_event(
    keys: &Keys,
    kind: u32,
    host: &str,
    entity_id: Uuid,
    version: i64,
    previous: Option<&Event>,
    status: &str,
    content: Value,
    extra_tags: impl IntoIterator<Item = Vec<String>>,
) -> Event {
    let mut tags = vec![
        vec!["d".to_string(), entity_id.to_string()],
        vec!["h".to_string(), host.to_string()],
        vec!["version".to_string(), version.to_string()],
        vec!["status".to_string(), status.to_string()],
    ];
    if let Some(previous) = previous {
        tags.push(vec!["prev".to_string(), previous.id.to_hex()]);
    }
    tags.extend(extra_tags);
    signed_event(keys, kind, content, tags)
}

// Person fixtures intentionally expose every version and DNC control independently.
#[allow(clippy::too_many_arguments)]
fn person_event(
    keys: &Keys,
    host: &str,
    entity_id: Uuid,
    schema_version: u64,
    version: i64,
    previous: Option<&Event>,
    status: &str,
    do_not_contact: bool,
    dnc_override_reason: Option<&str>,
) -> Event {
    let mut content = if schema_version == 1 {
        json!({
            "schema_version": 1,
            "record_type": "person",
            "entity_id": entity_id,
            "version": version,
            "status": status,
            "name": format!("Guest {entity_id}"),
            "do_not_contact": do_not_contact
        })
    } else {
        json!({
            "schema_version": 2,
            "record_type": "person",
            "entity_id": entity_id,
            "version": version,
            "status": status,
            "name": format!("Guest {entity_id}"),
            "do_not_contact": do_not_contact,
            "source": "phase1-test",
            "provenance": {"type": "test", "ref": entity_id}
        })
    };
    if let Some(reason) = dnc_override_reason {
        content["dnc_override_reason"] = Value::String(reason.to_string());
    }
    state_event(
        keys,
        KIND_MK_PERSON,
        host,
        entity_id,
        version,
        previous,
        status,
        content,
        [],
    )
}

fn content_event(
    keys: &Keys,
    host: &str,
    entity_id: Uuid,
    version: i64,
    previous: Option<&Event>,
    status: &str,
) -> Event {
    state_event(
        keys,
        KIND_MK_CONTENT,
        host,
        entity_id,
        version,
        previous,
        status,
        json!({
            "schema_version": 2,
            "record_type": "content",
            "entity_id": entity_id,
            "version": version,
            "status": status,
            "title": "Phase 1 approval target",
            "source": "phase1-test",
            "provenance": {"type": "test", "ref": entity_id}
        }),
        [],
    )
}

async fn insert_head(db: &Db, community: CommunityId, event: &Event) -> bool {
    let envelope = validate_state_event(event).expect("valid MK state fixture");
    db.replace_mkideas_entity_head(community, event, &envelope)
        .await
        .expect("store MK state fixture")
        .1
}

fn entity_id(event: &Event) -> Uuid {
    event
        .tags
        .iter()
        .find_map(|tag| {
            let values = tag.as_slice();
            (values.first().map(String::as_str) == Some("d"))
                .then(|| values.get(1))
                .flatten()
                .and_then(|value| Uuid::parse_str(value).ok())
        })
        .expect("state event d tag")
}

struct ApprovalFixture {
    community: CommunityId,
    host: String,
    human: Keys,
    target_id: Uuid,
    target: Event,
    proposal_id: Uuid,
    proposal: Event,
    approval_id: Uuid,
    approval: Event,
}

async fn create_approval_fixture(pool: &PgPool, db: &Db, label: &str) -> ApprovalFixture {
    let (community, host) = create_community(pool, label).await;
    let human = Keys::generate();
    let target_id = Uuid::new_v4();
    let target = content_event(&human, &host, target_id, 1, None, "in-review");
    assert!(insert_head(db, community, &target).await);

    let proposal_id = Uuid::new_v4();
    let proposal = signed_event(
        &Keys::generate(),
        KIND_MK_AGENT_PROPOSAL,
        json!({
            "schema_version": 2,
            "proposal_id": proposal_id,
            "proposal_version": 1,
            "run_id": Uuid::new_v4(),
            "target_id": target_id,
            "target_kind": KIND_MK_CONTENT,
            "target_event_id": target.id.to_hex(),
            "target_version": 1,
            "agent": "Content/Clip Copilot",
            "persona": "content-clip-copilot",
            "proposal_type": "content_review",
            "summary": "Approve the reviewed content item.",
            "input_hash": "11".repeat(32),
            "input_event_ids": [target.id.to_hex()],
            "template_version": "phase1-test-v1",
            "provenance": ["synthetic phase1 fixture"],
            "status": "proposed"
        }),
        [
            vec!["h".to_string(), host.clone()],
            vec!["status".to_string(), "proposed".to_string()],
            vec![
                "target".to_string(),
                KIND_MK_CONTENT.to_string(),
                target_id.to_string(),
            ],
        ],
    );
    validate_agent_proposal(&proposal).expect("valid proposal fixture");
    assert!(
        db.insert_event(community, &proposal, None)
            .await
            .expect("insert proposal")
            .1
    );

    let approval_id = Uuid::new_v4();
    let approval = state_event(
        &human,
        KIND_MK_APPROVAL,
        &host,
        approval_id,
        1,
        None,
        "pending",
        json!({
            "schema_version": 2,
            "record_type": "approval",
            "entity_id": approval_id,
            "version": 1,
            "status": "pending",
            "target_id": target_id,
            "target_kind": KIND_MK_CONTENT,
            "target_event_id": target.id.to_hex(),
            "target_version": 1,
            "proposal_id": proposal_id,
            "proposal_event_id": proposal.id.to_hex(),
            "source": "phase1-test",
            "provenance": {"type": "test", "ref": proposal.id.to_hex()}
        }),
        [
            vec![
                "target".to_string(),
                KIND_MK_CONTENT.to_string(),
                target_id.to_string(),
                target.id.to_hex(),
            ],
            vec![
                "proposal".to_string(),
                proposal_id.to_string(),
                proposal.id.to_hex(),
            ],
        ],
    );
    assert!(insert_head(db, community, &approval).await);

    ApprovalFixture {
        community,
        host,
        human,
        target_id,
        target,
        proposal_id,
        proposal,
        approval_id,
        approval,
    }
}

fn approval_action(
    fixture: &ApprovalFixture,
    decision: &str,
    result_event: Option<&Event>,
) -> Event {
    let mut content = json!({
        "schema_version": 2,
        "action_id": Uuid::new_v4(),
        "approval_id": fixture.approval_id,
        "approval_event_id": fixture.approval.id.to_hex(),
        "target_id": fixture.target_id,
        "target_kind": KIND_MK_CONTENT,
        "target_event_id": fixture.target.id.to_hex(),
        "target_version": 1,
        "proposal_id": fixture.proposal_id,
        "proposal_event_id": fixture.proposal.id.to_hex(),
        "decision": decision,
        "reason": format!("Phase 1 test decision: {decision}")
    });
    if let Some(result_event) = result_event {
        content["result_event"] = serde_json::to_value(result_event).expect("serialize result");
    }
    let action = signed_event(
        &fixture.human,
        KIND_MK_APPROVAL_ACTION,
        content,
        [vec!["h".to_string(), fixture.host.clone()]],
    );
    validate_approval_action(&action).expect("valid approval action fixture");
    action
}

async fn event_exists(db: &Db, community: CommunityId, event: &Event) -> bool {
    db.get_event_by_id(community, event.id.as_bytes())
        .await
        .expect("query event by id")
        .is_some()
}

#[tokio::test]
async fn heads_preserve_history_page_stably_and_accept_v1_and_v2() {
    let pool = setup_pool().await;
    let db = Db::from_pool(pool.clone());
    let (community, host) = create_community(&pool, "mk-heads").await;
    let (other_community, _) = create_community(&pool, "mk-heads-other").await;

    let entity_one = Uuid::from_u128(1);
    let original = person_event(
        &Keys::generate(),
        &host,
        entity_one,
        1,
        1,
        None,
        "potential",
        false,
        None,
    );
    assert!(insert_head(&db, community, &original).await);

    let first_edit = person_event(
        &Keys::generate(),
        &host,
        entity_one,
        1,
        2,
        Some(&original),
        "researching",
        false,
        None,
    );
    let second_edit = person_event(
        &Keys::generate(),
        &host,
        entity_one,
        1,
        2,
        Some(&original),
        "researching",
        false,
        None,
    );
    let first_envelope = validate_state_event(&first_edit).expect("first concurrent edit");
    let second_envelope = validate_state_event(&second_edit).expect("second concurrent edit");
    let (first_result, second_result) = tokio::join!(
        db.replace_mkideas_entity_head(community, &first_edit, &first_envelope),
        db.replace_mkideas_entity_head(community, &second_edit, &second_envelope),
    );
    let concurrent_results = [first_result, second_result];
    assert_eq!(
        concurrent_results
            .iter()
            .filter(|result| result.is_ok())
            .count(),
        1
    );
    assert_eq!(
        concurrent_results
            .iter()
            .filter(|result| matches!(result, Err(DbError::MkIdeasConflict { .. })))
            .count(),
        1
    );
    let winning_edit = if event_exists(&db, community, &first_edit).await {
        first_edit
    } else {
        second_edit
    };

    let entity_two = Uuid::from_u128(2);
    let schema_v2 = person_event(
        &Keys::generate(),
        &host,
        entity_two,
        2,
        1,
        None,
        "prospect",
        false,
        None,
    );
    assert!(insert_head(&db, community, &schema_v2).await);
    let entity_three = Uuid::from_u128(3);
    let third = person_event(
        &Keys::generate(),
        &host,
        entity_three,
        2,
        1,
        None,
        "prospect",
        false,
        None,
    );
    assert!(insert_head(&db, community, &third).await);

    let (first_page, cursor) = db
        .query_mkideas_heads(community, &[KIND_MK_PERSON], None, 2)
        .await
        .expect("first head page");
    assert_eq!(
        first_page
            .iter()
            .map(|stored| entity_id(&stored.event))
            .collect::<Vec<_>>(),
        vec![entity_one, entity_two]
    );
    let cursor = cursor.expect("more head rows");

    let after_page_update = person_event(
        &Keys::generate(),
        &host,
        entity_one,
        1,
        3,
        Some(&winning_edit),
        "research_ready",
        false,
        None,
    );
    assert!(insert_head(&db, community, &after_page_update).await);

    let (second_page, final_cursor) = db
        .query_mkideas_heads(community, &[KIND_MK_PERSON], Some(&cursor), 2)
        .await
        .expect("second head page");
    assert_eq!(second_page.len(), 1);
    assert_eq!(entity_id(&second_page[0].event), entity_three);
    assert!(final_cursor.is_none());

    let (history_one, cursor_one) = db
        .query_mkideas_history(community, KIND_MK_PERSON, entity_one, None, 1)
        .await
        .expect("newest history page");
    assert_eq!(history_one[0].event.id, after_page_update.id);
    let (history_two, cursor_two) = db
        .query_mkideas_history(community, KIND_MK_PERSON, entity_one, cursor_one, 1)
        .await
        .expect("middle history page");
    assert_eq!(history_two[0].event.id, winning_edit.id);
    let (history_three, cursor_three) = db
        .query_mkideas_history(community, KIND_MK_PERSON, entity_one, cursor_two, 1)
        .await
        .expect("oldest history page");
    assert_eq!(history_three[0].event.id, original.id);
    assert!(cursor_three.is_none());

    let revision_rows = sqlx::query(
        "SELECT r.schema_version, e.deleted_at \
         FROM mk_entity_revisions r \
         JOIN events e ON e.community_id = r.community_id AND e.id = r.event_id \
         WHERE r.community_id = $1 AND r.kind = $2 AND r.d_tag = $3 \
         ORDER BY r.version",
    )
    .bind(community.as_uuid())
    .bind(KIND_MK_PERSON as i32)
    .bind(entity_one.to_string())
    .fetch_all(&pool)
    .await
    .expect("load immutable revisions");
    assert_eq!(revision_rows.len(), 3);
    assert_eq!(
        revision_rows
            .iter()
            .map(|row| row.get::<i64, _>("schema_version"))
            .collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
    assert!(revision_rows.iter().all(|row| row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at")
        .is_none()));

    let (all_heads, _) = db
        .query_mkideas_heads(community, &[KIND_MK_PERSON], None, 20)
        .await
        .expect("all current heads");
    assert!(all_heads.iter().any(|stored| {
        validate_state_event(&stored.event).is_ok_and(|envelope| envelope.schema_version == 1)
    }));
    assert!(all_heads.iter().any(|stored| {
        validate_state_event(&stored.event).is_ok_and(|envelope| envelope.schema_version == 2)
    }));
    let (isolated_heads, _) = db
        .query_mkideas_heads(other_community, &[KIND_MK_PERSON], None, 20)
        .await
        .expect("query other community");
    assert!(isolated_heads.is_empty());

    delete_community(&pool, community).await;
    delete_community(&pool, other_community).await;
}

#[tokio::test]
async fn approval_approve_replay_and_reject_are_atomic() {
    let pool = setup_pool().await;
    let db = Db::from_pool(pool.clone());

    let approved = create_approval_fixture(&pool, &db, "mk-approval-approve").await;
    let result = content_event(
        &approved.human,
        &approved.host,
        approved.target_id,
        2,
        Some(&approved.target),
        "approved",
    );
    let action = approval_action(&approved, "approved", Some(&result));
    let (_, stored_result, inserted) = db
        .apply_mkideas_approval_action(approved.community, &action)
        .await
        .expect("commit approved action and result");
    assert!(inserted);
    assert_eq!(stored_result.expect("stored result").event.id, result.id);
    let target_head = db
        .get_mkideas_entity_head(approved.community, KIND_MK_CONTENT, approved.target_id)
        .await
        .expect("query approved target")
        .expect("approved target head");
    assert_eq!(target_head.event.id, result.id);

    let (_, replay_result, replay_inserted) = db
        .apply_mkideas_approval_action(approved.community, &action)
        .await
        .expect("idempotent action replay");
    assert!(!replay_inserted);
    assert!(replay_result.is_none());
    let decision_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM mk_approval_decisions WHERE community_id = $1 AND approval_id = $2",
    )
    .bind(approved.community.as_uuid())
    .bind(approved.approval_id)
    .fetch_one(&pool)
    .await
    .expect("count approval decisions");
    assert_eq!(decision_count, 1);

    let conflicting_action = approval_action(&approved, "rejected", None);
    assert!(matches!(
        db.apply_mkideas_approval_action(approved.community, &conflicting_action)
            .await,
        Err(DbError::MkIdeasConflict { .. })
    ));
    assert!(!event_exists(&db, approved.community, &conflicting_action).await);

    let rejected = create_approval_fixture(&pool, &db, "mk-approval-reject").await;
    let rejection = approval_action(&rejected, "rejected", None);
    let (_, rejection_result, rejection_inserted) = db
        .apply_mkideas_approval_action(rejected.community, &rejection)
        .await
        .expect("commit rejection");
    assert!(rejection_inserted);
    assert!(rejection_result.is_none());
    let rejected_head = db
        .get_mkideas_entity_head(rejected.community, KIND_MK_CONTENT, rejected.target_id)
        .await
        .expect("query rejected target")
        .expect("rejected target head");
    assert_eq!(rejected_head.event.id, rejected.target.id);
    let rejected_decision: String = sqlx::query_scalar(
        "SELECT decision FROM mk_approval_decisions WHERE community_id = $1 AND approval_id = $2",
    )
    .bind(rejected.community.as_uuid())
    .bind(rejected.approval_id)
    .fetch_one(&pool)
    .await
    .expect("load rejection projection");
    assert_eq!(rejected_decision, "rejected");

    delete_community(&pool, approved.community).await;
    delete_community(&pool, rejected.community).await;
}

#[tokio::test]
async fn stale_target_and_invalid_result_roll_back_action_and_projection() {
    let pool = setup_pool().await;
    let db = Db::from_pool(pool.clone());

    let stale = create_approval_fixture(&pool, &db, "mk-approval-stale").await;
    let changed_target = content_event(
        &stale.human,
        &stale.host,
        stale.target_id,
        2,
        Some(&stale.target),
        "draft",
    );
    assert!(insert_head(&db, stale.community, &changed_target).await);
    let stale_action = approval_action(&stale, "approved", None);
    assert!(matches!(
        db.apply_mkideas_approval_action(stale.community, &stale_action)
            .await,
        Err(DbError::MkIdeasValidation(message)) if message.contains("stale")
    ));
    assert!(!event_exists(&db, stale.community, &stale_action).await);
    let stale_decisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM mk_approval_decisions WHERE community_id = $1 AND approval_id = $2",
    )
    .bind(stale.community.as_uuid())
    .bind(stale.approval_id)
    .fetch_one(&pool)
    .await
    .expect("count stale decisions");
    assert_eq!(stale_decisions, 0);

    let rollback = create_approval_fixture(&pool, &db, "mk-approval-rollback").await;
    let wrong_signer_result = content_event(
        &Keys::generate(),
        &rollback.host,
        rollback.target_id,
        2,
        Some(&rollback.target),
        "approved",
    );
    let rollback_action = approval_action(&rollback, "approved", Some(&wrong_signer_result));
    assert!(matches!(
        db.apply_mkideas_approval_action(rollback.community, &rollback_action)
            .await,
        Err(DbError::MkIdeasValidation(message)) if message.contains("approving human")
    ));
    assert!(!event_exists(&db, rollback.community, &rollback_action).await);
    assert!(!event_exists(&db, rollback.community, &wrong_signer_result).await);
    let rollback_head = db
        .get_mkideas_entity_head(rollback.community, KIND_MK_CONTENT, rollback.target_id)
        .await
        .expect("query rollback target")
        .expect("rollback target head");
    assert_eq!(rollback_head.event.id, rollback.target.id);
    let rollback_decisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM mk_approval_decisions WHERE community_id = $1 AND approval_id = $2",
    )
    .bind(rollback.community.as_uuid())
    .bind(rollback.approval_id)
    .fetch_one(&pool)
    .await
    .expect("count rollback decisions");
    assert_eq!(rollback_decisions, 0);

    delete_community(&pool, stale.community).await;
    delete_community(&pool, rollback.community).await;
}

#[tokio::test]
async fn clearing_do_not_contact_requires_the_community_owner() {
    let pool = setup_pool().await;
    let db = Db::from_pool(pool.clone());
    let (community, host) = create_community(&pool, "mk-dnc").await;
    let owner = Keys::generate();
    let admin = Keys::generate();
    sqlx::query(
        "INSERT INTO relay_members (community_id, pubkey, role) VALUES ($1, $2, 'owner'), ($1, $3, 'admin')",
    )
    .bind(community.as_uuid())
    .bind(owner.public_key().to_hex())
    .bind(admin.public_key().to_hex())
    .execute(&pool)
    .await
    .expect("insert owner and admin roles");

    let guest_id = Uuid::new_v4();
    let protected = person_event(&owner, &host, guest_id, 2, 1, None, "prospect", true, None);
    assert!(insert_head(&db, community, &protected).await);

    let admin_clear = person_event(
        &admin,
        &host,
        guest_id,
        2,
        2,
        Some(&protected),
        "prospect",
        false,
        Some("Guest requested renewed contact."),
    );
    let admin_envelope = validate_state_event(&admin_clear).expect("valid admin clear payload");
    assert!(matches!(
        db.replace_mkideas_entity_head(community, &admin_clear, &admin_envelope)
            .await,
        Err(DbError::MkIdeasValidation(message)) if message.contains("owner")
    ));

    let owner_clear = person_event(
        &owner,
        &host,
        guest_id,
        2,
        2,
        Some(&protected),
        "prospect",
        false,
        Some("Guest requested renewed contact."),
    );
    assert!(insert_head(&db, community, &owner_clear).await);
    let head = db
        .get_mkideas_entity_head(community, KIND_MK_PERSON, guest_id)
        .await
        .expect("query DNC head")
        .expect("DNC head exists");
    assert_eq!(head.event.id, owner_clear.id);
    assert!(!event_exists(&db, community, &admin_clear).await);

    delete_community(&pool, community).await;
}

#[tokio::test]
async fn service_grants_are_exact_revocable_and_cross_community_safe() {
    let pool = setup_pool().await;
    let db = Db::from_pool(pool.clone());
    let (community, _) = create_community(&pool, "mk-service").await;
    let (other_community, _) = create_community(&pool, "mk-service-other").await;
    let owner = Keys::generate();
    let service = Keys::generate();
    let owner_pubkey = owner.public_key().to_bytes();
    let service_pubkey = service.public_key().to_bytes();

    sqlx::query("INSERT INTO users (community_id, pubkey) VALUES ($1, $2)")
        .bind(community.as_uuid())
        .bind(owner_pubkey.as_slice())
        .execute(&pool)
        .await
        .expect("insert service owner");
    sqlx::query("INSERT INTO users (community_id, pubkey, agent_owner_pubkey) VALUES ($1, $2, $3)")
        .bind(community.as_uuid())
        .bind(service_pubkey.as_slice())
        .bind(owner_pubkey.as_slice())
        .execute(&pool)
        .await
        .expect("insert owned service identity");
    let grant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mk_service_grants (id, community_id, service_pubkey, owner_pubkey, purpose, persona, allowed_event_kinds, allowed_target_kinds, expires_at) \
         VALUES ($1, $2, $3, $4, 'agent', 'guest-researcher', $5, $6, now() + interval '1 hour')",
    )
    .bind(grant_id)
    .bind(community.as_uuid())
    .bind(service_pubkey.as_slice())
    .bind(owner_pubkey.as_slice())
    .bind(vec![KIND_MK_AGENT_PROPOSAL as i32])
    .bind(vec![KIND_MK_PERSON as i32])
    .execute(&pool)
    .await
    .expect("insert scoped service grant");

    assert!(db
        .mkideas_service_grant_allows(
            community,
            service_pubkey.as_slice(),
            "agent",
            Some("guest-researcher"),
            KIND_MK_AGENT_PROPOSAL,
            Some(KIND_MK_PERSON),
            None,
        )
        .await
        .expect("check exact grant"));
    assert!(!db
        .mkideas_service_grant_allows(
            community,
            service_pubkey.as_slice(),
            "agent",
            Some("content-clip-copilot"),
            KIND_MK_AGENT_PROPOSAL,
            Some(KIND_MK_PERSON),
            None,
        )
        .await
        .expect("check wrong persona"));
    assert!(!db
        .mkideas_service_grant_allows(
            community,
            service_pubkey.as_slice(),
            "agent",
            Some("guest-researcher"),
            KIND_MK_AGENT_PROPOSAL,
            Some(KIND_MK_CONTENT),
            None,
        )
        .await
        .expect("check wrong target kind"));
    assert!(!db
        .mkideas_service_grant_allows(
            other_community,
            service_pubkey.as_slice(),
            "agent",
            Some("guest-researcher"),
            KIND_MK_AGENT_PROPOSAL,
            Some(KIND_MK_PERSON),
            None,
        )
        .await
        .expect("check cross-community grant"));

    // An agent capability is persona-scoped. Omitting the persona must not
    // turn an exact capability check into a wildcard grant.
    assert!(!db
        .mkideas_service_grant_allows(
            community,
            service_pubkey.as_slice(),
            "agent",
            None,
            KIND_MK_AGENT_PROPOSAL,
            Some(KIND_MK_PERSON),
            None,
        )
        .await
        .expect("check missing agent persona"));

    sqlx::query("UPDATE mk_service_grants SET revoked_at = now() WHERE id = $1")
        .bind(grant_id)
        .execute(&pool)
        .await
        .expect("revoke service grant");
    assert!(!db
        .mkideas_service_grant_allows(
            community,
            service_pubkey.as_slice(),
            "agent",
            Some("guest-researcher"),
            KIND_MK_AGENT_PROPOSAL,
            Some(KIND_MK_PERSON),
            None,
        )
        .await
        .expect("check revoked grant"));

    delete_community(&pool, community).await;
    delete_community(&pool, other_community).await;
}
