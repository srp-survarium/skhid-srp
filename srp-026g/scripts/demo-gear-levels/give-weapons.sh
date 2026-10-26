#!/usr/bin/env bash
# Demo (branch demo/ragdoll-bot-gear): give local accounts every weapon of
# the content (one of each) and 1000 units of every ammo that fits one,
# through the local admin form (tab=item). The list is weapons.tsv beside
# this script (item name, count), made from the content's game data.
#
# Usage: give-weapons.sh [account ...]
#   With no account: PocketTest1, CamoBob and CamoAlice.
#   SRP_ADMIN_URL   (default http://127.0.0.1:18088/admin-local)
#   SRP_ADMIN_TOKEN (default local-demo-token)
#
# Run it with the local srp running. A second run gives everything again
# (another weapon of each, 1000 more units). Sign the account in (again)
# after it if the lobby does not show the items.
set -euo pipefail

url=${SRP_ADMIN_URL:-http://127.0.0.1:18088/admin-local}
token=${SRP_ADMIN_TOKEN:-local-demo-token}
list="$(dirname "$0")/weapons.tsv"

if [ "$#" -eq 0 ]; then
    accounts=(PocketTest1 CamoBob CamoAlice)
else
    accounts=("$@")
fi

for account in "${accounts[@]}"; do
    given=0 refused=0
    while IFS=$'\t' read -r item count; do
        [ -n "$item" ] && [ "${item:0:1}" != "#" ] || continue
        body=$(curl -sS --fail "$url" \
            --data-urlencode "token=$token" \
            --data-urlencode "tab=item" \
            --data-urlencode "item_user=$account" \
            --data-urlencode "item=$item" \
            --data-urlencode "count=$count")
        if printf '%s' "$body" | grep -q "$account: given"; then
            given=$((given + 1))
        else
            refused=$((refused + 1))
            echo "$account: $item not given"
        fi
    done < "$list"
    echo "$account: $given items given, $refused refused"
done
