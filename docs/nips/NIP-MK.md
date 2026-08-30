NIP-MK
======

MK Ideas Operational State
--------------------------

`draft` `relay` `application-specific`

## Abstract

NIP-MK defines the private operational records used by MK Ideas Buzz. Human
state changes remain signed by the human who performed them. A Buzz relay
validates the change and transactionally projects one authoritative head per
`(community, kind, d)` even when different authorized partners sign successive
versions.

This is an MK Ideas extension to ordinary addressable-event behavior. Standard
NIP-01 coordinates include the author's pubkey and therefore do not by
themselves express a shared cross-author record.

## Reserved kinds

`30800` through `30899` are reserved for addressable MK Ideas state:

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
| 30809 | approval record |

`48200` through `48299` are reserved for append-only operations and system
events:

| Kind | Event |
|------|-------|
| 48200 | human approval action |
| 48201 | agent proposal or result |
| 48202 | migration receipt |
| 48203 | generated summary |
| 48204 | system activity or maintenance |

Buzz already uses `48001` for audit events and `48100` through `48106` for
huddles. For that reason NIP-MK does not claim `48000` through `48099`.

## Human-signed state envelope

Every state event MUST contain exactly one of each of these tags:

```jsonc
[
  ["d", "<stable entity UUID>"],
  ["h", "<normalized community host>"],
  ["version", "<positive monotonic integer>"],
  ["status", "<typed operational status>"],
  ["prev", "<previous authoritative event id>"]
]
```

`prev` MUST be absent on version 1 and MUST be present thereafter. The `h` tag
is the public host-derived community identifier clients already know; the relay
MUST compare it with its server-resolved tenant. Clients MUST NOT submit the
relay's internal community database UUID.

The JSON content MUST include `schema_version`, `record_type`, `entity_id`,
`version`, and `status`. `entity_id` and `version` MUST agree with the signed
tags. The complete typed record is carried in each accepted version.

Typed relationships are duplicated in signed tags so clients can filter and
render links without parsing every payload:

- interview: `["guest", "<person UUID>"]`
- content: `["interview", "<interview UUID>"]`
- approval: `["target", "<kind>", "<entity UUID>"]` and
  `["proposal", "<proposal UUID>", "<proposal event id>"]`
- agent proposal: `["target", "<kind>", "<entity UUID>"]`

`status` and relationship tags MUST match the JSON payload. The relay rejects
missing, duplicate, or inconsistent typed tags.

## Shared-head extension

The relay maintains `mk_entity_heads`, keyed by its internal
`(community_id, kind, d_tag)` tuple. Under a row lock it MUST:

1. verify the caller's membership and operational role;
2. verify the signed `h`, `version`, and `prev` values;
3. validate required fields and the legal domain transition;
4. store the incoming signed event unchanged;
5. make the prior event non-current and advance the projection atomically.

Two concurrent updates to one head cannot both succeed. A stale update returns
a conflict with the current version and event id. Ordinary current-state reads,
search, and subscriptions see the single live accepted event. Historical audit
and operation events remain append-only.

An MK state record is archived by publishing a validated state transition to
`archived`. A NIP-09 deletion request MUST NOT delete or supersede MK state.

## Authority

State and approval events are accepted only from an MK Ideas owner or admin.
The relay never re-signs a human action with a service identity.

Agent proposals, generated summaries, migration receipts, scheduled actions,
and maintenance events MUST be signed by a registered managed-agent identity.
An agent proposal MUST use `status: "proposed"` and MUST NOT include an approval
decision.

AI MAY research, draft, summarize, classify, extract, recommend, and propose.
AI MUST NOT approve, send external communication, publish, bypass a human gate,
or silently change protected operational state. Accepting an AI proposal
creates a new human-signed state event and, when applicable, a human-signed
approval action.

## V0 profile

V0 enables person/guest (`30803`), interview (`30804`), content (`30805`), and
approval (`30809`) state plus approval actions (`48200`) and agent proposals
(`48201`). The remaining reserved state kinds are held for V1 and MUST be
rejected until their typed transition rules are implemented.

## Team references

An MK Ideas record can be referenced from Buzz Team/chat with:

```text
buzz://mkideas?kind=<30803|30804|30805|30809>&id=<entity UUID>
```

An optional `proposal=<proposal UUID>` keeps the discussion tied to a specific
agent result. The desktop composer preserves this link, the timeline renders it
as a Buzz-native chip/card, and activating it opens People for person records or
Studio for interview, content, and approval records.
