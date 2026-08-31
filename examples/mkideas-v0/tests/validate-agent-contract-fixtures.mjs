import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const packDir = path.resolve(testDir, "..");
const fixtureDir = path.join(packDir, "fixtures", "agents", "v2");

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const hex64Pattern = /^[0-9a-f]{64}$/;
const forbiddenOutputKeys = new Set([
  "approved",
  "decision",
  "published_at",
  "sent_at",
  "state_patch",
  "mutation",
]);

async function readJson(filePath) {
  return JSON.parse(await readFile(filePath, "utf8"));
}

function assertUuid(value, label) {
  assert.match(value, uuidPattern, `${label} must be a lowercase RFC 4122 v4 UUID`);
}

function assertHex64(value, label) {
  assert.match(value, hex64Pattern, `${label} must be 64 lowercase hex characters`);
}

function assertExactTarget(target, label) {
  assert.equal(typeof target, "object", `${label} target is required`);
  assert.ok(Number.isInteger(target.kind), `${label} target kind must be an integer`);
  assertUuid(target.id, `${label} target id`);
  assertHex64(target.event_id, `${label} target event id`);
  assert.ok(Number.isInteger(target.version) && target.version > 0, `${label} target version must be positive`);
}

function assertNoProtectedKeys(value, location = "output") {
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertNoProtectedKeys(item, `${location}[${index}]`));
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    assert.ok(!forbiddenOutputKeys.has(key), `${location}.${key} is a protected agent output key`);
    assertNoProtectedKeys(child, `${location}.${key}`);
  }
}

function assertRun(runEvent, capability, personaId) {
  assert.equal(runEvent.kind, 48204, `${personaId} run must use kind 48204`);
  const run = runEvent.content;
  assert.equal(run.schema_version, 2);
  assert.equal(run.activity_type, "agent_run");
  assertUuid(run.run_id, `${personaId} run id`);
  assert.equal(run.persona_id, personaId);
  assertHex64(run.agent_pubkey, `${personaId} agent pubkey`);
  assertHex64(run.owner_pubkey, `${personaId} owner pubkey`);
  assertUuid(run.community_id, `${personaId} community id`);
  assertHex64(run.request_event_id, `${personaId} request event id`);
  assert.equal(run.status, "succeeded");
  assert.equal(run.attempt, 1);
  assert.ok(Date.parse(run.started_at) < Date.parse(run.completed_at));
  if (capability.target_kinds.length > 0) {
    assertExactTarget(run.target, `${personaId} run`);
    assert.ok(capability.target_kinds.includes(run.target.kind), `${personaId} run target kind is not allowed`);
  } else {
    assert.equal(run.target, undefined, `${personaId} community summary must not invent an entity target`);
    assertHex64(run.snapshot.head_digest, `${personaId} snapshot digest`);
  }
  return run;
}

function assertProvenance(output, personaId) {
  assert.ok(Array.isArray(output.input_event_ids) && output.input_event_ids.length > 0);
  output.input_event_ids.forEach((eventId, index) =>
    assertHex64(eventId, `${personaId} input_event_ids[${index}]`),
  );
  assertHex64(output.input_hash, `${personaId} input hash`);
  assert.ok(Array.isArray(output.provenance) && output.provenance.length > 0);
  for (const [index, source] of output.provenance.entries()) {
    assert.ok(source.source_id, `${personaId} provenance[${index}] source_id is required`);
    assert.ok(source.source_type, `${personaId} provenance[${index}] source_type is required`);
    assert.ok(source.title, `${personaId} provenance[${index}] title is required`);
    assert.ok(source.locator, `${personaId} provenance[${index}] locator is required`);
    assert.ok(Date.parse(source.retrieved_at), `${personaId} provenance[${index}] retrieved_at is invalid`);
    assertHex64(source.sha256, `${personaId} provenance[${index}] hash`);
  }
}

function assertOutput(outputEvent, run, capability, personaId) {
  assert.ok(capability.output_event_kinds.includes(outputEvent.kind), `${personaId} output kind is not allowed`);
  assert.ok(outputEvent.kind === 48201 || outputEvent.kind === 48203);
  const output = outputEvent.content;
  assert.equal(output.schema_version, 2);
  assert.equal(output.run_id, run.run_id);
  assert.equal(output.persona_id, personaId);
  assert.equal(output.agent_pubkey, run.agent_pubkey);
  assert.equal(output.community_id, run.community_id);
  assert.equal(output.provider, "fixture", `${personaId} golden output must stay offline`);
  assert.equal(output.model, "deterministic-local-v1");
  assert.ok(output.template_version.endsWith(".v2"));
  assert.ok(Date.parse(output.started_at) < Date.parse(output.completed_at));
  assert.equal(output.usage.cost_microunits, 0);
  assertProvenance(output, personaId);
  assertNoProtectedKeys(output);

  if (outputEvent.kind === 48201) {
    assertUuid(output.proposal_id, `${personaId} proposal id`);
    assert.equal(output.proposal_version, 1);
    assert.ok(capability.proposal_types.includes(output.proposal_type));
    assert.deepEqual(output.target, run.target, `${personaId} output must preserve the exact requested target`);
    assert.ok(capabilities.agent_writable_proposal_statuses.includes(output.status));
    assert.equal(output.status, "proposed");
    assert.ok(capabilities.agent_writable_review_states.includes(output.review_state));
    assert.equal(output.review_state, "pending");
    assert.equal(capability.human_review_required, true);
  } else {
    assertUuid(output.summary_id, `${personaId} summary id`);
    assert.equal(output.summary_version, 1);
    assert.ok(capability.summary_types.includes(output.summary_type));
    assert.deepEqual(output.snapshot, run.snapshot);
    assert.equal(output.status, "informational");
    assert.equal(output.review_state, undefined);
  }
  return output;
}

function assertExpectedSafety(expected, personaId) {
  assert.equal(expected.network_calls, 0, `${personaId} fixture must not use a live provider`);
  assert.equal(expected.protected_state_events, 0, `${personaId} fixture must not write MK state`);
  assert.equal(expected.approval_action_events, 0, `${personaId} fixture must not approve`);
  assert.equal(expected.human_action_required_for_state_change, true);
}

const manifest = await readJson(path.join(packDir, ".plugin", "plugin.json"));
const capabilities = await readJson(path.join(packDir, "contracts", "agent-capabilities.v2.json"));
assert.equal(capabilities.schema_version, 2);

const expectedPersonaFiles = {
  "guest-researcher": "guest-researcher.persona.md",
  "outreach-drafter": "outreach-drafter.persona.md",
  "interview-producer": "interview-producer.persona.md",
  "content-clip-copilot": "content-clip-copilot.persona.md",
  "operations-briefing-assistant": "operations-briefing-assistant.persona.md",
};
assert.equal(Object.keys(capabilities.personas).length, 5);
assert.deepEqual(
  manifest.personas,
  Object.values(expectedPersonaFiles).map((file) => `agents/${file}`),
  "manifest must register all five approved personas in product order",
);

for (const [personaId, file] of Object.entries(expectedPersonaFiles)) {
  const source = await readFile(path.join(packDir, "agents", file), "utf8");
  assert.match(source, new RegExp(`name: ${personaId}`));
  assert.match(source, /Never |never /, `${personaId} prompt must state a hard negative boundary`);
  assert.match(source, /schema-v2/, `${personaId} prompt must require the v2 output envelope`);
  assert.doesNotMatch(source, /^model:/m, `${personaId} must not pin a live model`);
  assert.doesNotMatch(source, /^mcp_servers:/m, `${personaId} must not connect an external MCP provider`);
}

const goldenFiles = {
  "guest-researcher": "guest-researcher.golden.json",
  "outreach-drafter": "outreach-drafter.golden.json",
  "interview-producer": "interview-producer.golden.json",
  "content-clip-copilot": "content-clip-copilot.golden.json",
  "operations-briefing-assistant": "operations-briefing-assistant.golden.json",
};

for (const [personaId, file] of Object.entries(goldenFiles)) {
  const fixture = await readJson(path.join(fixtureDir, file));
  const capability = capabilities.personas[personaId];
  const run = assertRun(fixture.run_event, capability, personaId);
  const output = assertOutput(fixture.output_event, run, capability, personaId);
  assertExpectedSafety(fixture.expected, personaId);

  if (personaId === "outreach-drafter") {
    assert.equal(output.output.delivery_state, "not_sent");
    assert.equal(output.output.dnc_checked, true);
    assert.equal(output.output.dnc_active, false);
    assert.equal(fixture.expected.external_messages_sent, 0);
  }
  if (personaId === "content-clip-copilot") {
    assertHex64(output.output.transcript_sha256, "transcript hash");
    for (const clip of output.output.clips) {
      assert.ok(clip.start_ms >= 0 && clip.end_ms > clip.start_ms);
      assert.ok(clip.end_ms <= output.output.duration_ms, "clip must remain inside transcript duration");
      assert.ok(clip.verbatim.length > 0, "clip must preserve transcript-grounded text");
    }
    assert.equal(fixture.expected.rendered_videos, 0);
    assert.equal(fixture.expected.published_items, 0);
  }
}

const lifecycle = await readJson(path.join(fixtureDir, "lifecycle-outcomes.golden.json"));
assert.deepEqual(
  lifecycle.events.map((event) => event.content.status),
  ["retrying", "failed", "cancelled", "timed_out"],
);
for (const event of lifecycle.events) {
  assert.equal(event.kind, 48204);
  assert.equal(event.content.schema_version, 2);
  assert.equal(event.content.activity_type, "agent_run");
  assert.ok(
    capabilities.personas[event.content.persona_id].output_event_kinds.includes(event.kind),
    `${event.content.persona_id} may not emit lifecycle events`,
  );
  assert.ok(capabilities.run_statuses.includes(event.content.status));
  assertUuid(event.content.run_id, "lifecycle run id");
  assertExactTarget(event.content.target, "lifecycle run");
  assert.ok(event.content.error.code && event.content.error.message);
  assertNoProtectedKeys(event.content);
}
const retrying = lifecycle.events[0].content;
assert.equal(retrying.attempt, 2);
assert.equal(retrying.error.retryable, true);
assertHex64(retrying.previous_activity_event_id, "retry previous activity event id");
assert.equal(lifecycle.expected.result_events, 0, "terminal/error activity fixtures must not forge proposal results");
assert.equal(lifecycle.expected.human_work_blocked, false);

const staleFixture = await readJson(path.join(fixtureDir, "stale-proposal.golden.json"));
const stale = staleFixture.proposal_event.content;
assert.equal(staleFixture.proposal_event.kind, 48201);
assert.equal(stale.schema_version, 2);
assert.equal(stale.status, "proposed", "history retains the original proposal status");
assert.equal(stale.review_state, "stale");
assert.ok(capabilities.agent_writable_review_states.includes(stale.review_state));
assert.equal(stale.stale.expected_event_id, stale.target.event_id);
assert.equal(stale.stale.expected_version, stale.target.version);
assert.ok(stale.stale.current_version > stale.stale.expected_version);
assert.notEqual(stale.stale.current_event_id, stale.stale.expected_event_id);
assert.equal(staleFixture.expected.approvable, false);
assert.equal(staleFixture.expected.requires_fresh_run, true);
assertNoProtectedKeys(stale);

const discovered = (await readdir(fixtureDir)).filter((file) => file.endsWith(".golden.json")).sort();
assert.deepEqual(
  discovered,
  [...Object.values(goldenFiles), "lifecycle-outcomes.golden.json", "stale-proposal.golden.json"].sort(),
  "every golden fixture must be explicitly validated",
);

console.log(`PASS: validated ${Object.keys(expectedPersonaFiles).length} personas and ${discovered.length} deterministic v2 fixture files`);
