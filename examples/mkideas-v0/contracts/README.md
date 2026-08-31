# MK Ideas agent contract fixtures

This directory is a pre-relay contract boundary for the five MK Ideas agent
personas. It deliberately lives outside the Rust event registry until NIP-MK v2
ingest and persistence land.

`agent-capabilities.v2.json` is machine-readable policy input for fixtures and
future code generation. It is not authorization by itself. The relay must bind
the signing service pubkey to an owner-controlled managed-agent persona and
apply an equivalent capability table server-side.

The golden files under `../fixtures/agents/v2/` use unsigned event-shaped
payloads. Fixed UUIDs, hashes, timestamps, and the `fixture` provider keep them
repeatable and offline. They demonstrate:

- exact target event IDs and monotonic versions;
- immutable input IDs and an aggregate input hash;
- structured provenance;
- proposal-only service output;
- retry, failure, cancellation, and timeout visibility;
- stale-result metadata when the current head advances;
- an informational `48203` operations briefing that cannot mutate state.

Run `node ../tests/validate-agent-contract-fixtures.mjs` from this directory,
or use the repository-root command documented in `../README.md`.
