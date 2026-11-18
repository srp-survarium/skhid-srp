#!/usr/bin/env python3
"""Apply both source edits and check the fence's wall, its trucks, containment and timing."""
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


def from_fence(posts, x, z):
    """The distance to the posts' line."""
    def to_side(a, b):
        dx, dz = b[0] - a[0], b[1] - a[1]
        t = max(0.0, min(1.0, ((x - a[0]) * dx + (z - a[1]) * dz) / (dx * dx + dz * dz)))
        return math.hypot(x - a[0] - t * dx, z - a[1] - t * dz)
    return min(to_side(a, b) for a, b in zip(posts, posts[1:] + posts[:1]))


def check(pack, dictionary, native, editor):
    recipe = arena.HERE.parent / 'content-edit/training-camp-arena-1.json'
    assert json.loads(recipe.read_text()) == arena.build(native), 'stale fence edit'
    ffa = json.loads(m.OUT.read_text())
    with tempfile.TemporaryDirectory(prefix='training-fence-check-') as tmp:
        live, first, final = (Path(tmp) / s for s in ('live', 'ffa', 'fence'))
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
        # Free for All takes its own points only (team 2): the copy's Team Deathmatch points stay
        # where that mode has them, at the gateways and outside.
        for point in points[24:]:
            x, _, z = point['position']
            assert point['team'] == 2 and inside(x, z), ('spawn outside the fence', point['point_id'], x, z)
        for i, point in enumerate(points[24:], 24):
            assert point['invulnerability_time'] == 0 and point['invulnerability_time_ms'] == 500
            assert m.c.rec(blob, m.c.find(blob, f'{m.POINTS}[{i}]/invulnerability_time_ms'))[3] == m.c.T_INT

        placed = arena.trucks()
        posts = arena.outline()
        collision = project['collision_objects']
        before = len(old['collision_objects'])
        assert collision[:before] == old['collision_objects'], 'existing collision changed'
        wall, solid = collision[before], collision[before + 1:]
        assert (wall['cmask'], wall['cgroup']) == (4, 2)
        assert wall['lib_name'] == f'resources/{arena.MODEL}/walker_collision'
        level = old['collision_objects'][0]
        visuals = project['render_visuals']
        at = next(i for i, v in enumerate(visuals) if v['lib_name'] == arena.LEVEL_TRUCK)
        drawn = visuals[at + 1:at + 1 + len(placed)]
        assert visuals[:at + 1] + visuals[at + 1 + len(placed):] == old['render_visuals'], 'existing visuals changed'
        assert len(solid) == len(drawn) == len(placed) >= 1
        for (gateway, position, rotation), body, picture in zip(placed, solid, drawn):
            # Solid as the level is (walkers and bullets), drawn at the same place and turn.
            assert body == dict(level, lib_name=f'resources/models/{arena.TRUCK}.model/collision',
                                position=body['position'], rotation=body['rotation']), gateway
            assert picture['lib_name'] == picture['visual_name'] == arena.TRUCK and picture['scale'] == [1.0, 1.0, 1.0]
            for entry in (body, picture):
                assert all(math.isclose(a, b, abs_tol=1e-5) for a, b in zip(entry['position'], position)), gateway
                assert all(math.isclose(a, b, abs_tol=1e-5) for a, b in zip(entry['rotation'], rotation)), gateway
            # It stands in the fence's line or against it: its middle is within its own length of it.
            assert from_fence(posts, position[0], position[2]) < 4.0, (gateway, 'is not at the fence')

        walls = m.tree((final / arena.MODEL / arena.PART).read_bytes())['primitives']
        corners = arena.behind(posts)
        assert len(walls) == len(posts) == len(corners)
        ring = list(zip(walls, corners, corners[1:] + corners[:1], posts, posts[1:] + posts[:1]))
        for wall, a, b, post_a, post_b in ring:
            assert wall['type'] == 1
            x, y, z = wall['position']
            sx, sy, sz = wall['scale']
            yaw = wall['rotation'][1]
            assert y - sy <= -30 and y + sy >= 50

            def place(px, pz):
                # Native primitive inverse yaw: along the wall, and across it.
                dx, dz = px - x, pz - z
                return dx * math.cos(yaw) + dz * math.sin(yaw), -dx * math.sin(yaw) + dz * math.cos(yaw)
            # Both ends lie inside the box, with overlap, so a closed outline has no walk-through seam.
            for corner in (a, b):
                along, across = place(*corner)
                assert abs(along) < sx - 0.7 and abs(across) < sz, (a, b, wall)
            # The wall is behind its side of the fence: its inner face 0.1 m outside the posts, which
            # are inside the outline it makes.
            for post in (post_a, post_b):
                along, across = place(*post)
                assert math.isclose(abs(across), sz + 0.1, abs_tol=1e-3), (post, wall)
            assert not inside(x, z), 'a wall inside the fence'
        project['collision_objects'] = collision[:before]
        project['render_visuals'] = visuals[:at + 1] + visuals[at + 1 + len(placed):]
        project['server_objects']['respawn_points'] = points[:24]
        assert project == old, 'unrelated project content changed'
        assert len(m.c.value_offsets(blob)) == len(set(m.c.value_offsets(blob)))
    print(f'PASS: {count} FFA spawns inside the fence, 24 TDM spawns unchanged; closed {len(walls)}-wall walker '
          f'boundary behind the fence; {len(placed)} trucks drawn and solid; 500 ms native integer override')


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--pack', type=Path, required=True)
    ap.add_argument('--dictionary', type=Path, required=True)
    ap.add_argument('--native', type=Path, required=True)
    ap.add_argument('--editor', type=Path, default=m.EDIT)
    a = ap.parse_args()
    check(a.pack, a.dictionary, a.native, a.editor)
