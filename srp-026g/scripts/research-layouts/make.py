#!/usr/bin/env python3
"""make.py: a second Research layout of a map, its three units at other places, as maps of their own
(the owner, 2026-11-22, of School: "it should be variation, either that one or another one";
docs/notes/research_mode.md, "Layouts").

usage: python3 -I make.py <layout>.json --base <folder> [--write <content-edit folder>]

<layout>  the layout: the map, its new ids, and for each unit where its station stands (`marker` x and
          z, the `floor` its generator stands on, a `yaw` in degrees about the marker), its capture
          zone as the mode config keeps it, its roof `antenna`'s place and its `cover` (the Research
          project's own cover inside `from`, x0 z0 x1 z1, moved with the station and by `shift`).
--base    the files as a build of release/content.tsv without the layout's list leaves them, at their
          game paths (`srp-game-data get`): the map's Team Deathmatch and Research projects and its
          Research configs.
--write   writes the layout's edit list there.

A layout's map is a copy of the map's Team Deathmatch project (the same spawns and ammo, probes and
lamps) with the Research project's stations at the new places: a station is what the Research project
has over the Team Deathmatch one within STATION of a unit (its models, their collision, its sounds,
sparks and light), moved as one piece. What else Research added there (other cover, a moved car)
is not taken. The config is the map's Research config with the units' places and zones; its
maps_dict row comes after the map's own, so the room's draw takes either layout alike
(session rooms `Left::take`: the ids of a map and mode).
"""
import argparse
import json
import math
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

MODES = 'gameplay/game_modes/'
DICTIONARY = 'gameplay/db_static_dictionaries'
SECTIONS = ('render_visuals', 'collision_objects')
GENERATOR = 'props/electric/diesel_generator'
ANTENNA = 'item/charging_device/antena'
# A station's pieces: within this of its unit's place across (m), and from under its floor to its top.
STATION, BELOW, ABOVE = 3.0, 1.5, 3.0


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def rx(a):
    c, s = math.cos(a), math.sin(a)
    return [[1, 0, 0], [0, c, s], [0, -s, c]]


def ry(a):
    c, s = math.cos(a), math.sin(a)
    return [[c, 0, -s], [0, 1, 0], [s, 0, c]]


def rz(a):
    c, s = math.cos(a), math.sin(a)
    return [[c, s, 0], [-s, c, 0], [0, 0, 1]]


def mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def matrix(angles):
    """The game's rotation of a `rotation` (research-mode make_pack.rotation: rows the local axes)."""
    return mul(mul(rx(-angles[0]), ry(-angles[1])), rz(-angles[2]))


def angles(m):
    """The `rotation` of a matrix (matrix's inverse)."""
    return (-math.atan2(m[1][2], m[2][2]), -math.asin(max(-1.0, min(1.0, -m[0][2]))), -math.atan2(m[0][1], m[0][0]))


class Move:
    """A piece's way from the Research project to the layout: about `pivot` (x, y, z) by `yaw`, to `to`."""

    def __init__(self, pivot, to, yaw):
        self.pivot, self.to, self.turn = pivot, to, ry(-math.radians(yaw))

    def place(self, position):
        rel = [position[i] - self.pivot[i] for i in range(3)]
        rel = [sum(rel[k] * self.turn[k][j] for k in range(3)) for j in range(3)]
        return [f32(self.to[i] + rel[i]) for i in range(3)]

    def rotation(self, rotation):
        if self.turn == ry(0.0):
            return list(rotation)
        new = angles(mul(matrix(rotation), self.turn))
        again = matrix(new)
        want = mul(matrix(rotation), self.turn)
        if max(abs(again[i][j] - want[i][j]) for i in range(3) for j in range(3)) > 1e-4:
            sys.exit(f'rotation {rotation}: not turned exactly')
        return [f32(a) for a in new]

    def shifted(self, shift):
        moved = Move(self.pivot, [self.to[i] + shift[i] for i in range(3)], 0)
        moved.turn = self.turn
        return moved

    def entry(self, entry):
        out = dict(entry)
        out['position'] = self.place(entry['position'])
        out['rotation'] = self.rotation(entry['rotation'])
        return out


def key(entry):
    return json.dumps(entry, sort_keys=True)


def plain(value):
    """bcw's values as JSON keeps them (tuples as lists)."""
    return json.loads(json.dumps(value))


def model_of(entry):
    name = entry.get('visual_name') or entry['lib_name']
    for part in ('resources/models/',):
        name = name.removeprefix(part)
    return name.split('.model/')[0]


def across(a, b):
    return math.hypot(a[0] - b[0], a[2] - b[2])


def layout_of(spec, tdm, research, config, said):
    """The layout's additions to the Team Deathmatch project and its units' config values."""
    zones = {zone['cp_id']: zone for zone in config['mode']['parameters']['capture_zones']}
    added = {section: [e for e in research[section] if key(e) not in {key(o) for o in tdm[section]}]
             for section in SECTIONS}
    names = {o['full_name'] for o in tdm['client_objects']}
    added['client_objects'] = [o for o in research['client_objects'] if o['full_name'] not in names]
    out = {section: [] for section in added}
    units = []
    taken = set()
    for unit in spec['units']:
        zone = zones[unit['cp_id']]
        old = zone['position']
        near = [e for e in added['render_visuals'] if across(e['position'], old) <= STATION]
        floors = [e['position'][1] for e in near if e['visual_name'] == GENERATOR]
        if len(floors) != 1:
            sys.exit(f"unit {unit['cp_id']}: {len(floors)} generators at its station, not 1")
        pivot = [old[0], floors[0], old[2]]
        move = Move(pivot, [unit['marker'][0], unit['floor'], unit['marker'][1]], unit.get('yaw', 0))
        count = {}
        for section, entries in added.items():
            for i, entry in enumerate(entries):
                p = entry['position']
                if across(p, old) <= STATION and -BELOW <= p[1] - pivot[1] <= ABOVE:
                    piece = move
                elif section != 'client_objects' and model_of(entry) == ANTENNA and unit.get('antenna') \
                        and min(spec['units'], key=lambda u: abs(zones[u['cp_id']]['position'][2] - p[2])) is unit:
                    piece = Move(p, unit['antenna'], 0)
                else:
                    piece = next((move.shifted(c['shift']) for c in unit.get('cover', [])
                                  if c['from'][0] <= p[0] <= c['from'][2] and c['from'][1] <= p[2] <= c['from'][3]),
                                 None)
                if piece is None:
                    continue
                if (section, i) in taken:
                    sys.exit(f'{section}: an entry of two units')
                taken.add((section, i))
                out[section].append(piece.entry(entry))
                count[section] = count.get(section, 0) + 1
        place = move.place(old)
        units.append((unit, zone, place))
        said.append(f"  unit {unit['cp_id']} ({unit['what']}): " + ', '.join(f'{n} {s}' for s, n in count.items())
                    + f"; marker {[round(v, 2) for v in old]} -> {[round(v, 2) for v in place]}")
    for section, entries in added.items():
        left = sorted({model_of(e) if section != 'client_objects' else e['full_name']
                       for i, e in enumerate(entries) if (section, i) not in taken})
        if left:
            said.append(f'  {section} not taken: ' + ', '.join(left))
    # Two entries alike (0.27d's stations have a cable corner twice) are one: an entry is known by its values.
    for section in SECTIONS:
        seen, once = set(), []
        for entry in out[section]:
            if key(entry) not in seen:
                seen.add(key(entry))
                once.append(entry)
        out[section] = once
    return out, units


def only(entries, entry, what):
    if sum(key(e) == key(entry) for e in entries) != 1:
        sys.exit(f'{what}: not the one entry of its values')
    return entry


def edits_of(spec, base):
    level = spec['level']
    edits = {'_about': '', 'copy': [], 'set_string': [], 'set_entry': [], 'add_entry': [], 'append_entry': []}
    said = []
    for row in spec['maps']:
        weather, cfg = row['weather'], row['config']
        source, target = f'projects/{level}/{weather}_4/', f"projects/{level}/{weather}_{spec['project']}/"
        tdm = plain(bcw.loads((base / source / 'client_project').read_bytes()))
        research = plain(bcw.loads((base / f'projects/{level}/{weather}_5/client_project').read_bytes()))
        config = plain(bcw.loads((base / MODES / cfg).read_bytes()))
        project = f'{level}/{weather}_5'
        if config['mode']['match_making_params']['map_name'] != project or config['mode']['mode'] != 5:
            sys.exit(f'{cfg}: not the Research config of {project}')
        said.append(f"{row['id']} {spec['folder']}/{cfg} ({level}/{weather}_{spec['project']}):")
        added, units = layout_of(spec, tdm, research, config, said)
        file = target + 'client_project'
        edits['copy'].append({'files': source + 'client_project', 'from': source, 'to': target})
        for section in SECTIONS:
            after = only(tdm[section], tdm[section][-1], f'{source}{section}: its last entry')
            like = next(e for e in tdm[section] if sum(key(e) == key(o) for o in tdm[section]) == 1)
            for entry in added[section]:
                if entry.keys() != like.keys():
                    sys.exit(f'{file}: a {section} entry of other keys')
                edits['add_entry'].append({'files': file, 'list': section, 'after': after, 'like': like,
                                           'expected_files': 1, 'values': entry})
                after = entry
        for i, entry in enumerate(added['client_objects']):
            edits['append_entry'].append({'file': file, 'list': 'client_objects',
                                          'expected_count': len(tdm['client_objects']) + i,
                                          'unique_key': 'full_name', 'vectors': True, 'value': entry})
        new = f"{MODES}{spec['folder']}/{cfg}"
        edits['copy'].append({'files': MODES + cfg, 'from': MODES, 'to': f"{MODES}{spec['folder']}/"})
        edits['set_string'].append({'file': new, 'path': 'mode/match_making_params/map_name', 'from': project,
                                    'to': f"{level}/{weather}_{spec['project']}"})
        for zone in config['mode']['parameters']['capture_zones']:
            unit, _, place = next(u for u in units if u[1] is zone)
            if len(zone['shapes']) != 1:
                sys.exit(f"{cfg}: unit {zone['cp_id']} of {len(zone['shapes'])} shapes")
            shape, want = zone['shapes'][0], unit['zone']
            if [shape['axis_x'], shape['axis_y'], shape['axis_z']] != [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]:
                sys.exit(f"{cfg}: unit {zone['cp_id']}'s shape is turned")
            # set_entry: its vectors are written anew (two units' equal `half` are one vector in the file).
            edits['set_entry'].append({
                'file': new, 'list': 'mode/parameters/capture_zones', 'where': {'cp_id': zone['cp_id']},
                'from': {'position': zone['position'], 'shapes[0]/type': shape['type'],
                         'shapes[0]/position': shape['position'], 'shapes[0]/half': shape['half']},
                'to': {'position': place, 'shapes[0]/type': want['type'],
                       'shapes[0]/position': [f32(v) for v in want['position']],
                       'shapes[0]/half': [f32(v) for v in want['half']]}})
        dictionary_id = next(e['id'] for e in plain(bcw.loads((base / DICTIONARY).read_bytes()))['maps_dict']
                             if e['cfg_mode_name'] == cfg)
        edits['add_entry'].append({'files': DICTIONARY, 'list': 'maps_dict', 'after': {'id': dictionary_id},
                                   'like': {'id': dictionary_id}, 'expected_files': 1,
                                   'values': {'cfg_mode_name': f"{spec['folder']}/{cfg}", 'id': row['id']}})
    ids = [row['id'] for row in spec['maps']]
    edits['_about'] = (
        f"Research's second layout of {spec['name'].split(',')[0]} (docs/notes/research_mode.md, \"Layouts\"; the "
        "owner, 2026-11-22: \"it should be variation, either that one or another one\"): maps_dict "
        f"{ids[0]}-{ids[-1]}, each a copy of the map's Research config under gameplay/game_modes/{spec['folder']}/ "
        f"with its units at " + '; '.join(u['what'] for u in spec['units']) + f", and the project "
        f"projects/{level}/<weather>_{spec['project']}/, a copy of the map's Team Deathmatch project with the "
        "Research project's stations there. Generated by scripts/research-layouts/make.py from "
        f"{Path(spec['file']).name}; after every list that edits the map's Team Deathmatch project or Research "
        "config, before client-dictionary-1.json.")
    return edits, said


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('layout', type=Path)
    parser.add_argument('--base', type=Path, required=True)
    parser.add_argument('--write', type=Path)
    args = parser.parse_args()
    spec = json.loads(args.layout.read_text())
    spec['file'] = str(args.layout)
    edits, said = edits_of(spec, args.base)
    print('\n'.join(said))
    if args.write:
        text = json.dumps(edits, ensure_ascii=False, separators=(',', ':')).replace('},{', '},\n{') + '\n'
        (args.write / spec['edits']).write_text(text, encoding='utf-8')
        print(f"{args.write / spec['edits']}: " + ', '.join(f'{len(v)} {k}' for k, v in edits.items() if k != '_about'))


if __name__ == '__main__':
    main()
