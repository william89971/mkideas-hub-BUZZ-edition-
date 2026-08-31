#!/bin/sh
set -eu

test -s /run/secrets/agent_private_key || {
  echo "missing required secret: agent_private_key" >&2
  exit 78
}
export BUZZ_PRIVATE_KEY="$(tr -d '\r\n' < /run/secrets/agent_private_key)"
exec /usr/local/bin/sprig-entrypoint

