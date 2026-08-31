-- Preserve every accepted MK Ideas state revision while ordinary Nostr reads
-- continue to resolve through mk_entity_heads. No foreign key points to the
-- partitioned events table.

CREATE TABLE mk_entity_revisions (
    community_id     UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    kind             INT NOT NULL CHECK (kind BETWEEN 30800 AND 30899),
    d_tag            TEXT NOT NULL CHECK (length(d_tag) BETWEEN 1 AND 512),
    version          BIGINT NOT NULL CHECK (version >= 1),
    event_id         BYTEA NOT NULL CHECK (length(event_id) = 32),
    previous_event_id BYTEA CHECK (previous_event_id IS NULL OR length(previous_event_id) = 32),
    signer_pubkey    BYTEA NOT NULL CHECK (length(signer_pubkey) = 32),
    schema_version   BIGINT NOT NULL CHECK (schema_version >= 1),
    accepted_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (community_id, kind, d_tag, version),
    UNIQUE (community_id, event_id)
);

CREATE INDEX idx_mk_entity_revisions_history
    ON mk_entity_revisions (community_id, kind, d_tag, version DESC, event_id);

SELECT attach_community_write_fence('mk_entity_revisions'::regclass);

-- Backfill the V0 chains before restoring ordinary event visibility. These
-- rows were validated at ingest; malformed legacy/non-MK events are excluded.
INSERT INTO mk_entity_revisions (
    community_id,
    kind,
    d_tag,
    version,
    event_id,
    previous_event_id,
    signer_pubkey,
    schema_version,
    accepted_at
)
SELECT
    e.community_id,
    e.kind,
    e.d_tag,
    (e.content::jsonb ->> 'version')::BIGINT,
    e.id,
    CASE
        WHEN previous_tag.value ~ '^[0-9a-f]{64}$'
        THEN decode(previous_tag.value, 'hex')
        ELSE NULL
    END,
    e.pubkey,
    COALESCE((e.content::jsonb ->> 'schema_version')::BIGINT, 1),
    e.received_at
FROM events e
LEFT JOIN LATERAL (
    SELECT tag ->> 1 AS value
    FROM jsonb_array_elements(e.tags) AS tag
    WHERE tag ->> 0 = 'prev'
    LIMIT 1
) AS previous_tag ON TRUE
WHERE e.kind BETWEEN 30800 AND 30899
  AND e.d_tag IS NOT NULL
  AND jsonb_typeof(e.content::jsonb) = 'object'
  AND (e.content::jsonb ->> 'version') ~ '^[1-9][0-9]*$'
ON CONFLICT DO NOTHING;

-- Supersession is no longer deletion. Generic query, count, and search paths
-- are head-scoped for this range before this migration is introduced.
UPDATE events
SET deleted_at = NULL
WHERE kind BETWEEN 30800 AND 30899
  AND deleted_at IS NOT NULL;
