"""Samples and scores of a prediction model.

A sample is a moment t of a living player and a delay d: the model sees the
player's inputs up to t - d (the ticks up to K0 = floor((t - d) / 50)) and
gives the inputs of the ticks K0 + 1 .. Kt = floor(t / 50). Its score:
- the position error at t: both input runs moved on from the same body
  state by the movement proxy (`kin`), the tick under way counted by its
  part gone;
- the look error at t: the yaw and pitch the turns add up to;
- the action error: predicted ticks whose one-off action bits differ.
"""
import hashlib
from dataclasses import dataclass, field

import numpy as np

from . import kin

HISTORY = 24        # known ticks a model sees, K0 included
WARMUP = 12         # ticks of the proxy's blend before K0
MAX_DELAY_MS = 600
HMAX = MAX_DELAY_MS // kin.TICK_MS + 1
FRAME_MS = 1000 / 60  # a receiver takes an arrived input at its next frame
STATE_AGE_CAP = 400


def is_test(name, share=0.3):
    """The match's side of the split, fixed by its name."""
    h = int.from_bytes(hashlib.sha256(name.encode()).digest()[:4], 'little')
    return h / 2**32 < share


def held_state(keys):
    return keys & kin.HELD_KEYS


def ages(values, cap=STATE_AGE_CAP):
    """Ticks since `values` last changed, at each tick (0 on a change)."""
    n = len(values)
    change = np.ones(n, bool)
    change[1:] = values[1:] != values[:-1]
    start = np.where(change, np.arange(n), 0)
    start = np.maximum.accumulate(start)
    return np.minimum(np.arange(n) - start, cap).astype(np.int32)


@dataclass
class Samples:
    hist_keys: np.ndarray   # [S, HISTORY] u32, the last column K0
    hist_yaw: np.ndarray    # [S, HISTORY]
    hist_pitch: np.ndarray  # [S, HISTORY]
    hist_node: np.ndarray   # [S, HISTORY] bool
    warm_keys: np.ndarray   # [S, WARMUP + 1] the keys of K0 - WARMUP .. K0
    state_age: np.ndarray   # [S] ticks the held state had lasted at K0
    node_age: np.ndarray    # [S] ticks since the last input at K0
    delay: np.ndarray       # [S] ms
    horizon: np.ndarray     # [S] predicted ticks H >= 0
    weights: np.ndarray     # [S, HMAX] seconds of each predicted tick's move
    true_keys: np.ndarray   # [S, HMAX]
    true_yaw: np.ndarray    # [S, HMAX]
    true_pitch: np.ndarray  # [S, HMAX]
    combat: np.ndarray      # [S] bool
    pos: np.ndarray         # [S, 2] map x, z at K0 (NaN: unknown)
    heading: np.ndarray     # [S] yaw at K0 (NaN: unknown)
    match: np.ndarray       # [S] index into `names`
    names: list = field(default_factory=list)

    def __len__(self):
        return len(self.delay)

    def take(self, mask):
        out = {}
        for k, v in self.__dict__.items():
            out[k] = v if k == 'names' else v[mask]
        return Samples(**out)

    @staticmethod
    def concat(parts):
        parts = [p for p in parts if len(p)]
        names, offset, matches = [], 0, []
        for p in parts:
            matches.append(p.match + offset)
            names += p.names
            offset += len(p.names)
        out = {k: np.concatenate([getattr(p, k) for p in parts])
               for k in parts[0].__dict__ if k not in ('names', 'match')}
        return Samples(match=np.concatenate(matches), names=names, **out)


def player_samples(player, every_ticks, delays, rng, receivers_rtt):
    """Samples of one player. `delays`: a fixed ms, or None for the realistic
    one (the server's lateness of this player's nearest input, plus half a
    random other player's round trip and a frame)."""
    n = len(player.keys)
    dead = np.concatenate([[0], np.cumsum(~player.alive)])
    kt = np.arange(WARMUP + HISTORY + HMAX, n, every_ticks)
    kt = kt + rng.integers(0, every_ticks, len(kt))
    kt = kt[kt < n]
    if len(kt) == 0:
        return None
    frac = rng.random(len(kt))
    if delays is None:
        node_ticks = np.flatnonzero(player.node)
        nearest = np.clip(np.searchsorted(node_ticks, kt), 0, len(node_ticks) - 1)
        late = player.late[np.minimum(nearest, len(player.late) - 1)].astype(np.float64)
        if not receivers_rtt:
            return None
        rtt = rng.choice(np.asarray(receivers_rtt, np.float64), len(kt))
        d = late + rtt / 2 + rng.random(len(kt)) * FRAME_MS
        d = np.clip(d, 0, MAX_DELAY_MS - 1)
    else:
        d = np.full(len(kt), float(delays))
    t = (kt + frac) * kin.TICK_MS
    k0 = np.floor((t - d) / kin.TICK_MS).astype(np.int64)
    h = kt - k0
    lo = k0 - max(WARMUP, HISTORY - 1)
    ok = (lo >= 0) & (h <= HMAX) & ((dead[kt + 1] - dead[lo]) == 0)
    kt, frac, d, k0, h = kt[ok], frac[ok], d[ok], k0[ok], h[ok]
    if len(kt) == 0:
        return None
    hist = k0[:, None] + np.arange(-HISTORY + 1, 1)[None, :]
    fut = np.minimum(k0[:, None] + 1 + np.arange(HMAX)[None, :], n - 1)
    warm = k0[:, None] + np.arange(-WARMUP, 1)[None, :]
    col = np.arange(HMAX)[None, :]
    weights = np.where(col < h[:, None] - 1, kin.TICK_S, 0.0)
    weights = np.where(col == h[:, None] - 1, kin.TICK_S * frac[:, None], weights)

    held = held_state(player.keys)
    state_age = ages(held)
    node_age = ages(np.cumsum(player.node))
    return dict(
        hist_keys=player.keys[hist], hist_yaw=player.yaw[hist], hist_pitch=player.pitch[hist],
        hist_node=player.node[hist], warm_keys=player.keys[warm],
        state_age=state_age[k0], node_age=node_age[k0], delay=d.astype(np.float32),
        horizon=h.astype(np.int32), weights=weights.astype(np.float32),
        # The whole future, also past the horizon: a table fitted for a later
        # tick learns from it (the score counts only the horizon).
        true_keys=player.keys[fut], true_yaw=player.yaw[fut], true_pitch=player.pitch[fut],
        combat=player.combat[kt], pos=player.pos[k0].astype(np.float32),
        heading=player.heading[k0].astype(np.float32))


def match_samples(match, every_ticks, delays, rng):
    parts = []
    for place, player in match.players.items():
        others = [r for p, r in match.rtt.items() if p != place and p in match.players]
        # A match with one human (against bots): its own line stands in for
        # the receiver's.
        if not others and match.rtt.get(place) is not None:
            others = [match.rtt[place]]
        s = player_samples(player, every_ticks, delays, rng, others)
        if s is not None:
            parts.append(s)
    if not parts:
        return None
    out = {k: np.concatenate([p[k] for p in parts]) for k in parts[0]}
    out['match'] = np.zeros(len(out['delay']), np.int32)
    return Samples(names=[match.name], **out)


def map_of(meta):
    return f"{meta.get('map')}"



def start_blend(s):
    blend = kin.Blend(kin.target_of(s.warm_keys[:, 0]))
    for k in range(1, s.warm_keys.shape[1]):
        blend.tick(kin.target_of(s.warm_keys[:, k]))
    return blend


@dataclass
class Prediction:
    keys: np.ndarray   # [S, HMAX]
    yaw: np.ndarray    # [S, HMAX]
    pitch: np.ndarray  # [S, HMAX]
    # [S, 2] a displacement the prediction made before K0 + 1 (a projection
    # that also covers known ticks), m; None: none.
    offset: np.ndarray = None


def truth_run(s, blend=None):
    blend = start_blend(s) if blend is None else blend
    zero = np.zeros(len(s))
    disp, _ = kin.run(blend, s.true_keys, s.true_yaw.astype(np.float64), zero, s.weights)
    return disp


def score(s, pred, blend=None, truth=None):
    """Per sample: position error m, look error degrees, action error share."""
    blend = start_blend(s) if blend is None else blend
    truth = truth_run(s, blend.copy()) if truth is None else truth
    within = np.arange(HMAX)[None, :] < s.horizon[:, None]
    zero = np.zeros(len(s))
    disp, _ = kin.run(blend.copy(), pred.keys, np.where(within, pred.yaw, 0).astype(np.float64),
                      zero, s.weights)
    if pred.offset is not None:
        disp = disp + pred.offset
    pos = np.hypot(*(disp - truth).T)
    ey = np.where(within, s.true_yaw - pred.yaw, 0).sum(1)
    ep = np.where(within, s.true_pitch - pred.pitch, 0).sum(1)
    look = np.degrees(np.hypot(ey, ep))
    one_off = ~np.int64(kin.HELD_KEYS)
    act = within & ((s.true_keys.astype(np.int64) & one_off) != (pred.keys.astype(np.int64) & one_off))
    actions = act.sum(1) / np.maximum(s.horizon, 1)
    return pos, look, actions
