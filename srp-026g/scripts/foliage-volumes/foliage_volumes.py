#!/usr/bin/env python3
"""The bushes a sight line can cross that the bots' sight and the bullets do not see: `bot-foliage.json`.

    foliage_volumes.py SRP_GAME_DATA RESOURCES_DB OUT.json [PROJECT ...]

`SRP_GAME_DATA` is the `srp-game-data` binary (`cargo build -p resources-db --bin srp-game-data`); PROJECT is
`level_01/day_2` and so on (default: every level project the database has). A bush is a `render_visuals` placement of the project whose
name says bush (`bush`, `kust`, `kruch`, `shrub`) and whose library has no collision object in the same project: it is
drawn and nothing more (the bots' sight is the bullet mesh), so a human cannot see through it and a bot does. Its volume
is the box round the model's `hq_collision` hull (its own simplified mesh; the LOD0 render mesh where it has none), through the placement's scale and
rotation. A bush that has a collision object is in the sight tree already and is left out.

OUT.json (`bot-foliage.json`, read from the mapdata folder by the match server): {"version": 1, "projects": {"level_01/day_2": {"bushes": [{"lib", "p": [x, y, z] centre, "h": [x, y, z] half
extents along the model's axes after the scale, "r": [x, y, z] the placement's angles (mapdata::math::Rotation::from_angles)}]}}}.
Run with `python -I`."""
import json
import re
import struct
import subprocess
import sys
import tempfile
from math import cos, sin
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'gen-data-026g'))
import bincfg  # noqa: E402

BUSH = re.compile(r'bush|kust|kruch|shrub', re.I)


def rotation(a):
    """mapdata::math::Rotation::from_angles: rows i, j, k."""
    xs, xc = sin(a[0]), cos(a[0])
    ys, yc = sin(a[1]), cos(a[1])
    zs, zc = sin(a[2]), cos(a[2])
    return [(yc * zc, -yc * zs, ys),
            (ys * xs * zc + xc * zs, -xs * ys * zs + xc * zc, -xs * yc),
            (-xc * ys * zc + xs * zs, xc * ys * zs + xs * zc, xc * yc)]


def apply(r, v):
    return tuple(r[0][k] * v[0] + r[1][k] * v[1] + r[2][k] * v[2] for k in range(3))


FILES = {}  # db -> the archive paths it holds


def files_of(tool, db):
    if db not in FILES:
        listing = subprocess.run([tool, 'files', '--db', db], check=True, capture_output=True, text=True).stdout
        FILES[db] = {line.split('	', 1)[0] for line in listing.splitlines()}
    return FILES[db]


def get(tool, db, paths):
    """The archive files `paths` that exist, as {path: bytes}."""
    want = [p for p in paths if p in files_of(tool, db)]
    out = {}
    if want:
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'out'
            subprocess.run([tool, 'get', '--db', db, '--out', str(dest), *want], check=True, capture_output=True)
            for p in want:
                out[p] = (dest / p).read_bytes()
    return out


def hull_box(vertices):
    """(min, max) of a `hq_collision/vertices` file: u32 type, u32 byte count, then xyz floats."""
    n = struct.unpack_from('<I', vertices, 4)[0] // 12
    pts = [struct.unpack_from('<3f', vertices, 8 + 12 * i) for i in range(n)]
    return tuple(min(p[k] for p in pts) for k in range(3)), tuple(max(p[k] for p in pts) for k in range(3))


def render_box(tool, db, lib):
    """(min, max) of the model's LOD0 render vertices (chunk 3 of each surface's `converted_model`: u32 count, then
    vertices of a stride whose first 12 bytes are xyz floats), for a model with no `hq_collision` hull."""
    settings = get(tool, db, [f'models/{lib}.model/settings'])
    if not settings:
        return None
    cfg = bincfg.parse(next(iter(settings.values())))
    surfaces = cfg.get('lod_hierrarchy', {}).get('LOD0', {}).get('surfaces', [])
    pts = []
    for data in get(tool, db, [f'models/{lib}.model/render/{s}/converted_model' for s in surfaces]).values():
        chunk, at = None, 0
        while at < len(data):
            kind, size = struct.unpack_from('<II', data, at)
            if kind == 3:
                chunk = data[at + 8:at + 8 + size]
            at += 8 + size
        if chunk is None:
            continue
        count = struct.unpack_from('<I', chunk)[0]
        stride = (len(chunk) - 4) // count if count else 0
        pts += [struct.unpack_from('<3f', chunk, 4 + stride * i) for i in range(count)]
    if not pts:
        return None
    return tuple(min(p[k] for p in pts) for k in range(3)), tuple(max(p[k] for p in pts) for k in range(3))


def main():
    tool, db, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    projects = sys.argv[4:] or sorted(
        m.group(1) for m in (re.fullmatch(r'projects/(level_\d+/[^/]+)/client_project', p) for p in files_of(tool, db)) if m)
    boxes = {}  # library -> (min, max) of the hull, or None
    result = {}
    for project in projects:
        raw = get(tool, db, [f'projects/{project}/client_project'])
        if not raw:
            sys.exit(f'{project}: no client_project in {db}')
        proj = bincfg.parse(raw[f'projects/{project}/client_project'])
        solid = {re.sub(r'#.*', '', o['lib_name']).replace('resources/models/', '').replace('.model/collision', '')
                 .replace('.model/walker_collision', '') for o in proj.get('collision_objects', [])}
        bushes, no_hull, solid_n = [], {}, 0
        for v in proj.get('render_visuals', []):
            lib = v['lib_name']
            if not BUSH.search(lib):
                continue
            if lib in solid:
                solid_n += 1
                continue
            if lib not in boxes:
                files = get(tool, db, [f'models/{lib}.model/hq_collision/vertices'])
                boxes[lib] = hull_box(next(iter(files.values()))) if files else render_box(tool, db, lib)
            box = boxes[lib]
            if box is None:
                no_hull[lib] = no_hull.get(lib, 0) + 1
                continue
            lo, hi = box
            scale = v.get('scale', (1.0, 1.0, 1.0))
            centre = tuple((lo[k] + hi[k]) / 2 * scale[k] for k in range(3))
            half = tuple(abs((hi[k] - lo[k]) / 2 * scale[k]) for k in range(3))
            ang = v['rotation']
            at = apply(rotation(ang), centre)
            pos = v['position']
            bushes.append({'lib': lib, 'p': [round(pos[k] + at[k], 3) for k in range(3)],
                           'h': [round(h, 3) for h in half], 'r': [round(a, 4) for a in ang]})
        result[project] = {'bushes': bushes}
        print(f'{project}: {len(bushes)} drawn-only bush volumes, {solid_n} with a collision object (left out), '
              f'{sum(no_hull.values())} with no hull {no_hull or ""}', file=sys.stderr)
    out.write_text(json.dumps({'version': 1, 'projects': result}, separators=(',', ':')) + '\n')


if __name__ == '__main__':
    main()
