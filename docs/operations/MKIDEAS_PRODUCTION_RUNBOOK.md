# MK Ideas Buzz production preparation runbook

Status: local preparation only. Do not run production steps without separate
authorization for the VPS, DNS, TLS exposure, external accounts, and deployment.

## Deployment contract

The initial target is one dedicated Docker Compose VPS for William and two
operational-admin partners. `deploy/mkideas/compose.yml` keeps the data plane on
an internal Docker network. Caddy is the only public listener. The relay is the
only continuing writable system of record; the old Command Center is never part
of the running topology.

Before any first start:

1. Record the approved host, region, provider, budget, and operator list.
2. Point `MKIDEAS_DOMAIN` only after DNS change authorization.
3. Build fork-owned relay, pairing, agent, and push images, scan them, and pin
   their immutable digests in `.env`.
4. Generate deployment secrets and store offline recovery copies of the relay
   key and Restic password.
5. Keep `BUZZ_PUSH_ENABLED=false`; do not create push secret files until APNs or
   Firebase use is approved.
6. Validate every Compose profile and verify the `.env` and `secrets/` paths are
   readable only by the deployment operator.

## Startup ordering and health

Compose starts PostgreSQL, Redis, and MinIO first, waits for their health checks,
runs the private bucket initializer, starts the relay, waits for relay readiness,
then starts Caddy. Do not bypass these dependencies with `--no-deps`.

Required checks:

```sh
docker compose --env-file .env -f deploy/mkideas/compose.yml ps
curl --fail --silent https://hub.example.invalid/.well-known/nostr.json
docker compose --env-file .env -f deploy/mkideas/compose.yml exec relay \
  bash -ec 'exec 3<>/dev/tcp/127.0.0.1/8080; printf "GET /_readiness HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n" >&3; grep "200 OK" <&3'
```

Pairing is started only for an enrollment window:

```sh
docker compose --env-file .env -f deploy/mkideas/compose.yml --profile pairing up -d pairing
# stop it after enrollment
docker compose --env-file .env -f deploy/mkideas/compose.yml stop pairing
```

Agent and push profiles are independent. Human work must remain available when
either profile is stopped.

## Upgrade

1. Rehearse on an isolated copy and read migrations/release notes.
2. Run a fresh encrypted backup and record its snapshot ID.
3. Replace only the intended immutable image digest in `.env`.
4. Pull the digest, then run
   `docker compose --env-file .env -f deploy/mkideas/compose.yml --profile maintenance run --rm admin migrate`
   and verify its output before restarting the relay. `BUZZ_AUTO_MIGRATE=false`
   is deliberate.
5. Start core services with normal dependency ordering.
6. Verify readiness, state projections, search, media, Team sync, and two-human
   shared edits before enabling optional profiles.
7. Record the old/new digests, migration version, backup snapshot, operator, and
   acceptance evidence.

## Rollback

Application rollback means restoring the prior immutable image digest only when
the database schema remains backward compatible. Never run an unreviewed down
migration. If a data restore is required, stop and follow the isolated restore
rehearsal in `MKIDEAS_BACKUP_RESTORE.md` before any production decision.

Push rollback is always safe at the relay boundary: set
`BUZZ_PUSH_ENABLED=false`, restart the relay, then stop the push profile. This
does not block human operations.

## External gates

- VPS purchase and provider credentials.
- DNS/TLS exposure and the real `hub.mkideas.org` value.
- Production image registry and signing keys.
- Apple, Firebase/Google, and Windows signing accounts.
- Real partner onboarding and production acceptance.
- Production Command Center export and final cutoff.
