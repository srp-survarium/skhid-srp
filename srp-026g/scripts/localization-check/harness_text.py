#!/usr/bin/env python3
"""harness_text.py <out dir> [language ...]: <out dir>/HarnessText.as for the Ruffle harnesses.

HarnessText.DATA[language] is what the exe gives the lobby and login movies through
set_localization_data in that language for srp's own texts: each st_srp_ key under its own name
(localization_026g.flash_rows, the rows the content compile adds to their UI mapping), its text
from the language's TSV. HarnessText.LANGUAGES lists the languages, the registry's order
(default: every registered language).
"""
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'client-patches'))
from localization_026g import CATALOG, LANGUAGES, flash_rows, read_translations  # noqa: E402


def main():
    out = Path(sys.argv[1])
    languages = sys.argv[2:] or [name for name, *_ in LANGUAGES]
    data = {}
    for language in languages:
        strings = read_translations(CATALOG / f'{language}.tsv')
        data[language] = {ui: strings[key] for ui, key in flash_rows(strings)}
    out.mkdir(parents=True, exist_ok=True)
    (out / 'HarnessText.as').write_text(
        'package\n{\n'
        '    // Made by srp-026g/scripts/localization-check/harness_text.py: do not edit.\n'
        '    public class HarnessText\n    {\n'
        f'        public static const LANGUAGES:Array = {json.dumps(languages)};\n'
        f'        public static const DATA:Object = {json.dumps(data, ensure_ascii=False, indent=1)};\n'
        '    }\n}\n', encoding='utf-8')


if __name__ == '__main__':
    main()
