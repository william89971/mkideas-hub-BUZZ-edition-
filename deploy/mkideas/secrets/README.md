# MK Ideas deployment secrets

This directory is a local mount point, never a source-controlled secret store.
Create the following files with mode `0600` on the deployment host. Every file
contains only the value, followed by one newline.

| File | Requirement |
|---|---|
| `postgres_password` | 32+ random URL-safe characters |
| `redis_password` | 32+ random URL-safe characters |
| `s3_access_key` | private MinIO access key |
| `s3_secret_key` | 32+ random MinIO secret |
| `relay_private_key` | 32-byte relay Nostr key as lowercase hex |
| `agent_private_key` | dedicated, capability-limited agent key |
| `restic_password` | 32+ random Restic repository password |
| `backup_s3_access_key` | write-only backup destination key where possible |
| `backup_s3_secret_key` | backup destination secret |

Push-only files are not created until provider use is approved:

- `push_grant_keys`
- `push_token_keys`
- `apns_identity.pem`
- `app_attest_root.pem`

Generate values with an audited password manager or operating-system CSPRNG.
Do not paste values into `.env`, shell history, issue comments, Actions logs, or
this repository. Back up the relay identity and Restic password in the approved
offline recovery system before the first production start.

