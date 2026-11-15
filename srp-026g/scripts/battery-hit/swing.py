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
# Designed for the first-person view (the Item bone, 65 degrees vertical, looking level), where
# the carry idle shows nothing (the battery is 40 degrees and more below the view): the battery
# rises in from the lower right (lift), is cocked high at the right edge of the screen with the
# chest wound right (wind-up; the carry idle already stands turned right), sweeps down across the
# lower half (impact at the centre, both hands on screen), leaves at the lower left (follow-
# through) and drops back out of view. In third person: up over the right shoulder, across the
# chest, down past the left hip. The camera is a child of Root and only dips with the punch. The
# offsets were solved so the battery's centre lands on a chosen screen point at a chosen depth
# with both arms within reach of the handles (preview.py prints what each frame shows).
KEYS = {
    #            frame: (cx,     cy,     cz,    pitch,  yaw,  roll,  twist, lean, kick_pitch, kick_yaw)
    'rest':      (0,  (0.000,  0.000,  0.000,    0,     0,     0,     0,    0,    0,    0)),
    'lift':      (2,  (0.078,  0.368,  0.185,  -39,    -7.5,  10.1,  -0.4,  9,    0,    0)),
    'windup':    (4,  (-0.030, 0.390,  0.200,    2.2,  29.7, -42.6, -13.5, -5.4,  0,    0)),
    'impact':    (6,  (0.076,  0.449,  0.196,    7.3,   4.2,  -9.5,  19.9, 13,    0,    0)),
    'follow':    (9,  (0.138,  0.234,  0.222,  -11.3,   3.4, -42.9,  45,    3.4, -5.0,  2.5)),
    'return':    (13, (-0.042, 0.139,  0.183,   -5,     2,   -15,    20,    1,   -1.0,  0.5)),
    'recover':   (END, (0.000, 0.000,  0.000,    0,     0,     0,     0,    0,    0,    0)),
}
HOLD = {'rest', 'windup', 'follow', 'recover'}


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
    """The carry idle's pose, the battery's attachment and grips there: what
    every authored frame starts from."""
    def __init__(self, rig, idle):
        self.rig = rig
        self.base_local = clips.local_pose(idle, rig.names, 0)
        bw = Pose(rig, self.base_local).world()
        self.yflip = matrix(clips.rotation([0, math.pi, 0]), [0, 0, 0])
        self.battery0 = self.yflip @ matrix(*bw['Weapon'])         # the battery root in model space
        self.centre0 = rig.battery.pose(rig.battery_world(bw)).mean(axis=0)
        self.grips = {s: matrix(*bw[s + 'Hand']) @ np.linalg.inv(self.battery0) for s in ('Left', 'Right')}
        self.hints = {s: bw[s + 'ForeArm'][1] for s in ('Left', 'Right')}
        self.chest0 = matrix(*bw['Chest'])

    def pose(self, v):
        """The pose for one set of key values (KEYS' columns)."""
        cx, cy, cz, pitch, yaw, roll, twist, lean, kick_p, kick_y = v
        pose = Pose(self.rig, self.base_local)
        # the upper body: twist about the vertical, lean about the right axis, spread over the spine
        for bone, share in (('Spin', 0.3), ('Spin_1', 0.3), ('Chest', 0.4)):
            pose.rotate_world(bone, rot_axis([0, 1, 0], math.radians(-twist) * share)
                              @ rot_axis([1, 0, 0], math.radians(lean) * share))
        # the head keeps looking ahead (third person); the camera is the Item bone, a child of Root
        pose.rotate_world('Neck', rot_axis([0, 1, 0], math.radians(twist) * 0.7))
        turned = np.linalg.inv(self.chest0) @ matrix(*pose.world()['Chest'])
        # the battery: its keyed offset about its centre, carried with the chest
        r = (rot_axis([1, 0, 0], math.radians(-pitch)) @ rot_axis([0, 1, 0], math.radians(-yaw))
             @ rot_axis([0, 0, 1], math.radians(roll)))
        about = np.eye(4); about[3, :3] = -self.centre0
        back = np.eye(4); back[3, :3] = self.centre0 + np.array([cx, cy, cz])
        turn = np.eye(4); turn[:3, :3] = r
        battery = self.battery0 @ about @ turn @ back @ turned     # carried by the turned chest
        # the hands on its handles
        for side in ('Left', 'Right'):
            target = self.grips[side] @ battery
            hint = (np.r_[self.hints[side], 1] @ turned)[:3]
            solve_arm(pose, side, target[3, :3], target[:3, :3], hint)
        # the Weapon bone carries the battery (the game attaches it there)
        weapon = np.linalg.inv(self.yflip) @ battery
        pose.set_world('Weapon', weapon[:3, :3], weapon[3, :3])
        # the camera punch
        r_item, _ = pose.world()['Item']
        pose.set_world('Item', rot_axis([1, 0, 0], math.radians(-kick_p))
                       @ rot_axis([0, 1, 0], math.radians(kick_y)) @ r_item)
        return pose


def author(rig, idle):
    """Every frame's local pose, 0..END."""
    swing = Swing(rig, idle)
    return [swing.pose(curve(float(f))) for f in range(END + 1)]


def write(rig, idle, frames, first, last):
    """A clip of frames [first, last], rebased to 0, each bone's nine channels
    fitted with the engine's basis; a bone that does not move keeps the idle's
    channels at their rest value."""
    times = [float(f - first) for f in range(first, last + 1)]
    out = []
    for name, idle_channels in zip(rig.names, idle):
        values = [[] for _ in range(9)]
        prev = None
        for f in range(first, last + 1):
            r, p = frames[f].local[name]
            e = clips.euler(r)
            if prev is not None:
                e = [a + 2 * math.pi * round((b - a) / (2 * math.pi)) for a, b in zip(e, prev)]
            prev = e
            for k in range(3):
                values[k].append(float(p[k]))
                values[3 + k].append(float(e[k]))
            for k in range(3):
                values[6 + k].append(float(idle_channels[6 + k].evaluate(0)))
        channels = []
        for k in range(9):
            vs = values[k]
            if max(vs) - min(vs) < 1e-6:
                channels.append(clips.Channel.constant(vs[0], times[-1]))
            else:
                channels.append(clips.Channel.fit(times, vs))
        out.append(channels)
    return out
