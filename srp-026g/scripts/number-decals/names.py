#!/usr/bin/env python3
"""names.py: the 1,000 number decals' ids, names and table rows (docs/notes/number-decals.md).

usage: python3 -I names.py --write    the rows into the localization, items.tsv and shop.tsv
       python3 -I names.py --check    whether those files hold exactly these rows

Every row of the number decals is made here, none by hand: the names in the twelve languages
(client-patches/localization/<language>.tsv, one key a number), the dictionary rows
(content-edit/items.tsv: no price, premium) and the shop rows (content-edit/shop.tsv: not sold).
--write takes out the rows of a number decal it finds and puts these in; nothing else changes.
make_pack.py reads the names back from the localization, as fumo-neco-decals does.
"""
import argparse
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
SRP = HERE.parents[1]
LOCALIZATION = SRP / 'client-patches' / 'localization'
ITEMS_TSV = SRP / 'scripts' / 'content-edit' / 'items.tsv'
SHOP_TSV = SRP / 'scripts' / 'content-edit' / 'shop.tsv'

NUMBERS = range(1, 1001)
# The dictionary ids 5001..6000, a free block (main and every open branch end at 4801): N's is
# FIRST_DICT_ID + N. The modification ids 210001..211000 the same way (decals use 200101..200799,
# 200000 + 100 x row + column; camouflages 130000..130400).
FIRST_DICT_ID = 5000
FIRST_MODIFICATION = 210000


def rc(number):
    """The decal's own name, as `4_77` is a flag's: `num_0042`. Zero-padded, so the keys sort
    in the numbers' order."""
    if number not in NUMBERS:
        raise ValueError(f'{number}: not a number decal (1..1000)')
    return f'num_{number:04}'


def dict_id(number):
    rc(number)
    return FIRST_DICT_ID + number


def item_name(number):
    return 'decals_' + rc(number)


def name_key(number):
    return f'st_character_decals_{rc(number)}_name'


def hungarian_suffix(number):
    """The -s ending of a numeral written in digits ("a 42-es szám", "a 3-as busz"): that of the
    last word read out. Units: egy, kettő, négy, hét, kilenc -es; három, nyolc -as; öt -ös;
    hat -os. Whole tens: tíz, negyven, ötven, hetven, kilencven -es; húsz, harminc, hatvan,
    nyolcvan -as. Whole hundreds (száz) -as, ezer -es."""
    units = {1: 'es', 2: 'es', 3: 'as', 4: 'es', 5: 'ös', 6: 'os', 7: 'es', 8: 'as', 9: 'es'}
    tens = {1: 'es', 2: 'as', 3: 'as', 4: 'es', 5: 'es', 6: 'as', 7: 'es', 8: 'as', 9: 'es'}
    if number % 10:
        return units[number % 10]
    if number % 100:
        return tens[number % 100 // 10]
    if number % 1000:
        return 'as'
    return 'es'


# Each language's own way of naming a number: "Number 42", "Номер 42".
FORMS = {
    'english': lambda n: f'Number {n}',
    'ukrainian': lambda n: f'Номер {n}',
    'russian': lambda n: f'Номер {n}',
    'german': lambda n: f'Nummer {n}',
    'polish': lambda n: f'Numer {n}',
    'serbian': lambda n: f'Broj {n}',
    'czech': lambda n: f'Číslo {n}',
    'hungarian': lambda n: f'{n}-{hungarian_suffix(n)} szám',
    'finnish': lambda n: f'Numero {n}',
    'belarusian': lambda n: f'Нумар {n}',
    'croatian': lambda n: f'Broj {n}',
    'portuguese': lambda n: f'Número {n}',
}


def name(language, number):
    return FORMS[language](number)


def item_rows():
    """items.tsv: dict_id, name, base_cost 0 (sold back for nothing), premium (as the other
    give-only decals: never dropped, and shop.tsv says no shop sells it)."""
    return [f'{dict_id(n)}\t{item_name(n)}\t0\ttrue\t\t' for n in NUMBERS]


def shop_rows():
    return [f'{item_name(n)}/not_sold\tNumber {n}: given by account id, never sold' for n in NUMBERS]


def is_ours(line, prefix):
    return line.startswith(prefix)


def with_rows(lines, ours, rows, sort):
    kept = [line for line in lines if not ours(line)]
    if not sort:
        return kept + rows
    header, body = kept[:1], kept[1:]
    merged = sorted(body + rows, key=lambda line: line.split('\t', 1)[0])
    # Only our rows move: the file was sorted already.
    if [line for line in merged if not ours(line)] != body:
        sys.exit('the localization file was not sorted by key')
    return header + merged


def planned():
    """{path: its lines with the number decals' rows}."""
    files = {}
    for language in FORMS:
        path = LOCALIZATION / f'{language}.tsv'
        rows = [f'{name_key(n)}\t{name(language, n)}' for n in NUMBERS]
        files[path] = with_rows(path.read_text(encoding='utf-8').splitlines(),
                                lambda line: line.startswith('st_character_decals_num_'), rows, True)
    for path, rows, prefix in ((ITEMS_TSV, item_rows(), None), (SHOP_TSV, shop_rows(), 'decals_num_')):
        lines = path.read_text(encoding='utf-8').splitlines()
        if prefix is None:
            ids = {str(dict_id(n)) for n in NUMBERS}
            ours = lambda line, ids=ids: line.split('\t', 1)[0] in ids  # noqa: E731
        else:
            ours = lambda line, prefix=prefix: line.startswith(prefix)  # noqa: E731
        files[path] = with_rows(lines, ours, rows, False)
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--write', action='store_true')
    action.add_argument('--check', action='store_true')
    args = parser.parse_args()
    stale = []
    for path, lines in planned().items():
        text = '\n'.join(lines) + '\n'
        if path.read_text(encoding='utf-8') != text:
            stale.append(path)
            if args.write:
                path.write_text(text, encoding='utf-8')
    for path in stale:
        print(f'{"wrote" if args.write else "not as names.py makes it:"} {path.relative_to(SRP.parent)}')
    if args.check and stale:
        sys.exit(1)


if __name__ == '__main__':
    main()
