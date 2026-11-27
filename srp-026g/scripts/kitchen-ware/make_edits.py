#!/usr/bin/env python3
"""Author kitchen-ware-1.json: the maps' small kitchen things are shot off.

The owner (2026-11-27, "shoot the can"): School's tin cans, the teapots of
Mamayev Kurgan and the Londons, Rudnya's wash-stand tanks, v0.11 Vostok's
can, teapot and pan jump off when shot or caught in a blast, tumble, and lie
where they rest until a later shot kicks them on; Rudnya's clay jugs and pot
shatter as School's bottles. Each is a row of its project's lamp table (the
client's patch 81, docs/notes/breakable_lamps.md):

- a whole thing (`ware<k>`): its bounds (and its base, the model's origin);
  its visual drawn while intact; its loose piece pack kitchen-ware@1's
  srp_<name>_loose placed at the model's centre, of kind 'ware' with its
  capsule (make_pack.shape: half, radius, axis); its break the bullet's hit
  of its own material (a tin's metal);
- a jug or a pot (`ceramic<k>`): the same, its loose pieces its eight shards
  (srp_<name>_shard<j>, kind 'shard', each at its centre), its break
  bullet x glass (the game's own shards and crash: ceramic has none).

Their collision objects are taken out, as the bottles' (a thing gone from
its table would leave its collision there); the break needs none. --projects
holds `projects/<level>/<project>/client_project` as the manifest leaves them
before this list; --stock the models' files (make_pack.py's) and
game_materials/game.materials. Indices are those of the final render_visuals
(build-content's lamp check refuses a build where a later list moved them).
"""
import argparse
import importlib.util
import json
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS / 'breakable-lamps'))
import fixtures  # noqa: E402
import make_edits as lamps  # noqa: E402

spec = importlib.util.spec_from_file_location('kitchen_ware', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pack)


def material_of(stock, name):
    """A model's collision material id (its settings' first game material)."""
    settings = pack.parse(bytearray((stock / pack.model_dir(name) / 'settings').read_bytes()))
    (_, table) = dict(settings[1])['game_material_settings']
    (_, first) = table[0][1]
    return dict(first)['game_material_id'][1]


def live_material(stock, name):
    materials = lamps.tree((stock / 'game_materials/game.materials').read_bytes())['materials']
    ids = [m['id'] for m in materials if m['name'] == name and not m.get('deleted')]
    if len(ids) != 1:
        raise ValueError(f'expected one live {name} material')
    return ids[0]


class Models:
    """What the rows need of each model, read once from --stock."""

    def __init__(self, stock):
        hashes = json.loads(Path(__file__).with_name('stock.json').read_text())
        self.glass, self.bullet = live_material(stock, 'glass'), live_material(stock, 'bullet')
        self.ware = {}
        for name in pack.WARE:
            originals = pack.originals_of(stock, name, hashes)
            self.ware[name] = (pack.centre(originals), pack.shape(originals), material_of(stock, name),
                               self.bounds(originals))
        self.ceramic = {}
        for name in pack.CERAMIC:
            self.ceramic[name] = (pack.shard_centres(stock, name, hashes),
                                  self.bounds(pack.originals_of(stock, name, hashes)))

    @staticmethod
    def bounds(originals):
        b = originals['render/export_properties']
        return [lamps.c.value_of(b, lamps.c.find(b, 'bounding_box/' + key)) for key in ('min', 'max')]


def placed_bounds(v, bounds):
    """A thing's world bounds, its base (its origin, under some meshes) inside."""
    lo, hi = lamps.world_bounds(v, *bounds)
    return [min(a, p) for a, p in zip(lo, v['position'])], [max(a, p) for a, p in zip(hi, v['position'])]


def rows_of(path, data, models):
    root = lamps.tree(data)
    visuals = root.get('render_visuals', [])
    things = [(i, v) for i, v in enumerate(visuals)
              if lamps.model_name(v) in models.ware or lamps.model_name(v) in models.ceramic]
    if not things:
        return None
    table = fixtures.table_of(path, root)
    placer = fixtures.Placer(path, visuals)
    rows, counts = {}, {'ware': 0, 'ceramic': 0}
    for index, v in things:
        name = lamps.model_name(v)
        if name in models.ware:
            if any(abs(s - 1) > 1e-3 for s in v['scale']):
                raise ValueError(f'{path}: a {name} at scale {v["scale"]}: its body is the unscaled model\'s')
            centre, (half, radius, axis), material, bounds = models.ware[name]
            at = lamps.world_bounds(v, centre, centre)[0]
            loose = placer.place(v, pack.loose_name(name), at)
            lo, hi = placed_bounds(v, bounds)
            key = f"ware{counts['ware']}"
            counts['ware'] += 1
            rows[key] = fixtures.row(lo, hi, at, at, break_material=material, effect_material=models.bullet,
                                     leftover='glass', visuals={'intact0': {'index': index, 'state': 0}}, lights={},
                                     loose={'piece0': {'index': loose, 'kind': 'ware', 'half': half, 'radius': radius,
                                                       'axis': axis}})
        else:
            centres, bounds = models.ceramic[name]
            if any(abs(s - pack.SCALE.get(name, 1.0)) > 1e-3 for s in v['scale']):
                raise ValueError(f'{path}: a {name} at scale {v["scale"]}, its shards cut at {pack.SCALE.get(name, 1.0)}')
            rigid = dict(v, scale=[1.0, 1.0, 1.0])
            loose = {}
            for j, (shard, centre) in enumerate(zip(pack.shard_names(name), centres)):
                at = lamps.world_bounds(rigid, centre, centre)[0]
                loose[f'shard{j}'] = {'index': placer.place(v, shard, at, [1.0, 1.0, 1.0]), 'kind': 'shard'}
            lo, hi = placed_bounds(v, bounds)
            middle = lamps.world_bounds(v, *bounds)
            middle = [(a + b) / 2 for a, b in zip(*middle)]
            key = f"ceramic{counts['ceramic']}"
            counts['ceramic'] += 1
            rows[key] = fixtures.row(lo, hi, middle, middle, break_material=models.glass,
                                     effect_material=models.bullet, leftover='glass',
                                     visuals={'intact0': {'index': index, 'state': 0}}, lights={}, loose=loose)
    fixtures.check_keys(path, table, rows)
    return placer.entries, rows, fixtures.collisions(path, root, set(models.ware) | set(models.ceramic))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    models = Models(args.stock)
    placed, found, removed, every = {}, {}, [], []
    for path, data in fixtures.projects(args.projects):
        every.append(path)
        made = rows_of(path, data, models)
        if made is None:
            continue
        placed[path] = made[0]
        found[path] = made[1]
        removed += made[2]
        print(f'{path}: {", ".join(sorted(made[1]))}')
    edits = {'_about': "The maps' small kitchen things are shot off (the owner, 2026-11-27, \"shoot the can\"): "
             "School's tin cans, the teapots, Rudnya's wash-stand tanks, v0.11 Vostok's can, teapot and pan jump off "
             "whole (kind ware), Rudnya's clay jugs and pot shatter into eight shards; each a row of its project's "
             'lamp table, its loose pieces pack kitchen-ware@1\'s placed after the project\'s last visual, their '
             'collision objects out (docs/notes/breakable_lamps.md). Authored by scripts/kitchen-ware/make_edits.py '
             'over the projects as the manifest leaves them; indices refer to the final render_visuals.',
             'add_entry': fixtures.compact(placed, every), 'add_value': fixtures.grouped(found), 'remove_entry': removed}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
