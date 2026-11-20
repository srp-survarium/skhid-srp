"""The chosen model's table for the client, as TSV.

    python -m predfit.export results.json 'projected-look[turn]' > table.tsv
"""
import itertools
import json
import sys

import numpy as np

from . import projection, tables

EDGES = {
    'turn': ('turn_from_rad', 'turn_to_rad', tables.TURN_EDGES[:-1] + [None]),
    'elapsed': ('elapsed_from_ticks', 'elapsed_to_ticks', projection.ELAPSED_EDGES[:-1] + [None]),
}


def main():
    res = json.load(open(sys.argv[1]))
    name = sys.argv[2]
    params = res['models'][name]['params']
    feats = params['features']
    gains = np.array(params['gains'])
    real = res['models'][name]['real']['all']
    base = res['models']['hold-last']['real']['all']
    print(f'# {name}: fitted on {res["train_matches"]} recorded matches, scored on '
          f'{res["test_matches"]} others (docs/notes/prediction_fit.md).')
    print(f'# Realistic delay: look error {base["look_deg"]["mean"]:.2f} -> '
          f'{real["look_deg"]["mean"]:.2f} deg, position {base["pos_cm"]["mean"]:.2f} -> '
          f'{real["pos_cm"]["mean"]:.2f} cm (mean), against hold-last.')
    print('# Keys: the newest input node\'s mask carried on unchanged (what the game does).')
    print('# Look: one projected input at the newest node\'s tick + 50 ms, made again every')
    print('# frame and taken out before each confirmed move: yaw turn = the node\'s yaw turn')
    print(f'# x yaw_gain / {projection.GAIN_SCALE}, pitch likewise. A bucket holds from <= x < to;')
    print('# turn = |yaw turn| + |pitch turn| of the node (radians). No projection while now is in')
    print('# the node\'s own tick, nor for a node 24 ticks (1.2 s) old or more.')
    if 'elapsed' in feats:
        print('# elapsed = whole ticks from the node\'s tick to now, the tick under way counted.')
    cols = []
    for f in feats:
        cols += list(EDGES[f][:2])
    print('\t'.join(cols + ['yaw_gain', 'pitch_gain']))
    sizes = [len(EDGES[f][2]) - 1 for f in feats]
    for i, idx in enumerate(itertools.product(*[range(n) for n in sizes])):
        row = []
        for f, b in zip(feats, idx):
            edges = EDGES[f][2]
            row += [f'{edges[b]:g}', '-' if edges[b + 1] is None else f'{edges[b + 1]:g}']
        g = gains[i]
        row += [str(int(round(g[0] * projection.GAIN_SCALE))),
                str(int(round(g[1] * projection.GAIN_SCALE)))]
        print('\t'.join(row))


if __name__ == '__main__':
    main()
