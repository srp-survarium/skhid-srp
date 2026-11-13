#!/usr/bin/env python3
"""Apply both source edits and check crater containment, native wall geometry and timing."""
import argparse
import json
import math
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import make_ffa_edits as m
import make_training_arena as arena


def inside(x, z):
    return inside_of(arena.outline(), x, z)


def inside_of(points, x, z):
    crossed = False
    for a, b in zip(points, points[1:] + points[:1]):
        if (a[1] > z) != (b[1] > z) and x < a[0] + (z - a[1]) * (b[0] - a[0]) / (b[1] - a[1]):
            crossed = not crossed
    return crossed


def check(pack, dictionary, native, editor):
    recipe = arena.HERE.parent / 'content-edit/training-camp-arena-1.json'
    assert json.loads(recipe.read_text()) == arena.build(native), 'stale crater edit'
    ffa = json.loads(m.OUT.read_text())
    with tempfile.TemporaryDirectory(prefix='training-crater-check-') as tmp:
        live, first, final = (Path(tmp) / s for s in ('live', 'ffa', 'crater'))
        paths = {m.DICTIONARY} | {e['from'] for e in ffa['copy']} | {e['files'] for e in ffa['add_entry']}
        for path in paths:
            src = dictionary if path == m.DICTIONARY else m.in_pack(pack, path)
            dst = live / path
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dst)
        for spec, source, out in [(m.OUT, live, first), (recipe, first, final)]:
            if spec == recipe:
                shutil.copytree(native, first, dirs_exist_ok=True)
            run = subprocess.run([sys.executable, str(editor), str(spec), str(source), str(out)],
                                 capture_output=True, text=True)
            assert run.returncode == 0, run.stdout + run.stderr
            assert '0 unexpected differences' in run.stdout, run.stdout
        blob = (final / arena.PROJECT).read_bytes()
        project = m.tree(blob)
        old = m.tree((pack / 'files' / arena.TDM_PROJECT).read_bytes())
        points = project['server_objects']['respawn_points']
        count = m.COUNTS['cs_training_camp']
        assert len(points) == 24 + count and points[:24] == old['server_objects']['respawn_points']
        for point in points:
            x, _, z = point['position']
            assert inside(x, z), ('spawn outside crater', point['team'], point['point_id'], x, z)
        for i, point in enumerate(points[24:], 24):
            assert point['invulnerability_time'] == 0 and point['invulnerability_time_ms'] == 500
            assert m.c.rec(blob, m.c.find(blob, f'{m.POINTS}[{i}]/invulnerability_time_ms'))[3] == m.c.T_INT
        collision = project['collision_objects']
        assert collision[:-1] == old['collision_objects'], 'existing collision changed or border missing'
        assert collision[-1]['cmask'] == 4 and collision[-1]['cgroup'] == 2
        assert collision[-1]['lib_name'] == f'resources/{arena.MODEL}/walker_collision'
        walls = m.tree((final / arena.MODEL / arena.PART).read_bytes())['primitives']
        outline = arena.outline()
        assert len(walls) == len(outline)
        for wall, a, b in zip(walls, outline, outline[1:] + outline[:1]):
            assert wall['type'] == 1
            x, y, z = wall['position']
            sx, sy, sz = wall['scale']
            yaw = wall['rotation'][1]
            assert y - sy <= -30 and y + sy >= 50
            # Native primitive inverse yaw. Both ends lie inside the box,
            # with overlap, so a closed outline has no walk-through seam.
            for px, pz in (a, b):
                dx, dz = px - x, pz - z
                along = dx * math.cos(yaw) + dz * math.sin(yaw)
                across = -dx * math.sin(yaw) + dz * math.cos(yaw)
                assert abs(along) < sx - 0.7 and abs(across) < sz, (a, b, wall)
        project['collision_objects'] = collision[:-1]
        project['server_objects']['respawn_points'] = points[:24]
        assert project == old, 'unrelated project content changed'
        assert len(m.c.value_offsets(blob)) == len(set(m.c.value_offsets(blob)))
    print(f'PASS: {count} FFA + 24 unchanged TDM spawns inside crater; closed 24-wall walker boundary; '
          '500 ms native integer override')


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--pack', type=Path, required=True)
    ap.add_argument('--dictionary', type=Path, required=True)
    ap.add_argument('--native', type=Path, required=True)
    ap.add_argument('--editor', type=Path, default=m.EDIT)
    a = ap.parse_args()
    check(a.pack, a.dictionary, a.native, a.editor)
