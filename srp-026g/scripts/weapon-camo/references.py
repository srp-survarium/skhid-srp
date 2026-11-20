#!/usr/bin/env python3
"""references.py: what 0.69d0 knows of the textures of a weapon_camo.py table (guns.tsv's format).

usage: python3 -I references.py <game folder> <0.69d0 folder> <vostok-port> <table.tsv>... [--write]

<game folder>    as weapon_camo.py's: the weapon configs, their models, materials and textures
<0.69d0 folder>  0.69d0's models/weapons, material_instances/weapons and textures/weapons at their
                 game paths (vostok-port extract-bundle of a .split volume list: its overlay/)
<vostok-port>    resource-porter's, for its dump-config (0.69d0's configs are not 0.26g's format)
<table.tsv>...   every table of the game's weapons (guns.tsv and the weapon packs' camo.tsv)
--write          rewrites the tables (otherwise prints them)

0.69d0 has no camouflage on weapons and so no masks: it has weapons painted by hand, a `_camo` or an
event texture beside the plain one, on the same UVs. Our weapon models are 0.69d0's meshes (most of
them, some with its modules baked in), so a texture of ours and 0.69d0's of the same part share their
UV layout whatever the two are called. For each row this finds the 0.69d0 surfaces whose UV points
lie on the row's own (SAME of a surface's points within EPS of one of ours) and, of the textures
those surfaces draw:
- the plain one: of those covering half our points that are no painted version of another (named after
  it, `ak74n_camo`, or in a folder below it), the one of our texture's name (but `_` and `-`), else
  the nearest to ours in colour;
- its metalness map (the plain one's material, `t_metalness`): the row's 6th column, by which
  weapon_camo.py tells polymer and wood from metal;
- the painted versions: the others in the plain one's folder or below it, each as
  `69:<painted>@69:<plain>` in the row's 2nd column (after the donors it has).
A row 0.69d0 has nothing for keeps its columns (the old Uzi, the SPAS-12, the TOZ-66: other meshes).

It also brings each row's surfaces (the 3rd column) up to the game folder's: every surface of a weapon
config's model (gameplay/items/weapons/wpn_*.options, object/model) that draws the texture, after the
ones the row has; and says which textures those models draw that no table has a row for.
"""
import collections
import functools
import os
import struct
import subprocess
import sys

import numpy as np
from PIL import Image
from scipy.spatial import cKDTree

import weapon_camo as wc

EPS = 0.0007       # UV: 0.69d0 keeps them as half floats (2^-11 near 1)
SAME = 0.9
COVERS = 0.5


def leaves(port, path):
    """A 0.69d0 config's string leaves by dotted path."""
    text = subprocess.run([port, 'dump-config', '--file', path, '--leaves'], capture_output=True, text=True, check=True).stdout
    out = {}
    for line in text.splitlines():
        key, _, value = line.partition('\t')
        if value.startswith('string:"'):
            out[key] = value[8:-1]
    return out


def uv_069(buf):
    """The UV points a 0.69d0 converted_model's triangles use: a vertex's last four bytes, two half
    floats (its 28-, 32- and 36-byte vertices alike); the index chunk (4) after whatever chunks follow."""
    kind, size, count = struct.unpack_from('<III', buf, 0)
    stride = (size - 4) // count
    raw = np.frombuffer(buf, np.uint8, count=count * stride, offset=12).reshape(count, stride)
    uv = raw[:, stride - 4:].copy().view('<f2').reshape(count, 2).astype(np.float64)
    at = 8 + size
    while struct.unpack_from('<I', buf, at)[0] != 4:
        at += 8 + struct.unpack_from('<I', buf, at + 4)[0]
    n = struct.unpack_from('<I', buf, at + 8)[0]
    return uv[np.unique(np.frombuffer(buf, '<u2', count=n, offset=at + 12))]


def surfaces_069(root, port):
    """[(UV points, diffuse texture, metalness texture)] of 0.69d0's weapon model surfaces."""
    materials, out = {}, []
    for folder, _, files in sorted(os.walk(os.path.join(root, 'models', 'weapons'))):
        if 'settings' not in files:
            continue
        for key, material in leaves(port, os.path.join(folder, 'settings')).items():
            if not (key.startswith('material_settings.') and key.endswith('.material_name')):
                continue
            mesh = os.path.join(folder, 'render', key[len('material_settings.'):-len('.material_name')], 'converted_model')
            path = os.path.join(root, 'material_instances', material + '.material')
            if not os.path.exists(mesh) or not os.path.exists(path):
                continue
            if material not in materials:
                m = leaves(port, path)
                materials[material] = tuple(m.get('material.g_stage.effect.%s.value' % k) for k in ('t_albedo', 't_metalness'))
            diffuse, metal = materials[material]
            points = uv_069(open(mesh, 'rb').read())
            if diffuse and len(points) >= 12:
                out.append((points, diffuse, metal))
    return out


def drawn(game):
    """{diffuse texture: [model:surface]} of the weapon configs' models, in the configs' order."""
    out = {}
    folder = os.path.join(game.root, 'gameplay', 'items', 'weapons')
    for name in sorted(os.listdir(folder)):
        if not (name.startswith('wpn_') and name.endswith('.options')):
            continue
        model = wc.config_value(open(os.path.join(folder, name), 'rb').read(), 'object/model')
        settings = game.read('models/%s.skinned_model/settings' % model)
        for surface in game.surfaces(model):
            material = wc.config_value(settings, 'material_settings/%s/material_name' % surface)
            texture = wc.config_value(game.read('material_instances/%s.material' % material),
                                      'material/g_stage/effect/texture_diffuse/value')
            if texture and '%s:%s' % (model, surface) not in out.setdefault(texture, []):
                out[texture].append('%s:%s' % (model, surface))
    return out


@functools.lru_cache(maxsize=None)
def small(path):
    return np.asarray(Image.open(path).convert('RGB').resize((256, 256), Image.BILINEAR), np.float32) / 255


def references(texture, surfaces, game, root, donors):
    """(the plain 0.69d0 texture of a row's UVs or None, its metalness map or None, its painted versions)."""
    ours = np.unique(np.round(np.concatenate([game.mesh(*s.split(':'))[0] for s in surfaces]), 5), axis=0)
    tree = cKDTree(ours)
    covered, metal_of = collections.defaultdict(lambda: np.zeros(len(ours), bool)), {}
    for points, diffuse, metal in donors:
        distance, index = tree.query(points, distance_upper_bound=EPS)
        on = np.isfinite(distance)
        if on.mean() >= SAME:
            covered[diffuse][index[on]] = True
            metal_of[diffuse] = metal
    file = lambda t: os.path.join(root, 'textures', t + '.dds')
    same = [t for t, c in covered.items() if c.mean() >= COVERS and os.path.exists(file(t))]
    if not same:
        return None, None, []
    # a painted version is named after its plain texture (ak74n_camo) or lies in a folder below it
    # (ak_74m_victoryday2020/ak_74m): the plain one is the nearest to ours that is no painted version
    painted_of = lambda t, plain: t != plain and (t.startswith(plain + '_') or t.startswith(plain.rsplit('/', 1)[0] + '/')
                                                  and t.count('/') > plain.count('/'))
    plains = [t for t in same if not any(painted_of(t, other) for other in same)]
    name = lambda t: t.rsplit('/', 1)[1].replace('_', '').replace('-', '')
    named = [t for t in plains if name(t) == name(texture)] or plains
    plain = named[0]
    if len(named) > 1:
        base = small(game.path('textures/%s.dds' % texture))
        plain = min(named, key=lambda t: (float(np.abs(small(file(t)) - base).mean()), t))
    folder = plain.rsplit('/', 1)[0] + '/'
    metal = metal_of[plain] if metal_of[plain] and os.path.exists(file(metal_of[plain])) else None
    return plain, metal, sorted(t for t in same if t != plain and t.startswith(folder))


def main():
    args = sys.argv[1:]
    write = '--write' in args
    if write:
        args.remove('--write')
    if len(args) < 4:
        sys.exit(__doc__)
    game, root, port, tables = wc.Files(args[0]), args[1], args[2], args[3:]
    donors = surfaces_069(root, port)
    weapons = drawn(game)
    known = set()
    for table in tables:
        out = []
        for line in open(table, encoding='utf-8').read().splitlines():
            cols = line.split('\t')
            if line and not line.startswith('#'):
                cols += [''] * (6 - len(cols))
                surfaces = [s for s in cols[2].split(',') if s]
                surfaces += [s for s in weapons.get(cols[0], []) if s not in surfaces]
                plain, metal, painted = references(cols[0], surfaces, game, root, donors)
                kept = [d for d in cols[1].split(',') if d and not d.startswith('69:')]
                cols[1] = ','.join(kept + ['69:%s@69:%s' % (p, plain) for p in painted])
                cols[2], cols[5] = ','.join(surfaces), '69:' + metal if metal else ''
                known.update([cols[0]] + cols[3].split(','))
                line = '\t'.join(cols).rstrip('\t')
                print('%-58s %s' % (cols[0], '0.69d0 %s, %d painted, %s' % (plain, len(painted), 'its metalness' if metal
                                                                             else 'no metalness map')
                                    if plain else 'nothing in 0.69d0'), file=sys.stderr)
            out.append(line)
        text = '\n'.join(out) + '\n'
        if write:
            open(table, 'w', encoding='utf-8').write(text)
        else:
            sys.stdout.write(text)
    for texture in sorted(set(weapons) - known):
        print('no row: %s (%s)' % (texture, ', '.join(weapons[texture][:3])), file=sys.stderr)


if __name__ == '__main__':
    main()
