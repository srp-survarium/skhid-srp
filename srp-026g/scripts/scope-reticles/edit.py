#!/usr/bin/env python3
"""The collimators' reticle lists: ../content-edit/scope-reticles-1.json.

usage: python3 -I edit.py           write it from reticles.tsv
       python3 -I edit.py --check   the committed list is what reticles.tsv makes

Each scope item of reticles.tsv gets `data/srp_reticle` (docs/notes/reticle-choice.md, "The data"):

    patterns: [ { name, model: weapons/reticles_069/<name> }, ... ]   index 0 the default
    sizes:    [ 1.0, 0.8, 0.6, 0.4 ]                                   0.69d0's dot sizes
    colours:  [ { name, rgb: float3 }, ... ]                           index 0 the default

In the config, `patterns`, `sizes` and `colours` are indexed tables, `rgb` a float3. A pattern's model
is a static model of pack weapon-modules (resource-porter's port-modules, RETICLE_PATTERNS), drawn
by the exe at the optic's `aim` place. The sizes are 0.69d0's `ncstar_4_dot_04` .. `_01` quads'
widths as ratios to `dot_04` (2.32, 1.86, 1.40 and 0.92 mm: 1, 0.80, 0.60, 0.40), the same scale for
every pattern. The colours are 0.69d0's `reticle_colors`.
"""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / 'content-edit' / 'scope-reticles-1.json'
MODELS = 'weapons/reticles_069'
PATTERNS = {'dot', 'eotech_ring', 'eotech_xr308', 'ncstar_cross', 'ncstar_bullseye', 'ncstar_starburst',
            'cobra_cross', 'cobra_tick', 'cobra_tick_dot', 'dppro_triangle'}
COLOURS = {'red': [1.0, 0.0, 0.0], 'green': [0.0, 1.0, 0.0], 'blue': [0.0, 0.0, 1.0]}
SIZES = [1.0, 0.8, 0.6, 0.4]
NAME = re.compile(r'[a-z0-9_]{1,23}')
ABOUT = ("The ten collimator scope items' reticle lists, data/srp_reticle {patterns, sizes, colours} "
         "(docs/notes/reticle-choice.md), from scripts/scope-reticles/reticles.tsv (made by edit.py): "
         "0.69d0's possible_reticles and reticle_colors of each optic, its dot's four sizes the size list.")


def rows():
    """[(dict id, item, patterns, colours)] of reticles.tsv, checked."""
    out = []
    for line in (HERE / 'reticles.tsv').read_text().splitlines():
        if not line or line.startswith('#'):
            continue
        dict_id, item, patterns, colours = line.split('\t')
        patterns, colours = patterns.split(','), colours.split(',')
        for name in patterns + colours:
            assert NAME.fullmatch(name), f'{item}: {name!r} is no name ([a-z0-9_], at most 23 bytes)'
        assert set(patterns) <= PATTERNS and len(set(patterns)) == len(patterns) <= 16, (item, patterns)
        assert set(colours) <= set(COLOURS) and len(set(colours)) == len(colours) <= 8, (item, colours)
        out.append((int(dict_id), item, patterns, colours))
    assert len({r[0] for r in out}) == len(out), 'a scope twice'
    return out


def make():
    edits = {'_about': ABOUT, 'add_value': []}
    for _, item, patterns, colours in rows():
        edits['add_value'].append({
            'files': [f'gameplay/items/scopes/{item}.options'],
            'table': 'data',
            'key': 'srp_reticle',
            'indexed': True,
            'value': {
                'patterns': [{'name': name, 'model': f'{MODELS}/{name}'} for name in patterns],
                'sizes': SIZES,
                'colours': [{'name': name, 'rgb': {'float3': COLOURS[name]}} for name in colours],
            },
        })
    return json.dumps(edits, indent=1, ensure_ascii=False) + '\n'


def main():
    text = make()
    if sys.argv[1:] == ['--check']:
        if OUT.read_text() != text:
            raise SystemExit(f'{OUT.name} is not what reticles.tsv makes: run edit.py')
        print(f'{OUT.name}: as reticles.tsv makes it')
        return
    OUT.write_text(text)
    print(f'{OUT.name}: {len(rows())} scope items')


if __name__ == '__main__':
    main()
