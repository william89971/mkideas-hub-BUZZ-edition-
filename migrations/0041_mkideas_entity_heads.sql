-- MK Ideas community-wide authoritative heads.
--
-- Standard NIP-33 coordinates include the author's pubkey. MK Ideas keeps the
-- human signature on every state change while projecting one accepted head
-- across authorized authors at (community_id, kind, d_tag).

CREATE TABLE mk_entity_heads (
    community_id     UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    kind             INT NOT NULL CHECK (kind BETWEEN 30800 AND 30899),
    d_tag            TEXT NOT NULL CHECK (length(d_tag) BETWEEN 1 AND 512),
    current_event_id BYTEA CHECK (current_event_id IS NULL OR length(current_event_id) = 32),
    current_version  BIGINT NOT NULL DEFAULT 0 CHECK (current_version >= 0),
    current_pubkey   BYTEA CHECK (current_pubkey IS NULL OR length(current_pubkey) = 32),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (community_id, kind, d_tag),
    CHECK (
        (current_version = 0 AND current_event_id IS NULL AND current_pubkey IS NULL)
        OR
        (current_version > 0 AND current_event_id IS NOT NULL AND current_pubkey IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_mk_entity_heads_current_event
    ON mk_entity_heads (community_id, current_event_id)
    WHERE current_event_id IS NOT NULL;
