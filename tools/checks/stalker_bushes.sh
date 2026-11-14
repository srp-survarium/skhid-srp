#!/usr/bin/env bash
# stalker_bushes.sh: the STALKER maps' bushes made again from stalker-maps@6 are what is committed:
# the pack of the manifest's pin and the edit list that places it.
set -euo pipefail
. release/lib.sh
load_env SKHID_PACKS_DIR
python3 -I srp-026g/scripts/stalker-bushes/make_pack.py --pack "$SKHID_PACKS_DIR/stalker-maps-6" \
    --out "$CHECK_WORK/stalker-bushes-1" --edits "$CHECK_WORK/stalker-bushes-1.json"
cmp "$CHECK_WORK/stalker-bushes-1.json" srp-026g/scripts/content-edit/stalker-bushes-1.json
pin=$(awk -F'\t' '$1 == "pack" && $2 == "stalker-bushes@1" { print $3 }' release/content.tsv)
made=$(sha256_of "$CHECK_WORK/stalker-bushes-1/pack.json")
[[ -n $pin && $made == "$pin" ]] || die "pack.json has sha256 $made, release/content.tsv pins ${pin:-nothing}"
