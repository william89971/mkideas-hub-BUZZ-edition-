#!/bin/sh
set -eu

read_secret() {
  name="$1"
  path="/run/secrets/$name"
  test -s "$path" || { echo "missing required backup secret: $name" >&2; exit 78; }
  tr -d '\r\n' < "$path"
}

export RESTIC_PASSWORD="$(read_secret restic_password)"
export AWS_ACCESS_KEY_ID="$(read_secret backup_s3_access_key)"
export AWS_SECRET_ACCESS_KEY="$(read_secret backup_s3_secret_key)"
exec restic "$@"

