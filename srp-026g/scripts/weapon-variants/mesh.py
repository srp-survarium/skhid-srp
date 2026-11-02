"""The TOZ-122 "Leather": 0.69d0's model of its own, on 0.26g's TOZ-122.

0.69d0 draws it as `toz_122_huntersface` (the TOZ-122 with its stock and forend cut for the wrap, on
the TOZ-122's own texture) and the default module `toz_122_huntersface_decor` (the leather wrap, a
static model with its own maps) at the locator `decor_01_point` of bone `offset`. The model here:

- the TOZ-122's model files (skeleton, bind pose, locators: 0.69d0's bind pose is byte for byte
  0.26g's), its surface `lpsg4` the huntersface mesh in 0.26g's vertex layout, on the TOZ-122's
  material (0.69d0's mesh shares 0.26g's UVs: make_pack.match);
- one more surface, the wrap, bound wholly to `offset` at that bone's bind position plus the
  locator (0.69d0's locators are written half a turn about y from the mesh: x and z negated), as
  resource-porter's weapons_069d bakes a structural module;
- the wrap's material the AKM's (a 0.26g weapon material: the shader switches 0.26g compiled) with
  the wrap's maps: its diffuse from 0.69d0's albedo by the TOZ-122's own paint factors
  (paint.factors: the wrap is leather, no metal), its normal map as 0.69d0 has it, its gloss map as
  weapons_069d makes it (0.36 x smoothness) and a flat 0 Fresnel (0.69d0's wrap has no metal).

The vertex conversions are resource-porter's (src/weapons_069d/convert.rs: skinned_069_to_026,
static_069_to_skinned), here in numpy.
"""
import struct

import numpy as np

import paint
from common import FOLDER, bcw

SKINNED_026 = 36
WRAP_SURFACE = 'toz_122_huntersface_deco_s'
WRAP_TEMPLATE = 'weapons/akmn'          # material_instances/<it>.material
GLOSS = 0.36


def chunks(data):
    out, at = [], 0
    while at < len(data):
        kind, size = struct.unpack_from('<II', data, at)
        out.append((kind, data[at + 8:at + 8 + size]))
        at += 8 + size
    return out


def _vertices(body):
    n = struct.unpack_from('<I', body, 0)[0]
    stride = (len(body) - 4) // n
    assert 4 + stride * n == len(body), 'vertex chunk size is not a stride'
    return np.frombuffer(body, np.uint8, count=n * stride, offset=4).reshape(n, stride), stride


def _indices(found, n):
    body = dict(found)[4]
    count = struct.unpack_from('<I', body, 0)[0]
    idx = np.frombuffer(body, '<u2', count=count, offset=4)
    assert count % 3 == 0 and (idx < n).all(), 'bad index chunk'
    return idx


def _basis_words(values, alphas=(0x7F, 0xFF, 0xFF)):
    """0.69d0's basis floats ((x << 16 | y << 8 | z) / 2^24) as 0.26g's packed words."""
    assert np.isfinite(values).all() and (values >= 0).all() and (values < 1).all(), 'not 0.69d0 basis floats'
    packed = np.round(values.astype(np.float64) * 16_777_216).astype(np.uint32)
    assert (packed <= 0xFFFFFF).all()
    return (packed | (np.array(alphas, np.uint32)[None, :] << 24)).astype('<u4')


def _write(vertices, indices):
    n = len(vertices)
    assert n <= 65536 and len(indices) % 3 == 0
    return b''.join([struct.pack('<III', 3, 4 + n * SKINNED_026, n), vertices.tobytes(),
                     struct.pack('<III', 4, 4 + 2 * len(indices), len(indices)),
                     np.asarray(indices, '<u2').tobytes()])


def skinned(data, bones):
    """A 0.69d0 skinned converted_model (32-byte vertices: position, bone, three basis floats, half2
    UV) as 0.26g's (36 bytes: position, bone, three packed basis words, float2 UV)."""
    found = chunks(data)
    assert {k for k, _ in found} <= {3, 4, 5, 6, 110, 114}, 'unknown 0.69d0 skinned chunk'
    raw, stride = _vertices(dict(found)[3])
    assert stride == 32, f'0.69d0 skinned vertex of {stride} bytes'
    out = np.zeros((len(raw), SKINNED_026), np.uint8)
    out[:, :16] = raw[:, :16]
    assert (raw[:, 12:16].copy().view('<u4') < bones).all(), 'vertex bone outside the skeleton'
    out[:, 16:28] = _basis_words(raw[:, 16:28].copy().view('<f4')).view(np.uint8).reshape(-1, 12)
    out[:, 28:36] = raw[:, 28:32].copy().view('<f2').astype('<f4').view(np.uint8).reshape(-1, 8)
    return _write(out, _indices(found, len(raw)))


def static(data, offset, bone):
    """A 0.69d0 static converted_model (28 or 32 bytes: position, three basis floats, half2 UV) as a
    0.26g skinned surface bound wholly to `bone`, moved by `offset` (not turned)."""
    found = chunks(data)
    assert {k for k, _ in found} <= {3, 4, 110, 114}, 'unknown 0.69d0 static chunk'
    raw, stride = _vertices(dict(found)[3])
    assert stride in (28, 32), f'0.69d0 static vertex of {stride} bytes'
    out = np.zeros((len(raw), SKINNED_026), np.uint8)
    pos = raw[:, :12].copy().view('<f4') + np.asarray(offset, '<f4')
    out[:, :12] = pos.astype('<f4').view(np.uint8).reshape(-1, 12)
    out[:, 12:16] = np.array([bone], '<u4').view(np.uint8)
    out[:, 16:28] = _basis_words(raw[:, 12:24].copy().view('<f4')).view(np.uint8).reshape(-1, 12)
    out[:, 28:36] = raw[:, 24:28].copy().view('<f2').astype('<f4').view(np.uint8).reshape(-1, 8)
    return _write(out, _indices(found, len(raw)))


def bind_pose(data):
    """[(bone name, bind translation)] of a bind_pose file."""
    n = struct.unpack_from('<H', data, 0)[0]
    at, out = 2, []
    for _ in range(n):
        end = data.index(0, at)
        name = data[at:end].decode()
        frame = struct.unpack_from('<9f', data, end + 1)
        out.append((name, frame[:3]))
        at = end + 1 + 36
    return out


def dds_full(template, image):
    """`image` (float RGB 0..1) encoded whole in the DXT1 layout of `template` (a 0.26g DDS of its size)."""
    return paint.encode_over(template, image, np.ones(image.shape[:2]))


def leather(inputs, row, model26, new_model, kfactors):
    """([(game path, bytes)] of the TOZ-122 "Leather"'s model, its wrap's material and maps,
    {the wrap's surface, its diffuse, the body's surface} for the icon)."""
    files = []
    prefix = f'models/{model26}.skinned_model/'
    donor = f'models/{row.donor_model}.skinned_model/'
    decor = f'models/{row.donor_model}_decor.model/'
    bind = inputs.read('compose', prefix + 'render/bind_pose')
    if inputs.read('donor', donor + 'render/bind_pose') != bind:
        raise ValueError(f'{row.name}: 0.69d0\'s bind pose is not the TOZ-122\'s')
    bones = bind_pose(bind)
    settings = inputs.config('compose', prefix + 'settings')
    (ours,) = settings['lod_hierrarchy']['LOD0']['surfaces']
    (theirs,) = inputs.config('donor', donor + 'settings')['lod_hierrarchy']['LOD0']['surfaces']
    body = skinned(inputs.read('donor', f'{donor}render/{theirs}/converted_model'), len(bones))
    # The wrap at its socket.
    locators = {l['name']: l for l in inputs.config('donor', donor + 'render/export_properties')['locators']}
    socket = locators['decor_01_point']
    bone = [name for name, _ in bones].index(socket['bone_name'])
    at = np.array(bones[bone][1]) + np.array([-1, 1, -1]) * np.array(socket['position'])
    if any(abs(r) > 1e-6 for r in socket['rotation']):
        raise ValueError('decor_01_point is turned; only a moved module is baked')
    decor_settings = inputs.config('donor', decor + 'settings')
    (wrap,) = decor_settings['lod_hierrarchy']['LOD0']['surfaces']
    wrap_mesh = static(inputs.read('donor', f'{decor}render/{wrap}/converted_model'), at, bone)
    out = f'models/{new_model}.skinned_model/'
    for path in inputs.paths('compose', prefix):
        rel = path[len(prefix):]
        if rel.startswith(f'render/{ours}/') or rel == 'settings':
            continue
        files.append((out + rel, inputs.read('compose', path)))
    properties = inputs.config('compose', f'{prefix}render/{ours}/export_properties')
    for surface, data, name in ((ours, body, theirs), (WRAP_SURFACE, wrap_mesh, wrap)):
        n = struct.unpack_from('<I', data, 8)[0]
        i = struct.unpack_from('<I', data, 12 + n * SKINNED_026 + 8)[0]
        p = {**properties, 'source': {**properties['source'], 'vertices': n, 'indices': i, 'primitives': i // 3,
                                       'mesh_name': f'|{row.donor_model}|{name}'}}
        files.append((f'{out}render/{surface}/converted_model', data))
        files.append((f'{out}render/{surface}/export_properties', bcw.dump(p)))
    # The wrap's maps and material.
    folder = f'{FOLDER}/{row.name}'
    material_name = decor_settings['material_settings'][wrap]['material_name']
    effect = inputs.config('donor', f'material_instances/{material_name}.material')['material']['g_stage']['effect']
    template = inputs.read('compose', 'textures/weapons/toz_122/toz122.dds')
    albedo = paint.resized(paint.decode_dds(inputs.read('donor', f"textures/{effect['t_albedo']['value']}.dds")),
                           (2048, 2048))
    diffuse = np.clip(albedo * kfactors[None, None, :, 0], 0, 1)
    smooth = paint.resized(paint.decode_dds(inputs.read('donor', f"textures/{effect['t_smoothness']['value']}.dds")),
                           (2048, 2048))
    names = {'diffuse': f'{folder}/wrap', 'normal': f'{folder}/wrap_nmap', 'gloss': f'{folder}/wrap_smap',
             'fresnel': f'{folder}/wrap_fmap'}
    images = {'diffuse': diffuse}
    files.append((f"textures/{names['diffuse']}.dds", dds_full(template, diffuse)))
    files.append((f"textures/{names['gloss']}.dds", dds_full(template, np.clip(GLOSS * smooth, 0, 1))))
    files.append((f"textures/{names['fresnel']}.dds", dds_full(template, np.zeros_like(diffuse))))
    files.append((f"textures/{names['normal']}.dds",
                  inputs.read('donor', f"textures/{effect['t_normal']['value']}.dds")))
    for key in names.values():
        files.append((f'textures/{key}.options', inputs.read('compose', 'textures/weapons/toz_122/toz122.options')))
    material = inputs.config('compose', f'material_instances/{WRAP_TEMPLATE}.material')
    e = material['material']['g_stage']['effect']
    e['texture_diffuse']['value'] = names['diffuse']
    e['texture_normal']['value'] = names['normal']
    e['texture_roughness']['value'] = names['gloss']
    e['texture_fresnel']['value'] = names['fresnel']
    wrap_material = f'{folder}/wrap'
    files.append((f'material_instances/{wrap_material}.material', bcw.dump(material)))
    settings['lod_hierrarchy']['LOD0']['surfaces'] = [ours, WRAP_SURFACE]
    settings['material_settings'][WRAP_SURFACE] = {**settings['material_settings'][ours],
                                                   'material_name': wrap_material}
    files.append((out + 'settings', bcw.dump(settings)))
    return files, {'surface': WRAP_SURFACE, 'diffuse': names['diffuse'], 'body': ours,
                   'meshes': {ours: body, WRAP_SURFACE: wrap_mesh}, 'wrap_image': images['diffuse']}
