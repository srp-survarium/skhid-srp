"""How well the movement proxy follows the game: from one uploaded world to
a later one of the same life, the body moved on by the proxy with the
player's true inputs, against where the later world has it."""
import numpy as np

from . import kin
from .load import WORLD_BODY

WARMUP = 12
MAX_GAP_MS = 2_000


def pairs(player):
    """(tick index of A, of B, row A, row B) for consecutive worlds."""
    w = player.worlds
    w = w[np.argsort(w['time'], kind='stable')]
    ok = (w['alive'] == 1) & ((w['flags'] & WORLD_BODY) != 0) & np.isfinite(w['x'])
    out = []
    for a in range(len(w) - 1):
        b = a + 1
        if not (ok[a] and ok[b]) or w['inserted'][a] != w['inserted'][b]:
            continue
        gap = int(w['time'][b]) - int(w['time'][a])
        if gap <= 0 or gap > MAX_GAP_MS or w['time'][a] % kin.TICK_MS or w['time'][b] % kin.TICK_MS:
            continue
        ka = int(w['time'][a]) // kin.TICK_MS - player.first
        kb = int(w['time'][b]) // kin.TICK_MS - player.first
        if ka - WARMUP < 0 or kb > len(player.keys):
            continue
        out.append((ka, kb, w[a], w[b]))
    return out


def check(player):
    """Rows: gap ms, proxy error m, distance moved m, yaw error rad,
    world speed, proxy speed at A, jumping flag, crouched flag."""
    rows = []
    for ka, kb, a, b in pairs(player):
        keys = player.keys
        blend = kin.Blend(kin.target_of(keys[ka - WARMUP:ka - WARMUP + 1]))
        for k in range(ka - WARMUP + 1, ka):
            blend.tick(kin.target_of(keys[k:k + 1]))
        h = kb - ka
        turns = player.yaw[ka:kb][None, :].astype(np.float64)
        yaw0 = np.array([a['yaw'] - player.yaw[ka]])
        probe = blend.copy()
        speed_at_a = float(np.hypot(*probe.tick(kin.target_of(keys[ka:ka + 1]))[0]))
        disp, yaw = kin.run(blend, keys[None, ka:kb], turns, yaw0, np.full((1, h), kin.TICK_S))
        moved = np.array([b['x'] - a['x'], b['z'] - a['z']])
        err = float(np.hypot(*(disp[0] - moved)))
        yaw_true = float(b['yaw'] - a['yaw'])
        yaw_inputs = float(player.yaw[ka + 1:kb + 1].sum()) if kb < len(player.yaw) else np.nan
        rows.append((h * kin.TICK_MS, err, float(np.hypot(*moved)), yaw_true - yaw_inputs,
                     float(a['speed']), speed_at_a, int(a['flags'] & 4 != 0 or b['flags'] & 4 != 0),
                     int(a['flags'] & 1 != 0)))
    return np.array(rows, dtype=np.float64).reshape(-1, 8)
