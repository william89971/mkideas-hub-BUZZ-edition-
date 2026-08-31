import assert from "node:assert/strict";
import { test } from "node:test";

import { buildAtomicApprovalAction, fetchMkEntityHistoryPage } from "./api.ts";

const targetEventId = "1".repeat(64);
const proposalEventId = "2".repeat(64);
const approvalEventId = "3".repeat(64);
const approvalId = "11111111-1111-4111-8111-111111111111";
const proposalId = "22222222-2222-4222-8222-222222222222";
const targetId = "33333333-3333-4333-8333-333333333333";

const approval = {
  eventId: approvalEventId,
  kind: 30809,
  author: "a".repeat(64),
  createdAt: 10,
  entityId: approvalId,
  version: 1,
  schemaVersion: 2,
  recordType: "approval",
  status: "pending",
  rawStatus: "pending",
  title: "Clip review",
  data: {
    target_id: targetId,
    target_kind: 30805,
    target_event_id: targetEventId,
    target_version: 2,
    proposal_id: proposalId,
    proposal_event_id: proposalEventId,
  },
};

const proposal = {
  eventId: proposalEventId,
  proposalId,
  targetId,
  targetKind: 30805,
  targetEventId,
  targetVersion: 2,
  schemaVersion: 2,
  agent: "Content / Clip Copilot",
  personaId: "content-clip-copilot",
  proposalType: "timestamped_clips",
  proposalVersion: 1,
  runId: "44444444-4444-4444-8444-444444444444",
  templateVersion: "clips.v2",
  provider: "fixture",
  model: "deterministic-local-v1",
  status: "proposed",
  reviewState: "pending",
  inputEventIds: [targetEventId],
  inputHash: "4".repeat(64),
  summary: "Two clips",
  provenance: ["Transcript v2"],
  clips: [],
  output: {},
  startedAt: "",
  completedAt: "",
  createdAt: 12,
};

test("atomic approval action repeats every exact request binding", () => {
  const action = buildAtomicApprovalAction("ws://hub.test", {
    approval,
    proposal,
    decision: "approved",
    reason: "Reviewed transcript timestamps and caption wording.",
  });
  assert.equal(action.kind, 48200);
  const body = JSON.parse(action.content);
  assert.equal(body.approval_event_id, approvalEventId);
  assert.equal(body.proposal_event_id, proposalEventId);
  assert.equal(body.target_event_id, targetEventId);
  assert.equal(body.target_version, 2);
  assert.equal(body.decision, "approved");
  assert.ok(body.action_id);
});

test("atomic approval refuses a proposal that no longer matches its request", () => {
  assert.throws(
    () =>
      buildAtomicApprovalAction("ws://hub.test", {
        approval,
        proposal: { ...proposal, targetVersion: 3 },
        decision: "rejected",
        reason: "The target changed.",
      }),
    /no longer matches/,
  );
});

test("entity history uses the history projection and preserves its cursor", async () => {
  let captured;
  const response = await fetchMkEntityHistoryPage(
    "wss://hub.test",
    { kind: 30805, entityId: targetId, cursor: "3" },
    async (query) => {
      captured = query;
      return {
        events: [
          {
            id: targetEventId,
            pubkey: "a".repeat(64),
            created_at: 9,
            kind: 30805,
            tags: [
              ["d", targetId],
              ["version", "2"],
              ["status", "in-review"],
            ],
            content: JSON.stringify({
              schema_version: 2,
              record_type: "content",
              entity_id: targetId,
              version: 2,
              status: "in-review",
              title: "Clip package",
            }),
            sig: "f".repeat(128),
          },
        ],
        nextCursor: "1",
      };
    },
  );
  assert.deepEqual(captured, {
    projection: "history",
    kinds: [30805],
    community: "hub.test",
    entityId: targetId,
    cursor: "3",
    limit: 20,
  });
  assert.equal(response.records[0].version, 2);
  assert.equal(response.nextCursor, "1");
});
