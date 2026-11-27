#!/usr/bin/env python3
"""Compare two native AK-12 receiver bakes of the default Snufkin placement."""
import argparse
import struct
from pathlib import Path

import numpy as np
from PIL import Image


def sample_surface(mesh, points, seed, dual_uv=False):
    data = mesh.read_bytes()
    kind, size, count = struct.unpack_from('<III', data)
    if kind != 3 or count == 0 or (size - 4) % count:
        raise ValueError(f'{mesh}: invalid vertex chunk')
    stride = (size - 4) // count
    vertices = np.ndarray((count, 3), '<f4', data, 12, (stride, 4))
    uvs = np.ndarray((count, 2), '<f4', data, 12 + stride - 8, (stride, 4))
    kind, index_size, indices = struct.unpack_from('<III', data, 8 + size)
    if kind != 4 or indices % 3 or index_size != indices * 2 + 4:
        raise ValueError(f'{mesh}: invalid index chunk')
    faces = np.frombuffer(data, '<u2', indices, 20 + size).reshape(-1, 3)
    a, b, c = (vertices[faces[:, i]] for i in range(3))
    areas = np.linalg.norm(np.cross(b - a, c - a), axis=1)
    rng = np.random.default_rng(seed)
    triangle = faces[rng.choice(len(faces), points, p=areas / areas.sum())]
    pair = rng.random((points, 2))
    reflect = pair.sum(axis=1) > 1
    pair[reflect] = 1 - pair[reflect]
    weights = np.column_stack((1 - pair.sum(axis=1), pair))
    bake_uvs = uvs
    if dual_uv:
        if stride != 36:
            raise ValueError(f'{mesh}: expected one-weight vertices')
        raw = np.ndarray((count, stride), 'u1', data, 12, (stride, 1))
        if not ((raw[:, 19] == ord('S')) & (raw[:, 27] == ord('D'))).all():
            raise ValueError(f'{mesh}: missing decal UV tag')
        def coord(low, high):
            return (raw[:, low].astype(np.uint32) + 256 * raw[:, high].astype(np.uint32)) / 65535
        bake_uvs = np.column_stack((coord(13, 14), coord(15, 23)))
    return tuple(np.einsum('ij,ijk->ik', weights, field[triangle]) for field in (vertices, uvs, bake_uvs))


def diffuse(path, uv):
    with Image.open(path) as image:
        rgb = np.asarray(image.convert('RGB'))
    x, y = ((uv % 1) * [rgb.shape[1], rgb.shape[0]]).astype(int).T
    return rgb[y, x].astype(np.int16)


def measure(mesh, base_diffuse, cache, dual_uv=False):
    xyz, source_uv, uv = sample_surface(mesh, points=400_000, seed=42, dual_uv=dual_uv)
    base = diffuse(base_diffuse, source_uv)
    # The game's surface indices can change when a pack changes a model. Match
    # the receiver to its source material independently in each isolated cache.
    scores = []
    for path in sorted(cache.glob('srpw*d.dds')):
        baked = diffuse(path, uv)
        scores.append((np.abs(baked - base).mean(axis=1).mean(), path))
    if not scores:
        raise ValueError(f'{cache}: no native weapon diffuse bakes')
    scores.sort()
    if len(scores) > 1 and scores[1][0] - scores[0][0] < 5:
        raise ValueError(f'{cache}: ambiguous receiver diffuse')
    baked = diffuse(scores[0][1], uv)
    # Snufkin is a yellow decal on a dark receiver. Count high-contrast paint
    # relative to that layout's own undecorated diffuse. Units here are metres.
    paint = (baked[:, 0] - baked[:, 2] > 35) & (baked[:, 0] - base[:, 0] > 30)
    remote = np.linalg.norm(xyz - [0, .05, .12], axis=1) > .075
    print(f'{scores[0][1].name}: {sum(paint & ~remote)} nearby, {sum(paint & remote)} remote')
    return xyz, sum(paint & ~remote), sum(paint & remote)


def check_city(original, fixed):
    """The City fixture covers almost all receiver UVs in only one original diffuse."""
    old_mesh, _, old_cache = original
    new_mesh, _, new_cache = fixed
    xyz, source_uv, _ = sample_surface(old_mesh, 400_000, 42)
    new_xyz, new_source, atlas = sample_surface(new_mesh, 400_000, 42, dual_uv=True)
    if not np.array_equal(xyz, new_xyz) or not np.array_equal(source_uv, new_source):
        raise ValueError("geometry or source material UVs changed")
    matches = [p for p in old_cache.glob('srpw*d.dds')
               if (diffuse(p, source_uv).max(axis=1) > 0).mean() > .95]
    if len(matches) != 1:
        raise ValueError(f'{old_cache}: ambiguous City receiver diffuse')
    for suffix in ('d', 's'):
        name = matches[0].name[:-5] + suffix + '.dds'
        before, after = diffuse(old_cache/name, source_uv), diffuse(new_cache/name, atlas)
        error = np.abs(before-after).mean(axis=1)
        correlation = np.corrcoef(before.ravel(), after.ravel())[0, 1]
        print(f'{name}: median RGB error {np.median(error):.2f}/255, correlation {correlation:.4f}')
        if np.median(error) > 5 or not correlation > .9:
            raise ValueError('City paint or finish moved on the surface')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('original', 'fixed'):
        parser.add_argument('--' + name, nargs=3, required=True, type=Path,
                            metavar=('MESH', 'UNDECORATED_DDS', 'NATIVE_CACHE'))
    parser.add_argument('--dual-uv', action='store_true', help='read the fixed bake in its separate atlas')
    parser.add_argument('--city', action='store_true', help='compare two City-only bakes on the same AK-12')
    args = parser.parse_args()
    if args.city:
        check_city(args.original, args.fixed)
        raise SystemExit(0)
    before, old_near, old_far = measure(*args.original)
    after, new_near, new_far = measure(*args.fixed, dual_uv=args.dual_uv)
    if not np.array_equal(before, after):
        raise SystemExit('mesh geometry or triangle order changed')
    if min(old_near, old_far, new_near) < 100 or new_far:
        raise SystemExit('expected visible native paint in both runs and remote paint only before the fix')
