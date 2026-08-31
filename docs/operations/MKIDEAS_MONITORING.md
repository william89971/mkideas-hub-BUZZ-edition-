# MK Ideas Buzz monitoring and alert checklist

The `monitoring` Compose profile exposes Prometheus and Alertmanager only on
loopback. Access them by an approved SSH tunnel or private access layer. The
default Alertmanager receiver stores alert state locally and sends nothing to an
external provider.

## Automated local signals

- Relay metrics availability.
- Relay and MinIO readiness through blackbox probes.
- PostgreSQL and Redis exporter availability.
- High PostgreSQL connection count.
- Redis memory pressure when a limit is configured.

## Production checklist

Add measured alerts and dashboards for:

- public TLS expiry and external relay reachability;
- disk, inode, database, media, and backup growth;
- newest successful backup age and last restore-drill date;
- relay errors, reconnects, write rejection reasons, and audit-chain failures;
- MK head/revision projection drift and search-index lag;
- media upload/hash/authorization failures;
- agent run success, duration, timeout, failure, and approved spend;
- notification/push delivery only after those providers are enabled;
- Caddy 4xx/5xx rates with privacy-preserving, redacted logs.

Do not put event content, names, contact details, private keys, transcript text,
media URLs, device tokens, or signed authorization headers into metric labels or
logs. Alert delivery to email/chat/paging is an external communication and is
not configured by this repository.

