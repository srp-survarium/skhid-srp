#!/usr/bin/env python3
"""The review pages of a weapon-modules build (docs/notes/attachments.md, docs/notes/remodel-069.md).

usage: python3 -I review.py <bundle>/modules-069d.json <renders folder> <out folder>

<renders folder>  port-modules --renders: <0.69d0 item>/_look.png (today's gun beside its remodel) and
                  <live item>/<module>.png (a fit as 0.69d0 places it beside ours)
Writes into <out folder> (private/renders/attachments): index.html (the numbers: guns, fits, the
check against 0.69d0, every failure and refusal), look.html (every gun's look against today's, the
most changed first), guns/<item>.html (each fit's pictures and numbers); the pictures are copied
beside them.
"""
import html
import json
import shutil
import sys
from pathlib import Path

STYLE = '''<style>
body{font:14px/1.4 sans-serif;background:#18181c;color:#ddd;margin:20px}
a{color:#8cf} table{border-collapse:collapse} td,th{border:1px solid #444;padding:3px 8px;text-align:left}
img{max-width:100%;background:#18181c} .fail{color:#f77} .ok{color:#8d8} h2{margin-top:28px}
.pair{margin:14px 0} .cap{color:#aaa}
</style>'''
LEGEND = ('<p class="cap">Left: 0.69d0 (its own base model, each module at its own locator). Right: ours (the bare body '
          'and what the table places on it). Top row from the side, bottom row from above. Grey: the gun; '
          'pale: its own parts; blue: a carrier; orange: the fitted module.</p>')


def page(title, body):
    return f'<!doctype html><meta charset="utf-8"><title>{html.escape(title)}</title>{STYLE}<h1>{html.escape(title)}</h1>{body}'


def box(a):
    if not a['share']:
        return '-'
    lo, hi = a['min'], a['max']
    return f"{a['share'] * 100:.1f} % (z {lo[2] * 100:.0f}..{hi[2] * 100:.0f} cm, y {lo[1] * 100:.0f}..{hi[1] * 100:.0f} cm)"


def main():
    report = json.loads(Path(sys.argv[1]).read_text())
    renders, out = Path(sys.argv[2]), Path(sys.argv[3])
    (out / 'guns').mkdir(parents=True, exist_ok=True)
    checks = report['checks']
    fails = [c for c in checks if not c['pass']]
    by_gun = {}
    for c in checks:
        by_gun.setdefault(c['gun'], {}).setdefault(c['fit'], []).append(c)
    items = report['items']  # live item -> [dict id, 0.69d0 item]
    kinds = {}
    for g in report['guns'].values():
        for f in g['plan']['fits']:
            key = (f['kind'], 'through carriers' if f['chain'] else 'direct')
            kinds[key] = kinds.get(key, 0) + 1
    body = (f"<p>{len(report['guns'])} guns of 0.69d0 for {len(items)} items, {report['modules']} module rows, a table "
            f"of {report['table_bytes']} bytes. {len(checks)} comparisons with 0.69d0, limit 0.5 mm: "
            f"<span class=\"{'fail' if fails else 'ok'}\">{len(fails)} failed</span>. "
            '<a href="look.html">Every gun against today\'s</a>.</p>')
    body += '<h2>Fits (0.69d0 guns)</h2><table><tr><th>kind</th><th>how</th><th>fits</th></tr>'
    body += ''.join(f'<tr><td>{k}</td><td>{h}</td><td>{n}</td></tr>' for (k, h), n in sorted(kinds.items())) + '</table>'
    body += '<h2>Failures</h2>' + (
        '<table><tr><th>gun</th><th>fit</th><th>what</th><th>mm</th><th>note</th></tr>' + ''.join(
            f"<tr><td><a href=\"guns/{c['gun']}.html\">{c['gun']}</a></td><td>{c['fit']}</td><td>{html.escape(c['what'])}</td>"
            f"<td>{c['mm']}</td><td>{html.escape(c['note'])}</td></tr>" for c in fails) + '</table>'
        if fails else '<p class="ok">None.</p>')
    body += '<h2>Refused by 0.69d0\'s own data</h2><table><tr><th>gun</th><th>kind</th><th>module</th><th>why</th></tr>'
    for name, g in sorted(report['guns'].items()):
        for r in g['plan']['refused']:
            body += f"<tr><td>{name}</td><td>{r['kind']}</td><td>{r['module']}</td><td>{html.escape(r['why'])}</td></tr>"
    body += '</table>'
    body += ('<h2>Laid on today\'s mesh</h2><p class="cap">Each suppressor and grip 0.69d0 links to a gun, in a free '
             'slot of the gun itself: one within 1.5 mm (median) is the gun\'s own part today, in both bodies, and no fit; '
             'a second in its slot is the same part in another paint.</p>'
             '<table><tr><th>gun</th><th>module</th><th>slot</th><th>gap mm</th><th></th></tr>')
    for name, g in sorted(report['guns'].items()):
        for laid in g['laid']:
            own = 'the gun\'s own' if laid['built_in'] else 'on the mesh too: no fit' if laid['gap_mm'] <= 1.5 else ''
            body += (f"<tr><td>{name}</td><td>{laid['module']}</td><td>{laid['slot']}</td><td>{laid['gap_mm']:.2f}</td>"
                     f"<td>{own}</td></tr>")
    body += '</table>'
    if report['suppressor_shapes']:
        body += ('<h2>Suppressors by their shape (no fit)</h2><p class="cap">A muzzle part every surface of which '
                 'draws a suppressor\'s material.</p><ul>' + ''.join(
                     f'<li>{module}: {of}</li>' for module, of in sorted(report['suppressor_shapes'].items())) + '</ul>')
    if report['without_item']:
        body += '<h2>Sold by 0.69d0, no item of ours (ids.tsv)</h2><p>' + ', '.join(report['without_item']) + '</p>'
    body += '<h2>Guns</h2><table><tr><th>item</th><th>0.69d0</th><th>fits</th><th>worst mm</th><th>failed</th></tr>'
    for item, (_, donor) in sorted(items.items()):
        rows = [c for fit in by_gun.get(item, {}).values() for c in fit]
        worst = max((c['mm'] for c in rows), default=0)
        bad = sum(not c['pass'] for c in rows)
        body += (f'<tr><td><a href="guns/{item}.html">{item}</a></td><td>{donor}</td><td>{len(by_gun.get(item, {}))}</td>'
                 f'<td>{worst}</td><td class="{"fail" if bad else "ok"}">{bad}</td></tr>')
    body += '</table>'
    (out / 'index.html').write_text(page('Weapon modules against 0.69d0', body))

    for item, (_, donor) in items.items():
        g = report['guns'][donor]
        body = f'<p><a href="../index.html">index</a> · 0.69d0 item {donor} · fits as {g["fits_like"]}</p>' + LEGEND
        body += '<h2>Own parts</h2><ul>' + ''.join(
            f"<li>{p['module']} in {p['slot']}{' (a fit can displace it: a part of its own)' if p['stock'] else ''}</li>"
            for p in g['plan']['parts']) + '</ul>'
        for fit in g['plan']['fits']:
            rows = by_gun.get(item, {}).get(fit['module'], [])
            chain = ' through ' + ' > '.join(f"{l['module']} in {l['slot']}" for l in fit['chain']) if fit['chain'] else ''
            body += f"<div class=\"pair\"><h2>{fit['kind']}: {fit['module']} in {fit['slot']}{chain}</h2>"
            src = renders / item / f"{fit['module']}.png"
            if src.exists():
                (out / 'guns' / item).mkdir(exist_ok=True)
                shutil.copyfile(src, out / 'guns' / item / src.name)
                body += f'<img src="{item}/{src.name}" loading="lazy">'
            body += '<table><tr><th>what</th><th>mm</th><th></th></tr>' + ''.join(
                f"<tr><td>{html.escape(c['what'])}</td><td>{c['mm']}</td>"
                f"<td class=\"{'ok' if c['pass'] else 'fail'}\">{'ok' if c['pass'] else html.escape(c['note'] or 'over 0.5 mm')}</td></tr>"
                for c in rows) + '</table></div>'
        (out / 'guns' / f'{item}.html').write_text(page(item, body))

    def change(g):
        look = g.get('look') or {}
        return max((look.get(k) or {}).get('share', 0) for k in ('live_only', 'remodel_only')) if look else 0
    body = ('<p><a href="index.html">index</a></p><p class="cap">Left: the gun as players have it today. Right: its 0.69d0 '
            'remodel (the whole body, drawn when nothing is fitted). Each surface in its texture\'s mean colour. '
            '"Only today\'s" and "only the remodel\'s": the share of that body\'s vertices more than 5 mm from the other, '
            'and where they are (z along the barrel, y up). The most changed first.</p>')
    for donor, g in sorted(report['guns'].items(), key=lambda kv: -change(kv[1])):
        live = [i for i, (_, d) in items.items() if d == donor]
        fit, look = g.get('fit'), g.get('look')
        body += f"<div class=\"pair\"><h2>{', '.join(live) or donor}</h2>"
        if fit and look:
            shift = [round(c * 1000, 1) for c in fit['shift']]
            body += (f"<p>Shift onto today's mesh {shift} mm (median gap {fit['gap_before_mm']:.1f} to "
                     f"{fit['gap_after_mm']:.1f} mm). Only today's: {box(look['live_only'])}. "
                     f"Only the remodel's: {box(look['remodel_only'])}.</p>")
        src = renders / donor / '_look.png'
        if src.exists():
            shutil.copyfile(src, out / f'look-{donor}.png')
            body += f'<img src="look-{donor}.png" loading="lazy">'
        body += '</div>'
    (out / 'look.html').write_text(page("Every gun against today's", body))
    (out / 'README.txt').write_text(
        'Weapon attachments: review pages of a weapon-modules build (srp-026g/scripts/weapon-modules/review.py).\n'
        'index.html  the numbers and every failure of the check against 0.69d0\n'
        'look.html   every gun as it is today (left) and its 0.69d0 remodel (right), the most changed first\n'
        'guns/       a page a gun: each fit as 0.69d0 places it (left) and as ours draws it (right)\n')
    print(f'{len(items)} gun pages, {len(fails)} failures')


if __name__ == '__main__':
    main()
