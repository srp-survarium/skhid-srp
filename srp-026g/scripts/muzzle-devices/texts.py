#!/usr/bin/env python3
"""texts.py [--check]: the muzzle devices' names and descriptions in the twelve dictionaries.

Writes, into every ../../client-patches/localization/<language>.tsv in key order (the tables' own
rule, check_localization_026g.py), for each device of devices.tsv:
- st_muzzle_<module>_name: the model's name (names.tsv), the kind's word and what follows the comma:
  "Zenit DTK-3 muzzle brake, M24x1.5". Short, to fit the inventory's tile;
- st_muzzle_<module>_description: what it changes, each stat in the dictionary's own word for it
  ("Weapon recoil -4 %. Aim time +3 %."); a device that changes no stat: none (a flash hider's card
  has its bonus line, st_srp_no_muzzle_flash: ../weapon-modules/ids.py, AttachmentStats);
- st_srp_no_muzzle_flash: that line, short as the stock bonus lines.
--check: writes nothing and fails when a dictionary is not what this gives.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import devices as dev  # noqa: E402

TABLES = HERE.parent.parent / 'client-patches/localization'
THREADS = {'m12x1l_muzzle': 'M12x1L', 'm14x1l_muzzle': 'M14x1L', 'm16x1l_muzzle': 'M16x1L',
           'm24x15_muzzle': 'M24x1.5', 'm22x075_muzzle': 'M22x0.75', 'm30x1_muzzle': 'M30x1',
           'i05x28_muzzle': '1/2"x28', 'i06x24_muzzle': '5/8"x24'}
# a family's word, and whether it goes before the model's name
KIND = {
    'brake': {
        'english': ('muzzle brake', False), 'german': ('Mündungsbremse', False),
        'polish': ('Hamulec wylotowy', True), 'russian': ('Дульный тормоз', True),
        'ukrainian': ('Дульне гальмо', True), 'belarusian': ('Дульны тормаз', True),
        'czech': ('Úsťová brzda', True), 'finnish': ('suujarru', False),
        'hungarian': ('csőszájfék', False), 'serbian': ('Usna kočnica', True),
        'croatian': ('Plinska kočnica', True), 'portuguese': ('Freio de boca', True)},
    'flash_hider': {
        'english': ('flash hider', False), 'german': ('Mündungsfeuerdämpfer', False),
        'polish': ('Tłumik płomieni', True), 'russian': ('Пламегаситель', True),
        'ukrainian': ("Полум'ягасник", True), 'belarusian': ('Полымягасільнік', True),
        'czech': ('Tlumič plamene', True), 'finnish': ('liekinvaimennin', False),
        'hungarian': ('lángrejtő', False), 'serbian': ('Skrivač plamena', True),
        'croatian': ('Skrivač plamena', True), 'portuguese': ('Quebra-chamas', True)},
    'choke': {
        'english': ('choke', False), 'german': ('Choke', False), 'polish': ('Czok', True),
        'russian': ('Чок', True), 'ukrainian': ('Чок', True), 'belarusian': ('Чок', True),
        'czech': ('Zahrdlení', True), 'finnish': ('supistaja', False), 'hungarian': ('szűkítő', False),
        'serbian': ('Čok', True), 'croatian': ('Čok', True), 'portuguese': ('Choke', True)},
    'duckbill': {
        'english': ('duckbill', False), 'german': ('Duckbill', False), 'polish': ('Kaczy dziób', True),
        'russian': ('Утиный клюв', True), 'ukrainian': ('Качиний дзьоб', True),
        'belarusian': ('Качыная дзюба', True), 'czech': ('Kachní zobák', True),
        'finnish': ('ankannokka', False), 'hungarian': ('kacsacsőr', False),
        'serbian': ('Pačji kljun', True), 'croatian': ('Pačji kljun', True),
        'portuguese': ('Bico de pato', True)},
}
# a stat's word: the dictionary's own key for it, in the order a description says them
STATS = (('recoil', 'st_multiplicative_recoil_modifier'), ('dispersion/base_dispersion', 'st_inv_dispersion'),
         ('parameters/aim_time', 'st_srp_medicine_aim_time'))
# a flash hider whose shot effect is the small puff
NO_FLASH = {'english': 'No muzzle flash', 'german': 'Kein Mündungsfeuer', 'polish': 'Brak błysku',
            'russian': 'Без вспышки', 'ukrainian': 'Без спалаху', 'belarusian': 'Без успышкі',
            'czech': 'Bez záblesku', 'finnish': 'Ei suuliekkiä', 'hungarian': 'Nincs torkolattűz',
            'serbian': 'Bez bljeska', 'croatian': 'Bez bljeska', 'portuguese': 'Sem clarão'}


def percent(value):
    return f'{"+" if value > 0 else "-"}{round(abs(value) * 100, 1):g} %'


def read(path):
    lines = path.read_text(encoding='utf-8').split('\n')
    if lines[0] != 'key\ttext' or lines[-1] != '':
        sys.exit(f'{path}: not a dictionary table')
    return [tuple(line.split('\t', 1)) for line in lines[1:-1]]


def texts(found, names, language, words):
    out = {'st_srp_no_muzzle_flash': NO_FLASH[language]}
    for device in found:
        name, tag = names[device['module']]
        word, first = KIND[device['family']][language]
        stem = device['item'].removeprefix('srp_')
        out[f'st_{stem}_name'] = (f'{word} {name}' if first else f'{name} {word}') + f', {tag}'
        stats = dict(device['stats'])
        if stats:
            said = [f'{words[key][:1].upper()}{words[key][1:]} {percent(stats.pop(stat))}.'
                    for stat, key in STATS if stat in stats]
            if stats:
                sys.exit(f'{device["module"]}: no word for {sorted(stats)}')
        else:
            said = []
        out[f'st_{stem}_description'] = ' '.join(said)
    return out


def main():
    found = dev.read_table()
    names = {module: (name, tag) for module, name, tag in dev.rows('names.tsv')}
    seen = set()
    for device in found:
        if device['module'] not in names:
            sys.exit(f'names.tsv: no {device["module"]}')
        name, tag = names[device['module']]
        thread = THREADS[device['slots'][0]]
        if tag != thread and not tag.startswith(thread + ' '):
            sys.exit(f'names.tsv: {device["module"]}: {tag!r} does not start with its thread {thread}')
        if (device['family'], name, tag) in seen:
            sys.exit(f'names.tsv: two {device["family"]} devices named {name}, {tag}')
        seen.add((device['family'], name, tag))
    if set(names) != {d['module'] for d in found}:
        sys.exit(f'names.tsv: no such device: {sorted(set(names) - {d["module"] for d in found})}')
    tables = sorted(TABLES.glob('*.tsv'))
    if {path.stem for path in tables} != set(NO_FLASH):
        sys.exit(f'the dictionaries are {[p.stem for p in tables]}, the texts here {sorted(NO_FLASH)}')
    stale = []
    for path in tables:
        table = read(path)
        words = {key: text.strip() for key, text in table if key in {k for _, k in STATS}}
        new = texts(found, names, path.stem, words)
        if table != sorted(table):
            sys.exit(f'{path}: its keys are not in order')
        merged = sorted({**dict(table), **new}.items())
        if merged != table:
            stale.append(path.name)
            if '--check' not in sys.argv:
                path.write_text('key\ttext\n' + ''.join(f'{key}\t{text}\n' for key, text in merged), encoding='utf-8')
    if stale and '--check' in sys.argv:
        sys.exit(f'not the muzzle devices\' texts (texts.py): {", ".join(stale)}')
    print(f'{2 * len(found)} keys in {len(tables)} dictionaries' + (f'; written: {", ".join(stale)}' if stale else ''))


if __name__ == '__main__':
    main()
