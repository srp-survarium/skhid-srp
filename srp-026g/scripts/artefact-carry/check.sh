#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: artefact take prompt in isolated CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
# The prompts' texts in each language as the exe hands them (exe patch 212): Use bound to its default
# key E, rebound to F, and unbound; the dictionaries' own texts with the key the exe puts in.
"${PYTHON:-python3}" - "$here/../../client-patches" "$work" <<'EOF2'
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, expand_keys, read_translations
KEYS = ('st_artefact_take_teammate', 'st_weapon_pick_up', 'st_pickup_item', 'st_put_item')
BINDINGS = {'default': {'USE': 'kE'}, 'rebound': {'USE': 'kF'}, 'unbound': {'USE': None}}
data = {}
for language, *_ in LANGUAGES:
    strings = read_translations(CATALOG / f'{language}.tsv')
    data[language] = {case: {key: expand_keys(strings[key], strings, keys) for key in KEYS}
                      for case, keys in BINDINGS.items()}
    data[language]['keys'] = {'default': strings['kE'], 'rebound': strings['kF'],
                              'unbound': strings['st_srp_key_unbound']}
(Path(sys.argv[2]) / 'CarryText.as').write_text(
    'package\n{\n    // Made by srp-026g/scripts/artefact-carry/check.sh: do not edit.\n'
    '    public class CarryText\n    {\n'
    f'        public static const DATA:Object = {json.dumps(data, ensure_ascii=False, indent=1)};\n'
    '    }\n}\n', encoding='utf-8')
EOF2
harness_compile "$here/CarryHarness.as" 1280,720 -source-path+="$work"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
harness_display 1280x720
harness_run CarryHarness.log -p low --width 1280 --height 720 -Pmovie=hud.swf CarryHarness.swf
wait_for CarryHarness.log 'CARRY RESULT' || true
harness_stop
harness_report 'CARRY .*' CarryHarness.log
grep -q 'CARRY RESULT pass' "$work/CarryHarness.log"
