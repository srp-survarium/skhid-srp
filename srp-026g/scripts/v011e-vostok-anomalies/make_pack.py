#!/usr/bin/env python3
"""Build v011e-vostok-anomalies@1 from the published base, never a release patch.

--base contains the six client_project files named in inputs.sha256, extracted
from stock 0.26g with base patches 20, 22 and 23 mounted (see README.md).
--out is a new resource-porter pack directory. release/build-content composes
it before content edits, including spawn protection and breakable lamps.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw

PACK_ID = 'v011e-vostok-anomalies'
VERSION = '1'
GROUPS = (
    ('level_03', 'logic/anomaly/random_anomalies/antena/',
     {'generic_anomaly_core': 3, 'damage_zone': 6, 'artefact_container': 6, 'collision_geometry': 12}),
    ('v021d_level_03', 'anomaly/electro/',
     {'generic_anomaly_core': 1, 'damage_zone': 10, 'artefact_container': 5,
      'collision_geometry': 6, 'particle': 21}),
)
MODES = ('day_2', 'day_4')


def project(map_name, mode):
    return f'projects/{map_name}/{mode}/client_project'


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def child(node, key):
    return next(value for name, value in node[1] if name == key.encode())


def validate_links(objects):
    """Every imported core -> zone/container -> shape link stays in its group."""
    by_name = {obj['full_name']: obj for obj in objects}
    if len(by_name) != len(objects):
        raise ValueError('duplicate object full_name')
    spawning = set()
    for obj in objects:
        links = [(entry['name'], 'collision_geometry') for entry in obj.get('collision_geometries', [])]
        if obj['game_object_type'] == 'generic_anomaly_core':
            links += [(name, 'artefact_container') for name in obj['artefact_containers']]
            links += [(name, 'damage_zone') for state in obj['states']
                      for group in state['groups'] for name in group['zones']]
            if obj['artefacts_enabled']:
                if not obj['artefacts'] or obj['max_artefacts_count'] < 1:
                    raise ValueError(f"no artefact spawning: {obj['full_name']}")
                spawning.update(obj['artefact_containers'])
        for name, kind in links:
            if name not in by_name or by_name[name]['game_object_type'] != kind:
                raise ValueError(f"{obj['full_name']}: unresolved {kind} {name}")
    containers = {name for name, obj in by_name.items() if obj['game_object_type'] == 'artefact_container'}
    if spawning != containers:
        raise ValueError('artefact locations without an enabled spawner')


def merge(target, donors):
    """Append complete groups as native typed nodes; keep every existing value."""
    original = bcw.read(target)
    old_objects = child(original, 'client_objects')
    names = {bcw.to_py(node)['full_name'] for node in old_objects[1]}
    additions = []
    for (_, prefix, counts), donor in zip(GROUPS, donors, strict=True):
        nodes = [node for node in child(bcw.read(donor), 'client_objects')[1]
                 if bcw.to_py(node)['full_name'].startswith(prefix)]
        objects = [bcw.to_py(node) for node in nodes]
        if Counter(obj['game_object_type'] for obj in objects) != counts:
            raise ValueError(f'{prefix}: unexpected source group')
        validate_links(objects)
        for obj in objects:
            if obj['full_name'] in names:
                raise ValueError(f"object already present: {obj['full_name']}")
            names.add(obj['full_name'])
        additions.extend(nodes)
    updated = (bcw.INDEXED, old_objects[1] + additions, old_objects[2] + len(additions))
    merged = (original[0], [(key, updated if key == b'client_objects' else value)
                            for key, value in original[1]], original[2])
    data = bcw.write(merged)
    # This includes every value's native type, vector bytes, count and table order.
    if bcw.read(data) != merged:
        raise ValueError('binary config round trip changed a value or type')
    return data


def build(base):
    hashes = {path: digest for digest, path in
              (line.split() for line in (HERE / 'inputs.sha256').read_text().splitlines())}
    inputs = {}
    for path, expected in hashes.items():
        data = (base / path).read_bytes()
        if sha256(data) != expected:
            raise ValueError(f'{path}: not the published base (expected sha256 {expected})')
        inputs[path] = data
    return {project('v011e_level_03', mode): merge(
                inputs[project('v011e_level_03', mode)],
                [inputs[project(name, mode)] for name, _, _ in GROUPS])
            for mode in MODES}, hashes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out already exists')
    files, hashes = build(args.base)
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': 'Vostok v0.11: main-building Electras from 0.26g and transformer anomalies '
                       'from the 0.21d port, with their Onyx and Spring spawns, in both modes',
        'operations': [{'op': 'replace_file', 'path': path, 'base_sha256': hashes[path]}
                       for path in sorted(files)],
        'notes': ['Built by scripts/v011e-vostok-anomalies/make_pack.py from inputs.sha256; '
                  '70 objects added per project, all existing values and native types preserved.',
                  'The 0.21d particle assets are already supplied by base patch 20. '
                  'Spawn protection and breakable lamps are applied afterwards by content edits.'],
    }
    for path, data in sorted(files.items()):
        destination = args.out / 'files' / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        print(f'{path}: {len(data)} bytes, sha256 {sha256(data)}')
    manifest = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(manifest)
    print(f'pack\t{PACK_ID}@{VERSION}\t{sha256(manifest)}\t{pack["description"]}')


if __name__ == '__main__':
    main()
