#!/bin/sh
set -eu

test -s /run/secrets/postgres_password || {
  echo "missing required secret: postgres_password" >&2
  exit 78
}
POSTGRES_PASSWORD="$(tr -d '\r\n' < /run/secrets/postgres_password)"
export DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}"
exec /usr/local/bin/buzz-admin "$@"

