# Downloadable apps and an always-available command center

The iPhone/Android app is the client. The Buzz relay, database, and private
media store run on a dedicated Linux server, independent of anyone's laptop.
Phones may sleep or close the app; the server stays available. Push delivery
requires the separate Apple/Google setup and real-device acceptance.

An existing website can keep its current hosting. Add a subdomain such as
`hub.<existing-domain>` and point only that DNS record at the Buzz server.
Do not change the website's apex, `www`, or email DNS records. A permanent
domain provides the stable address for client configuration and HTTPS.

## Deployment inputs still required

- Existing domain and DNS provider; selected command-center subdomain.
- Linux host address and administration access, with approved hosting costs.
- Owner public key; partner public keys (never identity private keys).
- Reviewed immutable images, private secret files, and off-server backup storage.
- Alert recipient/delivery configuration and an operator who will respond.
- Apple/Google app identifiers, signing access, distribution choice, and test phones.

## Start on boot and run daily backups

Install this checkout at `/opt/mkideas` on the selected Linux host. Configure
`deploy/mkideas/.env` and secret files first. Override infrastructure images
using `MKIDEAS_CADDY_IMAGE`, `MKIDEAS_POSTGRES_IMAGE`, `MKIDEAS_REDIS_IMAGE`,
`MKIDEAS_MINIO_IMAGE`, `MKIDEAS_MINIO_CLIENT_IMAGE`, `MKIDEAS_PROMETHEUS_IMAGE`,
`MKIDEAS_ALERTMANAGER_IMAGE`, `MKIDEAS_BLACKBOX_IMAGE`,
`MKIDEAS_POSTGRES_EXPORTER_IMAGE`, `MKIDEAS_REDIS_EXPORTER_IMAGE`, and
`MKIDEAS_RESTIC_IMAGE` with reviewed `@sha256:` references.

Run the static preflight with Node.js:

```sh
cd /opt/mkideas/deploy/mkideas
node scripts/preflight.mjs
```

It rejects placeholder deployment values, mutable images, public internal
ports, unprotected secret files, missing off-server backup storage, and the
default alert receiver that delivers nowhere. It never prints secret bytes.
Passing is configuration evidence, not proof of uptime or successful recovery.

After migrations and initial acceptance, install the prepared systemd units:

```sh
sudo install -m 0644 systemd/mkideas-buzz.service /etc/systemd/system/
sudo install -m 0644 systemd/mkideas-backup.service /etc/systemd/system/
sudo install -m 0644 systemd/mkideas-backup.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now mkideas-buzz.service mkideas-backup.timer
```

The service requires Docker, starts core and monitoring services, and waits
for readiness. Docker restarts crashed containers. The backup timer runs daily
at 03:00 UTC with a small randomized delay and catches up after downtime.
`flock` prevents overlapping backups. Stopping the service preserves volumes.
Install Docker Compose v2, Node.js, and util-linux (`flock`) on the host first.

## Acceptance before inviting the team

1. Check HTTPS and app login over cellular data from outside the server network.
2. Reboot the server; verify services and app access recover automatically.
3. Create a record on one phone and verify it appears on another.
4. Test offline edits, reconnection, conflicts, and revoked-device rejection.
5. Run a backup, restore to an isolated environment, and compare records/media.
6. Trigger a test outage and prove a real notification reaches the operator.
7. Install signed candidates on actual iPhone/Android devices and test push.

Keep device enforcement in `audit` until all clients send the enrolled proof,
recovery is rehearsed, and the enrollment/recovery control plane is hardened
against identity-key-only enrollment after enforcement. Audit mode observes
device grants but does not block clients whose grants are invalid or missing.

The supplied systemd units and DNS plan have not been installed on a production
host. A single server remains a single point of failure; backups and monitoring
provide recovery and detection, not zero-downtime failover.
