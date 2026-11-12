"""The note's plain-ASCII tables from `run` and `stats` results.

    python -m predfit.report results.json [stats.json]
"""
import json
import sys

LABELS = ['real', '50', '100', '150', '250']


def table(header, rows):
    widths = [max(len(str(r[i])) for r in [header] + rows) for i in range(len(header))]
    line = lambda r: '  '.join(str(c).rjust(w) if i else str(c).ljust(w)
                               for i, (c, w) in enumerate(zip(r, widths)))
    return '\n'.join([line(header), '  '.join('-' * w for w in widths)] + [line(r) for r in rows])


def models_table(res, metric, stat, subset='all', names=None, fmt='{:.2f}'):
    rows = []
    for name, row in res['models'].items():
        if names and name not in names:
            continue
        rows.append([name] + [fmt.format(row[l][subset][metric][stat]) if subset in row[l] else '-'
                              for l in LABELS])
    return table(['model'] + ['realistic' if l == 'real' else f'{l} ms' for l in LABELS], rows)


def growth(res, prefix):
    rows, prev = [], None
    base = res['models']['hold-movement']['real']['all']
    for name, row in res['models'].items():
        if not name.startswith(prefix) or ' + ' in name:
            continue
        r = row['real']['all']
        pos, look = r['pos_cm']['mean'], r['look_deg']['mean']
        value = pos if prefix.startswith('move') else look
        ref = base['pos_cm']['mean'] if prefix.startswith('move') else base['look_deg']['mean']
        gain = 100 * (ref - value) / ref
        step = '-' if prev is None else f'{100 * (prev - value) / ref:+.1f}%'
        r250 = row['250']['all']
        rows.append([name[len(prefix):].strip('[]'), f'{pos:.3f}', f'{look:.3f}',
                     f"{r250['pos_cm']['mean']:.2f}", f"{r250['look_deg']['mean']:.2f}",
                     f'{gain:+.1f}%', step, row['params'].get('cells', '-'),
                     row['params'].get('filled_cells', '-'), row['params'].get('bytes', '-')])
        prev = value
    return table(['key', 'pos cm', 'look deg', 'pos 250', 'look 250', 'vs hold', 'step',
                  'cells', 'filled', 'bytes'], rows)


def main():
    res = json.load(open(sys.argv[1]))
    print(f"train {res['train_matches']} matches ({res['train_samples']} samples), "
          f"test {res['test_matches']} matches\n")
    print('Summary at the realistic delay (mean; p99 for position):')
    rows = []
    for name, row in res['models'].items():
        r = row['real']
        cell = lambda sub, m, st='mean', f='{:.2f}': f.format(r[sub][m][st]) if sub in r else '-'
        rows.append([name, cell('all', 'pos_cm'), cell('all', 'pos_cm', 'p99'),
                     cell('all', 'look_deg'), cell('combat', 'pos_cm'), cell('combat', 'look_deg'),
                     cell('pvp', 'pos_cm'), cell('pvp', 'look_deg'),
                     cell('pvp_combat', 'pos_cm'), cell('pvp_combat', 'look_deg')])
    print(table(['model', 'pos cm', 'pos p99', 'look deg', 'cmb pos', 'cmb look', 'pvp pos',
                 'pvp look', 'pvp cmb pos', 'pvp cmb look'], rows))
    print('\nPosition error at t, mean cm (all alive time):')
    print(models_table(res, 'pos_cm', 'mean'))
    print('\nPosition error at t, p90 cm:')
    print(models_table(res, 'pos_cm', 'p90'))
    print('\nLook error at t, mean degrees:')
    print(models_table(res, 'look_deg', 'mean'))
    print('\nCombat (own shot within 3 s): position mean cm:')
    print(models_table(res, 'pos_cm', 'mean', 'combat'))
    print('\nCombat: look mean degrees:')
    print(models_table(res, 'look_deg', 'mean', 'combat'))
    print('\nAction ticks wrong (share of predicted ticks):')
    rows = [[n] + [f"{r[l]['all']['action_share']:.3f}" for l in LABELS]
            for n, r in res['models'].items()]
    print(table(['model'] + LABELS, rows))
    print('\nMove tables, grown one feature at a time (realistic delay):')
    print(growth(res, 'move-table'))
    print('\nLook tables, grown one feature at a time (realistic delay):')
    print(growth(res, 'look-table'))
    print('\nProjected look (one input after the newest node, no delay known), grown:')
    print(growth(res, 'projected-look'))
    if len(sys.argv) > 2:
        print('\n' + json.dumps(json.load(open(sys.argv[2])), indent=1))


if __name__ == '__main__':
    main()
