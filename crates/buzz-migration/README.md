# buzz-migration

Offline-first migration planning primitives for moving the legacy MK Ideas
Command Center into MK Ideas Buzz.

This crate intentionally has no PostgreSQL, relay, or media client. It is a
workspace library used by `buzz-admin migration`; the operator binary owns the
explicitly credentialed relay submission adapter while the relay owns atomic
receipt/import persistence.

## Safety contract

- Source extraction is read-only and must use a repeatable-read transaction.
- Imported events are signed by a narrow migration service identity; original
  actors remain provenance and are never impersonated.
- UUIDv5 identities, canonical JSON, source hashes, and source coordinates make
  reruns deterministic.
- `authUserId`, invitation token hashes, sessions, OAuth material, database
  URLs, passwords, and private keys must never enter a bundle or report.
- Existing entities are never fuzzy-merged automatically.
- Dry-run performs no upload or event submission.
- Final Command Center cutoff remains a separately authorized operation.

## Operator commands

The planning surface validates artifacts without reading credentials or
contacting either system:

```text
buzz-admin migration inspect --bundle <bundle-directory>
buzz-admin migration dry-run --bundle <bundle-directory> --mapping <mapping.json>
buzz-admin migration plan --bundle <bundle-directory> --mapping <mapping.json> --plan <plan.json>
buzz-admin migration verify --bundle <bundle-directory> --mapping <mapping.json> --plan <plan.json> --report <report.json>
buzz-admin migration reconcile --bundle <bundle-directory> --mapping <mapping.json> --plan <plan.json> --report <report.json>
buzz-admin migration report --report <report.json>
```

`export` still fails closed with exit code 3 because no source adapter or
source credential is configured. Apply/resume accept only explicit arguments;
they never scan environment variables and never contact the Command Center:

```text
buzz-admin migration apply --artifact <dry-run.json> --relay <wss-url> --service-key-file <key-file> --checkpoint-out <checkpoint.json>
buzz-admin migration resume --artifact <dry-run.json> --checkpoint <checkpoint.json> --relay <wss-url> --service-key-file <key-file> --checkpoint-out <checkpoint.json>
```

Pass `--auth-tag-file <tag.json>` only when the migration service uses a NIP-OA
membership delegation. Every submitted 48202 receipt embeds exactly one
separately signed imported event. The relay rechecks the exact dataset-scoped,
owner-associated, unexpired service grant inside the transaction and commits
the receipt, imported event, entity head/revision when applicable, and
`mk_migration_items` idempotency row together. Apply/resume have not been run
against production.

`dry-run` emits a deterministic dependency-ordered plan, complete
event-shaped source mappings, media integrity outcomes, a resumable planning
checkpoint, and a reconciliation report to stdout. It reads no credentials and
performs no source, relay, or media writes. Pass `--checkpoint <checkpoint.json>`
to validate an interrupted planning prefix before regenerating the same result.
`reconcile` is deliberately offline-only and marks `relay_checked: false`;
it does not claim destination verification.

## Remaining integration work

1. Add an explicitly authorized, read-only legacy PostgreSQL source adapter.
2. Add a Blossom client that supports validated generic files and WebVTT.
3. Add destination query/reconciliation beyond receipt idempotency checks.
4. Add real PostgreSQL/Redis/media integration tests before any production
   source credential is used.

Run the focused tests from the workspace root:

```text
cargo test -p buzz-migration
cargo test -p buzz-admin migration::
```
