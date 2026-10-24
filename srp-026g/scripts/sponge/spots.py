#!/usr/bin/env python3
"""spots.py: where the Sponge (758) and the Rattle (759) spawn: the owner's picks (spots.tsv) and the
edit list that puts them into the maps' mode configs (scripts/content-edit/artefact-spots-<n>.json),
which exe patch 59 reads (docs/notes/sponge.md "Spawns", client_patch.md 3.52).

usage: python3 -I spots.py import <editor overrides folder> <spots.tsv>
       python3 -I spots.py edit-list <spots.tsv> <live folder> <out .json>
       python3 -I spots.py check <spots.tsv> <live folder> <edit list .json>

import     the spawn editor's files, one a spot (`project`, `core`, `container`, `container_index`,
           `pos`, `from`, `to`, `scope` "spot", `kinds` [{to, count 1}]), to spots.tsv: one row a spot,
           sorted; nothing else of the files is kept.
edit-list  <live folder> holds the game's files as the players have them, at their game paths
           (`srp-game-data get` of the stock db and the listed patches): gameplay/db_static_dictionaries,
           gameplay/game_modes/, projects/*/client_project. Every row is checked against every project
           of its map (the folder: projects/<map>/<weather>/client_project): the core (its `full_name`)
           is there once, spawns artefacts (kinds and containers) and its `artefact_containers[index]`
           is the row's container; any other answer stops it. The rows of a map go into each mode
           config whose `mode/match_making_params/map_name` is in that folder (maps_dict's
           `cfg_mode_name`s), as `mode/artefact_spots`: [{core, index, container, kind}].
check      edit-list's checks, and that the edit list is what it writes (the committed one is current).

The rule (the owner, 2026-10-24): a picked container spawns only its kind; the anomaly's other
containers spawn its own kinds as before; an anomaly with no pick is as 0.26g has it.
"""
import collections
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

KINDS = (758, 759)
COLUMNS = ('map', 'core', 'index', 'container', 'kind', 'from', 'pos')
KEY = 'artefact_spots'


def die(text):
    sys.exit(f'spots.py: {text}')


def read_tsv(path):
    lines = [line for line in Path(path).read_text(encoding='utf-8').splitlines() if line and not line.startswith('#')]
    if tuple(lines[0].split('\t')) != COLUMNS:
        die(f'{path}: the columns are {", ".join(COLUMNS)}')
    rows = []
    for line in lines[1:]:
        cells = dict(zip(COLUMNS, line.split('\t')))
        if len(line.split('\t')) != len(COLUMNS):
            die(f'{path}: {line!r}: {len(COLUMNS)} cells')
        cells['index'], cells['kind'] = int(cells['index']), int(cells['kind'])
        if cells['kind'] not in KINDS:
            die(f'{path}: {line!r}: the kind is 758 or 759')
        rows.append(cells)
    spots = [(r['map'], r['core'], r['index']) for r in rows]
    if len(set(spots)) != len(spots):
        die(f'{path}: a spot twice')
    return rows


def import_overrides(folder, out):
    rows = []
    for file in sorted(Path(folder).glob('*.json')):
        o = json.loads(file.read_text(encoding='utf-8'))
        if o.get('scope') != 'spot' or o.get('to') not in KINDS or o.get('kinds') != [{'count': 1, 'kind': o['to']}]:
            die(f'{file}: not one spot of the Sponge or the Rattle')
        rows.append((o['project'], o['core'], str(o['container_index']), o['container'], str(o['to']),
                     ','.join(str(k) for k in o['from']), ' '.join(f'{v:g}' for v in o['pos'])))
    rows.sort(key=lambda r: (r[0], r[1], int(r[2])))
    head = ('# The owner\'s spots for the Sponge (758) and the Rattle (759), chosen in the spawn editor '
            '(2026-10-24): one row a container of an anomaly core of a map (the project folder); `from` '
            'the core\'s own kinds, `pos` where the container is (x y z, as the Maps tab draws it). '
            'srp-026g/scripts/sponge/spots.py makes the edit list artefact-spots-<n>.json of it.\n')
    Path(out).write_text(head + '\t'.join(COLUMNS) + '\n' + ''.join('\t'.join(r) + '\n' for r in rows),
                         encoding='utf-8')
    print(f'{out}: {len(rows)} spots, {collections.Counter(r[4] for r in rows)}')


def edit_list(rows, live):
    live = Path(live)
    by_map = collections.defaultdict(list)
    for r in rows:
        by_map[r['map']].append(r)
    projects = collections.defaultdict(list)
    for file in sorted(live.glob('projects/*/*/client_project')):
        projects[file.parent.parent.name].append(file)
    for m, picks in sorted(by_map.items()):
        if not projects.get(m):
            die(f'{m}: no project of that map')
        for file in projects[m]:
            cores = collections.defaultdict(list)
            for o in bcw.load(file).get('client_objects') or []:
                if o.get('game_object_type') == 'generic_anomaly_core':
                    cores[o.get('full_name')].append(o)
            for r in picks:
                found = cores.get(r['core'], [])
                if len(found) != 1:
                    die(f'{file}: {len(found)} cores named {r["core"]}')
                core = found[0]
                own = [a['dict_id'] for a in core.get('artefacts') or []]
                containers = core.get('artefact_containers') or []
                if not own or not containers:
                    die(f'{file}: {r["core"]} spawns no artefacts')
                if r['index'] >= len(containers) or containers[r['index']] != r['container']:
                    die(f'{file}: {r["core"]}: artefact_containers[{r["index"]}] is not {r["container"]}')
                if sorted(own) != sorted(int(k) for k in r['from'].split(',')):
                    die(f'{file}: {r["core"]}: its kinds are {own}, the row says {r["from"]}')
    dictionary = bcw.load(live / 'gameplay' / 'db_static_dictionaries')
    configs = collections.defaultdict(set)
    for row in dictionary['maps_dict']:
        path = live / 'gameplay' / 'game_modes' / row['cfg_mode_name']
        if not path.is_file():
            continue
        name = ((bcw.load(path).get('mode') or {}).get('match_making_params') or {}).get('map_name') or ''
        configs[name.split('/')[0]].add('gameplay/game_modes/' + row['cfg_mode_name'])
    edits = []
    for m, picks in sorted(by_map.items()):
        files = sorted(configs.get(m, ()))
        if not files:
            die(f'{m}: no mode config of maps_dict plays it')
        value = [{'core': r['core'], 'index': r['index'], 'container': r['container'], 'kind': r['kind']}
                 for r in sorted(picks, key=lambda r: (r['core'], r['index']))]
        edits.append({'files': files, 'table': 'mode', 'key': KEY, 'value': value})
    about = ('the owner\'s spots of the Sponge (758) and the Rattle (759) (2026-10-24): each map\'s mode configs get '
             '`mode/artefact_spots`, its picked containers ({core full_name, index in artefact_containers, '
             'container, kind}), which exe patch 59 reads: a picked container spawns only its kind, the '
             'anomaly\'s other containers its own kinds, an anomaly with no pick is 0.26g\'s (made by '
             'srp-026g/scripts/sponge/spots.py from spots.tsv)')
    return {'_about': about, 'add_value': edits}


def dump(edits):
    lines = ',\n'.join('  ' + json.dumps(e, ensure_ascii=False) for e in edits['add_value'])
    return '{\n "_about": ' + json.dumps(edits['_about'], ensure_ascii=False) + ',\n "add_value": [\n' + lines + '\n ]\n}\n'


def main():
    if len(sys.argv) == 4 and sys.argv[1] == 'import':
        out = Path(sys.argv[3])
        if out.exists():
            die(f'{out}: already there')
        import_overrides(sys.argv[2], out)
    elif len(sys.argv) == 5 and sys.argv[1] in ('edit-list', 'check'):
        rows = read_tsv(sys.argv[2])
        text = dump(edit_list(rows, sys.argv[3]))
        out = Path(sys.argv[4])
        if sys.argv[1] == 'check':
            if out.read_text(encoding='utf-8') != text:
                die(f'{out}: not what spots.tsv and the live files give (spots.py edit-list)')
        else:
            if out.exists():
                die(f'{out}: already there')
            out.write_text(text, encoding='utf-8')
        kinds = collections.Counter(r['kind'] for r in rows)
        print(f'{out}: {len(rows)} spots ({kinds[758]} Sponge, {kinds[759]} Rattle) on '
              f'{len({r["map"] for r in rows})} maps, every row checked against every project of its map')
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
