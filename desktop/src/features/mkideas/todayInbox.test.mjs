import assert from "node:assert/strict";
import { test } from "node:test";

import { buildTodayInbox } from "./todayInbox.ts";

const record = (overrides) => ({
  eventId: "a".repeat(64),
  kind: 30802,
  author: "b".repeat(64),
  createdAt: 1,
  entityId: "11111111-1111-4111-8111-111111111111",
  version: 1,
  schemaVersion: 2,
  recordType: "task",
  status: "blocked",
  rawStatus: "blocked",
  title: "Confirm studio brief",
  data: {},
  ...overrides,
});

test("Today deduplicates blocked, assigned, and overdue signals by entity", () => {
  const items = buildTodayInbox({
    records: [
      record({
        data: {
          blocked_reason: "Waiting for rights confirmation.",
          assignees: ["William"],
          due_at: "2026-08-29T12:00:00Z",
        },
      }),
    ],
    proposals: [],
    approvalActions: [],
    activities: [],
    now: Date.parse("2026-08-30T12:00:00Z"),
  });
  assert.equal(items.length, 1);
  assert.deepEqual(items[0].signals, ["overdue", "blocked", "assigned"]);
  assert.equal(items[0].category, "deadline");
});

test("a pending exact approval outranks its target's agent result", () => {
  const target = record({
    kind: 30805,
    recordType: "content",
    status: "in-review",
    rawStatus: "in-review",
  });
  const proposal = {
    eventId: "c".repeat(64),
    proposalId: "22222222-2222-4222-8222-222222222222",
    targetId: target.entityId,
    targetKind: 30805,
    targetEventId: target.eventId,
    targetVersion: 1,
    schemaVersion: 2,
    agent: "Content / Clip Copilot",
    personaId: "content-clip-copilot",
    proposalType: "timestamped_clips",
    proposalVersion: 1,
    runId: "",
    templateVersion: "clips.v2",
    provider: "fixture",
    model: "fixture",
    status: "proposed",
    reviewState: "pending",
    inputEventIds: [],
    inputHash: "",
    summary: "Review clips",
    provenance: [],
    clips: [],
    output: {},
    startedAt: "",
    completedAt: "",
    createdAt: 2,
  };
  const approval = record({
    eventId: "d".repeat(64),
    kind: 30809,
    entityId: "33333333-3333-4333-8333-333333333333",
    recordType: "approval",
    status: "pending",
    rawStatus: "pending",
    data: {
      proposal_id: proposal.proposalId,
      proposal_event_id: proposal.eventId,
      target_id: target.entityId,
      target_kind: 30805,
      target_event_id: target.eventId,
      target_version: 1,
    },
  });
  const items = buildTodayInbox({
    records: [target, approval],
    proposals: [proposal],
    approvalActions: [],
    activities: [],
  });
  assert.equal(items.length, 1);
  assert.equal(items[0].category, "approval");
  assert.equal(items[0].approval.entityId, approval.entityId);
});
