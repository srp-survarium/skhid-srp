"""The learned ceiling: gradient boosting (LightGBM) offline, only to show
how much a model with every feature could gain. It answers with inputs too:
per predicted tick, the movement keys (the class whose velocity is nearest
on average under the predicted probabilities) and the two turns."""
import warnings

import numpy as np

from . import kin
from .harness import HMAX, Prediction, start_blend

# The fitted models keep column names lightgbm made up; the arrays given
# later have none.
warnings.filterwarnings('ignore', message='X does not have valid feature names')

ROWS = 2_000_000
LAST = 8


def features(s):
    blend = start_blend(s).now()
    k = s.hist_keys.astype(np.int64)
    cur = k[:, -1]
    bits = [((cur & b) != 0) for b in (kin.FORWARD, kin.BACK, kin.LEFT, kin.RIGHT, kin.AIM,
                                       kin.CROUCH, kin.SPRINT, kin.FIRE, kin.JUMP, kin.RELOAD)]
    held = k & kin.HELD_KEYS
    differs = held != held[:, -1:]
    last = np.where(differs.any(1), differs.shape[1] - 1 - np.argmax(differs[:, ::-1], 1), 0)
    prev = held[np.arange(len(s)), last]
    prev_bits = [((prev & b) != 0) for b in (kin.FORWARD, kin.BACK, kin.LEFT, kin.RIGHT)]
    cols = [s.hist_yaw[:, -LAST:], s.hist_pitch[:, -LAST:], s.hist_node[:, -LAST:],
            np.stack(bits + prev_bits, 1), s.state_age[:, None], s.node_age[:, None],
            blend, np.hypot(*blend.T)[:, None], s.combat[:, None]]
    return np.concatenate([np.asarray(c, np.float32).reshape(len(s), -1) for c in cols], 1)


def stacked(x, s, rng, rows):
    """Rows (sample, tick) with the tick as a feature."""
    n = len(s)
    idx = rng.integers(0, n, rows)
    h = rng.integers(0, HMAX, rows)
    return np.concatenate([x[idx], h[:, None].astype(np.float32)], 1), idx, h


class Boosted:
    name = 'learned-ceiling (boosting)'

    def fit(self, train):
        import lightgbm as lgb
        rng = np.random.default_rng(1)
        x = features(train)
        xs, idx, h = stacked(x, train, rng, min(ROWS, len(train) * HMAX))
        common = dict(n_estimators=300, learning_rate=0.05, num_leaves=63, min_child_samples=200,
                      subsample=0.5, subsample_freq=1, colsample_bytree=0.8, verbose=-1, n_jobs=16)
        self.yaw = lgb.LGBMRegressor(objective='l1', **common).fit(xs, train.true_yaw[idx, h])
        self.pitch = lgb.LGBMRegressor(objective='l1', **common).fit(xs, train.true_pitch[idx, h])
        moves = train.true_keys[idx, h].astype(np.int64) & kin.MOVE_KEYS
        self.classes = np.unique(moves)
        self.moves = lgb.LGBMClassifier(objective='multiclass', **common).fit(xs, moves)
        return self

    def predict(self, s):
        x = features(s)
        n = len(s)
        cur = s.hist_keys[:, -1].astype(np.int64) & kin.HELD_KEYS
        stance = cur & ~np.int64(kin.MOVE_KEYS)
        keys = np.empty((n, HMAX), np.int64)
        yaw = np.empty((n, HMAX), np.float32)
        pitch = np.empty((n, HMAX), np.float32)
        classes = self.moves.classes_
        for h in range(HMAX):
            xh = np.concatenate([x, np.full((n, 1), h, np.float32)], 1)
            yaw[:, h] = self.yaw.predict(xh)
            pitch[:, h] = self.pitch.predict(xh)
            p = self.moves.predict_proba(xh)                    # [n, C]
            v = kin.target_of(stance[:, None] | classes[None, :])  # [n, C, 2]
            d = np.linalg.norm(v[:, :, None, :] - v[:, None, :, :], axis=-1)  # [n, cand, true]
            cost = np.einsum('nct,nt->nc', d, p)
            keys[:, h] = stance | classes[np.argmin(cost, 1)]
        return Prediction(keys, yaw, pitch)

    def params(self):
        return {'rows': ROWS, 'trees': 300}
