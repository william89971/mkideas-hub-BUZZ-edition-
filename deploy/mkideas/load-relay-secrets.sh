#!/bin/sh
set -eu

read_secret() {
  name="$1"
  path="/run/secrets/$name"
  test -s "$path" || { echo "missing required secret: $name" >&2; exit 78; }
  tr -d '\r\n' < "$path"
}

POSTGRES_PASSWORD="$(read_secret postgres_password)"
REDIS_PASSWORD="$(read_secret redis_password)"
export BUZZ_RELAY_PRIVATE_KEY="$(read_secret relay_private_key)"
export BUZZ_S3_ACCESS_KEY="$(read_secret s3_access_key)"
export BUZZ_S3_SECRET_KEY="$(read_secret s3_secret_key)"
export DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}"
export REDIS_URL="redis://:${REDIS_PASSWORD}@redis:6379"

exec /usr/local/bin/buzz-relay

