#!/usr/bin/env python3
"""Alternative band windows for chosen torso models, side by side, for the owner to pick:
the table's window, a narrower band at its centre, a band near the elbow, and any window
given with --window. Each row: red rest front and left, blue rifle aim left, red run right,
blue crouch front; its label gives what can be painted, the seen ring, the worst band area
and lean (measure.py's numbers).

usage: alternatives.py bands.tsv BASE_MODEL ... [--window FROM-TO ...] [--jobs N]
Output: $BANDS_WORK/review/alternatives/<model>.png
"""
import argparse
import os
import sys
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bandsenv as E
import bandlib as BL
import review as RV
from measure import read_table, measure_window
from choose import stats

VIEWS = [('red', 'rest', 'front'), ('red', 'rest', 'side'), ('blue', 'rifle_aim', 'side'), ('red', 'run', 'right'),
         ('blue', 'crouch', 'front')]


def options(row, extra):
    w0, w1 = float(row['window_from']), float(row['window_to'])
    c = (w0 + w1) / 2
    out = [('table', (w0, w1)), ('narrow', (round(c - 0.05, 2), round(c + 0.05, 2))), ('near the elbow', (0.62, 0.78))]
    out += [('given', w) for w in extra]
    seen, uniq = set(), []
    for name, w in out:
        if w not in seen:
            seen.add(w)
            uniq.append((name, w))
    return uniq


def task(args):
    row, models, opts, out = args
    base = row['base_model']
    rows = []
    for name, w in opts:
        rs, tex, frgb = RV.textures(models, w, base)
        r = next(x for x in rs if x['base'] == base)
        try:
            s = stats(measure_window(models, w))
            lab = '%s %.2f-%.2f: paint %.0f%%, seen %.0f%%, area x%.2f (%s), lean %.0f (%s)' % (
                name, w[0], w[1], 100 * s['paint'], 100 * s['cloth'], s['mean'], s['mean_pose'], s['lean'], s['lean_pose'])
        except Exception as e:
            lab = '%s %.2f-%.2f: %s' % (name, w[0], w[1], e)
        cells = [RV.shot(base, r['diffuse'], tex[c], p, v, size=240, label='%s %s %s' % (p, c, v)) for c, p, v in VIEWS]
        from PIL import ImageDraw
        ImageDraw.Draw(cells[0]).text((3, 226), lab[:60], fill=(255, 255, 0))
        rows.append((lab, cells))
        BL._PREP.clear()
    label = base.replace('character/human/', '').replace('/', '_')
    path = RV.grid([c for _, c in rows], '%s alternatives: %s' % (base, ' | '.join(l for l, _ in rows))[:230],
                   '%s/%s.png' % (out, label), cell=240)
    return path, [l for l, _ in rows]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('table')
    ap.add_argument('models', nargs='+')
    ap.add_argument('--window', action='append', default=[])
    ap.add_argument('--jobs', type=int, default=6)
    a = ap.parse_args()
    _, _, rows = read_table(a.table)
    by_mat = {}
    for r in rows:
        by_mat.setdefault(r['material'], []).append((r['base_model'], r['surface']))
    extra = [tuple(float(x) for x in w.split('-')) for w in a.window]
    out = E.work('review', 'alternatives', 'x')[:-2]
    jobs = []
    for m in a.models:
        row = next(r for r in rows if r['base_model'] == m)
        jobs.append((row, by_mat[row['material']], options(row, extra), out))
    with Pool(a.jobs) as p:
        for path, labels in p.map(task, jobs):
            print(path)
            for l in labels:
                print('   ', l)


if __name__ == '__main__':
    main()
