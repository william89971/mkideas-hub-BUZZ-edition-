# MK Ideas Buzz V0

## What this slice proves

V0 implements the private guest-to-content architecture without claiming V1
production readiness. Buzz remains the only ongoing system of record. Human
state changes retain the human signer; managed agents can append sourced
proposals but cannot approve, publish, send, or mutate protected state.

The permanent product navigation is Today, Work, People, Studio, and Team on
desktop and mobile. Search and capture remain contextual/universal rather than
additional permanent areas. The original V0 proof focused on Today, People,
Studio, and Team. The product-first V1 integration now makes Work operational
and keeps this document as the reproducible guest-to-content proof.

## Current product-first integration

The V1 integration branch extends this V0 path with goals, operational
projects, tasks, meetings, decisions, typed history and pagination, universal
Quick Capture, typed cross-product search, immutable private transcript
descriptors, all five human-gated agent personas, device-grant security seams,
notification projections, and the complete resumable migration command set.

The locally verified agent set is Guest Researcher, Outreach Drafter,
Interview Producer, Content / Clip Copilot, and Operations Briefing Assistant.
Every output remains a proposal or summary. Accepting protected state still
requires a separate human signature and relay transaction.

## Implemented V0 path

1. Create a guest in People on desktop or mobile. The client signs kind `30803`.
2. Run the managed Guest Researcher. It appends a kind `48201` proposal targeted
   at that guest, with proposal version and non-empty provenance.
3. Review the attached research in People or Today on another client.
4. Create an interview (`30804`) and upload a `.txt`, `.vtt`, or `.srt`
   transcript. The transcript advances the same shared interview head.
5. Create a content item (`30805`).
6. Run the managed Content / Clip Copilot against the interview or content item.
   Its proposal includes timestamped clip and caption drafts.
7. Approve or reject in Today or Studio. This creates a human-signed approval
   head (`30809`) and an append-only approval action (`48200`).
8. Use **Copy Team reference** and paste the `buzz://mkideas` link into a Buzz
   channel or DM. Desktop renders the reference as a native link and routes it
   back to People or Studio.
9. Use universal Search to find MK records, proposals, approvals, and linked
   Team discussion.

## Relay invariants

- V0 state is keyed by `(community_id, kind, d_tag)` in `mk_entity_heads`.
- Head advancement uses a row lock and checks signed `version` and `prev`.
- The accepted event is stored unchanged; the relay never re-signs a human
  update.
- Only owners/admins may write state or approvals.
- Only registered managed-agent identities may write proposals/system output.
- The relay checks relationship targets, required fields, legal transitions,
  do-not-contact, proposal provenance, proposal/approval linkage, and the
  human-only approval boundary.
- NIP-09 cannot delete MK state. Archiving is a typed state transition.

## Staging demonstration

Build the CLI and configure the normal Buzz authentication variables for the
private staging community:

```sh
cargo build --release -p buzz-cli
export BUZZ_RELAY_URL=wss://<private-staging-relay>
export BUZZ_PRIVATE_KEY=<owner-or-managed-agent-nsec>
```

Seed the stable synthetic V0 records with an owner/admin identity. Repeating
the command is safe:

```sh
buzz mk-ideas seed-demo
```

The stable IDs are:

- guest: `7f8d9c9f-2c1b-4f87-a4fe-cb4b639418b1`
- interview: `53566e42-c908-48c3-8e69-eb63acdb904c`
- content: `ebaaef77-2b1d-4f85-8a4f-c319458176ae`

Register/import the two personas in `examples/mkideas-v0`, then run proposals
from their managed identities. Example Guest Researcher output:

```sh
buzz mk-ideas propose \
  --target 7f8d9c9f-2c1b-4f87-a4fe-cb4b639418b1 \
  --target-kind 30803 \
  --agent "Guest Researcher" \
  --proposal-type guest-research \
  --summary "Maya connects practical editorial trust with small-team operating systems." \
  --provenance "https://example.org/source-one" \
  --provenance "MK Ideas guest criteria v1"
```

Example Content / Clip Copilot output:

```sh
buzz mk-ideas propose \
  --target ebaaef77-2b1d-4f85-8a4f-c319458176ae \
  --target-kind 30805 \
  --agent "Content / Clip Copilot" \
  --proposal-type timestamped-clips \
  --summary "Two transcript-grounded clips are ready for human review." \
  --provenance "mkideas-v0-synthetic-interview.vtt" \
  --clips-json '[{"start":"00:00:04.000","end":"00:00:18.000","title":"Conversations become memory","caption":"Useful ideas start as conversations worth remembering."}]'
```

The fixture transcript is at
`examples/mkideas-v0/fixtures/mkideas-v0-synthetic-interview.vtt`.

## Acceptance evidence and remaining external gates

The repository contains automated collision/contract tests, PostgreSQL
concurrency and migration tests, mobile analysis/tests, desktop type/build
checks, and a focused desktop Playwright flow with deterministic relay events
and distinct Today, Work, People, Studio, Team, Search, transcript, and agent
screenshots. A local relay-backed Rust acceptance test exercises two human
identities, a narrowly scoped agent identity, current-head and history
pagination, live subscriptions, stale-write rejection, agent self-approval
rejection, and an atomic human approval plus resulting state update.

The remaining V0 external evidence is the same workflow from an actual iPhone
or TestFlight build against an approved private staging relay. Provisioning
that relay, issuing releases, and using Apple signing remain separately
authorized actions under the approved plan. The local relay and desktop
evidence do not claim that external Apple gate has passed.

## Remaining production boundary

Three-partner production acceptance, real Command Center data rehearsal and
cutoff, advanced offline conflict UX, completed device enrollment/recovery UI,
real APNs/FCM delivery, backup restoration against provisioned infrastructure,
signed application artifacts, and production deployment remain externally or
operationally gated. External email/calendar/Drive/publishing adapters and
video rendering remain post-V1 and disabled.
