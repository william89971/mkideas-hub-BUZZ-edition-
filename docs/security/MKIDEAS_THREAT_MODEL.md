# MK Ideas Buzz Threat Model

Status: local architecture foundation; production enforcement remains disabled.

## 1. Overview

MK Ideas Buzz is a private operating environment for three partners. Signed
Nostr events cross a client-to-relay boundary, the relay resolves the community
from the connection host, and PostgreSQL stores community-scoped device,
recovery, identity-attribution, notification, and operational state. The relay
operator is trusted to process workspace plaintext; this design does not claim
server-blind end-to-end encryption for operational records.

The model covers the staged device-grant and notification foundation. Media,
agent, migration, release, and production-provider statements below are
conditional architecture requirements when those workflows are enabled. A
sequential source review was used for this isolated lane; it was not a separate
independent repository-wide security audit.

| Component | Responsibility and boundary | Source evidence |
| --- | --- | --- |
| Core contracts | Validate two-key enrollment, recovery-bundle descriptors, successor attribution, quiet hours, and content-free dedupe keys. | `crates/buzz-core/src/mkideas_device.rs:164`, `crates/buzz-core/src/mkideas_device.rs:268`, `crates/buzz-core/src/mkideas_notification.rs:104`, `crates/buzz-core/src/mkideas_notification.rs:130` |
| Relay authentication | Parse a challenge-, relay-, human-, device-, and server-community-bound proof after NIP-42 authentication, then apply staged device-grant policy. | `crates/buzz-relay/src/handlers/auth.rs:86`, `crates/buzz-relay/src/handlers/auth.rs:295`, `crates/buzz-relay/src/device_security.rs:162`, `crates/buzz-relay/src/device_security.rs:215` |
| PostgreSQL projections | Consume enrollment challenges, store grants, recover from an exact active surviving grant, revoke durably, retain inventory, and store notification preferences/dedupe claims. | `crates/buzz-db/src/store/device_grants.rs:114`, `crates/buzz-db/src/store/device_grants.rs:152`, `crates/buzz-db/src/store/device_grants.rs:398`, `crates/buzz-db/src/store/device_grants.rs:489`, `crates/buzz-db/src/store/mkideas_notifications.rs:94` |
| Push seam | Accept only a fixed reconnect signal and opaque installation identifier for fake APNs/FCM transports. No provider adapter is connected. | `crates/buzz-relay/src/mkideas_notifications.rs:12`, `crates/buzz-relay/src/mkideas_notifications.rs:21`, `crates/buzz-relay/src/mkideas_notifications.rs:43` |

| Deployment or workflow | Resource or capability | Configuration and precedence | Safe effective value or location | Recipients | Enforcing control | Evidence or unknowns |
| --- | --- | --- | --- | --- | --- | --- |
| Current local/default | Physical-device grant enforcement | `BUZZ_MK_DEVICE_GRANTS`, default when absent | `off` | Auth pipeline | Existing NIP-42 behavior is preserved | `crates/buzz-relay/src/device_security.rs:22`, `crates/buzz-relay/src/device_security.rs:82` |
| Audit rollout | Grant validation telemetry | `BUZZ_MK_DEVICE_GRANTS=audit` | Allow while recording missing/invalid/active outcomes | Relay operators | Mode-specific decision in authentication | `crates/buzz-relay/src/device_security.rs:215` |
| Future production | Required active device grant | `BUZZ_MK_DEVICE_GRANTS=enforce`; enrollment acknowledgement, closed membership, and owner are mandatory | Exact active grant for human, device, and community | Auth pipeline | Startup fails closed if prerequisites are absent | `crates/buzz-relay/src/device_security.rs:82`, `crates/buzz-relay/src/config.rs:802` |
| Notification provider test seam | Reconnect wake | Internal transport contract only | Provider enum plus opaque installation ID | Fake APNs/FCM transport | Validation rejects malformed identifiers; no content fields exist | `crates/buzz-relay/src/mkideas_notifications.rs:21`, `crates/buzz-relay/src/mkideas_notifications.rs:47` |

## 2. Threat Model, Trust Boundaries, and Assumptions

### Assets and security objectives

- Human Nostr identities and historical human attribution.
- Independent device keys, expiring/revocable grants, and device inventory.
- Recovery-bundle ciphertext and metadata; the relay never receives a recovery
  passphrase or decrypted key.
- Service identities constrained by community, owner, purpose, persona, kind,
  target, run/dataset identifier, and expiry.
- Entity revisions, approvals, DNC state, search projections, private media,
  transcripts, migration receipts, notification preferences, and dedupe state.
- The invariant that the server-resolved community, not an untrusted client
  field, authorizes every sensitive operation.

### Actors and boundaries

- A partner controls a human signing key and may authorize an independent
  device key. They do not gain owner-only DNC or successor authority merely by
  being a member.
- A stolen device attacker may control one enrolled device key and cached local
  data, but not another surviving device, the owner key, relay configuration,
  or service grant authority.
- A compromised agent or migration runner controls only its service key and
  permitted inputs. Service-identity bypass of the physical-device gate does
  not bypass event-level capability validation (`crates/buzz-db/src/store/device_grants.rs:80`).
- A malicious community member may know identifiers from another community,
  but must not use them to query, recover, revoke, deduplicate, or authorize
  cross-community state.
- The trusted relay operator can access plaintext in transit termination,
  databases, indexes, and media processing. TLS, storage access control,
  encrypted backups, audit, and operator policy mitigate this accepted
  boundary; they do not remove it.
- APNs/FCM, signing systems, production infrastructure, and external AI
  providers are outside the current local implementation and require separate
  credentials and authorization.

### Assumptions and unresolved controls

- Production enrollment completion and a recovery exercise occur before
  enforcement is enabled. Enforcement is intentionally not enabled by code or
  configuration in this lane.
- Revocation currently disconnects all live sessions for the human after the
  durable grant update; unaffected devices may reconnect. Exact per-device
  live-session termination remains client/session-registry work
  (`crates/buzz-relay/src/device_security.rs:278`).
- Rate-limit keys and conservative defaults are defined, but Redis admission is
  not wired yet (`crates/buzz-relay/src/device_security.rs:62`).
- Client enrollment, inventory, NIP-49 bundle creation, surviving-device
  recovery UI, offline conflict UX, and production push adapters remain future
  work. The relay validates descriptors but never decrypts a bundle.
- Media safety claims require the separate private-media admission and parsing
  paths to enforce byte, MIME, hash, path, authorization, and parser bounds;
  this device-security lane did not independently validate those paths.

## 3. Attack Surface, Mitigations, and Attacker Stories

These are prioritized hypotheses and design obligations, not confirmed
vulnerability findings.

| Priority | Scenario and capability gain | Prerequisites | Impact | Existing controls | Mitigation / remaining work | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| High | A stolen enrolled device continues authenticating after its owner revokes it. | Attacker has device key and a prior valid grant. | Continued private workspace access and signed writes. | Exact active-grant lookup; durable revoke increments the auth epoch before cluster disconnect. | Add exact per-device session registry and complete client inventory/recovery UX before enforcement. | `crates/buzz-relay/src/device_security.rs:215`, `crates/buzz-relay/src/device_security.rs:278`, `crates/buzz-db/src/store/device_grants.rs:398` |
| High | A valid proof is replayed against a different community, relay, challenge, human, or device. | Attacker observes or controls a signed proof. | Cross-community or stale-session authentication. | Payload equality, signatures, exact challenge digest, server-resolved community, and relay URL are validated; challenges are consumed transactionally. | Retain cross-community and replay conformance tests at every auth change. | `crates/buzz-core/src/mkideas_device.rs:164`, `crates/buzz-relay/src/device_security.rs:162`, `crates/buzz-db/src/store/device_grants.rs:152` |
| High | A compromised service key impersonates a partner or approves/publishes protected state. | Runner/service key compromise. | Human-authority bypass or external action. | Physical-device bypass is limited to active owner-associated services; event writes remain capability checked. Relay ingest separately enforces service capabilities. | Rotate/revoke narrow grants, monitor unexpected persona/volume/community, retain proposal-only agent tests. | `crates/buzz-db/src/store/device_grants.rs:80`, `crates/buzz-relay/src/handlers/ingest.rs:548` |
| High | A known grant, recovery, preference, or dedupe identifier is used across communities. | Member in one community knows another identifier. | Tenant data leakage or authorization bypass. | Community is part of table keys and query predicates; strict PostgreSQL integration test exercises two communities. | Extend the same result-level fence to media/search/session caches. | `migrations/0049_mkideas_device_security.sql:17`, `migrations/0049_mkideas_device_security.sql:37`, `migrations/0049_mkideas_device_security.sql:103`, `crates/buzz-db/tests/mkideas_device_security.rs:1` |
| Medium | A malicious attachment or transcript abuses filename paths, parser resources, or rendering. | Authorized member uploads crafted bytes. | Data disclosure, resource exhaustion, or code execution in a parser/rendering surface. | Architecture requires opaque object keys, authorization, hashes, MIME/size limits, bounded parsers, and untrusted rendering. | Validate the separate media implementation with malicious corpus and request-size tests before production. | Conditional architecture requirement; not independently validated in this lane. |
| Medium | Provider push leaks operational content or amplifies duplicate work. | Push transport and provider accounts are later connected. | Metadata disclosure or notification flooding. | Contract contains only provider plus opaque installation ID; semantic dedupe includes community/recipient/class/entity/version; quiet hours suppress delivery, not Today state. | Build fork-owned adapters without expanding payload; test retry/dedupe and provider logs with real accounts later. | `crates/buzz-relay/src/mkideas_notifications.rs:21`, `crates/buzz-core/src/mkideas_notification.rs:104`, `crates/buzz-core/src/mkideas_notification.rs:130`, `crates/buzz-db/src/store/mkideas_notifications.rs:94` |
| Medium | Enrollment/recovery abuse exhausts relay or database resources, or secrets enter logs. | Network reachability plus repeated malformed attempts. | Denial of service or credential/privacy disclosure. | Redacted, community-fenced rate keys and bounded local defaults; errors avoid proof contents. | Wire Redis admission, add log-capture assertions, secret/dependency scans, and production alerting. | `crates/buzz-relay/src/device_security.rs:62` |
| Low | A recovery request silently rewrites historical identity attribution. | Owner or surviving device initiates recovery. | Loss of audit meaning. | Recovery creates new grants; successor identity is a separate owner-signed attribution record with an authorization event ID. | Surface provenance in clients and audit recovery/successor operations. | `crates/buzz-db/src/store/device_grants.rs:489`, `crates/buzz-db/src/store/device_grants.rs:658`, `migrations/0050_mkideas_successor_proof.sql:5` |

### Logging invariant

Authentication events, enrollment/recovery proofs, raw challenges, NIP-49
ciphertext, passphrases, provider tokens, media URLs, credentials, contact
fields, and transcript text must not enter logs. Device-security rate-limit keys
hash their subject and include the community. Local defaults permit ten
enrollment, five recovery, and ten recovery-bundle attempts per subject per
hour; these are scaffolding until Redis admission is connected.

### Verification matrix

- Pure contract tests cover proof signatures and payload equality, challenge
  and relay/community binding, NIP-49 prefix/digest/size, owner-signed
  successor attribution, quiet hours, tenant/recipient dedupe, and content-free
  fake APNs/FCM transports.
- PostgreSQL integration covers cross-community grant, inventory, recovery,
  preference, dedupe, and revocation behavior.
- Relay tests cover the off/audit/enforce startup matrix, challenge/relay/
  community proof binding, redacted rate keys, and fake provider boundaries.
- Remaining production verification includes exact device-session termination,
  Redis limits, malicious-media tests, log capture, dependency/secret scans,
  client enrollment/recovery, physical push delivery, and staged enforcement.

## 4. Severity Calibration (Critical, High, Medium, Low)

- **Critical:** unauthenticated or ordinary-member compromise of all community
  human identities, owner authority, or production signing/recovery roots.
  Operator plaintext access under the explicitly trusted-server model is not by
  itself critical unless it exceeds the configured operator boundary.
- **High:** cross-community read/write access, a revoked device retaining
  durable protected authority, or a service identity gaining human approval,
  DNC-clear, publish, or impersonation capability.
- **Medium:** single-community metadata disclosure, bounded denial of service,
  duplicate notifications, or malicious-media impact requiring an authorized
  uploader and a vulnerable parser. Effective rate limits, sandboxing, and
  content-minimal payloads can reduce likelihood but not impact.
- **Low:** self-only recoverable errors, audit/UI attribution gaps without an
  authority gain, or noisy operational behavior that does not expose content or
  cross a human gate.

## External gates

Finished APNs/FCM delivery requires fork-owned Apple/Firebase accounts and
physical-device testing. Production enforcement requires enrolling all active
clients and an owner-approved recovery exercise. Signing, notarization,
production infrastructure, DNS, partner participation, and provider credentials
remain separately authorized actions.

Repository: git-remote-sha256:84fb39da1d3e61636ac3864447efca7aad23ef57e9a617f2494f0d9d2990740c
Version: codex-security-snapshot/v1:sha256:97f0152bc606e11e07e22453a4ca60f879faf12c1a01db20928ee76218d6510c
