-- Narrow, revocable capabilities for MK Ideas service identities. Being a
-- generic managed agent is not sufficient for schema-v2 operational output.

CREATE TABLE mk_service_grants (
    id                   UUID NOT NULL DEFAULT gen_random_uuid(),
    community_id         UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    service_pubkey       BYTEA NOT NULL CHECK (length(service_pubkey) = 32),
    owner_pubkey         BYTEA NOT NULL CHECK (length(owner_pubkey) = 32),
    purpose              TEXT NOT NULL CHECK (purpose IN ('agent', 'migration', 'maintenance')),
    persona              TEXT,
    allowed_event_kinds  INT[] NOT NULL,
    allowed_target_kinds INT[] NOT NULL DEFAULT '{}',
    dataset_sha256       TEXT,
    issued_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at           TIMESTAMPTZ NOT NULL,
    revoked_at           TIMESTAMPTZ,
    CHECK (expires_at > issued_at),
    CHECK (cardinality(allowed_event_kinds) > 0),
    CHECK (purpose <> 'agent' OR persona IS NOT NULL),
    CHECK (purpose <> 'migration' OR dataset_sha256 IS NOT NULL),
    PRIMARY KEY (community_id, id),
    UNIQUE (community_id, service_pubkey, purpose, persona, dataset_sha256)
);

CREATE INDEX idx_mk_service_grants_active
    ON mk_service_grants (community_id, service_pubkey, expires_at)
    WHERE revoked_at IS NULL;

SELECT attach_community_write_fence('mk_service_grants'::regclass);
