#!/usr/bin/env python3
"""Check the real pack against its base and the mounted archive file listing."""
import argparse
import copy
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_pack as pack
sys.path.insert(0, str(pack.HERE.parents[2] / 'release'))
import content_type_check as types


def refused(action):
    try:
        action()
    except ValueError:
        return
    raise AssertionError('invalid input accepted')


def check(base, folder, available):
    imported_by_mode = []
    assets = set()
    walk = types.load_walk(str(pack.HERE.parent / 'content-edit'))
    previous = {p.relative_to(base).as_posix(): p.read_bytes()
                for p in base.glob('projects/*/*/client_project')}
    corpus = types.corpus_with(walk, previous)
    for mode in pack.MODES:
        path = pack.project('v011e_level_03', mode)
        problems = types.violations(walk, previous[path], (folder / 'files' / path).read_bytes(),
                                    path, corpus, {})
        assert not problems, problems
        before = pack.bcw.loads((base / path).read_bytes())
        result = pack.bcw.loads((folder / 'files' / path).read_bytes())
        old = before.pop('client_objects')
        objects = result.pop('client_objects')
        assert result == before, 'map geometry, spawns or other root data changed'
        assert objects[:len(old)] == old, 'existing client objects changed'
        imported = objects[len(old):]
        assert len(imported) == 70
        assert len({o['full_name'] for o in objects}) == len(objects)
        imported_by_mode.append(imported)
        for _, prefix, _ in pack.GROUPS:
            group = [o for o in imported if o['full_name'].startswith(prefix)]
            pack.validate_links(group)
            active = [o for o in group if o['game_object_type'] == 'generic_anomaly_core'
                      and o['artefacts_enabled']]
            assert len(active) == 1, 'one spawner must own each group of containers'
            core = active[0]
            assert core['max_artefacts_count'] == 1
            assert core['artefacts_first_spawn_time_sec'] == 60
            expected_id, locations = (76, 6) if '/antena/' in prefix else (74, 5)
            assert {a['dict_id'] for a in core['artefacts']} == {expected_id}
            assert len(core['artefact_containers']) == locations
            damaged = copy.deepcopy(group)
            next(o for o in damaged if o['full_name'] == core['full_name'])['artefact_containers'][0] = 'missing'
            refused(lambda: pack.validate_links(damaged))
            refused(lambda: pack.validate_links(group + [group[0]]))
        for obj in imported:
            particle_paths = [obj[k] for k in ('idle_particle', 'activated_particle', 'deactivated_particle')
                              if obj.get(k)]
            if obj['game_object_type'] == 'particle':
                particle_paths.append(obj['lib_name'])
            assets.update(f'particles/{name}.particle' for name in particle_paths)
            for key in ('idle_sound', 'activated_sound', 'deactivated_sound', 'hit_sound'):
                if obj.get(key):
                    directory, suffix = {257: ('composite', 'composite_sound'),
                                         258: ('collections', 'sound_collection')}[obj[key + '_resource_id']]
                    assets.add(f'sounds/{directory}/{obj[key]}.{suffix}')
        # The transformer props are already in exactly the donor locations.
        donor = pack.bcw.load(base / pack.project('v021d_level_03', mode))
        def transformers(project):
            return sorted((o['position'], o['rotation'], o['scale']) for o in project['render_visuals']
                          if o['lib_name'].endswith('/transformator_big'))
        assert len(transformers(result)) == 3
        assert transformers(result) == transformers(donor)
        # A second application must refuse colliding full_names.
        donors = [(base / pack.project(name, mode)).read_bytes() for name, _, _ in pack.GROUPS]
        refused(lambda: pack.merge((folder / 'files' / path).read_bytes(), donors))
        print(f'{mode}: 70 additions, existing map preserved, links and artifact spawning checked')
    assert imported_by_mode[0] == imported_by_mode[1], 'game modes have different anomaly groups'
    assert assets <= available, f'missing assets: {sorted(assets - available)}'
    print(f'{len(assets)} particle/sound dependencies present in the published base; '
          'duplicate and dangling links refused')
    edits = pack.HERE.parent / 'content-edit'
    donor = json.loads((edits / 'v021d-radar-electric-1.json').read_text())
    expected = [entry | {'file': entry['file'].replace('v021d_level_03', 'v011e_level_03')}
                for entry in donor['set_entry']
                if entry['where']['full_name'] == 'anomaly/electro/electric_hit_damage_zone']
    actual = json.loads((edits / 'v011e-vostok-electric-1.json').read_text())
    assert len(expected) == 2 and actual['set_entry'] == expected, 'donor damage tuning drifted'
    manifest = (pack.HERE.parents[2] / 'release/content.tsv').read_text().splitlines()
    row = next(line.split('\t') for line in manifest if line.startswith(f'pack\t{pack.PACK_ID}@'))
    assert row[1] == f'{pack.PACK_ID}@{pack.VERSION}'
    assert row[2] == pack.sha256((folder / 'pack.json').read_bytes())
    print('Current donor tuning and manifest pack identity match')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', type=Path, required=True)
    parser.add_argument('--pack', type=Path, required=True)
    parser.add_argument('--files', type=Path, required=True,
                        help='srp-game-data files listing of stock with base patches 20, 22, 23')
    args = parser.parse_args()
    check(args.base, args.pack, {line.split('\t')[0] for line in args.files.read_text().splitlines()})
