# MK Ideas Buzz Local Acceptance

## Purpose

This runbook is the product-first acceptance gate before production accounts,
DNS, signing, provider credentials, real partner onboarding, or the final
Command Center cutoff are authorized. It distinguishes relay-backed evidence
from mock UI evidence and keeps external gates explicit.

## Product surface

The permanent operating areas are:

1. Today for ranked approvals, assignments, deadlines, blocked work, mentions,
   agent outcomes, upcoming records, and the operations briefing.
2. Work for goals, operational projects, tasks, meetings, and decisions.
3. People for guest profiles, relationship state, research, draft outreach,
   DNC, interviews, provenance, and linked discussion.
4. Studio for interviews, immutable transcript descriptors, timestamp search,
   clip and caption proposals, review, linked content, and history.
5. Team for Buzz channels, DMs, threads, files, forums, canvases, huddles, and
   stable MK entity links.

Quick Capture and typed Search are universal. Agents, knowledge, saved views,
and administration remain contextual.

## Relay-backed acceptance

Run PostgreSQL and Redis locally, start the relay against an isolated database,
and then run:

```sh
cargo test -p buzz-test-client --test e2e_mkideas -- --ignored --nocapture
```

The 501-event pagination case may raise only the disposable test relay's
`BUZZ_RATE_LIMIT_HUMAN_WS_EVENTS_PER_SEC`,
`BUZZ_RATE_LIMIT_HUMAN_MESSAGES_PER_MIN`, and
`BUZZ_RATE_LIMIT_AGENT_STANDARD_MESSAGES_PER_MIN` values. Do not change the
product defaults to make this test pass. Rate-limited `EVENT` submissions must
receive NIP-01 `OK false` immediately rather than timing out.

The test must prove:

- Two independent human signers can advance one shared MK coordinate.
- A stale human update is rejected and the winning human signature is kept.
- Live subscriptions deliver the accepted update without manual refresh.
- `mk-heads`, `mk-history`, and `mk-operations` return signed events with
  cursor pagination, including more than 500 operations and a dense timestamp.
- A service grant is limited to its community, persona, and event kind.
- An agent proposal is accepted but an agent approval action is rejected.
- One human approval transaction stores the decision, resulting human state,
  projected approval status, head advancement, and audit effects atomically.

## Client acceptance

Desktop evidence must include typechecking, Biome/policy checks, unit tests, an
E2E build, and the focused Playwright workflow:

```sh
cd desktop
pnpm typecheck
pnpm check
pnpm test
pnpm build:e2e
pnpm exec playwright test tests/e2e/mkideas-v0.spec.ts --project=smoke
```

Mobile evidence must include formatting, analysis, the focused MK suites, and
the complete widget/unit suite:

```sh
cd mobile
dart format --output=none --set-exit-if-changed .
flutter analyze
flutter test test/features/mkideas test/shared/mkideas
flutter test
```

Visual evidence lives under `desktop/test-results/mkideas-v0/`. Each screenshot
must depict a distinct state; duplicate hashes are a failed visual gate.

## Migration acceptance

The migration library and `buzz-admin` commands must support inspect, export,
plan, dry-run, apply, resume, verify, reconcile, and report. Validation covers
deterministic IDs, idempotency, interruption, checksums, media exceptions,
provenance, receipt emission, relationship integrity, and source/destination
counts. A production rehearsal still requires a separately approved read-only
snapshot and attachment source.

## External gates that remain open

- iPhone/TestFlight and Android physical-device acceptance.
- Hosted AI credentials, model selection, budget, and network policy.
- APNs, Firebase, application signing, notarization, and store accounts.
- VPS purchase, DNS, TLS exposure, and production deployment.
- Real partner onboarding and three-person operational acceptance.
- Production Command Center export, reconciliation, and final read-only cutoff.
- Backup restoration against provisioned off-server storage.

No local acceptance result authorizes pushing, merging, repository renaming,
deploying, publishing, connecting external accounts, sending communications,
publishing content, or performing the final cutoff.
