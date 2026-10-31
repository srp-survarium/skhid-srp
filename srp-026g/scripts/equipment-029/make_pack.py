#!/usr/bin/env python3
"""Build equipment-029 from pinned retail inputs and the reviewed items.tsv.

Inputs are extracted resource trees, never server-game-data.db. --donor is the
0.29c metadata tree, --assets its dependency closure, --base the released 0.26g
equipment configs/localizations/movie. inputs.sha256 pins every consumed file.
The output is a resource-porter content pack, not a hand-assembled patch.
"""
import argparse
import copy
import csv
import hashlib
import io
import json
from pathlib import Path
import struct
import sys
import zlib

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw

LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')
PACK = 'equipment-029'
VERSION = '4'
DICTIONARY = 'gameplay/db_static_dictionaries'
MOVIE = 'flash_movies/lobby_menu.swf'
DONOR_ATLAS = 'flash_movies/textures/ui_items/equipment.png'
HIT_TYPES = {'ranged', 'slash', 'explosive', 'radiation', 'shock', 'fire', 'toxic'}
PHYSICAL = {'ranged', 'slash', 'explosive'}


def sha(data):
    return hashlib.sha256(data).hexdigest()


class Inputs:
    def __init__(self, roots):
        self.roots = roots
        self.sums = {}
        for line in (HERE / 'inputs.sha256').read_text().splitlines():
            digest, path = line.split('  ', 1)
            self.sums[path] = digest
        self.used = set()

    def read(self, group, path):
        key = f'{group}/{path}'
        raw = (self.roots[group] / path).read_bytes()
        if self.sums.get(key) != sha(raw):
            raise ValueError(f'{key}: input missing from inputs.sha256 or changed')
        self.used.add(key)
        return raw

    def config(self, group, path):
        return bcw.loads(self.read(group, path))


def strings(raw):
    """Localization values are UTF-8, unlike ordinary config strings."""
    root = dict(bcw.read(raw)[1])
    return {key.decode(): value[1].decode('utf-8') for key, value in root[b'strings'][1]}


def porter_text(text):
    # The porter serializes strings with WHATWG Windows-1251, including 0x98.
    return ''.join('\x98' if b == 0x98 else bytes([b]).decode('cp1251') for b in text.encode())


def atlas(raw):
    """Read the exported equipment bitmap, retaining premultiplied RGBA."""
    if raw[:3] == b'CWS':
        body = zlib.decompress(raw[8:])
    elif raw[:3] == b'FWS':
        body = raw[8:]
    else:
        raise ValueError('unsupported SWF compression')
    at = (5 + 4 * (body[0] >> 3) + 7) // 8 + 4
    images, symbols = {}, {}
    while at < len(body):
        tag, = struct.unpack_from('<H', body, at)
        at += 2
        size = tag & 63
        if size == 63:
            size, = struct.unpack_from('<I', body, at)
            at += 4
        chunk = body[at:at + size]
        at += size
        if tag >> 6 == 36:
            ident, fmt, width, height = struct.unpack_from('<HBHH', chunk)
            if fmt == 5:
                argb = zlib.decompress(chunk[7:])
                if len(argb) != width * height * 4:
                    raise ValueError('bad atlas dimensions')
                rgba = bytearray(len(argb))
                for channel in range(3):
                    rgba[channel::4] = argb[channel + 1::4]
                rgba[3::4] = argb[0::4]
                images[ident] = (width, height, bytes(rgba))
        elif tag >> 6 == 76:
            count, = struct.unpack_from('<H', chunk)
            pos = 2
            for _ in range(count):
                ident, = struct.unpack_from('<H', chunk, pos)
                end = chunk.index(0, pos + 2)
                symbols[chunk[pos + 2:end].decode()] = ident
                pos = end + 1
    return images[symbols['LobbyMenu_EquipmentIcons']]


def cell(bitmap, row, col):
    width, height, pixels = bitmap
    x, y = (col - 1) * 171, (row - 1) * 91
    if x + 171 > width or y + 83 > height:
        raise ValueError(f'atlas cell {row},{col} outside {width}x{height}')
    return b''.join(pixels[((y + dy) * width + x) * 4:((y + dy) * width + x + 171) * 4]
                    for dy in range(83))


def png(pixels, width=171, height=83):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    rows = b''.join(b'\0' + pixels[y * width * 4:(y + 1) * width * 4] for y in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(rows, 9)) + chunk(b'IEND', b''))


def shipped_switches():
    """The (stage, effect id, switches on) combinations 0.26g has shaders for."""
    shipped = set()
    for line in (HERE / 'shader_switches.tsv').read_text().splitlines():
        if not line.startswith('#'):
            stage, effect, on = line.split('\t')
            shipped.add((stage, effect, frozenset(filter(None, on.split(',')))))
    return shipped


# A map's switch and the texture it samples.
MAPS = {
    'use_alphablended_normal': 'texture_alphablended_normal',
    'use_detail_nmap': 'texture_detail_normal',
    'use_emissive_map': 'texture_emissive',
    'use_nmap': 'texture_normal',
    'use_subsurface_scattering_mask_map': 'texture_sss_mask',
    'use_tdiffuse': 'texture_diffuse',
    'use_tfresnel': 'texture_fresnel',
    'use_troughness': 'texture_roughness',
    'use_tspecular_intensity': 'texture_specular_intensity',
    'use_tspecular_power': 'texture_specular_power',
    'use_vertex_blended_normal': 'texture_vertex_blended_normal',
    'use_vertex_blended_specular': 'texture_vertex_blended_specular',
}


def shipped_textures(own):
    """The character and editor textures players have, and the pack's `own`."""
    listed = (HERE / 'shipped_textures.txt').read_text().splitlines()
    return set(listed) | {path for path in own if path.startswith('textures/') and path.endswith('.dds')}


def material_switches(material):
    """Each stage's effect of a material instance: (stage, effect id, switches on)."""
    for stage, body in material['material'].items():
        effect = body.get('effect') if isinstance(body, dict) else None
        if isinstance(effect, dict) and 'effect_id' in effect:
            on = frozenset(key for key, flag in effect.items()
                           if key.startswith('use_') and isinstance(flag, dict) and flag.get('value') is True)
            yield stage, effect['effect_id'], on


def missing_maps(material, textures):
    """(stage, switch, texture) of each map switched on whose texture is nowhere."""
    for stage, _, on in material_switches(material):
        effect = material['material'][stage]['effect']
        for switch in sorted(on & MAPS.keys()):
            texture = effect.get(MAPS[switch], {}).get('value')
            if texture and f'textures/{texture}.dds' not in textures:
                yield stage, switch, texture


def drawable_material(path, raw, shipped, textures):
    """The donor material with what 0.26g cannot draw turned off.

    0.29 switches on maps whose texture neither game ships (the Watchman
    jacket's emissive map, ...) and draws a few switch combinations 0.26g never
    compiled a shader for (the Watchman gloves' subsurface mask, ...): either
    stops the game. Such a map's switch goes off; then, of the shipped
    combinations within the donor's, the one keeping most is kept.
    """
    material = bcw.loads(raw)
    changed = []
    for stage, switch, texture in list(missing_maps(material, textures)):
        material['material'][stage]['effect'][switch]['value'] = False
        changed.append(f'{stage}.{switch} (no {texture})')
    for stage, effect, on in list(material_switches(material)):
        if (stage, effect, on) in shipped:
            continue
        kept = max((have for s, e, have in shipped if s == stage and e == effect and have <= on),
                   key=len, default=None)
        if kept is None:
            raise ValueError(f'{path}: no shipped {stage} shader within {sorted(on)}')
        for switch in sorted(on - kept):
            material['material'][stage]['effect'][switch]['value'] = False
            changed.append(f'{stage}.{switch} (no shader)')
    if not changed:
        return raw
    out = bcw.dump(material)
    if bcw.dump(bcw.loads(out)) != out:
        raise ValueError(f'{path}: material roundtrip failed')
    print(f'{path}: off {", ".join(changed)}')
    return out


def specs():
    with (HERE / 'items.tsv').open() as source:
        return list(csv.DictReader(source, delimiter='\t'))


def movie_metadata(rows):
    """The movie cannot read extra item-config fields; generate its mirror."""
    entries = []
    for row in rows:
        if row['plain_id']:
            entries.append(f"         [{row['donor_id']},{row['plain_id']},[1240,{1240 + 10 * int(row['faction'])}]]")
    return ('package survarium.constants\n{\n'
            '   // Generated by scripts/equipment-029/make_pack.py from items.tsv.\n'
            '   // The server reads srp_premium from the item configs in game data.\n'
            '   public class Equipment029\n   {\n'
            '      private static const ITEMS:Array = [\n' + ',\n'.join(entries) + '\n      ];\n'
            '      public static function premium(dictId:int) : Array\n      {\n'
            '         for each(var row:Array in ITEMS)\n         {\n'
            '            if(row[0] == dictId) return row;\n         }\n'
            '         return null;\n      }\n   }\n}\n')


def convert(source, template, spec, plain=None):
    """Native parameters and hit vocabulary; donor clothing geometry and sounds."""
    result = copy.deepcopy(source)
    params = copy.deepcopy((plain or template)['parameters'])
    params.update(faction=int(spec['faction']), item_level=int(spec['level']),
                  match_making_level=int(spec['level']), base_rarity_level=0)
    if params['type'] != source['parameters']['type']:
        raise ValueError(f"{spec['donor_id']}: template has a different equipment kind")
    result['parameters'] = params
    if plain is None:
        params['weight'] = float(spec['weight'])
        params['drop_weight'] = int(spec['drop_weight'])
        for name in ('armor', 'arms_armor', 'back_armor'):
            if name in source['parameters']:
                params[name] = float(spec['armour']) / 100
        if 'ano_armor' in source['parameters']:
            params['ano_armor'] = float(spec['isolation']) / 100
        result['default_modifications'] = [int(value) for value in spec['default_modifications'].split(',') if value]
    else:
        params['drop_weight'] = 0
        result['default_modifications'] = plain.get('default_modifications', [])[:]
        result['srp_premium'] = {'plain_item': int(spec['plain_id']),
                                 'bonuses': [1240, 1240 + 10 * int(spec['faction'])]}
    if plain is not None and 'hit_params' in plain:
        result['hit_params'] = copy.deepcopy(plain['hit_params'])
    else:
        for slot in result.get('hit_params', {}).values():
            slot['hit_types'] = {
                kind: {'armor': params.get('armor' if kind in PHYSICAL else 'ano_armor', 0.0)}
                for kind in slot['hit_types'] if kind in HIT_TYPES
            }
    if result['data']['type'] == 5:
        result['data']['type'] = 4
    elif result['data']['type'] != 1:
        raise ValueError(f"unsupported equipment data type: {spec['donor_id']}")
    if int(spec['donor_id']) == 888:
        # The port keeps the player's native head, as the existing mask ports do.
        result['data'].pop('remove_head_model', None)
        result['data'].pop('additional_models', None)
    if params['type'] == 'arm_oxy':
        params['oxygen_amount'] = source['parameters']['oxygen_amount']
        result['data']['amount_time_sec'] = params['oxygen_amount']
    # The native client has no bleeding-protection card property (134).
    props = result['ui_desc']['props_list']
    result['ui_desc']['props_list'] = [p for p in props if p['prop_id'] != 134]
    for prop in result['ui_desc']['props_list']:
        key = {6: 'armor', 8: 'ano_armor', 15: 'weight', 11: 'oxygen_amount'}.get(prop['prop_id'])
        if key:
            value = params[key] * (100 if key in ('armor', 'ano_armor') else 1)
            prop['prop_value'] = f'{value:.1f}' if key == 'weight' else f'{value:g}'
    result['ui_desc']['icon'] = [int(spec['icon_row']), int(spec['icon_col'])]
    return result


def write_pack(inputs, out):
    donor = inputs.config('donor', DICTIONARY)['items_dict']
    base = inputs.config('base', DICTIONARY)['items_dict']
    donor_by_id = {r['dict_id']: r for r in donor}
    base_by_id = {r['dict_id']: r for r in base}
    base_paths = {r['cfg_name'] for r in base}
    rows = specs()
    # The balance audit's effect values must come from this exact published data.
    inputs.read('base', 'gameplay/items/modifications/modifications.options')
    ids = [int(r['donor_id']) for r in rows]
    if len(ids) != len(set(ids)) or set(ids) & base_by_id.keys():
        raise ValueError('new equipment ids are duplicated or already occupied')
    expected = {r['dict_id'] for r in donor if '/armour/' in r['cfg_name'] and r['cfg_name'] not in base_paths}
    # Protective mask already shipped as 1816 (face-069d), with the owner's stats.
    if expected != set(ids) | {948}:
        raise ValueError(f'uncategorized donor equipment: {expected ^ (set(ids) | {948})}')
    from PIL import Image
    donor_image = Image.open(io.BytesIO(inputs.read('donor', DONOR_ATLAS))).convert('RGBA')
    donor_atlas = (donor_image.width, donor_image.height, donor_image.tobytes())
    base_atlas = atlas(inputs.read('base', MOVIE))
    entries, icons, configs = [], [], {}
    operations = []
    text_keys = set()
    files = {}
    report = []
    for spec in sorted(rows, key=lambda r: (bool(r['plain_id']), int(r['donor_id']))):
        ident = int(spec['donor_id'])
        entry = copy.deepcopy(donor_by_id[ident])
        template_row = base_by_id[int(spec['template_id'])]
        template = inputs.config('base', template_row['cfg_name'])
        source = inputs.config('donor', entry['cfg_name'])
        plain = None
        if spec['plain_id']:
            plain_id = int(spec['plain_id'])
            plain = configs.get(plain_id)
            if plain is None:
                plain = inputs.config('base', base_by_id[plain_id]['cfg_name'])
        config = convert(source, template, spec, plain)
        configs[ident] = config
        entry['base_cost'] = int(spec['price'])
        # Keep the historical dictionary names (including the donor's toro typo).
        entries.append(entry)
        row, col = config['ui_desc']['icon']
        if col <= base_atlas[0] // 171 and any(cell(base_atlas, row, col)[3::4]):
            raise ValueError(f'occupied target icon cell {row},{col}')
        pixels = cell(donor_atlas, *source['ui_desc']['icon'])
        if not any(pixels[3::4]):
            raise ValueError(f'{ident}: donor icon is empty')
        icon_path = f'icons/{ident}.png'
        files[icon_path] = png(pixels)
        icons.append({'row': row, 'col': col, 'source': icon_path})
        for key, value in config['ui_desc']['text_descriptions'].items():
            # Own localization namespace: a stock string may have changed meaning.
            text_keys.add((value, 'st_srp_equipment029_' + value.removeprefix('st_')))
            config['ui_desc']['text_descriptions'][key] = 'st_srp_equipment029_' + value.removeprefix('st_')
        raw = bcw.dump(config)
        if bcw.loads(raw) != config:
            # f32 rounding is expected; verify using a second serialization instead.
            if bcw.dump(bcw.loads(raw)) != raw:
                raise ValueError(f'{ident}: config roundtrip failed')
        files['files/' + entry['cfg_name']] = raw
        operations.append({'op': 'add_file', 'path': entry['cfg_name']})
        report.append({'id': ident, 'name': spec['name'], 'level': config['parameters']['item_level'],
                       'armour': spec['armour'], 'isolation': spec['isolation'], 'price': entry['base_cost'],
                       'premium': entry['is_premium'], 'template_id': int(spec['template_id'])})
    # Pinned dependency closure. Shared assets are reused from the released game;
    # this list contains only new assets, including implicit camouflage AOAC maps.
    shipped = shipped_switches()
    assets = (HERE / 'assets.txt').read_text().splitlines()
    textures = shipped_textures(assets)
    for line in assets:
        raw = inputs.read('assets', line)
        if line.endswith('.material'):
            raw = drawable_material(line, raw, shipped, textures)
        files['files/' + line] = raw
        operations.append({'op': 'add_file', 'path': line})
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    for lang in LANGUAGES:
        loc = strings(inputs.read('donor', f'localization/{lang}/localization'))
        missing = {old for old, _ in text_keys if old not in loc}
        if any(not key.endswith('_desc') for key in missing):
            raise ValueError(f'{lang}: missing item names {missing}')
        # Retail shipped no descriptions for these keys, in any language.
        # An explicit empty description avoids showing a raw missing-key label.
        operations.append({'op': 'localization_add', 'languages': [lang],
                           'strings': {new: porter_text(loc.get(old, '')) for old, new in sorted(text_keys)}})
    operations.append({'op': 'atlas_icons_add', 'icons': icons})
    manifest = {'format_version': 1, 'id': PACK, 'version': VERSION, 'target_build': '0.26g',
                'operations': operations,
                'notes': ['0.29c retail equipment, adapted to 0.26g by scripts/equipment-029/items.tsv.',
                          'Protective mask 948 is already present as 1816; its model and stats are retained.']}
    files['pack.json'] = (json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode()
    files['balance.json'] = (json.dumps(report, ensure_ascii=False, indent=2) + '\n').encode()
    files['ui/Equipment029.as'] = movie_metadata(rows).encode()
    out.mkdir(parents=True, exist_ok=False)
    for name, raw in sorted(files.items()):
        dest = out / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(raw)
    print(f'{PACK}@{VERSION}: {len(entries)} items, {len(operations)} operations; pack.json {sha(files["pack.json"])}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('donor', 'assets', 'base', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    write_pack(Inputs({name: getattr(args, name) for name in ('donor', 'assets', 'base')}), args.out)


if __name__ == '__main__':
    main()
