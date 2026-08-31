NIP-MK v2
=========

MK Ideas Operational State
--------------------------

`draft` `relay` `application-specific`

## Abstract

NIP-MK defines the private operational records used by MK Ideas Buzz. Human
state changes remain signed by the human who performed them. A Buzz relay
validates the change and transactionally projects one authoritative head per
`(community, kind, d)` even when different partners sign successive versions.

This is an MK Ideas extension to ordinary addressable-event behavior. Standard
NIP-01 coordinates include the author's pubkey and do not by themselves express
a shared cross-author record.

The machine-readable source of truth is
`docs/nips/mkideas-event-registry.json`. Rust, TypeScript, and Dart mirrors MUST
be drift-tested against it.

## Reserved kinds

`30800` through `30899` are reserved for addressable state:

| Kind | Record |
|------|--------|
| 30800 | goal |
| 30801 | operational project |
| 30802 | task |
| 30803 | person or guest |
| 30804 | interview |
| 30805 | content item |
| 30806 | meeting |
| 30807 | decision |
| 30808 | knowledge entry |
| 30809 | approval request |

`48200` through `48299` are reserved for operations and system events:

| Kind | Event |
|------|-------|
| 48200 | human approval action |
| 48201 | agent proposal or result |
| 48202 | migration receipt |
| 48203 | generated summary |
| 48204 | system activity or maintenance |
| 48205 | external communication history |

Buzz already uses `48001` for audit and `48100` through `48106` for huddles,
so NIP-MK does not claim `48000` through `48099`.

## Human-signed state envelope

Every state event MUST contain one `d`, `h`, `version`, and `status` tag.
`prev` MUST be absent on version 1 and MUST contain the exact previous accepted
event ID thereafter.

```jsonc
[
  ["d", "<stable entity UUID>"],
  ["h", "<normalized community host>"],
  ["version", "<positive monotonic integer>"],
  ["status", "<typed operational status>"],
  ["prev", "<previous authoritative event id>"]
]
```

The JSON content MUST include `schema_version`, `record_type`, `entity_id`,
`version`, `status`, `source`, and a non-empty `provenance` object. It MAY carry
typed assignees, deadline, and related-entity links. `entity_id`, `version`, and
`status` MUST agree with the signed tags. Schema v2 is the write contract;
schema-v1 person, interview, content, and approval records remain readable
during rolling upgrades.

Typed relationships are duplicated in signed tags:

- interview: `["guest", "<person UUID>"]`
- content: `["interview", "<interview UUID>"]` when it belongs to an interview
- approval: `["target", "<kind>", "<entity UUID>", "<target event id>"]`
  and `["proposal", "<proposal UUID>", "<proposal event id>"]`
- agent proposal: `["target", "<kind>", "<entity UUID>"]`

## Shared heads and immutable history

The relay maintains `mk_entity_heads` keyed by its internal
`(community_id, kind, d_tag)` and an append-only `mk_entity_revisions`
projection. Under a row lock it MUST:

1. verify membership and operational role;
2. verify signed `h`, `version`, and `prev` values;
3. validate schema, fields, DNC, links, and the legal transition;
4. store the signed event and immutable revision unchanged;
5. advance the head atomically without deleting the previous revision.

Two concurrent updates cannot both win. A stale update returns the current
version and event ID. Ordinary Nostr query, count, and search paths expose only
the projected head. `/query` extensions `mk_projection: "heads"`,
`mk_projection: "history"`, and `mk_projection: "operations"` return
`{events, nextCursor}`; plain filters retain their standard array response.
The operations cursor is an opaque keyset over `(created_at, event_id)`, so a
client can traverse more than one page even when many operation events share a
timestamp. Live subscriptions continue carrying signed events.

Archiving is a validated state transition, not NIP-09 deletion.

## Status vocabularies

- Goal: `draft`, `active`, `on-hold`, `completed`, `archived`.
- Project: `planned`, `active`, `blocked`, `completed`, `archived`.
- Task: `backlog`, `to-do`, `in-progress`, `blocked`, `review`, `done`, `cancelled`.
- Person: `prospect`, `researching`, `ready-to-contact`, `contacted`, `responded`, `scheduled`, `interviewed`, `nurture`, `closed`, `archived`.
- Interview: `idea`, `planning`, `scheduled`, `recorded`, `transcribing`, `reviewing`, `complete`, `cancelled`.
- Content: `idea`, `draft`, `in-review`, `approved`, `scheduled`, `published`, `archived`.
- Meeting: `planned`, `completed`, `cancelled`.
- Decision: `proposed`, `decided`, `superseded`, `archived`.
- Knowledge: `draft`, `verified`, `archived`.
- Approval: `pending`, `approved`, `rejected`, `stale`, `cancelled`.

Do-not-contact is an independent sticky person field. Clearing it is owner-only,
requires a reason, and produces audit evidence.

## Authority and service capabilities

Normal state and approval actions are owner/admin human events. The relay never
re-signs them. AI MAY research, draft, summarize, classify, extract, recommend,
and propose. AI MUST NOT approve, send, publish, clear DNC, impersonate a human,
or directly author protected state.

Schema-v2 service output requires an active, owner-associated, expiring and
revocable MK service grant. The grant binds purpose, allowed event kinds,
allowed target kinds, persona when applicable, and a dataset hash for
migration. Generic managed-agent status is not enough.

Historical imported state is the sole exception to human state authorship. It
requires `imported: true`, deterministic identity, immutable source provenance,
an explicit `state_import` capability, and an idempotency receipt written in
the same relay transaction. It never grants agents normal state authority.

## Approval transaction

A schema-v2 `30809` request binds its exact event, target coordinate and
event/version, plus a proposal UUID and event. A human-signed `48200` action
repeats these bindings and includes a reason. The relay locks the approval and
target projections and atomically stores the action, optional separately
human-signed result event, head update, decision projection, and audit evidence.
A changed target is stale. A service identity cannot publish `48200`.

## Media and transcripts

Schema-v2 state MUST NOT embed full transcript text. It carries immutable
private-media descriptors: object/media ID, SHA-256, MIME type, size, original
filename, uploader, upload time, source/provenance, and transcript
format/language/version. The relay validates authorization, bytes, digest, MIME
type, and size. Replacement uploads create new immutable versions. Authorized
transcript segments are derived for timestamp navigation and search.

## Team references

The canonical entity link is:

```text
buzz://mkideas/entity?community=<h>&kind=<kind>&d=<entity UUID>
```

An optional `event=<event id>` opens a historical revision. Today, Search,
Team, notifications, desktop, and mobile resolve the same link. Legacy
`buzz://mkideas?kind=...&id=...` links remain readable during migration.

## Device grants and recovery

Human sessions may carry an independent device key in addition to the human
identity. Enrollment consumes a short-lived, hash-only challenge after both
keys sign the exact community, human, device, challenge, platform, and device
metadata. Session proofs also bind the current NIP-42 challenge and relay URL.
Device grants are revocable, expiring, inventoried per human, and never portable
across communities.

The rollout modes are `off`, `audit`, and `enforce`; `off` is the default.
`enforce` is a startup error until closed relay membership, an owner recovery
authority, and an explicit existing-client enrollment acknowledgement are all
configured. Narrow, owner-associated service identities bypass the physical
device gate but remain capability-checked for every operational event.

Recovery from a surviving device binds the exact active grant and creates the
replacement grant transactionally. Offline bundles use client-encrypted NIP-49
material in private media; the relay stores only object metadata, hash, and
size and never receives the passphrase or plaintext key. If the identity is
unrecoverable, an owner-authorized successor record preserves the predecessor's
historical attribution rather than rewriting old signatures.

The complete trust boundary, abuse cases, and verification matrix are in
[`../security/MKIDEAS_THREAT_MODEL.md`](../security/MKIDEAS_THREAT_MODEL.md).
