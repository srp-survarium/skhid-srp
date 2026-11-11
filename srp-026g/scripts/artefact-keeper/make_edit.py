#!/usr/bin/env python3
"""make_edit.py: write content-edit/artefact-keeper-1.json, the hidden Keeper anomaly of every map.

usage: python3 -I make_edit.py <live folder>

<live folder>  every map's live client_project, flattened as `dbgrab <db> get /client_project <dir>`
               writes them (projects__level_02__day_4__client_project), the stock resources.db's
               with every offered patch's over them in order.

Keeper of the Artifacts (docs/notes/artefact-skills.md) gives a Keeper player a dormant artefact
of the match's kind from a pool no anomaly of the map spawns from: one generic_anomaly_core
appended to each project's client_objects (every project but the lobby's), with

- a pool of 20 dormant artefacts of each kind (crates/artefact-carry/src/keeper.rs, KINDS and POOL;
  crates/survarium/tests/artefact_keeper.rs checks the list against them): its own cook makes them
  with the map's own cook chain, its tick ticks them and its save writes them in every world;
- no artefact container: it never spawns one (spawn_artefact 0x31B6C0 returns without a free
  container), and a chance of 0 besides;
- energy off and one state at threshold 0 with no groups: no zone, nothing drawn, heard or hit (the
  tick runs the selected state without a null check 0x31BCB0, so the state is there);
- every key a real anomaly has (level_02's stalactite anomaly): the game's config lookup has no end
  check, so the load must find each key it reads.

The entry pins each project's client_objects count: a map change in between is refused, run again.
"""
import json
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'content-edit'))
from content_edit import rec, name_at, children, T_NAMED, T_INDEXED  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'content-edit', 'artefact-keeper-1.json')
FULL_NAME = 'srp/artefact_keeper/pool'
# keeper.rs's KINDS in draw order: Larkspur (artefact_lifebone), Onyx, Spring; POOL of each.
KINDS = [54, 76, 74]
POOL = 20

ANOMALY = {
    'game_object_type': 'generic_anomaly_core',
    'type': 'object',
    'tool_name': 'misc',
    'lib_name': 'generic_anomaly',
    'lib_guid': '5c1f6e0a-7d43-4b8e-9a2f-a7e1c0de2b19',
    'name': 'artefact_keeper',
    'full_name': FULL_NAME,
    'layer': 'default',
    'is_temporary': False,
    'position': [0.0, -1000.0, 0.0],
    'rotation': [0.0, 0.0, 0.0],
    'pivot': [0.0, 0.0, 0.0],
    'scale': [1.0, 1.0, 1.0],
    'energy_enabled': False,
    'energy_initial': 0,
    'energy_maximum': 0,
    'energy_decrease_speed': 0,
    'energy_on_walk': 0,
    'energy_on_run': 0,
    'energy_on_sprint': 0,
    'energy_on_jump': 0,
    'energy_on_shoot': 0,
    'energy_on_explosion': 0,
    'energy_on_character_hit': 0,
    'energy_on_character_kill': 0,
    'energy_af_container_use': 0,
    'artefacts_enabled': True,
    'artefacts_first_spawn_time_sec': 60,
    'artefacts_respawn_time_sec': 60,
    'artefacts_respawn_variation_sec': 0,
    'artefacts_respawn_chance': 0,
    'max_artefacts_count': 0,
    'artefacts': [{'dict_id': kind, 'count': POOL} for kind in KINDS],
    'artefact_containers': [],
    'states': [{
        'enabled': True,
        'energy_threshold': 0,
        'active_time_sec': 0,
        'energy_on_exit': 0,
        'select_priority': 0,
        'shoot_trigger': False,
        'zone_activity_trigger': False,
        'groups': [],
    }],
}


def client_objects(buf):
    for c in children(buf, 0):
        if name_at(buf, rec(buf, c)[1]) == b'client_objects':
            return c
    return None


def main(folder):
    files, counts = [], []
    for f in sorted(os.listdir(folder)):
        if not f.startswith('projects__') or not f.endswith('__client_project'):
            continue
        path = f.replace('__', '/')
        if path.startswith('projects/lobby_scene/'):
            continue
        buf = open(os.path.join(folder, f), 'rb').read()
        if rec(buf, 0)[3] != T_NAMED:
            raise SystemExit(f'{path}: not a config')
        objects = client_objects(buf)
        if objects is None or rec(buf, objects)[3] != T_INDEXED:
            raise SystemExit(f'{path}: no client_objects')
        files.append(path)
        counts.append(rec(buf, objects)[4])
    about = ('Keeper of the Artifacts\' hidden anomaly on every map (docs/notes/artefact-skills.md; '
             'scripts/artefact-keeper/make_edit.py made this list): one generic_anomaly_core a project, '
             f'{FULL_NAME}, with no container, energy off and one empty state, and a pool of {POOL} '
             'dormant artefacts of each Keeper kind (Larkspur 54, Onyx 76, Spring 74) that only exe '
             'patch 109 takes from. Changes the simulation (simulation_changes.tsv).')
    entry = {'files': files, 'expected_counts': counts, 'list': 'client_objects', 'unique_key': 'full_name',
             'vectors': True, 'value': ANOMALY}
    with open(OUT, 'w') as out:
        json.dump({'_about': about, 'append_entry': [entry]}, out, indent=1)
        out.write('\n')
    print(f'{len(files)} projects -> {OUT}')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    main(sys.argv[1])
