#!/usr/bin/env python3
"""spawns.py: the edit list that puts the Sponge (items_dict 758) into the maps' anomalies
(srp-026g/scripts/content-edit/sponge-spawns-<n>.json, applied by content_edit.py).

usage: python3 -I spawns.py <projects folder> <out .json>

<projects folder>  every map's client project at its game path (projects/<map>/<weather>/client_project),
                   as the manifest's rows before the list leave them (`srp-game-data get <layers>
                   --out <folder> <path>...`; the stock maps and the base rows' maps).

Every generic_anomaly_core that spawns artefacts (an `artefacts` list and at least one artefact
container) gets {dict_id 758, count 1} after its last entry: one more kind in the anomaly's
artefacts (the others have count 2 to 10). Cores without containers (v011e_level_04's
disabled track controllers) spawn nothing and are left alone. Prints the cores per map and
weather, and each pool before and after.
"""
import collections
import json
import os
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

DICT_ID = 758


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    if out.exists():
        sys.exit(f'{out}: already there')
    entries, per_map, skipped, pools = [], collections.Counter(), collections.Counter(), collections.Counter()
    projects = sorted(p for p in root.rglob('client_project') if p.is_file())
    if not projects:
        sys.exit(f'{root}: no client projects')
    for file in projects:
        path = file.relative_to(root).as_posix()
        project = bcw.load(file)
        for index, obj in enumerate(project['client_objects']):
            if obj.get('game_object_type') != 'generic_anomaly_core':
                continue
            artefacts = obj.get('artefacts') or []
            if not artefacts or not obj.get('artefact_containers'):
                skipped[path] += 1
                continue
            if any(a['dict_id'] == DICT_ID for a in artefacts):
                sys.exit(f'{path}: {obj["full_name"]} has the Sponge already')
            last = {'dict_id': artefacts[-1]['dict_id'], 'count': artefacts[-1]['count']}
            if sum(1 for a in artefacts if a == last) != 1:
                sys.exit(f'{path}: {obj["full_name"]}: its last entry is not the only one so')
            entries.append({'files': path, 'list': f'client_objects[{index}]/artefacts', 'after': last, 'like': last,
                            'values': {'dict_id': DICT_ID, 'count': 1}})
            per_map[path] += 1
            before = ' + '.join(f"{a['dict_id']} x{a['count']}" for a in artefacts)
            pools[f'{obj["full_name"]}: {before} -> {before} + {DICT_ID} x1'] += 1
    about = ('the Sponge (items_dict 758, pack sponge@2) in the maps\' anomalies: every generic_anomaly_core '
             'with an artefact container gets {dict_id 758, count 1} after its last artefact kind '
             '(made by srp-026g/scripts/sponge/spawns.py)')
    # An entry a line: the list reads as the cores it changes.
    lines = ',\n'.join('  ' + json.dumps(entry, ensure_ascii=False) for entry in entries)
    out.write_text('{\n "_about": ' + json.dumps(about, ensure_ascii=False) + ',\n "add_entry": [\n' + lines + '\n ]\n}\n',
                   encoding='utf-8')
    if json.loads(out.read_text(encoding='utf-8'))['add_entry'] != entries:
        sys.exit(f'{out}: does not read back')
    for path in sorted(set(per_map) | set(skipped)):
        print(f'{path}: {per_map[path]} cores' + (f' ({skipped[path]} without containers left alone)'
                                                   if skipped[path] else ''))
    print('pools (a core: before -> after; how many projects):')
    for line, n in sorted(pools.items()):
        print(f'  {line}  [{n}]')
    print(f'{len(entries)} cores in {len(per_map)} projects; {sum(skipped.values())} cores left alone; {out}')


if __name__ == '__main__':
    main()
