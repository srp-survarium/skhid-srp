#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the weapon's upgrade window with the weapon kit
# (docs/notes/weapon-kit.md, docs/notes/weapon-camo.md), in Ruffle (CPU only: lavapipe on its own
# virtual X display, never the GPU). WeaponKitHarness.as makes the lobby movie's own upgrade window
# (SrvModifiersControlReskin), inventory list and profile and checks:
# - a weapon's two decal slots on its Appearance page, apart and inside the window, and armour's
#   window as the movie has it;
# - the kit's records (UpgradeWindow.kitUpgrades): the decals, the kit's own item in its slot, the
#   camouflage it names as the inventory's lowest item of that dict id; a kit whose own is the
#   camouflage; armour's records as they come;
# - a drop on a slot (UpgradeSlot.handleDropEvent, onItemDropped) shown at once: a decal into the
#   second decal slot, a camouflage beside the kit's scope;
# - the window's texts in every language of client-patches/localization fitting their fields.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 2 ]]; then
    work=$(readlink -f "$2")
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi

# The window's texts in each language as the exe gives them (set_localization_data): the movie's
# key and its dictionary key, as localization/flash_movies_localizations.options maps them for the
# lobby (scope_window_title: weapon-attachments-1.json).
"${PYTHON:-python3}" - "$here/../../client-patches" "$work" <<'EOF'
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
KEYS = {'btn_upgrade_label': 'st_btn_upgrade_label', 'scope_window_title': 'st_srp_attachments',
        'decal_slot': 'st_decal_slot', 'camo_slot': 'st_camo_slot',
        'scope_slot': 'st_picatinny_scope_medium_name', 'default_scope': 'st_default_scope_name',
        'scope_screen_hint': 'st_srp_scope_screen_hint', 'modification_screen_hint': 'st_modification_screen_hint',
        'modification_screen_edit_decal_hint': 'st_modification_screen_edit_decal_hint', 'confirm': 'st_confirm'}
languages = [name for name, *_ in LANGUAGES]
data = {}
for language in languages:
    strings = read_translations(CATALOG / f'{language}.tsv')
    data[language] = {ui: strings[key] for ui, key in KEYS.items()}
(Path(sys.argv[2]) / 'WeaponKitText.as').write_text(
    'package\n{\n    // Made by srp-026g/scripts/weapon-kit/check.sh: do not edit.\n'
    '    public class WeaponKitText\n    {\n'
    f'        public static const LANGUAGES:Array = {json.dumps(languages)};\n'
    f'        public static const DATA:Object = {json.dumps(data, ensure_ascii=False, indent=1)};\n'
    '    }\n}\n', encoding='utf-8')
EOF

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" -source-path+="$work" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=1280,720 \
    -output="$work/WeaponKitHarness.swf" "$here/WeaponKitHarness.as" >"$work/WeaponKitHarness.mxmlc.log" 2>&1 \
    || { cat "$work/WeaponKitHarness.mxmlc.log"; exit 1; }
# The native Scaleform placements Ruffle refuses, so the window's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
# Xvfb chooses an unused display without touching the owner's desktop.
"$xvfb" -displayfd 3 -screen 0 1280x720x24 -nolisten tcp 3>"$work/display" >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 2 ]] || rm -rf "$work"' EXIT
for _ in $(seq 100); do [[ -s $work/display ]] && break; sleep 0.1; done
[[ -s $work/display ]] || { cat "$work/xvfb.log"; exit 1; }

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=:$(cat display) VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 180 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 1280 --height 720 WeaponKitHarness.swf
) >"$work/WeaponKitHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1800); do grep -q 'WEAPONKIT RESULT' "$work/WeaponKitHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/WeaponKitHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/WeaponKitHarness.log" >&2
fi
grep -ho 'WEAPONKIT .*' "$work/WeaponKitHarness.log" || true
grep -q 'WEAPONKIT RESULT pass' "$work/WeaponKitHarness.log"
