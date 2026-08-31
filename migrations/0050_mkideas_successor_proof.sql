-- Preserve the exact owner-signed authorization event for successor identity
-- attribution. Kept separate because the local integration database applied
-- migration 0049 while the successor proof contract was being finalized.

ALTER TABLE mk_identity_successors
    ADD COLUMN authorization_event_id BYTEA NOT NULL
        CHECK (length(authorization_event_id) = 32);
