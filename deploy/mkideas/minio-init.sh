#!/bin/sh
set -eu
ACCESS_KEY="$(tr -d '\r\n' < /run/secrets/s3_access_key)"
SECRET_KEY="$(tr -d '\r\n' < /run/secrets/s3_secret_key)"
mc alias set local http://minio:9000 "$ACCESS_KEY" "$SECRET_KEY"
mc mb --ignore-existing "local/${BUZZ_S3_BUCKET}"
mc anonymous set none "local/${BUZZ_S3_BUCKET}"

