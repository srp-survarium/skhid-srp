"""Lookup tables fitted from the data (the owner's idea, 2026-11-05).

A context key, made of a few bucketed features of the last known tick, maps
- `MoveTable`: for each predicted tick, the held keys to predict (the state
  whose velocity is on average nearest to what followed in training: the
  most likely next input, weighted by how far off each other one would put
  the body); a byte per cell and tick;
- `LookTable`: the damping of the last known turn, per axis; 2 bytes a cell.
A cell seen fewer than `MIN_CELL` times falls back to the key without its
last feature, and so on down to the empty key. The size of a table is that
of its dense array: every cell of the key.
"""
import numpy as np

from . import kin
from .harness import HMAX, Prediction, start_blend

MIN_CELL = 200


def _bucket(edges):
    edges = np.asarray(edges, np.float64)
    return (lambda v: np.searchsorted(edges, v, side='right') - 1), len(edges) - 1


_hold, HOLD_N = _bucket([0, 1, 2, 4, 8, 16, 32, 64, 1e9])
_since, SINCE_N = _bucket([0, 1, 2, 4, 8, 16, 32, 1e9])
_speed, SPEED_N = _bucket([0, 0.5, 1.5, 2.5, 3.25, 4.0, 1e9])
TURN_EDGES = [0, 1e-4, 0.003, 0.008, 0.02, 0.05, 0.12, 1e9]
_turn, TURN_N = _bucket(TURN_EDGES)


def _moves(s):
    return (s.hist_keys[:, -1].astype(np.int64) & kin.MOVE_KEYS)


def _stance(s):
    k = s.hist_keys[:, -1].astype(np.int64)
    return ((k & kin.CROUCH) != 0) * 4 + ((k & kin.AIM) != 0) * 2 + ((k & kin.SPRINT) != 0)


def _speed_of(s):
    return _speed(np.hypot(*start_blend(s).now().T))


def _turn_of(s):
    return _turn(np.abs(s.hist_yaw[:, -1]) + np.abs(s.hist_pitch[:, -1]))


# name -> (function of samples, number of values)
FEATURES = {
    'moves': (_moves, 16),
    'stance': (_stance, 8),
    'hold_age': (lambda s: _hold(s.state_age), HOLD_N),
    'since_input': (lambda s: _since(s.node_age), SINCE_N),
    'speed': (_speed_of, SPEED_N),
    'turn': (_turn_of, TURN_N),
}


def context_codes(s, features):
    """For each prefix of `features`, the dense cell index of every sample."""
    codes, code, size = [], np.zeros(len(s), np.int64), 1
    codes.append((code.copy(), size))
    for name in features:
        f, n = FEATURES[name]
        code = code * n + f(s)
        size *= n
        codes.append((code.copy(), size))
    return codes


class _Table:
    entry_bytes = 1

    def __init__(self, features):
        self.features = list(features)

    def cells(self):
        size = 1
        for name in self.features:
            size *= FEATURES[name][1]
        return size

    def size_bytes(self):
        return self.cells() * HMAX * self.entry_bytes

    def lookup(self, s):
        """Per sample and tick, the entry of the finest key filled."""
        codes = context_codes(s, self.features)
        out = None
        for (code, _), (table, filled) in zip(codes, self.levels):
            vals = table[code]
            ok = filled[code]
            out = vals if out is None else np.where(ok, vals, out)
        return out

    def params(self):
        filled = [int(f.any(1).sum()) if f.ndim == 2 else int(f.sum()) for _, f in self.levels]
        return {'features': self.features, 'cells': self.cells(), 'filled_cells': filled[-1],
                'bytes': self.size_bytes()}


class MoveTable(_Table):
    """Context -> held keys of each predicted tick (an index into `states`)."""

    def __init__(self, features):
        super().__init__(features)
        self.name = 'move-table[' + ','.join(features) + ']'

    def fit(self, train):
        fut = train.true_keys.astype(np.int64) & kin.HELD_KEYS
        cur = train.hist_keys[:, -1].astype(np.int64) & kin.HELD_KEYS
        states, inverse = np.unique(np.concatenate([fut.ravel(), cur]), return_inverse=True)
        self.states = states
        nst = len(states)
        v = kin.target_of(states)
        dist = np.linalg.norm(v[:, None, :] - v[None, :, :], axis=-1)  # [cand, true]
        fut_i = inverse[:fut.size].reshape(fut.shape)
        self.levels = []
        for code, size in context_codes(train, self.features):
            table = np.zeros((size, HMAX), np.int32)
            filled = np.zeros((size, HMAX), bool)
            for h in range(HMAX):
                counts = np.bincount(code * nst + fut_i[:, h], minlength=size * nst)
                counts = counts.reshape(size, nst).astype(np.float64)
                total = counts.sum(1)
                cost = counts @ dist.T          # [cell, candidate]
                table[:, h] = np.argmin(cost, 1)
                filled[:, h] = total >= MIN_CELL
            self.levels.append((table, filled))
        # The coarsest level never names a state of its own: it is "hold".
        self.levels[0] = (np.full((1, HMAX), -1, np.int32), np.ones((1, HMAX), bool))
        return self

    def keys(self, s):
        idx = self.lookup(s)
        cur = s.hist_keys[:, -1].astype(np.int64) & kin.HELD_KEYS
        pred = np.where(idx >= 0, self.states[np.maximum(idx, 0)], cur[:, None])
        return pred

    def predict(self, s):
        z = np.zeros((len(s), HMAX), np.float32)
        return Prediction(self.keys(s), z, z.copy())


DAMPINGS = np.round(np.arange(0, 1.0001, 0.05), 2)


class LookTable(_Table):
    """Context -> the damping of each axis: the predicted turn of the s-th
    tick is the last known turn x damping^s. Per cell the damping with the
    least mean look error on training, from `DAMPINGS` (a byte per axis in
    the client: 2 bytes a cell, whatever the horizon)."""

    def __init__(self, features, keys_model=None):
        super().__init__(features)
        self.keys_model = keys_model
        self.name = 'look-table[' + ','.join(features) + ']'

    def size_bytes(self):
        return self.cells() * 2

    @staticmethod
    def _errors(hist, true, horizon):
        """[dampings, samples]: the look error of one axis at t."""
        within = np.arange(HMAX)[None, :] < horizon[:, None]
        truth = np.where(within, true, 0).sum(1)
        r = hist[:, -1].astype(np.float64)
        out = np.empty((len(DAMPINGS), len(r)))
        for i, lam in enumerate(DAMPINGS):
            powers = np.where(within, lam ** np.arange(1, HMAX + 1)[None, :], 0).sum(1)
            out[i] = np.abs(truth - r * powers)
        return out

    def fit(self, train):
        if self.keys_model is not None:
            self.keys_model.fit(train)
        errs = [self._errors(train.hist_yaw, train.true_yaw, train.horizon),
                self._errors(train.hist_pitch, train.true_pitch, train.horizon)]
        self.levels = []
        for code, size in context_codes(train, self.features):
            n = np.bincount(code, minlength=size)
            table = np.zeros((size, 2))
            for axis, e in enumerate(errs):
                sums = np.stack([np.bincount(code, row, size) for row in e])
                table[:, axis] = DAMPINGS[np.argmin(sums, 0)]
            self.levels.append((table, n >= MIN_CELL))
        return self

    def lookup(self, s):
        out = None
        for (code, _), (table, filled) in zip(context_codes(s, self.features), self.levels):
            vals = table[code]
            out = vals if out is None else np.where(filled[code][:, None], vals, out)
        return out

    def turns(self, s):
        lam = self.lookup(s)
        powers = np.arange(1, HMAX + 1)[None, :]
        return ((s.hist_yaw[:, -1, None] * lam[:, 0, None] ** powers).astype(np.float32),
                (s.hist_pitch[:, -1, None] * lam[:, 1, None] ** powers).astype(np.float32))

    def keys(self, s):
        if self.keys_model is None:
            return np.repeat(s.hist_keys[:, -1, None].astype(np.int64), HMAX, 1)
        return self.keys_model.keys(s)

    def predict(self, s):
        return Prediction(self.keys(s), *self.turns(s))

    def params(self):
        return {**super().params(), 'dampings': self.levels[-1][0].tolist()
                if self.cells() <= 64 else 'large'}
