"""The previews of the battery swing (private/renders/battery-hit/):

- carry_idle_fp.png: the stock carry idle in first person, looking level and
  20 and 35 degrees down;
- battery_hit_before_after(_slow).gif, battery_hit_frames.png: the current
  swing (the put's clips, player and battery, played in 0.3 s, then the carry
  idle) left, the authored swing right, first person over third person;
- battery_hit_tp_<view>.png: every frame of the authored swing in third person,
  large, per view (front three-quarter, right side, front);
- metrics.txt: per frame what first person shows and what would clip.

    python preview.py <assets folder> <out folder>
"""
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

import clips
import render
import swing
from rig import Rig, HANDS, matrix

rig = Rig(sys.argv[1])
out = Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
idle = rig.clip(HANDS + 'idle')
put = rig.clip(HANDS + 'hidetest')
S = swing.Swing(rig, idle)
frames = swing.author(rig, idle)
sw = swing.write(rig, idle, frames, 0, swing.IMPACT)
rc = swing.write(rig, idle, frames, swing.IMPACT, swing.END)
FP_SIZE = (640, 360)
CORE = {'Hip', 'Spin', 'Spin_1', 'Chest', 'Neck', 'Head'}


def core_vertices(world):
    pts = []
    for m in rig.body:
        v = m.pose(world)
        dom = [m.names[i] if i < len(m.names) else '' for i in m.indices[np.arange(len(v)), np.argmax(m.weights, axis=1)]]
        pts.append(v[np.array([d in CORE for d in dom])])
    return np.vstack(pts)


def battery(world, clip=None, t=0):
    bw = rig.battery_world(world, t, clip)
    return rig.battery.pose(bw), rig.battery.triangles, rig.battery.color


def on_screen(points, eye, axes, size=FP_SIZE):
    v = (points - eye) @ np.array(axes).T
    focal = size[1] / (2 * math.tan(math.radians(render.FOV) / 2))
    s = v[:, :2] / np.maximum(v[:, 2:3], 1e-6) * focal
    return (v[:, 2] > render.NEAR) & (np.abs(s[:, 0]) < size[0] / 2) & (np.abs(s[:, 1]) < size[1] / 2)


def first_person(world, bat, label):
    eye, r = render.fp_camera(world)
    img, near = render.draw([(m.pose(world), m.triangles, m.color) for m in rig.fp] + [bat], eye, r, size=FP_SIZE)
    shown = on_screen(bat[0], eye, r).mean()
    hands = ''.join(s[0] if on_screen(np.array([world[s + 'Hand'][1], world[s + 'Middle2'][1]]), eye, r).any() else '-'
                    for s in ('Left', 'Right'))
    ImageDraw.Draw(img).text((6, 4), '%s  FP  battery on screen %d%%  hands %s  near-clip %d'
                             % (label, 100 * shown, hands, near), fill=(240, 240, 240))
    return img, near, shown, hands


def third_person(world, bat, view, size, label=''):
    chest = world['Chest'][1]
    eye = chest + np.array(view)
    img, _ = render.draw([(m.pose(world), m.triangles, m.color) for m in rig.body] + [bat], eye,
                         render.look_at(eye, chest + [0, -0.12, 0.15]), size=size)
    if label:
        ImageDraw.Draw(img).text((6, 4), label, fill=(240, 240, 240))
    return img


def grip_error(world):
    root = S.yflip @ matrix(*world['Weapon'])
    return max(np.linalg.norm((matrix(*world[s + 'Hand']) @ np.linalg.inv(root))[3, :3] - S.grips[s][3, :3])
               for s in ('Left', 'Right'))


def after_pose(f):
    """The authored pose at frame f via the written clips (the game's way)."""
    if f <= swing.IMPACT:
        return clips.world_pose(clips.local_pose(sw, rig.names, float(f)), rig.parents, rig.names)
    return clips.world_pose(clips.local_pose(rc, rig.names, float(f - swing.IMPACT)), rig.parents, rig.names)


# the carry idle in first person, looking level and down
tiles = []
for look in (0, 20, 35):
    w = rig.world(idle, 0, look)
    tiles.append(first_person(w, battery(w), 'carry idle, looking %d deg down' % look)[0])
sheet = Image.new('RGB', (FP_SIZE[0], FP_SIZE[1] * 3))
for k, t in enumerate(tiles):
    sheet.paste(t, (0, k * FP_SIZE[1]))
sheet.save(out / 'carry_idle_fp.png')

# before / after, real time
SMALL = (FP_SIZE[0], FP_SIZE[1])
THREE_QUARTER = (1.3, 0.2, 1.6)
put_end = clips.clip_end(put)
put_item_end = clips.clip_end(rig.item_put)
rows, report = [], []
for f in range(swing.END + 1):
    t = f / swing.FPS
    if t <= 0.3:  # the put's clips (player and battery), scaled to the swing's 0.3 s
        wb = rig.world(put, t / 0.3 * put_end)
        bb = battery(wb, rig.item_put, t / 0.3 * put_item_end)
    else:
        wb = rig.world(idle, 0)
        bb = battery(wb)
    wa = after_pose(f)
    ba = battery(wa)
    bfp, bn, bs, bh = first_person(wb, bb, 'before %.2fs' % t)
    afp, an, as_, ah = first_person(wa, ba, 'after %.2fs' % t + ('  IMPACT' if f == swing.IMPACT else ''))
    bi = render.penetration(bb[0], core_vertices(wb))
    ai = render.penetration(ba[0], core_vertices(wa))
    btp = third_person(wb, bb, THREE_QUARTER, SMALL, 'before %.2fs  TP  body-in-box %d' % (t, bi))
    atp = third_person(wa, ba, THREE_QUARTER, SMALL, 'after %.2fs  TP  body-in-box %d' % (t, ai))
    report.append((f, bs * 100, bh, bn, bi, as_ * 100, ah, an, ai, grip_error(wa) * 1000))
    W, H = SMALL
    tile = Image.new('RGB', (W * 2, H * 2))
    tile.paste(bfp, (0, 0)); tile.paste(afp, (W, 0)); tile.paste(btp, (0, H)); tile.paste(atp, (W, H))
    rows.append(tile)
rows[0].save(out / 'battery_hit_before_after.gif', save_all=True, append_images=rows[1:] + [rows[-1]],
             duration=[33] * len(rows) + [1200], loop=0)
rows[0].save(out / 'battery_hit_before_after_slow.gif', save_all=True, append_images=rows[1:] + [rows[-1]],
             duration=[400] * len(rows) + [1500], loop=0)
W, H = SMALL
sheet = Image.new('RGB', (W * 2 * 3, H * 2 * ((len(rows) + 2) // 3)))
for k, tile in enumerate(rows):
    sheet.paste(tile, ((k % 3) * W * 2, (k // 3) * H * 2))
sheet.save(out / 'battery_hit_frames.png')

# third person, large, one sheet per view
BIG = (640, 480)
for name, view in (('three_quarter', THREE_QUARTER), ('side', (1.9, 0.15, 0.1)), ('front', (0.15, 0.2, 2.0))):
    sheet = Image.new('RGB', (BIG[0] * 3, BIG[1] * ((swing.END + 3) // 3)))
    for f in range(swing.END + 1):
        w = after_pose(f)
        img = third_person(w, battery(w), view, BIG, 'after frame %d  %.2fs%s' % (
            f, f / swing.FPS, '  IMPACT' if f == swing.IMPACT else ''))
        sheet.paste(img, ((f % 3) * BIG[0], (f // 3) * BIG[1]))
    sheet.save(out / ('battery_hit_tp_%s.png' % name))

lines = ['frame  before: on-screen hands near body   after: on-screen hands near body grip-mm']
lines += ['%5d  %13.0f%% %5s %4d %4d  %13.0f%% %5s %4d %4d %7.0f' % r for r in report]
(out / 'metrics.txt').write_text('\n'.join(lines) + '\n')
print('\n'.join(lines))
