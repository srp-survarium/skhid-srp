#!/usr/bin/env python3
"""hunt_text.py <out dir>: <out dir>/HuntText.as, the texts the hud movie shows in Artifact Hunt.

HuntText.DATA[language] is what the exe gives the hud movie (set_localization_data) for the keys
KEYS: each one's text in that language's dictionary (client-patches/localization/<language>.tsv);
one the dictionaries do not have yet is taken from the mode's pack's list
(scripts/artefact-hunt/texts.tsv), else PENDING's English, and listed in HuntText.PENDING. The exe
replaces {USE} with the use key's name before the movie gets a text (E here).
"""
import csv
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / 'client-patches'))
from localization_026g import CATALOG, LANGUAGES, read_translations  # noqa: E402

# The movie's own keys (st_srp_hud_: the hud is given them under their own names), the exe's and the
# pack's keys it shows (the hud's mapping must give it these: the offer prompt, the round's texts).
KEYS = ['st_srp_hud_artefact_hunt_carrying', 'st_srp_hud_artefact_hunt_sudden_death',
        'st_round_restart_in', 'st_final_countdown', 'st_pickup_item', 'st_put_item',
        'st_on_spawn_victory_artefact', 'st_artefact_hunt_you_picked', 'st_artefact_hunt_team_delivered',
        'st_artefact_hunt_enemy_delivered', 'st_on_destroyed_victory_artefact']
# English of the movie's keys until the dictionaries have them.
PENDING = {'st_srp_hud_artefact_hunt_carrying': 'You carry the artifact',
           'st_srp_hud_artefact_hunt_sudden_death': 'Sudden death'}


def main():
    out = Path(sys.argv[1])
    with open(HERE.parent / 'artefact-hunt/texts.tsv', encoding='utf-8') as f:
        rows = [line for line in f if not line.startswith('#')]
    pack = {row['key']: row for row in csv.DictReader(rows, delimiter='\t')}
    data, pending = {}, set()
    languages = [name for name, *_ in LANGUAGES]
    for language in languages:
        strings = read_translations(CATALOG / f'{language}.tsv')
        texts = {}
        for key in KEYS:
            if key in strings:
                text = strings[key]
            elif key in pack and pack[key].get(language):
                text = pack[key][language]
            else:
                text = PENDING[key]
                pending.add(key)
            texts[key] = text.replace('{USE}', 'E')
        data[language] = texts
    out.mkdir(parents=True, exist_ok=True)
    (out / 'HuntText.as').write_text(
        'package\n{\n'
        '    // Made by srp-026g/scripts/artefact-hunt-hud/hunt_text.py: do not edit.\n'
        '    public class HuntText\n    {\n'
        f'        public static const LANGUAGES:Array = {json.dumps(languages)};\n'
        f'        public static const PENDING:Array = {json.dumps(sorted(pending))};\n'
        f'        public static const DATA:Object = {json.dumps(data, ensure_ascii=False, indent=1)};\n'
        '    }\n}\n', encoding='utf-8')


if __name__ == '__main__':
    main()
