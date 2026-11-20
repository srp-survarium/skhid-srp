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
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# Each language's texts as the exe gives them: the search's own keys (st_srp_*, under their own
# names), the seller tabs' names, and the card's names of the properties the harness's items have
# (gameplay/static_game_parameters.options ui_properties: prop_id, prop_name).
"${PYTHON:-python3}" - "$here/../../client-patches" "$work" <<'EOF'
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
SEARCH = ['st_srp_inventory_search', 'st_srp_inventory_search_examples'] + [f'st_srp_inventory_search_tip_{part}{suffix}'
                                         for part in ('bonus', 'level', 'faction', 'kind')
                                         for suffix in ('', '_example')]
FACTIONS = {0: 'st_non_faction_items', 1: 'st_scavengers_faction', 2: 'st_black_market_faction', 3: 'st_renaissance_faction',
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

harness_compile "$here/ItemSearchHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the list's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
harness_run ItemSearchHarness.log -p low --width 1280 --height 720 ItemSearchHarness.swf
wait_for ItemSearchHarness.log 'ITEMSEARCH RESULT' || true
harness_stop
harness_report 'ITEMSEARCH .*' ItemSearchHarness.log
grep -q 'ITEMSEARCH RESULT pass' "$work/ItemSearchHarness.log"
