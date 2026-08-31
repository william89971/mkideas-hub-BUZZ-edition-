import assert from "node:assert/strict";
import { test } from "node:test";

import {
  collectMkProjection,
  normalizeMkProjectionPage,
} from "./projectionApi.ts";

const event = (id) => ({
  id: id.repeat(64),
  pubkey: "a".repeat(64),
  created_at: 1,
  kind: 30803,
  tags: [],
  content: "{}",
  sig: "f".repeat(128),
});

test("projection pages accept the relay camelCase cursor contract", () => {
  assert.deepEqual(
    normalizeMkProjectionPage({ events: [event("1")], nextCursor: "30803:x" }),
    { events: [event("1")], nextCursor: "30803:x" },
  );
});

test("projection collector follows every stable cursor without a fixed event cap", async () => {
  const calls = [];
  const result = await collectMkProjection(
    { projection: "heads", kinds: [30803], community: "hub.test", limit: 2 },
    async (query) => {
      calls.push(query.cursor ?? null);
      return query.cursor
        ? { events: [event("3")], nextCursor: null }
        : { events: [event("1"), event("2")], nextCursor: "30803:cursor" };
    },
  );
  assert.deepEqual(calls, [null, "30803:cursor"]);
  assert.deepEqual(
    result.map((item) => item.id[0]),
    ["1", "2", "3"],
  );
});

test("projection collector rejects a cursor loop", async () => {
  await assert.rejects(
    collectMkProjection(
      { projection: "heads", kinds: [30803], community: "hub.test" },
      async () => ({ events: [], nextCursor: "same" }),
    ),
    /repeated cursor/,
  );
});
