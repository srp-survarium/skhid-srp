#!/usr/bin/env python3
"""review.py: each texture's masks against its painted versions, and the parts.tsv picks of the better.

usage: python3 -I review.py <game folder> <0.69d0 folder> <out folder> <table.tsv>...

<game folder>    as weapon_camo.py's, with the masks players have (textures/<texture>_aoac.dds)
<0.69d0 folder>  as weapon_camo.py's --v069
<table.tsv>...   the tables to review (../remodel-069/camo.tsv, ../weapons-069/camo.tsv); a premium
                 row takes its plain row's painted versions, so give the tables together
$REVIEW_ONLY     textures to review (comma-separated), the others left out

The owner (2026-11-26): the right mask is where the game's own camouflaged versions paint. For each
texture, part by part (weapon_camo.py's parts): how its painted versions decide it (0.69d0's `_camo`
and premium textures, else its event skins; painted where one changed it: the mean blurred difference,
the largest channel, PAINT and more), the masks players have and weapon_camo.py's rule. Agreement:
the share of the texture's surface, by area, where a mask paints what the painted versions paint and
leaves bare what they leave. The better of the two (the players' masks on a tie), then, under GOOD,
the parts the painted versions decide clearly (SURE_PAINT, SURE_BARE). A texture with no painted
version keeps the players' masks but where the rule is clearly right (a rod bare, large polymer or
wood painted); one players have plain stays plain. Writes <out>/result-<pid>.json (per texture the
scores, the choice and the picks), the masks as PNG (rule/, chosen/, current/ at 1024 texels: the
review's pictures) and <out>/<table>-parts.tsv: the picks over the rule as parts.tsv lines.
"""
import os, sys, json, collections, re
import numpy as np
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import weapon_camo as wc

GOOD = 0.95          # agreement under which a texture's parts are fixed from the reference
PAINT = 0.02        # a part a painted version changed: its texels' mean difference (blurred, the largest channel)
SURE_PAINT, SURE_BARE = 0.03, 0.008   # untouched parts measure 0.005 and less (block compression)

def changed(plain, painted):
    sm = lambda img: np.stack([wc.blur(img[..., k], 2) for k in range(3)], 2)
    return np.abs(sm(plain) - sm(painted)).max(axis=2)
game_dir, v069_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]
tables = sys.argv[4:]
only = os.environ.get('REVIEW_ONLY')
game, v069 = wc.Files(game_dir), wc.Files(v069_dir)
sources = {'69': v069}

def tex(name, size=None):
    files = game
    if name[2:3] == ':':
        files, name = sources[name[:2]], name[3:]
    img = wc.load_rgb(files.read('textures/%s.dds' % name))
    return wc.resize(img, size) if size else img

rows = {}
for t in tables:
    for r in wc.read_rows(t):
        rows[r.texture] = ('/'.join(os.path.abspath(t).split(os.sep)[-2:]), r)
# a painted version that is itself a row (a premium texture): its plain row's references
plain_of = {}
for name, (_, r) in rows.items():
    for painted, plain in r.painted:
        p, q = painted.split(':', 1)[-1], plain.split(':', 1)[-1]
        if p != q and p in rows and q in rows and p != name:
            plain_of.setdefault(p, q)

def picks_lines(points, chosen, rule):
    add = sorted(set(chosen) - set(rule)); drop = sorted(set(rule) - set(chosen))
    return ['+%.4f,%.4f' % points[i] for i in add] + ['-%.4f,%.4f' % points[i] for i in drop]

os.makedirs(out, exist_ok=True)
for v in ('rule', 'chosen', 'current'):
    os.makedirs(os.path.join(out, v), exist_ok=True)
result = {}
for name, (table, row) in sorted(rows.items()):
    if only and name not in only.split(','):
        continue
    base = tex(name)
    size = base.shape[:2]
    meshes, spans = [], []
    for s in row.surfaces:
        m, surf = s.split(':')
        meshes.append(game.mesh(m, surf)); spans.append(game.span(m))
    tri = wc.island_triangles(meshes)
    labels = wc.islands(meshes, size, tri)
    parts = wc.parts_of(meshes, labels, tri, spans)
    metal = wc.shares(labels, tex(row.metal, size)[..., 0]) if row.metal else {}
    # references: a premium row takes its plain row's painted versions, measured against that plain texture
    ref_row, note = row, ''
    if name in plain_of:
        ref_row = rows[plain_of[name]][1]; note = 'premium of ' + plain_of[name]
    painted = [(p, q) for p, q in ref_row.painted]
    if name not in plain_of and painted and all(q.split(':', 1)[-1] == name for _, q in painted) \
            and re.search(r'camo|prem|_st$|_st_', name.rsplit('/', 1)[-1]):
        painted, note = [], 'premium texture, no plain found: no reference'
    if name in plain_of:
        painted = [(p, '69:' + plain_of[name]) if p.split(':', 1)[-1] != plain_of[name] else (p, q) for p, q in painted]
        # the row's own texture is a painted version of that plain one, too
        if not any(p.split(':', 1)[-1] == name for p, _ in painted):
            painted.append((name, plain_of[name]))
    camo_like = [(p, q) for p, q in painted if re.search(r'camo|prem|_st$|_st_', p.rsplit('/', 1)[-1])]
    kind = 'camo' if camo_like else ('event' if painted else 'none')
    if camo_like:
        painted = camo_like
    ref = {}
    for p, q in painted:
        try:
            sh = wc.shares(labels, changed(tex(q, size), tex(p, size)))
        except FileNotFoundError:
            continue
        for i, v in sh.items():
            ref[i] = max(ref.get(i, 0.0), v)
    has_ref = bool(ref)
    repaint = {}                                   # the rule as weapon_camo.py computes it: the row's own painted versions
    for p, q in row.painted:
        for i, v in wc.shares(labels, wc.repainted(tex(q, size), tex(p, size))).items():
            repaint[i] = max(repaint.get(i, 0.0), v)
    rule = set(wc.choose(parts, metal, repaint))
    cur_path = game.path('textures/%s_aoac.dds' % name)
    cur_a = wc.resize(wc.load_rgb(open(cur_path, 'rb').read()), size) if cur_path else None
    cur_share = wc.shares(labels, (cur_a[..., 1] < 0.5).astype(np.float32)) if cur_a is not None else {}
    cur = {i for i, v in cur_share.items() if v >= 0.5}
    total = sum(p.area for p in parts.values()) or 1e-9
    def score(sel):
        if not has_ref:
            return None
        return sum(p.area for i, p in parts.items() if (i in sel) == (ref.get(i, 0) >= PAINT)) / total
    s_cur, s_rule = score(cur), score(rule)
    fixes = []
    if has_ref:
        base_name, sel = ('rule', set(rule)) if s_rule > s_cur + 0.005 else ('current', set(cur))
        if max(s_cur, s_rule) < GOOD:
            for i, p in parts.items():
                if p.area < wc.TINY:
                    continue
                r = ref.get(i, 0.0)
                if r >= SURE_PAINT and i not in sel:
                    sel.add(i); fixes.append(('+', i))
                elif r <= SURE_BARE and i in sel:
                    sel.discard(i); fixes.append(('-', i))
    else:
        base_name, sel = 'current', set(cur)
        for i, p in (parts.items() if cur else ()):   # a texture left plain on purpose (cartridges) stays so       # the rule where it is clearly right: barrels bare, large furniture painted
            rod = p.tube is not None and p.along >= wc.FROM and p.tube < wc.ROD and not (metal and metal.get(i, 1) < wc.METAL)
            furniture = metal and metal.get(i, 1.0) < wc.METAL and p.area >= wc.BODY
            if rod and i in sel:
                sel.discard(i); fixes.append(('-', i))
            elif furniture and i not in sel:
                sel.add(i); fixes.append(('+', i))
    pts = wc.island_points(labels)
    lines = picks_lines(pts, sel, rule)
    a_rule = wc.aoac(base, np.isin(labels, sorted(rule)), labels)
    a_ch = wc.aoac(base, np.isin(labels, sorted(sel)), labels)
    small = lambda a: Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).resize((1024, 1024), Image.LANCZOS)
    key = name.replace('/', '__')
    small(a_rule).save(os.path.join(out, 'rule', key + '.png'))
    small(a_ch).save(os.path.join(out, 'chosen', key + '.png'))
    if cur_a is not None:
        small(cur_a).save(os.path.join(out, 'current', key + '.png'))
    result[name] = dict(table=table, note=note, kind=kind, area=total, has_ref=has_ref, base=base_name,
                        score_current=s_cur, score_rule=s_rule, score_chosen=score(sel), fixes=len(fixes),
                        painted_share={k: sum(parts[i].area for i in v if i in parts) / total
                                       for k, v in (('current', cur), ('rule', rule), ('chosen', sel),
                                                    ('reference', {i for i, x in ref.items() if x >= PAINT}))},
                        picks=lines, empty=not sel, rule_empty=not rule,
                        refs=[p for p, _ in painted])
    print('%-62s ref %-5s cur %5s rule %5s -> %-7s %s fixes %d chosen %5s' % (
        name, has_ref, '%.2f' % s_cur if s_cur is not None else '-', '%.2f' % s_rule if s_rule is not None else '-',
        base_name, note[:20], len(fixes), '%.2f' % score(sel) if has_ref else '-'), flush=True)
json.dump(result, open(os.path.join(out, 'result-%d.json' % os.getpid()), 'w'), indent=1)

for table in sorted({r['table'] for r in result.values()}):
    R = {t: r for t, r in result.items() if r['table'] == table}
    out_lines = []
    for t, r in sorted(R.items()):
        if not r['picks'] and not r['empty']:
            continue                                   # the rule as it is
        if r['empty'] and r['rule_empty']:
            why = 'plain (nothing painted)'
        else:
            why = {'rule': 'the rule', 'current': 'the parts the live mask paints'}[r['base']]
            if r['fixes']:
                why += ', %d part%s as the painted version' % (r['fixes'], 's' if r['fixes'] > 1 else '')
            if r['kind'] == 'none' and r['fixes']:
                why = 'the parts the live mask paints, %d by the rule (a rod bare, large furniture painted)' % r['fixes']
        out_lines.append('# %s' % why)
        out_lines.append('%s\t%s' % (t, ' '.join(r['picks']) if not r['empty'] else ''))
    open(os.path.join(out, '%s-parts.tsv' % table.replace('/', '__')), 'w').write('\n'.join(out_lines) + '\n')
