"""Decoded recordings (`prediction-decode`) as per-player tick timelines."""
import json
import pathlib
from dataclasses import dataclass

import numpy as np

from . import kin

INPUT = np.dtype([('place', 'u1'), ('precise', 'u1'), ('pad', '<u2'), ('time', '<u4'),
                  ('arrival', '<i4'), ('yaw', '<f4'), ('pitch', '<f4'), ('actions', '<u4')])
WORLD = np.dtype([('time', '<u4'), ('place', 'u1'), ('alive', 'u1'), ('flags', 'u1'),
                  ('agree', 'u1'), ('inserted', '<u4'), ('x', '<f4'), ('y', '<f4'), ('z', '<f4'),
                  ('yaw', '<f4'), ('pitch', '<f4'), ('actions', '<u4'), ('speed', '<f4')])
WORLD_BODY = 8
# Two worlds this far apart with the same life in both say nothing of the
# time between.
ALIVE_GAP_MS = 6_000
# A shot this close makes a tick a combat one.
COMBAT_MS = 3_000
# A turn of more than this in one tick is not a mouse (a respawn's or a
# resync's set of the view): it is left out of the look's truth.
MAX_TURN = 3.0


@dataclass
class Player:
    """One place's input timeline: tick k is game ms (first + k) * 50."""
    place: int
    first: int          # the first tick
    keys: np.ndarray    # [N] u32 actions held at each tick
    yaw: np.ndarray     # [N] f32 the yaw turn made at each tick
    pitch: np.ndarray   # [N] f32 the pitch turn
    node: np.ndarray    # [N] bool: an input at this tick
    late: np.ndarray    # [n] ms the server got each input after its game time
    alive: np.ndarray   # [N] bool: alive and in a known life
    combat: np.ndarray  # [N] bool: own shot within COMBAT_MS
    worlds: np.ndarray  # this place's world rows (WORLD)
    pos: np.ndarray     # [N, 2] map x, z of the feet before the tick's move (NaN: unknown)
    heading: np.ndarray  # [N] the yaw after the tick's turn (NaN: unknown)


@dataclass
class Match:
    name: str
    meta: dict
    players: dict       # place -> Player (humans with inputs)
    rtt: dict           # place -> median round trip ms (acks)


def timeline(rows):
    """Dense ticks from one place's inputs, sorted by time."""
    ticks = rows['time'].astype(np.int64) // kin.TICK_MS
    first = int(ticks[0])
    n = int(ticks[-1]) - first + 1
    idx = ticks - first
    yaw = np.zeros(n, np.float32)
    pitch = np.zeros(n, np.float32)
    turn_yaw = np.where(np.abs(rows['yaw']) < MAX_TURN, rows['yaw'], 0).astype(np.float32)
    turn_pitch = np.where(np.abs(rows['pitch']) < MAX_TURN, rows['pitch'], 0).astype(np.float32)
    np.add.at(yaw, idx, turn_yaw)
    np.add.at(pitch, idx, turn_pitch)
    node = np.zeros(n, bool)
    node[idx] = True
    # The keys of the last input of each tick, carried forward.
    last = np.full(n, -1, np.int64)
    last[idx] = np.arange(len(rows))
    last = np.maximum.accumulate(last)
    keys = rows['actions'][last].astype(np.uint32)
    return first, keys, yaw, pitch, node


def alive_ticks(worlds, first, n):
    """Ticks between two worlds of one life (both alive, body read)."""
    alive = np.zeros(n, bool)
    w = worlds[np.argsort(worlds['time'], kind='stable')]
    ok = (w['alive'] == 1) & ((w['flags'] & WORLD_BODY) != 0)
    for a, b in zip(range(len(w) - 1), range(1, len(w))):
        if not (ok[a] and ok[b]) or w['inserted'][a] != w['inserted'][b]:
            continue
        ta, tb = int(w['time'][a]), int(w['time'][b])
        if tb - ta > ALIVE_GAP_MS:
            continue
        lo = max(ta // kin.TICK_MS - first, 0)
        hi = min(tb // kin.TICK_MS - first, n)
        if hi > lo:
            alive[lo:hi] = True
    return alive


# A position is carried on from a world this many ticks at most.
CARRY_TICKS = 40


def positions(keys, turns, alive, worlds, first):
    """Each tick's position and yaw: the last world's, moved on by the
    proxy's target velocities (no blend: good to a few decimetres, for map
    cells)."""
    n = len(keys)
    pos = np.full((n, 2), np.nan)
    heading = np.full(n, np.nan)
    w = worlds[(worlds['alive'] == 1) & ((worlds['flags'] & WORLD_BODY) != 0)
               & np.isfinite(worlds['x']) & (worlds['time'] % kin.TICK_MS == 0)]
    w = w[np.argsort(w['time'], kind='stable')]
    wt = w['time'].astype(np.int64) // kin.TICK_MS - first
    keep = (wt >= 0) & (wt < n)
    w, wt = w[keep], wt[keep]
    if len(w) == 0:
        return pos, heading
    k = np.arange(n)
    i = np.searchsorted(wt, k, side='right') - 1
    ok = (i >= 0) & alive
    i = np.maximum(i, 0)
    ok &= (k - wt[i]) <= CARRY_TICKS
    turned = np.cumsum(turns.astype(np.float64))
    yaw = w['yaw'][i] + turned - turned[wt[i]]
    v = kin.to_world(kin.target_of(keys), yaw) * kin.TICK_S
    moved = np.concatenate([[[0.0, 0.0]], np.cumsum(v, 0)[:-1]])
    p = np.stack([w['x'][i], w['z'][i]], 1) + moved - moved[wt[i]]
    pos[ok] = p[ok]
    heading[ok] = yaw[ok]
    return pos, heading


def near(mask, ticks):
    """Ticks within `ticks` of a set one."""
    c = np.concatenate([[0], np.cumsum(mask)])
    n = len(mask)
    i = np.arange(n)
    return (c[np.minimum(i + ticks + 1, n)] - c[np.maximum(i - ticks, 0)]) > 0


def load(path):
    path = pathlib.Path(path)
    meta = json.loads((path / 'meta.json').read_text())
    inputs = np.fromfile(path / 'inputs.bin', INPUT)
    worlds = np.fromfile(path / 'worlds.bin', WORLD)
    humans = {p['place'] for p in meta['places'] if not p.get('bot')}
    rtt = {p['place']: p['rtt_p50'] for p in meta['places'] if p.get('rtt_p50') is not None}
    players = {}
    for place in np.unique(inputs['place']):
        place = int(place)
        if place not in humans:
            continue
        rows = inputs[inputs['place'] == place]
        if len(rows) < 20:
            continue
        rows = rows[np.argsort(rows['time'], kind='stable')]
        first, keys, yaw, pitch, node = timeline(rows)
        n = len(keys)
        own = worlds[worlds['place'] == place]
        alive = alive_ticks(own, first, n)
        combat = near((keys & kin.FIRE) != 0, COMBAT_MS // kin.TICK_MS)
        late = (rows['arrival'].astype(np.int64) - rows['time'].astype(np.int64))
        pos, heading = positions(keys, yaw, alive, own, first)
        players[place] = Player(place, first, keys, yaw, pitch, node, late, alive, combat, own,
                                pos, heading)
    return Match(path.name, meta, players, rtt)


def match_dirs(root):
    return sorted(p for p in pathlib.Path(root).iterdir() if (p / 'meta.json').exists())
