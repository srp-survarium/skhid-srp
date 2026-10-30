"""Contact sheets of the tape: per slot, a row per `tape` row of tape.tsv (its model's every surface
and its tape shell), blue and red, at rest from the front, side and back, in the weapon holds,
running, crouching and dead, and two close-ups of the wrap; pages of PAGE rows, plus an index of
every row's status. Renders on the CPU (tape_render)."""
import os
import subprocess
from multiprocessing import Pool
import numpy as np
from PIL import Image, ImageDraw
import bandsenv as E
import bandlib as BL
import model
import pose as PO
import render as RD
import shell as SH
import tapelib as T
import tape as TP
import tape_render as TR

CELL = (150, 200)
CLOSE = (200, 200)
PAGE = 20
# (colour, pose, view): the full-figure cells of a row.
CELLS = [('blue', 'rest', 'front'), ('blue', 'rest', 'side'), ('blue', 'rest', 'back'),
         ('red', 'rifle_aim', 'front'), ('red', 'pistol', 'right'), ('red', 'shotgun', 'front'),
         ('blue', 'run', 'side'), ('red', 'crouch', 'front'), ('blue', 'death', 'front')]
# (colour, side, view): the close-ups of the first limb.
CLOSES = [('blue', 'Right', 'front'), ('red', 'Left', 'back'), ('red', 'Right', 'right')]
LABEL_H = 26

_ATLAS = None


def atlas():
    global _ATLAS
    if _ATLAS is None:
        maps, nrm, rough, _ = T.atlas(T.Roll())
        _ATLAS = ({k: v.astype(np.float32) for k, v in maps.items()}, nrm.astype(np.float32))
    return _ATLAS


def shell_path(row):
    return 'models/%s.skinned_model/render/%s/converted_model' % (TP.shell_model(row['base_model'], row['surface']), TP.SURFACE)


def posed(row, pose, root):
    """(item surfaces, shell surface) in `pose` ('rest': the bind pose)."""
    rel = shell_path(row)
    if pose == 'rest':
        geo = None
        raw = open(os.path.join(root, rel), 'rb').read()
    else:
        geo = PO.posed_dir(pose)
        args = [E.POSER, '--target-db', E.layers()[0]]
        for p in E.layers()[1:]:
            args += ['--patch', p]
        args += ['--torso', row['base_model'], '--output', geo, '--surfaces-root', root, '--surface', rel] + PO.POSES[pose]
        r = subprocess.run(args, capture_output=True, text=True)
        if r.returncode:
            raise RuntimeError(r.stderr[-300:])
        raw = open(os.path.join(geo, rel), 'rb').read()
    parts = [dict(s=s, dif=RD.game_texture(m.get('diffuse')), nrm=RD.game_texture(m.get('normal')))
             for n, s, m, _ in model.model(row['base_model'], geo)]
    return parts, model.surface(raw)


def row_image(row):
    try:
        return _row_image(row)
    except Exception as e:
        img = Image.new('RGB', (len(CELLS) * CELL[0] + len(CLOSES) * CLOSE[0], CELL[1] + LABEL_H), (90, 20, 20))
        ImageDraw.Draw(img).text((4, 4), '%s %s: %s' % (row['base_model'], row['surface'], e), fill=(255, 255, 255))
        return img


def _row_image(row):
    dif, nrm = atlas()
    root = E.work('shells')
    limbs = TP.tape_limbs(row)
    sh, _ = SH.build(row['base_model'], row['surface'], limbs)
    s_stock = {n: s for n, s, _, _ in model.model(row['base_model'])}[row['surface']]
    path = os.path.join(root, shell_path(row))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, 'wb').write(SH.encode(sh, SH.front_sign(s_stock)))
    cells = []
    cache = {}
    for colour, pose, view in CELLS:
        if pose not in cache:
            cache[pose] = posed(row, pose, root)
        parts, shell = cache[pose]
        img, _ = TR.render(parts + [dict(s=shell, dif=dif[colour], nrm=nrm, alpha_test=True, sheen=0.25)],
                           view=view, size=CELL)
        cells.append(img)
    parts, shell = cache['rest']
    names, bp = BL.bind_pose(row['base_model'])
    limb = limbs[0]
    for colour, side, view in CLOSES:
        a, d, L, inner = SH.frame(bp, limb, side)
        c = a + d * SH.LIMB[limb]['at'] * L
        right, up, fwd, _ = RD.VIEWS[view]
        h = 0.10 if limb == 'arm' else 0.13
        img, _ = TR.render(parts + [dict(s=shell, dif=dif[colour], nrm=nrm, alpha_test=True, sheen=0.25)],
                           view=view, size=CLOSE, frame=(c @ right - h, c @ right + h, c @ up - h, c @ up + h))
        cells.append(img)
    W = sum(i.size[0] for i in cells)
    out = Image.new('RGB', (W, CELL[1] + LABEL_H), (28, 28, 30))
    x = 0
    for im in cells:
        out.paste(im, (x, LABEL_H))
        x += im.size[0]
    dr = ImageDraw.Draw(out)
    items = row['items'] if len(row['items']) < 90 else row['items'][:87] + '...'
    dr.text((4, 1), '%s  |  %s  |  %s  |  arm %s  thigh %s  |  %s %s' % (
        row['base_model'], row['surface'], row['limbs'], row['arm_cover_pct'], row['thigh_cover_pct'],
        row['status'], row['notes']), fill=(235, 235, 235))
    dr.text((4, 13), 'items: ' + items, fill=(170, 170, 175))
    return out


def review(a):
    out = a.out or E.work('review')
    os.makedirs(out, exist_ok=True)
    rows = TP.read_table()
    slots = a.slot or TP.SLOTS
    for slot in slots:
        rs = [r for r in rows if r['slot'] == slot and r['method'] == 'tape'
              and (not a.only or any(o in r['base_model'] for o in a.only))]
        if rs:
            with Pool(a.jobs) as p:
                imgs = p.map(row_image, rs, chunksize=1)
            for k in range(0, len(imgs), PAGE):
                page = imgs[k:k + PAGE]
                sheet = Image.new('RGB', (page[0].size[0], sum(i.size[1] for i in page)), (20, 20, 22))
                y = 0
                for im in page:
                    sheet.paste(im, (0, y))
                    y += im.size[1]
                name = os.path.join(out, '%s_%02d.png' % (slot, k // PAGE + 1))
                sheet.save(name)
                print(name)
        # The index: every row of the slot and what it got.
        allrows = [r for r in rows if r['slot'] == slot]
        idx = Image.new('RGB', (1500, 14 * (len(allrows) + 2)), (24, 24, 26))
        dr = ImageDraw.Draw(idx)
        dr.text((4, 2), '%s: %d surfaces, %d with tape' % (slot, len(allrows), sum(r['method'] == 'tape' for r in allrows)),
                fill=(255, 255, 255))
        for i, r in enumerate(allrows):
            colour = {'tape': (120, 220, 130), 'none': (160, 160, 160)}[r['method']]
            if r['status'] in ('flagged', 'off'):
                colour = (240, 170, 60)
            dr.text((4, 16 + 14 * i), '%-5s %-7s %-60s %-34s arm %-7s thigh %-7s %s' % (
                r['method'], r['status'], r['base_model'], r['surface'], r['arm_cover_pct'], r['thigh_cover_pct'],
                r['notes']), fill=colour)
        name = os.path.join(out, '%s_index.png' % slot)
        idx.save(name)
        print(name)
