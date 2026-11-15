"""The previews of the battery swing (private/renders/battery-hit/), posed as
the game poses them:

- the carrier: a battery state's player clip on the arms alone (bone mask 2),
  every other bone from the stand set's base clip (Rig.game_world;
  computed_local_bone_transform's mask test 0x3B20F2, the battery's player
  lexeme's mask 2 set at 0x345000);
- the battery: its own skeleton posed by the state's battery clip, hung from
  the carrier's Weapon bone (Rig.battery_world);
- every clip evaluated as the game's baker and evaluate_frame do (clips.py
  matches them within 1e-6: engine_bake.py runs the game's code under
  Unicorn).

Out:
- carry_idle_fp.png: the carry idle in first person, looking level, 20 and 35
  degrees down;
- battery_hit_before_after(_slow).gif, battery_hit_frames.png: the build the
  owner played (its clips in `before`, the carry's battery clip) left, the
  new swing right, first person over third person;
- battery_hit_tp_<view>.png: every frame of the new swing, third person, large;
- metrics.txt: per frame what first person shows and what would clip.

    python preview.py <assets folder> <out folder> <before clips folder>
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
before_dir = Path(sys.argv[3])
idle = rig.clip(HANDS + 'idle')
S = swing.Swing(rig, idle)
here = Path(__file__).resolve().parent / 'clips'
load = lambda path: clips.parse_clip(Path(path).read_bytes())[0]
after = {'user': (load(here / 'battery_hit.b-spline'), load(here / 'battery_hit_recover.b-spline')),
         'item': (load(here / 'battery_hit_electrobox.b-spline'), load(here / 'battery_hit_recover_electrobox.b-spline'))}
before = {'user': (load(before_dir / 'battery_hit.b-spline'), load(before_dir / 'battery_hit_recover.b-spline')),
          'item': ((load(before_dir / 'battery_hit_electrobox.b-spline'), load(before_dir / 'battery_hit_recover_electrobox.b-spline'))
                   if (before_dir / 'battery_hit_electrobox.b-spline').exists() else (rig.item_idle, rig.item_idle))}
FP_SIZE = (640, 360)
FRAMES = sorted(set(range(int(swing.END) + 1)) | {swing.END})
CORE = {'Hip', 'Spin', 'Spin_1', 'Chest', 'Neck', 'Head'}


def game_pose(set_, f, look=0.0):
    """The carrier and the battery at swing frame f (the swing state's clips
    to their end, the recovery's after), as the game mixes them."""
    end = clips.clip_end(set_['user'][0])
    k, t = (0, float(f)) if f <= end else (1, min(float(f - end), clips.clip_end(set_['user'][1])))
    world = rig.game_world(set_['user'][k], t, look)
    bw = rig.battery_world(world, t, set_['item'][k])
    return world, bw


def core_vertices(world):
    pts = []
    for m in rig.body:
        v = m.pose(world)
        dom = [m.names[i] if i < len(m.names) else '' for i in m.indices[np.arange(len(v)), np.argmax(m.weights, axis=1)]]
        pts.append(v[np.array([d in CORE for d in dom])])
    return np.vstack(pts)


def on_screen(points, eye, axes, size=FP_SIZE):
    v = (points - eye) @ np.array(axes).T
    focal = size[1] / (2 * math.tan(math.radians(render.FOV) / 2))
    s = v[:, :2] / np.maximum(v[:, 2:3], 1e-6) * focal
    return (v[:, 2] > render.NEAR) & (np.abs(s[:, 0]) < size[0] / 2) & (np.abs(s[:, 1]) < size[1] / 2)


def first_person(world, bw, label):
    bat = (rig.battery.pose(bw), rig.battery.triangles, rig.battery.color)
    eye, r = render.fp_camera(world)
    img, near = render.draw([(m.pose(world), m.triangles, m.color) for m in rig.fp] + [bat], eye, r, size=FP_SIZE)
    shown = on_screen(bat[0], eye, r).mean()
    hands = ''.join(s[0] if on_screen(np.array([world[s + 'Hand'][1], world[s + 'Middle2'][1]]), eye, r).any() else '-'
                    for s in ('Left', 'Right'))
    ImageDraw.Draw(img).text((6, 4), '%s  FP  battery on screen %d%%  hands %s  near-clip %d'
                             % (label, 100 * shown, hands, near), fill=(240, 240, 240))
    return img, near, shown, hands


def third_person(world, bw, view, size, label=''):
    bat = (rig.battery.pose(bw), rig.battery.triangles, rig.battery.color)
    chest = world['Chest'][1]
    eye = chest + np.array(view)
    img, _ = render.draw([(m.pose(world), m.triangles, m.color) for m in rig.body] + [bat], eye,
                         render.look_at(eye, chest + [0, -0.12, 0.25]), size=size)
    if label:
        ImageDraw.Draw(img).text((6, 4), label, fill=(240, 240, 240))
    return img


def grip_error(world, bw):
    """The worse hand's distance from where it holds the battery in the carry
    (its idle frame relative to the battery's offset bone)."""
    move = np.linalg.inv(matrix(*S.item_world0['offset'])) @ matrix(*bw['offset'])
    return max(np.linalg.norm(world[s + 'Hand'][1] - (matrix(*S.world0[s + 'Hand']) @ move)[3, :3])
               for s in ('Left', 'Right'))


def battery_travel(bw):
    return rig.battery.pose(bw).mean(axis=0) - S.centre0


# the carry idle in first person, looking level and down
tiles = []
for look in (0, 20, 35):
    w = rig.game_world(idle, 0, look)
    tiles.append(first_person(w, rig.battery_world(w), 'carry idle, looking %d deg down' % look)[0])
sheet = Image.new('RGB', (FP_SIZE[0], FP_SIZE[1] * 3))
for k, t in enumerate(tiles):
    sheet.paste(t, (0, k * FP_SIZE[1]))
sheet.save(out / 'carry_idle_fp.png')

THREE_QUARTER = (1.3, 0.2, 1.6)
rows, report, sheet_rows = [], [], []
HALF = [i * 0.5 for i in range(int(swing.END * 2) + 1)]   # every 1/60 s
for f in HALF:
    t = f / swing.FPS
    wb, bb = game_pose(before, f)
    wa, ba = game_pose(after, f)
    tag = '  IMPACT' if f == swing.IMPACT else ''
    bfp, bn, bs, bh = first_person(wb, bb, 'before %.2fs%s' % (t, tag))
    afp, an, as_, ah = first_person(wa, ba, 'after %.2fs%s' % (t, tag))
    bi = render.penetration(rig.battery.pose(bb), core_vertices(wb))
    ai = render.penetration(rig.battery.pose(ba), core_vertices(wa))
    btp = third_person(wb, bb, THREE_QUARTER, FP_SIZE, 'before %.2fs  TP  body-in-box %d' % (t, bi))
    atp = third_person(wa, ba, THREE_QUARTER, FP_SIZE, 'after %.2fs  TP  body-in-box %d' % (t, ai))
    if f in FRAMES:
      report.append((f, np.linalg.norm(battery_travel(bb)) * 100, grip_error(wb, bb) * 1000, bs * 100, bn, bi,
                   *(battery_travel(ba) * 100), grip_error(wa, ba) * 1000, as_ * 100, ah, an, ai))
    W, H = FP_SIZE
    tile = Image.new('RGB', (W * 2, H * 2))
    tile.paste(bfp, (0, 0)); tile.paste(afp, (W, 0)); tile.paste(btp, (0, H)); tile.paste(atp, (W, H))
    rows.append(tile)
    if f in FRAMES:
        sheet_rows.append(tile)
rows[0].save(out / 'battery_hit_before_after.gif', save_all=True, append_images=rows[1:] + [rows[-1]],
             duration=[17] * len(rows) + [1200], loop=0)
rows[0].save(out / 'battery_hit_before_after_slow.gif', save_all=True, append_images=rows[1:] + [rows[-1]],
             duration=[200] * len(rows) + [1500], loop=0)
rows = sheet_rows
W, H = FP_SIZE
sheet = Image.new('RGB', (W * 2 * 3, H * 2 * ((len(rows) + 2) // 3)))
for k, tile in enumerate(rows):
    sheet.paste(tile, ((k % 3) * W * 2, (k // 3) * H * 2))
sheet.save(out / 'battery_hit_frames.png')

BIG = (640, 480)
for name, view in (('three_quarter', THREE_QUARTER), ('side', (1.9, 0.15, 0.3)), ('front', (0.15, 0.2, 2.0))):
    sheet = Image.new('RGB', (BIG[0] * 3, BIG[1] * ((len(FRAMES) + 2) // 3)))
    for i, f in enumerate(FRAMES):
        w, bw = game_pose(after, f)
        img = third_person(w, bw, view, BIG, 'after frame %d  %.2fs%s' % (
            f, f / swing.FPS, '  IMPACT' if f == swing.IMPACT else ''))
        sheet.paste(img, ((i % 3) * BIG[0], (i // 3) * BIG[1]))
    sheet.save(out / ('battery_hit_tp_%s.png' % name))

# between frames too (the game draws at its own frame rate): a quarter frame apart
between = 0
for q in range(int(swing.END * 4) + 1):
    w, bw = game_pose(after, q / 4)
    eye, r = render.fp_camera(w)
    between += render.draw([(m.pose(w), m.triangles, m.color) for m in rig.fp]
                           + [(rig.battery.pose(bw), rig.battery.triangles, rig.battery.color)], eye, r, size=(160, 90))[1]

# smoothness: how far the battery and the hands move from one drawn frame to the next, and how
# far the hands are off the battery, at 60 and 240 frames a second (the clips are evaluated at
# any time, as the game's visual skeleton does: object_skeleton_visual 0x292620 ticks to its
# timer's now)
def steps(set_, rate):
    total = clips.clip_end(set_['user'][0]) + clips.clip_end(set_['user'][1])
    prev, out = None, []
    for k in range(int(total / swing.FPS * rate) + 1):
        f = k / rate * swing.FPS
        w, bw = game_pose(set_, f)
        c = rig.battery.pose(bw).mean(axis=0)
        h = np.array([w['LeftHand'][1], w['RightHand'][1]])
        if prev is not None:
            out.append((f / swing.FPS, np.linalg.norm(c - prev[0]) * 100, np.abs(h - prev[1]).max() * 100,
                        grip_error(w, bw) * 1000))
        prev = (c, h)
    return np.array(out)


smooth = {}
for name, set_ in (('before', before), ('after', after)):
    for rate in (60, 240):
        smooth[name, rate] = steps(set_, rate)
plot = Image.new('RGB', (960, 600), (24, 28, 36))
draw = ImageDraw.Draw(plot)
for row, (title, col, scale) in enumerate((('battery step per 1/240 s (cm)', 1, 10), ('hand step per 1/240 s (cm)', 2, 10),
                                          ('hands off the battery (mm)', 3, 1000))):
    y0 = 20 + row * 195
    draw.text((10, y0 - 14), title + '   grey: before (the build played), yellow: after', fill=(220, 220, 220))
    draw.rectangle((40, y0, 940, y0 + 170), outline=(90, 90, 90))
    for name, colour in (('before', (150, 150, 150)), ('after', (240, 200, 60))):
        d = smooth[name, 240]
        top = max(smooth['before', 240][:, col].max(), smooth['after', 240][:, col].max(), 1e-6)
        pts = [(40 + 900 * t / 0.6, y0 + 170 - 170 * v / top) for t, v in zip(d[:, 0], d[:, col])]
        draw.line(pts, fill=colour, width=2)
    draw.text((44, y0 + 2), 'max %.1f' % max(smooth['before', 240][:, col].max(), smooth['after', 240][:, col].max()),
              fill=(200, 200, 200))
plot.save(out / 'battery_hit_smoothness.png')

lines = ['frame | before: battery moved cm, grip mm, FP on-screen %, near, body'
         ' | after: battery moved x y z cm, grip mm, FP on-screen %, hands, near, body']
lines += ['%5.1f | %5.1f %6.0f %5.0f%% %3d %4d | %+6.1f %+6.1f %+6.1f %5.0f %5.0f%% %3s %3d %4d' % r for r in report]
lines.append('after, every quarter frame: %d triangles crossing the near plane in view' % between)
lines.append('')
lines.append('smoothness (the clips evaluated between frames, as the game draws them):')
for name in ('before', 'after'):
    for rate in (60, 240):
        d = smooth[name, rate]
        lines.append('  %-6s at %3d/s: battery moves at most %5.2f cm a frame, a hand %5.2f cm, the hands at most %5.1f mm off the battery'
                     % (name, rate, d[:, 1].max(), d[:, 2].max(), d[:, 3].max()))
lines.append('  after, every 1/60 s: t, battery step cm, hand step cm, hands off mm')
lines += ['    %.3f %5.2f %5.2f %5.1f' % tuple(r) for r in smooth['after', 60]]
(out / 'metrics.txt').write_text('\n'.join(lines) + '\n')
print('\n'.join(lines))
