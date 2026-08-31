-- One immutable projection row per atomically accepted Command Center source
-- unit. Event rows remain the signed authority; this table is the exact
-- idempotency and reconciliation boundary for apply/resume.

CREATE TABLE mk_migration_items (
    community_id        UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    dataset_sha256      TEXT NOT NULL CHECK (length(dataset_sha256) = 64),
    batch_id            UUID NOT NULL,
    source_system       TEXT NOT NULL CHECK (length(source_system) BETWEEN 1 AND 120),
    source_workspace_id UUID NOT NULL,
    source_type         TEXT NOT NULL CHECK (length(source_type) BETWEEN 1 AND 120),
    source_id           TEXT NOT NULL CHECK (length(source_id) BETWEEN 1 AND 512),
    source_revision     BIGINT NOT NULL CHECK (source_revision >= 1),
    source_sha256       TEXT NOT NULL CHECK (length(source_sha256) = 64),
    record_id           UUID NOT NULL,
    destination_kind    INT NOT NULL,
    destination_d_tag   TEXT,
    imported_event_id   BYTEA NOT NULL CHECK (length(imported_event_id) = 32),
    receipt_event_id    BYTEA NOT NULL CHECK (length(receipt_event_id) = 32),
    service_pubkey      BYTEA NOT NULL CHECK (length(service_pubkey) = 32),
    accepted_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (
        community_id, dataset_sha256, batch_id, source_system,
        source_workspace_id, source_type, source_id, source_revision
    ),
    UNIQUE (community_id, imported_event_id),
    UNIQUE (community_id, receipt_event_id),
    CHECK (
        (destination_kind BETWEEN 30800 AND 30899 AND destination_d_tag IS NOT NULL)
        OR
        (destination_kind NOT BETWEEN 30800 AND 30899 AND destination_d_tag IS NULL)
    )
);

CREATE INDEX idx_mk_migration_items_batch
    ON mk_migration_items (community_id, dataset_sha256, batch_id, accepted_at);

SELECT attach_community_write_fence('mk_migration_items'::regclass);
