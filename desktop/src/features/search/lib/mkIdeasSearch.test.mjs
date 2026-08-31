import assert from "node:assert/strict";
import test from "node:test";

import {
  mkIdeasSearchArea,
  parseMkIdeasSearchHit,
  projectCurrentMkIdeasSearchHits,
} from "./mkIdeasSearch.ts";

function hit(eventId, kind, content, createdAt = 1) {
  return {
    eventId,
    content: JSON.stringify(content),
    kind,
    pubkey: "a".repeat(64),
    channelId: null,
    channelName: null,
    createdAt,
    score: 1,
  };
}

test("typed search presentation covers Work and v1-compatible guest state", () => {
  assert.deepEqual(
    parseMkIdeasSearchHit(
      hit("task-v2", 30802, {
        schema_version: 2,
        entity_id: "task-id",
        version: 3,
        status: "in-progress",
        title: "Confirm Maya interview brief",
      }),
    ),
    {
      area: "work",
      entityId: "task-id",
      label: "Task",
      schemaVersion: 2,
      status: "in-progress",
      title: "Confirm Maya interview brief",
      version: 3,
    },
  );
  assert.equal(
    parseMkIdeasSearchHit(
      hit("guest-v1", 30803, {
        entity_id: "guest-id",
        version: 1,
        status: "research_ready",
        name: "Maya Chen",
      }),
    )?.schemaVersion,
    1,
  );
});

test("typed agent results inherit the target product area", () => {
  const proposal = hit("proposal", 48201, {
    schema_version: 2,
    target_id: "content-id",
    target_kind: 30805,
    agent: "Content / Clip Copilot",
    summary: "Two timestamped clips are ready for review.",
    status: "proposed",
  });
  assert.equal(mkIdeasSearchArea(proposal), "studio");
  assert.equal(parseMkIdeasSearchHit(proposal)?.label, "Agent proposal");
});

test("search projects revision matches to the highest current head", () => {
  const oldTask = hit(
    "task-v1",
    30802,
    { entity_id: "task-id", version: 1, status: "to-do", title: "Draft" },
    10,
  );
  const currentTask = hit(
    "task-v2",
    30802,
    {
      entity_id: "task-id",
      version: 2,
      status: "done",
      title: "Draft",
    },
    20,
  );
  const message = hit("message", 9, { body: "Draft" }, 30);
  const projected = projectCurrentMkIdeasSearchHits([
    oldTask,
    message,
    currentTask,
    currentTask,
  ]);
  assert.deepEqual(
    projected.map((item) => item.eventId),
    ["task-v2", "message"],
  );
});

test("malformed MK results remain visible as generic search hits", () => {
  const malformed = hit("malformed", 30802, { title: "Missing identity" });
  assert.equal(parseMkIdeasSearchHit(malformed), null);
  assert.deepEqual(projectCurrentMkIdeasSearchHits([malformed]), [malformed]);
});
