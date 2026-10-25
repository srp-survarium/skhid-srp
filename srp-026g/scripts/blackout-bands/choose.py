#!/usr/bin/env python3
"""Choose each sleeve material's band window from measure.json and rewrite bands.tsv's numbers.

usage: choose.py bands.tsv [--dry]

A row whose status is not `draft` keeps its window and method (a reviewed choice); its numbers
are refreshed. For a draft row: among the measured windows where at least PAINT_MIN of the
band can be painted on every model of the material, the one with the most visible cloth,
less penalties for stretch (the band-area-weighted mean posed/bind face area, worst pose) and
for the edges' lean from square to the posed upper arm (worst pose); then a status:
problem (with why) when the best still fails a limit, adjusted (with what) when the window
moved from the table's, good otherwise.
"""
import argparse
import json
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bandsenv as E
from measure import read_table
import texband as TB

PAINT_MIN = 0.88
STRETCH_OK, STRETCH_BAD = 1.3, 1.8    # band-area-weighted mean posed/bind area, worst pose
LEAN_OK, LEAN_BAD = 12.0, 30.0
CLOTH_BAD = 0.45
HEAD = ['base_model', 'surface', 'material', 'sides', 'method', 'window_from', 'window_to', 'selection_rule',
        'paintable_pct', 'hole_before_cm2', 'hole_after_cm2', 'shared_texels', 'visible_cloth_pct',
        'covered_pct', 'stretch_mean', 'stretch_p95', 'stretch_worst_pose', 'lean_deg', 'lean_worst_pose', 'status', 'notes']
RULE = ('per-texel: sleeve faces (a vertex Arm+Arm_Roll>=%.1f, edge flood from cloth seeds, no piece on top '
        'within %d mm); texel Arm+Arm_Roll>=%.1f, forearm<=%.2f; edges AA %.1f texels; gutter %d'
        % (TB.ARM_MIN, round(TB.LAYER * 1000), TB.ARM_MIN, TB.FOREARM_MAX, TB.AA, TB.GUTTER))


def stats(win):
    """Material-level numbers for one measured window (worst model)."""
    ms = win['models'].values()
    paint = min(m['paintable'] for m in ms)
    cloth = min(min(m['cloth']) for m in ms)
    p95, p95_pose, mean, mean_pose, lean, lean_pose = 0.0, '', 0.0, '', 0.0, ''
    for m in ms:
        for pose, v in m['poses'].items():
            if v['stretch_p95'] > p95:
                p95, p95_pose = v['stretch_p95'], pose
            if v['stretch_mean'] > mean:
                mean, mean_pose = v['stretch_mean'], pose
            if v['lean'] == v['lean'] and v['lean'] > lean:
                lean, lean_pose = v['lean'], pose
    return dict(window=tuple(win['window']), paint=paint, cloth=cloth, p95=p95, p95_pose=p95_pose, mean=mean,
                mean_pose=mean_pose, lean=lean, lean_pose=lean_pose, shared=win['shared_texels'],
                before=max(m['hole_before_cm2'] for m in ms), after=max(m['hole_after_cm2'] for m in ms))


def score(s):
    w = s['window'][1] - s['window'][0]
    return (min(s['cloth'], 0.95) - 0.6 * max(0.0, s['mean'] - STRETCH_OK)
            - 0.015 * max(0.0, s['lean'] - LEAN_OK) + (0.03 if w >= 0.19 else 0.0))


def why_problem(s):
    out = []
    if s['cloth'] < CLOTH_BAD:
        out.append('mostly covered: %.0f%% of the ring seen' % (100 * s['cloth']))
    if s['lean'] > LEAN_BAD:
        out.append('edges lean %.0f deg in %s (sleeve weight shear)' % (s['lean'], s['lean_pose']))
    if s['mean'] > STRETCH_BAD:
        out.append('band area x%.1f in %s' % (s['mean'], s['mean_pose']))
    if s['paint'] < PAINT_MIN:
        out.append('only %.0f%% paintable' % (100 * s['paint']))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('table')
    ap.add_argument('--dry', action='store_true')
    a = ap.parse_args()
    lines, _, rows = read_table(a.table)
    comments = [l for l in lines if l.startswith('#')]
    meas = json.load(open(os.path.join(E.WORK, 'measure.json')))
    chosen = {}
    for mat, m in meas.items():
        wins = [stats(w) for w in m['windows'] if 'error' not in w]
        ok = [s for s in wins if s['paint'] >= PAINT_MIN] or wins
        chosen[mat] = (max(ok, key=score), {s['window']: s for s in wins})
    out = []
    for r in rows:
        best, by_win = chosen[r['material']]
        old = (float(r['window_from']), float(r['window_to']))
        reviewed = r.get('status', 'draft') not in ('draft', '')
        s = by_win.get(old, best) if reviewed else best
        m = meas[r['material']]
        win = next(w for w in m['windows'] if tuple(w['window']) == s['window'])
        mm = win['models'][r['base_model']]
        row = dict(r)
        row.update(window_from='%.2f' % s['window'][0], window_to='%.2f' % s['window'][1], selection_rule=RULE,
                   method=row.get('method') or 'paint', sides=row.get('sides') or 'both',
                   paintable_pct='%.0f' % (100 * mm['paintable']), hole_before_cm2='%.1f' % mm['hole_before_cm2'],
                   hole_after_cm2='%.1f' % mm['hole_after_cm2'], shared_texels=str(win['shared_texels']),
                   visible_cloth_pct='%.0f' % (100 * min(mm['cloth'])), covered_pct='%.0f' % (100 * (1 - min(mm['cloth']))))
        p95 = max(mm['poses'].items(), key=lambda kv: kv[1]['stretch_mean'])
        lean = max(((k, v) for k, v in mm['poses'].items() if v['lean'] == v['lean']), key=lambda kv: kv[1]['lean'])
        row.update(stretch_mean='%.2f' % p95[1]['stretch_mean'], stretch_p95='%.2f' % p95[1]['stretch_p95'], stretch_worst_pose=p95[0],
                   lean_deg='%.0f' % lean[1]['lean'], lean_worst_pose=lean[0])
        if not reviewed:
            problems = why_problem(s)
            if problems:
                row['status'], row['notes'] = 'problem', '; '.join(problems)
            elif s['window'] != old:
                row['status'] = 'adjusted'
                row['notes'] = 'window %.2f-%.2f -> %.2f-%.2f (seen %.0f%%, area x%.2f, lean %.0f deg)' % (
                    old[0], old[1], s['window'][0], s['window'][1], 100 * s['cloth'], s['mean'], s['lean'])
            else:
                row['status'], row['notes'] = 'good', ''
        out.append(row)
    text = '\n'.join(l for l in comments if 'face-level sweep' not in l) + '\n'
    text += '\t'.join(HEAD) + '\n' + ''.join('\t'.join(r.get(h, '') for h in HEAD) + '\n' for r in out)
    if a.dry:
        print(text)
    else:
        open(a.table, 'w').write(text)
    for mat, (s, _) in sorted(chosen.items(), key=lambda kv: score(kv[1][0])):
        print('%-72s %.2f-%.2f paint %3.0f%% seen %3.0f%% area x%.2f (%s) lean %2.0f (%s)' % (
            mat, s['window'][0], s['window'][1], 100 * s['paint'], 100 * s['cloth'], s['mean'], s['mean_pose'], s['lean'], s['lean_pose']))


if __name__ == '__main__':
    main()
