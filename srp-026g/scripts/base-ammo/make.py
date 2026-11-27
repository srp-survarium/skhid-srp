#!/usr/bin/env python3
"""Ammo only at the bases, as on Rudnya (v0.69) (rudnya-ammo-1.json; the owner, 2026-10-31: "remove ammo
boxes not on bases in 0.69", 2026-11-01: "there should only be 2 crates and the team should only be able
to refill ammo on the closest crate"; 2026-11-22, of Vostok (v0.69): "remove ammo crates and make them like
in Rudnya").

    python3 -I make.py --live <folder> --out <content-edit list> <project>...

Each project (`projects/<map>/<variant>/client_project` under <folder>, as the manifest's rows above the
list leave it) keeps one crate a team: its base, the crate nearest the mean of that team's respawn points
(and nearer them than the other team's), refilling that team alone (the crate's resupply_zone of the other
team removed; its decal and collision geometry stay). Every other crate is removed whole, as
make_no_crates_edits.py takes one out (its zones, decals and collision geometry, its container_closed
models and their collision). A project with breakable lamps already (srp_lamps) is refused: their
visual indices would move; the list goes before the map's lamp list.
"""
import argparse
import importlib.util
import json
import math
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'ffa-mode'))
import make_edits as base  # noqa: E402

_spec = importlib.util.spec_from_file_location('no_crates', HERE.parent / 'ffa-mode' / 'make_no_crates_edits.py')
no_crates = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(no_crates)


def centre(points):
    return [sum(p[i] for p in points) / len(points) for i in range(3)]


def edits(live, projects):
    out, said = [], []
    for path in projects:
        tree = base.tree((live / path).read_bytes())
        if 'srp_lamps' in tree:
            raise ValueError(f'{path}: has breakable lamps already; the ammo list goes before its lamp list')
        found, models = no_crates.crates(tree, path)
        spawns = {t: centre([p['position'] for p in tree['server_objects']['respawn_points'] if p['team'] == t])
                  for t in (0, 1)}
        where = {folder: centre([o['position'] for o in inside if o['game_object_type'] == 'resupply_zone'])
                 for folder, inside in found}
        bases = {}
        for team in (0, 1):
            folder = min(where, key=lambda f: math.dist(where[f], spawns[team]))
            if math.dist(where[folder], spawns[team]) >= math.dist(where[folder], spawns[1 - team]):
                raise ValueError(f'{path}: team {team}\'s nearest crate {folder} is nearer the other team')
            if folder in bases.values():
                raise ValueError(f'{path}: {folder} is both teams\' nearest crate')
            bases[team] = folder
        removed_models = []
        for folder, inside in found:
            team = next((t for t, f in bases.items() if f == folder), None)
            if team is not None:
                zones = [o for o in inside if o['game_object_type'] == 'resupply_zone']
                if not any(o['team'] == team for o in zones):
                    raise ValueError(f'{path}: base {folder} has no zone of team {team}')
                gone = [o for o in zones if o['team'] != team]
                said.append(f'{path.split("/")[2]}: team {team}\'s base {folder} '
                            f'({", ".join(f"{v:.1f}" for v in where[folder])}), less {len(gone)} zone(s) of the other team')
            else:
                gone = inside
                removed_models += [(i, v) for i, v in models if any(
                    math.dist(v['position'], o['position']) <= no_crates.RADIUS for o in inside)]
                said.append(f'{path.split("/")[2]}: {folder} ({", ".join(f"{v:.1f}" for v in where[folder])}) removed')
            for o in gone:
                out.append({'file': path, 'list': 'client_objects',
                            'where': {'game_object_type': o['game_object_type'], 'full_name': o['full_name']}})
        kept = {i for folder in bases.values() for i, v in models
                if any(math.dist(v['position'], o['position']) <= no_crates.RADIUS
                       for o in dict(found)[folder])}
        for i, v in sorted({i: v for i, v in removed_models}.items()):
            if i in kept:
                raise ValueError(f'{path}: {v["lib_name"]} at {no_crates.at(v["position"])} is a base\'s and another crate\'s')
            out.append({'file': path, 'list': 'render_visuals',
                        'where': {'lib_name': v['lib_name'], 'position': no_crates.at(v['position'])}})
            prefix = f'resources/models/{v["lib_name"]}.model/'
            collisions = [c for c in tree['collision_objects'] if c['lib_name'].startswith(prefix)
                          and math.dist(c['position'], v['position']) < 0.0005]
            if not collisions:
                raise ValueError(f'{path}: {v["lib_name"]} at {no_crates.at(v["position"])} has no collision')
            for c in collisions:
                out.append({'file': path, 'list': 'collision_objects',
                            'where': {'lib_name': c['lib_name'], 'position': no_crates.at(c['position'])}})
    return out, said


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--about', required=True, help="the list's _about, before what it found")
    parser.add_argument('projects', nargs='+')
    args = parser.parse_args()
    removals, said = edits(args.live, args.projects)
    base.write({'_about': args.about + ' ' + '; '.join(said) + '. Generated by scripts/base-ammo/make.py. '
                'Changes the simulation (simulation_changes.tsv).', 'remove_entry': removals}, args.out)
    print('\n'.join(said))


if __name__ == '__main__':
    main()
