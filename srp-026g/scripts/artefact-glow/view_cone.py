#!/usr/bin/env python3
"""view_cone.py: how close each emitter of an artefact glow comes to the middle of the
first-person view (../../docs/notes/artefacts.md, "Your own glow").

usage: python3 -I view_cone.py <particles/fx/artifacts/human_<kind>.particle> [--twin]

For every emitter: its body parts (the animated domain's), its reach (the largest particle size,
the domain's scale over a 5 cm part, and the drift: the initial velocity times the lifetime, plus
10 cm for turbulence), and, for two nominal first-person poses (the camera at the eyes, looking
ahead; the rifle at the hip and aimed), the least angle between the view's axis and any point the
particles reach. An emitter whose particles come within CENTRE degrees of the axis would cross the
middle of the view.

The poses are nominal (a 1.8 m player holding a rifle, positions in metres from the eyes: x right,
y up, z ahead), not the game's animations: the measure says which body parts are near the middle
of the view, not exact angles.
"""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_pack as mp  # noqa: E402

CENTRE = 15.0          # degrees from the view's axis: the middle of a 65 degree view
PART = 0.05            # a body part's own half-size, m
TURBULENCE = 0.10      # m a particle may wander on top of its velocity
POSES = {
    'hip': {
        'Head': (0, 0, 0), 'Face': (0, -0.05, 0.08), 'Neck': (0, -0.15, -0.02),
        'FrontChest': (0, -0.35, 0.12), 'BackChest': (0, -0.35, -0.12),
        'FrontSpin': (0, -0.65, 0.12), 'BackSpin': (0, -0.65, -0.12),
        'LeftArm': (-0.2, -0.3, 0.05), 'RightArm': (0.2, -0.3, 0.05),
        'LeftForeArm': (-0.15, -0.38, 0.3), 'RightForeArm': (0.15, -0.4, 0.2),
        'LeftHand': (-0.08, -0.35, 0.48), 'RightHand': (0.12, -0.38, 0.3),
        'LeftUpLeg': (-0.1, -0.85, 0.05), 'RightUpLeg': (0.1, -0.85, 0.05),
        'LeftLeg': (-0.1, -1.2, 0.05), 'RightLeg': (0.1, -1.2, 0.05),
        'LeftFoot': (-0.1, -1.6, 0.1), 'RightFoot': (0.1, -1.6, 0.1),
    },
}
aimed = dict(POSES['hip'])
aimed.update({'LeftForeArm': (-0.1, -0.2, 0.3), 'RightForeArm': (0.12, -0.18, 0.12),
              'LeftHand': (-0.03, -0.14, 0.42), 'RightHand': (0.06, -0.12, 0.2),
              'LeftArm': (-0.2, -0.22, 0.05), 'RightArm': (0.2, -0.2, 0.0)})
POSES['aimed'] = aimed


def ys(node):
    """Every curve key value (the y of a position) under a property."""
    t, v = node
    if t == mp.T_NAMED:
        out = [c[1][1] for k, c in v if k == 'position' and c[0] == 6]
        for k, c in v:
            out += ys(c)
        return out
    if t == mp.T_INDEXED:
        return [y for c in v for y in ys(c)]
    return []


def emitters(tree):
    lod = dict(tree[1])['LOD 0']
    for guid, e in dict(lod[1])['children'][1]:
        name = mp.get(e, 'properties/Name')[1]
        props = dict(mp.get(e, 'properties')[1])
        life = max(ys(props['LifeTime']) or [1.0])
        parts, size, velocity, scale = [], 0.0, 0.0, 1.0
        for _, a in dict(e[1])['children'][1]:
            kind = mp.get(a, 'particle_entity_type')[1]
            p = dict(mp.get(a, 'properties')[1])
            if kind == mp.DOMAIN:
                parts = [k for k, c in p.items() if c == (mp.T_BOOL, 1) and k in POSES['hip']]
                if 'Scale' in p:
                    scale = max([abs(y) for y in ys(p['Scale'])] or [1.0])
            if 'InitSize' in p:
                size = max([abs(y) for y in ys(p['InitSize'])] or [0.0])
            if 'InitVelocity' in p:
                velocity = max([abs(y) for y in ys(p['InitVelocity'])] or [0.0])
        reach = size / 2 + PART * max(scale, 1.0) + velocity * life + TURBULENCE
        yield name, props.get('Material', (0, ''))[1], parts, reach


def least_angle(position, reach):
    x, y, z = position
    dist = math.sqrt(x * x + y * y + z * z)
    if dist <= reach:
        return 0.0
    axis = math.degrees(math.atan2(math.hypot(x, y), z)) if z > 0 else 90.0 + math.degrees(math.atan2(-z, math.hypot(x, y)))
    return max(0.0, axis - math.degrees(math.asin(reach / dist)))


def main():
    path = Path(sys.argv[1])
    tree = mp.parse(path.read_bytes())
    for name, material, parts, reach in emitters(tree):
        worst = {}
        for pose, where in POSES.items():
            angles = [(least_angle(where[p], reach), p) for p in parts]
            worst[pose] = min(angles) if angles else (180.0, '-')
        flag = 'CENTRE' if any(a < CENTRE for a, _ in worst.values()) else 'clear'
        print(f'{name:12} {flag:6} reach {reach:4.2f} m  hip {worst["hip"][0]:5.1f} ({worst["hip"][1]})  '
              f'aimed {worst["aimed"][0]:5.1f} ({worst["aimed"][1]})  {material}  {",".join(parts)}')


if __name__ == '__main__':
    main()
