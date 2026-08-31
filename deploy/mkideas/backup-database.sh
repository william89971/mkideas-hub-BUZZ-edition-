#!/bin/sh
set -eu

test -s /run/secrets/postgres_password || {
  echo "missing required secret: postgres_password" >&2
  exit 78
}
export PGPASSWORD="$(tr -d '\r\n' < /run/secrets/postgres_password)"
install -d -m 0700 /staging/database
pg_dump --host postgres --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
  --format custom --no-owner --no-privileges --file /staging/database/postgres.dump
pg_restore --list /staging/database/postgres.dump > /staging/database/postgres.contents

