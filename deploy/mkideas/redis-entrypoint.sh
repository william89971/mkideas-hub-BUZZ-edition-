#!/bin/sh
set -eu
test -s /run/secrets/redis_password || { echo "missing redis_password" >&2; exit 78; }
PASSWORD="$(tr -d '\r\n' < /run/secrets/redis_password)"
exec redis-server --appendonly yes --requirepass "$PASSWORD"

