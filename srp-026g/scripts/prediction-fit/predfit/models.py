"""Prediction models: each gives the inputs of the predicted ticks.

Every model answers with inputs (keys and turns per tick), never with
positions: the game moves the body from them.
"""
import numpy as np

from . import kin
from .harness import HMAX, Prediction, score, start_blend, truth_run

# Hold-age buckets, ticks: bucket i holds ages AGE_EDGES[i] ..< AGE_EDGES[i + 1].
AGE_EDGES = np.array([0, 1, 2, 3, 4, 5, 6, 8, 10, 13, 16, 20, 25, 32, 40, 50, 64, 80, 100,
                      128, 160, 200, 1 << 30])
AGE_BUCKETS = len(AGE_EDGES) - 1
DIRECTIONS = ('none', 'forward', 'forward-side', 'side', 'back')
MODES = ('walk', 'sprint', 'aim', 'crouch')
CLASSES = len(DIRECTIONS) * len(MODES)
# The share of a bin that must be there for its own entry; else its class's.
MIN_BIN = 300


def age_bucket(age):
    return np.searchsorted(AGE_EDGES, age, side='right') - 1


def motion_class(keys):
    """Direction x mode of a mask (`DIRECTIONS`, `MODES`)."""
    keys = np.asarray(keys, np.int64)
    held = lambda bit: ((keys & bit) != 0).astype(np.int64)
    ahead = held(kin.FORWARD) - held(kin.BACK)
    side = held(kin.LEFT) - held(kin.RIGHT)
    direction = np.select([(ahead == 0) & (side == 0), (ahead > 0) & (side == 0), ahead > 0,
                           ahead == 0], [0, 1, 2, 3], 4)
    sprints = ((keys & kin.SPRINT) != 0) & (ahead > 0) & \
        ((keys & (kin.BACK | kin.FIRE | kin.CROUCH | kin.AIM)) == 0)
    mode = np.select([(keys & kin.CROUCH) != 0, (keys & kin.AIM) != 0, sprints], [3, 2, 1], 0)
    return direction * len(MODES) + mode


def released(keys):
    """The keys with the movement let go (the stance kept)."""
    return keys & ~np.int64(kin.MOVE_KEYS)


def last_keys(s, mask):
    return np.repeat((s.hist_keys[:, -1].astype(np.int64) & mask)[:, None], HMAX, 1)


def still(s):
    z = np.zeros((len(s), HMAX), np.float32)
    return z, z.copy()


class HoldLast:
    """The game's own: the last input's keys carried forward, no turn."""
    name = 'hold-last'
    mask = 0xFFFFFFFF

    def fit(self, train):
        return self

    def keys(self, s):
        return last_keys(s, self.mask)

    def turns(self, s):
        return still(s)

    def predict(self, s):
        return Prediction(self.keys(s), *self.turns(s))

    def params(self):
        return {}


class HoldHeld(HoldLast):
    """The movement and stance keys carried forward; one-off and held
    actions (fire, reload, use, quick use, grenade, spot, melee) dropped."""
    name = 'hold-movement'
    mask = kin.HELD_KEYS


def release_after(keys, j):
    """Keys held for the first `j` predicted ticks [S], then let go."""
    col = np.arange(HMAX)[None, :]
    return np.where(col < np.asarray(j)[:, None], keys, released(keys))


def candidates_scores(train, keys_of_j, js):
    """Position error of each sample for each candidate hold length."""
    blend = start_blend(train)
    truth = truth_run(train, blend.copy())
    z = np.zeros((len(train), HMAX), np.float32)
    out = np.empty((len(js), len(train)))
    for i, j in enumerate(js):
        p = Prediction(keys_of_j(np.full(len(train), j)), z, z)
        out[i] = score(train, p, blend, truth)[0]
    return out


class Fade(HoldHeld):
    """Movement held for a fixed time past the last known tick, then let go."""
    name = 'fade-to-stop'

    def __init__(self, ticks=None):
        self.ticks = ticks

    def fit(self, train):
        js = list(range(HMAX + 1))
        base = super().keys(train)
        errs = candidates_scores(train, lambda j: release_after(base, j), js)
        self.ticks = js[int(np.argmin(errs.mean(1)))]
        self.curve = errs.mean(1)
        return self

    def keys(self, s):
        return release_after(super().keys(s), np.full(len(s), self.ticks))

    def params(self):
        return {'hold_ticks': self.ticks}


class Hazard(HoldHeld):
    """Per motion class and hold age at the last known tick, how many more
    ticks the movement is held before it is let go: the length with the
    least mean position error on the training matches (the release hazard
    of each class and age, folded into the input it calls for)."""
    name = 'hazard-table'

    def fit(self, train):
        js = np.arange(HMAX + 1)
        base = super().keys(train)
        errs = candidates_scores(train, lambda j: release_after(base, j), js)
        cls = motion_class(train.hist_keys[:, -1])
        bucket = age_bucket(train.state_age)
        table = np.full((CLASSES, AGE_BUCKETS), HMAX, np.int32)
        counts = np.zeros((CLASSES, AGE_BUCKETS), np.int64)
        for c in range(CLASSES):
            in_c = cls == c
            if not in_c.any():
                continue
            fallback = int(js[np.argmin(errs[:, in_c].mean(1))])
            for b in range(AGE_BUCKETS):
                sel = in_c & (bucket == b)
                counts[c, b] = sel.sum()
                table[c, b] = int(js[np.argmin(errs[:, sel].mean(1))]) if counts[c, b] >= MIN_BIN \
                    else fallback
        # A class that does not move has nothing to let go.
        table[:len(MODES)] = HMAX
        self.table, self.counts = table, counts
        return self

    def hold_ticks(self, s):
        return self.table[motion_class(s.hist_keys[:, -1]), age_bucket(s.state_age)]

    def keys(self, s):
        return release_after(super().keys(s), self.hold_ticks(s))

    def params(self):
        return {'table': self.table.tolist()}


class Markov(HoldHeld):
    """Per held state and hold age (and with `order` 2 the state before),
    the state each predicted tick most likely brings, chosen as the one
    whose velocity is nearest on average to what follows in training."""
    name = 'markov'

    def __init__(self, order=1):
        self.order = order
        self.name = f'markov-{order}'

    def context(self, s):
        cur = s.hist_keys[:, -1].astype(np.int64) & kin.HELD_KEYS
        ctx = cur * AGE_BUCKETS + age_bucket(s.state_age)
        if self.order == 2:
            # The held state before the present one, within the history.
            h = s.hist_keys.astype(np.int64) & kin.HELD_KEYS
            differs = h != cur[:, None]
            last = np.where(differs.any(1), differs.shape[1] - 1 - np.argmax(differs[:, ::-1], 1), -1)
            prev = np.where(last >= 0, h[np.arange(len(h)), np.maximum(last, 0)], 0xFFFF)
            ctx = ctx * 0x10000 + prev
        return ctx

    def fit(self, train):
        ctx = self.context(train)
        fut = train.true_keys.astype(np.int64) & kin.HELD_KEYS
        self.table = {}
        order = np.argsort(ctx, kind='stable')
        ctx_sorted = ctx[order]
        bounds = np.flatnonzero(np.diff(ctx_sorted)) + 1
        for group in np.split(order, bounds):
            if len(group) < MIN_BIN:
                continue
            row = []
            for h in range(HMAX):
                states, n = np.unique(fut[group, h], return_counts=True)
                top = states[np.argsort(-n)][:8]
                v_all = kin.target_of(states)
                v_top = kin.target_of(top)
                cost = (np.linalg.norm(v_top[:, None, :] - v_all[None, :, :], axis=-1) * n).sum(1)
                row.append(int(top[np.argmin(cost)]))
            self.table[int(ctx[group[0]])] = row
        return self

    def keys(self, s):
        out = super().keys(s)
        unique, inverse = np.unique(self.context(s), return_inverse=True)
        rows = np.array([self.table.get(int(c), [-1] * HMAX) for c in unique],
                        np.int64).reshape(-1, HMAX)[inverse]
        return np.where(rows >= 0, rows, out)

    def params(self):
        return {'contexts': len(self.table)}


class DampedLook:
    """A key model, and the look turning on at the mean rate of the last
    `m` known ticks, damped by `lam` each tick."""

    def __init__(self, keys_model, m=None, lam_yaw=None, lam_pitch=None):
        self.inner = keys_model
        self.m, self.lam_yaw, self.lam_pitch = m, lam_yaw, lam_pitch
        self.name = keys_model.name + ' + damped-look'

    @staticmethod
    def turns_of(hist, m, lam):
        rate = hist[:, -m:].mean(1)
        powers = lam ** np.arange(1, HMAX + 1)
        return (rate[:, None] * powers[None, :]).astype(np.float32)

    def look_error(self, s, m, lam_y, lam_p):
        within = np.arange(HMAX)[None, :] < s.horizon[:, None]
        ey = np.where(within, s.true_yaw - self.turns_of(s.hist_yaw, m, lam_y), 0).sum(1)
        ep = np.where(within, s.true_pitch - self.turns_of(s.hist_pitch, m, lam_p), 0).sum(1)
        return np.abs(ey).mean(), np.abs(ep).mean()

    def fit(self, train):
        self.inner.fit(train)
        dampings = (0.0, 0.3, 0.5, 0.6, 0.7, 0.8, 0.85, 0.9, 0.95, 1.0)
        # One window for both axes (the yaw's best), each axis its own damping.
        _, self.m, self.lam_yaw = min((self.look_error(train, m, lam, lam)[0], m, lam)
                                      for m in (1, 2, 3, 4, 6) for lam in dampings)
        self.lam_pitch = min((self.look_error(train, self.m, 0, lam)[1], lam) for lam in dampings)[1]
        return self

    def predict(self, s):
        return Prediction(self.inner.keys(s), self.turns_of(s.hist_yaw, self.m, self.lam_yaw),
                          self.turns_of(s.hist_pitch, self.m, self.lam_pitch))

    def keys(self, s):
        return self.inner.keys(s)

    def params(self):
        return {**self.inner.params(), 'look_window': self.m, 'yaw_damping': self.lam_yaw,
                'pitch_damping': self.lam_pitch}

