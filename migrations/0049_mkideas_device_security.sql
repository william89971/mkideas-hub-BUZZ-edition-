-- Staged MK Ideas device enrollment, revocation, recovery, and successor
-- attribution. Enforcement remains disabled at runtime until every current
-- client has an active device grant.

ALTER TABLE mk_device_grants
    ADD COLUMN grant_version SMALLINT NOT NULL DEFAULT 1
        CHECK (grant_version = 1),
    ADD COLUMN auth_epoch BIGINT NOT NULL DEFAULT 1
        CHECK (auth_epoch > 0),
    ADD COLUMN enrollment_method TEXT NOT NULL DEFAULT 'two-key-proof'
        CHECK (enrollment_method IN ('two-key-proof', 'surviving-device', 'owner-assisted')),
    ADD COLUMN human_proof_event_id BYTEA
        CHECK (human_proof_event_id IS NULL OR length(human_proof_event_id) = 32),
    ADD COLUMN device_proof_event_id BYTEA
        CHECK (device_proof_event_id IS NULL OR length(device_proof_event_id) = 32);

CREATE TABLE mk_device_enrollment_challenges (
    id                    UUID NOT NULL,
    community_id          UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    human_pubkey          BYTEA NOT NULL CHECK (length(human_pubkey) = 32),
    device_pubkey         BYTEA NOT NULL CHECK (length(device_pubkey) = 32),
    issued_by             BYTEA NOT NULL CHECK (length(issued_by) = 32),
    challenge_hash        BYTEA NOT NULL CHECK (length(challenge_hash) = 32),
    issued_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at            TIMESTAMPTZ NOT NULL,
    consumed_at           TIMESTAMPTZ,
    consumed_by_grant_id  UUID,
    CHECK (expires_at > issued_at AND expires_at <= issued_at + interval '10 minutes'),
    CHECK ((consumed_at IS NULL) = (consumed_by_grant_id IS NULL)),
    PRIMARY KEY (community_id, id),
    FOREIGN KEY (community_id, consumed_by_grant_id)
        REFERENCES mk_device_grants(community_id, id),
    UNIQUE (community_id, challenge_hash)
);

CREATE INDEX idx_mk_device_enrollment_challenges_active
    ON mk_device_enrollment_challenges (community_id, human_pubkey, expires_at)
    WHERE consumed_at IS NULL;

CREATE TABLE mk_device_recovery_requests (
    id                    UUID NOT NULL,
    community_id          UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    human_pubkey          BYTEA NOT NULL CHECK (length(human_pubkey) = 32),
    surviving_grant_id    UUID NOT NULL,
    requested_device_pubkey BYTEA NOT NULL CHECK (length(requested_device_pubkey) = 32),
    challenge_hash        BYTEA NOT NULL CHECK (length(challenge_hash) = 32),
    requested_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at            TIMESTAMPTZ NOT NULL,
    decided_at            TIMESTAMPTZ,
    decided_by            BYTEA CHECK (decided_by IS NULL OR length(decided_by) = 32),
    state                 TEXT NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'approved', 'rejected', 'expired', 'consumed')),
    resulting_grant_id    UUID,
    decision_reason       TEXT,
    CHECK (expires_at > requested_at AND expires_at <= requested_at + interval '10 minutes'),
    CHECK (requested_device_pubkey <> human_pubkey),
    CHECK ((decided_at IS NULL) = (decided_by IS NULL)),
    PRIMARY KEY (community_id, id),
    FOREIGN KEY (community_id, surviving_grant_id)
        REFERENCES mk_device_grants(community_id, id),
    FOREIGN KEY (community_id, resulting_grant_id)
        REFERENCES mk_device_grants(community_id, id),
    UNIQUE (community_id, challenge_hash)
);

CREATE INDEX idx_mk_device_recovery_requests_pending
    ON mk_device_recovery_requests (community_id, human_pubkey, expires_at)
    WHERE state = 'pending';

CREATE TABLE mk_nip49_recovery_bundles (
    id                    UUID NOT NULL,
    community_id          UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    human_pubkey          BYTEA NOT NULL CHECK (length(human_pubkey) = 32),
    object_key            TEXT NOT NULL CHECK (length(object_key) BETWEEN 1 AND 512),
    sha256                BYTEA NOT NULL CHECK (length(sha256) = 32),
    size_bytes            INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 4096),
    created_by_grant_id   UUID NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at            TIMESTAMPTZ,
    revoked_by            BYTEA CHECK (revoked_by IS NULL OR length(revoked_by) = 32),
    PRIMARY KEY (community_id, id),
    FOREIGN KEY (community_id, created_by_grant_id)
        REFERENCES mk_device_grants(community_id, id),
    UNIQUE (community_id, object_key),
    UNIQUE (community_id, sha256)
);

CREATE INDEX idx_mk_nip49_recovery_bundles_active
    ON mk_nip49_recovery_bundles (community_id, human_pubkey, created_at DESC)
    WHERE revoked_at IS NULL;

CREATE TABLE mk_identity_successors (
    id                    UUID NOT NULL,
    community_id          UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    predecessor_pubkey    BYTEA NOT NULL CHECK (length(predecessor_pubkey) = 32),
    successor_pubkey      BYTEA NOT NULL CHECK (length(successor_pubkey) = 32),
    authorized_by         BYTEA NOT NULL CHECK (length(authorized_by) = 32),
    incident_id           UUID NOT NULL,
    reason                TEXT NOT NULL CHECK (length(reason) BETWEEN 8 AND 500),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    superseded_at         TIMESTAMPTZ,
    CHECK (predecessor_pubkey <> successor_pubkey),
    PRIMARY KEY (community_id, id),
    UNIQUE (community_id, predecessor_pubkey),
    UNIQUE (community_id, successor_pubkey),
    UNIQUE (community_id, incident_id)
);

CREATE INDEX idx_mk_identity_successors_active
    ON mk_identity_successors (community_id, predecessor_pubkey)
    WHERE superseded_at IS NULL;

-- Actionable notification preferences and semantic delivery dedupe. Platform
-- transports still receive only the fixed content-free reconnect signal.
CREATE TABLE mk_notification_preferences (
    community_id          UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    human_pubkey          BYTEA NOT NULL CHECK (length(human_pubkey) = 32),
    schema_version        SMALLINT NOT NULL DEFAULT 1 CHECK (schema_version = 1),
    enabled_classes       JSONB NOT NULL,
    quiet_start_minute    SMALLINT CHECK (quiet_start_minute BETWEEN 0 AND 1439),
    quiet_end_minute      SMALLINT CHECK (quiet_end_minute BETWEEN 0 AND 1439),
    timezone              TEXT CHECK (timezone IS NULL OR length(timezone) BETWEEN 1 AND 80),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((quiet_start_minute IS NULL) = (quiet_end_minute IS NULL)),
    CHECK ((quiet_start_minute IS NULL) = (timezone IS NULL)),
    CHECK (quiet_start_minute IS NULL OR quiet_start_minute <> quiet_end_minute),
    PRIMARY KEY (community_id, human_pubkey)
);

CREATE TABLE mk_notification_delivery_dedupe (
    community_id          UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    human_pubkey          BYTEA NOT NULL CHECK (length(human_pubkey) = 32),
    dedupe_key            BYTEA NOT NULL CHECK (length(dedupe_key) = 32),
    notification_class    TEXT NOT NULL CHECK (notification_class IN (
        'assignment', 'approval', 'deadline', 'mention',
        'agent-outcome', 'important-transition'
    )),
    claimed_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at            TIMESTAMPTZ NOT NULL,
    CHECK (expires_at > claimed_at),
    PRIMARY KEY (community_id, human_pubkey, dedupe_key)
);

CREATE INDEX idx_mk_notification_delivery_dedupe_expiry
    ON mk_notification_delivery_dedupe (expires_at);

SELECT attach_community_write_fence('mk_device_enrollment_challenges'::regclass);
SELECT attach_community_write_fence('mk_device_recovery_requests'::regclass);
SELECT attach_community_write_fence('mk_nip49_recovery_bundles'::regclass);
SELECT attach_community_write_fence('mk_identity_successors'::regclass);
SELECT attach_community_write_fence('mk_notification_preferences'::regclass);
SELECT attach_community_write_fence('mk_notification_delivery_dedupe'::regclass);
