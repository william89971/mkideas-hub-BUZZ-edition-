# MK Ideas Buzz backup and restore

## Backup contents and retention

`deploy/mkideas/backup.sh` creates:

- a PostgreSQL custom-format logical dump and catalog listing;
- an authenticated mirror of the private media bucket;
- a read-only snapshot of relay Git storage;
- one encrypted, off-server Restic snapshot tagged `mkideas-buzz`.

Restic provides encryption before upload. The maintenance job applies 30 daily,
12 weekly, and 12 monthly recovery points, prunes unreferenced data, runs a
repository check, and prints the newest snapshot. A scheduler is intentionally
not installed by this repository; production scheduling is an infrastructure
action that requires approval.

The database dump and media mirror share a staging volume. This is suitable for
the three-person first topology, but it is not a globally atomic snapshot across
PostgreSQL and object storage. The migration receipt/event IDs and media hashes
must be reconciled after restore.

## Restore safety invariant

No repository script restores over the live environment. `stage-restore.sh`
requires both:

```sh
export MKIDEAS_RESTORE_CONFIRMATION=RESTORE_TO_ISOLATED_ENVIRONMENT
export MKIDEAS_RESTORE_ENVIRONMENT=mkideas-restore-drill-YYYYMMDD
```

It rejects an empty or `production` environment and only stages bytes. After
staging, an operator must create a separate Compose project, database volume,
media bucket, relay identity copy, and private hostname. Restore PostgreSQL with
`pg_restore --clean --if-exists --no-owner` only inside that isolated project.

## Quarterly restore drill

1. Select a snapshot without exposing its credentials in logs.
2. Stage it into a new, isolated Compose project.
3. Restore the database and media into new volumes/buckets.
4. Start the relay with external ingress disabled.
5. Verify row counts, MK head/history projections, search, media hashes, three
   representative entity chains, an approval, transcript timestamps, and Team
   links.
6. Record recovery-point age and recovery duration.
7. Destroy the isolated drill environment only after evidence is retained and
   the exact target is re-verified.

Production restore remains an irreversible action requiring separate approval.

