"""The movement proxy: how a body moves for the keys it holds, vectorised.

The same model as the match server's bots (`servers/match-server/src/v026g/
bots/kin.rs`, PROVEN in the 0.26g client and checked on recordings): each
tick the body goes at the desired speed of its keys, reached by a linear
blend over 6 ticks held in the body's frame; forward at yaw `a` is
(-sin a, cos a) in the map's x/z; a positive yaw turns left; a tick's turn
is made before its move. Walls, slopes, jumps and being hit are not known.
"""
import numpy as np

TICK_S = 0.05
TICK_MS = 50
BLEND_TICKS = 6

FORWARD, BACK, LEFT, RIGHT = 0x1, 0x2, 0x4, 0x8
JUMP, FIRE, RELOAD, AIM, CROUCH, SPRINT = 0x10, 0x20, 0x40, 0x80, 0x100, 0x200
MOVE_KEYS = FORWARD | BACK | LEFT | RIGHT
# The keys that set a body's velocity (FIRE blocks a sprint).
MOTION_KEYS = MOVE_KEYS | AIM | CROUCH | SPRINT | FIRE
# Keys held as states, the movement and the stance: everything else of the
# mask is a one-off action or a held action (fire, reload, use, quick use,
# grenade, spot, melee, weapon switch).
HELD_KEYS = MOVE_KEYS | AIM | CROUCH | SPRINT

# Desired speeds (m/s): forward, side, back.
WALK = (3.5, 2.5, 3.0)
AIMED = (3.0, 2.0, 2.5)
CROUCHED = (2.5, 2.0, 2.0)
CROUCHED_AIMED = (2.0, 1.5, 1.5)
SPRINT_SPEED = 5.0


def target_local(keys):
    """The velocity the keys ask for, body frame (ahead, left), m/s."""
    keys = np.asarray(keys, dtype=np.int64)
    held = lambda bit: ((keys & bit) != 0).astype(np.float32)
    ahead = held(FORWARD) - held(BACK)
    left = held(LEFT) - held(RIGHT)
    crouch = (keys & CROUCH) != 0
    aim = (keys & AIM) != 0
    speeds = np.empty(keys.shape + (3,), dtype=np.float32)
    for (c, a), row in {(False, False): WALK, (False, True): AIMED,
                        (True, False): CROUCHED, (True, True): CROUCHED_AIMED}.items():
        speeds[(crouch == c) & (aim == a)] = row
    sprints = ((keys & SPRINT) != 0) & ((keys & FORWARD) != 0) & \
        ((keys & (BACK | FIRE | CROUCH | AIM)) == 0)
    speed = np.where(ahead > 0, speeds[..., 0], np.where(ahead < 0, speeds[..., 2], speeds[..., 1]))
    speed = np.where(sprints, SPRINT_SPEED, speed)
    n = np.hypot(ahead, left)
    scale = np.where(n > 0, speed / np.maximum(n, 1e-9), 0.0)
    return np.stack([ahead * scale, left * scale], axis=-1).astype(np.float32)


# The table of target velocities for every combination of the motion keys.
_ALL = np.arange(MOTION_KEYS + 1)
TARGET_TABLE = target_local(_ALL)


def target_of(keys):
    return TARGET_TABLE[np.asarray(keys, dtype=np.int64) & MOTION_KEYS]


def to_world(local, yaw):
    """World (x, z) velocity of a body-frame one at `yaw`."""
    s, c = np.sin(yaw), np.cos(yaw)
    fx, fz = -s, c
    lx, lz = -fz, fx
    return np.stack([local[..., 0] * fx + local[..., 1] * lx,
                     local[..., 0] * fz + local[..., 1] * lz], axis=-1)


class Blend:
    """Blend states of many bodies at once (`kin.rs` `Blend`)."""

    def __init__(self, at):
        at = np.asarray(at, dtype=np.float32)
        self.frm = at.copy()
        self.to = at.copy()
        self.step = np.full(at.shape[:-1], BLEND_TICKS, dtype=np.int32)

    def now(self):
        t = (self.step / BLEND_TICKS)[..., None]
        return self.frm + (self.to - self.frm) * t

    def copy(self):
        b = Blend.__new__(Blend)
        b.frm, b.to, b.step = self.frm.copy(), self.to.copy(), self.step.copy()
        return b

    def tick(self, target):
        changed = np.any(target != self.to, axis=-1)
        if changed.any():
            now = self.now()
            self.frm = np.where(changed[..., None], now, self.frm)
            self.to = np.where(changed[..., None], target, self.to)
            self.step = np.where(changed, 0, self.step)
        self.step = np.minimum(self.step + 1, BLEND_TICKS)
        return self.now()


def run(blend, keys, turns, yaw0, weights):
    """Moves bodies on over ticks.

    blend: Blend of S bodies (modified); keys: [S, H] masks; turns: [S, H]
    yaw changes made at each tick; yaw0: [S]; weights: [S, H] the seconds
    of each tick's move that count (TICK_S, a part of it for the tick under
    way, 0 past a body's horizon). Returns the displacement [S, 2] and the
    yaw [S] after the turns.
    """
    S, H = keys.shape
    yaw = np.asarray(yaw0, dtype=np.float64).copy()
    disp = np.zeros((S, 2))
    for h in range(H):
        yaw = yaw + turns[:, h]
        v = to_world(blend.tick(target_of(keys[:, h])), yaw)
        disp += v * weights[:, h, None]
    return disp, yaw
