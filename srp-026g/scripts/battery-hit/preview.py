"""The previews of the battery's strikes (private/renders/battery-hit/), sets
of clips side by side, posed as the game poses them:

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
  the swing clip from its state's start, the recovery clip from the
  millisecond the swing ended, the carry idle again. The game's animation
  time is whole milliseconds, so that is the finest step measured.

Two comparisons:

    python preview.py <assets folder> <out folder> <before clips folder> [jobs [plots]]

the ram before (a folder of the clips of the build last played) beside after
(clips/), as battery_hit_*; and

    python preview.py <assets folder> <out folder> strikes [jobs [plots]]

the strikes (swing.STRIKES: the ram, the shove, the slam) beside one another,
as battery_hit_strikes*, with the ram beside each other strike alone as
battery_hit_<strike>.* and each other strike's beats large as
battery_hit_<strike>_fp.png and battery_hit_<strike>_tp_<view>.png. (`plots`
after the jobs: the plots and the cycle's numbers alone.)

Out, <name> being battery_hit or battery_hit_strikes:
- <name>_before_after.gif or <name>.gif (game speed, a frame every 1/50 s),
  _slow.gif (a fifth of it) and, with ffmpeg at hand, .mp4 (60 frames a
  second: three times at game speed, then once at a fifth): a column a set;
  first person, third person three-quarter, third person from the right;
- <name>_velocity.png: the battery's and the right hand's speed, the
  battery's acceleration and the hands' hold over the cycle;
- <name>_joints.png: when the battery, its turn, the elbow and the shoulder
  move, each as a share of its own top speed;
- <name>_frames.png: every 1/30 s on one sheet;
- battery_hit_tp_<view>.png: the ram's beats, third person, large;
- carry_idle_fp.png: the carry idle in first person, looking level, 20 and 35
  degrees down;
- metrics.txt or <name>_metrics.txt: the numbers.
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
STRIKES = sys.argv[3] == 'strikes'
JOBS = int(sys.argv[4]) if len(sys.argv) > 4 else 8
idle = rig.clip(HANDS + 'idle')
S = swing.Swing(rig, idle)
here = Path(__file__).resolve().parent / 'clips'
FPS = swing.FPS


def clip_set(folder, name):
    """A strike's four clips of a folder, with its two states' times (s)."""
    load = lambda stem: clips.parse_clip((Path(folder) / (stem + '.b-spline')).read_bytes())[0]
    user, item = (load(name), load(name + '_recover')), (load(name + '_electrobox'), load(name + '_recover_electrobox'))
    return {'user': user, 'item': item, 'swing': clips.clip_end(user[0]) / FPS, 'recover': clips.clip_end(user[1]) / FPS}


if STRIKES:
    SETS = {name[len('battery_hit_'):] or 'ram': clip_set(here, name) for name in swing.STRIKES}
    NAME, COLOURS = 'battery_hit_strikes', ('#2a78d6', '#eb6834', '#1baf7a')
else:
    SETS = {'before': clip_set(sys.argv[3], 'battery_hit'), 'after': clip_set(here, 'battery_hit')}
    NAME, COLOURS = 'battery_hit', ('#eb6834', '#2a78d6')
COLUMNS = list(SETS)
COLOUR = dict(zip(COLUMNS, COLOURS))
total = lambda name: SETS[name]['swing'] + SETS[name]['recover']
CYCLE = (-0.1, max(total(name) for name in COLUMNS) + 0.15)     # the carry before, the states, the carry after
TILE = (480, 270)
CORE = {'Hip', 'Spin', 'Spin_1', 'Chest', 'Neck', 'Head'}
VIEWS = {'three_quarter': (1.3, 0.2, 1.6), 'side': (1.9, 0.15, 0.3), 'front': (0.15, 0.2, 2.0)}
# every millisecond of the cycle (the game's animation time is whole milliseconds)
TS = np.arange(int(round(CYCLE[0] * 1000)), int(round(CYCLE[1] * 1000)) + 1) / 1000.0


def phase(name, t):
    return 'carry' if t < 0 or t >= total(name) else 'swing' if t < SETS[name]['swing'] else 'recovery'


def game_pose(name, t, look=0.0):
    """The carrier and the battery of clip set `name` at time t (s) of the
    cycle: the carry idle before the swing's state and after the recovery's,
    each state's clips from its start."""
    set_ = SETS[name]
    state = phase(name, t)
    if state == 'carry':
        world = rig.game_world(idle, 0, look)
        return world, rig.battery_world(world)
    k, at = (0, t) if state == 'swing' else (1, t - set_['swing'])
    world = rig.game_world(set_['user'][k], at * FPS, look)
    return world, rig.battery_world(world, at * FPS, set_['item'][k])


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
    text = '%s  first person  %s' % (name, phase(name, t)) if label is None else label
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
    text = '%s  third person, %s  %s' % (name, view.replace('_', ' '), phase(name, t)) if label is None else label
    ImageDraw.Draw(img).text((6, 4), text, fill=(240, 240, 240))
    return img, render.penetration(bat[0], core_vertices(world))


def grip(world, bw):
    """Each hand's distance from its hold on the battery, m. The ram's hands
    are fixed on the handles: the wrist from where the carry's hold, moved
    with the battery's offset bone, has it. The other strikes' hands turn on
    their handles (upright bars): the middle of the curled fingers from the
    bar's place."""
    move = np.linalg.inv(matrix(*S.item_world0['offset'])) @ matrix(*bw['offset'])
    if not STRIKES:
        return [np.linalg.norm(world[s + 'Hand'][1] - (matrix(*S.world0[s + 'Hand']) @ move)[3, :3]) for s in ('Left', 'Right')]
    fingers = lambda pose, s: np.mean([pose[s + f][1] for f in ('Index2', 'Middle2', 'Pinky2')], axis=0)
    return [np.linalg.norm(fingers(world, s) - (np.r_[fingers(S.world0, s), 1] @ move)[:3]) for s in ('Left', 'Right')]


def in_view(bw, world):
    """The battery's box before the eye (m): how far ahead, left to right, and below the look."""
    eye, r = render.fp_camera(world)
    e = (rig.battery.pose(bw) - eye) @ np.array(r).T
    return e[:, 2].min(), e[:, 2].max(), e[:, 0].min(), e[:, 0].max(), -e[:, 1].max(), -e[:, 1].min()


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
    strike = '  STRIKE' if abs(t - SETS[name]['swing']) < 1e-9 else ''
    if kind == 'big':
        return third_person(name, t, arg, (640, 480), '%s %.3f s  %s%s' % (name, t, phase(name, t), strike))[0]
    if kind == 'bigfp':
        return first_person(name, t, (640, 360), 0.0, '%s %.3f s  first person  %s%s' % (name, t, phase(name, t), strike))[0]
    if kind == 'track':                     # the measured places at one millisecond
        world, bw = game_pose(name, t)
        move = np.linalg.inv(matrix(*S.item_world0['offset'])) @ matrix(*bw['offset'])
        centre = (np.r_[S.centre0, 1] @ move)[:3]
        return np.r_[centre, world['RightHand'][1], world['RightForeArm'][1], world['RightArm'][1], world['LeftHand'][1],
                     world['LeftForeArm'][1], world['LeftArm'][1], grip(world, bw), bw['offset'][0].ravel()]
    raise ValueError(kind)


def timeline(width, t, name):
    """A strip under a column: its states over the cycle, the moment marked."""
    strip = Image.new('RGB', (width, 34), (18, 22, 28))
    draw = ImageDraw.Draw(strip)
    x = lambda s: 10 + (width - 20) * (s - CYCLE[0]) / (CYCLE[1] - CYCLE[0])
    swing_s = SETS[name]['swing']
    for a, b, text, colour in ((CYCLE[0], 0, 'carry', (60, 66, 76)), (0, swing_s, 'swing', (164, 108, 44)),
                               (swing_s, total(name), 'recovery', (58, 104, 150)), (total(name), CYCLE[1], 'carry', (60, 66, 76))):
        draw.rectangle((x(a), 16, x(b) - 1, 28), fill=colour)
        draw.text((x(a) + 4, 2), text, fill=(215, 215, 215))
    draw.text((x(swing_s) - 38, 2), 'strike', fill=(240, 240, 240))
    draw.rectangle((x(t) - 1, 12, x(t) + 1, 32), fill=(255, 255, 255))
    draw.text((width - 62, 2), '%+.3f s' % t, fill=(215, 215, 215))
    return strip


def render_tiles(pool, times):
    """Every column's three views at each time: {(name, time index, row): image}."""
    jobs = [(kind, name, t, arg) for t in times for kind, arg in (('fp', None), ('tp', 'three_quarter'), ('tp', 'side'))
            for name in COLUMNS]
    images = pool.map(tile, jobs, chunksize=4)
    keys = [(name, i, row) for i in range(len(times)) for row in range(3) for name in COLUMNS]
    return dict(zip(keys, images))


def frames_of(tiles, times, names):
    """The film's frames for the columns `names`: a column a set; first person, three-quarter, side."""
    W, H = TILE
    frames = []
    for i, t in enumerate(times):
        frame = Image.new('RGB', (W * len(names), H * 3 + 34))
        for c, name in enumerate(names):
            for row in range(3):
                frame.paste(tiles[name, i, row], (c * W, row * H))
            frame.paste(timeline(W, t, name), (c * W, H * 3))
        frames.append(frame)
    return frames


def save_gifs(frames, stem):
    frames[0].save(out / (stem + '.gif'), save_all=True, append_images=frames[1:],
                   duration=[20] * (len(frames) - 1) + [700], loop=0)
    frames[0].save(out / (stem + '_slow.gif'), save_all=True, append_images=frames[1:],
                   duration=[100] * (len(frames) - 1) + [1200], loop=0)


def save_film(frames, stem):
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
                        str(out / (stem + '.mp4'))], check=True)


def tracked(pool):
    return {name: np.array(pool.map(tile, [('track', name, float(t), None) for t in TS], chunksize=16)) for name in COLUMNS}


def plots(track):
    """<name>_velocity.png and <name>_joints.png."""
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    ts = TS
    surface, ink, ink2, muted, grid, axis = '#fcfcfb', '#0b0b0b', '#52514e', '#898781', '#e1e0d9', '#c3c2b7'
    plt.rcParams.update({'font.size': 10, 'axes.edgecolor': axis, 'axes.labelcolor': ink2, 'xtick.color': muted,
                         'ytick.color': muted, 'text.color': ink, 'figure.facecolor': surface, 'axes.facecolor': surface,
                         'savefig.facecolor': surface, 'lines.solid_capstyle': 'round', 'lines.solid_joinstyle': 'round'})
    speed = lambda name, cols: np.linalg.norm(np.diff(track[name][:, cols], axis=0), axis=1) / np.diff(ts)
    mid = (ts[1:] + ts[:-1]) / 2
    last = COLUMNS[-1]

    def dress(ax, title, unit, name=None):
        ax.set_title(title, loc='left', fontsize=11, color=ink, pad=8)
        ax.set_ylabel(unit)
        ax.grid(axis='y', color=grid, linewidth=1)
        ax.set_axisbelow(True)
        for side in ('top', 'right'):
            ax.spines[side].set_visible(False)
        if name:
            for edge in (0.0, SETS[name]['swing'], total(name)):
                ax.axvline(edge, color=axis, linewidth=1)
        ax.set_xlim(CYCLE)

    def states(ax, name):
        low, top = ax.get_ylim()
        for a, text in ((CYCLE[0], 'carry'), (0, 'swing'), (SETS[name]['swing'], 'recovery'), (total(name), 'carry')):
            ax.text(a + 0.006, top - (top - low) * 0.02, text, ha='left', va='top', fontsize=9, color=muted)

    def turning(name):
        """The battery's turn, degrees a second."""
        r = track[name][:, 23:32].reshape(-1, 3, 3)
        cos = (np.einsum('nij,nij->n', r[:-1], r[1:]) - 1) / 2
        return np.degrees(np.arccos(np.clip(cos, -1, 1))) / np.diff(ts)

    v = {name: speed(name, slice(0, 3)) for name in COLUMNS}
    panels = [("The battery's speed (its centre) over the cycle", 'm/s', v, mid)]
    if STRIKES:
        panels.append(("The right hand's speed", 'm/s', {name: speed(name, slice(3, 6)) for name in COLUMNS}, mid))
    panels.append(("The battery's acceleration along its path (speed gained a second); drawn to 400", 'm/s²',
                   {name: np.clip(np.diff(v[name]) / np.diff(mid), -420, 420) for name in COLUMNS}, ts[1:-1]))
    panels.append(('The worse hand off its hold on the battery', 'mm',
                   {name: track[name][:, 21:23].max(axis=1) * 1000 for name in COLUMNS}, ts))
    fig, axes = plt.subplots(len(panels), 1, figsize=(11, 3.5 * len(panels) if STRIKES else 10.5), sharex=True,
                             gridspec_kw={'height_ratios': [3] + [2] * (len(panels) - 2) + [1.6]})
    for ax, (title, unit, values, at) in zip(axes, panels):
        for name in COLUMNS:
            ax.plot(at, values[name], color=COLOUR[name], linewidth=2, label=name)
        dress(ax, title, unit, None if STRIKES else last)
        if STRIKES and unit != 'mm':            # the strikes' times differ: each one's strike marked on its own line
            for name in COLUMNS:
                k = np.searchsorted(at, SETS[name]['swing'])
                ax.plot([at[k]], [values[name][k]], marker='o', markersize=8, color=COLOUR[name], markeredgecolor=surface,
                        markeredgewidth=2, linestyle='none')
        ax.legend(loc='lower right' if unit == 'm/s²' else 'upper right', frameon=False, ncol=1 if unit == 'm/s' else len(COLUMNS))
    for ax, (_, unit, values, _) in zip(axes, panels):
        if unit == 'm/s':
            ax.set_ylim(0, max(x.max() for x in values.values()) * 1.2)
        elif unit == 'mm':
            ax.set_ylim(0, max(4.0, max(x.max() for x in values.values()) * 1.15))
        else:
            ax.set_ylim(-440, 440)
    axes[-1].set_xlabel("time from the swing state's start, s" + (' (a dot: the strike)' if STRIKES else ' (the strike at %.1f)' % SETS[last]['swing']))
    if not STRIKES:
        states(axes[0], last)
        ax = axes[0]
        note = lambda text, at, where: ax.annotate(text, at, where, fontsize=9, color=ink2,
                                                   arrowprops={'arrowstyle': '-', 'color': muted, 'linewidth': 1})
        swing_s, whole = SETS[last]['swing'], total(last)
        strike = np.searchsorted(mid, swing_s)
        gather = (mid > 0) & (mid < swing_s / 2)
        if v['before'][gather].max() > 0.6 * v['before'].max():
            k = np.flatnonzero(gather)[v['before'][gather].argmax()]
            note('before: the gather nearly as fast as the push\n(%.1f against %.1f m/s), a stop between them'
                 % (v['before'][k], v['before'].max()), (mid[k], v['before'][k]), (-0.092, 4.6))
        still = (v['before'] < 0.25) & (mid > 0.02) & (mid < whole - 0.02)
        runs = np.flatnonzero(np.diff(np.r_[0, still.astype(int), 0]))
        longest = max(zip(runs[::2], runs[1::2]), key=lambda r: r[1] - r[0], default=None)
        if longest and longest[1] - longest[0] >= 30:
            note('before: still for %.2f s' % ((longest[1] - longest[0]) / 1000), (mid[(longest[0] + longest[1]) // 2 + 20], 0.04), (0.285, 1.5))
        top = v['after'][strike - 3:strike + 3].max()
        if top > 0.97 * v['after'].max():
            note('after: fastest at the strike (%.1f m/s),\nthen carried on and checked' % top, (swing_s, top), (0.238, 5.5))
        jumps = np.diff(v['before']) / np.diff(mid)
        start = np.searchsorted(mid, 0.0)
        if jumps[start - 2:start + 3].max() > 420:
            axes[-2].annotate('before: speed jumps within 1 ms\n(%.0f m/s² at the start, %.0f at the strike)'
                              % (jumps[start - 2:start + 3].max(), jumps[strike - 3:strike + 3].min()), (0.003, 400), (0.022, 250),
                              fontsize=9, color=ink2, arrowprops={'arrowstyle': '-', 'color': muted, 'linewidth': 1})
    fig.tight_layout()
    fig.savefig(out / (NAME + '_velocity.png'), dpi=110)
    plt.close(fig)

    parts = (("the battery's place", slice(0, 3), '#2a78d6'), ("the battery's turn", None, '#eb6834'),
             ('the right elbow', slice(6, 9), '#1baf7a'), ('the right shoulder', slice(9, 12), '#eda100'))
    fig, axes = plt.subplots(len(COLUMNS), 1, figsize=(11, 3.8 * len(COLUMNS)), sharex=True, sharey=True)
    for ax, name in zip(axes, COLUMNS):
        still_shoulders = True
        for label, cols, c in parts:
            s = turning(name) if cols is None else speed(name, cols)
            if s.max() < 1e-3:
                continue
            still_shoulders &= cols != slice(9, 12)
            ax.plot(mid, s / s.max(), color=c, linewidth=2, label='%s (top %s at %.3f s)' % (
                label, '%.0f deg/s' % s.max() if cols is None else '%.1f m/s' % s.max(), mid[s.argmax()]))
        dress(ax, '%s: when each part moves (its speed as a share of its own top speed)' % name.capitalize()
              + ('; the shoulders never move' if still_shoulders else ''), 'share of top speed', name)
        ax.set_ylim(0, 1.3)
        ax.set_yticks([0, 0.25, 0.5, 0.75, 1.0])
        ax.legend(loc='upper right', bbox_to_anchor=(1.0, 0.94), frameon=False, fontsize=9, ncol=2)
        states(ax, name)
    axes[-1].set_xlabel("time from the swing state's start, s")
    fig.tight_layout()
    fig.savefig(out / (NAME + '_joints.png'), dpi=110)
    plt.close(fig)


def numbers(track):
    """The cycle every millisecond, as text: each set's speeds, jumps, stops, joints."""
    ts = TS
    mid = (ts[1:] + ts[:-1]) / 2
    lines = ['smoothness: the cycle carry -> swing -> recovery -> carry, every millisecond (the game\'s animation time)']
    for name in COLUMNS:
        d = track[name]
        swing_s, whole = SETS[name]['swing'], total(name)
        v = np.linalg.norm(np.diff(d[:, :3], axis=0), axis=1) * 1000
        a = np.diff(v) * 1000
        inside = (mid > 0.02) & (mid < whole - 0.02)
        still = inside & (v < 0.25)
        runs = np.flatnonzero(np.diff(np.r_[0, still.astype(int), 0]))
        stops = ', '.join('%.3f-%.3f' % (mid[i], mid[j - 1]) for i, j in zip(runs[::2], runs[1::2]) if j - i >= 10)
        at = lambda t: v[np.searchsorted(mid, t) - 2:np.searchsorted(mid, t) + 3]
        lines.append('  %s%s' % (name, ' (the strike at %.2f s, the carry again at %.2f s)' % (swing_s, whole) if STRIKES else ''))
        lines.append('    battery: top speed %.2f m/s at %.3f s; the gather\'s top %.2f m/s; the way back\'s top %.2f m/s'
                     % (v.max(), mid[v.argmax()], v[(mid > 0) & (mid < swing_s / 2)].max(), v[mid > swing_s + 0.1].max()))
        lines.append('    speed in the first millisecond of the swing %.2f m/s; just before and after the strike %.2f / %.2f m/s;'
                     ' in the last millisecond of the recovery %.2f m/s' % (at(0.0).max(), v[np.searchsorted(mid, swing_s) - 2],
                                                                            v[np.searchsorted(mid, swing_s) + 1], at(whole).max()))
        lines.append('    largest change of speed in one millisecond: %.2f m/s (%.0f m/s^2)' % (np.abs(np.diff(v)).max(), np.abs(a).max()))
        lines.append('    slower than 0.25 m/s for 10 ms or more inside the motion: %s' % (stops or 'never'))
        lines.append('    battery moved 25 ms into the swing %.1f cm, 50 ms in %.1f cm (a swing begun between two ticks starts over at the'
                     ' tick: what the restart shows)' % tuple(np.linalg.norm(d[np.searchsorted(ts, t), :3] - d[np.searchsorted(ts, 0.0), :3]) * 100
                                                              for t in (0.025, 0.05)))
        lines.append('    hands off their hold: left at most %.1f mm, right at most %.1f mm' % (d[:, 21].max() * 1000, d[:, 22].max() * 1000))
        if STRIKES:
            rest = np.searchsorted(ts, 0.0)
            lines.append('    hands moved from their carry places: left at most %.1f cm, right at most %.1f cm' % (
                np.linalg.norm(d[:, 12:15] - d[rest, 12:15], axis=1).max() * 100, np.linalg.norm(d[:, 3:6] - d[rest, 3:6], axis=1).max() * 100))
        for label, cols in (('right shoulder', slice(9, 12)), ('right elbow', slice(6, 9)), ('right hand', slice(3, 6)),
                            ('left shoulder', slice(18, 21)), ('left elbow', slice(15, 18)), ('left hand', slice(12, 15))):
            s = np.linalg.norm(np.diff(d[:, cols], axis=0), axis=1) * 1000
            lines.append('    %-14s top speed %.2f m/s at %.3f s' % (label, s.max(), mid[s.argmax()]))
        for rate in (60, 75, 100, 130):
            step = [np.linalg.norm(d[np.searchsorted(ts, (k + 1) / rate), :3] - d[np.searchsorted(ts, k / rate), :3]) * 100
                    for k in range(int(whole * rate))]
            lines.append('    drawn %3d times a second: the battery moves at most %.1f cm between two frames' % (rate, max(step)))
    for name in COLUMNS[1:] if STRIKES else COLUMNS[-1:]:
        d = track[name]
        v = np.linalg.norm(np.diff(d[:, :3], axis=0), axis=1) * 1000
        h = np.linalg.norm(np.diff(d[:, 3:6], axis=0), axis=1) * 1000
        if STRIKES:
            lines.append('  %s, every 10 ms: time s, battery speed m/s, forward speed m/s, right hand speed m/s, hands off mm' % name)
        else:
            lines.append('  %s, every 10 ms: time s, battery speed m/s, forward speed m/s, hands off mm' % name)
        for t in np.arange(0, total(name) + 1e-9, 0.01):
            i = np.searchsorted(ts, t)
            j = min(i, len(v) - 1)
            hand = ' %5.2f' % h[j] if STRIKES else ''
            lines.append('    %.2f %5.2f %+6.2f%s %5.1f%s' % (t, v[j], (d[j + 1, 2] - d[j, 2]) * 1000, hand, d[i, 21:23].max() * 1000,
                                                            '   <- the strike' if abs(t - SETS[name]['swing']) < 1e-9 else ''))
    return lines


def main():
    pool = multiprocessing.get_context('fork').Pool(JOBS)
    W, H = TILE
    if 'plots' in sys.argv[5:]:             # the plots and the cycle's numbers alone
        track = tracked(pool)
        plots(track)
        print('\n'.join(numbers(track)))
        return

    if not STRIKES:
        # the carry idle in first person, looking level and down
        sheet = Image.new('RGB', (640, 360 * 3))
        for k, look in enumerate((0, 20, 35)):
            sheet.paste(first_person(COLUMNS[-1], -0.05, (640, 360), look, 'carry idle, looking %d deg down' % look)[0], (0, k * 360))
        sheet.save(out / 'carry_idle_fp.png')

    # the films: all the columns and, of the strikes, the ram beside each other strike too
    films = [(NAME if STRIKES else NAME + '_before_after', COLUMNS)]
    if STRIKES:
        films += [('battery_hit_' + name, [COLUMNS[0], name]) for name in COLUMNS[1:]]
    fifty = [CYCLE[0] + k / 50 for k in range(int(round((CYCLE[1] - CYCLE[0]) * 50)) + 1)]
    tiles = render_tiles(pool, fifty)
    for stem, names in films:
        save_gifs(frames_of(tiles, fifty, names), stem)
    if shutil.which('ffmpeg'):
        sixty = [CYCLE[0] + k / 60 for k in range(int(round((CYCLE[1] - CYCLE[0]) * 60)) + 1)]
        tiles = render_tiles(pool, sixty)
        for stem, names in films:
            save_film(frames_of(tiles, sixty, names), stem)

    # every 1/30 s on one sheet, and the metrics' rows
    whole = max(total(name) for name in COLUMNS)
    thirty = sorted({round(k / 30, 6) for k in range(int(round(whole * 30)) + 1)} | {round(total(name), 6) for name in COLUMNS}
                    | {round(SETS[name]['swing'], 6) for name in COLUMNS})
    stills = frames_of(render_tiles(pool, thirty), thirty, COLUMNS)
    across = 3 if len(COLUMNS) == 2 else 2
    sheet = Image.new('RGB', (W * len(COLUMNS) * across, (H * 3 + 34) * ((len(stills) + across - 1) // across)))
    for k, frame in enumerate(stills):
        sheet.paste(frame, ((k % across) * W * len(COLUMNS), (k // across) * (H * 3 + 34)))
    sheet.save(out / (NAME + '_frames.png'))
    rows = {name: pool.map(tile, [('row', name, t, None) for t in thirty]) for name in COLUMNS}

    # the beats, large: the ram's in its own comparison, each other strike's among the strikes
    for name in COLUMNS[1:] if STRIKES else COLUMNS[-1:]:
        s = SETS[name]['swing']
        beats = [0.0, s * 0.25, s * 0.5, s * 0.75, s * 0.9, s, s + 0.02, s + 0.045, s + 0.11, s + 0.17, s + 0.23, total(name) - 0.06]
        stem = 'battery_hit_%s' % name if STRIKES else 'battery_hit'
        for view in VIEWS:
            big = pool.map(tile, [('big', name, t, view) for t in beats])
            sheet = Image.new('RGB', (640 * 3, 480 * ((len(beats) + 2) // 3)))
            for i, img in enumerate(big):
                sheet.paste(img, ((i % 3) * 640, (i // 3) * 480))
            sheet.save(out / ('%s_tp_%s.png' % (stem, view)))
        if STRIKES:
            big = pool.map(tile, [('bigfp', name, t, None) for t in beats])
            sheet = Image.new('RGB', (640 * 3, 360 * ((len(beats) + 2) // 3)))
            for i, img in enumerate(big):
                sheet.paste(img, ((i % 3) * 640, (i // 3) * 360))
            sheet.save(out / ('%s_fp.png' % stem))

    # between frames: every 1/240 s through the near plane
    between = {name: sum(pool.map(tile, [('near', name, k / 240, None) for k in range(int(round(total(name) * 240)) + 1)], chunksize=8))
               for name in (COLUMNS if STRIKES else COLUMNS[-1:])}
    track = tracked(pool)
    pool.close()
    plots(track)

    lines = []
    if STRIKES:
        for name in COLUMNS:
            lines.append('%s: time s | battery moved x y z cm, grip mm, FP on-screen %%, hands, near, body' % name)
            for t, r in zip(thirty, rows[name]):
                if t <= total(name) + 1e-9:
                    lines.append('%6.3f | %+6.1f %+6.1f %+6.1f %5.1f %5.0f%% %3s %3d %4d%s' % (
                        t, *r[0], r[1], r[2], r[3], r[4], r[5], '   <- the strike' if abs(t - SETS[name]['swing']) < 1e-9 else ''))
            lines.append('%s, every 1/240 s: %d triangles crossing the near plane in view' % (name, between[name]))
            lines.append('')
        lines.append('where the battery is before the eye (m): ahead, across (- left, + right), below the look; then the same on the')
        lines.append('plane of the strike\'s rays, 0.7 m ahead (battery.rs: each strike\'s grid of 5 x 3 points there)')
        for name in COLUMNS:
            for label, t in (('at the strike', SETS[name]['swing']), ('0.045 s on', SETS[name]['swing'] + 0.045)):
                world, bw = game_pose(name, t)
                near, far, left, right, top, bottom = in_view(bw, world)
                scale = 0.7 / far
                lines.append('  %-6s %-14s ahead %.2f..%.2f, across %+.2f..%+.2f, below %.2f..%.2f; on the plane: across %+.2f..%+.2f, below %.2f..%.2f'
                             % (name, label, near, far, left, right, top, bottom, left * scale, right * scale, top * scale, bottom * scale))
        lines.append('')
    else:
        lines.append('time s | before: battery moved cm, grip mm, FP on-screen %, near, body'
                     ' | after: battery moved x y z cm, grip mm, FP on-screen %, hands, near, body')
        swing_s = SETS['after']['swing']
        for t, b, a in zip(thirty, rows['before'], rows['after']):
            lines.append('%6.3f | %5.1f %6.1f %5.0f%% %3d %4d | %+6.1f %+6.1f %+6.1f %5.1f %5.0f%% %3s %3d %4d%s' % (
                t, np.linalg.norm(b[0]), b[1], b[2], b[4], b[5], *a[0], a[1], a[2], a[3], a[4], a[5],
                '   <- the strike' if abs(t - swing_s) < 1e-9 else ''))
        lines.append('after, every 1/240 s: %d triangles crossing the near plane in view' % between['after'])
        lines.append('')
    lines += numbers(track)
    (out / (NAME + '_metrics.txt' if STRIKES else 'metrics.txt')).write_text('\n'.join(lines) + '\n')
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
