#!/usr/bin/env python3
"""Build lab-capsule-glass@1 from Vector Laboratory's glass capsules, for release/content.tsv.

--stock contains the models' files at their archive paths (srp-game-data get
with the stock db and every listed patch; every path of stock.json). --out is
a new resource-porter pack directory. Never builds a release patch.

Vector's four capsules (props/chemicalflaskcentre/lab_flask, 3.7 m tall, on
its lower floor at y 13.2; the owner, 2026-11-27) are glass tubes of two
halves: the capsule's front, its surface capsuleglass_s (a cylinder's
segment of radius 0.70 m about its axis, 0.61 to 3.07 m up: 2.7 m²), and its
door, a model of its own beside it (props/chemicalflaskcentre/lab_flaskdoor,
the same glass facing the other way; the owner: "only one wall, even if I
shoot another"). A shot breaks a half (lab-capsule-glass-1.json, a fixture
of the map's lamp table each, docs/notes/breakable_lamps.md). For each:

- srp_<model>_broken: the model without its glass (every LOD's glass
  surface out of its settings, the frame's surfaces as they are);
- srp_<model>_glass<k>: SHARDS pieces of the glass, cut as the bottle's
  (scripts/bottle-shards: two-sided, each about its centre) but to 20 cm
  triangles, jagged by up to 6 cm, its seeds spread over the pane
  (far_seeds): some 40 cm a piece, the game's own glass bursts the fine
  ones. `shard_info` gives each one's centre, its glass's outward normal
  there (from the cylinder's axis, fitted to the glass) and its long side
  (its points' principal axis): the flying body lies along the shard and
  drops out of the frame (`ragdoll::LoneBody::pane_shattered`).

Collision is disabled in all: the map's capsule and door keep their own
(their glass's stays where the glass was, as a lamp's does).
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ware = load('kitchen_ware', 'kitchen-ware/make_pack.py')
bottle = ware.bottle
parse = ware.parse

GLASS = 'capsuleglass_s'
SHARDS = 16
STEP = 0.2
JITTER = 0.06


class Glass:
    """A model of glass that breaks: its broken copy and shards' names."""

    def __init__(self, model):
        self.model = model
        folder, name = model.rsplit('/', 1)
        self.broken = f'{folder}/srp_{name}_broken'
        self.shard_names = [f'{folder}/srp_{name}_glass{k}' for k in range(SHARDS)]


CAPSULE = Glass('props/chemicalflaskcentre/lab_flask')
DOOR = Glass('props/chemicalflaskcentre/lab_flaskdoor')
GLASSES = (CAPSULE, DOOR)


def cut(data):
    first = max(ware.points_of(data), key=lambda p: p[1])
    return bottle.Cut(bottle.far_seeds(data, SHARDS, STEP, first), step=STEP, jitter=JITTER)


def glass_of(originals):
    return originals[f'render/{GLASS}/converted_model']


def axis_of(points):
    """The glass cylinder's axis (x, z) in the model's frame: the circle
    fitted to its points across (least squares, Kasa's)."""
    n = len(points)
    mx, mz = sum(p[0] for p in points) / n, sum(p[2] for p in points) / n
    suu = svv = suv = suuu = svvv = suvv = svuu = 0.0
    for x, _, z in points:
        u, v = x - mx, z - mz
        suu += u * u
        svv += v * v
        suv += u * v
        suuu += u * u * u
        svvv += v * v * v
        suvv += u * v * v
        svuu += v * u * u
    det = suu * svv - suv * suv
    assert abs(det) > 1e-9, 'no circle'
    bu, bv = (suuu + suvv) / 2, (svvv + svuu) / 2
    return mx + (bu * svv - bv * suv) / det, mz + (bv * suu - bu * suv) / det


def principal(points):
    """The unit direction of the points' greatest spread (about their mean)."""
    n = len(points)
    mean = [sum(p[a] for p in points) / n for a in range(3)]
    c = [[sum((p[i] - mean[i]) * (p[j] - mean[j]) for p in points) for j in range(3)] for i in range(3)]
    v = [1.0, 1.0, 1.0]
    for _ in range(100):
        w = [sum(c[i][j] * v[j] for j in range(3)) for i in range(3)]
        length = math.sqrt(sum(x * x for x in w))
        assert length > 0, 'no spread'
        v = [x / length for x in w]
    return v


def shard_info(stock, hashes, glass):
    """Each shard's (centre, out, along) in its model's frame: where
    lab-capsule-glass-1.json places it, the glass's level outward normal
    there and its long side, rounded as the rows keep them."""
    data = glass_of(ware.originals_of(stock, glass.model, hashes))
    ax, az = axis_of(ware.points_of(data))
    out = []
    for _, points, _, centre in bottle.shards(data, cut(data)):
        dx, dz = centre[0] - ax, centre[2] - az
        length = math.hypot(dx, dz)
        assert length > 0.3, 'a shard on the axis'
        normal = [round(dx / length, 4), 0.0, round(dz / length, 4)]
        out.append((centre, normal, [round(a, 4) for a in principal(points)]))
    return out


def glass_bounds(stock, hashes, glass):
    """The glass's bounds in its model's frame: the fixture's (what a shot must reach)."""
    return bottle.mesh_tools.bounds(ware.points_of(glass_of(ware.originals_of(stock, glass.model, hashes))))


def broken(originals, target):
    settings = parse(bytearray(originals['settings']))
    frame = [s for s in ware.lamp_pack.surfaces(settings) if not s.endswith(GLASS)]
    files = {}
    points = []
    for name in frame:
        for part in ('converted_model', 'export_properties'):
            files[target + f'render/{name}/{part}'] = originals[f'render/{name}/{part}']
        if name in ware.lod0(settings):
            points += ware.points_of(originals[f'render/{name}/converted_model'])
    files[target + 'render/export_properties'] = ware.root_props(originals['render/export_properties'], points)
    files[target + 'settings'] = ware.settings_of(originals['settings'], keep=set(frame))
    return files


def build(stock, hashes):
    files = {}
    for glass in GLASSES:
        originals = ware.originals_of(stock, glass.model, hashes)
        files |= broken(originals, ware.model_dir(glass.broken))
        files |= ware.shard_models(originals, GLASS, [ware.model_dir(t) for t in glass.shard_names],
                                   cut(glass_of(originals)))
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out already exists')
    hashes = json.loads(Path(__file__).with_name('stock.json').read_text())
    files = build(args.stock, hashes)
    pack = {'format_version': 1, 'id': 'lab-capsule-glass', 'version': '1', 'target_build': '0.26g',
            'description': "Vector Laboratory's glass capsule and its door without their glass, and each "
                           "one's glass as 16 shards",
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['Generated from stock 0.26g by scripts/lab-capsule/make_pack.py; '
                      "Vector's placement is lab-capsule-glass-1.json."]}
    for path, data in files.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    data = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
