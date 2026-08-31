#!/bin/sh
set -eu
PASSWORD="$(tr -d '\r\n' < /run/secrets/postgres_password)"
export DATA_SOURCE_NAME="postgresql://${POSTGRES_USER}:${PASSWORD}@postgres:5432/${POSTGRES_DB}?sslmode=disable"
exec /bin/postgres_exporter

