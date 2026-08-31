#!/bin/sh
set -eu

cd "$(dirname "$0")"
COMPOSE_FILE="${COMPOSE_FILE:-compose.yml}"
COMPOSE="docker compose --env-file .env -f ${COMPOSE_FILE} --profile maintenance"

test -f .env || { echo "deploy/mkideas/.env is required" >&2; exit 78; }

echo "Creating consistent logical database dump"
$COMPOSE run --rm backup-database

echo "Mirroring private media into the encrypted-backup staging volume"
$COMPOSE run --rm backup-media

echo "Writing encrypted off-server Restic snapshot"
$COMPOSE run --rm restic backup /source/staging /source/relay-git \
  --tag mkideas-buzz --host "${MKIDEAS_BACKUP_HOST:-mkideas-primary}"

echo "Applying retention: 30 daily, 12 weekly, 12 monthly recovery points"
$COMPOSE run --rm restic forget --tag mkideas-buzz \
  --keep-daily 30 --keep-weekly 12 --keep-monthly 12 --prune

echo "Verifying repository metadata and newest snapshot"
$COMPOSE run --rm restic check --read-data-subset=5%
$COMPOSE run --rm restic snapshots --tag mkideas-buzz --latest 1

