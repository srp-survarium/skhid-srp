#!/usr/bin/env python3
"""Blackout's duct tape team marks (docs/notes/blackout.md): the per-item table, the content pack
and the review sheets.

usage:
  tape.py table [--jobs N]              measure every arm and leg item surface, write tape.tsv
                                        (keeps each row's status and notes the review gave it)
  tape.py build <packs dir> <version>   the pack blackout-tape-<version>: a shell per `tape` row,
                                        the atlas maps, the two materials, pack.json; prints the
                                        manifest row
  tape.py flags [--jobs N]              flag draft rows whose cloth is close to a tape colour
  tape.py review [--slot S] [--jobs N]  contact sheets per slot under $BANDS_WORK/review (or
                                        --out): every `tape` row in blue and red, poses and views

Inputs from the environment (bandsenv.py; TAPE_SOURCE the tape scan's folder, tape_source.json).
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from PIL import Image, ImageDraw
import bandsenv as E
import gameview as gv
import model
import shell as SH
import tapelib as T

HERE = os.path.dirname(os.path.abspath(__file__))
TABLE = os.path.join(HERE, 'tape.tsv')
SLOTS = ['torso', 'gloves', 'legs']
DEFAULT_PARTS = 'gameplay/bodyparts/default'
DEFAULT_SLOTS = {'torso': 'torso', 'gloves': 'gloves', 'pants': 'legs'}
DIR = 'ported/blackout_tape'
SURFACE = 'tape'
COVER_MIN = 60          # % of the tape's footprint the shell must carry on each side
HEAD = ['slot', 'base_model', 'surface', 'items', 'method', 'limbs', 'arm_faces', 'arm_cover_pct',
        'thigh_faces', 'thigh_cover_pct', 'status', 'notes']
COMMENT = """# Blackout duct tape per item surface (docs/notes/blackout.md, scripts/blackout-tape): one row per
# (base_model, surface) an arm or leg item shows; `method` tape = a tape shell is shipped and the exe
# adds it (patch 65), none = no tape (notes say why). Measured by tape.py table: the shell's faces per
# side and how much of the tape's footprint they carry (cover, % left/right); status by the review
# sheets: draft (not yet reviewed) | good | flagged (with why) | off (reviewed: no tape, kept by later
# measures) | none (nothing to tape). The wrap: one on each upper arm (torso sleeves; a gloves item only where it
# reaches the upper arm) and one on each thigh (legs), centred 0.65 of the way from shoulder to elbow and
# 0.45 from hip to knee.
"""


def shell_model(base, surface):
    return '%s/%s__%s' % (DIR, base.replace('/', '_'), surface)


def inventory():
    """[(slot, base_model, surface, [item names])] of every torso, gloves and legs item."""
    found = {}
    for it in sorted(p for p in gv.IDX if p.startswith('gameplay/items/armour/') and p.endswith('.options')):
        slot = it.split('/')[3]
        if slot not in SLOTS:
            continue
        c = gv.config(it)
        for k, v in c.items():
            if k.endswith('.base_model') and k.split('.')[0].startswith('data') and not k.endswith('_hud.base_model'):
                pre = k[:-len('.base_model')]
                key = (slot, gv.string(v), gv.string(c.get(pre + '.part_name', '')))
                found.setdefault(key, set()).add(it.split('/')[-1][:-len('.options')])
    # What a player wears without an item of the slot: the default body parts.
    parts = gv.config(DEFAULT_PARTS)
    for k, v in parts.items():
        bits = k.split('.')
        if len(bits) == 4 and bits[0] == 'body_parts' and bits[3] == 'base_model' and bits[2].startswith('data'):
            slot = DEFAULT_SLOTS.get(bits[1])
            if slot:
                key = (slot, gv.string(v), gv.string(parts.get('.'.join(bits[:3] + ['part_name']), '')))
                found.setdefault(key, set()).add('(no %s item)' % slot)
    return [(s, b, p, sorted(i)) for (s, b, p), i in sorted(found.items())]


def coverage(sh, limb):
    """Per side, the % of the tape's footprint (its atlas cell's alpha over one turn) the shell's
    triangles round `limb` carry."""
    out = {}
    alpha = T.atlas_alpha()
    row, _ = T.LIMBS[limb]
    for col, side in enumerate(T.SIDES):
        Wc, Hc = T.ATLAS_W // 2 // 4, T.ATLAS_H // 2 // 4
        cell = alpha[row * Hc * 4:(row + 1) * Hc * 4, col * Wc * 4:(col + 1) * Wc * 4][::4, ::4] > 0.5
        hit = np.zeros_like(cell)
        idx = [i for i, s in enumerate(sh['side']) if s == (limb, side)] if sh else []
        if idx:
            keep = np.zeros(len(sh['pos']), bool)
            keep[idx] = True
            for t in sh['tri']:
                if not keep[t].all():
                    continue
                uv = sh['uv'][t]
                x = (uv[:, 0] * 2 - col) * Wc
                y = (uv[:, 1] * 2 - row) * Hc
                x0, x1 = int(max(np.floor(x.min()), 0)), int(min(np.ceil(x.max()), Wc - 1))
                y0, y1 = int(max(np.floor(y.min()), 0)), int(min(np.ceil(y.max()), Hc - 1))
                if x1 < x0 or y1 < y0:
                    continue
                gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
                d = (y[1] - y[2]) * (x[0] - x[2]) + (x[2] - x[1]) * (y[0] - y[2])
                if abs(d) < 1e-12:
                    continue
                l0 = ((y[1] - y[2]) * (gx - x[2]) + (x[2] - x[1]) * (gy - y[2])) / d
                l1 = ((y[2] - y[0]) * (gx - x[2]) + (x[0] - x[2]) * (gy - y[2])) / d
                inside = (l0 >= -0.02) & (l1 >= -0.02) & (1 - l0 - l1 >= -0.02)
                hit[y0:y1 + 1, x0:x1 + 1] |= inside
        # One turn: fold the margins onto the turn they repeat.
        m = int(round(T.MARGIN / (1 + 2 * T.MARGIN) * Wc))
        turn = slice(m, Wc - m)
        h = hit[:, turn].copy()
        n = h.shape[1]
        h[:, n - m:] |= hit[:, :m]
        h[:, :m] |= hit[:, Wc - m:]
        foot = cell[:, turn]
        out[side] = int(round(100 * (h & foot).sum() / max(foot.sum(), 1)))
    return out


def measure(job):
    """Per limb the slot allows: faces and cover per side; `limbs` those whose cover passes."""
    slot, base, surface, items = job
    res = dict(slot=slot, base_model=base, surface=surface, items=items, limb={})
    try:
        for limb in SH.SLOT_LIMBS[slot]:
            sh, faces = SH.build(base, surface, (limb,))
            res['limb'][limb] = dict(faces=[faces[(limb, s)] for s in T.SIDES], cover=list(coverage(sh, limb).values()))
    except Exception as e:
        res['error'] = '%s: %s' % (type(e).__name__, e)
    return res


def tape_limbs(row):
    """The limbs a row's shell goes round (its `limbs` column)."""
    return tuple(l for l in row['limbs'].split('+') if l) if row.get('limbs') not in (None, '', '-') else ()


def read_table(path=TABLE):
    if not os.path.exists(path):
        return []
    lines = [l for l in open(path).read().splitlines() if l and not l.startswith('#')]
    head = lines[0].split('\t')
    return [dict(zip(head, l.split('\t'))) for l in lines[1:]]


def cmd_table(a):
    old = {(r['base_model'], r['surface']): r for r in read_table()}
    jobs = inventory()
    with Pool(a.jobs) as p:
        res = p.map(measure, jobs)
    rows = []
    for r in res:
        key = (r['base_model'], r['surface'])
        prev = old.get(key, {})
        row = dict(slot=r['slot'], base_model=key[0], surface=key[1], items=','.join(r['items']))
        for limb in ('arm', 'thigh'):
            m = r['limb'].get(limb)
            row[limb + '_faces'] = '%d/%d' % tuple(m['faces']) if m else '-'
            row[limb + '_cover_pct'] = '%d/%d' % tuple(m['cover']) if m else '-'
        ok = [l for l, m in r['limb'].items() if min(m['cover']) >= COVER_MIN]
        if 'error' in r:
            row.update(method='none', limbs='-', status='none', notes='no shell: ' + r['error'])
        elif prev.get('status') == 'off':
            # The review's decision: no tape here (notes say why); kept across measures.
            row.update(method='none', limbs='-', status='off', notes=prev.get('notes', ''))
        elif ok:
            status = prev.get('status', 'draft')
            row.update(method='tape', limbs='+'.join(ok), status='draft' if status == 'none' else status,
                       notes=prev.get('notes', '') if prev.get('method') == 'tape' else '')
        else:
            where = {'arm': 'upper arm', 'thigh': 'thigh'}
            best = max(r['limb'].items(), key=lambda kv: max(kv[1]['faces']))
            if max(best[1]['faces']) == 0:
                notes = ('does not reach the upper arm (the wrap is on the torso sleeve)' if r['slot'] == 'gloves'
                         else 'no cloth of its own round the %s' % ' or '.join(where[l] for l in r['limb']))
            else:
                notes = 'its cloth carries %d%% / %d%% of the tape round the %s (< %d%%)' % (
                    best[1]['cover'][0], best[1]['cover'][1], where[best[0]], COVER_MIN)
            if prev.get('method') == 'none' and prev.get('notes') and prev.get('status') != 'none':
                notes = prev['notes']
            row.update(method='none', limbs='-', status='none', notes=notes)
        rows.append(row)
    rows.sort(key=lambda r: (SLOTS.index(r['slot']), r['base_model'], r['surface']))
    with open(TABLE, 'w') as f:
        f.write(COMMENT)
        f.write('\t'.join(HEAD) + '\n')
        for r in rows:
            f.write('\t'.join(r[h] for h in HEAD) + '\n')
    for slot in SLOTS:
        rs = [r for r in rows if r['slot'] == slot]
        print('%-6s %3d surfaces, %3d tape, %3d none' % (slot, len(rs), sum(r['method'] == 'tape' for r in rs),
                                                         sum(r['method'] == 'none' for r in rs)))


# ---- the pack ----------------------------------------------------------------------------------

def _config_lib():
    sys.path.insert(0, os.path.join(HERE, '..', 'content-edit'))
    import content_edit as CE
    return CE


def _set_float3(CE, buf, path, values):
    off = CE.find(buf, path)
    data = CE.rec(buf, off)[0]
    buf[data:data + 12] = np.asarray(values, '<f4').tobytes()


def _set_int(CE, buf, path, value):
    off = CE.find(buf, path)
    data, i, c, t, n = CE.rec(buf, off)
    CE.REC.pack_into(buf, off, (data & ~0xffffffff) | (int(value) & 0xffffffff), i, c, t, n)


def _set_string(CE, buf, path, text):
    off = CE.find(buf, path)
    d, i, c, t, n = CE.rec(buf, off)
    noff = CE.append(buf, text.encode() + b'\0')
    CE.REC.pack_into(buf, off, noff, i, c, t, len(text.encode()) + 1)


TEMPLATE_MODEL = 'models/character/human/base/base_character_torso_s1.skinned_model'
TEMPLATE_MATERIAL = 'material_instances/characters/human/edge_hunter_01/edge_hunter_01_torso_alpha.material'
TEMPLATE_OPTIONS = {
    'diffuse': 'textures/character/human/edge_hunter_01/edge_hunter_01_torso.options',
    'normal': 'textures/character/human/edge_hunter_01/edge_hunter_01_torso_nmap.options',
    'roughness': 'textures/character/human/edge_hunter_01/edge_hunter_01_torso_smap.options',
    'fresnel': 'textures/character/human/edge_hunter_01/edge_hunter_01_torso_fmap.options',
}


def _dds(rgba_or_l, path, fmt, srgb_note=''):
    """A DDS with mips by ImageMagick (DXT1 or DXT5), the stock files' layout."""
    with tempfile.TemporaryDirectory() as tmp:
        src = os.path.join(tmp, 'in.png')
        Image.fromarray(rgba_or_l).save(src)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        r = subprocess.run(['magick', src, '-define', 'dds:compression=%s' % fmt,
                            path], capture_output=True, text=True)
        if r.returncode:
            raise SystemExit('ImageMagick: ' + r.stderr)


def _model_files(CE, base, surface, sh, out):
    """models/<shell>.skinned_model/...: the shell surface, the item model's own bind pose and
    skeleton, settings and export properties after the stock template's."""
    name = shell_model(base, surface)
    root = os.path.join(out, 'files', 'models', name + '.skinned_model')
    stock = 'models/%s.skinned_model' % base
    s_stock = {n: s for n, s, _, _ in model.model(base)}[surface]
    raw = SH.encode(sh, SH.front_sign(s_stock))
    os.makedirs(os.path.join(root, 'render', SURFACE), exist_ok=True)
    open(os.path.join(root, 'render', SURFACE, 'converted_model'), 'wb').write(raw)
    open(os.path.join(root, 'render', 'bind_pose'), 'wb').write(gv.read(stock + '/render/bind_pose'))
    open(os.path.join(root, 'skeleton'), 'wb').write(gv.read(stock + '/skeleton'))
    lo, hi = sh['pos'].min(0), sh['pos'].max(0)
    # settings: the template's one surface renamed, its material the blue tape (the exe sets the
    # team's on the record).
    st = bytearray(gv.read(TEMPLATE_MODEL + '/settings'))
    tsurf = _template_surface()
    assert CE.rename_key(st, 'material_settings', tsurf, SURFACE)
    _set_string(CE, st, 'material_settings/%s/material_name' % SURFACE, '%s/tape_blue' % DIR)
    lod = [p for p, t, v in CE.walk(st) if t == CE.T_STR and v == tsurf.encode()]
    for p in lod:
        _set_string(CE, st, p.lstrip('/'), SURFACE)
    open(os.path.join(root, 'settings'), 'wb').write(st)
    ep = bytearray(gv.read(TEMPLATE_MODEL + '/render/export_properties'))
    _set_string(CE, ep, 'src_filename', 'blackout-tape/' + base)
    _set_float3(CE, ep, 'bounding_box/max', hi)
    _set_float3(CE, ep, 'bounding_box/min', lo)
    open(os.path.join(root, 'render', 'export_properties'), 'wb').write(ep)
    sp = bytearray(gv.read(TEMPLATE_MODEL + '/render/%s/export_properties' % tsurf))
    n_idx = len(sh['tri']) * 3
    _set_int(CE, sp, 'source/indices', n_idx)
    _set_int(CE, sp, 'source/primitives', len(sh['tri']))
    _set_int(CE, sp, 'source/vertices', len(sh['pos']))
    _set_string(CE, sp, 'source/mesh_name', '|%s|%s' % (base, SURFACE))
    _set_string(CE, sp, 'source/texture_name', '%s/tape_blue' % DIR)
    _set_float3(CE, sp, 'bounding_box/max', hi)
    _set_float3(CE, sp, 'bounding_box/min', lo)
    open(os.path.join(root, 'render', SURFACE, 'export_properties'), 'wb').write(sp)
    return name


def _template_surface():
    s = gv.config(TEMPLATE_MODEL + '/settings')
    names = sorted({k.split('.')[1] for k in s if k.startswith('material_settings.')})
    return names[0]


def cmd_build(a):
    CE = _config_lib()
    out = os.path.join(a.packs, 'blackout-tape-%s' % a.version)
    if os.path.exists(out):
        raise SystemExit('%s: already there; never overwritten' % out)
    rows = [r for r in read_table() if r['method'] == 'tape']
    os.makedirs(out)
    files = os.path.join(out, 'files')
    # The atlas maps and their options.
    roll = T.Roll()
    maps, nrm, rough, _ = T.atlas(roll)
    tex = os.path.join(files, 'textures', DIR)
    for colour, rgba in maps.items():
        _dds(rgba, os.path.join(tex, 'tape_%s_diffuse.dds' % colour), 'dxt5')
        open(os.path.join(tex, 'tape_%s_diffuse.options' % colour), 'wb').write(gv.read(TEMPLATE_OPTIONS['diffuse']))
    _dds(nrm, os.path.join(tex, 'tape_normal.dds'), 'dxt5')
    open(os.path.join(tex, 'tape_normal.options'), 'wb').write(gv.read(TEMPLATE_OPTIONS['normal']))
    _dds(rough, os.path.join(tex, 'tape_roughness.dds'), 'dxt1')
    open(os.path.join(tex, 'tape_roughness.options'), 'wb').write(gv.read(TEMPLATE_OPTIONS['roughness']))
    # The fresnel map is sampled in the tape's own UVs: a flat one, the cloth's mean.
    fm = Image.open(gv.local('textures/' + TEMPLATE_OPTIONS['fresnel'][len('textures/'):-len('.options')] + '.dds')).convert('RGB')
    mean = np.asarray(fm, np.float32).reshape(-1, 3).mean(0)
    _dds(np.tile(mean.round().astype(np.uint8), (16, 16, 1)), os.path.join(tex, 'tape_fresnel.dds'), 'dxt1')
    open(os.path.join(tex, 'tape_fresnel.options'), 'wb').write(gv.read(TEMPLATE_OPTIONS['fresnel']))
    # The two materials: the stock alpha-tested character material with the tape's maps.
    mdir = os.path.join(files, 'material_instances', DIR)
    os.makedirs(mdir, exist_ok=True)
    leaves = gv.config(TEMPLATE_MATERIAL)
    get = lambda k: gv.string(leaves['material.g_stage.effect.%s.value' % k])
    for colour in maps:
        m = bytearray(gv.read(TEMPLATE_MATERIAL))
        swaps = {get('texture_diffuse'): '%s/tape_%s_diffuse' % (DIR, colour),
                 get('texture_normal'): '%s/tape_normal' % DIR,
                 get('texture_alphablended_normal'): '%s/tape_normal' % DIR,
                 get('texture_roughness'): '%s/tape_roughness' % DIR,
                 get('texture_fresnel'): '%s/tape_fresnel' % DIR}
        for frm, to in swaps.items():
            CE.replace_string(m, frm, to)
        open(os.path.join(mdir, 'tape_%s.material' % colour), 'wb').write(m)
    # The shells.
    made = []
    for r in rows:
        sh, _ = SH.build(r['base_model'], r['surface'], tape_limbs(r))
        made.append(_model_files(CE, r['base_model'], r['surface'], sh, out))
    paths = sorted(os.path.relpath(os.path.join(d, f), files) for d, _, fs in os.walk(files) for f in fs)
    for p in paths:
        if p in gv.IDX:
            raise SystemExit('%s: the game has it already' % p)
    manifest = {
        'format_version': 1,
        'id': 'blackout-tape',
        'version': str(a.version),
        'target_build': '0.26g',
        'description': 'Blackout duct tape team marks: a tape shell for each arm and leg item surface of '
                       'scripts/blackout-tape/tape.tsv (%d), the tape atlas in blue and red from %s, '
                       'its two materials; exe patch 65 adds the shell in the player\'s team colour'
                       % (len(made), roll.source),
        'operations': [{'op': 'add_file', 'path': p} for p in paths],
    }
    open(os.path.join(out, 'pack.json'), 'w').write(json.dumps(manifest, indent=2) + '\n')
    size = sum(os.path.getsize(os.path.join(files, p)) for p in paths)
    sha = hashlib.sha256(open(os.path.join(out, 'pack.json'), 'rb').read()).hexdigest()
    print('pack %s: %d files, %.1f MB, %d shells' % (out, len(paths), size / 1e6, len(made)))
    print('row:\tpack\tblackout-tape@%s\t%s' % (a.version, sha))


# ---- flags ---------------------------------------------------------------------------------------

CLOSE_DE = 25.0         # a cloth texel this near a tape colour (CIELAB) hides it
CLOSE_MAX = 0.35        # flag a row when more of its cloth under the tape is that near


def _lab(rgb):
    c = np.asarray(rgb, float) / 255.0
    c = np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    xyz = c @ np.array([[0.4124, 0.2126, 0.0193], [0.3576, 0.7152, 0.1192], [0.1805, 0.0722, 0.9505]])
    xyz = xyz / np.array([0.9505, 1.0, 1.089])
    f = np.where(xyz > 0.008856, np.cbrt(xyz), 7.787 * xyz + 16 / 116)
    return np.stack([116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]), 200 * (f[..., 1] - f[..., 2])], -1)


def contrast(row):
    """{colour: the share of the cloth under the tape (the item's own diffuse at the shell's
    faces) within CLOSE_DE of that tape colour}."""
    import bandlib as BL
    names, bp = BL.bind_pose(row['base_model'])
    parts = {n: (x, m) for n, x, m, _ in model.model(row['base_model'])}
    s, maps = parts[row['surface']]
    dif = BL.texture(maps['diffuse'])[..., :3]
    h, w = dif.shape[:2]
    px = []
    for limb in tape_limbs(row):
        for side in T.SIDES:
            el, _, _ = SH.limb_faces(s, names, bp, limb, side)
            uv = s['uv'][s['tri'][el]]
            for b in ([1 / 3, 1 / 3, 1 / 3], [0.6, 0.2, 0.2], [0.2, 0.6, 0.2], [0.2, 0.2, 0.6]):
                p = (np.asarray(b) @ uv) % 1.0
                px.append(dif[(p[:, 1] * h).astype(int).clip(0, h - 1), (p[:, 0] * w).astype(int).clip(0, w - 1)])
    lab = _lab(np.concatenate(px))
    return {c: float((np.linalg.norm(lab - _lab(np.asarray(rgb, float)), axis=1) < CLOSE_DE).mean())
            for c, rgb in T.TEAM.items()}


def cmd_flags(a):
    """Flag `draft` rows whose cloth is near a tape colour (and say so in notes)."""
    rows = read_table()
    todo = [r for r in rows if r['method'] == 'tape' and r['status'] == 'draft']
    with Pool(a.jobs) as p:
        res = p.map(contrast, todo)
    for r, c in zip(todo, res):
        low = [k for k, v in c.items() if v > CLOSE_MAX]
        if low:
            r['status'] = 'flagged'
            r['notes'] = '; '.join(filter(None, [r['notes'], 'little contrast: %s of the cloth under the tape is near the %s tape'
                                                 % (' / '.join('%d%%' % round(100 * c[k]) for k in low), ' and '.join(low))]))
    with open(TABLE, 'w') as f:
        f.write(COMMENT)
        f.write('\t'.join(HEAD) + '\n')
        for r in rows:
            f.write('\t'.join(r[h] for h in HEAD) + '\n')
    print('flagged', sum(r['status'] == 'flagged' for r in rows))


# ---- review sheets -----------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='cmd', required=True)
    t = sub.add_parser('table')
    t.add_argument('--jobs', type=int, default=6)
    b = sub.add_parser('build')
    b.add_argument('packs')
    b.add_argument('version')
    f = sub.add_parser('flags')
    f.add_argument('--jobs', type=int, default=6)
    r = sub.add_parser('review')
    r.add_argument('--slot', action='append')
    r.add_argument('--jobs', type=int, default=6)
    r.add_argument('--out')
    r.add_argument('--only', action='append')
    a = ap.parse_args()
    if a.cmd == 'table':
        cmd_table(a)
    elif a.cmd == 'build':
        cmd_build(a)
    elif a.cmd == 'flags':
        cmd_flags(a)
    else:
        import tape_review
        tape_review.review(a)


if __name__ == '__main__':
    main()
