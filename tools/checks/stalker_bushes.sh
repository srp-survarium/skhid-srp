#!/usr/bin/env bash
# stalker_bushes.sh: the STALKER maps' bushes made again from the pack the script names
# (make_pack.py's SOURCE, its inputs pinned there) are what is committed: the pack of the script's
# PACK_JSON (the script refuses another) and the edit list, which the script first places the
# models with in the source pack's own projects. The manifest's rows are stalker-bushes-manifest's.
set -euo pipefail
. release/lib.sh
load_env SKHID_PACKS_DIR
python3 -I srp-026g/scripts/stalker-bushes/make_pack.py --packs "$SKHID_PACKS_DIR" \
    --out "$CHECK_WORK/pack" --edits "$CHECK_WORK/stalker-bushes-1.json"
cmp "$CHECK_WORK/stalker-bushes-1.json" srp-026g/scripts/content-edit/stalker-bushes-1.json
