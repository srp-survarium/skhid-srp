#!/usr/bin/env python3
"""Check weapon-variants against its pinned inputs.

usage: python3 check_pack.py <pack folder> <prepare_inputs.py's folder>

- every skin is its gun: the config the gun's as the edit lists start from but for the model, the
  drop weight (0), the icon, the name and srp_premium (the gun, the silver bonus); its dictionary row
  a premium (gold) row of the gun's category; its items_compatibility rows the gun's;
- every edit list the release applies (release/content.tsv's `edit` rows made here) names a skin's
  config as often as its gun's, so the skin takes every change its gun takes;
- the model is the gun's with only the materials renamed (the TOZ-122 "Leather": the TOZ-122's
  files but for its mesh and the wrap); a material is the gun's with only its diffuse changed (the
  wrap's: the AKM's with its four maps), so no shader combination is new; a texture keeps the
  gun's format, size and mips, and its _aoac is the gun's byte for byte;
- the skin's 0.69d0 surfaces share the gun's UVs (make_pack.match: at least 98 % of the shared
  vertices at the same UV);
- each scope fit is the gun's with its ids, model and weapon changed, its model the gun's fit model
  with the skin's materials;
- ids and atlas cells are free in the inputs and the cells empty in the players' lobby movie;
- the lobby movie's mirror (WeaponVariants1.as) is the pack's, and the names are names.tsv's;
- paint.py on the AKM and SKS "camo_st" textures both builds ship lands no farther from 0.26g's own
  than 0.69d0's albedo as it is (the AKM's: half as far; the SKS's paint is 0.26g's colour already).
"""
import json
from pathlib import Path
import subprocess
import sys

import numpy as np

import paint
import swf
from common import (CALIBRATION, DICTIONARY, FOLDER, HERE, MOVIE, ROOT, SILVER_BONUS, SYMBOL, WEAPONS, Inputs, bcw,
                    exo_gear, name_key, variants)

ALLOWED = {'object/model', 'parameters/drop_weight', 'ui_desc/icon[0]', 'ui_desc/icon[1]',
           'ui_desc/text_descriptions/name'}
MOVIE_MIRROR = ROOT / 'srp-026g' / 'flash' / 'lobby_menu' / 'src' / 'survarium' / 'constants' / 'WeaponVariants1.as'


def flat(value, path='', out=None):
    out = {} if out is None else out
    if isinstance(value, dict):
        for key in value:
            flat(value[key], f'{path}/{key}' if path else key, out)
    elif isinstance(value, list):
        for i, item in enumerate(value):
            flat(item, f'{path}[{i}]', out)
    else:
        out[path] = value
    return out


class Check:
    def __init__(self):
        self.failed = 0

    def __call__(self, ok, text):
        if not ok:
            self.failed += 1
            print(f'FAIL {text}')


def dds_shape(data):
    return exo_gear().dds_levels(data), data[84:88]


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    pack, inputs = Path(sys.argv[1]), Inputs(sys.argv[2])
    check = Check()
    manifest = json.loads((pack / 'pack.json').read_text())
    files = {}
    for op in manifest['operations']:
        if op['op'] == 'add_file':
            files[op['path']] = (pack / 'files' / op['path']).read_bytes()
    on_disk = {str(p.relative_to(pack / 'files')) for p in (pack / 'files').rglob('*') if p.is_file()}
    check(on_disk == set(files), f'files/ and the add_file operations differ: {sorted(on_disk ^ set(files))[:3]}')
    read = lambda path: files[path] if path in files else inputs.read('compose', path)
    dictionary = inputs.config('compose', DICTIONARY)
    items = {i['name']: i for i in dictionary['items_dict']}
    ids = {i['dict_id'] for i in dictionary['items_dict']}
    (entries,) = [op['entries'] for op in manifest['operations'] if op['op'] == 'dictionary_add']
    (compat,) = [op['values'] for op in manifest['operations'] if op['op'] == 'config_set']
    (cells,) = [op['icons'] for op in manifest['operations'] if op['op'] == 'atlas_icons_add']
    check(not ({e['dict_id'] for e in entries} & ids), 'a dict id the game has')
    check(len({e['dict_id'] for e in entries}) == len(entries), 'a dict id twice')
    check(not ({e['name'] for e in entries} & set(items)), 'a name the game has')
    by_name = {e['name']: e for e in entries}
    rows = variants()
    records = json.loads((pack / 'report.json').read_text())
    report = {r['variant']: r for r in records if 'variant' in r}
    atlas = swf.atlas(inputs.read('players', MOVIE), SYMBOL)
    occupied = swf.occupied(atlas)
    check(len({(c['row'], c['col']) for c in cells}) == len(cells), 'an atlas cell twice')
    for c in cells:
        check(not occupied.get((c['row'], c['col']), False), f"cell {c['row']},{c['col']} is drawn already")
    edits = applied_edit_lists()
    for row in rows:
        gun = items[row.base]
        entry = by_name.get(row.name)
        check(entry and entry['dict_id'] == row.dict_id and entry['is_premium'] and entry['base_cost'] == row.gold
              and entry['item_category'] == gun['item_category'] and not entry['is_stack'],
              f'{row.name}: dictionary row')
        cfg = f'{WEAPONS}/wpn_{row.name}.options'
        check(entry and entry['cfg_name'] == cfg, f'{row.name}: config path')
        mine, theirs = bcw.loads(files[cfg]), inputs.config('compose', gun['cfg_name'])
        premium = mine.pop('srp_premium', None)
        check(premium == {'plain_item': gun['dict_id'], 'bonuses': [SILVER_BONUS]}, f'{row.name}: srp_premium')
        a, b = flat(mine), flat(theirs)
        differ = {k for k in set(a) | set(b) if a.get(k) != b.get(k)}
        check(differ <= ALLOWED, f'{row.name}: differs from its gun at {sorted(differ - ALLOWED)}')
        check(mine['parameters']['drop_weight'] == 0 and list(mine['ui_desc']['icon']) == list(row.icon)
              and mine['ui_desc']['text_descriptions']['name'] == name_key(row), f'{row.name}: skin fields')
        check(not mine.get('default_modifications') and not mine['parameters'].get('base_rarity_level'),
              f'{row.name}: bonuses or rarity of its own')
        theirs_compat = sorted((p['first_item'] == gun['dict_id'], p['second_item'] if p['first_item'] == gun['dict_id']
                                else p['first_item']) for p in dictionary['items_compatibility'].values()
                               if gun['dict_id'] in (p['first_item'], p['second_item']))
        mine_compat = sorted((v['table'].get('first_item') == row.dict_id,
                              v['table']['second_item'] if v['table'].get('first_item') == row.dict_id
                              else v['table']['first_item']) for k, v in compat.items()
                             if row.dict_id in v['table'].values())
        check(theirs_compat == mine_compat, f'{row.name}: items_compatibility differs from its gun\'s')
        for name, text in edits.items():
            n_gun = text.count(f'"{gun["cfg_name"]}"')
            n_skin = text.count(f'"{cfg}"')
            check(n_gun == n_skin, f'{row.name}: {name} names its gun {n_gun} times, it {n_skin}')
        check(any(c['row'] == row.icon[0] and c['col'] == row.icon[1] and c['source'] == f'icons/{row.name}.png'
                  for c in cells), f'{row.name}: icon cell')
        check_model(check, row, mine['object']['model'], theirs['object']['model'], files, read, inputs, report)
    copies = [r for r in records if 'of' in r]
    for r in copies:
        check_fit(check, r, files, read, inputs, by_name, items)
    check_fits_taken(check, rows, manifest, inputs, items, copies)
    # The mirror and the names.
    mirror = (pack / 'ui' / 'WeaponVariants1.as').read_text()
    check(MOVIE_MIRROR.exists() and MOVIE_MIRROR.read_text() == mirror, f'{MOVIE_MIRROR} is not the pack\'s mirror')
    for row in rows:
        check(f'[{row.dict_id},{items[row.base]["dict_id"]},[{SILVER_BONUS}]]' in mirror, f'{row.name}: mirror row')
    names = subprocess.run([sys.executable, str(HERE / 'names.py'), '--inputs', sys.argv[2], '--check'],
                           capture_output=True, text=True)
    check(names.returncode == 0, f'names: {names.stdout}{names.stderr}')
    calibrate(check, inputs)
    print(f'weapon-variants: {len(rows)} skins, {len(copies)} scope fits copied checked; '
          + ('FAILED: %d' % check.failed if check.failed else 'all good'))
    sys.exit(1 if check.failed else 0)


def applied_edit_lists():
    """{file: text} of the edit lists the release applies itself (release/content.tsv `edit` rows with
    no base patch: a base patch's edit is in the configs packs start from)."""
    out = {}
    for line in (ROOT / 'release' / 'content.tsv').read_text().splitlines():
        cols = line.split('\t')
        if cols[0] == 'edit' and cols[2] == '-':
            out[cols[1]] = (ROOT / 'srp-026g' / 'scripts' / 'content-edit' / cols[1]).read_text()
    return out


def check_model(check, row, model, gun_model, files, read, inputs, report):
    prefix, gun_prefix = f'models/{model}.skinned_model/', f'models/{gun_model}.skinned_model/'
    check(model == f'{FOLDER}/{row.name}', f'{row.name}: model path')
    settings, gun_settings = bcw.loads(read(prefix + 'settings')), inputs.config('compose', gun_prefix + 'settings')
    mine = sorted(p[len(prefix):] for p in files if p.startswith(prefix))
    theirs = sorted(p[len(gun_prefix):] for p in inputs.paths('compose', gun_prefix))
    leather = row.name == 'toz_122_premium'
    if leather:
        import mesh as donor_mesh
        extra = {f'render/{donor_mesh.WRAP_SURFACE}/converted_model', f'render/{donor_mesh.WRAP_SURFACE}/export_properties'}
        check(set(mine) == set(theirs) | extra, f'{row.name}: model files')
        (body,) = gun_settings['lod_hierrarchy']['LOD0']['surfaces']
        same = [p for p in theirs if p != 'settings' and not p.startswith(f'render/{body}/')]
    else:
        check(mine == theirs, f'{row.name}: model files differ from the gun\'s')
        same = [p for p in theirs if p != 'settings']
    for p in same:
        check(files.get(prefix + p) == inputs.read('compose', gun_prefix + p), f'{row.name}: {p} is not the gun\'s')
    entries = report[row.name]['surfaces']
    for surface, entry in settings['material_settings'].items():
        material = entry['material_name']
        gun_entry = gun_settings['material_settings'].get(surface)
        if gun_entry is None:
            check(leather, f'{row.name}: surface {surface} the gun lacks')
            check_material(check, row, material, 'weapons/akmn', files, read, inputs, wrap=True)
            continue
        if material == gun_entry['material_name']:
            continue
        check(material.startswith(f'{FOLDER}/{row.name}/'), f'{row.name}: {material} outside its folder')
        check_material(check, row, material, gun_entry['material_name'], files, read, inputs)
    for s in entries:
        if s.get('model'):
            continue
        check(s.get('donor') is None or (s['shared'] >= 0.5 and s['uv_same'] >= 0.98),
              f"{row.name}: {s['surface']} shares too little with 0.69d0's ({s.get('shared')}, {s.get('uv_same')})")
    if not leather:
        check(any(s.get('texture') for s in entries), f'{row.name}: nothing repainted')


def check_material(check, row, material, gun_material, files, read, inputs, wrap=False):
    mine = flat(bcw.loads(read(f'material_instances/{material}.material')))
    theirs = flat(inputs.config('compose', f'material_instances/{gun_material}.material'))
    maps = ({'texture_diffuse', 'texture_normal', 'texture_roughness', 'texture_fresnel'} if wrap
            else {'texture_diffuse'})
    differ = {k for k in set(mine) | set(theirs) if mine.get(k) != theirs.get(k)}
    keys = {f'material/g_stage/effect/{m}/value' for m in maps}
    check(differ <= keys, f'{row.name}: {material} differs from {gun_material} at {sorted(differ - keys)}')
    for key in keys:
        texture, gun_texture = mine[key], theirs[key]
        if texture == gun_texture:
            continue
        data = files.get(f'textures/{texture}.dds')
        check(data is not None and f'textures/{texture}.options' in files, f'{row.name}: {texture} not in the pack')
        if data is None:
            continue
        base = inputs.read('compose', f'textures/{gun_texture}.dds') if inputs.has('compose', f'textures/{gun_texture}.dds') else None
        if base is not None and key.endswith('texture_diffuse/value'):
            check(dds_shape(data) == dds_shape(base) and len(data) == len(base),
                  f'{row.name}: {texture} is not in {gun_texture}\'s format')
        if not wrap:
            aoac = files.get(f'textures/{texture}_aoac.dds')
            check(aoac == inputs.read('players', f'textures/{gun_texture}_aoac.dds'),
                  f'{row.name}: {texture}_aoac is not {gun_texture}_aoac')


def check_fit(check, r, files, read, inputs, by_name, items):
    entry = by_name[r['fit']]
    gun_fit = items[r['of']]
    check(entry['dict_id'] == r['dict_id'] and entry['item_category'] == gun_fit['item_category']
          and entry['base_cost'] == gun_fit['base_cost'] and entry['is_premium'] == gun_fit['is_premium'],
          f"{r['fit']}: dictionary row")
    mine = bcw.loads(files[entry['cfg_name']])
    theirs = inputs.config('compose', gun_fit['cfg_name'])
    fid = r['dict_id']
    check(mine['parameters']['modification'] == 990000 + fid and mine['modifications'][0]['id'] == 990000 + fid,
          f"{r['fit']}: modification ids")
    a, b = flat(mine), flat(theirs)
    differ = {k for k in set(a) | set(b) if a.get(k) != b.get(k) and not k.startswith('srp_scope_fit/weapons')}
    weapons = mine['srp_scope_fit']['weapons']
    check(len(weapons) == 1 and weapons[0] in r['fit'] and weapons[0] in by_name, f"{r['fit']}: names its skin alone")
    allowed = {'parameters/modification', 'modifications[0]/id'}
    for i, m in enumerate(mine['modifications'][0]['modifiers']):
        if m['path'] == ['addons']:
            check(m['value'][1] == fid, f"{r['fit']}: puts another scope")
            allowed.add(f'modifications[0]/modifiers[{i}]/value[1]')
        elif m['path'] == ['addons', 'rifle_scope_dict_id']:
            check(m['value'] == fid, f"{r['fit']}: puts another scope")
            allowed.add(f'modifications[0]/modifiers[{i}]/value')
        elif m['path'] == ['object', 'model']:
            allowed.add(f'modifications[0]/modifiers[{i}]/value')
            gun_model = theirs['modifications'][0]['modifiers'][i]['value']
            prefix, gun_prefix = f"models/{m['value']}.skinned_model/", f'models/{gun_model}.skinned_model/'
            mine_files = sorted(p[len(prefix):] for p in files if p.startswith(prefix))
            check(mine_files == sorted(p[len(gun_prefix):] for p in inputs.paths('compose', gun_prefix)),
                  f"{r['fit']}: model files")
            for p in mine_files:
                if p != 'settings':
                    check(files[prefix + p] == inputs.read('compose', gun_prefix + p), f"{r['fit']}: {p}")
    check(differ <= allowed, f"{r['fit']}: differs from {r['of']} at {sorted(differ - allowed)}")


def check_fits_taken(check, rows, manifest, inputs, items, copies):
    """Every scope fit of a gun is its skins' too: one that only puts the scope on names them
    (config_append), one that puts a model on has a copy for each (not the TOZ-122 "Leather")."""
    appended = {op['path']: op['values'] for op in manifest['operations'] if op['op'] == 'config_append'}
    copied = {(r['of'], r['fit']) for r in copies}
    for path in inputs.paths('compose', 'gameplay/items/scopes/fit'):
        fit = inputs.config('compose', path)
        weapons = fit.get('srp_scope_fit', {}).get('weapons', [])
        swaps = any(m['path'] == ['object', 'model'] for m in fit['modifications'][0]['modifiers'])
        name = path.rsplit('/', 1)[1][:-len('.options')]
        for row in rows:
            if row.base not in weapons:
                continue
            if not swaps:
                check(row.name in appended.get(path, []), f'{row.name}: not on {name}')
            elif row.name != 'toz_122_premium':
                check(any(of == name and row.name in fit_name for of, fit_name in copied), f'{row.name}: no copy of {name}')
    for path, names in appended.items():
        weapons = inputs.config('compose', path)['srp_scope_fit']['weapons']
        check(all(any(r.name == n and r.base in weapons for r in rows) for n in names), f'{path}: appends a stranger')


def calibrate(check, inputs):
    """paint.repaint on the premium textures both builds ship: closer to 0.26g's own than 0.69d0's albedo."""
    for ours, theirs, gun, gun69, metal69 in CALIBRATION:
        truth = paint.decode_dds(inputs.read('compose', f'textures/{ours}.dds'))
        skin = paint.decode_dds(inputs.read('donor', f'textures/{theirs}.dds'))
        metal = paint.decode_dds(inputs.read('donor', f'textures/{metal69}.dds'))
        everywhere = np.ones(truth.shape[:2], bool)
        image, mask = paint.repaint(paint.decode_dds(inputs.read('compose', f'textures/{gun}.dds')),
                                    paint.decode_dds(inputs.read('donor', f'textures/{gun69}.dds')), metal, skin, metal,
                                    everywhere)
        where = mask > 0.5
        ours_error = float(np.abs(image - truth).mean(-1)[where].mean())
        albedo_error = float(np.abs(paint.resized(skin, truth.shape[:2]) - truth).mean(-1)[where].mean())
        print(f'paint check {ours}: {ours_error:.3f} a channel where repainted ({where.mean():.0%} of it), '
              f'0.69d0\'s albedo as it is {albedo_error:.3f}')
        check(ours_error <= albedo_error + 0.002, f'paint check {ours}')


if __name__ == '__main__':
    main()
