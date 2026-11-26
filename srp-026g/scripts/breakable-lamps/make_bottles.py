#!/usr/bin/env python3
"""Author breakable-bottles-1.json: School's vodka bottles break as glass.

The owner (2026-11-26, School's table at 17.0, 19.5, 33.7: "there are
bottles in school which can be breakable"). Each `props/kitchen/bottle_vodka`
of a project that has a lamp table becomes a fixture of that table, as a lit
tube or a kerosene lamp's glass is: its one intact visual, no broken model,
no lights. A bullet through its bounds, or a blast that reaches it, hides it
and plays the project's glass pair (bullet x glass, the bottle's own
collision material) along the bottle; the history keeps it broken.

Bottles stay out of make_edits.py's lamp rule (`PARTS`): that rule merges
touching models into one fixture, and three bottles lie within 0.3 m of one
another, so each is a row of its own here, `bottle<k>` in the order of the
project's visuals.

--projects holds `projects/<level>/<project>/client_project` as the manifest
leaves them before this list (a quick build's patch over players' files);
--stock holds each bottle model's `models/.../render/export_properties`
(stock's, and the v0.21 port's own copy, from its pack). The lobby is
skipped: its two bottles stand on a table with no lamp table and no one
shooting. The visual indices are those of the final render_visuals:
build-content's lamp check (make_edits.check_built) refuses a build where a
later list moved them.
"""
import argparse
import json
from pathlib import Path

import make_edits as lamps

BOTTLE = lamps.BOTTLES


def rows(path, data, bounds):
    """A project's bottle rows by key, or None when it has no bottle."""
    root = lamps.tree(data)
    visuals = root.get('render_visuals', [])
    bottles = [(i, v) for i, v in enumerate(visuals) if v.get('visual_name') in BOTTLE]
    if not bottles:
        return None
    table = root.get('srp_lamps')
    if table is None:
        raise ValueError(f'{path}: bottles but no lamp table to add them to')
    found = {}
    for k, (i, v) in enumerate(bottles):
        key = f'bottle{k}'
        if key in table['lamps']:
            raise ValueError(f'{path}: {key} is already a fixture')
        lo, hi = lamps.world_bounds(v, *bounds[v['visual_name']])
        # The model's origin is its base, 5 mm under the mesh (its bounds' min y 0.005):
        # the box holds it too, as check_built reads a fixture's placement inside its box.
        lo, hi = [min(a, p) for a, p in zip(lo, v['position'])], [max(a, p) for a, p in zip(hi, v['position'])]
        start, end = lamps.glass_line([(i, v)], bounds)
        row = dict(zip(('min_x', 'min_y', 'min_z', 'max_x', 'max_y', 'max_z'), lo + hi))
        row |= dict(zip(('glass_from_x', 'glass_from_y', 'glass_from_z', 'glass_to_x', 'glass_to_y', 'glass_to_z'),
                        start + end))
        found[key] = dict(row, visuals={'intact0': {'index': i, 'state': 0}}, lights={})
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    bounds = {}

    def bounds_of(name):
        if name not in bounds:
            b = (args.stock / f'models/{name}.model/render/export_properties').read_bytes()
            bounds[name] = [lamps.c.value_of(b, lamps.c.find(b, 'bounding_box/' + key)) for key in ('min', 'max')]
        return bounds[name]
    # One add_value a row, its files every project that has that row alike.
    alike = {}
    for p in sorted((args.projects / 'projects').glob('*/*/client_project')):
        if p.parts[-3] == 'lobby_scene':   # the lobby's table holds two bottles, no lamps, nothing to shoot
            continue
        path = str(p.relative_to(args.projects))
        data = p.read_bytes()
        for v in lamps.tree(data).get('render_visuals', []):
            if v.get('visual_name') in BOTTLE:
                bounds_of(v['visual_name'])
        found = rows(path, data, bounds)
        if found is None:
            continue
        for key, row in found.items():
            alike.setdefault(json.dumps([key, row], sort_keys=True), []).append(path)
        print(f'{path}: {len(found)} bottles')
    edits = {'_about': "Breakable bottles (the owner, 2026-11-26: School's vodka bottles): each "
             'props/kitchen/bottle_vodka of a project with a lamp table is a fixture of it with its one intact '
             "visual, no broken model and no lights; a break hides it and plays the project's glass pair "
             '(docs/notes/breakable_lamps.md). Authored by scripts/breakable-lamps/make_bottles.py over the '
             'projects as the manifest leaves them; indices refer to the final render_visuals.',
             'add_value': [{'files': files, 'table': 'srp_lamps/lamps', 'key': key, 'value': row}
                           for spec, files in sorted(alike.items(), key=lambda kv: (kv[1], kv[0]))
                           for key, row in [json.loads(spec)]]}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
