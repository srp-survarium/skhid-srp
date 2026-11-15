"""The battery swing, authored on the stock rig like an animator would: a
keyed curve for each thing that moves (the battery's place and turn, each
shoulder, each elbow), each with its own timing, the arms solved onto the
battery's handles every sample. Written as two clips: the swing (the gather
and the drive, its last frame the strike) and the recovery (the
follow-through, the settle and the way back to the carry idle).

Body frame: x right, y up, z forward (the carrier faces +z; left-handed, as
the engine). Times in seconds and angles in degrees in the keys; frames (30 a
second) and radians inside."""
import math

import numpy as np

import clips
from rig import matrix

FPS = 30.0
IMPACT = 6          # frames: 0.2 s, the swing clip's end and the strike (battery_hit::SWING_S)
END = 16.5          # frames: 0.55 s, the recovery's end, back on the carry idle (RECOVER_S 0.35 s)
STEP = 0.5          # frames between a clip's samples at most: 60 a second (swing.track adds where needed)
STRIKE = IMPACT / FPS
REST = END / FPS


class Curve:
    """One keyed value over the swing's time (s). Keys are (time, value),
    (time, value, speed) or (time, value, speed, acceleration); between two
    keys the value is the quintic through both keys' value, speed and
    acceleration, so all three are continuous over the whole curve. A speed or
    an acceleration not given is that of the parabola through the key and its
    two neighbours; the first and the last key are at rest (both 0)."""
    def __init__(self, *keys):
        t, p = [float(k[0]) for k in keys], [float(k[1]) for k in keys]
        n = len(keys)
        v, a = [0.0] * n, [0.0] * n
        for i in range(1, n - 1):
            h0, h1 = t[i] - t[i - 1], t[i + 1] - t[i]
            d0, d1 = (p[i] - p[i - 1]) / h0, (p[i + 1] - p[i]) / h1
            v[i] = (h1 * d0 + h0 * d1) / (h0 + h1)
            a[i] = 2 * (d1 - d0) / (h0 + h1)
        for i, k in enumerate(keys):
            if len(k) > 2 and k[2] is not None:
                v[i] = float(k[2])
            if len(k) > 3 and k[3] is not None:
                a[i] = float(k[3])
        self.t, self.p, self.v, self.a = t, p, v, a

    def __call__(self, time):
        t, p, v, a = self.t, self.p, self.v, self.a
        if time <= t[0]:
            return p[0]
        if time >= t[-1]:
            return p[-1]
        i = max(j for j in range(len(t) - 1) if t[j] <= time)
        h = t[i + 1] - t[i]
        s = (time - t[i]) / h
        s2, s3, s4, s5 = s * s, s ** 3, s ** 4, s ** 5
        return ((1 - 10 * s3 + 15 * s4 - 6 * s5) * p[i] + (s - 6 * s3 + 8 * s4 - 3 * s5) * h * v[i]
                + (0.5 * s2 - 1.5 * s3 + 1.5 * s4 - 0.5 * s5) * h * h * a[i]
                + (0.5 * s3 - s4 + 0.5 * s5) * h * h * a[i + 1]
                + (-4 * s3 + 7 * s4 - 3 * s5) * h * v[i + 1] + (10 * s3 - 15 * s4 + 6 * s5) * p[i + 1])


# A forward ram with weight (the owner, 2026-11-15: "the battery should clearly move forward with
# hands straightening out", then "it should feel human like", "with momentum and force"), its
# beats those of Dirty Bomb's two-handed objective shove (measured as a reference only,
# private/renders/battery-hit/dirty-bomb-reference):
#   the gather (to 0.10 s): slow from rest, the battery drawn back and up against the body, the
#     shoulders up and back before it, the elbows folding and leaning out;
#   the drive (0.10 to 0.20 s): one push that only gains speed, the shoulders thrown forward
#     ahead of the arms, the battery's turn trailing its travel; the strike is on the fastest
#     frame (0.2 s, the swing clip's end), three quarters of the way out;
#   the follow-through (to 0.245 s): the battery's weight carries it on to full reach, pulling
#     the arms straight and the shoulders forward, until they check it;
#   the settle (to 0.30 s): it sinks a little, its turn still arriving;
#   the way back (to 0.55 s): one reach, slow to start and slow to land, the shoulders easing
#     last.
# The first 50 ms hardly move: a carrier sees his own swing start, then start over at the next
# tick (his input is moved onto it, snap_player_inputs 0x2D4E40). Nothing stops on the way but
# at the turn at full reach, and no two things turn on the same millisecond.
#
# The battery's values are its move from the carry pose: its centre's offset (cm: x right, y up,
# z forward) and its turn about that centre (degrees), which its own clip's `offset` bone makes;
# the shoulders and the arms, the only bones of the carrier its clips can move (bone mask 2, the
# clavicles to the fingers), follow it. Full reach (16 cm left, 24 up, 29 forward, squared to
# the front) has the battery at the lower centre of the first-person view. A key is (time,
# value[, speed[, acceleration]]).
BATTERY = {
    'x': Curve((0, 0), (0.100, 1.5, 0), (STRIKE, -10.6, -230, -300), (0.252, -16.1, 0, 500), (0.304, -15.4, 18), (REST, 0)),
    'y': Curve((0, 0), (0.095, 4.0, 45), (STRIKE, 18.0, 370, 0), (0.247, 24.3, 0, -900), (0.300, 22.9, -38), (REST, 0)),
    'z': Curve((0, 0), (0.104, -4.0, 0), (STRIKE, 20.7, 440, 0), (0.242, 28.8, 0, -900), (0.296, 27.5, -30), (REST, 0)),
    'pitch': Curve((0, 0), (0.105, -2.0, 0), (STRIKE, 7.2, 200), (0.262, 13.7, 0), (0.325, 12.4, -10), (REST, 0)),
    'yaw': Curve((0, 0), (0.108, -3.0, 0), (STRIKE, 11.0, 190), (0.258, 18.6, 0), (0.320, 17.1, -14), (REST, 0)),
    'roll': Curve((0, 0), (0.112, -2.0, 0), (STRIKE, 5.2, 185), (0.268, 11.4, 0), (0.330, 10.3, -8), (REST, 0)),
}
# Each shoulder joint's move from its carry place (cm), made by turning its clavicle: forward
# (z) and up (y). Past these keys a shoulder also gives way to its arm: as the arm nears its full
# length (REACH) the shoulder is drawn on toward the hand, so the battery's weight, not a key,
# pulls it forward at the end of the push.
SHOULDER = {
    'Left': {'z': Curve((0, 0), (0.086, -1.4, 0), (0.240, 2.9, 0), (0.40, 1.9), (REST, 0)),
             'y': Curve((0, 0), (0.090, 1.5, 0), (0.262, -0.5, 0), (REST, 0))},
    'Right': {'z': Curve((0, 0), (0.082, -1.8, 0), (0.248, 1.5, 0), (0.40, 1.0), (REST, 0)),
              'y': Curve((0, 0), (0.086, 1.8, 0), (0.258, -0.6, 0), (REST, 0))},
}
REACH = 1.012        # of the arm's length: what a shoulder gives way to
REACH_SOFT = 0.02    # how gradually: the elbows open to 155 degrees
# Each elbow's lean out of its carry plane (cm at the elbow: out to its side, and back).
ELBOW = {
    'Left': {'out': Curve((0, 0), (0.092, 5.0, 0), (0.215, -1.0, -30), (0.29, 0.6), (REST, 0)),
             'back': Curve((0, 0), (0.096, 3.0, 0), (0.21, -1.0), (0.30, 0.0), (REST, 0))},
    'Right': {'out': Curve((0, 0), (0.088, 6.0, 0), (0.212, -1.2, -34), (0.29, 0.8), (REST, 0)),
              'back': Curve((0, 0), (0.092, 3.5, 0), (0.21, -1.2), (0.30, 0.0), (REST, 0))},
}


def motion(t):
    """Every keyed value at time t (s)."""
    return {'battery': tuple(BATTERY[k](t) for k in ('x', 'y', 'z', 'pitch', 'yaw', 'roll')),
            'shoulder': {s: (SHOULDER[s]['z'](t), SHOULDER[s]['y'](t)) for s in SHOULDER},
            'elbow': {s: (ELBOW[s]['out'](t), ELBOW[s]['back'](t)) for s in ELBOW}}


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


def solve_arm(pose, side, wrist_target, hand_rot_target, elbow_hint):
    """Two-bone IK: the upper arm and forearm reach the wrist target with
    their idle lengths, the elbow in the plane of the hint, then the hand
    takes its target rotation. Returns how much of the arm's length the reach
    takes (1 is straight: beyond it the hand cannot hold its place)."""
    arm, fore, hand = side + 'Arm', side + 'ForeArm', side + 'Hand'
    w = pose.world()
    s = w[arm][1]
    e0, h0 = w[fore][1], w[hand][1]
    l1, l2 = np.linalg.norm(e0 - s), np.linalg.norm(h0 - e0)
    d = wrist_target - s
    reach = float(np.linalg.norm(d)) / (l1 + l2)
    dist = float(np.clip(np.linalg.norm(d), abs(l1 - l2) + 1e-3, (l1 + l2) * 0.999))
    dn = d / np.linalg.norm(d)
    along = (l1*l1 - l2*l2 + dist*dist) / (2 * dist)
    height = math.sqrt(max(l1*l1 - along*along, 0))
    pole = elbow_hint - s
    pole = pole - dn * np.dot(pole, dn)
    pole /= max(np.linalg.norm(pole), 1e-9)
    elbow = s + dn * along + pole * height
    wrist = s + dn * dist

    # turn the bone so it points at its new tip, keeping its roll by the arm's plane
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
    return reach


class Swing:
    """The carry idle as the game mixes it, the battery's own bones and the
    hands' hold on it there: what every authored sample starts from.

    The game plays a battery state's player clips on the arms alone (bone
    mask 2: clavicles to fingers) over the stand set's base clip, and its two
    battery clips on the battery's own skeleton; the battery hangs from the
    carrier's Weapon bone (a mask-1 bone: the base clip's, still in every
    battery state) through its own `offset` bone. So the battery moves by its
    clip's `offset` bone, the shoulders and the hands follow it by the arm
    bones, and nothing else of the carrier (spine, head, the Item camera) can
    move."""
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
        self.arm = {s: np.linalg.norm(self.world0[s + 'ForeArm'][1] - self.world0[s + 'Arm'][1])
                    + np.linalg.norm(self.world0[s + 'Hand'][1] - self.world0[s + 'ForeArm'][1]) for s in ('Left', 'Right')}

    def delta(self, v):
        """The battery's move (model space, row vectors) for its keyed values v (cm, degrees)."""
        cx, cy, cz, pitch, yaw, roll = v
        r = (rot_axis([1, 0, 0], math.radians(-pitch)) @ rot_axis([0, 1, 0], math.radians(-yaw))
             @ rot_axis([0, 0, 1], math.radians(roll)))
        about = np.eye(4); about[3, :3] = -self.centre0
        back = np.eye(4); back[3, :3] = self.centre0 + np.array([cx, cy, cz]) / 100
        turn = np.eye(4); turn[:3, :3] = r
        return about @ turn @ back

    def pose(self, t):
        """(the player's pose, the battery's own bones' locals, each arm's reach) at time t (s)."""
        m = motion(t)
        d = self.delta(m['battery'])
        rig = self.rig
        # the battery: its offset bone carries it (key_object, its parent, stays)
        offset = matrix(*self.item_world0['offset']) @ d
        key = matrix(*self.item_world0['key_object'])
        local = offset @ np.linalg.inv(key)
        item_local = dict(self.item_local0)
        item_local['offset'] = (local[:3, :3], local[3, :3])
        pose = Pose(rig, self.base_local)
        reach = {}
        for side, out in (('Left', -1.0), ('Right', 1.0)):
            # the shoulder joint moved by its clavicle's turn: its keys, then on toward the
            # hand by what the arm lacks of the reach (a smooth limit, so nothing snaps)
            pivot, joint = self.world0[side + 'Clavicle'][1], self.world0[side + 'Arm'][1]
            forward, up = m['shoulder'][side]
            r0 = joint - pivot
            r1 = r0 + np.array([0, up, forward]) / 100
            r1 *= np.linalg.norm(r0) / np.linalg.norm(r1)
            wrist = (matrix(*self.world0[side + 'Hand']) @ d)[3, :3]
            for _ in range(4):
                to = wrist - (pivot + r1)
                far = np.linalg.norm(to) / self.arm[side]
                over = REACH_SOFT * math.log1p(math.exp((far - REACH) / REACH_SOFT))
                r1 = r1 + to / np.linalg.norm(to) * over * self.arm[side]
                r1 *= np.linalg.norm(r0) / np.linalg.norm(r1)
            axis = np.cross(r0, r1)
            if np.linalg.norm(axis) > 1e-12:
                angle = math.atan2(np.linalg.norm(axis), float(np.dot(r0, r1)))
                pose.rotate_world(side + 'Clavicle', rot_axis(axis, angle))
            # the hands keep their hold on the battery: their idle frames moved with it
            target = matrix(*self.world0[side + 'Hand']) @ d
            lean_out, lean_back = m['elbow'][side]
            hint = self.hints[side] + np.array([out * lean_out, 0, -lean_back]) / 100
            reach[side] = solve_arm(pose, side, target[3, :3], target[:3, :3], hint)
        return pose, item_local, reach


def channels(names, local):
    """Each bone's local position and Euler rotation (bones x 6)."""
    out = np.zeros((len(names), 6))
    for i, name in enumerate(names):
        r, p = local[name]
        out[i, :3], out[i, 3:] = p, clips.euler(r)
    return out


def nearest(values, like):
    """`values` with each angle (the last three of every row) the turn nearest to `like`'s."""
    out = values.copy()
    out[..., 3:] += 2 * math.pi * np.round((like[..., 3:] - out[..., 3:]) / (2 * math.pi))
    return out


class Authored:
    """The motion sampled on demand: the carrier's ('player') and the
    battery's ('item') bones' channels at any frame."""
    def __init__(self, rig, idle):
        self.rig, self.swing = rig, Swing(rig, idle)
        self.cache = {}
        self.reach = 0.0

    def at(self, kind, frame):
        frame = min(max(frame, 0.0), END)
        key = round(frame * 1e6)
        if key not in self.cache:
            pose, item, reach = self.swing.pose(frame / FPS)
            self.reach = max(self.reach, *reach.values())
            self.cache[key] = {'player': channels(self.rig.names, pose.local), 'item': channels(self.rig.item_names, item)}
        return self.cache[key][kind]

    def slope(self, kind, frame, like):
        """The channels' slopes (a frame) at `frame`, where their values are
        `like`: the motion's own, from a thousandth of a frame each side; none
        at the motion's two ends, where it rests."""
        if frame <= 0.0 or frame >= END:
            return np.zeros_like(like)
        eps = 1e-3
        return (nearest(self.at(kind, frame + eps), like) - nearest(self.at(kind, frame - eps), like)) / (2 * eps)


TURN_TOLERANCE = math.radians(0.02)   # a bone's turn between two samples may differ from the motion's by this
PLACE_TOLERANCE = 2e-5                # and its place by this (m)
FINEST = STEP / 16


def hermite(t0, t1, v0, v1, s0, s1, t):
    """The cubic through two samples with their slopes, at t: what the game
    evaluates between two knots of clips.Channel.bezier."""
    h = t1 - t0
    u = (t - t0) / h
    return ((2 * u**3 - 3 * u**2 + 1) * v0 + (u**3 - 2 * u**2 + u) * h * s0
            + (-2 * u**3 + 3 * u**2) * v1 + (u**3 - u**2) * h * s1)


def track(authored, kind, bone, first, last):
    """One bone's samples from frame `first` to `last`: (frames, values,
    slopes), each value its six channels. Every STEP, and wherever the cubic
    between two samples would leave the motion's own pose by more than the
    tolerances, again halfway between them. An arm bone near the turn where
    its Euler angles run wild (the right upper arm at the end of the push:
    its y angle reaches 85 degrees, the other two then swing 100 degrees in
    10 ms) takes samples every 1/480 s there; without them the right hand
    left the battery by 19 mm between two samples 1/60 s apart."""
    frames = [first + i * STEP for i in range(int(round((last - first) / STEP)) + 1)]
    while True:
        values, slopes = [], []
        for f in frames:
            v = authored.at(kind, f)[bone]
            v = nearest(v, values[-1]) if values else v
            values.append(v)
            slopes.append(authored.slope(kind, f, authored.at(kind, f))[bone])
        added = []
        for i in range(len(frames) - 1):
            a, b = frames[i], frames[i + 1]
            if b - a < 2 * FINEST - 1e-9:
                continue
            mid = (a + b) / 2
            got = hermite(a, b, values[i], values[i + 1], slopes[i], slopes[i + 1], mid)
            want = authored.at(kind, mid)[bone]
            turn = clips.rotation(got[3:]).T @ clips.rotation(want[3:])
            angle = math.acos(max(-1.0, min(1.0, (np.trace(turn) - 1) / 2)))
            if angle > TURN_TOLERANCE or np.linalg.norm(got[:3] - want[:3]) > PLACE_TOLERANCE:
                added.append(mid)
        if not added:
            return frames, np.array(values), np.array(slopes)
        frames = sorted(frames + added)


def author(rig, idle):
    """The motion, ready to be sampled into clips; fails if a hand is asked
    to hold the battery farther than its arm reaches."""
    authored = Authored(rig, idle)
    for i in range(int(round(END / STEP)) + 1):
        authored.at('player', i * STEP)
    assert authored.reach < 0.994, 'an arm is asked to reach %.3f of its length' % authored.reach
    return authored


def write(authored, kind, scales, first, last):
    """A clip of the motion from frame `first` to `last`, rebased to 0: each
    bone's nine channels the game's Bezier form through the bone's samples
    with their slopes (clips.Channel.bezier), so its curve leaves and meets
    each sample as the motion does, and rests where the motion rests (frame 0
    and END); the scale channels `scales`' (a clip of the same bones) rest
    values; a channel that does not move is a constant."""
    out = []
    for bone, scale_channels in enumerate(scales):
        frames, values, slopes = track(authored, kind, bone, first, last)
        times = [f - first for f in frames]
        bone_channels = []
        for k in range(6):
            vs = [float(v) for v in values[:, k]]
            if max(vs) - min(vs) < 1e-6:
                bone_channels.append(clips.Channel.constant(vs[0], times[-1]))
            else:
                bone_channels.append(clips.Channel.bezier(times, vs, [float(d) for d in slopes[:, k]]))
        for k in range(3):
            bone_channels.append(clips.Channel.constant(float(scale_channels[6 + k].evaluate(0)), times[-1]))
        out.append(bone_channels)
    return out


def clips_of(rig, idle, authored):
    """The four clips: (player swing, player recovery, battery swing, battery recovery)."""
    return (write(authored, 'player', idle, 0, IMPACT), write(authored, 'player', idle, IMPACT, END),
            write(authored, 'item', rig.item_idle, 0, IMPACT), write(authored, 'item', rig.item_idle, IMPACT, END))
