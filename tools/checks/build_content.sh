#!/usr/bin/env bash
# build_content.sh: release/build-content of this checkout for the revision above every patch
# release/patches.tsv lists, into $CHECK_WORK (tools/check removes it after a pass).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
last=$(sed -n 's/^resources\.db\.patch_\([0-9]*\)[[:space:]].*/\1/p' "$root/release/patches.tsv" | sort -n | tail -1)
"$root/release/build-content" "$((last + 1))" --out "${CHECK_WORK:?run by tools/check}/content"
