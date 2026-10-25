#!/usr/bin/env python3
"""What else a player wears that can reach the band: back items (straps over the shoulders)
and gloves (how far up the arm they go), measured in the bind pose; then renders of torsos with
the back items that come nearest the upper arm, with the band painted at the table's window.

usage: combos.py bands.tsv [--torso BASE_MODEL ...] [--backs N] [--jobs N]
       (pose.py --model <each back item's model> first, for the posed renders)
Output: $BANDS_WORK/review/combos/
"""
import argparse
import os
import sys
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import bandsenv as E
import bandlib as BL
import gameview as gv
import model
import review as RV
from measure import read_table

DEFAULT_TORSOS = ['character/human/scavengers/scavengers_01_torso_s1',
                  'character/human/black_market/black_market_05_torso_s1',
                  'character/human/renesanse/renesanse_novice_combat_torso']
VIEWS = [('red', 'rest', 'front'), ('red', 'rest', 'side'), ('blue', 'rifle_aim', 'side'), ('red', 'run', 'right'),
         ('blue', 'rest', 'back')]


def item_models(kind):
    """{item config: [base_model, ...]} of gameplay/items/armour/<kind>/."""
    out = {}
    for p in gv.children('gameplay/items/armour/%s/' % kind):
        if not p.endswith('.options'):
            continue
        leaves = gv.config(p)
        ms = sorted({gv.string(v) for k, v in leaves.items() if k.endswith('base_model')})
        if ms:
            out[p.split('/')[-1][:-8]] = ms
    return out


def reach(base, ref='character/human/scavengers/scavengers_01_torso_s1'):
    """Of a model's vertices near an upper arm (within 12 cm of the Arm -> ForeArm line, between
    the shoulder joint and the elbow): how many, and the lowest point down the arm (0 shoulder,
    1 elbow) any of them reaches."""
    _, bp = BL.bind_pose(ref)
    pts = np.concatenate([s['pos'] for _, s, _, _ in model.model(base)])
    n, low = 0, None
    for side in ['Left', 'Right']:
        a, d, L, _, _ = BL.arm_frame(bp, side)
        h = (pts - a) @ d / L
        r = np.linalg.norm((pts - a) - np.outer((pts - a) @ d, d), axis=1)
        near = (r < 0.12) & (h > 0.05) & (h < 1.0)
        n += int(near.sum())
        if near.any():
            low = max(low or 0, float(h[near].max()))
    return n, low


def combo_task(args):
    torso, back, row, models, out = args
    rs, tex, frgb = RV.textures(models, (float(row['window_from']), float(row['window_to'])), torso)
    r = next(x for x in rs if x['base'] == torso)
    cells = [RV.shot([torso, back], r['diffuse'], tex[c], p, v, size=220, label='%s %s %s' % (p, c, v))
             for c, p, v in VIEWS]
    BL._PREP.clear()
    return torso, back, cells


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('table')
    ap.add_argument('--torso', action='append', default=[])
    ap.add_argument('--backs', type=int, default=3)
    ap.add_argument('--jobs', type=int, default=6)
    a = ap.parse_args()
    _, _, rows = read_table(a.table)
    by_mat = {}
    for r in rows:
        by_mat.setdefault(r['material'], []).append((r['base_model'], r['surface']))
    out = E.work('review', 'combos', 'x')[:-2]
    print('back items: vertices near the upper arms, lowest point down the arm (0 shoulder, 1 elbow)')
    backs = []
    for item, ms in sorted(item_models('back').items()):
        for m in ms:
            n, low = reach(m)
            backs.append((n, item, m, low))
            print('  %-40s %-60s %5d %s' % (item, m, n, '-' if low is None else '%.2f' % low))
    print('gloves: lowest point up the arm from the elbow reached (1 = the elbow, smaller = higher)')
    for item, ms in sorted(item_models('gloves').items()):
        for m in ms:
            _, bp = BL.bind_pose('character/human/scavengers/scavengers_01_torso_s1')
            pts = np.concatenate([s['pos'] for _, s, _, _ in model.model(m)])
            tops = []
            for side in ['Left', 'Right']:
                a_, d, L, _, _ = BL.arm_frame(bp, side)
                h = (pts - a_) @ d / L
                r = np.linalg.norm((pts - a_) - np.outer((pts - a_) @ d, d), axis=1)
                arm = (r < 0.12) & (h > 0.0)
                if arm.any():
                    tops.append(float(h[arm].min()))
            print('  %-40s %-60s top %s' % (item, m, '%.2f' % min(tops) if tops else '-'))
    # The back items nearest the arms (an exo's torso model listed under a back item is not one).
    picks = [m for n, _, m, _ in sorted(backs, reverse=True) if '_torso' not in m][:a.backs]
    torsos = a.torso or DEFAULT_TORSOS
    jobs = []
    for t in torsos:
        row = next(r for r in rows if r['base_model'] == t)
        for b in picks:
            jobs.append((t, b, row, by_mat[row['material']], out))
    with Pool(a.jobs) as p:
        res = p.map(combo_task, jobs)
    for t in torsos:
        lines = [cells for tt, b, cells in res if tt == t]
        names = [b.split('/')[-1] for tt, b, _ in res if tt == t]
        path = RV.grid(lines, '%s with back items (rows): %s' % (t, ', '.join(names)),
                       '%s/%s.png' % (out, t.replace('character/human/', '').replace('/', '_')), cell=220)
        print(path)


if __name__ == '__main__':
    main()
