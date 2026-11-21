#!/usr/bin/env python3
"""texts.py: the grips' names and descriptions in the eleven localization tables
(../../client-patches/localization/<language>.tsv), under 0.69d0's own keys.

usage: python3 -I texts.py <donor folder> [--write]

A name is 0.69d0's own in the languages it ships (the donor folder's names/), names.tsv's in the
others. A description is the grip's stats (grips.py), a line of DESCRIPTION's words: the numbers
are never typed. Each table gets the keys in its key order (check_localization_026g.py wants the
keys sorted); rows this made before are replaced, so it can be run again after a retune or a
merge. Without --write it only says what would change.
"""
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import grips as lib  # noqa: E402

TABLES = HERE.parent.parent / 'client-patches' / 'localization'
DONOR_LANGUAGES = ('english', 'russian', 'ukrainian', 'german', 'polish')
# language: (recoil, aim time), filled with a signed percentage
DESCRIPTION = {
    'english': ('Recoil {}%.', 'Aim time {}%.'),
    'russian': ('Отдача {}%.', 'Время прицеливания {}%.'),
    'ukrainian': ('Віддача {}%.', 'Час прицілювання {}%.'),
    'belarusian': ('Аддача {}%.', 'Час прыцэльвання {}%.'),
    'german': ('Rückstoß {}%.', 'Zielzeit {}%.'),
    'polish': ('Odrzut {}%.', 'Czas celowania {}%.'),
    'serbian': ('Trzaj {}%.', 'Vreme nišanjenja {}%.'),
    'croatian': ('Trzaj {}%.', 'Vrijeme ciljanja {}%.'),
    'czech': ('Zpětný ráz {}%.', 'Doba míření {}%.'),
    'hungarian': ('Visszarúgás {}%.', 'Célzási idő {}%.'),
    'finnish': ('Rekyyli {}%.', 'Tähtäysaika {}%.'),
}


def donor_text(text):
    """A text of a `dump-db-config` file: the porter prints a config's UTF-8 bytes as
    Windows-1251 (WHATWG's: byte 0x98 is U+0098)."""
    return bytes(0x98 if c == '\x98' else c.encode('cp1251')[0] for c in text).decode('utf-8')


def texts(donor):
    """{language: {key: text}} of every grip's two keys."""
    grips = lib.grips(donor)
    lines = (HERE / 'names.tsv').read_text(encoding='utf-8').splitlines()
    languages = next(line for line in lines if line.startswith('# module\t')).split('\t')[1:]
    typed = {row[0]: dict(zip(languages, row[1:], strict=True))
             for row in (line.split('\t') for line in lines if line and not line.startswith('#'))}
    out = {}
    for language, (recoil, aim_time) in DESCRIPTION.items():
        own = (lib.leaves(Path(donor) / 'names' / f'{language}.leaves') if language in DONOR_LANGUAGES else None)
        table = out[language] = {}
        for grip in grips:
            table[grip.name_key] = (donor_text(own['strings.' + grip.name_key]) if own is not None
                                    else typed[grip.module][language])
            line = recoil.format(lib.percent(grip.recoil))
            if grip.aim_time:
                line += ' ' + aim_time.format(lib.percent(grip.aim_time))
            table[grip.description_key] = line
    return out


def main():
    donor, write = Path(sys.argv[1]), '--write' in sys.argv[2:]
    changed = 0
    for language, ours in texts(donor).items():
        path = TABLES / f'{language}.tsv'
        head, *lines = path.read_text(encoding='utf-8').split('\n')
        if lines[-1] != '':
            sys.exit(f'{path}: no newline at its end')
        rows = dict(line.split('\t', 1) for line in lines[:-1])
        if len(rows) != len(lines) - 1:
            sys.exit(f'{path}: a key twice')
        before = dict(rows)
        rows.update(ours)
        if rows != before:
            changed += 1
            print(f'{language}: {sum(1 for k in ours if before.get(k) != ours[k])} texts')
            if write:
                path.write_text('\n'.join([head] + [f'{k}\t{rows[k]}' for k in sorted(rows)]) + '\n', encoding='utf-8')
    print(f'{changed} tables {"written" if write else "would change"}' if changed else 'nothing to change')


if __name__ == '__main__':
    main()
