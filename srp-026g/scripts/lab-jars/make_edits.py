#!/usr/bin/env python3
"""Author lab-jars-1.json: Vector Laboratory's glass jars break one by one.

The owner (2026-11-27, of the racks of jars on Vector's lower floor: "I
expected those one"): each props/lab/chemical_flask of Vector (level_07,
five racks a project, ten jars each) draws pack lab-jars@1's
srp_chemical_flask_frame instead (the rack without its glass: its visual's
names set), and each of its jars is a fixture of its project's lamp table,
`jar<r>_<k>` (rack r in the project's order, jar k the pack's): the jar's
bounds 1 cm larger (what a shot or a blast must reach), srp_chemical_flask_jar<k>
drawn while intact and its _broken stump while broken, both at the jar's
centre, placed after the project's last visual; the glass line the jar's
axis, its loose pieces its five shards (kind 'pane_shard', each at its
centre with the jar's outward normal `out_*` and its long side `along_*`;
the owner: "jar racks don't have shards"). The racks' collision objects
take the pack's srp_chemical_flask_collision, the rack's without the jars'
glass (the owner: "glass bullet holes are left"): a bullet breaks a jar on
its way through it.

--projects holds `projects/<level>/<project>/client_project` as the manifest
leaves them before this list; --stock the rack's files (make_pack.py's).
Indices are those of the final render_visuals (build-content's lamp check
refuses a build where a later list moved them).
"""
import argparse
import importlib.util
import json
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS / 'breakable-lamps'))
import fixtures  # noqa: E402
import make_edits as lamps  # noqa: E402

spec = importlib.util.spec_from_file_location('lab_jars', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pack)

LEVELS = ('level_07',)
MARGIN = 0.01


def pane_shard(index, out, along):
    piece = {'index': index, 'kind': 'pane_shard'}
    piece |= {f'out_{a}': v for a, v in zip('xyz', out)}
    piece |= {f'along_{a}': v for a, v in zip('xyz', along)}
    return piece


def rows_of(path, data, jars, shards):
    root = lamps.tree(data)
    visuals = root.get('render_visuals', [])
    racks = [(i, v) for i, v in enumerate(visuals) if lamps.model_name(v) == pack.RACK]
    if not racks:
        return None
    table = fixtures.table_of(path, root)
    placer = fixtures.Placer(path, visuals)
    names = [{'file': path, 'path': f'render_visuals[{i}]/{key}', 'from': pack.RACK, 'to': pack.FRAME}
             for i, v in racks for key in fixtures.names(v)]
    # the racks' collision without the jars' glass (the walkers' stays the rack's)
    whole, glassless = (f'resources/models/{model}.model/collision' for model in (pack.RACK, pack.COLLISION))
    collisions = [i for i, o in enumerate(root.get('collision_objects', [])) if o.get('lib_name') == whole]
    assert len(collisions) == len(racks), f'{path}: {len(collisions)} rack collisions, {len(racks)} racks'
    names += [{'file': path, 'path': f'collision_objects[{i}]/lib_name', 'from': whole, 'to': glassless}
              for i in collisions]
    rows = {}
    for r, (_, rack) in enumerate(racks):
        # the list sets the names before it adds entries: they are placed as the frame
        v = rack | {key: pack.FRAME for key in fixtures.names(rack)}
        for k, ((centre, (low, high)), whole, broken) in enumerate(zip(jars, pack.jar_names(), pack.broken_names())):
            at = lamps.world_bounds(v, centre, centre)[0]
            intact = placer.place(v, whole, at)
            stump = placer.place(v, broken, at)
            lo, hi = lamps.world_bounds(v, [a - MARGIN for a in low], [a + MARGIN for a in high])
            axis = [[centre[0], low[1] + 0.05, centre[2]], [centre[0], high[1] - 0.05, centre[2]]]
            start, end = (lamps.world_bounds(v, p, p)[0] for p in axis)
            loose = {f'glass{j}': pane_shard(placer.place(v, name, lamps.world_bounds(v, c, c)[0]), out, along)
                     for j, (name, (c, out, along)) in enumerate(zip(pack.shard_names(k), shards[k]))}
            rows[f'jar{r}_{k}'] = fixtures.row(lo, hi, start, end, visuals={'intact0': {'index': intact, 'state': 0},
                                                                            'broken0': {'index': stump, 'state': 1}},
                                               lights={}, loose=loose)
    fixtures.check_keys(path, table, rows)
    return names, placer.entries, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    hashes = json.loads(Path(__file__).with_name('stock.json').read_text())
    jars, shards = pack.jars(args.stock, hashes), pack.shard_info(args.stock, hashes)
    names, placed, found, every = [], {}, {}, []
    for path, data in fixtures.projects(args.projects, LEVELS):
        every.append(path)
        made = rows_of(path, data, jars, shards)
        if made is None:
            continue
        names += made[0]
        placed[path] = made[1]
        found[path] = made[2]
        print(f'{path}: {len(made[2])} jars')
    edits = {'_about': "Vector Laboratory's glass jars break (the owner, 2026-11-27): each rack of jars "
             '(props/lab/chemical_flask) draws its frame without glass (pack lab-jars@1) and each of its ten jars is '
             "a row of its project's lamp table, the jar drawn while intact, its stump once broken and its five shards "
             "flying, placed after "
             "the project's last visual (docs/notes/breakable_lamps.md \"Vector's jars\"). Authored by "
             'scripts/lab-jars/make_edits.py over the projects as the manifest leaves them; indices refer to the '
             'final render_visuals.',
             'set_string': names, 'add_entry': fixtures.compact(placed, every), 'add_value': fixtures.grouped(found)}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
