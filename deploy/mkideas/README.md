# MK Ideas Buzz private deployment

For downloadable phone apps, domain setup, server startup on boot, and the
daily backup timer, see [the always-on guide](../../docs/operations/MKIDEAS_ALWAYS_ON.md).
Run `node scripts/preflight.mjs` against the real deployment configuration
before installing the systemd units in `systemd/`.

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

1. Copy `.env.example` to `.env`, replace invalid image/domain markers, and set
   `RELAY_OWNER_PUBKEY` to William's 64-character hexadecimal **public** key.
   Never put a private key in `.env`.
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

The core profile is closed by default: REST token checks and relay membership
checks are mandatory, and the configured owner is bootstrapped idempotently.
Add each partner only after the relay is healthy, using the maintenance CLI and
the partner's public key:

```sh
docker compose --env-file .env -f compose.yml --profile maintenance run --rm admin \
  add-member --pubkey <PARTNER_64_HEX_PUBLIC_KEY> --role member
```

Do not share identities. Each partner signs in with their own key. The permanent
HTTPS URL is the relay and invitation surface; the five-area MK Ideas product is
used through the configured desktop or mobile client.
