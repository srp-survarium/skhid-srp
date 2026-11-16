"""The battery's strikes, authored on the stock rig like an animator would:
a keyed curve for each thing that moves (the battery's place and turn, each
shoulder, each elbow), each with its own timing, the arms solved onto the
battery's handles every sample. Three kinds (STRIKES), which a carrier's
strikes take in turn: the ram (both arms push the battery out), the shove
(the right arm shoves its right end forward about the left hand) and the
slam (both arms lift it and throw it down). Each is written as two clips:
the swing (the gather and the drive, its last frame the strike) and the
recovery (the follow-through, the settle and the way back to the carry
idle).

Body frame: x right, y up, z forward (the carrier faces +z; left-handed, as
the engine). Times in seconds and angles in degrees in the keys; frames (30 a
second) and radians inside."""
import math

import numpy as np

import clips
from rig import matrix

FPS = 30.0
STEP = 0.5          # frames between a clip's samples at most: 60 a second (swing.track adds where needed)
# The ram's and the shove's times (s): the strike, the swing clip's end (battery_hit.rs `times`:
# the swing 0.2 s), and the rest, the recovery's end, back on the carry idle (its recovery 0.35 s).
STRIKE = 0.2
REST = 0.55


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
RAM_BATTERY = {
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
RAM_SHOULDER = {
    'Left': {'z': Curve((0, 0), (0.086, -1.4, 0), (0.240, 2.9, 0), (0.40, 1.9), (REST, 0)),
             'y': Curve((0, 0), (0.090, 1.5, 0), (0.262, -0.5, 0), (REST, 0))},
    'Right': {'z': Curve((0, 0), (0.082, -1.8, 0), (0.248, 1.5, 0), (0.40, 1.0), (REST, 0)),
              'y': Curve((0, 0), (0.086, 1.8, 0), (0.258, -0.6, 0), (REST, 0))},
}
REACH = 1.012        # of the arm's length: what a shoulder gives way to
REACH_SOFT = 0.02    # how gradually: the elbows open to 155 degrees
# Each elbow's lean out of its carry plane (cm at the elbow: out to its side, back, and for the
# shove and the slam up).
RAM_ELBOW = {
    'Left': {'out': Curve((0, 0), (0.092, 5.0, 0), (0.215, -1.0, -30), (0.29, 0.6), (REST, 0)),
             'back': Curve((0, 0), (0.096, 3.0, 0), (0.21, -1.0), (0.30, 0.0), (REST, 0))},
    'Right': {'out': Curve((0, 0), (0.088, 6.0, 0), (0.212, -1.2, -34), (0.29, 0.8), (REST, 0)),
              'back': Curve((0, 0), (0.092, 3.5, 0), (0.21, -1.2), (0.30, 0.0), (REST, 0))},
}


class Strike:
    """One kind of strike: its keyed curves. `about` names the point the
    battery's turn is keyed about, which its place keys move (Swing.pivots).
    With `turning`, each hand turns on its handle (an upright bar) as its
    forearm comes round it, so a wide turn of the battery does not break the
    wrists; without, the hands are fixed on the handles. `strike` and `rest`
    are its two clips' ends (s): the swing's, where it strikes, and the
    recovery's, back on the carry idle."""
    def __init__(self, about, battery, shoulder, elbow, turning=False, strike=STRIKE, rest=REST):
        self.about, self.battery, self.shoulder, self.elbow, self.turning = about, battery, shoulder, elbow, turning
        self.impact, self.end = strike * FPS, rest * FPS      # frames

    def at(self, t):
        """Every keyed value at time t (s)."""
        return {'battery': tuple(self.battery[k](t) for k in ('x', 'y', 'z', 'pitch', 'yaw', 'roll')),
                'shoulder': {s: (self.shoulder[s]['z'](t), self.shoulder[s]['y'](t)) for s in self.shoulder},
                'elbow': {s: (self.elbow[s]['out'](t), self.elbow[s]['back'](t), self.elbow[s]['up'](t) if 'up' in self.elbow[s] else 0.0)
                          for s in self.elbow}}


RAM = Strike('centre', RAM_BATTERY, RAM_SHOULDER, RAM_ELBOW)

# The shove (the owner, 2026-11-15: "the second animation where left hand doesn't move far but
# right one does", "it is like pushing the right side of the battery and hitting with that"): the
# left hand stays the pivot, the right arm shoves the battery's right end forward and up about it,
# and that end lands. The ram's beats and times. The values are the left hand's hold's move from
# its carry place (cm) and the battery's turn about it (degrees: yaw swings the right end forward,
# roll lifts it, pitch tips the top back). The lift is what brings the right end and the right
# hand into the first-person view (a level turn stays under it, as the carry does) and within the
# right arm's reach (its shoulder stands back in the carry).
SHOVE = Strike('left hand', {
    'x': Curve((0, 0), (0.100, 1.5, 0), (STRIKE, -5.2, -120), (0.250, -8.0, 0), (0.310, -7.5, 6), (REST, 0)),
    'y': Curve((0, 0), (0.100, -0.5, 0), (STRIKE, 2.0, 45), (0.250, 3.0, 0), (0.310, 2.7, -4), (REST, 0)),
    'z': Curve((0, 0), (0.100, 1.0, 0), (STRIKE, 3.0, 10), (0.250, 2.0, 0), (REST, 0)),
    'pitch': Curve((0, 0), (0.108, 1.5, 0), (STRIKE, -1.5, -60), (0.258, -5.0, 0), (0.318, -4.6, 5), (REST, 0)),
    'yaw': Curve((0, 0), (0.104, -6.0, 0), (STRIKE, 23.0, 480, 0), (0.242, 32.0, 0, -1100), (0.296, 30.6, -34), (REST, 0)),
    'roll': Curve((0, 0), (0.104, -3.0, 0), (STRIKE, 17.5, 360, 0), (0.250, 24.0, 0, -800), (0.305, 22.8, -26), (0.400, 9.0, -150), (REST, 0)),
}, {
    'Left': {'z': Curve((0, 0), (0.090, 1.0, 0), (0.250, -1.2, 0), (REST, 0)),
             'y': Curve((0, 0), (0.250, 0.6, 0), (REST, 0))},
    'Right': {'z': Curve((0, 0), (0.085, -2.0, 0), (0.245, 2.2, 0), (0.40, 1.3), (REST, 0)),
              'y': Curve((0, 0), (0.090, -1.2, 0), (0.255, 2.5, 0), (REST, 0))},
}, {
    'Left': {'out': Curve((0, 0), (0.240, 3.0, 0), (REST, 0)),
             'back': Curve((0, 0), (0.240, 2.0, 0), (REST, 0))},
    'Right': {'out': Curve((0, 0), (0.092, 6.0, 0), (0.245, 8.0, 0), (REST, 0)),
              'back': Curve((0, 0), (0.094, 4.0, 0), (0.210, 0.0), (REST, 0)),
              'up': Curve((0, 0), (0.245, 12.0, 0), (REST, 0))},
}, turning=True)

# The slam (the owner, 2026-11-15: "maybe the third one when both hands are raised up and then
# shown down (we can at least try)"): both arms lift the battery in front of the chest until its
# top is at the eyes, the elbows out and up, then throw it down and forward, its front edge
# first. As high as the arms alone take it: the spine and the head do not move (mask 1), so it
# cannot go over the head, and any higher it fills the first-person view. The lift takes longer
# than the other strikes' gather, so the slam has its own times (battery_hit.rs `times`): the
# strike 0.4 s in (on the way down, at its fastest), the rest at 0.7 s.
SLAM_STRIKE = 0.4
SLAM_REST = 0.7
SLAM = Strike('centre', {
    'x': Curve((0, 0), (0.275, -6.0, 0), (0.450, -8.0, 0), (SLAM_REST, 0)),
    'y': Curve((0, 0), (0.285, 28.0, 0, -700), (SLAM_STRIKE, 13.5, -400, 0), (0.452, 4.5, 0, 1400), (0.505, 5.6, 8), (SLAM_REST, 0)),
    'z': Curve((0, 0), (0.120, 14.0, 60), (0.285, 18.0, 0), (SLAM_STRIKE, 18.4, 20), (0.450, 19.4, 0), (SLAM_REST, 0)),
    'pitch': Curve((0, 0), (0.290, -4.0, 0), (SLAM_STRIKE, 9.0, 400), (0.460, 22.0, 0), (0.515, 20.5, -14), (SLAM_REST, 0)),
    'yaw': Curve((0, 0), (0.280, 6.0, 0), (0.450, 8.0, 0), (SLAM_REST, 0)),
    'roll': Curve((0, 0), (0.280, 3.0, 0), (0.450, 4.0, 0), (SLAM_REST, 0)),
}, {
    'Left': {'z': Curve((0, 0), (0.240, -1.5, 0), (0.435, 2.6, 0), (0.55, 1.4), (SLAM_REST, 0)),
             'y': Curve((0, 0), (0.250, 3.0, 0), (0.440, -1.0, 0), (SLAM_REST, 0))},
    'Right': {'z': Curve((0, 0), (0.235, -1.5, 0), (0.440, 2.0, 0), (0.55, 1.0), (SLAM_REST, 0)),
              'y': Curve((0, 0), (0.245, 3.4, 0), (0.435, -1.2, 0), (SLAM_REST, 0))},
}, {
    'Left': {'out': Curve((0, 0), (0.265, 8.0, 0), (0.445, -1.0, 0), (SLAM_REST, 0)),
             'back': Curve((0, 0), (0.265, 2.0, 0), (0.445, -1.0, 0), (SLAM_REST, 0)),
             'up': Curve((0, 0), (0.270, 14.0, 0), (0.445, 0.0, 0), (SLAM_REST, 0))},
    'Right': {'out': Curve((0, 0), (0.260, 9.0, 0), (0.445, -1.2, 0), (SLAM_REST, 0)),
              'back': Curve((0, 0), (0.260, 2.0, 0), (0.445, -1.0, 0), (SLAM_REST, 0)),
              'up': Curve((0, 0), (0.265, 16.0, 0), (0.445, 0.0, 0), (SLAM_REST, 0))},
}, turning=True, strike=SLAM_STRIKE, rest=SLAM_REST)

# The strikes and their clips' names. A carrier's strikes take them in this order, the ram
# first, and round again (the battery's states, docs/notes/battery-hit.md).
STRIKES = {'battery_hit': RAM, 'battery_hit_shove': SHOVE, 'battery_hit_slam': SLAM}


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
        self.pivots = {'centre': self.centre0, 'left hand': self.world0['LeftHand'][1]}
        # the handles: upright bars at the battery's two ends, each inside its hand's curled fingers
        vertices = rig.battery.pose(self.item_world0)
        self.bar = np.linalg.svd(vertices - self.centre0, full_matrices=False)[2][2]
        self.bar = self.bar * (1 if self.bar[1] > 0 else -1)
        self.handles = {s: np.mean([self.world0[s + f][1] for f in ('Index2', 'Middle2', 'Pinky2')], axis=0)
                        for s in ('Left', 'Right')}
        self.arm = {s: np.linalg.norm(self.world0[s + 'ForeArm'][1] - self.world0[s + 'Arm'][1])
                    + np.linalg.norm(self.world0[s + 'Hand'][1] - self.world0[s + 'ForeArm'][1]) for s in ('Left', 'Right')}

    def delta(self, v, pivot):
        """The battery's move (model space, row vectors) for its keyed values
        v (cm, degrees): turned about `pivot`, which is moved by the place."""
        cx, cy, cz, pitch, yaw, roll = v
        r = (rot_axis([1, 0, 0], math.radians(-pitch)) @ rot_axis([0, 1, 0], math.radians(-yaw))
             @ rot_axis([0, 0, 1], math.radians(roll)))
        about = np.eye(4); about[3, :3] = -pivot
        back = np.eye(4); back[3, :3] = pivot + np.array([cx, cy, cz]) / 100
        turn = np.eye(4); turn[:3, :3] = r
        return about @ turn @ back

    def arm_to(self, pose, side, m, target):
        """One arm of `pose` brought to the hand's `target` frame, its
        shoulder and elbow by the keyed values m; returns its reach."""
        # the shoulder joint moved by its clavicle's turn: its keys, then on toward the
        # hand by what the arm lacks of the reach (a smooth limit, so nothing snaps)
        pivot, joint = self.world0[side + 'Clavicle'][1], self.world0[side + 'Arm'][1]
        forward, up = m['shoulder'][side]
        r0 = joint - pivot
        r1 = r0 + np.array([0, up, forward]) / 100
        r1 *= np.linalg.norm(r0) / np.linalg.norm(r1)
        wrist = target[3, :3]
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
        lean_out, lean_back, lean_up = m['elbow'][side]
        hint = self.hints[side] + np.array([(1.0 if side == 'Right' else -1.0) * lean_out, lean_up, -lean_back]) / 100
        return solve_arm(pose, side, target[3, :3], target[:3, :3], hint)

    def pose(self, strike, t):
        """(the player's pose, the battery's own bones' locals, each arm's
        reach) of `strike` at time t (s)."""
        m = strike.at(t)
        d = self.delta(m['battery'], self.pivots[strike.about])
        rig = self.rig
        # the battery: its offset bone carries it (key_object, its parent, stays)
        offset = matrix(*self.item_world0['offset']) @ d
        key = matrix(*self.item_world0['key_object'])
        local = offset @ np.linalg.inv(key)
        item_local = dict(self.item_local0)
        item_local['offset'] = (local[:3, :3], local[3, :3])
        pose = Pose(rig, self.base_local)
        reach = {}
        for side in ('Left', 'Right'):
            # the hand keeps its hold on the battery: its idle frame moved with it
            target = matrix(*self.world0[side + 'Hand']) @ d
            if strike.turning:
                # ... and turned about its handle until its wrist is as straight about the bar
                # as in the carry: by the angle its forearm has come round the bar since
                bar = self.bar @ d[:3, :3]
                handle = (np.r_[self.handles[side], 1] @ d)[:3]
                across = lambda v: (v - bar * np.dot(v, bar)) / np.linalg.norm(v - bar * np.dot(v, bar))
                carried = across(handle - (np.r_[self.world0[side + 'ForeArm'][1], 1] @ d)[:3])
                to, back, turn = np.eye(4), np.eye(4), np.eye(4)
                to[3, :3], back[3, :3] = -handle, handle
                rigid = target
                for _ in range(2):
                    trial = Pose(rig, self.base_local)
                    self.arm_to(trial, side, m, target)
                    now = across(handle - trial.world()[side + 'ForeArm'][1])
                    turn[:3, :3] = rot_axis(bar, math.atan2(float(np.dot(np.cross(carried, now), bar)), float(np.dot(carried, now))))
                    target = rigid @ to @ turn @ back
            reach[side] = self.arm_to(pose, side, m, target)
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
    """A strike's motion sampled on demand: the carrier's ('player') and the
    battery's ('item') bones' channels at any frame."""
    def __init__(self, rig, idle, strike):
        self.rig, self.swing, self.strike = rig, Swing(rig, idle), strike
        self.cache = {}
        self.reach = 0.0

    def at(self, kind, frame):
        frame = min(max(frame, 0.0), self.strike.end)
        key = round(frame * 1e6)
        if key not in self.cache:
            pose, item, reach = self.swing.pose(self.strike, frame / FPS)
            self.reach = max(self.reach, *reach.values())
            self.cache[key] = {'player': channels(self.rig.names, pose.local), 'item': channels(self.rig.item_names, item)}
        return self.cache[key][kind]

    def slope(self, kind, frame, like):
        """The channels' slopes (a frame) at `frame`, where their values are
        `like`: the motion's own, from a thousandth of a frame each side; none
        at the motion's two ends, where it rests."""
        if frame <= 0.0 or frame >= self.strike.end:
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


def author(rig, idle, strike):
    """A strike's motion, ready to be sampled into clips; fails if a hand is
    asked to hold the battery farther than its arm reaches."""
    authored = Authored(rig, idle, strike)
    for i in range(int(round(strike.end / STEP)) + 1):
        authored.at('player', i * STEP)
    assert authored.reach < 0.994, 'an arm is asked to reach %.3f of its length' % authored.reach
    return authored


def write(authored, kind, scales, first, last):
    """A clip of the motion from frame `first` to `last`, rebased to 0: each
    bone's nine channels the game's Bezier form through the bone's samples
    with their slopes (clips.Channel.bezier), so its curve leaves and meets
    each sample as the motion does, and rests where the motion rests (its
    first and last frame); the scale channels `scales`' (a clip of the same bones) rest
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
    impact, end = authored.strike.impact, authored.strike.end
    return (write(authored, 'player', idle, 0, impact), write(authored, 'player', idle, impact, end),
            write(authored, 'item', rig.item_idle, 0, impact), write(authored, 'item', rig.item_idle, impact, end))
