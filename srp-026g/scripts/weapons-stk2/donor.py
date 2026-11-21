#!/usr/bin/env python3
"""The STK2 AK-74s' files in 0.26g's layouts: the donor tree resource-porter's `port-weapons`
ports them from (donor build "stk2"; docs/notes/weapons-stk2.md).

    python3 -I donor.py --porter <vostok-port> --stock <resources.db> --stk2 <stk2-ak74> \
        --reload-from <config> --magick <magick> --out <tree>

<stk2-ak74>: the files of /mnt/toshiba/surv-builds/STK2.7z under xr2-prog/resources/: exported/models/
item/weapon/ak74/ak74_4tir*.model (each a resource db) and sources/textures/item/weapon/ak74/*.tga.

Per gun of guns.json, from the live AK-74M (the stock files):
- the model: STK2's surfaces as they are (STK2 draws the gun in the AK-74M's frame), each mesh
  island on the bone guns.json gives it, one bone a vertex (36-byte vertices as the AK-74M's);
  the AK-74M's skeleton, bind pose and locators, but the eye (`aim`, on the line from the rear
  sight's notch to the front post, where the AK-74M's eye is along the bore) and the muzzle
  (`barrel_point`);
- materials: the AK-74M's metal or polymer one per STK2 texture set, its four textures STK2's:
  the diffuse; the normal map (STK2's RGB tangent space) as 0.26g's DXT5nm (X in alpha, Y in
  green); STK2's one specular map as both the fresnel (F0) and the roughness map, each matched
  to the AK-74M's own map's distribution (STK2's brighter specular stays brighter, at the
  AK-74M's levels);
- the item config: the stock AK-74M's with the live AK-74M's two-clip reload (its tactical and
  empty clips, pack tactical-clips-029's: --reload-from, the AK-74M's config as that pack makes it;
  reload-timings-1 then retimes them for each gun as for the AK-74M); the port sets what differs.

Needs NumPy and Pillow; magick (ImageMagick 7 with DDS: resource-porter's dev shell's).
"""
import argparse
import json
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.sparse import coo_matrix
from scipy.sparse.csgraph import connected_components

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import content_edit as ce  # noqa: E402
import model as M  # noqa: E402

GUNS = json.loads((HERE / 'guns.json').read_text())
HALF = np.array([-1.0, 1.0, -1.0])


def config_bytes(value, vectors=True):
    """A binary config of a JSON-like value (with `vectors`, lists of 2 to 4 numbers are vectors)."""
    buf = bytearray(24)
    buf[:24] = ce.value_record(buf, ce.append(buf, b'\0'), '', ce.as_table(value, vectors=vectors))
    return bytes(buf)


def item_config(stock, reload_from):
    """The stock AK-74M's config with the live AK-74M's reload state (it has no vectors)."""
    cfg = M.config(stock.get(GUNS['template']['config']))
    reload = M.config(reload_from)['states']['reload']
    if not reload['user_animations'][0].endswith('/reload_tactic'):
        sys.exit(f'{reload_from}: not the two-clip reload of pack tactical-clips-029')
    cfg['states']['reload'] = reload
    return cfg, config_bytes(cfg, vectors=False)


class Stock:
    def __init__(self, porter, db, work):
        self.porter, self.db, self.work = porter, db, work

    def get(self, path):
        dest = self.work / path
        if not dest.exists():
            dest.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run([self.porter, 'extract', '--db', self.db, '--path', path, '--output', str(dest)],
                           check=True, stdout=subprocess.DEVNULL)
        return dest


def stk2_surfaces(porter, model_db, work):
    """{surface: arrays} of an exported STK2 static model (56-byte vertices: position, normal,
    tangent, binormal, uv; 16-bit indices)."""
    listing = subprocess.run([porter, 'list', '--db', str(model_db), '--prefix', 'render/'],
                             check=True, capture_output=True, text=True).stdout.split()
    out = {}
    for name in sorted({p.split('/')[1] for p in listing if p.count('/') == 2}):
        files = {}
        for leaf in ('vertices', 'indices', 'export_properties'):
            dest = work / model_db.name / name / leaf
            dest.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run([porter, 'extract', '--db', str(model_db), '--path', f'render/{name}/{leaf}',
                            '--output', str(dest)], check=True, stdout=subprocess.DEVNULL)
            files[leaf] = dest.read_bytes()
        vb, ib = files['vertices'], files['indices']
        n = struct.unpack_from('<I', vb)[0]
        if len(vb) != 4 + 56 * n:
            sys.exit(f'{model_db.name} {name}: not 56-byte static vertices')
        cols = np.frombuffer(vb, '<f4', n * 14, 4).reshape(n, 14).astype(np.float64)
        m = struct.unpack_from('<I', ib)[0]
        tri = np.frombuffer(ib, '<u2', m, 4).reshape(-1, 3).astype(np.int64)
        texture = re.search(rb'texture_name = "([^"]*)"', files['export_properties']).group(1).decode()
        out[name] = dict(pos=cols[:, 0:3], nrm=cols[:, 3:6], tan=cols[:, 6:9], bin=cols[:, 9:12],
                         uv=cols[:, 12:14], tri=tri, texture=texture.rsplit('/', 1)[1])
    return out


def islands(pos, tri):
    """Each vertex's island: triangles joined by shared corners (texture seams welded)."""
    _, weld = np.unique(np.round(pos / 1e-5).astype(np.int64), axis=0, return_inverse=True)
    weld = weld.ravel()
    t = weld[tri]
    a, b = np.r_[t[:, 0], t[:, 1], t[:, 2]], np.r_[t[:, 1], t[:, 2], t[:, 0]]
    _, label = connected_components(coo_matrix((np.ones(len(a)), (a, b)), shape=(weld.max() + 1,) * 2),
                                    directed=False)
    return label[weld]


def bone_of_vertices(surface, bones, names):
    """The bone index of each vertex: its island's, by the boxes of guns.json."""
    label = islands(surface['pos'], surface['tri'])
    index = np.full(len(label), names.index('offset'))
    for i in np.unique(label):
        p = surface['pos'][label == i]
        lo, hi = p.min(0), p.max(0)
        for rule in bones:
            box = np.asarray(rule['box'])
            if (lo >= box[0]).all() and (hi <= box[1]).all():
                index[label == i] = names.index(rule['bone'])
                break
    return index


def packed(v, w):
    """0.26g's basis bytes z, y, x, w of a unit vector."""
    v = v / np.maximum(np.linalg.norm(v, axis=1, keepdims=True), 1e-12)
    c = np.clip(np.floor((v + 1) * 127.5), 0, 255).astype(np.uint8)
    return np.stack([c[:, 2], c[:, 1], c[:, 0], w], axis=1)


def uv_fraction(x):
    r = x * 1024
    return np.clip(np.floor(255.5 * (r - np.floor(r))), 0, 255).astype(np.uint8)


def converted_model(surface, bones):
    """A one-bone 0.26g skinned surface: chunk 3 the vertices, chunk 4 the indices."""
    n = len(surface['pos'])
    raw = np.zeros((n, 36), np.uint8)
    raw[:, 0:12] = surface['pos'].astype('<f4').view(np.uint8).reshape(n, 12)
    raw[:, 12:16] = bones[:, None].astype(np.uint8)
    raw[:, 16:20] = packed(surface['nrm'], np.full(n, 127, np.uint8))
    raw[:, 20:24] = packed(surface['tan'], uv_fraction(surface['uv'][:, 0]))
    raw[:, 24:28] = packed(surface['bin'], uv_fraction(surface['uv'][:, 1]))
    raw[:, 28:36] = surface['uv'].astype('<f4').view(np.uint8).reshape(n, 8)
    vertices = struct.pack('<I', n) + raw.tobytes()
    indices = surface['tri'].astype('<u2').tobytes()
    indices = struct.pack('<I', surface['tri'].size) + indices
    return struct.pack('<2I', 3, len(vertices)) + vertices + struct.pack('<2I', 4, len(indices)) + indices


def turned_box(points):
    """A bounding box in the locators' frame (the model's, turned half about Y), as the stock
    export_properties write it."""
    p = points * HALF
    return {'max': p.max(0).tolist(), 'min': p.min(0).tolist()}


def aim_locator(notch, post, eye_z):
    """The eye on the sight line, at the AK-74M's eye's place along the bore, looking along it:
    the locator (frame turned half about Y) whose rotation about X is the line's drop."""
    notch, post = np.asarray(notch), np.asarray(post)
    slope = (post[1] - notch[1]) / (post[2] - notch[2])
    y = notch[1] + (eye_z - notch[2]) * slope
    return [0.0, float(y), float(-eye_z)], [float(np.arctan(-slope)), 0.0, 0.0]


def matched(values, reference):
    """`values` mapped onto `reference`'s distribution, by rank."""
    order = np.argsort(values, kind='stable')
    reference = np.sort(reference)
    out = np.empty(len(values))
    out[order] = reference[np.linspace(0, len(reference) - 1, len(values)).round().astype(np.int64)]
    return out


def write_dds(magick, image, out, compression, srgb):
    """A texture as 0.26g's cooked ones are: its mips down to 4x4, the header's PITCH and
    LINEARSIZE flags with the size of all that follows the header, and the trailing sRGB byte the loader takes
    off the end (read_srgb_flag). ImageMagick's own DDS (mips to 1x1, the top level's size, no
    sRGB byte) shows as garbage colours in the game's lower mips."""
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        src = Path(tmp) / 'in.png'
        image.save(src)
        subprocess.run([magick, str(src), '-define', f'dds:compression={compression}', str(out)], check=True)
    data = bytearray(out.read_bytes())
    height, width = struct.unpack_from('<2I', data, 12)
    block = 8 if compression == 'dxt1' else 16
    levels, payload = 0, 0
    while True:
        payload += max(1, width >> levels >> 2) * max(1, height >> levels >> 2) * block
        levels += 1
        if (width >> (levels - 1)) <= 4 or (height >> (levels - 1)) <= 4:
            break
    if len(data) < 128 + payload or struct.unpack_from('<I', data, 28)[0] < levels:
        sys.exit(f'{out}: {len(data)} bytes, not {levels} mips')
    data = data[:128 + payload]
    struct.pack_into('<I', data, 8, 0xA100F)
    struct.pack_into('<I', data, 20, payload + 1)  # with the sRGB byte, as the stock files
    struct.pack_into('<I', data, 28, levels)
    data[32:76] = bytes(44)
    out.write_bytes(bytes(data) + bytes([srgb]))


def textures(args, stock, out):
    """Each STK2 texture set's diffuse, DXT5nm normal, fresnel and roughness maps."""
    t = GUNS['template']['textures']
    ref = {k: np.asarray(Image.open(stock.get(t[k] + '.dds')).convert('RGB'), np.float64) for k in ('fresnel', 'roughness')}
    folder = GUNS['textures']['folder']
    src = args.stk2 / 'sources/textures/item/weapon/ak74'
    written = {}
    for name in GUNS['textures']['sets']:
        diffuse = Image.open(src / f'{name}.tga').convert('RGB')
        nmap = np.asarray(Image.open(src / f'{name}_nmap.tga').convert('RGB'))
        spec = np.asarray(Image.open(src / f'{name}_spec.tga').convert('L'), np.float64).ravel()
        base = f'textures/{folder}/{name.replace("ak74", "ak74_tactical")}'
        write_dds(args.magick, diffuse, out / f'{base}.dds', 'dxt1', 1)
        dxt5nm = np.zeros(nmap.shape[:2] + (4,), np.uint8)
        dxt5nm[..., 1], dxt5nm[..., 3] = nmap[..., 1], nmap[..., 0]
        write_dds(args.magick, Image.fromarray(dxt5nm, 'RGBA'), out / f'{base}_nmap.dds', 'dxt5', 0)
        for kind, suffix in (('fresnel', '_fmap'), ('roughness', '_smap')):
            level = matched(spec, ref[kind].mean(2).ravel()).reshape(nmap.shape[:2])
            grey = np.repeat(np.clip(np.round(level), 0, 255).astype(np.uint8)[..., None], 3, 2)
            write_dds(args.magick, Image.fromarray(grey, 'RGB'), out / f'{base}{suffix}.dds', 'dxt1', 0)
        for suffix, kind in (('', 'diffuse'), ('_nmap', 'normal'), ('_fmap', 'fresnel'), ('_smap', 'roughness')):
            shutil.copyfile(stock.get(t[kind] + '.options'), out / f'{base}{suffix}.options')
        written[name] = base.removeprefix('textures/')
    return written


def material(stock, kind, texture, out_path):
    """The AK-74M's material of `kind` with the texture set's four maps."""
    path = GUNS['template']['materials'][kind]
    buf = bytearray(stock.get(path).read_bytes())
    live = M.config(stock.get(path))['material']['g_stage']['effect']
    for key, suffix in (('texture_diffuse', ''), ('texture_normal', '_nmap'), ('texture_fresnel', '_fmap'),
                        ('texture_roughness', '_smap')):
        ce.edit_text(buf, f'material/g_stage/effect/{key}/value', live[key]['value'], texture + suffix)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_bytes(bytes(buf))


def gun(args, stock, name, spec, texture_paths, out):
    template = GUNS['template']
    tdir = f'models/{template["model"]}.skinned_model'
    model_dir = out / f'models/{spec["model"]}.skinned_model'
    (model_dir / 'render').mkdir(parents=True)
    for leaf in ('skeleton', 'render/bind_pose'):
        shutil.copyfile(stock.get(f'{tdir}/{leaf}'), model_dir / leaf)
    names = _bind_names(stock.get(f'{tdir}/render/bind_pose'))
    model_db = args.stk2 / f'exported/models/item/weapon/ak74/{spec["stk2"]}.model'
    surfaces = stk2_surfaces(args.porter, model_db, args.work)
    surface_props = M.config(stock.get(f'{tdir}/render/{template["surface"]}/export_properties'))
    lods, materials, every = [], {}, []
    for surface, s in surfaces.items():
        if surface in spec.get('leave_out', []):
            continue
        bones = bone_of_vertices(s, GUNS['bones'], names)
        (model_dir / 'render' / surface).mkdir()
        (model_dir / 'render' / surface / 'converted_model').write_bytes(converted_model(s, bones))
        props = json.loads(json.dumps(surface_props))
        props['source'] = dict(indices=int(s['tri'].size), primitives=len(s['tri']), mesh_name=f'|{spec["stk2"]}|{surface}',
                               texture_name=texture_paths[s['texture']], vertices=len(s['pos']))
        props['bounding_box'] = turned_box(s['pos'])
        (model_dir / 'render' / surface / 'export_properties').write_bytes(config_bytes(props))
        lods.append(surface)
        materials[surface] = {'material_name': texture_paths[s['texture']].replace('weapons/ak74_tactical/', 'weapons/'),
                              'used_in_portals': False}
        every.append(s['pos'])
    settings = M.config(stock.get(f'{tdir}/settings'))
    settings['lod_hierrarchy']['LOD0']['surfaces'] = lods
    settings['material_settings'] = materials
    (model_dir / 'settings').write_bytes(config_bytes(settings))
    props = M.config(stock.get(f'{tdir}/render/export_properties'))
    props['src_filename'] = f'STK2 xr2-prog exported/models/item/weapon/ak74/{spec["stk2"]}.model'
    props['bounding_box'] = turned_box(np.concatenate(every))
    eye_z = -next(l for l in props['locators'] if l['name'] == 'aim')['position'][2]
    for locator in props['locators']:
        if locator['name'] == 'aim':
            locator['position'], locator['rotation'] = aim_locator(spec['notch'], spec['post'], eye_z)
        elif locator['name'] == 'barrel_point':
            locator['position'] = (np.asarray(spec['muzzle']) * HALF).tolist()
    (model_dir / 'render/export_properties').write_bytes(config_bytes(props))


def _bind_names(path):
    data = path.read_bytes()
    names, at = [], 2
    for _ in range(struct.unpack_from('<H', data)[0]):
        end = data.index(0, at)
        names.append(data[at:end].decode())
        at = end + 37
    return names


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--porter', required=True)
    ap.add_argument('--stock', required=True)
    ap.add_argument('--stk2', type=Path, required=True)
    ap.add_argument('--reload-from', type=Path, required=True)
    ap.add_argument('--magick', required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out} is already there')
    args.out.mkdir(parents=True)
    with tempfile.TemporaryDirectory() as tmp:
        args.work = Path(tmp)
        stock = Stock(args.porter, args.stock, args.work / 'stock')
        texture_paths = textures(args, stock, args.out)
        (args.out / 'donor').mkdir()
        for name, spec in GUNS['guns'].items():
            gun(args, stock, name, spec, texture_paths, args.out)
            cfg, data = item_config(stock, args.reload_from)
            (args.out / f'donor/{name}.options').write_bytes(data)
            if M.config(args.out / f'donor/{name}.options') != cfg:
                sys.exit('the item config does not read back')
        for stk2_set, kind in GUNS['textures']['sets'].items():
            path = texture_paths[stk2_set]
            material(stock, kind, path, args.out / f'material_instances/{path.replace("weapons/ak74_tactical/", "weapons/")}.material')
    print(f'{args.out}: {sum(1 for p in args.out.rglob("*") if p.is_file())} files')


if __name__ == '__main__':
    main()
