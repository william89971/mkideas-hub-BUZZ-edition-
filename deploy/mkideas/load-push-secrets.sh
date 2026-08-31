#!/bin/sh
set -eu

read_secret() {
  name="$1"
  path="/run/secrets/$name"
  test -s "$path" || { echo "missing required push secret: $name" >&2; exit 78; }
  tr -d '\r\n' < "$path"
}

POSTGRES_PASSWORD="$(read_secret postgres_password)"
export DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}"
export BUZZ_PUSH_GRANT_KEYS="$(read_secret push_grant_keys)"
export BUZZ_PUSH_TOKEN_KEYS="$(read_secret push_token_keys)"
exec /usr/local/bin/buzz-push-gateway

