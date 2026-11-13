#!/usr/bin/env python3
"""Writes content-edit/no-ammo-crates-1.json: Free for All plays on its own projects, without ammo
crates (the owner, 2026-11-12/13: "there should just be no ammo crates", "TDM is good, nothing should
change about it").

Each Free for All config (gameplay/game_modes/ffa/*.options) names its Team Deathmatch map's project
(mode.match_making_params.map_name, as ffa-1 copied it); the list points it at the map's own copy
(make_edits.copy_of, `<weather>_8`, made by ffa-projects-1 and edited by the FFA lists) and takes
every ammo crate out of the copy. TDM's projects keep theirs. A crate, as rudnya-ammo-1.json removes one, is everything of
client_objects in the folder of a resupply_zone (its zones, one a team it serves, its decals and
the collision geometry the zones name) and its item/quest/container_closed models (render_visuals,
one or two stacked) with their collision_objects (collision, walker_collision). Every
container_closed of these projects is a crate's (within RADIUS of a zone or its geometry): one
that is not, a folder holding anything else or a crate without a model is refused. The
breakable lamps' render_visuals indices (srp_lamps, breakable-lamps-1) after a removed model drop
by the models removed before them.

    python3 make_no_crates_edits.py --live <folder>

<folder> holds the FFA configs and their projects as the manifest's rows above the list leave
them: release/build_content.py's edits/no-ammo-crates-1.json/live (the list's `_inputs` brings
the configs there).
"""
import argparse
import math
from pathlib import Path

import make_edits as base

OUTPUT = base.HERE.parent / 'content-edit/no-ammo-crates-1.json'
CONFIGS = 'gameplay/game_modes/ffa/'
CRATE = 'item/quest/container_closed'
CRATE_TYPES = {'resupply_zone', 'decal', 'collision_geometry'}
RADIUS = 3.0


def projects(live):
    """[(config, its TDM map's project name, the copy's)], by config."""
    configs = sorted((live / CONFIGS).glob('*.options'))
    if not configs:
        raise ValueError(f'{live / CONFIGS}: no Free for All configs')
    out = []
    for f in configs:
        name = base.tree(f.read_bytes())['mode']['match_making_params']['map_name']
        copy = base.copy_of(f'projects/{name}/client_project')
        out.append((f.relative_to(live).as_posix(), name, copy.removeprefix('projects/').removesuffix('/client_project')))
    return out


def at(position):
    return [round(x, 4) for x in position]


def crates(tree, path):
    """The crates of a project ([(folder, its client objects)], in client_objects' order) and
    their models. Two crates may stand side by side (one a team): a model is every crate's near it."""
    objects = tree['client_objects']
    folders = {}
    for o in objects:
        if o.get('game_object_type') == 'resupply_zone':
            folders.setdefault(o['full_name'].rsplit('/', 1)[0], [])
    for o in objects:
        folder = o['full_name'].rsplit('/', 1)[0]
        if folder in folders:
            if o.get('game_object_type') not in CRATE_TYPES:
                raise ValueError(f'{path}: {o["full_name"]} is not a crate\'s')
            folders[folder].append(o)
    def near(v, folder):
        return any(math.dist(v['position'], o['position']) <= RADIUS for o in folders[folder])
    models = [(i, v) for i, v in enumerate(tree['render_visuals']) if v['lib_name'].endswith(CRATE)]
    for _, v in models:
        if not any(near(v, folder) for folder in folders):
            raise ValueError(f'{path}: {v["lib_name"]} at {at(v["position"])} is no crate\'s')
    for folder in folders:
        if not any(near(v, folder) for _, v in models):
            raise ValueError(f'{path}: crate {folder} has no model')
    return list(folders.items()), models


def edits(live):
    out, lamps, counts = [], [], []
    configs = projects(live)
    repoint = [{'files': config, 'from': name, 'to': copy} for config, name, copy in configs]
    for path in sorted({f'projects/{copy}/client_project' for _, _, copy in configs}):
        tree = base.tree((live / path).read_bytes())
        found, models = crates(tree, path)
        if not found:
            raise ValueError(f'{path}: no ammo crate')
        for _, inside in found:
            for o in inside:
                out.append({'file': path, 'list': 'client_objects',
                            'where': {'game_object_type': o['game_object_type'], 'full_name': o['full_name']}})
        removed = sorted(i for i, _ in models)
        for lamp, row in tree.get('srp_lamps', {}).get('lamps', {}).items():
            for key, visual in row['visuals'].items():
                index = visual['index']
                if index in removed:
                    raise ValueError(f'{path}: lamp {lamp} {key} is a crate\'s model')
                below = sum(i < index for i in removed)
                if below:
                    lamps.append({'file': path, 'path': f'srp_lamps/lamps/{lamp}/visuals/{key}/index',
                                  'from': index, 'to': index - below})
        for _, v in models:
            out.append({'file': path, 'list': 'render_visuals',
                        'where': {'lib_name': v['lib_name'], 'position': at(v['position'])}})
            prefix = f'resources/models/{v["lib_name"]}.model/'
            collisions = [c for c in tree['collision_objects'] if c['lib_name'].startswith(prefix)
                          and math.dist(c['position'], v['position']) < 0.0005]
            if not collisions:
                raise ValueError(f'{path}: {v["lib_name"]} at {at(v["position"])} has no collision')
            for c in collisions:
                out.append({'file': path, 'list': 'collision_objects',
                            'where': {'lib_name': c['lib_name'], 'position': at(c['position'])}})
        counts.append(f'{path.removeprefix("projects/").removesuffix("/client_project")} {len(found)}')
    about = ('Free for All on its own projects, without ammo crates (the owner, 2026-11-12/13: "there should '
             'just be no ammo crates", "TDM is good, nothing should change about it"): each Free for All config\'s '
             'map_name, its TDM map\'s project, becomes the map\'s own copy (<weather>_8, ffa-projects-1, as the '
             'FFA lists leave it), and every ammo crate of the copy is removed whole: its resupply_zone(s), '
             'decal(s) and collision_geometry (client_objects), its container_closed model(s) (render_visuals) and '
             'their collision_objects; the breakable lamps\' visual indices after them follow. Team Deathmatch\'s '
             'projects keep their crates. Crates a project: ' + ', '.join(counts) + '. '
             'Generated by scripts/ffa-mode/make_no_crates_edits.py from the files as every row above leaves '
             'them; `_inputs` brings the configs it reads. Changes the simulation (simulation_changes.tsv).')
    return {'_about': about, '_inputs': [{'files': CONFIGS + '*'}], 'replace_string': repoint, 'set': lamps,
            'remove_entry': out}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', type=Path, required=True)
    args = parser.parse_args()
    base.write(edits(args.live), OUTPUT)
    print(OUTPUT)


if __name__ == '__main__':
    main()
