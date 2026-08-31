# MK Ideas Buzz private deployment

This directory is the fork-owned, three-person production topology. It is a
preparation artifact, not an active deployment. Nothing here purchases a host,
changes DNS, connects an external account, publishes an app, or sends a message.

## Topology

- Caddy terminates TLS and exposes only ports 80/443.
- The Buzz relay owns current state, history, search, media authorization, and
  Team synchronization.
- PostgreSQL, Redis, and MinIO are isolated on an internal Docker network.
- Pairing is an opt-in profile used only during an approved enrollment window.
- The deterministic/local agent runner is an opt-in profile and uses a distinct
  capability-limited identity.
- Push is disabled by default and cannot start without the explicit `push`
  profile and provider secrets.
- Prometheus, exporters, blackbox probes, and Alertmanager are an opt-in profile;
  their UIs bind to loopback.
- Maintenance jobs create a logical database dump, mirror private media, and
  write encrypted Restic snapshots to an off-server repository.

## Safe preparation

1. Copy `.env.example` to `.env` and replace invalid image/domain markers.
2. Create the file-mounted secrets described in `secrets/README.md`.
3. Pin every MK executable image by digest.
   Before production, convert every third-party infrastructure image tag to a
   reviewed digest as well.
4. Run `./verify-config.ps1` on Windows or `docker compose --env-file .env -f compose.yml --profile '*' config --quiet` on Linux.
5. Follow `docs/operations/MKIDEAS_PRODUCTION_RUNBOOK.md` only after deployment,
   DNS, TLS exposure, and infrastructure have separate authorization.

The normal core start is intentionally explicit:

```sh
docker compose --env-file .env -f compose.yml up -d postgres redis minio minio-init relay caddy
```

Optional profiles are started separately so an unavailable push provider or
agent credential cannot block the human operating product.
