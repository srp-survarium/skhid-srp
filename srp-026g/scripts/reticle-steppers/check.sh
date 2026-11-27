#!/usr/bin/env bash
# check.sh <lobby_menu tree> [work dir]: the reticle's steppers of the weapon's Attachments window
# (docs/notes/reticle-choice.md, "The movie"), in Ruffle (CPU only: lavapipe on its own virtual X
# display, never the GPU). As scripts/weapon-kit/check.sh: a test copy of the tree is built with
# weapon-kit's HarnessExternal.as (it records the exe's calls) in place of ExternalInterface, over the
# original movie.json names as tools/checks/movie.sh keeps it (run that first). ReticleHarness.as
# makes the lobby movie's own upgrade window and inventory, fed as the exe feeds them (a collimator
# scope item's srp_reticle member), and checks:
# - the panel (title and the Pattern, Size, Colour, Brightness steppers) shown only while the scope
#   slot holds a scope with the member: not for a scope without it, an empty slot, the Appearance
#   page, armour; a scope put in or taken out shows or hides it at once;
# - each step's call: srp_set_reticle(scope item, pattern, size, colour, brightness), all four values,
#   the one stepped one on or back; pattern, size and colour wrapping around their lists, brightness
#   stopping at 0 and 10 (its arrow disabled there, a press sending nothing); a list of one: nothing;
# - the panel showing what the account brings (the inventory again), not what a step asked for;
# - members out of the contract's bounds giving no panel (ReticleChoice.parse);
# - its place: under the scope's label, over the next slot, inside the window and above Confirm, on a
#   weapon with the scope alone and on the HK416 with all four slots and the second sight's toggle;
# - every language's title, names and each pattern's and colour's text fitting their fields.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log,
# reticle.png: the HK416's window with the panel).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
tree=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The original, as tools/checks/movie.sh keeps it by its sha256.
sha=$(sed -n 's/.*"movie_sha256": *"\([^"]*\)".*/\1/p' "$tree/movie.json")
original=${SKHID_CHECKS_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/skhid-checks}/originals/$sha.swf
[[ -f $original ]] || { echo "check.sh: $original: missing (tools/checks/movie.sh lobby_menu keeps it)" >&2; exit 1; }
# The test copy: the recorder for ExternalInterface. It sits where a tree does (srp-026g/flash/<movie>):
# movie.json's data are the repository's tables, three levels up (the scripts, linked).
copy=$work/repo/srp-026g/flash/lobby_menu
rm -rf "$work/repo" "$work/lobby_menu.built.swf"
mkdir -p "$(dirname "$copy")"
cp -r "$tree" "$copy"
ln -s "$(readlink -f "$tree/../../scripts")" "$work/repo/srp-026g/scripts"
cp "$here/../weapon-kit/HarnessExternal.as" "$copy/src/HarnessExternal.as"
grep -rlZ "ExternalInterface" "$copy/src" | xargs -0 perl -pi -e 's/^\s*import flash\.external\.ExternalInterface;\n//; s/\bExternalInterface\./HarnessExternal./g'
"${PYTHON:-python3}" "$here/../../../tools/actionscript/flash_tree.py" --tools "$tools" build "$copy" "$original" \
    "$work/lobby_menu.built.swf" >"$work/lobby_menu.build.log" 2>&1 || { cat "$work/lobby_menu.build.log"; exit 1; }
lobby=$work/lobby_menu.built.swf

# The window's texts in each language as the exe gives them (set_localization_data), as
# weapon-kit/check.sh has them, and every st_srp_reticle_ key under its own name.
"${PYTHON:-python3}" - "$here/../../client-patches" "$work" <<'EOF'
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, expand_keys, read_translations
KEYS = {'btn_upgrade_label': 'st_btn_upgrade_label', 'scope_window_title': 'st_srp_attachments',
        'decal_slot': 'st_decal_slot', 'camo_slot': 'st_camo_slot',
        'scope_slot': 'st_picatinny_scope_medium_name', 'default_scope': 'st_default_scope_name',
        'scope_screen_hint': 'st_srp_scope_screen_hint', 'modification_screen_hint': 'st_modification_screen_hint',
        'modification_screen_edit_decal_hint': 'st_modification_screen_edit_decal_hint', 'confirm': 'st_confirm',
        'st_srp_scope_slot': 'st_srp_scope_slot', 'st_srp_muzzle_slot': 'st_srp_muzzle_slot',
        'st_srp_grip_slot': 'st_srp_grip_slot', 'st_srp_second_sight': 'st_srp_second_sight',
        'st_srp_side_left': 'st_srp_side_left', 'st_srp_side_right': 'st_srp_side_right',
        'st_srp_second_sight_hint': 'st_srp_second_sight_hint'}
languages = [name for name, *_ in LANGUAGES]
data = {}
for language in languages:
    strings = read_translations(CATALOG / f'{language}.tsv')
    keys = dict(KEYS, **{key: key for key in strings if key.startswith('st_srp_reticle_')})
    data[language] = {ui: expand_keys(strings[key], strings) for ui, key in keys.items()}
(Path(sys.argv[2]) / 'ReticleText.as').write_text(
    'package\n{\n    // Made by srp-026g/scripts/reticle-steppers/check.sh: do not edit.\n'
    '    public class ReticleText\n    {\n'
    f'        public static const LANGUAGES:Array = {json.dumps(languages)};\n'
    f'        public static const DATA:Object = {json.dumps(data, ensure_ascii=False, indent=1)};\n'
    '    }\n}\n', encoding='utf-8')
EOF

harness_compile "$here/ReticleHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the window's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720 imagemagick
harness_run ReticleHarness.log -p low --width 1280 --height 720 ReticleHarness.swf
if wait_for ReticleHarness.log 'RETICLE shot'; then
    sleep 1
    DISPLAY=$display import -window root "$work/reticle.png" 2>/dev/null || true
fi
wait_for ReticleHarness.log 'RETICLE RESULT' || true
harness_stop
harness_report 'RETICLE .*' ReticleHarness.log
grep -q 'RETICLE RESULT pass' "$work/ReticleHarness.log"
