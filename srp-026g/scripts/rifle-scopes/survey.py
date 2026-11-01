#!/usr/bin/env python3
"""survey.py: each weapon's scope mount as our variants have it, and which weapons each new scope
fits (mounts.tsv, fit.tsv; make_pack.py builds from them).

usage: python3 -I survey.py --donor <folder> --live <folder> --survey <weapon survey's survey.json>

--donor   prepare_donor.sh's folder (the 0.69d0 modules, new and references.tsv's).
--live    prepare_live.sh's folder (our scope items, their models, the weapons' configs and models).
--survey  the weapon survey's work/survey.json: 0.69d0's modules, each with the weapons that take it
          (`mods.<name>.used_by`).

The mount (PROVEN in our data): patch 19's variants (models/weapons/attach/fit*, "0.51c5 modules,
placed by locfit.py") are 0.69d0's own module meshes (0.51c5's are the same, vertex for vertex) moved
by one translation per weapon, whatever the optic: the picatinny optics' origin is the rail's. A
weapon's mount is that translation and the rail surfaces its variant adds (picatini_lastochka on a
Kalashnikov, a scope rail on a weapon without one); for a collimator the variant of a collimator of
ours, for a scope a scope's (its aimed model also gives the eyepiece: background, lens, the reticle's
centre). A weapon with no variant of a picatinny optic has no mount: no new scope goes on it.

The fit: a new scope fits each weapon of ours 0.69d0 gives it (its name, or without a
_premium[_N] ending, or ALIASES) that has a mount of the scope's kind (a collimator's or a scope's;
a collimator takes a scope's mount when the weapon has no collimator's).
"""
import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import scopes_lib as lib  # noqa: E402

ALIASES = {'a_545': 'a545'}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--survey', required=True, type=Path)
    args = parser.parse_args()
    modules = lib.Modules(args.donor)
    found = lib.variants(args.live, modules)
    mounts = {}
    for rec in found:
        if not rec['module'] or len(rec['weapons']) != 1:
            continue
        w = rec['weapons'][0]
        mounts.setdefault(w, {}).setdefault(rec['kind'], rec)
    used = {m['name']: set(m['used_by']) for m in json.load(open(args.survey))['mods'].values()
            if isinstance(m, dict) and 'used_by' in m}
    lines = ['# made by survey.py: each weapon of ours with a mount, for a collimator and for a scope: the variant of ours',
             '# whose mount it takes, its translation (metres, the scope frame) and the rails it adds (- none)',
             '# weapon\tcollimator\tits translation\tits rails\tscope\tits translation\tits rails']

    def cells(rec):
        if not rec:
            return ['-', '-', '-']
        return [rec['name'], ','.join('%.5f' % x for x in rec['t']), ' '.join(rec['rails']) or '-']
    for w in sorted(mounts):
        lines.append('\t'.join([w] + cells(mounts[w].get('C')) + cells(mounts[w].get('S'))))
    (lib.HERE / 'mounts.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    lines = ['# made by survey.py: each new scope, the weapons of ours it fits (0.69d0 gives it to them, they have its mount)',
             '# scope\tweapons']
    for scope in lib.scopes():
        given = used.get(scope['module_name'], set())
        fits = []
        for w in sorted(mounts):
            plain = re.sub(r'_premium(_\d+)?$', '', w)
            if not ({w, plain, ALIASES.get(plain, plain)} & given):
                continue
            if scope['kind'] == 'S' and 'S' not in mounts[w]:
                continue
            fits.append(w)
        lines.append(f'{scope["name"]}\t{" ".join(fits)}')
        print(f'{scope["name"]}: {len(fits)} weapons')
    (lib.HERE / 'fit.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print(f'{len(mounts)} weapons with a mount')


if __name__ == '__main__':
    main()
