#!/usr/bin/env python3
"""Apply the FFA source edit and check native types, preserved TDM data and registration.

--pack is stalker-maps@6; --dictionary is the composed dictionary containing
its six TDM rows. Uses a temporary folder; never changes source game data.
"""
import argparse
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import make_ffa_edits as m


def check(pack, dictionary):
    expected = m.edits(pack, dictionary)
    assert m.OUT.read_text() == m.encoded(expected), 'stale generated edit list'
    with tempfile.TemporaryDirectory(prefix='stalker-ffa-check-') as tmp:
        root = Path(tmp)
        live, out = root / 'live', root / 'out'
        paths = {m.DICTIONARY}
        paths.update(e['from'] for e in expected['copy'])
        paths.update(e['files'] for e in expected['add_entry'])
        for path in paths:
            source = dictionary if path == m.DICTIONARY else m.in_pack(pack, path)
            dest = live / path
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, dest)
        run = subprocess.run([sys.executable, str(m.EDIT), str(m.OUT), str(live), str(out)],
                             capture_output=True, text=True)
        if run.returncode:
            raise RuntimeError(run.stdout + run.stderr)
        assert '0 unexpected differences' in run.stdout
        original_dict = m.tree(dictionary.read_bytes())
        new_dict = m.tree((out / m.DICTIONARY).read_bytes())
        new_rows = new_dict['maps_dict']
        assert len({r['id'] for r in new_rows}) == len(new_rows), 'duplicate maps_dict ID'
        assert len({r['cfg_mode_name'] for r in new_rows}) == len(new_rows)
        ids = {ffa for _, _, ffa in m.MAPS}
        restored = copy.deepcopy(new_dict)
        restored['maps_dict'] = [r for r in new_rows if r['id'] not in ids]
        assert restored == original_dict, 'unrelated dictionary data changed'
        for key, _, ffa_id in m.MAPS:
            path = m.ffa_copy(f'projects/{m.project(key)}/client_project')
            old = m.tree((live / path).read_bytes())
            new_buf = (out / path).read_bytes()
            new = m.tree(new_buf)
            spawns = new['server_objects']['respawn_points']
            count = m.COUNTS[key]
            assert len(spawns) == 24 + count
            assert spawns[:24] == old['server_objects']['respawn_points'], 'TDM spawns changed'
            assert len({(s['team'], s['point_id']) for s in spawns}) == 24 + count
            assert [s['point_id'] for s in spawns[24:]] == list(range(count))
            assert len({s['full_name'] for s in spawns}) == 24 + count
            assert len({s['name'] for s in spawns[24:]}) == count
            assert not ({s['name'] for s in spawns[:24]} & {s['name'] for s in spawns[24:]})
            for i, point in enumerate(spawns[24:], start=24):
                assert (point['team'], point['invulnerability_time'], point['priority']) == (2, 2, 1)
                assert point['enemy_check_radius'] == 30.0 and point['ally_check_radius'] == 1.0
                assert point['is_temporary'] == 0
                # The engine reads protection as an integer and radii as typed floats.
                for field, kind in [('invulnerability_time', m.c.T_INT),
                                    ('enemy_check_radius', m.c.T_FLOAT), ('ally_check_radius', m.c.T_FLOAT)]:
                    off = m.c.find(new_buf, f'{m.POINTS}[{i}]/{field}')
                    assert m.c.rec(new_buf, off)[3] == kind
            new['server_objects']['respawn_points'] = spawns[:24]
            assert new == old, 'non-spawn project data changed'
            tdm = f'gameplay/game_modes/v010b_{key}_level_03_day_4.options'
            assert not (out / tdm).exists(), 'TDM config was edited'
            row = next(r for r in new_rows if r['id'] == ffa_id)
            config = out / 'gameplay/game_modes' / row['cfg_mode_name']
            mode = m.tree(config.read_bytes())['mode']
            old_mode = m.tree((live / tdm).read_bytes())['mode']
            assert mode['mode'] == 8 and mode['rules'] == [93, 91, 94]
            assert mode['parameters']['kills_to_win'] == 30
            assert mode['match_making_params']['max_players'] == 10
            assert mode['match_making_params']['map_name'] == m.project(key)
            assert mode['ui']['mode_name'] == 'st_mode_08_name'
            assert mode['ui']['description'] == 'st_mode_08_description'
            for field in ('map_name', 'loading_screen_id', 'icon'):
                assert mode['ui'][field] == old_mode['ui'][field]
            assert {k: v for k, v in mode['parameters'].items() if k != 'kills_to_win'} == old_mode['parameters']
            print(f'{key}: {count} FFA spawns; TDM and other project data preserved; mode 8 map {ffa_id}')
        # An already registered map must not silently receive a second ID or row.
        try:
            m.edits(pack, out / m.DICTIONARY)
        except ValueError as e:
            assert 'already present' in str(e)
        else:
            raise AssertionError('already registered maps accepted')
        print('PASS: generated content, native types, dictionary uniqueness and unchanged TDM data')


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--pack', type=Path, required=True)
    ap.add_argument('--dictionary', type=Path, required=True)
    args = ap.parse_args()
    check(args.pack, args.dictionary)
