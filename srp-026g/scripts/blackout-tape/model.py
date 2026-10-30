"""0.26g skinned models: each render surface's converted_model (positions, packed normal/
tangent/binormal, bone indices and weights, UVs, triangles) and its material's maps."""
import os
import struct
import numpy as np
import gameview as gv


def unpack(u):
    return np.stack([((u >> 16) & 255), ((u >> 8) & 255), (u & 255)], -1) / 127.5 - 1.0


def surface(b):
    size, n = struct.unpack_from('<II', b, 4)
    stride = (size - 4) // n
    raw = np.frombuffer(b, dtype=np.uint8, count=n * stride, offset=12).reshape(n, stride)
    pos = raw[:, 0:12].copy().view('<f4').reshape(n, 3).astype(np.float64)
    basis = raw[:, stride - 20:stride - 8].copy().view('<u4').reshape(n, 3)
    uv = raw[:, stride - 8:stride].copy().view('<f4').reshape(n, 2).astype(np.float64)
    ic = 12 + n * stride
    m = struct.unpack_from('<I', b, ic + 8)[0]
    tri = np.frombuffer(b, dtype='<u2', count=m, offset=ic + 12).reshape(-1, 3).astype(np.int64)
    bones = raw[:, 12:16].astype(np.int64)
    nw = {48: 3, 44: 2, 40: 1, 36: 0}[stride]
    w = np.zeros((n, 4))
    if nw:
        w[:, :nw] = raw[:, 16:16 + 4 * nw].copy().view('<f4').reshape(n, nw)
    w[:, nw] = 1 - w[:, :nw].sum(1)
    return dict(pos=pos, nrm=unpack(basis[:, 0]), tan=unpack(basis[:, 1]), bin=unpack(basis[:, 2]),
                uv=uv, tri=tri, bones=bones, w=w)


def material_maps(material):
    leaves = gv.config('material_instances/%s.material' % material)
    get = lambda k: gv.string(leaves.get('material.g_stage.effect.%s.value' % k, 'string:""'))
    return dict(diffuse=get('texture_diffuse'), normal=get('texture_normal'))


def model(base_model, geometry=None):
    """[(surface, surface dict, maps, material)] of models/<base_model>.skinned_model; with
    `geometry`, the posed converted_models under that folder replace the bind-pose ones."""
    root = 'models/%s.skinned_model' % base_model
    settings = gv.config(root + '/settings')
    out = []
    for p in gv.children(root + '/render/'):
        if not p.endswith('/converted_model'):
            continue
        name = p.split('/')[-2]
        mat = gv.string(settings.get('material_settings.%s.material_name' % name, 'string:""'))
        posed = os.path.join(geometry, p) if geometry else None
        raw = open(posed, 'rb').read() if posed and os.path.exists(posed) else gv.read(p)
        out.append((name, surface(raw), material_maps(mat) if mat else {}, mat))
    return out
