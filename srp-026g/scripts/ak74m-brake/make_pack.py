#!/usr/bin/env python3
"""make_pack.py: the content pack of the AK-74M's muzzle brake, the first weapon attachment on
the weapon kit (client patch 68; ../../docs/notes/weapon-kit.md, "The AK-74M's brake").

usage: python3 -I make_pack.py --live <folder> --out <packs folder>/ak74m-brake-<version>

--live  the players' files at their game paths, every path of LIVE (`srp-game-data get <the
        released layers> --out <folder> <paths>`).
--out   the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and
        the manifest's `pack` row (release/content.tsv).

0.26g's AK-74M has its muzzle brake in the gun's mesh: a closed part of surface ak74lpsg4 (255
vertices, 260 triangles, sharing no vertex with the barrel; muzzle-cut.md, class c). 0.69d0 made
that same mesh a module of its own, `ak74_m24x15_muzzlebrake` (255 vertices, 260 triangles, 8.1 cm
long, its origin at the barrel's end). The pack:

- cuts the part out of ak74lpsg4 in the three AK-74M bodies (the plain one and the Cobra's and
  EOTech's, whose meshes are the same file): the index block without its triangles; the vertices,
  skinning and the surface's other files as they are, so a bare AK-74M shows its barrel's end;
- makes the brake a static model, `weapons/attach/muzzle_brake/ak74_m24x15`, of the part's own
  vertices in barrel_point's frame (patch 68 draws a device there; forward is -z, as 0.46c2's
  devices), with the gun's material (`weapons/ak_74m`), so it sits and looks as the stock brake;
- adds the item `srp_ak74_m24x15` (DICT_ID, category 22, type srp_muzl, sold by seller 0) whose
  modification adds data/srp_muzzle_model and srp_muzzle_fire_mm (the brake's front, 29 mm past
  barrel_point) and scales the kick up, min and max_top_recoil x0.9 (type 2: base x (1 + sum));
- draws its shop icon from the model (../weapon-attachments/render_icons.py) into the consumables
  atlas, LobbyMenu_AmmoIcons row 1, column 5.

The item fits the AK-74M alone (content-edit ak74m-brake-1: one items_compatibility row).
Needs numpy and Pillow (for the icon).
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
sys.path.insert(0, str(HERE.parent / 'weapon-attachments'))
import content_edit as ce  # noqa: E402

PACK_ID = 'ak74m-brake'
VERSION = '1'
DICT_ID = 2621
NAME = 'srp_ak74_m24x15'
PRICE = 9000
MODIFICATION = 262101
KICK = -0.1                      # type 2: min and max_top_recoil x (1 + KICK)
ICON = (1, 5)                    # ui_desc/icon: [row, col] of LobbyMenu_AmmoIcons, 1-based
BODIES = ('ak74m', 'ak74m_cobra_ekp_8', 'ak74m_eotech_552')
SURFACE = 'ak74lpsg4'
MATERIAL = 'weapons/ak_74m'
DEVICE = 'weapons/attach/muzzle_brake/ak74_m24x15'
DEVICE_SURFACE = 'ak74_m24x15_s'
CONFIG = f'gameplay/items/attachments/{NAME}.options'
TEXTURE = 'textures/weapons/ak74m/ak74m.dds'          # the material's texture_diffuse


def body(name, file):
    return f'models/weapons/ak74m/{name}.skinned_model/{file}'


# the players' files: sha256
LIVE = {
    body('ak74m', f'render/{SURFACE}/converted_model'):
        '5eccd3f6fff41b88853430338fc0837c70329f0e88e532104d4be20238aacfe2',
    body('ak74m_cobra_ekp_8', f'render/{SURFACE}/converted_model'):
        '5eccd3f6fff41b88853430338fc0837c70329f0e88e532104d4be20238aacfe2',
    body('ak74m_eotech_552', f'render/{SURFACE}/converted_model'):
        '5eccd3f6fff41b88853430338fc0837c70329f0e88e532104d4be20238aacfe2',
    body('ak74m', 'render/export_properties'):
        'd40f3f6cf3cb6c96246fd23f4470300ee193701d42aa14787173bbfd17089b40',
    body('ak74m', 'render/bind_pose'):
        '065aa7cb357e5e1a402fb0c493442a80adf050dcdf8ec8efc138c26c17dcbbe7',
    'material_instances/weapons/ak_74m.material':
        '518ad071eb784d1121bb617ed0f4ef153b0c6b10976204a6a8d9fc19116b3694',
    TEXTURE: None,                       # the icon's colours only
}
# the part: its vertex and triangle counts (muzzle-cut.md) and its length along the barrel
PART_VERTICES, PART_TRIANGLES = 255, 260


def sha256(data):
    return hashlib.sha256(data).hexdigest()


# -- binary configs (0.26g: 24-byte values {data, name, crc, type, count}) --

T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR, T_FLOAT3 = 0, 1, 2, 3, 4, 5, 7


class Float3(tuple):
    pass


def config(tree):
    """A binary config of `tree`: dicts are named tables (children sorted by the CRC of their
    names), lists indexed ones, bool, int, float, str and Float3 leaves. The values first (each
    table's children together), then the names and the strings and vectors, as the game's files
    are laid out; content_edit.walk reads it back."""
    values = [None]
    blob = bytearray()
    names = {}

    def text(s):
        raw = s.encode() + b'\0'
        if raw not in names:
            names[raw] = len(blob)
            blob.extend(raw)
        return names[raw]

    def put(slot, key, value):
        if isinstance(value, dict):
            items = sorted(value.items(), key=lambda kv: zlib.crc32(kv[0].encode()))
        elif isinstance(value, list):
            items = [(None, v) for v in value]
        else:
            items = None
        if items is not None:
            first = len(values)
            values.extend([None] * len(items))
            for i, (k, v) in enumerate(items):
                put(first + i, k, v)
            values[slot] = ('table', first, key, T_NAMED if isinstance(value, dict) else T_INDEXED, len(items))
        elif isinstance(value, bool):
            values[slot] = ('raw', int(value), key, T_BOOL, 4)
        elif isinstance(value, int):
            values[slot] = ('raw', value & 0xFFFFFFFF, key, T_INT, 4)
        elif isinstance(value, Float3):
            values[slot] = ('blob', struct.pack('<3f', *value), key, T_FLOAT3, 12)
        elif isinstance(value, float):
            values[slot] = ('raw', struct.unpack('<I', struct.pack('<f', value))[0], key, T_FLOAT, 4)
        elif isinstance(value, str):
            values[slot] = ('str', value, key, T_STR, len(value.encode()) + 1)
        else:
            raise TypeError(f'{key}: {value!r}')

    put(0, None, tree)
    base = 24 * len(values)
    out = bytearray()
    for kind, data, key, kind_id, count in values:
        name = base + text(key) if key is not None else 0
        crc = zlib.crc32(key.encode()) if key is not None else 0
        if kind == 'table':
            data = 24 * data
        elif kind == 'str':
            data = base + text(data)
        elif kind == 'blob':
            data, raw = base + len(blob), data
            blob.extend(raw)
        out += struct.pack('<QQIHH', data, name, crc, kind_id, count)
    return bytes(out + blob)


def tree_of(buf):
    """content_edit's reading of a config as `config`'s tree (for the read-back check)."""
    def at(off):
        data, _, _, t, cnt = ce.rec(buf, off)
        if t == T_NAMED:
            return {ce.name_at(buf, ce.rec(buf, c)[1]).decode(): at(c) for c in ce.children(buf, off)}
        if t == T_INDEXED:
            return [at(c) for c in ce.children(buf, off)]
        if t == T_BOOL:
            return bool(data & 0xFFFFFFFF)
        if t == T_INT:
            return ce.as_number(t, data & 0xFFFFFFFF)
        if t == T_FLOAT:
            return ce.as_number(t, data & 0xFFFFFFFF)
        if t == T_STR:
            return bytes(buf[data:data + cnt - 1]).decode()
        if t == T_FLOAT3:
            return Float3(struct.unpack_from('<3f', buf, data))
        raise TypeError(t)
    return at(0)


def same(a, b):
    if isinstance(a, dict):
        return isinstance(b, dict) and a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return isinstance(b, list) and len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    if isinstance(a, float) or isinstance(b, float) or isinstance(a, Float3):
        return np.allclose(np.asarray(a, dtype=np.float64), np.asarray(b, dtype=np.float64), atol=1e-6)
    return a == b


def checked_config(tree):
    data = config(tree)
    back = tree_of(bytearray(data))
    if not same(tree, back):
        sys.exit(f'a config does not read back as written: {tree} != {back}')
    return data


# -- meshes --

def skinned_mesh(data):
    """(raw vertices [n, 36] bytes, triangles [m, 3], the index block's offset)."""
    kind, size, count = struct.unpack_from('<3I', data, 0)
    if kind != 3 or size != 4 + 36 * count:
        sys.exit('ak74lpsg4: not a skinned mesh of 36-byte vertices')
    raw = np.frombuffer(data, dtype=np.uint8, count=36 * count, offset=12).reshape(count, 36)
    at = 8 + size
    kind, isize, n = struct.unpack_from('<3I', data, at)
    if kind != 4 or isize != 4 + 2 * n or at + 8 + isize != len(data):
        sys.exit('ak74lpsg4: not one index block after the vertices')
    tris = np.frombuffer(data, dtype='<u2', count=n, offset=at + 12).reshape(-1, 3)
    return raw, tris, at


def positions(raw):
    return raw[:, 0:12].copy().view('<f4').reshape(-1, 3).astype(np.float64)


def parts(pos, tris):
    """The mesh's closed parts: (vertex indices, triangle indices), vertices welded by
    position (0.1 mm) as muzzle-cut.md's survey."""
    key = [tuple(np.round(p, 4)) for p in pos]
    parent = {}

    def find(a):
        while parent.setdefault(a, a) != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a
    for t in tris:
        a = find(key[t[0]])
        for v in t[1:]:
            parent[find(key[v])] = a
    groups = {}
    for i, t in enumerate(tris):
        groups.setdefault(find(key[t[0]]), []).append(i)
    out = []
    for tri_ids in groups.values():
        verts = sorted({int(v) for i in tri_ids for v in tris[i]})
        out.append((verts, tri_ids))
    return out


def locator(export_properties, name):
    buf = bytearray(export_properties)
    for i in range(len(ce.children(buf, ce.find(buf, 'locators')))):
        if ce.value_of(buf, ce.find(buf, f'locators[{i}]/name')) == name:
            position = ce.value_of(buf, ce.find(buf, f'locators[{i}]/position'))
            return np.array(position, dtype=np.float64), ce.value_of(buf, ce.find(buf, f'locators[{i}]/bone_name'))
    sys.exit(f'no locator {name}')


def bind_pose(data):
    n, = struct.unpack_from('<H', data)
    at, out = 2, {}
    for _ in range(n):
        end = data.index(0, at)
        out[data[at:end].decode()] = struct.unpack_from('<9f', data, end + 1)
        at = end + 37
    return out


def flip_packed(column):
    """A packed normal's x or z negated: u8 around 127.5."""
    return (255 - column.astype(np.int32)).astype(np.uint8)


def device_mesh(raw, tris, verts, tri_ids, barrel_point):
    """The part as a static mesh (28-byte vertices {float3 position, three packed normals, half2
    uv}) in barrel_point's frame. The vertices are in the model's space: bone `offset` is bound
    rotated pi about y with no translation (checked), so its space is (-x, y, -z) of the model's,
    and barrel_point is at `barrel_point` in it."""
    index = {v: i for i, v in enumerate(verts)}
    part = raw[verts]
    pos = positions(part)
    local = np.stack([-pos[:, 0], pos[:, 1], -pos[:, 2]], axis=1) - barrel_point
    out = np.zeros((len(verts), 28), dtype=np.uint8)
    out[:, 0:12] = local.astype('<f4').view(np.uint8).reshape(-1, 12)
    for column, at in ((16, 12), (20, 16), (24, 20)):        # normal, tangent, binormal
        packed = part[:, column:column + 4].copy()
        packed[:, 0] = flip_packed(packed[:, 0])
        packed[:, 2] = flip_packed(packed[:, 2])
        packed[:, 3] = 0x7F if at == 12 else 0xFF            # as the game's static meshes
        out[:, at:at + 4] = packed
    uv = part[:, 28:36].copy().view('<f4').reshape(-1, 2)
    out[:, 24:28] = uv.astype('<f2').view(np.uint8).reshape(-1, 4)
    indices = np.array([[index[int(v)] for v in tris[i]] for i in tri_ids], dtype='<u2')
    # the bone's pi turn mirrors nothing (a rotation): the winding stays
    vertex_block = out.tobytes()
    index_block = indices.tobytes()
    data = (struct.pack('<3I', 3, 4 + len(vertex_block), len(verts)) + vertex_block
            + struct.pack('<3I', 4, 4 + len(index_block), indices.size) + index_block)
    return data, local


def cut_mesh(data, at, tris, tri_ids):
    keep = np.delete(tris, tri_ids, axis=0).astype('<u2')
    block = keep.tobytes()
    return data[:at] + struct.pack('<3I', 4, 4 + len(block), keep.size) + block


def box(local):
    lo, hi = local.min(axis=0), local.max(axis=0)
    return {'min': Float3(lo), 'max': Float3(hi)}


def item_config(fire_mm):
    def modifier(path, value, kind):
        return {'path': path, 'value': value, 'type': kind}
    return {
        'parameters': {
            'degrade_per_death': 0, 'weight': 0, 'available_modifications_mask': '0', 'drop_weight': 0,
            'type': 'srp_muzl', 'match_making_level': 1, 'degrade_per_match': 0,
            'available_defects_mask': '0', 'faction_affinity': 0, 'item_level': 1, 'degrade_per_use': 0,
            'rarity': [0], 'modification': MODIFICATION,
        },
        'modifications': [{
            'id': MODIFICATION,
            'modifiers': [
                modifier(['data'], ['srp_muzzle_model', DEVICE], 3),
                modifier(['data'], ['srp_muzzle_fire_mm', fire_mm], 3),
                modifier(['recoil', 'min_top_recoil'], KICK, 2),
                modifier(['recoil', 'max_top_recoil'], KICK, 2),
            ],
        }],
        'ui_desc': {
            'text_descriptions': {'name': f'st_{NAME}_name', 'description': f'st_{NAME}_description'},
            'icon': list(ICON),
            'props_list': {},
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    live = {}
    for path, want in LIVE.items():
        data = (args.live / path).read_bytes()
        if want is not None and sha256(data) != want:
            sys.exit(f'{path}: sha256 {sha256(data)}, not the players\' {want}')
        live[path] = data

    mesh_path = body('ak74m', f'render/{SURFACE}/converted_model')
    data = live[mesh_path]
    raw, tris, at = skinned_mesh(data)
    pos = positions(raw)
    bp, bone = locator(live[body('ak74m', 'render/export_properties')], 'barrel_point')
    bind = bind_pose(live[body('ak74m', 'render/bind_pose')])[bone]
    if bone != 'offset' or any(abs(t) > 1e-6 for t in bind[:3]) or abs(abs(bind[3]) - np.pi) > 1e-4 \
            or abs(bind[4]) > 1e-4 or abs(abs(bind[5]) - np.pi) > 1e-4:
        sys.exit(f'barrel_point is not on bone offset bound pi about y: {bone} {bind}')
    # barrel_point in the model's space, and the barrel along +z there
    front_z = pos[:, 2].max()
    found = [(v, t) for v, t in parts(pos, tris)
             if len(v) == PART_VERTICES and len(t) == PART_TRIANGLES and np.isclose(pos[v, 2].max(), front_z)]
    if len(found) != 1:
        sys.exit(f'{mesh_path}: no single front part of {PART_VERTICES} vertices and {PART_TRIANGLES} triangles')
    verts, tri_ids = found[0]
    device, local = device_mesh(raw, tris, verts, tri_ids, bp)
    fire_mm = round(-local[:, 2].min() * 1000)
    cut = cut_mesh(data, at, tris, tri_ids)

    files = args.out / 'files'
    ops = []

    def add(path, content, op='add_file', base=None):
        (files / path).parent.mkdir(parents=True, exist_ok=True)
        (files / path).write_bytes(content)
        entry = {'op': op, 'path': path}
        if base is not None:
            entry['base_sha256'] = base
        ops.append(entry)

    for name in BODIES:
        path = body(name, f'render/{SURFACE}/converted_model')
        add(path, cut, 'replace_file', LIVE[path])
    model = f'models/{DEVICE}.model'
    surface = {'material_name': MATERIAL, 'used_in_portals': False}
    add(f'{model}/settings', checked_config({
        'collision_use_raytrace_for_walker': False, 'walker_collision_enabled': False,
        'lod_hierrarchy': {'LOD0': {'surfaces': [DEVICE_SURFACE]}},
        'material_settings': {DEVICE_SURFACE: surface}, 'calculate_ao': False,
        'game_material_settings': {DEVICE_SURFACE: {'game_material_id': 65535, 'game_material_name': ''}},
        'lod_switching': {'default_params': True, 'type': 0}, 'collision_enabled': False,
    }))
    add(f'{model}/render/export_properties', checked_config({
        'src_filename': 'X:/resources/sources/scenes/weapons/modules/muzzlebrakes/ak74_m24x15_muzzlebrake.mb',
        'type': 1, 'version': 0, 'bounding_box': box(local),
    }))
    centre = (local.min(axis=0) + local.max(axis=0)) / 2
    add(f'{model}/render/{DEVICE_SURFACE}/export_properties', checked_config({
        'bounding_sphere': {'radius': float(np.linalg.norm(local - centre, axis=1).max()), 'center': Float3(centre)},
        'source': {'indices': 3 * len(tri_ids), 'primitives': len(tri_ids),
                   'mesh_name': '|ak74_m24x15_muzzlebrake|vis|ak_74m_muzzle_brake|ak_74m_muzzle_brakeShape',
                   'texture_name': '', 'vertices': len(verts)},
        'type': 0, 'version': 0, 'bounding_box': box(local),
    }))
    add(f'{model}/render/{DEVICE_SURFACE}/converted_model', device)
    add(CONFIG, checked_config(item_config(fire_mm)))
    ops.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [{
        'base_cost': PRICE, 'cfg_name': CONFIG, 'dict_id': DICT_ID, 'is_premium': False,
        'is_stack': False, 'item_category': 22, 'name': NAME}]})

    # the shop icon, drawn from the model with the gun's diffuse texture
    import render_icons
    material = bytearray(live['material_instances/weapons/ak_74m.material'])
    texture = ce.value_of(material, ce.find(material, 'material/g_stage/effect/texture_diffuse/value'))
    if f'textures/{texture}.dds' != TEXTURE:
        sys.exit(f'{MATERIAL}: texture_diffuse {texture}, not {TEXTURE}')
    import io
    from PIL import Image
    diffuse = np.asarray(Image.open(io.BytesIO(live[TEXTURE])).convert('RGB'), dtype=np.float64)
    vpos, uv, idx = render_icons.mesh(device)
    icon = f'icons/LobbyMenu_AmmoIcons/r{ICON[0]:02d}c{ICON[1]:02d}.png'
    (args.out / icon).parent.mkdir(parents=True, exist_ok=True)
    render_icons.render([(vpos, uv, idx, diffuse)]).save(args.out / icon)
    ops.append({'op': 'atlas_icons_add', 'swf': 'flash_movies/lobby_menu.swf', 'symbol': 'LobbyMenu_AmmoIcons',
                'icons': [{'row': ICON[0], 'col': ICON[1], 'source': icon}]})

    (args.out / 'pack.json').write_text(json.dumps({
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': "the AK-74M's muzzle brake as a weapon-kit muzzle device: its stock brake cut out of the "
                       "gun's mesh, the same brake (0.69d0's ak74_m24x15 module) a static model at barrel_point, "
                       f"the item {NAME} ({DICT_ID}): vertical kick -10 %",
        'notes': ['made by srp-026g/scripts/ak74m-brake/make_pack.py from the released files'],
        'operations': ops,
    }, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')
    print(f'part: {len(verts)} vertices, {len(tri_ids)} triangles; z {local[:, 2].min():.4f}..{local[:, 2].max():.4f} '
          f'in barrel_point\'s frame; fire point {fire_mm} mm')
    for op in ops:
        print(op['op'], op.get('path', op.get('table', op.get('symbol'))))


if __name__ == '__main__':
    main()
