#!/usr/bin/env python3
"""Check the 0.21 maps' weather packs against their authored day-source export."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('lamps', ROOT / 'srp-026g/scripts/breakable-lamps/make_edits.py')
lamps = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lamps)

# Each weather pack's maps: rain 2/4 and evening 2/4 a level.
PACKS = {'maps-021d-weathers-2': {'03': (170, 171, 172, 173), '06': (179, 180, 253, 254),
                                  '02': (1, 2, 3, 4), '05': (5, 6, 7, 8), '01': (262, 263, 264, 265)},
         'maps-021d-chemical-weathers-1': {'04': (268, 269, 270, 271)}}
MAPS = {level: ids for maps in PACKS.values() for level, ids in maps.items()}
DAY_MARKERS = (-32281, -32195, -32198, -32187, -32196, -32287)
LIGHTING_PACK = 'three-maps-021-lighting-8'


def check(packs, source):
    lighting = packs / LIGHTING_PACK
    weathers = [packs / name for name in PACKS]
    for folder in (*weathers, lighting):
        manifest = folder / 'pack.json'
        pack = json.loads(manifest.read_text())
        pin = f"pack\t{pack['id']}@{pack['version']}\t{hashlib.sha256(manifest.read_bytes()).hexdigest()}\t"
        assert pin in (ROOT / 'release/content.tsv').read_text(), f'{folder.name}: content pin differs'
    projects = []
    for weather in weathers:
        pack = json.loads((weather / 'pack.json').read_text())
        rows = [row for op in pack['operations'] if op['op'] == 'dictionary_add' for row in op['entries']]
        expected = {n for ids in PACKS[weather.name].values() for n in ids}
        assert len(rows) == len(expected) and {row['id'] for row in rows} == expected
        assert pack['requires'] == ['three-maps-021-lighting']
        own = sorted((weather / 'files').glob('projects/*/*/client_project'))
        assert len(own) == len(expected)
        assert not (weather / 'files/shaders').exists(), 'weather pack must not overwrite combined lighting'
        projects += [(weather / 'files', project) for project in own]
    source_info = json.loads((source / 'source.json').read_text())
    from make_environment import make
    from make_geometry import apply as geometry_apply
    geometry_edits = json.loads((ROOT / 'srp-026g/scripts/content-edit/v021d-weather-geometry-1.json').read_text())
    environment_edits = json.loads((ROOT / 'srp-026g/scripts/content-edit/v021d-weather-environment-1.json').read_text())
    assert make(*weathers) == environment_edits, 'original constructor-default edit needs regeneration'
    for files, project in projects:
        path = project.relative_to(files).as_posix()
        before = str(Path(path).with_name('client_project')).replace('/rain_', '/day_').replace('/evening_', '/day_')
        day_bytes = (source / before).read_bytes()
        assert hashlib.sha256(day_bytes).hexdigest() == source_info['files'][before]
        data = bytearray(project.read_bytes())
        for edit in environment_edits['add_value']:
            if path in edit['files']:
                lamps.c.add_value(data, edit['table'], edit['key'], edit['value'])
        expected = geometry_apply(day_bytes, path, geometry_edits)
        data = geometry_apply(data, path, geometry_edits)
        day, new = lamps.tree(expected), lamps.tree(data)
        assert day.keys() == new.keys(), path
        for key in day.keys() - {'client_objects'}:
            assert day[key] == new[key], f'{path}: {key} changed'
        assert not lamps.check_built(path, data), path
        assert 'srp_renderer_v021d' not in new
        environment = [o for o in new['client_objects'] if o.get('game_object_type') == 'environment']
        assert len(environment) == 1, path
        assert all(key in environment[0]['properties'] for key in ('height_based_ambient_low_color', 'height_based_ambient_high_color')), path
        level = project.parts[-3][-2:]
        rain = project.parts[-2].startswith('rain_')
        marker = -32021 - MAPS[level][0 if rain else 2]
        assert environment[0]['properties']['mie_fog_base_height'] == marker, path
    shaders = [p.read_bytes() for p in (lighting / 'files/shaders').rglob('*') if p.is_file()]
    assert len(shaders) == 128, len(shaders)
    markers = [*DAY_MARKERS, *(-32021 - ids[i] for ids in MAPS.values() for i in (0, 2))]
    for marker in markers:
        assert any(struct.pack('<f', marker) in shader for shader in shaders), f'missing shader marker {marker}'
    # School and Chemical Plant rain: 8 visuals + 4 collisions and 2 fixtures a project; the Fort: 9 removals a project.
    assert len(geometry_edits['append_entry']) == 48
    assert len(geometry_edits['add_value']) == 8
    assert len(geometry_edits['remove_entry']) == 36
    print(f'{len(projects)} weather projects: day gameplay preserved with exact authored weather fixture/barrel deltas; all lamp bindings valid; {len(markers)} lighting profiles in 128 shaders')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packs', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True)
    args = parser.parse_args()
    check(args.packs, args.source)


if __name__ == '__main__':
    main()
