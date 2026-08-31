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
contacting either system. `export` is the only source-reading command and
accepts its legacy PostgreSQL URL exclusively through an explicit JSON file:

```text
buzz-admin migration inspect --bundle <bundle-directory>
buzz-admin migration export --source-config <source.json> --output <new-bundle-directory>
buzz-admin migration dry-run --bundle <bundle-directory> --mapping <mapping.json>
buzz-admin migration plan --bundle <bundle-directory> --mapping <mapping.json> --plan <plan.json>
buzz-admin migration verify --bundle <bundle-directory> --mapping <mapping.json> --plan <plan.json> --report <report.json>
buzz-admin migration reconcile --bundle <bundle-directory> --mapping <mapping.json> --plan <plan.json> --report <report.json>
buzz-admin migration report --report <report.json>
```

The source configuration has this versioned shape:

```json
{
  "schema_version": 1,
  "database_url": "postgresql://read_only_user:REDACTED@legacy-host/database",
  "source_workspace_id": "00000000-0000-4000-8000-000000000000",
  "schema": "public",
  "media": [
    {
      "source": "transcript:legacy-id",
      "path": "C:/approved-export/transcript.vtt",
      "mime_type": "text/vtt"
    }
  ]
}
```

Keep this file outside the repository. The exporter opens one connection,
requires a repeatable-read read-only transaction, allowlists the supported
legacy tables, scopes every row to `source_workspace_id`, excludes forbidden
credential/session/OAuth columns before selection, writes canonical NDJSON,
and hashes every table and explicitly listed media file. The output directory
must not already exist. It never writes to the source database.

Apply/resume accept only explicit arguments; they never scan environment
variables and never contact the Command Center:

```text
buzz-admin migration apply --artifact <dry-run.json> --bundle <export-directory> --relay <wss-url> --service-key-file <key-file> --checkpoint-out <checkpoint.json>
buzz-admin migration resume --artifact <dry-run.json> --bundle <export-directory> --checkpoint <checkpoint.json> --relay <wss-url> --service-key-file <key-file> --checkpoint-out <checkpoint.json>
```

Pass `--auth-tag-file <tag.json>` only when the migration service uses a NIP-OA
membership delegation. Every submitted 48202 receipt embeds exactly one
separately signed imported event. The relay rechecks the exact dataset-scoped,
owner-associated, unexpired service grant inside the transaction and commits
the receipt, imported event, entity head/revision when applicable, and
`mk_migration_items` idempotency row together. Apply/resume have not been run
against production.

Ready attachments are revalidated against the matching bundle, uploaded through
the relay's existing Blossom endpoint, and written into the signed receipt only
after the relay descriptor matches the planned SHA-256, byte count, and MIME
type. The checkpoint advances after receipt acceptance. If delivery is
interrupted between upload and receipt, resume safely repeats the same
content-addressed upload.

`dry-run` emits a deterministic dependency-ordered plan, complete
event-shaped source mappings, media integrity outcomes, a resumable planning
checkpoint, and a reconciliation report to stdout. It reads no credentials and
performs no source, relay, or media writes. Pass `--checkpoint <checkpoint.json>`
to validate an interrupted planning prefix before regenerating the same result.
`reconcile` remains offline by default. Supplying `--relay` and
`--service-key-file` makes it page the existing MK operations projection and
validate every signed 48202 receipt against the immutable source coordinate,
hash, batch, destination kind, and addressable ID in the plan. Missing,
duplicate, malformed, and unexpected receipts fail reconciliation. It performs
no writes.

## Remaining external validation

The operator now includes validated Blossom media submission and paginated,
signed destination-receipt reconciliation. Before any production source
credential is used, run the documented PostgreSQL/Redis/media rehearsal against
an authorized snapshot and reconcile its reports with the MK Ideas team. The
final source access and cutoff remain separately authorized actions.

Run the focused tests from the workspace root:

```text
cargo test -p buzz-migration
cargo test -p buzz-admin migration::
```
