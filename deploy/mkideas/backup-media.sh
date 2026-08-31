#!/bin/sh
set -eu

read_secret() {
  name="$1"
  path="/run/secrets/$name"
  test -s "$path" || { echo "missing required secret: $name" >&2; exit 78; }
  tr -d '\r\n' < "$path"
}

ACCESS_KEY="$(read_secret s3_access_key)"
SECRET_KEY="$(read_secret s3_secret_key)"
mc alias set source http://minio:9000 "$ACCESS_KEY" "$SECRET_KEY"
mkdir -p /staging/media
mc mirror --overwrite --remove "source/${BUZZ_S3_BUCKET}" /staging/media

