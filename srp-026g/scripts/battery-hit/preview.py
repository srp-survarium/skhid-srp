"""Before/after previews: the current swing (the put clip played as a
0.3 s thrust) left, the authored swing right, first and third person, same
cameras; red is geometry crossing the near plane in view; the number is the
carrier's body inside the battery's box (vertices of the torso core, head
and helmet)."""
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

import clips
import render
import swing
from rig import Rig, HANDS

rig = Rig(sys.argv[1])
out = Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
idle = rig.clip(HANDS + 'idle')
put = rig.clip(HANDS + 'hidetest')
frames = swing.author(rig, idle)
sw = swing.write(rig, idle, frames, 0, swing.IMPACT)
rc = swing.write(rig, idle, frames, swing.IMPACT, swing.END)
(out / 'battery_hit.b-spline').write_bytes(clips.write_clip(sw))
(out / 'battery_hit_recover.b-spline').write_bytes(clips.write_clip(rc))

CORE = {'Hip', 'Spin', 'Spin_1', 'Chest', 'Neck', 'Head'}


def core_vertices(world):
    pts = []
    for m in rig.body:
        v = m.pose(world)
        dom = [m.names[i] if i < len(m.names) else '' for i in m.indices[np.arange(len(v)), np.argmax(m.weights, axis=1)]]
        keep = np.array([d in CORE for d in dom])
        pts.append(v[keep])
    return np.vstack(pts)


def views(world, label):
    bw = rig.battery_world(world)
    bat = (rig.battery.pose(bw), rig.battery.triangles, rig.battery.color)
    eye, r = render.fp_camera(world)
    fp, near = render.draw([(m.pose(world), m.triangles, m.color) for m in rig.hud] + [bat], eye, r)
    chest = world['Chest'][1]
    e = chest + np.array([1.6, 0.25, 1.9])
    body = [(m.pose(world), m.triangles, m.color) for m in rig.body]
    inside = render.penetration(bat[0], core_vertices(world))
    tp, _ = render.draw(body + [bat], e, render.look_at(e, chest + [0, 0.05, 0.1]))
    for img, text in ((fp, '%s  FP  near-clip %d' % (label, near)), (tp, '%s  TP  body-in-box %d' % (label, inside))):
        ImageDraw.Draw(img).text((6, 4), text, fill=(240, 240, 240))
    return fp, tp, near, inside


def frame_pose_after(f):
    """The authored pose at frame f via the written clips (the game's way)."""
    if f <= swing.IMPACT:
        return clips.world_pose(clips.local_pose(sw, rig.names, float(f)), rig.parents, rig.names)
    return clips.world_pose(clips.local_pose(rc, rig.names, float(f - swing.IMPACT)), rig.parents, rig.names)


steps = list(range(0, swing.END + 1))
rows, report = [], []
put_end = clips.clip_end(put)
for k, f in enumerate(steps):
    t = f / swing.FPS
    # before: the put clip time-scaled to 0.3 s, then the carry idle
    tb = min(t / 0.3, 1.0) * put_end
    wb = rig.world(put, tb) if t <= 0.3 else rig.world(idle, 0)
    wa = frame_pose_after(f)
    bfp, btp, bn, bi = views(wb, 'before %.2fs' % t)
    afp, atp, an, ai = views(wa, 'after %.2fs' % t + ('  IMPACT' if f == swing.IMPACT else ''))
    report.append((f, bn, bi, an, ai))
    W, H = render.W, render.H
    tile = Image.new('RGB', (W * 2, H * 2))
    tile.paste(bfp, (0, 0)); tile.paste(afp, (W, 0)); tile.paste(btp, (0, H)); tile.paste(atp, (W, H))
    rows.append(tile)
# real time (30 fps, a pause at the end) and slowed down (0.4 s a frame)
rows[0].save(out / 'battery_hit_before_after.gif', save_all=True, append_images=rows[1:] + [rows[-1]],
             duration=[33] * len(rows) + [1200], loop=0)
rows[0].save(out / 'battery_hit_before_after_slow.gif', save_all=True, append_images=rows[1:] + [rows[-1]],
             duration=[400] * len(rows) + [1500], loop=0)
sheet = Image.new('RGB', (render.W * 2 * 3, render.H * 2 * ((len(rows) + 2) // 3)))
for k, tile in enumerate(rows):
    sheet.paste(tile, ((k % 3) * render.W * 2, (k // 3) * render.H * 2))
sheet.save(out / 'battery_hit_frames.png')
for line in report:
    print('frame %2d  before near %3d body %4d   after near %3d body %4d' % line)
