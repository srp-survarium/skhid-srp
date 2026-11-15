"""The battery swing, authored on the stock rig like an animator would:
keyframes for the battery's path and the upper body over the carry idle,
both arms solved onto the battery's handles every frame, the camera (the
Item bone) punched on the impact. Written as two clips at 30 fps: the swing
(wind-up and strike, its last frame the impact) and the recovery (follow-
through and back to the carry idle).

Body frame: x right, y up, z forward (the carrier faces +z; left-handed, as
the engine). Angles in degrees in the keys, radians inside."""
import math

import numpy as np

import clips
from rig import matrix

FPS = 30.0
IMPACT = 6          # frames: 0.2 s, the swing clip's end and the strike
END = 17            # frames: 0.567 s, the recovery's end (back on the carry idle)

# keys: frame -> values; `hold` keys have zero tangent (an extreme), others pass through with speed.
# A forward ram (the owner, 2026-11-15: "the battery should clearly move forward with hands
# straightening out"), timed on Dirty Bomb's two-handed objective shove (measured as a reference,
# private/renders/battery-hit/dirty-bomb-reference): pulled in to the chest, elbows bent, by 0.1 s,
# one still frame cocked, struck out in 2 frames until both arms are straight (the impact at 0.2 s,
# the strike), held there 3 frames, a soft recovery to the carry. Values: the battery's move from
# its carry pose (its centre's offset in metres, body frame: x right, y up, z forward; its turn
# about the centre in degrees), which its own clip's `offset` bone makes; the arms follow it, the
# only bones of the carrier its clips can move. The impact is as far forward as both arms reach
# with the carry's shoulders (the right one stands back): 0.29 m, the elbows straight (175 deg),
# the battery at the lower centre of the first-person view.
KEYS = {
    #            frame: (cx,     cy,     cz,     pitch, yaw,  roll)
    'rest':      (0,  (0.000,  0.000,  0.000,    0,     0,    0)),
    'pull':      (3,  (-0.080, 0.120,  0.050,    0,    12,    0)),
    'cocked':    (4,  (-0.080, 0.120,  0.050,    0,    12,    0)),
    'impact':    (IMPACT, (-0.161, 0.243, 0.288, 12.6, 17.3, 10.5)),
    'held':      (9,  (-0.161, 0.243,  0.288,   12.6,  17.3, 10.5)),
    'recover':   (END, (0.000, 0.000,  0.000,    0,     0,    0)),
}
HOLD = {'rest', 'pull', 'cocked', 'impact', 'held', 'recover'}


def curve(t):
    """The keyed values at frame t: cubic Hermite, zero tangent at holds,
    Catmull-Rom speed elsewhere."""
    keys = sorted(KEYS.items(), key=lambda kv: kv[1][0])
    frames = [k[1][0] for k in keys]
    values = [np.array(k[1][1], float) for k in keys]
    names = [k[0] for k in keys]
    if t <= frames[0]:
        return values[0]
    if t >= frames[-1]:
        return values[-1]
    i = max(j for j in range(len(frames) - 1) if frames[j] <= t)

    def tangent(j):
        if names[j] in HOLD or j == 0 or j == len(frames) - 1:
            return np.zeros_like(values[j])
        return (values[j + 1] - values[j - 1]) / (frames[j + 1] - frames[j - 1])
    h = frames[i + 1] - frames[i]
    s = (t - frames[i]) / h
    h00, h10, h01, h11 = 2*s**3 - 3*s**2 + 1, s**3 - 2*s**2 + s, -2*s**3 + 3*s**2, s**3 - s**2
    return h00 * values[i] + h10 * h * tangent(i) + h01 * values[i + 1] + h11 * h * tangent(i + 1)


def rot_axis(axis, angle):
    """Row-vector rotation by `angle` about the unit `axis` (p @ R)."""
    a = np.array(axis, float) / np.linalg.norm(axis)
    c, s = math.cos(angle), math.sin(angle)
    x, y, z = a
    # column-vector Rodrigues, transposed for row vectors
    m = np.array([[c + x*x*(1-c), x*y*(1-c) - z*s, x*z*(1-c) + y*s],
                  [y*x*(1-c) + z*s, c + y*y*(1-c), y*z*(1-c) - x*s],
                  [z*x*(1-c) - y*s, z*y*(1-c) + x*s, c + z*z*(1-c)]])
    return m.T


class Pose:
    """Local transforms; world on demand."""
    def __init__(self, rig, local):
        self.rig = rig
        self.local = {n: (r.copy(), p.copy()) for n, (r, p) in local.items()}

    def world(self):
        return clips.world_pose(self.local, self.rig.parents, self.rig.names)

    def set_world(self, name, r, p=None):
        world = self.world()
        parent = self.rig.parents.get(name)
        pr, pp = world[parent] if parent in world else (np.eye(3), np.zeros(3))
        lr = r @ pr.T
        lp = self.local[name][1] if p is None else (p - pp) @ pr.T
        self.local[name] = (lr, lp)

    def rotate_world(self, name, rot):
        """Turn a bone (and its subtree) by `rot` about its own origin, in world axes."""
        r, p = self.world()[name]
        self.set_world(name, r @ rot)


def frame_of(r_world):
    """A rotation's rows as (right, up, forward) unit axes."""
    return r_world


def solve_arm(pose, side, wrist_target, hand_rot_target, elbow_hint):
    """Two-bone IK: the upper arm and forearm reach the wrist target with
    their idle lengths, the elbow in the plane of the hint, then the hand
    takes its target rotation."""
    arm, fore, hand = side + 'Arm', side + 'ForeArm', side + 'Hand'
    w = pose.world()
    s = w[arm][1]
    e0, h0 = w[fore][1], w[hand][1]
    l1, l2 = np.linalg.norm(e0 - s), np.linalg.norm(h0 - e0)
    d = wrist_target - s
    dist = float(np.clip(np.linalg.norm(d), abs(l1 - l2) + 1e-3, (l1 + l2) * 0.999))
    dn = d / np.linalg.norm(d)
    along = (l1*l1 - l2*l2 + dist*dist) / (2 * dist)
    height = math.sqrt(max(l1*l1 - along*along, 0))
    pole = elbow_hint - s
    pole = pole - dn * np.dot(pole, dn)
    pole /= max(np.linalg.norm(pole), 1e-9)
    elbow = s + dn * along + pole * height
    wrist = s + dn * dist
    # turn the upper arm so its bone points at the new elbow, keeping its roll by the plane
    def aim(bone, origin, old_tip, new_tip, old_hint, new_hint):
        r, _ = pose.world()[bone]
        a0 = (old_tip - origin); a0 /= np.linalg.norm(a0)
        a1 = (new_tip - origin); a1 /= np.linalg.norm(a1)
        n0 = np.cross(a0, old_hint - origin); n0 /= max(np.linalg.norm(n0), 1e-9)
        n1 = np.cross(a1, new_hint - origin); n1 /= max(np.linalg.norm(n1), 1e-9)
        f0 = np.array([a0, n0, np.cross(a0, n0)])
        f1 = np.array([a1, n1, np.cross(a1, n1)])
        pose.set_world(bone, r @ (f0.T @ f1))
    aim(arm, s, e0, elbow, h0, wrist)
    w = pose.world()
    e_now, h_now = w[fore][1], w[hand][1]
    aim(fore, e_now, h_now, wrist, s, s)
    pose.set_world(hand, hand_rot_target)
    return np.linalg.norm(pose.world()[hand][1] - wrist)


class Swing:
    """The carry idle as the game mixes it, the battery's own bones and the
    hands' hold on it there: what every authored frame starts from.

    The game plays a battery state's player clips on the arms alone (bone
    mask 2: clavicles to fingers) over the stand set's base clip, and its two
    battery clips on the battery's own skeleton; the battery hangs from the
    carrier's Weapon bone (a mask-1 bone: the base clip's, still in every
    battery state) through its own `offset` bone. So the battery moves by its
    clip's `offset` bone, the hands follow it by the arm bones, and nothing
    else of the carrier (spine, head, the Item camera) can move."""
    def __init__(self, rig, idle):
        self.rig = rig
        self.base_local = {n: (r.copy(), p.copy()) for n, (r, p) in clips.local_pose(idle, rig.names, 0).items()}
        base = clips.local_pose(rig.base, rig.names, 0)
        for n in rig.names:                       # the game's mix of the carry idle
            if not rig.masks.get(n, 0) & 2:
                self.base_local[n] = base[n]
        self.world0 = Pose(rig, self.base_local).world()
        self.item_local0 = clips.local_pose(rig.item_idle, rig.item_names, 0)
        self.item_world0 = rig.battery_world_local(self.world0, self.item_local0)
        self.centre0 = rig.battery.pose(self.item_world0).mean(axis=0)
        self.hints = {s: self.world0[s + 'ForeArm'][1] for s in ('Left', 'Right')}

    def delta(self, v):
        """The battery's move (model space, row vectors) for key values v."""
        cx, cy, cz, pitch, yaw, roll = v
        r = (rot_axis([1, 0, 0], math.radians(-pitch)) @ rot_axis([0, 1, 0], math.radians(-yaw))
             @ rot_axis([0, 0, 1], math.radians(roll)))
        about = np.eye(4); about[3, :3] = -self.centre0
        back = np.eye(4); back[3, :3] = self.centre0 + np.array([cx, cy, cz])
        turn = np.eye(4); turn[:3, :3] = r
        return about @ turn @ back

    def pose(self, v):
        """(the player's pose, the battery's own bones' locals) for key values v."""
        d = self.delta(v)
        rig = self.rig
        # the battery: its offset bone carries it (key_object, its parent, stays)
        offset = matrix(*self.item_world0['offset']) @ d
        key = matrix(*self.item_world0['key_object'])
        local = offset @ np.linalg.inv(key)
        item_local = dict(self.item_local0)
        item_local['offset'] = (local[:3, :3], local[3, :3])
        # the hands keep their hold on it: their idle frames moved with it
        pose = Pose(rig, self.base_local)
        for side in ('Left', 'Right'):
            target = matrix(*self.world0[side + 'Hand']) @ d
            solve_arm(pose, side, target[3, :3], target[:3, :3], self.hints[side])
        return pose, item_local


def author(rig, idle):
    """Every frame's (player pose, battery bones' locals), 0..END."""
    swing = Swing(rig, idle)
    return [swing.pose(curve(float(f))) for f in range(END + 1)]


def write(names, scales, locals_, first, last):
    """A clip of frames [first, last] (`locals_[f]`: each bone's local
    rotation and position), rebased to 0, each bone's nine channels fitted
    with the engine's basis; the scale channels `scales`' (a clip of the same
    bones) rest values; a channel that does not move is a constant."""
    times = [float(f - first) for f in range(first, last + 1)]
    out = []
    for name, scale_channels in zip(names, scales):
        values = [[] for _ in range(9)]
        prev = None
        for f in range(first, last + 1):
            r, p = locals_[f][name]
            e = clips.euler(r)
            if prev is not None:
                e = [a + 2 * math.pi * round((b - a) / (2 * math.pi)) for a, b in zip(e, prev)]
            prev = e
            for k in range(3):
                values[k].append(float(p[k]))
                values[3 + k].append(float(e[k]))
            for k in range(3):
                values[6 + k].append(float(scale_channels[6 + k].evaluate(0)))
        channels = []
        for k in range(9):
            vs = values[k]
            if max(vs) - min(vs) < 1e-6:
                channels.append(clips.Channel.constant(vs[0], times[-1]))
            else:
                channels.append(clips.Channel.fit(times, vs))
        out.append(channels)
    return out


def clips_of(rig, idle, frames):
    """The four clips: (player swing, player recovery, battery swing, battery recovery)."""
    player = [pose.local for pose, _ in frames]
    item = [item for _, item in frames]
    return (write(rig.names, idle, player, 0, IMPACT), write(rig.names, idle, player, IMPACT, END),
            write(rig.item_names, rig.item_idle, item, 0, IMPACT), write(rig.item_names, rig.item_idle, item, IMPACT, END))
