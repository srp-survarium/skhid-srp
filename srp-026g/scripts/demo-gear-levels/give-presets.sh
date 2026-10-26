#!/usr/bin/env bash
# Demo (branch demo/bot-gear-levels): put the bots' 15 gear-level kits on
# local accounts' presets through the local admin form (tab=levels):
# preset n, kit k is gear level 3(n-1)+k, with the items it needs given.
# The kits worn before stop being a preset (their items stay owned).
#
# Usage: give-presets.sh [account ...]
#   With no account: every account of private/local-clients/config.json.
#   SRP_ADMIN_URL   (default http://127.0.0.1:18088/admin-local)
#   SRP_ADMIN_TOKEN (default local-demo-token)
#
# Run it with the local srp running; sign the account in (again) after it,
# so the lobby shows the new presets.
set -euo pipefail

url=${SRP_ADMIN_URL:-http://127.0.0.1:18088/admin-local}
token=${SRP_ADMIN_TOKEN:-local-demo-token}
config=${LOCAL_CLIENTS_CONFIG:-$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)/../private/local-clients/config.json}

if [ "$#" -eq 0 ]; then
    mapfile -t accounts < <(jq -r '.accounts[].name' "$config")
else
    accounts=("$@")
fi

for account in "${accounts[@]}"; do
    body=$(curl -sS --fail "$url" \
        --data-urlencode "token=$token" \
        --data-urlencode "tab=levels" \
        --data-urlencode "item_user=$account")
    line=$(printf '%s' "$body" | grep -o "$account: [^<]*" | head -n 1 || true)
    echo "${line:-$account: no answer (wrong token, account or URL?)}"
done
