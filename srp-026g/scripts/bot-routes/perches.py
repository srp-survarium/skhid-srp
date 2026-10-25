"""Human perches for the bots (play 17's table, `bots/routes.rs`): where real players stand still on high ground.

Reads the routes tool's tables made from the live host's saved worlds (private; this script holds no data):
  matches.json   {match: {"level": 1-8, "mode": 2|3|4, "project": "level_01_evn_4", "alive_places": n,
                          "bot_match": bool, ...}}
  positions.csv  match,time,place,alive,x,y,z,...,spawned   (time and spawned in ms; x, y, z the capsule's centre, so the
                 feet are y - 0.9; one row about every 5 s of a place; `spawned` is the life's start)
and writes the two optional top-level keys of `bot-routes.json` (`docs/notes/player_bots_mapdata.md`, "Human perches"):
  "perches":    [{"level": "level_01", "x": .., "y": .., "z": .., "rate": .., "dwell": .., "n": ..}, ...]
  "perch_maps": [{"level": "level_01", "p75": .., "share": .., "share_batteries": .., "share_tdm": ..,
                  "visits": .., "minutes": ..}, ...]

A perch visit is a run of one life's samples that are all above the level's p75 feet height, within STILL_M of the run's first
sample, no more than GAP_S apart, and lasting at least MIN_DWELL_S: a stay point (a distance and a time threshold, as in
stay-point detection of trajectories). A run that is the life's wait at its spawn is no visit (SPAWN_WAIT_S, SPAWN_WAIT_M).
Visits are grouped by peak cover (CLUSTER_M across, CLUSTER_DY_M up or down): the visit with most neighbours takes all its
unassigned neighbours, then the next one. A perch is a group's mean x, z and median feet y; `rate` is its visits per 1000 alive
player-minutes of the level, `dwell` the median dwell in seconds (the worlds are about 5 s apart, so it comes in 5 s steps),
`n` its visits. The perches kept are the groups seen in MIN_MATCHES matches, most visits first, until they cover COVER of the
level's visits. `perch_maps` holds per level the p75 line, the share of alive time spent on a perch visit (`share`: all modes;
`share_batteries`: modes 2 and 3; `share_tdm`: mode 4, each from that mode's matches alone, with the level's p75 line and the same
rules; a field is left out where the level has no alive time in that mode), and the totals behind `rate`.

usage: perches.py <matches.json> <positions.csv> [--routes routes.json --out merged.json] [--min-players 12]
       [--weather all|day] [--print]
`--routes` + `--out` write that table with the two keys set (its other keys kept); without them nothing is written.
"""
import argparse
import collections
import csv
import json
import math
import statistics
import sys

FEET_BELOW_CENTRE_M = 0.9
MIN_DWELL_S = 3.0
STILL_M = 1.5
GAP_S = 7.0
CLUSTER_M = 4.0
CLUSTER_DY_M = 2.0
COVER = 0.8
MIN_MATCHES = 3
SPAWN_WAIT_S = 30.0
SPAWN_WAIT_M = 4.0
HIGH_Q = 0.75
LEVELS = range(1, 9)
MODES = (2, 3, 4)
THIN_MIN = 1000.0


def weather_of(project):
    for tag in ('_evn_', '_rain_', '_prerain_'):
        if tag in project:
            return tag.strip('_')
    return 'day'


def load_matches(path, min_players, weather):
    """The human matches kept: {match: info}."""
    out = {}
    with open(path, encoding='utf-8') as fh:
        info = json.load(fh)
    for m, v in info.items():
        if v.get('bot_match') or (v.get('alive_places') or 0) < min_players or v.get('level') not in LEVELS:
            continue
        if v.get('mode') not in MODES:
            continue
        if weather == 'day' and weather_of(v.get('project') or '') != 'day':
            continue
        out[m] = v
    return out


def load_lives(path, matches):
    """{level: [(match, samples, spawned_ms)]}; samples = [(ms, x, feet_y, z)] of one life, alive rows only."""
    rows = collections.defaultdict(list)
    with open(path, newline='', encoding='utf-8') as fh:
        for r in csv.DictReader(fh):
            m = r['match']
            if m in matches and r['alive'] == '1' and r['x']:
                rows[(m, r['place'], int(r['spawned']))].append(
                    (int(r['time']), float(r['x']), float(r['y']) - FEET_BELOW_CENTRE_M, float(r['z'])))
    lives = collections.defaultdict(list)
    for (m, _, spawned), v in rows.items():
        v.sort()
        lives[matches[m]['level']].append((m, v, spawned))
    return lives


def p75_of(lives):
    ys = sorted(p[2] for _, v, _ in lives for p in v)
    return ys[int(HIGH_Q * (len(ys) - 1))]


def visits_of(lives, p75):
    """Perch visits of one level: [(x, feet_y, z, dwell_s, match)]."""
    out = []
    for m, v, spawned in sorted(lives, key=lambda life: (life[0], life[2], life[1][0])):
        i = 0
        while i < len(v):
            t, x, y, z = v[i]
            j = i
            if y > p75:
                while (j + 1 < len(v) and v[j + 1][2] > p75 and 0 < v[j + 1][0] - v[j][0] <= GAP_S * 1000
                       and math.hypot(v[j + 1][1] - x, v[j + 1][3] - z) < STILL_M):
                    j += 1
                dwell = (v[j][0] - t) / 1000.0
                if dwell >= MIN_DWELL_S:
                    run = v[i:j + 1]
                    first = v[0]
                    waiting = ((t - spawned) / 1000.0 < SPAWN_WAIT_S
                               and math.dist((x, y, z), (first[1], first[2], first[3])) < SPAWN_WAIT_M)
                    if not waiting:
                        out.append((sum(p[1] for p in run) / len(run), statistics.median(p[2] for p in run),
                                    sum(p[3] for p in run) / len(run), dwell, m))
            i = j + 1 if j > i else i + 1
    return out


def alive_minutes(lives):
    s = 0.0
    for _, v, _ in lives:
        for a, b in zip(v, v[1:]):
            dt = (b[0] - a[0]) / 1000.0
            if 0 < dt <= GAP_S:
                s += dt
    return s / 60.0


def cluster(visits):
    """Peak cover. Deterministic and independent of the order of `visits`: they are sorted first, and a tie is taken by that order."""
    vs = sorted(visits, key=lambda v: (round(v[0], 3), round(v[2], 3), round(v[1], 3), v[4]))
    n = len(vs)
    cell = {}
    for k, v in enumerate(vs):
        cell.setdefault((int(v[0] // CLUSTER_M), int(v[2] // CLUSTER_M)), []).append(k)
    near = []
    for v in vs:
        cx, cz = int(v[0] // CLUSTER_M), int(v[2] // CLUSTER_M)
        near.append([q for dx in (-1, 0, 1) for dz in (-1, 0, 1) for q in cell.get((cx + dx, cz + dz), ())
                     if math.hypot(vs[q][0] - v[0], vs[q][2] - v[2]) < CLUSTER_M and abs(vs[q][1] - v[1]) < CLUSTER_DY_M])
    free = [True] * n
    count = [len(x) for x in near]
    groups = []
    while True:
        best, pick = 0, -1
        for k in range(n):
            if free[k] and count[k] > best:
                best, pick = count[k], k
        if pick < 0:
            return groups
        members = [q for q in near[pick] if free[q]]
        for q in members:
            free[q] = False
        for q in members:
            for r in near[q]:
                count[r] -= 1
        groups.append([vs[q] for q in members])


BATTERIES = (2, 3)
TDM = (4,)


def share_of(lives, p75):
    """(share of alive time on a perch visit, alive minutes) of these lives; share None without alive time."""
    minutes = alive_minutes(lives)
    if minutes <= 0:
        return None, 0.0
    return sum(v[3] for v in visits_of(lives, p75)) / 60.0 / minutes, minutes


def build_level(level, lives, modes):
    """(perches, perch_maps row, stats) of one level; `modes` maps a match to its mode."""
    p75 = p75_of(lives)
    minutes = alive_minutes(lives)
    visits = visits_of(lives, p75)
    name = f'level_{level:02d}'
    groups = sorted(cluster(visits), key=lambda g: (-len(g), g[0][0], g[0][2]))
    kept, covered = [], 0
    for g in groups:
        if len({m[4] for m in g}) < MIN_MATCHES:
            continue
        kept.append(dict(level=name, x=round(sum(m[0] for m in g) / len(g), 1), y=round(statistics.median(m[1] for m in g), 1),
                         z=round(sum(m[2] for m in g) / len(g), 1), rate=round(1000.0 * len(g) / minutes, 2),
                         dwell=round(statistics.median(m[3] for m in g), 1), n=len(g)))
        covered += len(g)
        if covered >= COVER * len(visits):
            break
    share = sum(v[3] for v in visits) / 60.0 / minutes if minutes else 0.0
    info = dict(level=name, p75=round(p75, 1), share=round(share, 4))
    stats = dict(groups=len(groups), kept=len(kept), covered=covered / len(visits) if visits else 0.0,
                 matches=len({m for m, _, _ in lives}))
    for key, wanted in (('share_batteries', BATTERIES), ('share_tdm', TDM)):
        part = [life for life in lives if modes[life[0]] in wanted]
        mode_share, mode_minutes = share_of(part, p75)
        if mode_share is not None:
            info[key] = round(mode_share, 4)
        stats[key] = (mode_share, mode_minutes, len({m for m, _, _ in part}))
    info.update(visits=len(visits), minutes=round(minutes, 1))
    return kept, info, stats


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('matches')
    ap.add_argument('positions')
    ap.add_argument('--routes')
    ap.add_argument('--out')
    ap.add_argument('--min-players', type=int, default=12)
    ap.add_argument('--weather', choices=('all', 'day'), default='all')
    ap.add_argument('--print', action='store_true')
    a = ap.parse_args()
    matches = load_matches(a.matches, a.min_players, a.weather)
    lives = load_lives(a.positions, matches)
    perches, maps = [], []
    for level in LEVELS:
        if level not in lives:
            continue
        kept, info, stats = build_level(level, lives[level], {m: v['mode'] for m, v in matches.items()})
        perches += kept
        maps.append(info)
        if a.print:
            print(f"{info['level']}: {stats['matches']} matches, {info['minutes']:.0f} alive player-min, p75 {info['p75']} m, "
                  f"{info['visits']} visits ({1000 * info['visits'] / info['minutes']:.0f} per 1000 min), "
                  f"{100 * info['share']:.1f} % of alive time on a perch; {stats['groups']} groups, kept {stats['kept']} "
                  f"covering {100 * stats['covered']:.0f} % of the visits", file=sys.stderr)
            for key in ('share_batteries', 'share_tdm'):
                mode_share, mode_minutes, mode_matches = stats[key]
                text = 'none' if mode_share is None else f'{100 * mode_share:.1f} %'
                thin = ' THIN' if mode_minutes < THIN_MIN else ''
                print(f"    {key}: {text} of {mode_minutes:.0f} alive min in {mode_matches} matches{thin}", file=sys.stderr)
    if a.out:
        table = {'format': 2, 'routes': []}
        if a.routes:
            with open(a.routes, encoding='utf-8') as fh:
                table = json.load(fh)
        table['perches'] = perches
        table['perch_maps'] = maps
        with open(a.out, 'w', encoding='utf-8', newline='\n') as fh:
            json.dump(table, fh, separators=(',', ':'))
            fh.write('\n')


if __name__ == '__main__':
    main()
