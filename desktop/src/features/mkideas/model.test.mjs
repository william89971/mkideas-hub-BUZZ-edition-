import assert from "node:assert/strict";
import test from "node:test";

import {
  MK_RECORD_KIND_BY_TYPE,
  MK_RECORD_TYPES,
  MK_STATUSES,
  parseMkIdeasEvents,
} from "./model.ts";

function event(kind, content, tags = []) {
  return {
    id: `${kind}`.padStart(64, "a").slice(-64),
    pubkey: "b".repeat(64),
    kind,
    content: JSON.stringify(content),
    created_at: kind,
    tags,
    sig: "c".repeat(128),
  };
}

test("schema-v2 parser accepts every MK state kind and typed initial status", () => {
  const events = MK_RECORD_TYPES.map((recordType, index) => {
    const kind = MK_RECORD_KIND_BY_TYPE[recordType];
    const entityId = `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`;
    return event(
      kind,
      {
        schema_version: 2,
        record_type: recordType,
        entity_id: entityId,
        version: 1,
        status: MK_STATUSES[recordType][0],
        title: `${recordType} title`,
      },
      [
        ["d", entityId],
        ["version", "1"],
        ["status", MK_STATUSES[recordType][0]],
      ],
    );
  });
  const snapshot = parseMkIdeasEvents(events);
  assert.equal(snapshot.records.length, MK_RECORD_TYPES.length);
  assert.deepEqual(
    new Set(snapshot.records.map((record) => record.recordType)),
    new Set(MK_RECORD_TYPES),
  );
  assert.ok(snapshot.records.every((record) => record.schemaVersion === 2));
});

test("schema-v1 aliases remain readable and normalize into V2 statuses", () => {
  const records = parseMkIdeasEvents([
    event(30803, {
      record_type: "person",
      entity_id: "11111111-1111-4111-8111-111111111111",
      version: 1,
      status: "potential",
      name: "Legacy guest",
    }),
    event(30804, {
      record_type: "interview",
      entity_id: "22222222-2222-4222-8222-222222222222",
      version: 2,
      status: "content_processing",
      title: "Legacy interview",
    }),
    event(30805, {
      record_type: "content",
      entity_id: "33333333-3333-4333-8333-333333333333",
      version: 1,
      status: "internal_review",
      title: "Legacy content",
    }),
  ]).records;
  assert.deepEqual(records.map((record) => record.status).sort(), [
    "in-review",
    "prospect",
    "transcribing",
  ]);
  assert.ok(records.every((record) => record.schemaVersion === 1));
});

test("parser uses kind, d, version, and status tags when V2 content is minimal", () => {
  const entityId = "44444444-4444-4444-8444-444444444444";
  const snapshot = parseMkIdeasEvents([
    event(30802, { schema_version: 2, title: "Tagged task" }, [
      ["d", entityId],
      ["version", "3"],
      ["status", "in-progress"],
    ]),
  ]);
  assert.deepEqual(snapshot.records[0], {
    eventId: `${30802}`.padStart(64, "a").slice(-64),
    kind: 30802,
    author: "b".repeat(64),
    createdAt: 30802,
    entityId,
    version: 3,
    schemaVersion: 2,
    recordType: "task",
    status: "in-progress",
    rawStatus: "in-progress",
    title: "Tagged task",
    data: { schema_version: 2, title: "Tagged task" },
  });
});

test("parser rejects kind/type mismatches and invalid statuses", () => {
  const snapshot = parseMkIdeasEvents([
    event(30802, {
      schema_version: 2,
      record_type: "person",
      entity_id: "55555555-5555-4555-8555-555555555555",
      version: 1,
      status: "prospect",
    }),
    event(30802, {
      schema_version: 2,
      record_type: "task",
      entity_id: "66666666-6666-4666-8666-666666666666",
      version: 1,
      status: "published",
    }),
  ]);
  assert.deepEqual(snapshot.records, []);
});
