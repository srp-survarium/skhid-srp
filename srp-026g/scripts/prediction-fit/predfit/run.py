"""Fits the models on the training matches and scores them on the test ones.

    python -m predfit.run --data <decoded dir> --cache <samples dir> --out <results.json>
"""
import argparse
import json
import time

import numpy as np

from . import load, models, projection, samples, tables
from .harness import score

LABELS = ['real', '50', '100', '150', '250']


MOVE_GROWTH = ['moves', 'stance', 'hold_age', 'since_input', 'speed', 'turn']
LOOK_GROWTH = ['turn', 'since_input', 'moves', 'stance', 'hold_age', 'speed']
PROJECTED_GROWTH = ['turn', 'elapsed', 'stance', 'moves']


def table_set():
    out = []
    for i in range(1, len(MOVE_GROWTH) + 1):
        out.append(tables.MoveTable(MOVE_GROWTH[:i]))
    for i in range(1, len(LOOK_GROWTH) + 1):
        out.append(tables.LookTable(LOOK_GROWTH[:i]))
    for i in range(1, len(PROJECTED_GROWTH) + 1):
        out.append(projection.ProjectedLook(PROJECTED_GROWTH[:i]))
    return out


def model_set():
    return [
        models.HoldLast(),
        models.HoldHeld(),
        models.Fade(),
        models.Hazard(),
        models.Markov(1),
        models.Markov(2),
        models.DampedLook(models.HoldHeld()),
        models.DampedLook(models.HoldLast()),
        models.DampedLook(models.Hazard()),
    ]


def summary(values):
    return {'mean': float(np.mean(values)), 'p50': float(np.median(values)),
            'p90': float(np.percentile(values, 90)), 'p99': float(np.percentile(values, 99))}


def several_humans(data):
    """The matches with two humans or more (not against bots alone)."""
    out = set()
    for d in load.match_dirs(data):
        meta = json.loads((d / 'meta.json').read_text())
        if sum(1 for p in meta['places'] if not p.get('bot') and p.get('inputs', 0) >= 20) >= 2:
            out.add(d.name)
    return out


def evaluate(model, s, pvp):
    pos, look, act = score(s, model.predict(s))
    out = {}
    humans = np.isin(np.array(s.names)[s.match], list(pvp))
    for subset, mask in (('all', np.ones(len(s), bool)), ('combat', s.combat),
                         ('pvp', humans), ('pvp_combat', humans & s.combat)):
        if not mask.any():
            continue
        out[subset] = {'n': int(mask.sum()), 'pos_cm': summary(pos[mask] * 100),
                       'look_deg': summary(look[mask]), 'action_share': float(act[mask].mean())}
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True)
    ap.add_argument('--cache', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--every', type=int, default=40, help='ticks between samples of a player')
    ap.add_argument('--limit', type=int, default=None, help='matches read per side (testing)')
    ap.add_argument('--ceiling', action='store_true', help='also fit the learned ceiling')
    ap.add_argument('--flow', action='store_true', help='also fit the per-map flow tables')
    args = ap.parse_args()
    t0 = time.time()
    samples.build(args.data, args.cache, args.every)
    print(f'samples built {time.time() - t0:.0f}s', flush=True)
    train = samples.read(args.cache, 'real', test=False, limit=args.limit)
    tests = {label: samples.read(args.cache, label, test=True, limit=args.limit) for label in LABELS}
    print(f'train {len(train)} samples of {len(train.names)} matches; test '
          + ', '.join(f'{k} {len(v)}' for k, v in tests.items()), flush=True)
    results = {'train_matches': len(train.names), 'test_matches': len(tests['real'].names),
               'train_samples': len(train), 'models': {}}
    pvp = several_humans(args.data)
    ms = model_set() + table_set()
    if args.flow:
        from . import flow
        maps = flow.FlowMaps(args.data)
        ms.append(flow.FlowLook(models.DampedLook(models.HoldLast()), maps))
        ms.append(flow.FlowLook(tables.LookTable(["turn"]), maps))
    if args.ceiling:
        from . import ceiling
        ms.append(ceiling.Boosted())
    for model in ms:
        t1 = time.time()
        model.fit(train)
        row = {'params': model.params()}
        for label, s in tests.items():
            row[label] = evaluate(model, s, pvp)
        results['models'][model.name] = row
        r = row['real']['all']
        print(f"{model.name:40s} real: pos mean {r['pos_cm']['mean']:.2f} cm p90 "
              f"{r['pos_cm']['p90']:.2f}; look mean {r['look_deg']['mean']:.3f} deg; "
              f"{time.time() - t1:.0f}s", flush=True)
    with open(args.out, 'w') as f:
        json.dump(results, f, indent=1)


if __name__ == '__main__':
    main()
