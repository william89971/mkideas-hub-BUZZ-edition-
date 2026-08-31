#!/bin/sh
set -eu
PASSWORD="$(tr -d '\r\n' < /run/secrets/redis_password)"
export REDIS_ADDR=redis://redis:6379
export REDIS_PASSWORD="$PASSWORD"
exec /redis_exporter

