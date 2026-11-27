#!/usr/bin/env bash
# v021d-vertex-blend@4: @3 with 0.69d's cut of an alpha-tested blend by its second layer
set -euo pipefail
. release/lib.sh
load_env SKHID_PACKS_DIR
python3 srp-026g/scripts/v069d-blend-cut/check_blend_cut.py --packs "$SKHID_PACKS_DIR"
