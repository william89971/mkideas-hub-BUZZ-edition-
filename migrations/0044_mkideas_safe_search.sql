-- Search only an allowlisted MK Ideas document. Contact details, raw
-- transcripts, approval payloads, and other private JSON fields must not leak
-- into NIP-50 snippets merely because they exist in an event.

CREATE OR REPLACE FUNCTION mkideas_search_document(event_kind INT, event_content TEXT)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
STRICT
AS $$
    SELECT concat_ws(
        ' ',
        event_content::jsonb ->> 'name',
        event_content::jsonb ->> 'title',
        event_content::jsonb ->> 'status',
        event_content::jsonb ->> 'organization',
        event_content::jsonb ->> 'summary',
        event_content::jsonb ->> 'proposal_type',
        event_content::jsonb ->> 'persona',
        CASE
            WHEN jsonb_typeof(event_content::jsonb -> 'topics') = 'array'
            THEN (event_content::jsonb -> 'topics')::TEXT
            ELSE NULL
        END,
        CASE
            WHEN jsonb_typeof(event_content::jsonb -> 'tags') = 'array'
            THEN (event_content::jsonb -> 'tags')::TEXT
            ELSE NULL
        END
    )
$$;

ALTER TABLE events DROP COLUMN search_tsv;
ALTER TABLE events ADD COLUMN search_tsv TSVECTOR GENERATED ALWAYS AS (
    CASE
        WHEN kind BETWEEN 30800 AND 30809
          OR kind IN (48201, 48203, 48204, 48205)
            THEN to_tsvector('simple', mkideas_search_document(kind, content))
        WHEN kind IN (1059, 30179, 30300, 30350, 30622, 44100, 44101, 44200)
            THEN NULL::tsvector
        ELSE to_tsvector('simple', content)
    END
) STORED;
CREATE INDEX idx_events_search_tsv ON events USING GIN (search_tsv);
