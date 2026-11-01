#!/usr/bin/env python3
"""Build weapon-variants from the pinned inputs (prepare_inputs.py) and variants.tsv.

usage: python3 make_pack.py --inputs <prepare_inputs.py's folder> --out <packs folder>/weapon-variants-<version>
           [--previews <folder>]

Each skin is a gold-bought copy of its live gun (docs/notes/weapon-variants.md), as the live golden
weapons are (the AKM's two premium copies): the gun's config as the edit lists start from it, with
the skin's model, icon and name, no drop weight, and `srp_premium` (its plain gun and the silver bonus
srp gives it, golden_weapons.md); its model the gun's, its materials the gun's with the skin's
diffuse (paint.py: 0.69d0's paint on 0.26g's texture); each new diffuse's camouflage mask (_aoac,
client patch 44) the gun's own, byte for byte; the gun's ammo rows in items_compatibility; the gun's
scope fits (one that only puts a scope on names the skin too; one that puts the gun's model with a
mount on is copied, its model with the skin's materials). The TOZ-122 "Leather" is a
model of its own (mesh.py: 0.69d0's mesh and its decor, on the TOZ-122's texture).

--previews writes, per skin, a picture of the gun before and after (render.py) and each repainted
texture with its mask.
"""
import argparse
import copy
import json
from pathlib import Path

import numpy as np

import icons
import mesh as donor_mesh
import paint
from common import (DICTIONARY, FOLDER, PACK, SCOPES, SILVER_BONUS, SYMBOL, WEAPONS, Inputs, bcw, mesh, name_key,
                    sha, variants)

VERSION = '1'


def dump(config):
    raw = bcw.dump(config)
    if bcw.dump(bcw.loads(raw)) != raw:
        raise ValueError('config does not read back as written')
    return raw


def model_files(inputs, group, model, kind='skinned_model'):
    return [p for p in inputs.paths(group, f'models/{model}.{kind}/')]


def effect(material):
    return material['material']['g_stage']['effect']


def surfaces26(inputs, model):
    settings = inputs.config('compose', f'models/{model}.skinned_model/settings')
    out = {}
    for surface, entry in settings['material_settings'].items():
        material = entry['material_name']
        path = f'models/{model}.skinned_model/render/{surface}/converted_model'
        diffuse = effect(inputs.config('compose', f'material_instances/{material}.material'))['texture_diffuse']['value']
        out[surface] = {'material': material, 'diffuse': diffuse, 'mesh': mesh(inputs.read('compose', path), '0.26g')}
    return out


def surfaces69(inputs, model):
    settings = inputs.config('donor', f'models/{model}.skinned_model/settings')
    out = {}
    for surface in settings['lod_hierrarchy']['LOD0']['surfaces']:
        material = settings['material_settings'][surface]['material_name']
        e = effect(inputs.config('donor', f'material_instances/{material}.material'))
        path = f'models/{model}.skinned_model/render/{surface}/converted_model'
        out[surface] = {'material': material, 'albedo': e['t_albedo']['value'], 'metal': e['t_metalness']['value'],
                        'mesh': mesh(inputs.read('donor', path), '0.69d0')}
    return out


def match(ours, theirs):
    """Our surface's 0.69d0 surface: the one holding most of its vertices at the same place with the
    same UV (0.69d0 keeps 0.26g's guns but for the modules it made separate), or one with the same
    triangles and UVs moved as a whole (0.69d0's SPAS-12). None when no 0.69d0 surface is it."""
    pos, uv, tris = ours
    best, score = None, (0, 0)
    for name, (p, u, t) in theirs.items():
        index = {}
        for key, uv_ in zip(map(tuple, np.round(p, 4)), u):
            index.setdefault(key, []).append(uv_)
        shared = same = 0
        for key, uv_ in zip(map(tuple, np.round(pos, 4)), uv):
            there = index.get(key)
            if there:
                shared += 1
                same += any(np.abs(x - uv_).max() < 2e-3 for x in there)
        moved = (len(p) == len(pos) and t.shape == tris.shape and (t == tris).all()
                 and np.abs(u - uv).max() < 2e-3)
        if moved:
            shared = same = len(pos)
        if (same, shared) > score:
            best, score = name, (same, shared)
    same, shared = score
    if best is None or shared < 0.5 * len(pos) or same < 0.98 * shared:
        return None, shared / len(pos), (same / shared if shared else 0)
    return best, shared / len(pos), same / shared


def plan(inputs, row, base_config):
    """Per surface of the live gun's model: its material, diffuse and, where the skin repaints it, the
    0.69d0 maps to repaint it from."""
    model = base_config['object']['model']
    ours = surfaces26(inputs, model)
    skin = surfaces69(inputs, row.donor_model)
    gun = surfaces69(inputs, row.donor_base_model)
    out = []
    for surface, s in sorted(ours.items()):
        theirs, shared, same = match(s['mesh'], {k: v['mesh'] for k, v in skin.items()})
        entry = {'surface': surface, 'material': s['material'], 'diffuse': s['diffuse'], 'donor': theirs,
                 'shared': round(shared, 3), 'uv_same': round(same, 4), 'repaint': None}
        if theirs and theirs in gun and skin[theirs]['albedo'] != gun[theirs]['albedo']:
            entry['repaint'] = {'skin': skin[theirs]['albedo'], 'skin_metal': skin[theirs]['metal'],
                                'gun': gun[theirs]['albedo'], 'gun_metal': gun[theirs]['metal'],
                                'skin_mesh': skin[theirs]['mesh']}
        out.append(entry)
    return model, ours, out


def gun_factors(inputs, row, surfaces):
    """paint.factors of the gun's one texture (the TOZ-122's: the wrap's diffuse from 0.69d0's albedo)."""
    (s,) = surfaces
    gun = surfaces69(inputs, row.donor_base_model)
    (theirs,) = gun
    diffuse = paint.decode_dds(inputs.read('compose', f"textures/{s['diffuse']}.dds"))
    albedo = paint.decode_dds(inputs.read('donor', f"textures/{gun[theirs]['albedo']}.dds"))
    metal = paint.decode_dds(inputs.read('donor', f"textures/{gun[theirs]['metal']}.dds")).mean(-1)
    _, uv, tris = gun[theirs]['mesh']
    return paint.factors(diffuse, albedo, metal, paint.islands(uv, tris, diffuse.shape[:2]))


def texture_name(row, diffuse):
    return f"{FOLDER}/{row.name}/{diffuse.rsplit('/', 1)[1]}"


def write_pack(inputs, out, previews):
    rows = variants()
    dictionary = inputs.config('compose', DICTIONARY)
    items = {item['name']: item for item in dictionary['items_dict']}
    taken = {item['dict_id'] for item in dictionary['items_dict']}
    files, ops, entries, compat, cells, report = {}, [], [], {}, [], []

    def add(path, raw):
        if inputs.has('compose', path):
            raise ValueError(f'{path}: the game has it already')
        files['files/' + path] = raw
        ops.append({'op': 'add_file', 'path': path})

    fits, shared = [], {}
    for path in inputs.paths('compose', SCOPES + '/fit'):
        fits.append((path, inputs.config('compose', path)))
    next_fit = max(r.dict_id for r in rows) + 1
    for row in rows:
        if row.name in items or row.dict_id in taken:
            raise ValueError(f'{row.name} ({row.dict_id}): name or id taken')
        base = items[row.base]
        base_path = f'{WEAPONS}/wpn_{row.base}.options'
        config = inputs.config('compose', base_path)
        model26, ours, surfaces = plan(inputs, row, config)
        new_model = f'{FOLDER}/{row.name}'
        swap = {}                       # live material -> the skin's
        for s in surfaces:
            if not s['repaint']:
                continue
            r = s['repaint']
            dds = inputs.read('compose', f"textures/{s['diffuse']}.dds")
            diffuse = paint.decode_dds(dds)
            _, skin_uv, skin_tris = r['skin_mesh']
            image, mask = paint.repaint(diffuse,
                                        paint.decode_dds(inputs.read('donor', f"textures/{r['gun']}.dds")),
                                        paint.decode_dds(inputs.read('donor', f"textures/{r['gun_metal']}.dds")),
                                        paint.decode_dds(inputs.read('donor', f"textures/{r['skin']}.dds")),
                                        paint.decode_dds(inputs.read('donor', f"textures/{r['skin_metal']}.dds")),
                                        paint.islands(skin_uv, skin_tris, diffuse.shape[:2]))
            texture = texture_name(row, s['diffuse'])
            if s['material'] in swap:
                raise ValueError(f"{row.name}: {s['material']} repainted twice")
            add(f'textures/{texture}.dds', paint.encode_over(dds, image, mask))
            add(f'textures/{texture}.options', inputs.read('compose', f"textures/{s['diffuse']}.options"))
            # The camouflage mask is the gun's (the same UVs): weapon-camo's file, byte for byte.
            for ext in ('.dds', '.options'):
                source = (('players', f"textures/{s['diffuse']}_aoac.dds") if ext == '.dds'
                          else ('players', 'textures/character/human/black_market_01/black_market_01_torso_aoac.options'))
                add(f'textures/{texture}_aoac{ext}', inputs.read(*source))
            material = copy.deepcopy(inputs.config('compose', f"material_instances/{s['material']}.material"))
            effect(material)['texture_diffuse']['value'] = texture
            swap[s['material']] = f"{FOLDER}/{row.name}/{s['material'].rsplit('/', 1)[1]}"
            add(f"material_instances/{swap[s['material']]}.material", dump(material))
            s['texture'] = texture
            s['repainted'] = round(float(mask.mean()), 4)
            if previews:
                icons.texture_preview(previews / f"{row.name}__{s['surface']}.png", paint.decode_dds(dds), image, mask)
        gun_view = [(ours[s['surface']]['mesh'], paint.decode_dds(inputs.read('compose', f"textures/{s['diffuse']}.dds")))
                    for s in surfaces]
        if row.name == 'toz_122_premium':
            # The model of its own: 0.69d0's mesh and its decor on the TOZ-122's texture (mesh.py).
            if swap:
                raise ValueError(f'{row.name}: a model of its own repaints nothing')
            leather, wrap = donor_mesh.leather(inputs, row, model26, new_model, gun_factors(inputs, row, surfaces))
            for path, raw in leather:
                add(path, raw)
            body = mesh(wrap['meshes'][wrap['body']], '0.26g')
            skin_view = [(body, gun_view[0][1]), (mesh(wrap['meshes'][wrap['surface']], '0.26g'), wrap['wrap_image'])]
            surfaces.append({'surface': wrap['surface'], 'texture': wrap['diffuse'], 'model': '0.69d0 decor'})
        elif not swap:
            raise ValueError(f'{row.name}: nothing repainted')
        else:
            copy_model(inputs, model26, new_model, swap, add)
            skin_view = [(m, paint.decode_dds(files[f"files/textures/{s['texture']}.dds"]) if s.get('texture') else t)
                         for (m, t), s in zip(gun_view, surfaces)]
        # The config: the gun's as the edit lists start from it.
        variant = copy.deepcopy(config)
        variant['object']['model'] = new_model
        variant['parameters']['drop_weight'] = 0
        variant['ui_desc']['icon'] = list(row.icon)
        variant['ui_desc']['text_descriptions']['name'] = name_key(row)
        if variant.get('default_modifications') or variant['parameters'].get('base_rarity_level', 0):
            raise ValueError(f'{row.base}: the gun has bonuses or a rarity of its own')
        variant['srp_premium'] = {'plain_item': base['dict_id'], 'bonuses': [SILVER_BONUS]}
        cfg = f'{WEAPONS}/wpn_{row.name}.options'
        add(cfg, dump(variant))
        entries.append({'base_cost': row.gold, 'cfg_name': cfg, 'dict_id': row.dict_id, 'is_premium': True,
                        'is_stack': False, 'item_category': base['item_category'], 'name': row.name})
        for name, pair in dictionary['items_compatibility'].items():
            for mine, other in (('first_item', 'second_item'), ('second_item', 'first_item')):
                if pair[mine] == base['dict_id']:
                    key = f'items_compatibility.compatibility_variant_{row.dict_id}_{pair[other]}'
                    if key in compat:
                        raise ValueError(f'{key}: twice')
                    compat[key] = {'table': {mine: row.dict_id, other: pair[other]}}
        cells.append({'row': row.icon[0], 'col': row.icon[1], 'source': f'icons/{row.name}.png', 'premultiplied': True})
        files[f'icons/{row.name}.png'], icon_fit = icons.icon(inputs, row, config, gun_view, skin_view, previews)
        # The gun's scope fits, for the skin: one that only puts the scope on is the skin's too (its
        # weapons list, as the live AK-74N's copy shares its gun's); one that puts the gun's model with
        # a mount on is copied, its model the skin's materials on that model (the TOZ-122 "Leather",
        # of another mesh, takes none of those).
        for path, fit in fits:
            if row.base not in fit['srp_scope_fit']['weapons']:
                continue
            fit_row = items[path.rsplit('/', 1)[1][:-len('.options')]]
            if not any(m['path'] == ['object', 'model'] for m in fit['modifications'][0]['modifiers']):
                shared.setdefault(path, []).append(row.name)
                report.append({'fit': fit_row['name'], 'shared': row.name})
                continue
            if row.name == 'toz_122_premium':
                report.append({'fit': fit_row['name'], 'skipped': row.name, 'why': 'the fit\'s model is the gun\'s mesh'})
                continue
            fit_id = next_fit
            next_fit += 1
            stem = fit_row['name']
            fit_name = (stem[:-len(row.base)] + row.name if stem.startswith('fitn_') and stem.endswith('_' + row.base)
                        else 'fit_' + row.name + stem[len('fit_' + row.base):])
            if fit_name == stem or not stem.startswith(('fitn_', f'fit_{row.base}_')):
                raise ValueError(f'{stem}: no name for its copy')
            new = copy.deepcopy(fit)
            new['parameters']['modification'] = 990000 + fit_id
            mod = new['modifications'][0]
            mod['id'] = 990000 + fit_id
            for m in mod['modifiers']:
                if m['path'] == ['addons']:
                    m['value'][1] = fit_id
                elif m['path'] == ['addons', 'rifle_scope_dict_id']:
                    m['value'] = fit_id
                elif m['path'] == ['object', 'model']:
                    fit_model = f'{FOLDER}/{fit_name}'
                    copy_model(inputs, m['value'], fit_model, swap, add)
                    m['value'] = fit_model
            new['srp_scope_fit']['weapons'] = [row.name]
            add(f'{SCOPES}/{fit_name}.options', dump(new))
            entries.append({**fit_row, 'cfg_name': f'{SCOPES}/{fit_name}.options', 'dict_id': fit_id,
                            'name': fit_name})
            report.append({'fit': fit_name, 'dict_id': fit_id, 'of': fit_row['name']})
        report.append({'variant': row.name, 'dict_id': row.dict_id, 'base': row.base, 'gold': row.gold,
                       'cell': list(row.icon), 'model': new_model, 'icon': icon_fit,
                       'surfaces': [{k: v for k, v in s.items() if k != 'repaint'} for s in surfaces]})
    ops.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    ops.append({'op': 'config_set', 'path': DICTIONARY, 'values': dict(sorted(compat.items()))})
    ops.append({'op': 'atlas_icons_add', 'symbol': SYMBOL, 'icons': cells})
    for path, names in sorted(shared.items()):
        ops.append({'op': 'config_append', 'path': path, 'array_path': 'srp_scope_fit.weapons', 'values': names})
    manifest = {'format_version': 1, 'id': PACK, 'version': VERSION, 'target_build': '0.26g',
                'description': 'the owner\'s pick of 0.69d0\'s skins of live guns, each a gold-bought copy of its gun '
                               '(scripts/weapon-variants; docs/notes/weapon-variants.md)',
                'operations': ops,
                'notes': ['made by srp-026g/scripts/weapon-variants/make_pack.py from its pinned inputs']}
    files['pack.json'] = (json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode()
    files['ui/WeaponVariants1.as'] = movie_mirror(rows, items).encode()
    files['report.json'] = (json.dumps(report, ensure_ascii=False, indent=1) + '\n').encode()
    out.mkdir(parents=True, exist_ok=False)
    for name, raw in sorted(files.items()):
        dest = out / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(raw)
    print(f'{PACK}@{VERSION}: {len(rows)} skins, {len(entries) - len(rows)} scope fits copied, {len(shared)} shared, '
          f'{len(ops)} operations; '
          f'pack.json {sha(files["pack.json"])}')


def movie_mirror(rows, items):
    """The lobby movie's copy of the skins' srp_premium (the movie cannot read the configs):
    flash/lobby_menu/src/survarium/constants/WeaponVariants1.as, which check_pack.py holds to it."""
    lines = ',\n'.join(f'         [{row.dict_id},{items[row.base]["dict_id"]},[{SILVER_BONUS}]]' for row in rows)
    return f'''package survarium.constants
{{
   // Generated by scripts/weapon-variants/make_pack.py from variants.tsv: the skins of live guns,
   // golden weapons (srp_premium: [dictId, plain gun, bonuses]). The server reads srp_premium
   // from the item configs in game data.
   public class WeaponVariants1
   {{
      private static const ITEMS:Array = [
{lines}
      ];
      public static function premium(dictId:int) : Array
      {{
         for each(var row:Array in ITEMS)
         {{
            if(row[0] == dictId) return row;
         }}
         return null;
      }}
   }}
}}
'''


def copy_model(inputs, model, new_model, swap, add):
    """`model`'s files under `new_model`, its settings naming the skin's materials."""
    prefix = f'models/{model}.skinned_model/'
    paths = model_files(inputs, 'compose', model)
    if prefix + 'settings' not in paths:
        raise ValueError(f'{model}: not in the inputs')
    for path in paths:
        raw = inputs.read('compose', path)
        if path == prefix + 'settings':
            settings = bcw.loads(raw)
            for entry in settings['material_settings'].values():
                entry['material_name'] = swap.get(entry['material_name'], entry['material_name'])
            raw = dump(settings)
        add(f'models/{new_model}.skinned_model/' + path[len(prefix):], raw)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--previews', type=Path)
    args = parser.parse_args()
    if args.previews:
        args.previews.mkdir(parents=True, exist_ok=True)
    write_pack(Inputs(args.inputs), args.out, args.previews)


if __name__ == '__main__':
    main()
