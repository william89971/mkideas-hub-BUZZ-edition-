import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildCommitLink,
  buildIssueLink,
  buildMkIdeasLink,
  buildProjectLink,
  buildPullRequestLink,
  buildRepoLink,
  entityLinkProjectRouteId,
  ENTITY_LINK_TABS,
  isEntityLink,
  isLinkableCoordinate,
  mkIdeasAreaForKind,
  mkIdeasLabelForKind,
  parseEntityLink,
} from "./entityLink.ts";

const GOLDEN = JSON.parse(
  readFileSync(
    new URL("../../../../test-fixtures/entity-links.json", import.meta.url),
    "utf8",
  ),
);
const OWNER = GOLDEN.owner;
const EVENT_ID = GOLDEN.eventId;

// This fixture is also consumed by buzz-cli and the Tauri deep-link validator.
test("builders emit the canonical cross-language link format", () => {
  assert.equal(
    buildPullRequestLink({ id: EVENT_ID, owner: OWNER, dtag: GOLDEN.dtag }),
    GOLDEN.links.pullRequest,
  );
  assert.equal(
    buildIssueLink({ id: EVENT_ID, owner: OWNER, dtag: GOLDEN.dtag }),
    GOLDEN.links.issue,
  );
  assert.equal(
    buildRepoLink({ owner: OWNER, dtag: GOLDEN.dtag }),
    GOLDEN.links.repository,
  );
  assert.equal(
    buildProjectLink({ owner: OWNER, dtag: GOLDEN.dtag }),
    GOLDEN.links.project,
  );
  assert.equal(
    buildCommitLink({
      commitHash: EVENT_ID,
      owner: OWNER,
      dtag: GOLDEN.dtag,
    }),
    GOLDEN.links.commit,
  );
  assert.deepEqual(ENTITY_LINK_TABS, GOLDEN.tabs);
  for (const dtag of GOLDEN.validDtags) {
    assert.equal(isLinkableCoordinate(OWNER, dtag), true);
  }
  for (const dtag of GOLDEN.invalidDtags) {
    assert.equal(isLinkableCoordinate(OWNER, dtag), false);
  }
});

test("builders reject invalid identifiers", () => {
  assert.throws(() =>
    buildRepoLink({ owner: "not-a-pubkey", dtag: "buzz-world" }),
  );
  assert.throws(() => buildRepoLink({ owner: OWNER, dtag: ".hidden" }));
  assert.throws(() => buildRepoLink({ owner: OWNER, dtag: "a..b" }));
  assert.throws(() =>
    buildPullRequestLink({ id: "short", owner: OWNER, dtag: "buzz-world" }),
  );
});

test("parseEntityLink round-trips built links", () => {
  const link = buildPullRequestLink({
    id: EVENT_ID,
    owner: OWNER,
    dtag: "buzz-world",
  });
  assert.deepEqual(parseEntityLink(link), {
    ok: true,
    value: { type: "pr", id: EVENT_ID, owner: OWNER, dtag: "buzz-world" },
  });

  const repoLink = buildRepoLink({ owner: OWNER, dtag: "buzz-world" });
  assert.deepEqual(parseEntityLink(repoLink), {
    ok: true,
    value: { type: "repo", owner: OWNER, dtag: "buzz-world" },
  });

  const projectLink = buildProjectLink({ owner: OWNER, dtag: "buzz-world" });
  assert.deepEqual(parseEntityLink(projectLink), {
    ok: true,
    value: { type: "project", owner: OWNER, dtag: "buzz-world" },
  });
});

test("MK Ideas links round-trip the canonical community-aware coordinate", () => {
  const id = "11111111-1111-4111-8111-111111111111";
  const proposalId = "22222222-2222-4222-8222-222222222222";
  const eventId = "a".repeat(64);
  const link = buildMkIdeasLink({
    community: "relay.mkideas.org",
    kind: 30800,
    id,
    eventId,
    proposalId,
  });
  assert.equal(
    link,
    `buzz://mkideas/entity?community=relay.mkideas.org&kind=30800&d=${id}&event=${eventId}&proposal=${proposalId}`,
  );
  assert.deepEqual(parseEntityLink(link), {
    ok: true,
    value: {
      type: "mkideas",
      community: "relay.mkideas.org",
      kind: 30800,
      id,
      eventId,
      proposalId,
    },
  });
});

test("MK Ideas parser preserves legacy V0 links", () => {
  const id = "11111111-1111-4111-8111-111111111111";
  const proposalId = "22222222-2222-4222-8222-222222222222";
  assert.deepEqual(
    parseEntityLink(
      `buzz://mkideas?kind=30803&id=${id}&proposal=${proposalId}`,
    ),
    {
      ok: true,
      value: {
        type: "mkideas",
        kind: 30803,
        id,
        proposalId,
        legacy: true,
      },
    },
  );
  assert.deepEqual(
    parseEntityLink(
      `buzz://mkideas/entity?community=relay.mkideas.org&kind=30803&d=${id}&event=short`,
    ),
    { ok: false, reason: "invalid-mkideas-event" },
  );
  assert.deepEqual(
    parseEntityLink(
      `buzz://mkideas/entity?community=bad%20host&kind=30803&d=${id}`,
    ),
    { ok: false, reason: "invalid-mkideas-community" },
  );
  assert.deepEqual(parseEntityLink(`buzz://mkideas?kind=30900&id=${id}`), {
    ok: false,
    reason: "invalid-mkideas-kind",
  });
});

test("MK Ideas kinds map to the permanent product areas", () => {
  assert.deepEqual(
    [30800, 30801, 30802, 30806, 30807].map(mkIdeasAreaForKind),
    ["work", "work", "work", "work", "work"],
  );
  assert.equal(mkIdeasAreaForKind(30803), "people");
  assert.equal(mkIdeasAreaForKind(30804), "studio");
  assert.equal(mkIdeasAreaForKind(30805), "studio");
  assert.equal(mkIdeasAreaForKind(30808), "today");
  assert.equal(mkIdeasAreaForKind(30809), "today");
  assert.equal(mkIdeasLabelForKind(30801), "Operational project");
});

test("commit links select an exact repository commit", () => {
  const link = buildCommitLink({
    commitHash: EVENT_ID,
    owner: OWNER,
    dtag: "buzz-world",
  });
  assert.equal(
    link,
    `buzz://repo?owner=${OWNER}&d=buzz-world&tab=commits&commit=${EVENT_ID}`,
  );
  assert.deepEqual(parseEntityLink(link), {
    ok: true,
    value: {
      type: "repo",
      owner: OWNER,
      dtag: "buzz-world",
      tab: "commits",
      commitHash: EVENT_ID,
    },
  });
  assert.deepEqual(
    parseEntityLink(
      `buzz://repo?owner=${OWNER}&d=buzz-world&tab=files&commit=${EVENT_ID}`,
    ),
    { ok: false, reason: "invalid-commit" },
  );
});

test("parseEntityLink lowercase-normalizes hex identifiers", () => {
  const parsed = parseEntityLink(
    `buzz://issue?id=${EVENT_ID.toUpperCase()}&owner=${OWNER.toUpperCase()}&d=buzz-world`,
  );
  assert.deepEqual(parsed, {
    ok: true,
    value: { type: "issue", id: EVENT_ID, owner: OWNER, dtag: "buzz-world" },
  });
});

test("parseEntityLink rejects malformed links", () => {
  const cases = [
    ["not a url at all", "invalid-url"],
    [`https://pr?id=${EVENT_ID}&owner=${OWNER}&d=repo`, "wrong-scheme"],
    [`buzz://message?channel=x&id=${EVENT_ID}`, "wrong-host"],
    [`buzz://pr?id=${EVENT_ID}&owner=nope&d=repo`, "invalid-owner"],
    [`buzz://pr?id=${EVENT_ID}&owner=${OWNER}&d=.hidden`, "invalid-dtag"],
    [`buzz://pr?id=${EVENT_ID}&owner=${OWNER}`, "invalid-dtag"],
    [`buzz://pr?owner=${OWNER}&d=repo`, "invalid-id"],
    [`buzz://issue?id=short&owner=${OWNER}&d=repo`, "invalid-id"],
  ];
  for (const [href, reason] of cases) {
    assert.deepEqual(parseEntityLink(href), { ok: false, reason }, href);
  }
});

test("isEntityLink matches entity hosts and excludes message links", () => {
  assert.equal(isEntityLink(`buzz://pr?id=${EVENT_ID}`), true);
  assert.equal(isEntityLink(`buzz://issue?id=${EVENT_ID}`), true);
  assert.equal(isEntityLink(`buzz://repo?owner=${OWNER}`), true);
  assert.equal(isEntityLink(`buzz://project?owner=${OWNER}`), true);
  assert.equal(
    isEntityLink(
      "buzz://mkideas/entity?community=relay.mkideas.org&kind=30803&d=11111111-1111-4111-8111-111111111111",
    ),
    true,
  );
  assert.equal(isEntityLink("buzz://message?channel=x&id=y"), false);
  assert.equal(isEntityLink("https://github.com/block/buzz"), false);
  assert.equal(isEntityLink(null), false);
});

test("entityLinkProjectRouteId emits the canonical 30617 coordinate route id", () => {
  const parsed = parseEntityLink(
    buildRepoLink({ owner: OWNER, dtag: "buzz-world" }),
  );
  assert.ok(parsed.ok);
  assert.equal(
    entityLinkProjectRouteId(parsed.value),
    `30617:${OWNER}:buzz-world`,
  );
});

test("entityLinkProjectRouteId routes project links to the 30621 coordinate", () => {
  const parsed = parseEntityLink(
    buildProjectLink({ owner: OWNER, dtag: "buzz-world" }),
  );
  assert.ok(parsed.ok);
  assert.equal(
    entityLinkProjectRouteId(parsed.value),
    `30621:${OWNER}:buzz-world`,
  );
});

test("coordinate links carry an optional workspace tab", () => {
  const link = buildProjectLink({
    owner: OWNER,
    dtag: "buzz-world",
    tab: "prs",
  });
  assert.equal(link, `buzz://project?owner=${OWNER}&d=buzz-world&tab=prs`);
  assert.deepEqual(parseEntityLink(link), {
    ok: true,
    value: { type: "project", owner: OWNER, dtag: "buzz-world", tab: "prs" },
  });

  const repoLink = buildRepoLink({
    owner: OWNER,
    dtag: "buzz-world",
    tab: "issues",
  });
  assert.deepEqual(parseEntityLink(repoLink), {
    ok: true,
    value: { type: "repo", owner: OWNER, dtag: "buzz-world", tab: "issues" },
  });

  // The default overview has no tab spelling; unknown values are rejected
  // rather than silently dropped, and event links accept no tab at all.
  assert.throws(() =>
    buildRepoLink({ owner: OWNER, dtag: "buzz-world", tab: "overview" }),
  );
  assert.deepEqual(
    parseEntityLink(`buzz://repo?owner=${OWNER}&d=buzz-world&tab=overview`),
    { ok: false, reason: "invalid-tab" },
  );
  assert.deepEqual(
    parseEntityLink(`buzz://repo?owner=${OWNER}&d=buzz-world&tab=`),
    { ok: false, reason: "invalid-tab" },
  );
  assert.deepEqual(
    parseEntityLink(
      `buzz://pr?id=${EVENT_ID}&owner=${OWNER}&d=buzz-world&tab=prs`,
    ),
    { ok: false, reason: "unknown-param" },
  );
});

test("isLinkableCoordinate gates coordinates the link format cannot express", () => {
  assert.equal(isLinkableCoordinate(OWNER, "buzz-world"), true);
  assert.equal(isLinkableCoordinate(OWNER, "a".repeat(64)), true);
  // Addressable d-tags allow far more than the link charset does.
  assert.equal(isLinkableCoordinate(OWNER, "a".repeat(65)), false);
  assert.equal(isLinkableCoordinate(OWNER, "has space"), false);
  assert.equal(isLinkableCoordinate(OWNER, ".hidden"), false);
  assert.equal(isLinkableCoordinate("not-a-pubkey", "buzz-world"), false);
});

test("parseEntityLink rejects noncanonical extras", () => {
  // Unexpected path segments — reserved for future versioning.
  assert.deepEqual(
    parseEntityLink(
      `buzz://pr/ignored?id=${EVENT_ID}&owner=${OWNER}&d=buzz-world`,
    ),
    { ok: false, reason: "unexpected-path" },
  );
  // Fragment — not part of the canonical format.
  assert.deepEqual(
    parseEntityLink(`buzz://repo?owner=${OWNER}&d=buzz-world#section`),
    { ok: false, reason: "unexpected-fragment" },
  );
  // Unknown query parameter — reject to preserve forward-compat posture.
  assert.deepEqual(
    parseEntityLink(
      `buzz://repo?owner=${OWNER}&d=buzz-world&relay=wss%3A%2F%2Frelay.example`,
    ),
    { ok: false, reason: "unknown-param" },
  );
  // Duplicate required parameter — reject.
  assert.deepEqual(
    parseEntityLink(`buzz://repo?owner=${OWNER}&d=buzz-world&owner=${OWNER}`),
    { ok: false, reason: "duplicate-param" },
  );
});
