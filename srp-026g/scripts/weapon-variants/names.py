#!/usr/bin/env python3
"""names.py: the skins' names (names.tsv) into the ten localization tables, in key order.

usage: python3 names.py --inputs <prepare_inputs.py's folder> [--check]

A skin's name is its live gun's name in that language (the dictionary key the gun's config names)
and the skin's word (names.tsv); `st_srp_variant_<name>_name`. Its description is the gun's (the
config keeps the gun's key). --check only says whether the tables hold exactly these names.
"""
import argparse
from pathlib import Path
import sys

from common import HERE, LANGUAGES, ROOT, WEAPONS, Inputs, name_key, variants

TABLES = ROOT / 'srp-026g' / 'client-patches' / 'localization'


def words():
    out = {}
    for line in (HERE / 'names.tsv').read_text(encoding='utf-8').splitlines():
        if line.startswith('#') or not line.strip():
            continue
        cols = line.split('\t')
        out[cols[0]] = dict(zip(LANGUAGES, cols[1:]))
    return out


def table(language):
    rows = {}
    for line in (TABLES / f'{language}.tsv').read_text(encoding='utf-8').splitlines()[1:]:
        key, _, text = line.partition('\t')
        rows[key] = text
    return rows


def names(inputs):
    """{language: {key: text}} of every skin."""
    by_word = words()
    out = {language: {} for language in LANGUAGES}
    for row in variants():
        gun_key = inputs.config('compose', f'{WEAPONS}/wpn_{row.base}.options')['ui_desc']['text_descriptions']['name']
        for language in LANGUAGES:
            gun = table(language)[gun_key]
            word = by_word[row.name][language]
            out[language][name_key(row)] = f'{gun} {word}' if '"' in gun else f'{gun} "{word}"'
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    wanted = names(Inputs(args.inputs))
    wrong = 0
    for language, texts in wanted.items():
        path = TABLES / f'{language}.tsv'
        lines = path.read_text(encoding='utf-8').splitlines()
        rows = dict(line.split('\t', 1) for line in lines[1:])
        if args.check:
            for key, text in texts.items():
                if rows.get(key) != text:
                    print(f'{language}: {key} is {rows.get(key)!r}, not {text!r}')
                    wrong += 1
            continue
        rows.update(texts)
        path.write_text('\n'.join([lines[0]] + [f'{k}\t{rows[k]}' for k in sorted(rows)]) + '\n', encoding='utf-8')
    if wrong:
        sys.exit(f'{wrong} names differ from names.tsv')
    print('names: ' + ('the tables hold them' if args.check else f'{len(variants())} skins in {len(wanted)} tables'))


if __name__ == '__main__':
    main()
