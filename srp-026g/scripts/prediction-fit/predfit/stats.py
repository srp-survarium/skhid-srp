"""The dataset in aggregate, the delays and the movement proxy's own error.

    python -m predfit.stats --data <decoded dir> --cache <samples dir> --out <stats.json>

Only counts and distributions are written: no place, account or match.
"""
import argparse
import collections
import json
import multiprocessing

import numpy as np

from . import kin, load, proxy, samples

PCT = [5, 10, 25, 50, 75, 90, 95, 99]


def one(path):
    m = load.load(path)
    out = {'mode': m.meta.get('mode'), 'humans': len(m.players), 'alive_ticks': 0, 'ticks': 0,
           'combat_ticks': 0, 'nodes': 0, 'late': [], 'rtt': [], 'proxy': [], 'held_runs': [],
           'moving_ticks': 0, 'turning_ticks': 0, 'revisions': []}
    for p in m.players.values():
        out['ticks'] += len(p.keys)
        out['alive_ticks'] += int(p.alive.sum())
        out['combat_ticks'] += int((p.alive & p.combat).sum())
        out['nodes'] += int(p.node.sum())
        out['late'].append(p.late)
        a = p.alive
        out['moving_ticks'] += int((a & ((p.keys & kin.MOVE_KEYS) != 0)).sum())
        out['turning_ticks'] += int((a & ((p.yaw != 0) | (p.pitch != 0))).sum())
        held = p.keys & kin.HELD_KEYS
        change = np.flatnonzero(np.diff(held.astype(np.int64)) != 0)
        runs = np.diff(change)
        out['held_runs'].append(runs[runs > 0])
        r = proxy.check(p)
        if len(r):
            out['proxy'].append(r)
    for pl in m.meta['places']:
        if pl.get('rtt_p50') is not None and pl['place'] in m.players:
            out['rtt'].append(pl['rtt_p50'])
        if pl.get('revision') is not None and pl['place'] in m.players:
            out['revisions'].append(pl['revision'])
    for k in ('late', 'held_runs', 'proxy'):
        out[k] = np.concatenate(out[k]) if out[k] else np.zeros((0, 8) if k == 'proxy' else 0)
    return out


def pct(v):
    v = np.asarray(v, np.float64)
    return {str(p): float(np.percentile(v, p)) for p in PCT} | {'mean': float(v.mean()),
                                                                 'n': int(len(v))}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True)
    ap.add_argument('--cache', required=True)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    dirs = load.match_dirs(args.data)
    with multiprocessing.Pool(12) as pool:
        rows = pool.map(one, dirs, chunksize=4)
    rows = [r for r in rows if r['humans']]
    hours = lambda t: t * kin.TICK_S / 3600
    late = np.concatenate([r['late'] for r in rows])
    proxy_rows = np.concatenate([r['proxy'] for r in rows])
    real = samples.read(args.cache, 'real', test=True)
    real_train = samples.read(args.cache, 'real', test=False)
    delays = np.concatenate([real.delay, real_train.delay])
    stats = {
        'matches': len(rows),
        'matches_by_mode': dict(collections.Counter(str(r['mode']) for r in rows)),
        'matches_by_humans': dict(sorted(collections.Counter(
            '1' if r['humans'] == 1 else '2-5' if r['humans'] <= 5 else '6-10'
            if r['humans'] <= 10 else '11+' for r in rows).items())),
        'player_hours_in_match': hours(sum(r['ticks'] for r in rows)),
        'player_hours_alive': hours(sum(r['alive_ticks'] for r in rows)),
        'player_hours_combat': hours(sum(r['combat_ticks'] for r in rows)),
        'share_alive_moving': sum(r['moving_ticks'] for r in rows) / sum(r['alive_ticks'] for r in rows),
        'share_alive_turning': sum(r['turning_ticks'] for r in rows) / sum(r['alive_ticks'] for r in rows),
        'inputs': int(sum(r['nodes'] for r in rows)),
        'revisions': dict(collections.Counter(str(v) for r in rows for v in r['revisions'])),
        'server_lateness_ms': pct(late[(late > -1000) & (late < 5000)]),
        'round_trip_ms_per_player': pct(np.concatenate([r['rtt'] for r in rows])),
        'end_to_end_delay_ms': pct(delays),
        'held_state_run_ticks': pct(np.concatenate([r['held_runs'] for r in rows])),
        'proxy': {},
    }
    gap = proxy_rows[:, 0]
    for lo, hi in [(50, 100), (100, 200), (200, 300), (300, 500), (500, 1000), (1000, 2001)]:
        sel = (gap >= lo) & (gap < hi) & (proxy_rows[:, 6] == 0)
        if sel.sum() == 0:
            continue
        s = proxy_rows[sel]
        stats['proxy'][f'{lo}-{hi - 1}ms'] = {
            'pairs': int(sel.sum()), 'error_m_p50': float(np.median(s[:, 1])),
            'error_m_p90': float(np.percentile(s[:, 1], 90)),
            'moved_m_p50': float(np.median(s[:, 2])), 'moved_m_p90': float(np.percentile(s[:, 2], 90)),
            'yaw_error_rad_p99': float(np.nanpercentile(np.abs(s[:, 3]), 99))}
    sp = proxy_rows[np.isfinite(proxy_rows[:, 4])]
    stats['proxy']['speed_abs_diff_m_s_p50'] = float(np.median(np.abs(sp[:, 4] - sp[:, 5])))
    with open(args.out, 'w') as f:
        json.dump(stats, f, indent=1)
    print(json.dumps(stats, indent=1))


if __name__ == '__main__':
    main()
