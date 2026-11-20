#!/usr/bin/env bash
# check.sh <rebuilt main_menu.swf> [work dir]: the Settings' Language choice
# (exe patch 79) in Ruffle, on a private CPU-only virtual display: the list
# from the exe's answer, the saved pick, the note, the other tabs and the
# glyphs of every registered language's own name and texts. No game, no server.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
menu=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The registry's languages and each dictionary's two texts, as JSON.
read -r languages texts < <("${PYTHON:-python3}" - "$here/../../client-patches" <<'PY'
import json, sys
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
keys = ('st_srp_language_option', 'st_srp_language_restart')
texts = [[read_translations(CATALOG / f'{name}.tsv')[k] for k in keys] for name, *_ in LANGUAGES]
print(json.dumps([[name, native] for name, native, *_ in LANGUAGES], ensure_ascii=False, separators=(',', ':')),
      json.dumps(texts, ensure_ascii=False, separators=(',', ':')))
PY
)

harness_compile "$here/LanguageHarness.as" 1280,720
cp "$menu" "$work/main_menu.swf"
harness_display 1280x720
harness_run LanguageHarness.log -p low --width 1280 --height 720 "-Planguages=$languages" "-Ptexts=$texts" LanguageHarness.swf
wait_for LanguageHarness.log 'LANGUAGE RESULT' || true
harness_stop
harness_report 'LANGUAGE .*' LanguageHarness.log
grep -q 'LANGUAGE RESULT pass' "$work/LanguageHarness.log"
