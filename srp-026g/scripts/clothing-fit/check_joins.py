#!/usr/bin/env python3
"""Check the joins a piece leaves to the other armour: no see-through hole at the neck or the ankle.

Two rules (docs/notes/exo-joins.md), each over every pair the game can draw:

- neck: a helmet or mask that hides the head (`data.remove_head_model`) takes the native neck with
  it, the neck every collar is made around. With each torso, its surfaces (`data` and its
  additional models) and the torso's must close the native neck's band: no horizontal ray from the
  native neck's axis, from the native neck's bottom to NECK_BAND above it, every NECK_STEP
  degrees, passes through them all.
- shins: a torso that covers the trousers' slot (`data.additional_model_slots` has `pants`) brings
  its own legs. With each pair of boots, its surfaces and the boots' must close both shins from
  SHIN_LOW to SHIN_HIGH: no horizontal ray from a shin's axis (the bind pose's Leg to Foot) passes
  through them all.

A ray that hits nothing is a hole seen from outside along it (one-sided surfaces: what is behind
the hole is the inside of the far side, which the game does not draw). It does not find a piece
poking through another, nor gaps that open only in an animated pose; render the pairs as well
(render_pairs.py). --native and --tree are extracted resource trees, in overlay order.
"""
import argparse
import json
from pathlib import Path
import sys

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'exo-joins'))
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import joins  # noqa: E402
from headwear import bind_records  # noqa: E402
from check_boot_classes import Resources, Trees, bcw  # noqa: E402

ARMOUR = 'gameplay/items/armour'
BODYPARTS = 'gameplay/bodyparts/default'
NECK_BAND, NECK_STEP, NECK_DY = 0.10, 10, 0.01
SHIN_LOW, SHIN_HIGH, SHIN_STEP, SHIN_DY = 0.10, 0.32, 10, 0.01
# Render bind order: each shin bone and its foot.
SHINS = ((15, 16), (10, 11))


def configs(view, slot):
    return sorted(p for p in view.paths() if p.startswith(f'{ARMOUR}/{slot}/') and p.endswith('.options'))


def model_root(part):
    return f"models/{part['base_model']}.skinned_model/render"


def parts(data):
    return [data, *data.get('additional_models', [])]


class Pieces:
    """Configs and their surfaces' triangles, read once."""

    def __init__(self, view):
        self.view = view
        self.meshes = {}

    def data(self, path):
        return bcw.loads(self.view.read(path))['data']

    def mesh(self, part):
        key = f"{model_root(part)}/{part['part_name']}/converted_model"
        if key not in self.meshes:
            self.meshes[key] = joins.read_mesh(self.view.read(key))
        return self.meshes[key]

    def tris(self, data):
        return [self.mesh(part).tris for part in parts(data)]


def hit_any(tris_list, origins, dirs):
    hit = np.zeros(len(origins), bool)
    for tris in tris_list:
        near, _ = joins.ray_hits(tris, origins, dirs)
        hit |= ~np.isnan(near)
    return hit


def native_neck(pieces):
    head = bcw.loads(pieces.view.read(BODYPARTS))['body_parts']['head']['data']
    mesh = pieces.mesh(head)
    neck = mesh.weight(joins.NECK) > 0.3
    center = mesh.pos[neck][:, [0, 2]].mean(0)
    return center, mesh.pos[neck, 1].min()


def neck_rows(pieces):
    center, bottom = native_neck(pieces)
    origins, dirs, heights, _ = joins.ring_rays(center, np.arange(bottom, bottom + NECK_BAND, NECK_DY),
                                                np.arange(0, 360, NECK_STEP))
    torsos = {p: hit_any(pieces.tris(pieces.data(p)), origins, dirs) for p in configs(pieces.view, 'torso')}
    rows = []
    for slot in ('helmet', 'mask'):
        for path in configs(pieces.view, slot):
            data = pieces.data(path)
            if not data.get('remove_head_model'):
                continue
            own = hit_any(pieces.tris(data), origins, dirs)
            for torso, hit in torsos.items():
                holes = ~(own | hit)
                rows.append({'rule': 'neck', 'piece': path, 'with': torso, 'holes': int(holes.sum()),
                             'heights_m': [round(float(heights[holes].min()), 3),
                                           round(float(heights[holes].max()), 3)] if holes.any() else None})
    return rows


def shin_rays(bind):
    frames = {i: np.frombuffer(frame, '<f4', 3).astype(float) for i, (_, frame) in enumerate(bind_records(bind))}
    origins, dirs, heights = [], [], []
    for leg, foot in SHINS:
        top, bottom = frames[leg], frames[foot]
        for y in np.arange(SHIN_LOW, SHIN_HIGH, SHIN_DY):
            at = top + (top[1] - y) / (top[1] - bottom[1]) * (bottom - top)
            o, d, h, _ = joins.ring_rays(at[[0, 2]], [y], np.arange(0, 360, SHIN_STEP))
            origins.append(o)
            dirs.append(d)
            heights.append(h)
    return np.concatenate(origins), np.concatenate(dirs), np.concatenate(heights)


def shin_rows(pieces):
    rows = []
    bodies = [p for p in configs(pieces.view, 'torso') if 'pants' in pieces.data(p).get('additional_model_slots', [])]
    if not bodies:
        return rows
    first = pieces.data(bodies[0])
    origins, dirs, heights = shin_rays(pieces.view.read(f'{model_root(first)}/bind_pose'))
    boots = {p: hit_any(pieces.tris(pieces.data(p)), origins, dirs) for p in configs(pieces.view, 'boots')}
    for body in bodies:
        own = hit_any(pieces.tris(pieces.data(body)), origins, dirs)
        for boot, hit in boots.items():
            holes = ~(own | hit)
            rows.append({'rule': 'shins', 'piece': body, 'with': boot, 'holes': int(holes.sum()),
                         'heights_m': [round(float(heights[holes].min()), 3),
                                       round(float(heights[holes].max()), 3)] if holes.any() else None})
    return rows


def audit(view):
    pieces = Pieces(view)
    return neck_rows(pieces) + shin_rows(pieces)


class TreeView(Trees):
    def paths(self):
        return sorted({str(p.relative_to(t)) for t in self.trees for p in t.rglob('*') if p.is_file()})


class FileView(Resources):
    def paths(self):
        return sorted(self.files)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--tree', type=Path, action='append', default=[])
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    rows = audit(TreeView([args.native, *args.tree]))
    args.out.write_text(json.dumps(rows, indent=2) + '\n')
    failures = [r for r in rows if r['holes']]
    for row in failures:
        print(f"{row['rule']}: {row['piece']} with {row['with']}: {row['holes']} rays through, "
              f"{row['heights_m'][0]:.3f}-{row['heights_m'][1]:.3f} m")
    print(f'{len(rows)} pairs checked; {len(failures)} with a hole')
    return bool(failures)


if __name__ == '__main__':
    sys.exit(main())
