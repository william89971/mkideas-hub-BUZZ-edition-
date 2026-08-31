#!/bin/sh
set -eu

if [ "${MKIDEAS_RESTORE_CONFIRMATION:-}" != "RESTORE_TO_ISOLATED_ENVIRONMENT" ]; then
  echo "Refusing restore: set MKIDEAS_RESTORE_CONFIRMATION=RESTORE_TO_ISOLATED_ENVIRONMENT" >&2
  exit 78
fi
if [ "${MKIDEAS_RESTORE_ENVIRONMENT:-}" = "production" ] || [ -z "${MKIDEAS_RESTORE_ENVIRONMENT:-}" ]; then
  echo "Refusing restore: MKIDEAS_RESTORE_ENVIRONMENT must name a non-production target" >&2
  exit 78
fi
if [ "$#" -ne 1 ]; then
  echo "usage: $0 <restic-snapshot-id>" >&2
  exit 64
fi

cd "$(dirname "$0")"
COMPOSE="docker compose --env-file .env -f compose.yml --profile maintenance"
SNAPSHOT="$1"

echo "Staging encrypted snapshot $SNAPSHOT for isolated target: $MKIDEAS_RESTORE_ENVIRONMENT"
$COMPOSE run --rm restic restore "$SNAPSHOT" --target /restore
echo "Restore bytes are staged only. Follow docs/operations/MKIDEAS_BACKUP_RESTORE.md"
echo "No database, media bucket, or running service was modified."

