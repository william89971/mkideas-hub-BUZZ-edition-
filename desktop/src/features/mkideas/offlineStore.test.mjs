import assert from "node:assert/strict";
import { test } from "node:test";

import {
  cacheMkIdeasEvents,
  discardMkOutboxItem,
  isMkConflictError,
  readCachedMkIdeasSnapshot,
  readMkOutbox,
  storeMkOutboxItem,
} from "./offlineStore.ts";

class MemoryStorage {
  values = new Map();

  getItem(key) {
    return this.values.get(key) ?? null;
  }

  setItem(key, value) {
    this.values.set(key, value);
  }

  removeItem(key) {
    this.values.delete(key);
  }
}

const relayUrl = "wss://hub.mkideas.org";
const entityId = "11111111-1111-4111-8111-111111111111";
const event = {
  id: "a".repeat(64),
  pubkey: "b".repeat(64),
  created_at: 10,
  kind: 30802,
  tags: [
    ["d", entityId],
    ["h", "hub.mkideas.org"],
    ["version", "1"],
    ["status", "to-do"],
  ],
  content: JSON.stringify({
    schema_version: 2,
    record_type: "task",
    entity_id: entityId,
    version: 1,
    status: "to-do",
    title: "Book the studio",
  }),
  sig: "c".repeat(128),
};

test("cached events restore a readable MK Ideas snapshot", () => {
  const storage = new MemoryStorage();
  cacheMkIdeasEvents(relayUrl, [event], storage);

  const cached = readCachedMkIdeasSnapshot(relayUrl, storage);
  assert.equal(cached.snapshot.records.length, 1);
  assert.equal(cached.snapshot.records[0].title, "Book the studio");
  assert.ok(cached.savedAt);
});

test("outbox preserves signed drafts until explicit discard", () => {
  const storage = new MemoryStorage();
  const now = new Date().toISOString();
  storeMkOutboxItem(
    {
      id: event.id,
      relayUrl,
      event,
      input: {
        kind: 30802,
        recordType: "task",
        status: "to-do",
        fields: { title: "Book the studio" },
      },
      status: "queued",
      attempts: 1,
      createdAt: now,
      updatedAt: now,
      error: "Relay connection closed.",
    },
    storage,
  );

  assert.equal(readMkOutbox(relayUrl, storage).length, 1);
  discardMkOutboxItem(relayUrl, event.id, storage);
  assert.deepEqual(readMkOutbox(relayUrl, storage), []);
});

test("only version and stale-write failures become review conflicts", () => {
  assert.equal(
    isMkConflictError(new Error("conflict: MK Ideas entity is at version 4")),
    true,
  );
  assert.equal(isMkConflictError(new Error("Relay connection closed.")), false);
});
