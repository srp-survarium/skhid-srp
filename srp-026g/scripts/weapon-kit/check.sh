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
# - a double click on an inventory item (InventoryList.onItemDoubleClick, UpgradeWindow.attach): a
#   camouflage into its slot, a decal into the first free decal slot (over the first when both are
#   full), a scope into its slot, on the page that shows the slot; armour's too; window closed: nothing;
# - the window's texts in every language of client-patches/localization fitting their fields.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

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

harness_compile "$here/WeaponKitHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the window's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
harness_run WeaponKitHarness.log -p low --width 1280 --height 720 WeaponKitHarness.swf
wait_for WeaponKitHarness.log 'WEAPONKIT RESULT' || true
harness_stop
harness_report 'WEAPONKIT .*' WeaponKitHarness.log
grep -q 'WEAPONKIT RESULT pass' "$work/WeaponKitHarness.log"
