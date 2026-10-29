#!/usr/bin/env python3
"""Inspect the authored native clips on the stock rig, without launching a game.

Writes front/side joint projections, not a claim about in-game rendering.
The green circle is the intended RightHand attachment, not an artefact mesh.
"""
import argparse
import json
import math
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as content


def multiply(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def vector(v, m):
    return [sum(v[k] * m[k][j] for k in range(3)) for j in range(3)]


def rotation(a):
    sx, sy, sz = map(math.sin, a)
    cx, cy, cz = map(math.cos, a)
    return [[cy*cz, -cy*sz, sy],
            [sy*sx*cz+cx*sz, -sx*sy*sz+cx*cz, -sx*cy],
            [-cx*sy*cz+sx*sz, cx*sy*sz+sx*cz, cx*cy]]


def pose(clip, names, parents, t):
    data = content.clip_parse(clip)[2]
    offset, world = 0, {}
    for name in names:
        values = []
        for _ in range(9):
            count = struct.unpack_from('<I', data, offset)[0]
            pairs = [struct.unpack_from('<ff', data, offset + 4 + i*8) for i in range(count)]
            offset += 4 + count*8
            if count != 5 or [k for k, _ in pairs[:4]] != [0]*4:
                raise ValueError('preview expects the authored clamped cubic tracks')
            a, b, c, d = [v for _, v in pairs[:4]]
            values.append((1-t)**3*a + 3*(1-t)**2*t*b + 3*(1-t)*t*t*c + t**3*d)
        r, position = rotation(values[3:6]), values[:3]
        if parents[name] is not None:
            pr, pp = world[parents[name]]
            r, position = multiply(r, pr), [a+b for a, b in zip(vector(position, pr), pp)]
        world[name] = r, position
    return world


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('clip', 'names', 'skeleton', 'out'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    edits = json.loads((HERE / 'pose.json').read_text())['make_pose_clip']
    source, rig = args.clip.read_bytes(), args.names.read_bytes()
    names = [content.name_at(rig, content.rec(rig, at)[0]).decode()
             for at in content.children(rig, content.find(rig, 'bones_names'))]
    parents = {}
    for path, kind, _ in content.walk(args.skeleton.read_bytes()):
        if path and kind == content.T_NAMED:
            parts = path.split('/')[1:]
            parents[parts[-1]] = parts[-2] if len(parts) > 1 else None
    clips = [content.make_pose_clip(source, rig, spec) for spec in edits]
    # Evaluate each seam from independently generated files.
    assert pose(clips[0], names, parents, 1) == pose(clips[1], names, parents, 0)
    assert pose(clips[1], names, parents, 1) == pose(clips[2], names, parents, 0)
    assert pose(clips[2], names, parents, 1) == pose(clips[0], names, parents, 0)
    labels = ['Lowered', 'Raising · 0.3 s', 'Holding · 0.6 s']
    samples = [pose(clips[0], names, parents, t) for t in (0, .5, 1)]
    out = ['<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="740" viewBox="0 0 1200 740">',
           '<rect width="1200" height="740" fill="#111820"/>',
           '<style>text{fill:#dbe8ef;font-family:sans-serif}line{stroke-linecap:round}</style>',
           '<text x="30" y="35" font-size="22">Artefact carry · authored rig preview</text>',
           '<text x="30" y="61" font-size="14">Front and side joint projections. Green: planned palm attachment. Not an in-game screenshot.</text>']
    for row, axis in enumerate((0, 2)):
        for column, world in enumerate(samples):
            cx, cy, zoom = 190 + 400*column, 365 + 330*row, 165
            project = lambda p: (cx + p[axis]*zoom, cy - p[1]*zoom)
            out.append(f'<text x="{column*400+30}" y="{95+330*row}" font-size="16">{labels[column]} · {"front" if row == 0 else "side"}</text>')
            for name in names:
                parent = parents[name]
                if parent is None or name in ('Root', 'Weapon', 'Item', 'Pistol'):
                    continue
                a, b = project(world[parent][1]), project(world[name][1])
                color = '#f0c798' if name.startswith('Right') and 'Leg' not in name and 'Foot' not in name and 'Toe' not in name and 'Knee' not in name else '#8195a3'
                out.append(f'<line x1="{a[0]}" y1="{a[1]}" x2="{b[0]}" y2="{b[1]}" stroke="{color}" stroke-width="3"/>')
            r, p = world['RightHand']
            attachment = [a+b for a, b in zip(vector([-.07, .11, 0], r), p)]
            x, y = project(attachment)
            out.append(f'<circle cx="{x}" cy="{y}" r="8" fill="#70d6a0" stroke="#b8ffda"/>')
    out.append('</svg>')
    args.out.write_text('\n'.join(out) + '\n')
    print('Native clip seams agree; right wrist rises from', samples[0]['RightHand'][1], 'to', samples[-1]['RightHand'][1])
    print(args.out)


if __name__ == '__main__':
    main()
