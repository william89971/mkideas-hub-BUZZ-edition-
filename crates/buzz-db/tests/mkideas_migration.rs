//! PostgreSQL tests for the atomic migration receipt/import boundary.

use buzz_core::kind::{
    KIND_MK_EXTERNAL_COMMUNICATION, KIND_MK_MIGRATION_RECEIPT, KIND_MK_SYSTEM_ACTIVITY,
};
use buzz_core::mkideas_migration::{
    mkcc_batch_id, mkcc_destination_id, validate_migration_receipt, MkMigrationReceipt,
    MkMigrationSource,
};
use buzz_core::CommunityId;
use buzz_db::{migration, Db, DbError};
use nostr::{Event, EventBuilder, Keys, Kind, Tag};
use serde_json::json;
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

fn receipt_pair(
    keys: &Keys,
    host: &str,
    workspace_id: Uuid,
    dataset_sha256: &str,
    unit_id: &str,
    target_kind: u32,
) -> Event {
    let source = MkMigrationSource {
        system: "mkideas-command-center".to_string(),
        workspace_id,
        unit_kind: "activity_events".to_string(),
        unit_id: unit_id.to_string(),
        revision: 1,
    };
    let source_tag = source.tag_value();
    let source_sha256 = "a".repeat(64);
    let record_id = mkcc_destination_id(workspace_id, &source.unit_kind, &source.unit_id);
    let imported = EventBuilder::new(
        Kind::Custom(target_kind as u16),
        json!({
            "schema_version": 2,
            "record_id": record_id,
            "community": host,
            "version": 1,
            "migration": {
                "dataset_sha256": dataset_sha256,
                "source_system": source.system,
                "source_workspace_id": workspace_id,
                "source_table": source.unit_kind,
                "source_id": source.unit_id,
                "source_sha256": source_sha256
            }
        })
        .to_string(),
    )
    .tags([
        Tag::parse(["h", host]).expect("h"),
        Tag::parse(["source", source_tag.as_str()]).expect("source"),
        Tag::parse(["source_sha256", source_sha256.as_str()]).expect("source hash"),
    ])
    .sign_with_keys(keys)
    .expect("sign imported event");
    let receipt = MkMigrationReceipt {
        schema_version: 2,
        community: host.to_string(),
        dataset_sha256: dataset_sha256.to_string(),
        batch_id: mkcc_batch_id(workspace_id, dataset_sha256),
        receipt_id: mkcc_destination_id(workspace_id, "receipt", &source_tag),
        source,
        source_sha256,
        record_id,
        target_kind,
        destination_d: None,
        imported_event: imported,
    };
    let batch = receipt.batch_id.to_string();
    let imported_id = receipt.imported_event.id.to_hex();
    EventBuilder::new(
        Kind::Custom(KIND_MK_MIGRATION_RECEIPT as u16),
        serde_json::to_string(&receipt).expect("serialize receipt"),
    )
    .tags([
        Tag::parse(["h", host]).expect("h"),
        Tag::parse(["dataset", dataset_sha256]).expect("dataset"),
        Tag::parse(["batch", batch.as_str()]).expect("batch"),
        Tag::parse(["source", source_tag.as_str()]).expect("source"),
        Tag::parse(["e", imported_id.as_str()]).expect("event"),
    ])
    .sign_with_keys(keys)
    .expect("sign receipt")
}

#[tokio::test]
async fn receipt_apply_is_idempotent_scoped_and_atomic() {
    let pool = setup_pool().await;
    let db = Db::from_pool(pool.clone());
    let community_id = Uuid::new_v4();
    let community = CommunityId::from_uuid(community_id);
    let host = format!("mk-migration-{}.test", community_id.simple());
    let owner = Keys::generate();
    let service = Keys::generate();
    let owner_pubkey = owner.public_key().to_bytes();
    let service_pubkey = service.public_key().to_bytes();
    let workspace_id = Uuid::new_v4();
    let dataset_sha256 = "b".repeat(64);

    sqlx::query("INSERT INTO communities (id, host) VALUES ($1, $2)")
        .bind(community_id)
        .bind(&host)
        .execute(&pool)
        .await
        .expect("insert community");
    sqlx::query("INSERT INTO users (community_id, pubkey) VALUES ($1, $2)")
        .bind(community_id)
        .bind(owner_pubkey.as_slice())
        .execute(&pool)
        .await
        .expect("insert owner");
    sqlx::query("INSERT INTO users (community_id, pubkey, agent_owner_pubkey) VALUES ($1, $2, $3)")
        .bind(community_id)
        .bind(service_pubkey.as_slice())
        .bind(owner_pubkey.as_slice())
        .execute(&pool)
        .await
        .expect("insert service");
    sqlx::query(
        "INSERT INTO mk_service_grants (community_id, service_pubkey, owner_pubkey, purpose, allowed_event_kinds, allowed_target_kinds, dataset_sha256, expires_at) \
         VALUES ($1,$2,$3,'migration',$4,$5,$6,now() + interval '1 hour')",
    )
    .bind(community_id)
    .bind(service_pubkey.as_slice())
    .bind(owner_pubkey.as_slice())
    .bind(vec![KIND_MK_MIGRATION_RECEIPT as i32])
    .bind(vec![KIND_MK_SYSTEM_ACTIVITY as i32])
    .bind(&dataset_sha256)
    .execute(&pool)
    .await
    .expect("insert migration grant");

    let receipt = receipt_pair(
        &service,
        &host,
        workspace_id,
        &dataset_sha256,
        "first",
        KIND_MK_SYSTEM_ACTIVITY,
    );
    let envelope = validate_migration_receipt(&receipt).expect("valid receipt");
    let first = db
        .apply_mkideas_migration_item(community, &receipt, &envelope)
        .await
        .expect("first apply");
    let replay = db
        .apply_mkideas_migration_item(community, &receipt, &envelope)
        .await
        .expect("exact replay");
    assert!(first.inserted);
    assert!(!replay.inserted);
    assert_eq!(first.receipt.event.id, replay.receipt.event.id);
    assert_eq!(first.imported.event.id, replay.imported.event.id);

    let unauthorized = receipt_pair(
        &service,
        &host,
        workspace_id,
        &dataset_sha256,
        "unauthorized",
        KIND_MK_EXTERNAL_COMMUNICATION,
    );
    let unauthorized_envelope =
        validate_migration_receipt(&unauthorized).expect("valid but ungranted receipt");
    assert!(matches!(
        db.apply_mkideas_migration_item(community, &unauthorized, &unauthorized_envelope)
            .await,
        Err(DbError::AccessDenied(_))
    ));

    let rollback = receipt_pair(
        &service,
        &host,
        workspace_id,
        &dataset_sha256,
        "rollback",
        KIND_MK_SYSTEM_ACTIVITY,
    );
    let rollback_envelope = validate_migration_receipt(&rollback).expect("rollback envelope");
    sqlx::query(
        "INSERT INTO mk_migration_items (community_id,dataset_sha256,batch_id,source_system,source_workspace_id,source_type,source_id,source_revision,source_sha256,record_id,destination_kind,imported_event_id,receipt_event_id,service_pubkey) \
         VALUES ($1,$2,$3,'fixture',$4,'fixture','collision',1,$5,$6,$7,$8,$9,$10)",
    )
    .bind(community_id)
    .bind(&dataset_sha256)
    .bind(Uuid::new_v4())
    .bind(workspace_id)
    .bind("c".repeat(64))
    .bind(Uuid::new_v4())
    .bind(KIND_MK_SYSTEM_ACTIVITY as i32)
    .bind(rollback_envelope.receipt.imported_event.id.as_bytes().as_slice())
    .bind([9_u8; 32].as_slice())
    .bind(service_pubkey.as_slice())
    .execute(&pool)
    .await
    .expect("seed late unique collision");
    assert!(db
        .apply_mkideas_migration_item(community, &rollback, &rollback_envelope)
        .await
        .is_err());
    assert!(db
        .get_event_by_id(community, rollback.id.as_bytes())
        .await
        .expect("query rolled back receipt")
        .is_none());
    assert!(db
        .get_event_by_id(
            community,
            rollback_envelope.receipt.imported_event.id.as_bytes()
        )
        .await
        .expect("query rolled back import")
        .is_none());

    for (table, statement) in [
        (
            "mk_migration_items",
            "DELETE FROM mk_migration_items WHERE community_id = $1",
        ),
        (
            "mk_service_grants",
            "DELETE FROM mk_service_grants WHERE community_id = $1",
        ),
        ("events", "DELETE FROM events WHERE community_id = $1"),
        ("users", "DELETE FROM users WHERE community_id = $1"),
    ] {
        sqlx::query(statement)
            .bind(community_id)
            .execute(&pool)
            .await
            .unwrap_or_else(|error| panic!("delete isolated rows from {table}: {error}"));
    }
    sqlx::query("DELETE FROM communities WHERE id = $1")
        .bind(community_id)
        .execute(&pool)
        .await
        .expect("delete community");
}
