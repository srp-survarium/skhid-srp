#!/usr/bin/env python3
"""make_edits.py: the edit list of the room acoustics (exe patch 155), content-edit/room-acoustics-1.json.

usage: python3 -I make_edits.py --donor <folder> --live <folder> --paths <file> [--out <edit list>]

--donor  0.69d's files at their game paths, as make_pack.py's: the nine maps'
         projects/<level>/day_tdm/client_project and the collections and composites of their beds
--live   players' 0.26g game_materials/game.materials and material.pairs at their game paths
--paths  players' file list (`vostok-port list --db <their resources.db split>`): the client
         projects the zones go into
--out    default ../content-edit/room-acoustics-1.json

- `srp_sound_environments` (`add_value` at the root) in every client project of the nine maps both
  builds have, levels 1 to 9 (every weather and mode, Research's and Free for All's copies too), and
  of the 0.69d Rudnya port (v069d_level_01), from 0.69d's day_tdm project of the same level (0.69d's
  other modes and weathers load the same rooms; the nine are at 0.69d's coordinates). An environment
  each of 0.69d's (`is_scripted` ones left out: 0.69d ignores them until a script activates them),
  named by its object: `room` its environment_name; `ambient_sound` and its fades when it has a
  bed (rooms.bed, make_pack.py's path); `meshes` each mesh of its linked sound_environment_geometry
  primitives, named mesh_<k>: `kind` 0.69d's type (1 box, 2 cylinder, 0 sphere), `x y z` the
  world centre, `r00`..`r22` the rows of rot(mesh rotation) (local = R (world - centre)), `hx hy hz`
  the half sizes (a cylinder's radius and half height along its own y, a sphere's radius).
- the run and sprint pairs of material.pairs whose sound is a collection (`set_entry`): [257,
  the pack's composite of that collection and its room layers] (rooms.step_composite).
- the run and sprint pairs' composites (step/<ground>, step/<ground>_sprint) keep their items and
  take the room layers of their kind (`append_entry`, the pack's files), here and not in the pack:
  crouch-steps-1.json, after the packs, copies these composites for the crouch pairs, and its
  copies stay without rooms (0.69d has no crouch room layer). This list comes after it.
"""
import argparse
import json
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rooms  # noqa: E402

PORTS = {'v069d_level_01': 'level_01'}
PAIRS = 'game_materials/material.pairs'


def zones(donor, level):
    envs, scripted = rooms.environments(donor, level)
    if len(envs) > rooms.MAX_ENVIRONMENTS:
        sys.exit(f'{level}: {len(envs)} environments, more than the exe keeps')
    out, meshes, beds = {}, 0, []
    for name, env, shapes in envs:
        entry = {'room': env['environment_name']}
        found = rooms.bed(donor, env)
        if found:
            single, source, samples = found
            if single:
                entry['ambient_sound'] = rooms.moved(single)
                entry['fade_in'] = int(env['ambient_sound_fade_in_duration'])
                entry['fade_out'] = int(env['ambient_sound_fade_out_duration'])
            beds.append((name, single, source, samples, entry.get('fade_in'), entry.get('fade_out')))
        entry['meshes'] = {}
        for k, (kind, centre, rows, half) in enumerate(shapes):
            mesh = {'kind': kind, 'x': round(centre[0], 4), 'y': round(centre[1], 4), 'z': round(centre[2], 4)}
            mesh.update({f'r{i}{j}': round(rows[i][j], 6) + 0.0 for i in range(3) for j in range(3)})
            mesh.update({'hx': round(half[0], 4), 'hy': round(half[1], 4), 'hz': round(half[2], 4)})
            entry['meshes'][f'mesh_{k:03d}'] = {key: float(v) if key != 'kind' else v for key, v in mesh.items()}
        meshes += len(shapes)
        out[name] = entry
    if meshes > rooms.MAX_MESHES:
        sys.exit(f'{level}: {meshes} meshes, more than the exe keeps')
    return {'version': 1, 'environments': out}, scripted, beds, meshes


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--donor', required=True, type=Path)
    ap.add_argument('--live', required=True, type=Path)
    ap.add_argument('--paths', required=True, type=Path)
    ap.add_argument('--out', type=Path, default=Path(__file__).resolve().parent.parent / 'content-edit/room-acoustics-1.json')
    a = ap.parse_args()
    donor, live = rooms.Files(a.donor), rooms.Files(a.live)
    projects = {}
    for path in a.paths.read_text().split():
        m = re.match(r'projects/([^/]+)/[^/]+/client_project$', path)
        if m and (m.group(1) in rooms.LEVELS or m.group(1) in PORTS):
            projects.setdefault(PORTS.get(m.group(1), m.group(1)), []).append(path)
    edits = {'_about': "Room acoustics (exe patch 155, pack room-acoustics@1; docs/notes/room-acoustics.md): each "
                       "client project of levels 1-9 and of the 0.69d Rudnya port takes srp_sound_environments, "
                       "0.69d's day_tdm sound environments of its level (rooms, beds, zone meshes); the run and "
                       "sprint pairs whose sound is a collection play the pack's composite of it and 0.69d's room "
                       "step layers. Made by scripts/room-acoustics/make_edits.py.",
             'add_value': [], 'set_entry': []}
    for level in rooms.LEVELS:
        value, scripted, beds, meshes = zones(donor, level)
        files = sorted(projects.get(level, []))
        edits['add_value'].append({'files': files, 'table': '', 'key': 'srp_sound_environments', 'value': value})
        print(f'{level}: {len(files)} projects, {len(value["environments"])} environments, {meshes} meshes, '
              f'{len(scripted)} scripted left out')
        for name, single, source, samples, fade_in, fade_out in beds:
            if single:
                more = f', the first of {len(samples)}: {", ".join(samples)}' if len(samples) > 1 else ''
                print(f'  bed {name}: {rooms.moved(single)} in {fade_in} ms, out {fade_out} ms (0.69d {source}{more})')
            else:
                print(f'  bed {name}: none (0.69d {source}, a composite of {", ".join(samples)})')
    names = {m['id']: m['name'] for m in rooms.plain(live.config('game_materials/game.materials'))['materials']}
    for p in rooms.plain(live.config(PAIRS))['pairs']:
        kind = names.get(p['mtrl_1_id'])
        if kind in rooms.STEP_ROOTS and p.get('collision_sound') and p['collision_sound_type'] == rooms.RES_COLLECTION:
            edits['set_entry'].append({
                'file': PAIRS, 'list': 'pairs', 'where': {'mtrl_1_id': p['mtrl_1_id'], 'mtrl_2_id': p['mtrl_2_id']},
                'from': {'collision_sound': p['collision_sound'], 'collision_sound_type': rooms.RES_COLLECTION},
                'to': {'collision_sound': rooms.step_composite(kind, p['collision_sound']),
                       'collision_sound_type': rooms.RES_COMPOSITE}})
    print(f'{PAIRS}: {len(edits["set_entry"])} pairs to a new composite')
    edits['append_entry'] = []
    composites = {}
    for kind, sound_type, sound in sorted(rooms.step_pairs(live)):
        if sound_type == rooms.RES_COMPOSITE:
            composites.setdefault(kind, {})[sound] = len(rooms.live_items(live, sound))
    for kind, counts in sorted(composites.items()):
        items = [rooms.as_item(i) for i in rooms.room_items(donor, rooms.STEP_ROOTS[kind])]
        files = sorted(counts)
        for k, item in enumerate(items):
            edits['append_entry'].append({
                'files': [f'sounds/composite/{n}.composite_sound' for n in files],
                'expected_counts': [counts[n] + k for n in files],
                'list': 'composite_sound/sound_items', 'unique_key': None, 'value': item})
        print(f'{kind}: {len(files)} composites take {len(items)} room items ({rooms.STEP_ROOTS[kind]})')
    a.out.write_text(json.dumps(edits, indent=1, ensure_ascii=False) + '\n')
    print(f'written {a.out}')


if __name__ == '__main__':
    main()
