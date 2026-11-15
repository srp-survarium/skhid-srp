"""The previews of the battery swing (private/renders/battery-hit/), before
(a folder of the clips of the build last played) beside after (clips/),
posed as the game poses them:

- the carrier: a battery state's player clip on the arms alone (bone mask 2),
  every other bone from the stand set's base clip (Rig.game_world;
  computed_local_bone_transform's mask test 0x3B20F2, the battery's player
  lexeme's mask 2 set at 0x345000);
- the battery: its own skeleton posed by the state's battery clip, hung from
  the carrier's Weapon bone (Rig.battery_world);
- every clip evaluated as the game's baker and evaluate_frame do (clips.py
  matches them within 1e-6: engine_bake.py runs the game's code under
  Unicorn);
- over the whole cycle, as the game's states play it: the carry idle (still),
  the swing clip from its state's first tick, the recovery clip from the tick
  the swing ended on, the carry idle again. The game's animation time is whole
  milliseconds, so that is the finest step measured.

Out:
- battery_hit_before_after.gif (game speed, a frame every 1/50 s), _slow.gif
  (a fifth of it) and, with ffmpeg at hand, .mp4 (60 frames a second: three
  times at game speed, then once at a fifth): before left, after right; first
  person, third person three-quarter, third person from the right;
- battery_hit_velocity.png: the battery's speed and acceleration and the
  hands' hold over the cycle, before and after;
- battery_hit_joints.png: when the shoulder, the elbow, the hand and the
  battery move, each as a share of its own top speed;
- battery_hit_frames.png: every 1/30 s on one sheet;
- battery_hit_tp_<view>.png: the new swing, third person, large;
- carry_idle_fp.png: the carry idle in first person, looking level, 20 and 35
  degrees down;
- metrics.txt: the numbers.

    python preview.py <assets folder> <out folder> <before clips folder> [jobs [plots]]

(`plots` after the jobs: the two plots alone.)
"""
import math
import multiprocessing
import shutil
import subprocess
import sys
import tempfile
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
JOBS = int(sys.argv[4]) if len(sys.argv) > 4 else 8
idle = rig.clip(HANDS + 'idle')
S = swing.Swing(rig, idle)
here = Path(__file__).resolve().parent / 'clips'
load = lambda folder, name: clips.parse_clip((Path(folder) / (name + '.b-spline')).read_bytes())[0]
SETS = {name: {'user': (load(folder, 'battery_hit'), load(folder, 'battery_hit_recover')),
               'item': (load(folder, 'battery_hit_electrobox'), load(folder, 'battery_hit_recover_electrobox'))}
        for name, folder in (('before', before_dir), ('after', here))}
SWING_S, RECOVER_S = swing.IMPACT / swing.FPS, (swing.END - swing.IMPACT) / swing.FPS
CYCLE = (-0.1, SWING_S + RECOVER_S + 0.15)        # the carry before, the swing, the recovery, the carry after
TILE = (480, 270)
CORE = {'Hip', 'Spin', 'Spin_1', 'Chest', 'Neck', 'Head'}
VIEWS = {'three_quarter': (1.3, 0.2, 1.6), 'side': (1.9, 0.15, 0.3), 'front': (0.15, 0.2, 2.0)}


def phase(t):
    return 'carry' if t < 0 or t >= SWING_S + RECOVER_S else 'swing' if t < SWING_S else 'recovery'


def game_pose(name, t, look=0.0):
    """The carrier and the battery of clip set `name` at time t (s) of the
    cycle: the carry idle before the swing's state and after the recovery's,
    each state's clips from its first tick."""
    set_ = SETS[name]
    state = phase(t)
    if state == 'carry':
        world = rig.game_world(idle, 0, look)
        return world, rig.battery_world(world)
    k, at = (0, t) if state == 'swing' else (1, t - SWING_S)
    world = rig.game_world(set_['user'][k], at * swing.FPS, look)
    return world, rig.battery_world(world, at * swing.FPS, set_['item'][k])


def core_vertices(world):
    pts = []
    for m in rig.body:
        v = m.pose(world)
        dom = [m.names[i] if i < len(m.names) else '' for i in m.indices[np.arange(len(v)), np.argmax(m.weights, axis=1)]]
        pts.append(v[np.array([d in CORE for d in dom])])
    return np.vstack(pts)


def on_screen(points, eye, axes, size=TILE):
    v = (points - eye) @ np.array(axes).T
    focal = size[1] / (2 * math.tan(math.radians(render.FOV) / 2))
    s = v[:, :2] / np.maximum(v[:, 2:3], 1e-6) * focal
    return (v[:, 2] > render.NEAR) & (np.abs(s[:, 0]) < size[0] / 2) & (np.abs(s[:, 1]) < size[1] / 2)


def first_person(name, t, size=TILE, look=0.0, label=None):
    """(image, triangles through the near plane, share of the battery on screen, hands on screen)."""
    world, bw = game_pose(name, t, look)
    bat = (rig.battery.pose(bw), rig.battery.triangles, rig.battery.color)
    eye, r = render.fp_camera(world)
    img, near = render.draw([(m.pose(world), m.triangles, m.color) for m in rig.fp] + [bat], eye, r, size=size)
    shown = on_screen(bat[0], eye, r, size).mean()
    hands = ''.join(s[0] if on_screen(np.array([world[s + 'Hand'][1], world[s + 'Middle2'][1]]), eye, r, size).any() else '-'
                    for s in ('Left', 'Right'))
    text = '%s  first person  %s' % (name, phase(t)) if label is None else label
    ImageDraw.Draw(img).text((6, 4), text, fill=(240, 240, 240))
    return img, near, shown, hands


def third_person(name, t, view, size=TILE, label=None):
    """(image, torso, neck, head and helmet vertices inside the battery)."""
    world, bw = game_pose(name, t)
    bat = (rig.battery.pose(bw), rig.battery.triangles, rig.battery.color)
    chest = world['Chest'][1]
    eye = chest + np.array(VIEWS[view])
    img, _ = render.draw([(m.pose(world), m.triangles, m.color) for m in rig.body] + [bat], eye,
                         render.look_at(eye, chest + [0, -0.12, 0.25]), size=size)
    text = '%s  third person, %s  %s' % (name, view.replace('_', ' '), phase(t)) if label is None else label
    ImageDraw.Draw(img).text((6, 4), text, fill=(240, 240, 240))
    return img, render.penetration(bat[0], core_vertices(world))


def grip(world, bw):
    """Each hand's distance from where it holds the battery in the carry
    (its idle frame relative to the battery's offset bone), m."""
    move = np.linalg.inv(matrix(*S.item_world0['offset'])) @ matrix(*bw['offset'])
    return [np.linalg.norm(world[s + 'Hand'][1] - (matrix(*S.world0[s + 'Hand']) @ move)[3, :3]) for s in ('Left', 'Right')]


def tile(job):
    kind, name, t, arg = job
    if kind == 'fp':
        return first_person(name, t)[0]
    if kind == 'tp':
        return third_person(name, t, arg)[0]
    if kind == 'row':                       # the metrics' row of one frame
        world, bw = game_pose(name, t)
        _, near, shown, hands = first_person(name, t)
        inside = third_person(name, t, 'three_quarter')[1]
        return (rig.battery.pose(bw).mean(axis=0) - S.centre0) * 100, max(grip(world, bw)) * 1000, shown * 100, hands, near, inside
    if kind == 'near':                      # triangles through the near plane, small
        return first_person(name, t, size=(160, 90))[1]
    if kind == 'big':
        return third_person(name, t, arg, (640, 480), 'after %.3f s  %s%s' % (t, phase(t), '  STRIKE' if abs(t - SWING_S) < 1e-9 else ''))[0]
    if kind == 'track':                     # the measured places at one millisecond
        world, bw = game_pose(name, t)
        move = np.linalg.inv(matrix(*S.item_world0['offset'])) @ matrix(*bw['offset'])
        centre = (np.r_[S.centre0, 1] @ move)[:3]
        return np.r_[centre, world['RightHand'][1], world['RightForeArm'][1], world['RightArm'][1], world['LeftHand'][1],
                     world['LeftForeArm'][1], world['LeftArm'][1], grip(world, bw), bw['offset'][0].ravel()]
    raise ValueError(kind)


def timeline(width, t):
    """A strip under a frame: the states over the cycle, the moment marked."""
    strip = Image.new('RGB', (width, 34), (18, 22, 28))
    draw = ImageDraw.Draw(strip)
    x = lambda s: 10 + (width - 20) * (s - CYCLE[0]) / (CYCLE[1] - CYCLE[0])
    for a, b, name, colour in ((CYCLE[0], 0, 'carry', (60, 66, 76)), (0, SWING_S, 'swing', (164, 108, 44)),
                               (SWING_S, SWING_S + RECOVER_S, 'recovery', (58, 104, 150)),
                               (SWING_S + RECOVER_S, CYCLE[1], 'carry', (60, 66, 76))):
        draw.rectangle((x(a), 16, x(b) - 1, 28), fill=colour)
        draw.text((x(a) + 4, 2), name, fill=(215, 215, 215))
    draw.text((x(SWING_S) - 44, 2), 'strike', fill=(240, 240, 240))
    draw.rectangle((x(t) - 1, 12, x(t) + 1, 32), fill=(255, 255, 255))
    draw.text((width - 62, 2), '%+.3f s' % t, fill=(215, 215, 215))
    return strip


def compose(tiles, t):
    """One frame of the film: before | after; first person, three-quarter, side."""
    W, H = TILE
    frame = Image.new('RGB', (W * 2, H * 3 + 34))
    for k, img in enumerate(tiles):
        frame.paste(img, ((k % 2) * W, (k // 2) * H))
    frame.paste(timeline(W * 2, t), (0, H * 3))
    return frame


def film(pool, times):
    jobs = [(kind, name, t, arg) for t in times for kind, arg in (('fp', None), ('tp', 'three_quarter'), ('tp', 'side'))
            for name in ('before', 'after')]
    tiles = pool.map(tile, jobs, chunksize=4)
    return [compose(tiles[6 * i:6 * i + 6], t) for i, t in enumerate(times)]


def plots(ts, track):
    """battery_hit_velocity.png and battery_hit_joints.png."""
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    surface, ink, ink2, muted, grid, axis = '#fcfcfb', '#0b0b0b', '#52514e', '#898781', '#e1e0d9', '#c3c2b7'
    colour = {'after': '#2a78d6', 'before': '#eb6834'}
    plt.rcParams.update({'font.size': 10, 'axes.edgecolor': axis, 'axes.labelcolor': ink2, 'xtick.color': muted,
                         'ytick.color': muted, 'text.color': ink, 'figure.facecolor': surface, 'axes.facecolor': surface,
                         'savefig.facecolor': surface, 'lines.solid_capstyle': 'round', 'lines.solid_joinstyle': 'round'})
    speed = lambda name, cols: np.linalg.norm(np.diff(track[name][:, cols], axis=0), axis=1) / np.diff(ts)
    mid = (ts[1:] + ts[:-1]) / 2

    def dress(ax, title, unit):
        ax.set_title(title, loc='left', fontsize=11, color=ink, pad=8)
        ax.set_ylabel(unit)
        ax.grid(axis='y', color=grid, linewidth=1)
        ax.set_axisbelow(True)
        for side in ('top', 'right'):
            ax.spines[side].set_visible(False)
        for edge in (0.0, SWING_S, SWING_S + RECOVER_S):
            ax.axvline(edge, color=axis, linewidth=1)
        ax.set_xlim(CYCLE)

    def states(ax):
        low, top = ax.get_ylim()
        for a, b, name in ((CYCLE[0], 0, 'carry'), (0, SWING_S, 'swing'), (SWING_S, SWING_S + RECOVER_S, 'recovery'),
                           (SWING_S + RECOVER_S, CYCLE[1], 'carry')):
            ax.text(a + 0.006, top - (top - low) * 0.02, name, ha='left', va='top', fontsize=9, color=muted)

    def turning(name):
        """The battery's turn, degrees a second."""
        r = track[name][:, 23:32].reshape(-1, 3, 3)
        cos = (np.einsum('nij,nij->n', r[:-1], r[1:]) - 1) / 2
        return np.degrees(np.arccos(np.clip(cos, -1, 1))) / np.diff(ts)

    fig, axes = plt.subplots(3, 1, figsize=(11, 10.5), sharex=True, gridspec_kw={'height_ratios': [3, 2, 1.6]})
    ax = axes[0]
    for name in ('before', 'after'):
        ax.plot(mid, speed(name, slice(0, 3)), color=colour[name], linewidth=2, label=name)
    dress(ax, "The battery's speed over the cycle", 'm/s')
    ax.set_ylim(0, 7.4)
    states(ax)
    ax.legend(loc='upper right', frameon=False)
    v = {name: speed(name, slice(0, 3)) for name in colour}
    strike = np.searchsorted(mid, SWING_S)
    note = lambda text, at, where: ax.annotate(text, at, where, fontsize=9, color=ink2,
                                               arrowprops={'arrowstyle': '-', 'color': muted, 'linewidth': 1})
    gather = (mid > 0) & (mid < SWING_S / 2)
    if v['before'][gather].max() > 0.6 * v['before'].max():
        k = np.flatnonzero(gather)[v['before'][gather].argmax()]
        note('before: the gather nearly as fast as the push\n(%.1f against %.1f m/s), a stop between them'
             % (v['before'][k], v['before'].max()), (mid[k], v['before'][k]), (-0.092, 4.6))
    still = (v['before'] < 0.25) & (mid > 0.02) & (mid < SWING_S + RECOVER_S - 0.02)
    runs = np.flatnonzero(np.diff(np.r_[0, still.astype(int), 0]))
    longest = max(zip(runs[::2], runs[1::2]), key=lambda r: r[1] - r[0], default=None)
    if longest and longest[1] - longest[0] >= 30:
        note('before: still for %.2f s' % ((longest[1] - longest[0]) / 1000), (mid[(longest[0] + longest[1]) // 2 + 20], 0.04), (0.285, 1.5))
    top = v['after'][strike - 3:strike + 3].max()
    if top > 0.97 * v['after'].max():
        note('after: fastest at the strike (%.1f m/s),\nthen carried on and checked' % top, (SWING_S, top), (0.238, 5.5))
    ax = axes[1]
    for name in ('before', 'after'):
        a = np.diff(v[name]) / np.diff(mid)
        ax.plot(ts[1:-1], np.clip(a, -420, 420), color=colour[name], linewidth=2, label=name)
    dress(ax, "The battery's acceleration along its path (speed gained a second); drawn to 400", 'm/s²')
    ax.set_ylim(-440, 440)
    jumps = np.diff(v['before']) / np.diff(mid)
    start = np.searchsorted(mid, 0.0)
    if jumps[start - 2:start + 3].max() > 420:
        ax.annotate('before: speed jumps within 1 ms\n(%.0f m/s² at the start, %.0f at the strike)'
                    % (jumps[start - 2:start + 3].max(), jumps[strike - 3:strike + 3].min()), (0.003, 400), (0.022, 250),
                    fontsize=9, color=ink2, arrowprops={'arrowstyle': '-', 'color': muted, 'linewidth': 1})
    ax.legend(loc='lower right', frameon=False, ncol=2)
    ax = axes[2]
    for name in ('before', 'after'):
        ax.plot(ts, track[name][:, 21:23].max(axis=1) * 1000, color=colour[name], linewidth=2, label=name)
    dress(ax, 'The worse hand off its hold on the battery', 'mm')
    ax.set_ylim(0, 24)
    ax.set_xlabel('time from the swing state\'s start, s (the strike at %.1f)' % SWING_S)
    ax.legend(loc='upper right', frameon=False, ncol=2)
    fig.tight_layout()
    fig.savefig(out / 'battery_hit_velocity.png', dpi=110)
    plt.close(fig)

    parts = (("the battery's place", slice(0, 3), '#2a78d6'), ("the battery's turn", None, '#eb6834'),
             ('the right elbow', slice(6, 9), '#1baf7a'), ('the right shoulder', slice(9, 12), '#eda100'))
    fig, axes = plt.subplots(2, 1, figsize=(11, 7.6), sharex=True, sharey=True)
    for ax, name in zip(axes, ('before', 'after')):
        for label, cols, c in parts:
            s = turning(name) if cols is None else speed(name, cols)
            if s.max() < 1e-3:
                continue
            ax.plot(mid, s / s.max(), color=c, linewidth=2, label='%s (top %s at %.3f s)' % (
                label, '%.0f deg/s' % s.max() if cols is None else '%.1f m/s' % s.max(), mid[s.argmax()]))
        dress(ax, '%s: when each part moves (its speed as a share of its own top speed)' % name.capitalize()
              + ('; the shoulders never move' if name == 'before' else ''), 'share of top speed')
        ax.set_ylim(0, 1.3)
        ax.set_yticks([0, 0.25, 0.5, 0.75, 1.0])
        ax.legend(loc='upper right', bbox_to_anchor=(1.0, 0.94), frameon=False, fontsize=9, ncol=2)
        states(ax)
    axes[1].set_xlabel('time from the swing state\'s start, s')
    fig.tight_layout()
    fig.savefig(out / 'battery_hit_joints.png', dpi=110)
    plt.close(fig)


# every millisecond of the cycle (the game's animation time is whole milliseconds)
TS = np.arange(int(round(CYCLE[0] * 1000)), int(round(CYCLE[1] * 1000)) + 1) / 1000.0


def tracked(pool):
    return {name: np.array(pool.map(tile, [('track', name, float(t), None) for t in TS], chunksize=16)) for name in ('before', 'after')}


def main():
    pool = multiprocessing.get_context('fork').Pool(JOBS)
    W, H = TILE
    if 'plots' in sys.argv[5:]:             # the two plots alone
        plots(TS, tracked(pool))
        return

    # the carry idle in first person, looking level and down
    sheet = Image.new('RGB', (640, 360 * 3))
    for k, look in enumerate((0, 20, 35)):
        sheet.paste(first_person('after', -0.05, (640, 360), look, 'carry idle, looking %d deg down' % look)[0], (0, k * 360))
    sheet.save(out / 'carry_idle_fp.png')

    # the films
    fifty = [CYCLE[0] + k / 50 for k in range(int(round((CYCLE[1] - CYCLE[0]) * 50)) + 1)]
    frames = film(pool, fifty)
    frames[0].save(out / 'battery_hit_before_after.gif', save_all=True, append_images=frames[1:],
                   duration=[20] * (len(frames) - 1) + [700], loop=0)
    frames[0].save(out / 'battery_hit_before_after_slow.gif', save_all=True, append_images=frames[1:],
                   duration=[100] * (len(frames) - 1) + [1200], loop=0)
    if shutil.which('ffmpeg'):
        sixty = [CYCLE[0] + k / 60 for k in range(int(round((CYCLE[1] - CYCLE[0]) * 60)) + 1)]
        frames = film(pool, sixty)
        with tempfile.TemporaryDirectory() as folder:
            order = (list(range(len(frames))) + [len(frames) - 1] * 30) * 3 + [i for i in range(len(frames)) for _ in range(5)]
            for n, i in enumerate(order):
                target = Path(folder) / ('%05d.png' % n)
                if n == order.index(i):
                    frames[i].save(target)
                else:
                    target.symlink_to(Path(folder) / ('%05d.png' % order.index(i)))
            subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-framerate', '60', '-i', str(Path(folder) / '%05d.png'),
                            '-pix_fmt', 'yuv420p', '-crf', '18', '-vf', 'pad=ceil(iw/2)*2:ceil(ih/2)*2',
                            str(out / 'battery_hit_before_after.mp4')], check=True)

    # every 1/30 s on one sheet, and the metrics' rows
    thirty = [k / 30 for k in range(int(round((SWING_S + RECOVER_S) * 30)) + 1)] + [SWING_S + RECOVER_S]
    thirty = sorted(set(round(t, 6) for t in thirty))
    stills = film(pool, thirty)
    sheet = Image.new('RGB', (W * 2 * 3, (H * 3 + 34) * ((len(stills) + 2) // 3)))
    for k, frame in enumerate(stills):
        sheet.paste(frame, ((k % 3) * W * 2, (k // 3) * (H * 3 + 34)))
    sheet.save(out / 'battery_hit_frames.png')
    rows = {name: pool.map(tile, [('row', name, t, None) for t in thirty]) for name in ('before', 'after')}

    # the new swing large: its beats
    beats = [0.0, 0.05, 0.1, 0.15, 0.18, SWING_S, 0.22, 0.245, 0.31, 0.37, 0.43, 0.49]
    for view in VIEWS:
        big = pool.map(tile, [('big', 'after', t, view) for t in beats])
        sheet = Image.new('RGB', (640 * 3, 480 * ((len(beats) + 2) // 3)))
        for i, img in enumerate(big):
            sheet.paste(img, ((i % 3) * 640, (i // 3) * 480))
        sheet.save(out / ('battery_hit_tp_%s.png' % view))

    # between frames: every 1/240 s through the near plane
    quarter = [k / 240 for k in range(int(round((SWING_S + RECOVER_S) * 240)) + 1)]
    between = sum(pool.map(tile, [('near', 'after', t, None) for t in quarter], chunksize=8))

    track = tracked(pool)
    pool.close()
    plots(TS, track)
    ts = TS

    lines = ['time s | before: battery moved cm, grip mm, FP on-screen %, near, body'
             ' | after: battery moved x y z cm, grip mm, FP on-screen %, hands, near, body']
    for t, b, a in zip(thirty, rows['before'], rows['after']):
        lines.append('%6.3f | %5.1f %6.1f %5.0f%% %3d %4d | %+6.1f %+6.1f %+6.1f %5.1f %5.0f%% %3s %3d %4d%s' % (
            t, np.linalg.norm(b[0]), b[1], b[2], b[4], b[5], *a[0], a[1], a[2], a[3], a[4], a[5],
            '   <- the strike' if abs(t - SWING_S) < 1e-9 else ''))
    lines.append('after, every 1/240 s: %d triangles crossing the near plane in view' % between)
    lines.append('')
    lines.append('smoothness: the cycle carry -> swing -> recovery -> carry, every millisecond (the game\'s animation time)')
    mid = (ts[1:] + ts[:-1]) / 2
    for name in ('before', 'after'):
        d = track[name]
        v = np.linalg.norm(np.diff(d[:, :3], axis=0), axis=1) * 1000
        a = np.diff(v) * 1000
        inside = (mid > 0.02) & (mid < SWING_S + RECOVER_S - 0.02)
        still = inside & (v < 0.25)
        runs = np.flatnonzero(np.diff(np.r_[0, still.astype(int), 0]))
        stops = ', '.join('%.3f-%.3f' % (mid[i], mid[j - 1]) for i, j in zip(runs[::2], runs[1::2]) if j - i >= 10)
        at = lambda t: v[np.searchsorted(mid, t) - 2:np.searchsorted(mid, t) + 3]
        lines.append('  %s' % name)
        lines.append('    battery: top speed %.2f m/s at %.3f s; the gather\'s top %.2f m/s; the way back\'s top %.2f m/s'
                     % (v.max(), mid[v.argmax()], v[(mid > 0) & (mid < 0.1)].max(), v[mid > SWING_S + 0.1].max()))
        lines.append('    speed in the first millisecond of the swing %.2f m/s; just before and after the strike %.2f / %.2f m/s;'
                     ' in the last millisecond of the recovery %.2f m/s' % (at(0.0).max(), v[np.searchsorted(mid, SWING_S) - 2],
                                                                            v[np.searchsorted(mid, SWING_S) + 1], at(SWING_S + RECOVER_S).max()))
        lines.append('    largest change of speed in one millisecond: %.2f m/s (%.0f m/s^2)' % (np.abs(np.diff(v)).max(), np.abs(a).max()))
        lines.append('    slower than 0.25 m/s for 10 ms or more inside the motion: %s' % (stops or 'never'))
        lines.append('    battery moved 25 ms into the swing %.1f cm, 50 ms in %.1f cm (a swing begun between two ticks starts over at the'
                     ' tick: what the restart shows)' % tuple(np.linalg.norm(d[np.searchsorted(ts, t), :3] - d[np.searchsorted(ts, 0.0), :3]) * 100
                                                              for t in (0.025, 0.05)))
        lines.append('    hands off their hold: left at most %.1f mm, right at most %.1f mm' % (d[:, 21].max() * 1000, d[:, 22].max() * 1000))
        for label, cols in (('right shoulder', slice(9, 12)), ('right elbow', slice(6, 9)), ('right hand', slice(3, 6)),
                            ('left shoulder', slice(18, 21)), ('left elbow', slice(15, 18)), ('left hand', slice(12, 15))):
            s = np.linalg.norm(np.diff(d[:, cols], axis=0), axis=1) * 1000
            lines.append('    %-14s top speed %.2f m/s at %.3f s' % (label, s.max(), mid[s.argmax()]))
        for rate in (60, 75, 100, 130):
            step = [np.linalg.norm(d[np.searchsorted(ts, (k + 1) / rate), :3] - d[np.searchsorted(ts, k / rate), :3]) * 100
                    for k in range(int((SWING_S + RECOVER_S) * rate))]
            lines.append('    drawn %3d times a second: the battery moves at most %.1f cm between two frames' % (rate, max(step)))
    lines.append('  after, every 10 ms: time s, battery speed m/s, forward speed m/s, hands off mm')
    d = track['after']
    v = np.linalg.norm(np.diff(d[:, :3], axis=0), axis=1) * 1000
    for t in np.arange(0, SWING_S + RECOVER_S + 1e-9, 0.01):
        i = np.searchsorted(ts, t)
        j = min(i, len(v) - 1)
        lines.append('    %.2f %5.2f %+6.2f %5.1f%s' % (t, v[j], (d[j + 1, 2] - d[j, 2]) * 1000, d[i, 21:23].max() * 1000,
                                                      '   <- the strike' if abs(t - SWING_S) < 1e-9 else ''))
    (out / 'metrics.txt').write_text('\n'.join(lines) + '\n')
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
