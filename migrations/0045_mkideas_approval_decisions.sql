-- One durable decision per MK Ideas approval request. The action event and this
-- projection are written in one transaction; an optional resulting state event
-- can be attached once present in the same transaction.

CREATE TABLE mk_approval_decisions (
    community_id       UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    approval_id        UUID NOT NULL,
    approval_event_id  BYTEA NOT NULL CHECK (length(approval_event_id) = 32),
    target_kind        INT NOT NULL CHECK (target_kind BETWEEN 30800 AND 30808),
    target_d_tag       TEXT NOT NULL CHECK (length(target_d_tag) BETWEEN 1 AND 512),
    target_event_id    BYTEA NOT NULL CHECK (length(target_event_id) = 32),
    target_version     BIGINT NOT NULL CHECK (target_version >= 1),
    proposal_id        UUID NOT NULL,
    proposal_event_id  BYTEA NOT NULL CHECK (length(proposal_event_id) = 32),
    action_event_id    BYTEA NOT NULL CHECK (length(action_event_id) = 32),
    result_event_id    BYTEA CHECK (result_event_id IS NULL OR length(result_event_id) = 32),
    decision           TEXT NOT NULL CHECK (decision IN ('approved', 'rejected')),
    decided_by         BYTEA NOT NULL CHECK (length(decided_by) = 32),
    reason             TEXT NOT NULL CHECK (length(reason) BETWEEN 1 AND 2000),
    decided_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (community_id, approval_id),
    UNIQUE (community_id, action_event_id)
);

CREATE INDEX idx_mk_approval_decisions_target
    ON mk_approval_decisions (community_id, target_kind, target_d_tag, decided_at DESC);

SELECT attach_community_write_fence('mk_approval_decisions'::regclass);
