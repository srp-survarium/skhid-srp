#!/usr/bin/env python3
"""Verify the finished candidate against released maps and the current Radar repairs.

The reference/composed folders are srp-game-data get outputs from all released
archives, and those same archives plus the candidate, respectively. No patch
is built here. See README.md for the paths to extract.
"""
import argparse
import copy
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_pack as pack


def check(reference, composed, packs):
    for mode in pack.MODES:
        path = pack.project('v011e_level_03', mode)
        before = pack.bcw.read((reference / path).read_bytes())
        after = pack.bcw.read((composed / path).read_bytes())
        original_objects = pack.child(before, 'client_objects')
        result_objects = pack.child(after, 'client_objects')
        imported, retained = [], []
        prefixes = tuple(prefix for _, prefix, _ in pack.GROUPS)
        for node in result_objects[1]:
            (imported if pack.bcw.to_py(node)['full_name'].startswith(prefixes) else retained).append(node)
        assert len(imported) == 70
        assert retained == original_objects[1], f'{mode}: an existing object changed or disappeared'
        without_additions = (after[0], [(key, original_objects if key == b'client_objects' else value)
                                       for key, value in after[1]], after[2])
        assert without_additions == before, f'{mode}: existing geometry, lighting, spawns or settings changed'
        parsed = pack.bcw.to_py(after)
        assert len(parsed['server_objects']['respawn_points']) == 28
        assert all(o['invulnerability_time'] == 6 for o in parsed['server_objects']['respawn_points'])
        assert any(o['full_name'] == 'srp/artefact_keeper/pool' for o in parsed['client_objects'])
        for donor, prefix, _ in pack.GROUPS:
            source = pack.bcw.load(reference / pack.project(donor, mode))
            expected = [copy.deepcopy(o) for o in source['client_objects'] if o['full_name'].startswith(prefix)]
            # maps.tsv gives v0.11 a six-second search; v0.21 uses five.
            for obj in expected:
                if obj['game_object_type'] == 'artefact_container':
                    obj['artefacts_search_time_sec'] = 6
                    assert obj['artefacts_cooldown_time_sec'] == 80
            actual = [pack.bcw.to_py(o) for o in imported if pack.bcw.to_py(o)['full_name'].startswith(prefix)]
            assert actual == expected, f'{mode}: group differs from the current donor and v0.11 rules: {prefix}'
            pack.validate_links(actual)
        print(f'{mode}: all released values preserved; 28 spawns, protection, lamps and Keeper retained; '
              '70 additions match current donors and map rules')
    repaired = 0
    for ident in ('radar-v011e-seams-1', 'radar-v011e-textures-1'):
        folder = packs / ident
        manifest = json.loads((folder / 'pack.json').read_text())
        for op in manifest['operations']:
            path = op['path']
            assert (composed / path).read_bytes() == (folder / 'files' / path).read_bytes(), path
            repaired += 1
    materials = list((composed / 'models/ported/v011e/level/03').glob('terrain_p_*.model/settings'))
    assert materials, 'no terrain materials extracted'
    changed_materials = 0
    for path in materials:
        original = pack.bcw.load(reference / path.relative_to(composed))
        expected = detailed_material(original)
        assert pack.bcw.load(path) == expected, path
        changed_materials += expected != original
    assert changed_materials > 0, 'terrain repair was not exercised'
    print(f'{repaired} seam/texture files match the current repair packs; '
          f'{len(materials)} terrain model settings retain the corrected materials')


def detailed_material(value):
    if isinstance(value, dict):
        return {key: detailed_material(child) for key, child in value.items()}
    if isinstance(value, list):
        return [detailed_material(child) for child in value]
    if value == 'ported/v011e/terrain/03_base_final_lod':
        return 'ported/v011e/terrain/03_base_final'
    return value


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--composed', type=Path, required=True)
    parser.add_argument('--packs', type=Path, required=True)
    args = parser.parse_args()
    check(args.reference, args.composed, args.packs)
