"""The look prediction as the client can run it: one projected input.

Inputs are change events (PR #1279): carrying the last one forward is what
the player did until a new one comes, and it is world state. A better guess
can only be a temporary projection, made again each frame before the render
move and taken out before every confirmed move. And the client does not
know whether a change is on its way: it knows the newest input node of the
place (its tick tau, mask, turn), and how long ago that was (now - tau).

So the projection here is one input at tau + 1 tick: the node's own mask
(nothing about the keys is guessed: every model that guessed a key change
did worse, `prediction_fit.md`), and the node's turn times a gain G read
from a table keyed by the turn's size and the time since the node. The
gain is fitted per cell to the least mean look error at `now` on training
(per axis); a rare cell falls back to the coarser key.

Scored as the client would show it: the truth from tau on has no turn up
to K0 (no input came) and the real turns after; the projected turn also
rotates the moves of the ticks tau + 1 .. K0, counted in the offset.
"""
import numpy as np

from . import kin
from .harness import HMAX, Prediction, start_blend
from .tables import FEATURES, MIN_CELL, _bucket

GAINS = np.round(np.arange(0, 4.0001, 0.05), 2)
# The gain in the client: a byte, G = byte / GAIN_SCALE.
GAIN_SCALE = 64
ELAPSED_EDGES = [0, 1, 2, 3, 4, 5, 6, 8, 10, 14, 1e9]
_elapsed, ELAPSED_N = _bucket(ELAPSED_EDGES)


def newest(s):
    """The newest known node's turns (0 when older than the history) and the
    ticks from it to now (the tick under way counted)."""
    age = s.node_age.astype(np.int64)
    known = age < s.hist_yaw.shape[1]
    col = s.hist_yaw.shape[1] - 1 - np.minimum(age, s.hist_yaw.shape[1] - 1)
    row = np.arange(len(s))
    yaw = np.where(known, s.hist_yaw[row, col], 0).astype(np.float64)
    pitch = np.where(known, s.hist_pitch[row, col], 0).astype(np.float64)
    return yaw, pitch, age + s.horizon


def _turn(s):
    yaw, pitch, _ = newest(s)
    return FEATURES['turn'][0](_Last(yaw, pitch))


class _Last:
    """Samples-like view whose last history column is the newest node."""

    def __init__(self, yaw, pitch):
        self.hist_yaw = yaw[:, None]
        self.hist_pitch = pitch[:, None]


KEY_FEATURES = {
    'turn': (_turn, FEATURES['turn'][1]),
    'elapsed': (lambda s: _elapsed(newest(s)[2]), ELAPSED_N),
    'stance': FEATURES['stance'],
    'moves': FEATURES['moves'],
}


def codes(s, features):
    out, code, size = [(np.zeros(len(s), np.int64), 1)], np.zeros(len(s), np.int64), 1
    for name in features:
        f, n = KEY_FEATURES[name]
        code = code * n + f(s)
        size *= n
        out.append((code.copy(), size))
    return out


class ProjectedLook:
    def __init__(self, features):
        self.features = list(features)
        self.name = 'projected-look[' + ','.join(features) + ']'

    def cells(self):
        n = 1
        for f in self.features:
            n *= KEY_FEATURES[f][1]
        return n

    def size_bytes(self):
        return self.cells() * 2

    @staticmethod
    def _truth(s):
        within = np.arange(HMAX)[None, :] < s.horizon[:, None]
        return (np.where(within, s.true_yaw, 0).sum(1), np.where(within, s.true_pitch, 0).sum(1))

    def fit(self, train):
        yaw, pitch, _ = newest(train)
        ty, tp = self._truth(train)
        errs = []
        for r, t in ((yaw, ty), (pitch, tp)):
            errs.append(np.stack([np.abs(t - r * g) for g in GAINS]))
        self.levels = []
        for code, size in codes(train, self.features):
            n = np.bincount(code, minlength=size)
            table = np.zeros((size, 2))
            for axis, e in enumerate(errs):
                sums = np.stack([np.bincount(code, row, size) for row in e])
                # Ties (cells of no turn) go to the smallest gain.
                sums = sums + 1e-9 * GAINS[:, None]
                table[:, axis] = GAINS[np.argmin(sums, 0)]
            self.levels.append((table, n >= MIN_CELL))
        return self

    def gains(self, s):
        out = None
        for (code, _), (table, filled) in zip(codes(s, self.features), self.levels):
            vals = table[code]
            out = vals if out is None else np.where(filled[code][:, None], vals, out)
        return out

    def predict(self, s):
        yaw, pitch, _ = newest(s)
        g = self.gains(s)
        turn_y, turn_p = yaw * g[:, 0], pitch * g[:, 1]
        keys = np.repeat(s.hist_keys[:, -1, None].astype(np.int64), HMAX, 1)
        z = np.zeros((len(s), HMAX), np.float32)
        py, pp = z.copy(), z.copy()
        py[:, 0], pp[:, 0] = turn_y, turn_p
        # The ticks tau + 1 .. K0 (known, without a turn) moved on rotated.
        before = np.maximum(s.node_age, 0).astype(np.float64)
        v = start_blend(s).now().astype(np.float64)           # body frame at K0
        world = kin.to_world(v, np.zeros(len(s)))
        rotated = kin.to_world(v, turn_y)
        offset = (rotated - world) * (before * kin.TICK_S)[:, None]
        # From K0 + 1 on the heading is already turned: the first predicted
        # tick's turn rotates the rest as the truth does not.
        return Prediction(keys, py, pp, offset)

    def params(self):
        last = self.levels[-1][0]
        return {'features': self.features, 'cells': self.cells(),
                'filled_cells': int(self.levels[-1][1].sum()), 'bytes': self.size_bytes(),
                'gains': last.tolist() if self.cells() <= 200 else 'large'}
