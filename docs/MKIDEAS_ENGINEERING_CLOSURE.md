# MK Ideas Buzz Engineering Closure

Original closure date: 2026-08-31

Local completion update: 2026-09-16

Branch: `codex/mkideas-production-readiness`

Baseline: `c89c57cf`

## Latest macOS verification and launch preparation (2026-09-16)

This section supersedes the historical Windows host limitations below.

- Desktop and mobile enrollment now persist a scoped grant receipt. NIP-42
  login attaches an independently device-signed proof bound to the human,
  community, relay, and fresh challenge. Missing enrolled keys fail closed
  instead of silently generating replacements. Devices enrolled by the older
  build need receipt migration/re-enrollment handling before enforcement.
- Fixed two mobile enrollment/settings compile errors missed by the prior
  verification claim: the Nostr key property and the `AppListRow` import.
  Current Flutter analysis is clean; all 191 relay/MK Ideas tests and 15
  MK Ideas widget tests pass.
- Fixed the upstream activity-badge smoke test to exercise the actual MK Ideas
  sidebar instead of its removed Agents menu. The test now passes.
- Added Linux startup-on-boot and daily encrypted-backup systemd units,
  immutable infrastructure image overrides, and a deployment preflight with
  three passing regression tests. Alertmanager now has external egress.
- Desktop native compile and E2E build passed. These are local checks, not
  proof of signed phone distribution or a running production host.
- Both native device-proof regression tests passed, including signature
  verification and human/relay/challenge binding.
- The mobile enrollment-to-login contract test passed with a mocked HTTP
  transport, verified NIP-98/device signatures, and a secure-storage receipt.
- Local phone packaging is currently blocked: `flutter doctor -v` reports
  no Android SDK and an incomplete Xcode installation. No physical phone is
  connected. GitHub Actions is enabled, but signed distribution credentials
  and the production relay address have not been supplied.

Remaining engineering: complete surviving-device and owner-successor recovery
ceremonies; migrate existing enrollment receipts; close identity-key-only
enrollment and all alternate authentication paths before enabling device
enforcement. Keep the deployment in audit mode meanwhile. Signed phone builds,
push delivery, production hosting, DNS/TLS, off-server restore, alert delivery,
and partner acceptance still need their real accounts and environment.

See [always-on deployment and downloadable apps](operations/MKIDEAS_ALWAYS_ON.md).

## Scope and boundaries

This closure pass completed and verified the safe work that was locally
achievable on Windows. It did not push or merge branches, rename the
repository, deploy infrastructure, alter DNS, connect external accounts,
publish applications or content, access production Command Center data, or
perform the final cutoff.

The implementation reuses NIP-MK, the generic query/event submission paths,
the existing PostgreSQL projections, Buzz pubsub connection controls, Blossom
media, and current client repositories. It adds no event kind, authoritative
store, or entity-specific HTTP API.

## Closure status

| Requirement | Status | Evidence and remaining boundary |
|---|---|---|
| Pinned desktop dependency installation | PASS | Frozen pnpm install completed through a workspace-local store. The complete desktop suite passed 5,821 tests and exited naturally. Typecheck, policy checks, production build, admin-web build, and focused Playwright smoke passed. |
| Dependency vulnerability gate | PASS | Production pnpm audit reports no known vulnerabilities after lockfile-pinned overrides for affected transitive packages. |
| Shared state, history, concurrency, approval, DNC, and service capabilities | PASS | Real PostgreSQL/Redis/relay tests passed for two human signers, live subscription, stale writes, paginated history, atomic approval, owner-only DNC clearing, and scoped service grants. |
| Operation queries beyond 500 records | PASS | The existing generic projection now uses stable keyset pagination. A real relay-backed test published and retrieved 501 same-timestamp operation events without omission or duplication. |
| NIP-01 rate-limit rejection behavior | PASS | Rejected `EVENT` writes now receive `OK false`; subscription requests receive `CLOSED`. Unit and real client evidence confirm writes no longer wait for a timeout. |
| Exact device-session revocation at the relay | PASS | Durable grant revocation now broadcasts the exact human/device pair, closes only that authenticated session, preserves sibling sessions, and rechecks the grant after authentication. |
| Five-area desktop product, Quick Capture, search, Team links, and deterministic agents | PASS — EXTERNAL VALIDATION STILL REQUIRED | Desktop unit/build/Playwright evidence passed and nine distinct visual states were captured. Real three-partner use and mobile runtime parity still require external devices and participants. |
| Five proposal-only agents | PASS — EXTERNAL VALIDATION STILL REQUIRED | Deterministic provider and capability tests cover all five personas and human gates. Hosted provider credentials, model selection, budget, and research-network policy remain external. |
| Migration export, dry-run, plan, apply/resume, media, and destination reconciliation | PASS — EXTERNAL VALIDATION STILL REQUIRED | Export is read-only/repeatable-read and secret-excluding; apply uses existing Blossom and relay paths; reconciliation pages signed receipts. Two synthetic runs produced the same hash with 39 units, one media descriptor, and no duplicates or failures. Production snapshot access and cutoff are not authorized. |
| Local Compose syntax and relay-backed services | PASS | Compose rendered for every profile. Disposable PostgreSQL, Redis, MinIO, and relay services reached readiness and ran the relay-backed tests. No production infrastructure was changed. |
| PostgreSQL backup and isolated restore | PASS — EXTERNAL VALIDATION STILL REQUIRED | A custom-format dump restored into a separate disposable database. All 77 public tables and representative record counts matched. Off-server encrypted storage, media recovery, and a production restore exercise remain external. |
| Desktop/mobile durable offline cache, outbox, and conflict-resolution UI | PASS — RUNTIME ACCEPTANCE STILL REQUIRED | Both clients cache the last relay snapshot, persist the exact signed event before publishing, retry queued events after reconnect, preserve conflicts, and expose explicit reapply/discard controls. Unit, type, format, and analyzer checks pass; physical-device offline/reconnect acceptance remains external. |
| Client device enrollment, inventory, revocation, and NIP-49 backup/restore | PASS — ENFORCEMENT ROLLOUT STILL REQUIRED | Desktop and mobile now generate independent device keys in secure storage, use a short-lived hash-bound two-key challenge, list grant history, and revoke with an explicit reason. Existing NIP-49 creation, testing, and restore remain the identity-recovery path. The surviving-device replacement and owner-attributed successor ceremonies are still server foundations only and must be completed before switching `BUZZ_MK_DEVICE_GRANTS` from audit to enforce. |
| APNs/FCM delivery | BLOCKED — EXTERNAL CREDENTIAL/HARDWARE | Fake/local contracts remain testable, but provider accounts, credentials, profiles, and physical devices were not available or authorized. |
| Flutter analysis, tests, and Android runtime | BLOCKED — EXTERNAL CREDENTIAL/HARDWARE | Changed Dart files pass formatting in the pinned Dart 3.11 container. The host has no functional Flutter SDK, Android SDK/emulator, or ADB, so Flutter analysis/tests and runtime evidence could not run. |
| macOS/iOS runtime and signing | BLOCKED — EXTERNAL CREDENTIAL/HARDWARE | Windows has no Xcode/macOS environment, Apple signing account, or physical iPhone acceptance environment. |
| Unsigned Windows NSIS smoke artifact | BLOCKED — EXTERNAL CREDENTIAL/HARDWARE | Host Rust/Hermit and NSIS executables are unavailable; Rust checks were completed in the pinned container, but a native Windows installer could not be built or smoked. |
| Production data, partners, VPS, DNS, signing, hosted AI, and cutoff | BLOCKED — EXTERNAL CREDENTIAL/HARDWARE | These remain separately authorized external actions by design. |

## Verification record

Completed gates include:

- Rust formatting and focused strict Clippy for the changed relay, database,
  pubsub, and admin crates.
- Real relay-backed shared-head and 501-operation pagination workflows.
- PostgreSQL integration tests for MK state, migration, device, approval, DNC,
  service capability, history, and compatibility behavior.
- Desktop typecheck, Biome/policy checks, 5,821-test suite with natural exit,
  production build, admin-web build, and focused Playwright smoke.
- Nine visually distinct Today, Work, People, Studio, Team, Search, Quick
  Capture, approval, and agent states under
  `desktop/test-results/mkideas-v0/`.
- Dart formatting for the changed mobile repository/provider/test files.
- Event-registry synchronization, file-size gates, production dependency
  audit, Compose profile verification, migration determinism, and a generated
  CycloneDX source SBOM.

The local relay pagination test intentionally raises only the disposable test
relay's human/service rate limits so it can submit 501 events quickly. Product
defaults remain unchanged.

## Host constraints

Docker Desktop was usable through its installed absolute CLI path without
altering global PATH, WSL state, volumes, VHDX files, or unrelated containers.
Hermit could not provide a functional native Windows Rust/Flutter toolchain,
so Linux Rust and Dart container equivalents were used where compatible.
Apple-only and Android-runtime checks remain limited to their platform lanes.

## Overall result

The existing MK Ideas architecture, desktop operating experience, shared
relay state, pagination, agents, and migration path are materially hardened
and relay-backed. The locally achievable offline/conflict and core
device-management gaps are closed. Production is still gated on a rehearsed
device-enforcement/recovery rollout plus the external mobile, signing, push,
infrastructure, data, partner-acceptance, hosted-AI, and cutoff work listed
above.
