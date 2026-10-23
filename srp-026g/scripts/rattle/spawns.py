#!/usr/bin/env python3
"""spawns.py: the edit list that puts the Rattle (items_dict 759) into the maps' anomalies
(srp-026g/scripts/content-edit/rattle-spawns-<n>.json, applied by content_edit.py after
sponge-spawns-1).

usage: python3 -I spawns.py <sponge-spawns-1.json> <out .json>

The Rattle spawns where the Sponge does (docs/notes/rattle.md): every generic_anomaly_core the
Sponge's list gives {dict_id 758, count 1} (each one with an artefact container: 200 cores of
74 projects) gets {dict_id 759, count 1} right after it. The Sponge's list is the source, so the
two lists name the same cores; the Rattle's entry follows the Sponge's, which its `after` and
`like` name.
"""
import collections
import json
from pathlib import Path
import sys

SPONGE = {'dict_id': 758, 'count': 1}
RATTLE = {'dict_id': 759, 'count': 1}


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    source, out = Path(sys.argv[1]), Path(sys.argv[2])
    if out.exists():
        sys.exit(f'{out}: already there')
    sponge = json.loads(source.read_text(encoding='utf-8'))
    if set(sponge) != {'_about', 'add_entry'}:
        sys.exit(f'{source}: not an add_entry list')
    entries, per_map, seen = [], collections.Counter(), set()
    for entry in sponge['add_entry']:
        if entry['values'] != SPONGE or not entry['list'].endswith('/artefacts'):
            sys.exit(f'{source}: an entry that is not the Sponge in a core\'s artefacts: {entry}')
        core = (entry['files'], entry['list'])
        if core in seen:
            sys.exit(f'{source}: {core} twice')
        seen.add(core)
        entries.append({'files': entry['files'], 'list': entry['list'], 'after': dict(SPONGE),
                        'like': dict(SPONGE), 'values': dict(RATTLE)})
        per_map[entry['files']] += 1
    about = ('the Rattle (items_dict 759, pack rattle@1) in the maps\' anomalies: every generic_anomaly_core '
             'sponge-spawns-1 gives the Sponge (each one with an artefact container) gets {dict_id 759, count 1} '
             'after the Sponge (made by srp-026g/scripts/rattle/spawns.py)')
    lines = ',\n'.join('  ' + json.dumps(entry, ensure_ascii=False) for entry in entries)
    out.write_text('{\n "_about": ' + json.dumps(about, ensure_ascii=False) + ',\n "add_entry": [\n' + lines + '\n ]\n}\n',
                   encoding='utf-8')
    if json.loads(out.read_text(encoding='utf-8'))['add_entry'] != entries:
        sys.exit(f'{out}: does not read back')
    for path in sorted(per_map):
        print(f'{path}: {per_map[path]} cores')
    print(f'{len(entries)} cores in {len(per_map)} projects; {out}')


if __name__ == '__main__':
    main()
