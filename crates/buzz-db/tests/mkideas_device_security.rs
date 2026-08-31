//! PostgreSQL contract tests for staged device and notification security.

use buzz_core::mkideas_notification::{
    mk_notification_dedupe_key, MkNotificationClass, MkNotificationDedupeInput,
    MkNotificationPreferencesV1,
};
use buzz_core::{CommunityId, Keys};
use buzz_db::device_grants::{MkDeviceEnrollmentInput, MkDeviceGrantStatus};
use buzz_db::{migration, Db, DbError};
use chrono::{Duration, Utc};
use sqlx::PgPool;
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

async fn community(pool: &PgPool, prefix: &str) -> CommunityId {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO communities (id, host) VALUES ($1, $2)")
        .bind(id)
        .bind(format!("{prefix}-{}.mkideas.test", id.simple()))
        .execute(pool)
        .await
        .expect("insert test community");
    CommunityId::from_uuid(id)
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

#[tokio::test]
async fn device_and_notification_state_is_strictly_community_scoped() {
    let pool = setup_pool().await;
    let db = Db::from_pool(pool.clone());
    let community_a = community(&pool, "device-a").await;
    let community_b = community(&pool, "device-b").await;
    let human = Keys::generate().public_key().to_bytes();
    let device = Keys::generate().public_key().to_bytes();
    let other = Keys::generate().public_key().to_bytes();
    let human_proof = [2; 32];
    let device_proof = [3; 32];
    let challenge_id = Uuid::new_v4();
    db.create_mkideas_device_enrollment_challenge(
        community_a,
        challenge_id,
        &human,
        &device,
        &human,
        &[1; 32],
        Utc::now() + Duration::minutes(5),
    )
    .await
    .expect("create challenge in A");

    let enroll = |challenge_id| MkDeviceEnrollmentInput {
        challenge_id,
        human_pubkey: &human,
        device_pubkey: &device,
        device_name: "Test Windows PC",
        platform: "windows",
        human_proof_event_id: &human_proof,
        device_proof_event_id: &device_proof,
    };
    assert!(matches!(
        db.enroll_mkideas_device(community_b, enroll(challenge_id))
            .await,
        Err(DbError::AccessDenied(_))
    ));
    let grant_id = db
        .enroll_mkideas_device(community_a, enroll(challenge_id))
        .await
        .expect("enroll in A");
    assert_eq!(
        db.mkideas_device_grant_status_by_id(community_a, grant_id, &human, &device)
            .await
            .expect("status A"),
        MkDeviceGrantStatus::Active
    );
    assert_eq!(
        db.mkideas_device_grant_status_by_id(community_b, grant_id, &human, &device)
            .await
            .expect("status B"),
        MkDeviceGrantStatus::Missing
    );
    assert_eq!(
        db.list_mkideas_device_grants(community_a, &human)
            .await
            .expect("inventory A")
            .len(),
        1
    );
    assert!(db
        .list_mkideas_device_grants(community_b, &human)
        .await
        .expect("inventory B")
        .is_empty());
    assert!(matches!(
        db.create_mkideas_device_recovery_request(
            community_b,
            Uuid::new_v4(),
            &human,
            grant_id,
            &other,
            &[4; 32],
            Utc::now() + Duration::minutes(5),
        )
        .await,
        Err(DbError::AccessDenied(_))
    ));

    let preferences = MkNotificationPreferencesV1 {
        version: 1,
        enabled: vec![MkNotificationClass::Approval],
        quiet_start_minute: Some(22 * 60),
        quiet_end_minute: Some(7 * 60),
        timezone: Some("America/Los_Angeles".into()),
    };
    db.upsert_mkideas_notification_preferences(community_a, &human, &preferences)
        .await
        .expect("preferences A");
    assert_eq!(
        db.get_mkideas_notification_preferences(community_a, &human)
            .await
            .expect("read A"),
        Some(preferences)
    );
    assert_eq!(
        db.get_mkideas_notification_preferences(community_b, &human)
            .await
            .expect("read B"),
        None
    );
    let entity_id = Uuid::new_v4();
    let key = mk_notification_dedupe_key(MkNotificationDedupeInput {
        community_id: *community_a.as_uuid(),
        recipient_pubkey: &human,
        class: MkNotificationClass::Approval,
        kind: 30809,
        entity_id,
        version: 1,
    });
    let expiry = Utc::now() + Duration::hours(1);
    assert!(db
        .claim_mkideas_notification_delivery(
            community_a,
            &human,
            &key,
            MkNotificationClass::Approval,
            expiry,
        )
        .await
        .expect("first A claim"));
    assert!(!db
        .claim_mkideas_notification_delivery(
            community_a,
            &human,
            &key,
            MkNotificationClass::Approval,
            expiry,
        )
        .await
        .expect("duplicate A claim"));
    assert!(db
        .claim_mkideas_notification_delivery(
            community_b,
            &human,
            &key,
            MkNotificationClass::Approval,
            expiry,
        )
        .await
        .expect("same bytes isolated in B"));

    assert_eq!(
        db.revoke_mkideas_device_grant(community_a, grant_id, &other, "Unauthorized revocation")
            .await
            .expect("unauthorized is a no-op"),
        None
    );
    assert!(db
        .revoke_mkideas_device_grant(
            community_a,
            grant_id,
            &human,
            "Device intentionally retired"
        )
        .await
        .expect("self revoke")
        .is_some());
    assert_eq!(
        db.mkideas_device_grant_status_by_id(community_a, grant_id, &human, &device)
            .await
            .expect("revoked status"),
        MkDeviceGrantStatus::Revoked
    );

    for community in [community_a, community_b] {
        delete_community(&pool, community).await;
    }
}
