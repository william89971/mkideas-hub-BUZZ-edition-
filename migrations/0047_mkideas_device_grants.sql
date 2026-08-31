-- Device-bound authentication seam for staged MK Ideas revocation. Existing
-- shared-key clients are enrolled before production enforcement is enabled.

CREATE TABLE mk_device_grants (
    id               UUID NOT NULL DEFAULT gen_random_uuid(),
    community_id     UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    human_pubkey     BYTEA NOT NULL CHECK (length(human_pubkey) = 32),
    device_pubkey    BYTEA NOT NULL CHECK (length(device_pubkey) = 32),
    issued_by        BYTEA NOT NULL CHECK (length(issued_by) = 32),
    device_name      TEXT NOT NULL CHECK (length(device_name) BETWEEN 1 AND 120),
    platform         TEXT NOT NULL CHECK (platform IN ('windows', 'macos', 'ios', 'android', 'linux', 'web')),
    issued_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at       TIMESTAMPTZ,
    last_seen_at     TIMESTAMPTZ,
    revoked_at       TIMESTAMPTZ,
    revoked_by       BYTEA CHECK (revoked_by IS NULL OR length(revoked_by) = 32),
    revocation_reason TEXT,
    PRIMARY KEY (community_id, id),
    UNIQUE (community_id, device_pubkey)
);

CREATE INDEX idx_mk_device_grants_human
    ON mk_device_grants (community_id, human_pubkey, issued_at DESC);
CREATE INDEX idx_mk_device_grants_active
    ON mk_device_grants (community_id, device_pubkey)
    WHERE revoked_at IS NULL;

SELECT attach_community_write_fence('mk_device_grants'::regclass);
