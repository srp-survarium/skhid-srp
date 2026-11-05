"""A per-map "flow" table: map cell -> the typical direction of travel there,
from the training matches; a model then turns a moving player's look
toward it. Shipped per map as content if it paid, so it has to beat the
generic tables clearly."""
import multiprocessing

import numpy as np

from . import kin, load
from .harness import HMAX, Prediction, is_test

CELL_M = 4.0
EVERY = 5          # ticks between the travel samples of a player
MIN_COUNT = 20
MIN_SPEED = 1.0


def _travel(path):
    m = load.load(path)
    out = []
    for p in m.players.values():
        k = np.arange(0, len(p.keys), EVERY)
        v = kin.to_world(kin.target_of(p.keys[k]), np.nan_to_num(p.heading[k]))
        ok = np.isfinite(p.pos[k, 0]) & (np.hypot(*v.T) >= MIN_SPEED)
        if ok.any():
            out.append(np.concatenate([p.pos[k][ok], v[ok] / np.hypot(*v[ok].T)[:, None]], 1))
    return m.meta.get('map'), m.name, (np.concatenate(out) if out else np.zeros((0, 4)))


def cell_of(pos):
    return np.floor(pos / CELL_M).astype(np.int64)


class FlowMaps:
    """Per map: cell -> (mean unit travel vector, count)."""

    def __init__(self, data):
        dirs = [d for d in load.match_dirs(data) if not is_test(d.name)]
        with multiprocessing.Pool(12) as pool:
            rows = pool.map(_travel, dirs, chunksize=4)
        self.match_map = {}
        per_map = {}
        for map_name, _, r in rows:
            per_map.setdefault(map_name, []).append(r)
        for d in load.match_dirs(data):
            import json
            self.match_map[d.name] = json.loads((d / 'meta.json').read_text()).get('map')
        self.maps = {}
        for name, parts in per_map.items():
            r = np.concatenate(parts)
            if not len(r):
                continue
            c = cell_of(r[:, :2])
            lo = c.min(0)
            dims = c.max(0) - lo + 1
            idx = (c[:, 0] - lo[0]) * dims[1] + (c[:, 1] - lo[1])
            n = np.bincount(idx, minlength=dims[0] * dims[1])
            sx = np.bincount(idx, r[:, 2], dims[0] * dims[1])
            sz = np.bincount(idx, r[:, 3], dims[0] * dims[1])
            self.maps[name] = (lo, dims, n, sx, sz)

    def cells_bytes(self):
        """Bytes per map at 2 bytes a cell (direction and strength), the
        cells of the map's bounding box."""
        return {k: int(v[1][0] * v[1][1] * 2) for k, v in self.maps.items()}

    def flow(self, map_name, pos):
        """Per sample: the travel vector (x, z) and its strength in 0..1."""
        out = np.zeros((len(pos), 2))
        strength = np.zeros(len(pos))
        if map_name not in self.maps:
            return out, strength
        lo, dims, n, sx, sz = self.maps[map_name]
        c = cell_of(np.nan_to_num(pos, nan=-1e9)) - lo
        ok = np.isfinite(pos[:, 0]) & (c[:, 0] >= 0) & (c[:, 1] >= 0) & (c[:, 0] < dims[0]) & \
            (c[:, 1] < dims[1])
        idx = np.where(ok, c[:, 0] * dims[1] + c[:, 1], 0)
        cnt = np.where(ok, n[idx], 0)
        good = cnt >= MIN_COUNT
        vec = np.stack([sx[idx], sz[idx]], 1) / np.maximum(cnt, 1)[:, None]
        out[good] = vec[good]
        strength[good] = np.hypot(*vec[good].T)
        return out, strength


class FlowLook:
    """A look model with a turn toward the cell's flow added each tick:
    gain x (the flow's yaw - the heading), when the flow is strong and the
    player moves."""

    def __init__(self, base, flow_maps, min_strength=0.6):
        self.base, self.flow_maps, self.min_strength = base, flow_maps, min_strength
        self.name = base.name + ' + flow'
        self.gain = np.zeros(HMAX)

    def _delta(self, s):
        delta = np.zeros(len(s))
        names = np.array(s.names)[s.match]
        for name in np.unique(names):
            sel = names == name
            vec, strength = self.flow_maps.flow(self.flow_maps.match_map.get(name), s.pos[sel])
            target = np.arctan2(-vec[:, 0], vec[:, 1])
            d = (target - s.heading[sel] + np.pi) % (2 * np.pi) - np.pi
            moving = (s.hist_keys[sel, -1].astype(np.int64) & kin.MOVE_KEYS) != 0
            delta[sel] = np.where((strength >= self.min_strength) & moving & np.isfinite(d), d, 0)
        return delta

    def fit(self, train):
        self.base.fit(train)
        base = self.base.predict(train)
        d = self._delta(train)
        for h in range(HMAX):
            resid = train.true_yaw[:, h] - base.yaw[:, h]
            den = (d * d).sum()
            self.gain[h] = (d * resid).sum() / den if den > 0 else 0
        return self

    def predict(self, s):
        p = self.base.predict(s)
        add = self._delta(s)[:, None] * self.gain[None, :]
        return Prediction(p.keys, (p.yaw + add).astype(np.float32), p.pitch)

    def keys(self, s):
        return self.base.keys(s)

    def params(self):
        sizes = self.flow_maps.cells_bytes()
        return {**self.base.params(), 'flow_gain': self.gain.tolist(), 'maps': len(sizes),
                'bytes_per_map_median': float(np.median(list(sizes.values()))) if sizes else 0,
                'bytes_all_maps': int(sum(sizes.values()))}
