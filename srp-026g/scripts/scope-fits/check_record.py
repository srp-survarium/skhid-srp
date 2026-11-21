#!/usr/bin/env python3
"""The scopes of a weapon-modules build, checked from its record (docs/notes/attachments-scopes.md).

usage: python3 -I check_record.py <modules-069d.json> [--guns-built] [--tsv <file>]

<modules-069d.json>  the record resource-porter's port-modules wrote beside the bundle
--guns-built         a quick build of some guns: only the released pairs of the guns it built are asked for
--tsv                also writes a row a scope fit: gun, optic, the reticle's surface, how far its middle
                     is off the eye's line at the start, middle and end of the gun's aimed idle clip
                     (mrad, and pixels at 1280x720 for the optic's zoom), how far before the eye it is

It fails on:
- a released pair (released-pairs.tsv) that is no scope fit of its gun in the record: neither in
  0.69d0's plan nor a legacy row; and any pair the build itself lists as lost;
- a legacy row whose optic the written table puts more than 0.5 mm from where its variant had it;
- a scope fit without its reticle's row, or whose reticle is off the eye's line at rest (the clip's
  start is the pose the eye is anchored in, so this is 0 by construction: it proves the arithmetic
  of the records, through the game's two formulas, not the game);
- an optic whose reticle leaves the eye's line by more than a pixel through the aimed idle clip
  (the gun's clip moves the optic's bone against the bone its own `aim` hangs on).
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
# One pixel at 1280x720 and uro_fov 65 (11.1 px a degree), in mrad, at zoom x1.
PIXEL_MRAD = 1000 * 3.14159265 / 180 / 11.1
AT_REST = 0.05      # mrad
LEGACY_MM = 0.5


def main():
    record = json.loads(Path(sys.argv[1]).read_text())
    built_only = '--guns-built' in sys.argv
    tsv = Path(sys.argv[sys.argv.index('--tsv') + 1]) if '--tsv' in sys.argv else None
    ids = {}
    for line in (HERE.parent / 'weapon-modules/ids.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            module, dict_id, kind, _ = line.split('\t')
            if kind == 'scope':
                ids[int(dict_id)] = module
    donor_of = {live: donor for live, (_, donor) in record['items'].items()}
    fits = {}       # live gun -> the optics it takes
    for live, donor in donor_of.items():
        gun = record['guns'][donor]
        like = record['guns'].get(gun['fits_like'], gun)
        own = {f['module'] for f in gun['plan']['fits'] if f['kind'] == 'scope'}
        fits[live] = own or {f['module'] for f in like['plan']['fits'] if f['kind'] == 'scope'}
    for row in record['legacy']:
        fits.setdefault(row['gun'], set()).add(row['module'])
    problems = []
    for lost in record['lost']:
        problems.append(f"lost by the build: {lost['gun']} + scope {lost['scope']}: {lost['why']}")
    released = 0
    for line in (HERE / 'released-pairs.tsv').read_text().splitlines():
        if not line or line.startswith('#'):
            continue
        weapon, _, scope, scope_id, _, _ = line.split('\t')
        if built_only and weapon not in donor_of:
            continue
        released += 1
        if ids.get(int(scope_id)) not in fits.get(weapon, set()):
            problems.append(f'a released pair is no fit: {weapon} + {scope}')
    for row in record['legacy']:
        if row['optic_mm'] > LEGACY_MM:
            problems.append(f"legacy {row['gun']} + {row['module']}: the optic {row['optic_mm']} mm from its variant's")
    zoom = {}       # by optic: 1 / fov_factor is not in the record; 0.69d0's group names the zoom
    for module, optic in record['optics'].items():
        group = optic['group']
        zoom[module] = float(group[len('scope_'):-1]) if group.startswith('scope_') and group.endswith('x') else 1.0
    seen = {(c['gun'], c['fit']): c for c in record['centres']}
    worst = (0.0, None)
    rows = []
    for gun, optics in sorted(fits.items()):
        for module in sorted(optics):
            c = seen.get((gun, module))
            if c is None:
                problems.append(f'no reticle row: {gun} + {module}')
                continue
            pixels = [m * zoom.get(module, 1.0) / PIXEL_MRAD for m in c['off_mrad']]
            rows.append((gun, module, c['surface'], *c['off_mrad'], *(round(p, 2) for p in pixels), c['ahead'],
                         c['eye_behind_optic']))
            if c['off_mrad'][0] > AT_REST:
                problems.append(f"{gun} + {module}: the reticle {c['off_mrad'][0]} mrad off the eye's line at rest")
            if max(pixels) > 1.0:
                problems.append(f'{gun} + {module}: the reticle leaves the centre by {max(pixels):.2f} px in the aimed idle clip')
            worst = max(worst, (max(pixels), f'{gun} + {module}'))
    if tsv:
        head = 'gun\toptic\treticle\tmrad start\tmrad middle\tmrad end\tpx start\tpx middle\tpx end\tahead m\teye behind optic m\n'
        tsv.write_text(head + ''.join('\t'.join(map(str, r)) + '\n' for r in rows))
    print(f"{len(rows)} scope fits on {sum(1 for o in fits.values() if o)} guns; {released} released pairs asked for; "
          f"{len(record['legacy'])} legacy rows; the reticle furthest from the centre in an aimed idle clip: "
          f"{worst[0]:.2f} px ({worst[1]})")
    for problem in problems:
        print('FAIL', problem)
    sys.exit(1 if problems else 0)


if __name__ == '__main__':
    main()
