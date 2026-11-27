#!/usr/bin/env bash
# check.sh <lobby_menu tree> [work dir]: the weapon's upgrade window with the weapon kit
# (docs/notes/weapon-kit.md, docs/notes/weapon-camo.md), in Ruffle (CPU only: lavapipe on its own
# virtual X display, never the GPU). The window calls the exe (ExternalInterface; Ruffle has none): a
# test copy of the tree is built (flash_tree.py) with HarnessExternal.as, which records the calls, in
# its place, over the original movie.json names as tools/checks/movie.sh keeps it (run that first).
# WeaponKitHarness.as makes the lobby movie's own upgrade window (SrvModifiersControlReskin),
# inventory list and profile and checks:
# - a weapon's two decal slots on its Appearance page, apart and inside the window, and armour's
#   window as the movie has it;
# - the kit's records (UpgradeWindow.kitUpgrades): the decals, the kit's own item in its slot, the
#   camouflage it names as the inventory's lowest item of that dict id; a kit whose own is the
#   camouflage; armour's records as they come;
# - a drop on a slot (UpgradeSlot.handleDropEvent, onItemDropped) shown at once: a decal into the
#   second decal slot, a camouflage beside the kit's scope;
# - a double click on an inventory item (InventoryList.onItemDoubleClick, UpgradeWindow.attach): a
#   camouflage into its slot, a decal into the first free decal slot (over the first when both are
#   full), a scope into its slot, on the page that shows the slot; armour's too; window closed: nothing;
# - the Attachments page of a weapon that takes muzzle devices and grips: the scope's, the device's
#   and the grip's slots one under the other, the kit's device and grip in them, a double click on
#   each into its own slot, a grip lit up only on the grip's slot, a refused pair's slot emptied at
#   the account's update; a weapon that takes grips alone: its grip slot at the top;
# - the second sight (SecondSights; the HK416's rows): a wide kit's optic in its slot under the
#   scope's (the scope's own optic's dict id: the other item of it), its side's Left/Right toggle
#   under its square, the four slots and the toggle inside the window; a press of a side sends
#   srp_set_upgrade_value(host, optic, side) and shows it at once, the account's update the side the
#   server holds; a kit whose own is the second sight; the slot chosen: its optics bright, the
#   rest dim, a double click into it; a weapon without rows: no such slot; second.png, the window
#   with all four;
# - real clicks (xdotool) on the Appearance page as its right-click's entry opens it: the list shows a
#   slot's own kind, a camouflage slot's the camouflages, empty or filled (its middle is its change
#   button), after a decal slot's click and a decal edit too; a decal's change button starts its edit
#   (start_decal_edit), a camouflage's never;
# - the window's texts in every language of client-patches/localization fitting their fields.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
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
cp "$here/HarnessExternal.as" "$copy/src/HarnessExternal.as"
grep -rlZ "ExternalInterface" "$copy/src" | xargs -0 perl -pi -e 's/^\s*import flash\.external\.ExternalInterface;\n//; s/\bExternalInterface\./HarnessExternal./g'
"${PYTHON:-python3}" "$here/../../../tools/actionscript/flash_tree.py" --tools "$tools" build "$copy" "$original" \
    "$work/lobby_menu.built.swf" >"$work/lobby_menu.build.log" 2>&1 || { cat "$work/lobby_menu.build.log"; exit 1; }
lobby=$work/lobby_menu.built.swf

# The window's texts in each language as the exe gives them (set_localization_data): the movie's
# key and its dictionary key, as localization/flash_movies_localizations.options maps them for the
# lobby (scope_window_title: weapon-attachments-1.json).
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
        # srp's own keys come to the lobby under their own names (localization_026g.py)
        'st_srp_scope_slot': 'st_srp_scope_slot', 'st_srp_muzzle_slot': 'st_srp_muzzle_slot',
        'st_srp_grip_slot': 'st_srp_grip_slot', 'st_srp_second_sight': 'st_srp_second_sight',
        'st_srp_side_left': 'st_srp_side_left', 'st_srp_side_right': 'st_srp_side_right',
        'st_srp_second_sight_hint': 'st_srp_second_sight_hint'}
languages = [name for name, *_ in LANGUAGES]
data = {}
for language in languages:
    strings = read_translations(CATALOG / f'{language}.tsv')
    # a key a text names ({SCOPE_NEXT}) is the action's default key, as the exe makes it
    data[language] = {ui: expand_keys(strings[key], strings) for ui, key in KEYS.items()}
(Path(sys.argv[2]) / 'WeaponKitText.as').write_text(
    'package\n{\n    // Made by srp-026g/scripts/weapon-kit/check.sh: do not edit.\n'
    '    public class WeaponKitText\n    {\n'
    f'        public static const LANGUAGES:Array = {json.dumps(languages)};\n'
    f'        public static const DATA:Object = {json.dumps(data, ensure_ascii=False, indent=1)};\n'
    '    }\n}\n', encoding='utf-8')
EOF

harness_compile "$here/WeaponKitHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the window's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720 imagemagick xdotool
harness_run WeaponKitHarness.log -p low --width 1280 --height 720 WeaponKitHarness.swf
if wait_for WeaponKitHarness.log 'WEAPONKIT shot second'; then
    sleep 1
    DISPLAY=$display import -window root "$work/second.png" 2>/dev/null || true
fi
# The real clicks (xdotool on the Ruffle window, the one window on this display): one where the window
# puts the stage (a menu bar above it), then each "click <n> at <x>,<y>" (stage) the harness asks for.
log=$work/WeaponKitHarness.log
if wait_for WeaponKitHarness.log 'WEAPONKIT calibrate'; then
    window=$(DISPLAY=$display timeout 20 xdotool search --sync --onlyvisible --name "." | head -1 || true)
    DISPLAY=$display xdotool mousemove --window "$window" 1200 700 click 1 || true
    wait_for WeaponKitHarness.log 'WEAPONKIT mouse down at' 600 || true
    read -r sx sy < <(sed -n 's/.*WEAPONKIT mouse down at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log" | head -1) \
        || echo "the harness saw no click: see $log" >&2
    n=0
    while [[ -n ${sy:-} ]] && wait_for WeaponKitHarness.log "WEAPONKIT click $n at\|WEAPONKIT RESULT" 600 \
        && grep -q "WEAPONKIT click $n at" "$log"; do
        read -r tx ty < <(sed -n "s/.*WEAPONKIT click $n at \([0-9]*\),\([0-9]*\).*/\1 \2/p" "$log")
        DISPLAY=$display xdotool mousemove --window "$window" $((tx + 1200 - sx)) $((ty + 700 - sy)) click 1 || true
        n=$((n + 1))
    done
fi
wait_for WeaponKitHarness.log 'WEAPONKIT RESULT' || true
harness_stop
harness_report 'WEAPONKIT .*' WeaponKitHarness.log
grep -q 'WEAPONKIT RESULT pass' "$work/WeaponKitHarness.log"
