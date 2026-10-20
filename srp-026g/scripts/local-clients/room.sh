#!/usr/bin/env bash
# Fake members of the local server's rooms (local tests only; the server
# must run with SRP_FAKE_ROOMS set, see docs/SRP-026G.md "Fake rooms").
#
#   room.sh                     every room, its members and its queue
#   room.sh show [ROOM]         one room
#   room.sh add [player] [ROOM]     a fake member more in the room
#   room.sh remove [player] [ROOM]  a fake member out: a queued player moves in
#
# ROOM is a room's name (its maker's) or its key; none: the first fake room.
# It calls the local web admin (tab=fake_room). Its port, path and token come
# from the environment (SRP_WEB_PORT, SRP_ADMIN_PATH, SRP_ADMIN_TOKEN), else
# from the local launcher's private/local-clients/config.json (needs jq),
# else 18088, /admin-local and local-demo-token.
set -euo pipefail

action=${1:-show}
shift || true
if [[ ${1:-} == player ]]; then
  shift
fi
room=${*:-}
case $action in
  show | add | remove) ;;
  -h | --help | help)
    sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  *)
    echo "room.sh: unknown action '$action' (show, add, remove)" >&2
    exit 2
    ;;
esac

config=${SRP_LOCAL_CONFIG:-}
if [[ -z $config ]]; then
  here=$(cd "$(dirname "$0")" && pwd)
  # The main checkout's private/ (a worktree has none of its own).
  if common=$(git -C "$here" rev-parse --path-format=absolute --git-common-dir 2>/dev/null); then
    config="$(dirname "$common")/private/local-clients/config.json"
  fi
fi
from_config() {
  if [[ -n $config && -f $config ]] && command -v jq >/dev/null; then
    jq -r "$1 // empty" "$config"
  fi
}

port=${SRP_WEB_PORT:-$(from_config '.ports[3]')}
path=${SRP_ADMIN_PATH:-$(from_config '.server.env.SRP_ADMIN_PATH')}
token=${SRP_ADMIN_TOKEN:-$(from_config '.server.env.SRP_ADMIN_TOKEN')}
port=${port:-18088}
path=${path:-/admin-local}
token=${token:-local-demo-token}

curl -sS --max-time 10 -X POST "http://127.0.0.1:${port}${path}" \
  --data-urlencode "token=${token}" \
  --data-urlencode "tab=fake_room" \
  --data-urlencode "action=${action}" \
  --data-urlencode "room=${room}"
