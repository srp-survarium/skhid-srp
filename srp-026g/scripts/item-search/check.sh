#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the item lists' search (docs/notes/item-search.md) in Ruffle
# (CPU only: lavapipe on its own virtual X display, never the GPU). ItemSearchHarness.as makes the
# lobby movie's own inventory list, fed items as the exe feeds them, types into its box and checks:
# - the "+" words: levels (any), factions by the seller tabs' names (any), bonuses by the card's
#   property names or the English words (each), all of them with the name's words; a defect is no
#   bonus; a "+" word naming nothing lists nothing; "+" alone is nothing typed;
# - every language: the box's hint on one line inside the box, each of its examples and the tip's
#   examples finding what they say, the bonuses by the language's own names and by English;
# - the tip: on the box while it is empty, its lines the dictionary's, gone once something is
#   typed, back when the box is emptied.
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

# Each language's texts as the exe gives them: the search's own keys (st_srp_*, under their own
# names), the seller tabs' names, and the card's names of the properties the harness's items have
# (gameplay/static_game_parameters.options ui_properties: prop_id, prop_name).
"${PYTHON:-python3}" - "$here/../../client-patches" "$work" <<'EOF'
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
SEARCH = ['st_srp_inventory_search', 'st_srp_inventory_search_examples',
          'st_srp_inventory_search_tip_bonus_list', 'st_srp_inventory_search_tip_kind_list'] + [f'st_srp_inventory_search_tip_{part}{suffix}'
                                         for part in ('bonus', 'level', 'faction', 'kind')
                                         for suffix in ('', '_example')]
FACTIONS = {1: 'st_scavengers_faction', 2: 'st_black_market_faction', 3: 'st_renaissance_faction',
            4: 'st_border_faction'}
PROPS = {3: 'st_inv_arp', 6: 'st_inv_armor', 9: 'st_inv_artefact_slots', 10: 'st_inv_device_slots',
         15: 'st_inv_item_weight', 105: 'st_movement_speed_correction_perc_name',
         20: 'st_inv_additional_ammo_bags', 102: 'st_aiming_speed_correction_perc_name',
         115: 'st_multiplicative_recoil_modifier', 117: 'st_additive_quick_slot_size_modifier'}
languages = [name for name, *_ in LANGUAGES]
data = {}
for language in languages:
    strings = read_translations(CATALOG / f'{language}.tsv')
    data[language] = {
        'ui': {**{key: strings[key] for key in SEARCH}, 'non_faction_items': strings['st_non_faction_items']},
        'factions': {str(id): strings[key] for id, key in FACTIONS.items()},
        'props': {str(id): strings[key] for id, key in PROPS.items()},
    }
(Path(sys.argv[2]) / 'ItemSearchText.as').write_text(
    'package\n{\n    // Made by srp-026g/scripts/item-search/check.sh: do not edit.\n'
    '    public class ItemSearchText\n    {\n'
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
    -output="$work/ItemSearchHarness.swf" "$here/ItemSearchHarness.as" >"$work/ItemSearchHarness.mxmlc.log" 2>&1 \
    || { cat "$work/ItemSearchHarness.mxmlc.log"; exit 1; }
# The native Scaleform placements Ruffle refuses, so the list's named children exist.
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
        --width 1280 --height 720 ItemSearchHarness.swf
) >"$work/ItemSearchHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1800); do grep -q 'ITEMSEARCH RESULT' "$work/ItemSearchHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/ItemSearchHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/ItemSearchHarness.log" >&2
fi
grep -ho 'ITEMSEARCH .*' "$work/ItemSearchHarness.log" || true
grep -q 'ITEMSEARCH RESULT pass' "$work/ItemSearchHarness.log"
