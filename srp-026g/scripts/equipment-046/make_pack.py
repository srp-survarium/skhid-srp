#!/usr/bin/env python3
"""Build equipment-046 from the pinned inputs (prepare_inputs.py) and the reviewed items.tsv.

Inputs are extracted resource trees, never server-game-data.db: --inputs is
prepare_inputs.py's folder (groups donor031, donor046, assets031, assets046,
base); inputs.sha256 pins every consumed file. The item texts are the
localization catalog's (client-patches/localization/*.tsv). The output is a
resource-porter content pack, not a hand-assembled patch.

The shared machinery (icon atlas, shader switches, dictionary encoding) is
equipment-029's; this module adds the two donors' formats: 0.31a2 is 0.29c's
format, 0.46c2 stores its configs in 16-byte records, its items inside
gameplay/game.options without surface names, and its models with LOD1/LOD2.
"""
import argparse
import copy
import hashlib
import io
import json
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent


def _equipment029():
    """equipment-029's make_pack, under its own name (this module has the same file name)."""
    import importlib.util
    sys.path.insert(0, str(SCRIPTS / 'research-mode'))  # its bcw
    spec = importlib.util.spec_from_file_location('equipment029_make_pack', SCRIPTS / 'equipment-029' / 'make_pack.py')
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


e029 = _equipment029()
bcw, atlas, cell, png = e029.bcw, e029.atlas, e029.cell, e029.png
drawable_material, shipped_switches = e029.drawable_material, e029.shipped_switches
from prepare_inputs import sound_file, specs  # noqa: E402

PACK = 'equipment-046'
VERSION = '2'
DICTIONARY = 'gameplay/db_static_dictionaries'
GAME_OPTIONS = 'gameplay/game.options'
MODIFICATIONS = 'gameplay/items/modifications/modifications.options'
MOVIE = 'flash_movies/lobby_menu.swf'
CATALOG = SCRIPTS.parent / 'client-patches' / 'localization'
FLASH_MAPPING = SCRIPTS.parent / 'flash/lobby_menu/src/survarium/constants/Equipment046.as'
# The pack's dictionaries; the catalog's `original` is built from russian.tsv.
LANGUAGES = {'english': 'english', 'german': 'german', 'original': 'russian', 'polish': 'polish',
             'russian': 'russian', 'ukrainian': 'ukrainian'}
HIT_TYPES = ('ranged', 'slash', 'explosive', 'radiation', 'shock', 'fire', 'toxic')
PHYSICAL = {'ranged', 'slash', 'explosive'}
DATA_TYPE = 4  # the native equipment data type (donors: 5)
SURFACE_CHUNKS = (3, 4)  # vertices, indices: all a stock 0.26g skinned surface has
# Model and material files that are configs (0.46c2 writes them in 16-byte records).
CONFIG_NAMES = ('settings', 'skeleton', 'export_properties')
# A 0.46c2 material stage key 0.26g's materials never have.
LATER_STAGE_KEYS = ('use_foliage_sun_backlighting',)
CELL_W, CELL_H = 171, 83  # an equipment atlas cell


def sha(data):
    return hashlib.sha256(data).hexdigest()


class Inputs:
    def __init__(self, root):
        self.root = root
        self.sums = dict(reversed(line.split('  ', 1)) for line in (HERE / 'inputs.sha256').read_text().splitlines())

    def read(self, group, path):
        key = f'{group}/{path}'
        raw = (self.root / key).read_bytes()
        if self.sums.get(key) != sha(raw):
            raise ValueError(f'{key}: input missing from inputs.sha256 or changed')
        return raw

    def config(self, group, path):
        return bcw.loads(self.read(group, path))

    def group(self, group):
        return sorted(key.split('/', 1)[1] for key in self.sums if key.split('/', 1)[0] == group)


def catalog(language):
    """A catalog TSV as {key: text} (its escapes undone)."""
    rows = {}
    with (CATALOG / f'{language}.tsv').open(encoding='utf-8') as source:
        next(source)
        for line in source:
            key, text = line.rstrip('\n').split('\t', 1)
            rows[key] = text.replace('\\n', '\n').replace('\\t', '\t').replace('\\r', '\r').replace('\\\\', '\\')
    return rows


def text_keys(spec):
    stem = 'st_srp_equipment046_' + spec['id_069']
    return {'name': stem + '_name', 'description': stem + '_desc'}


def config_path(spec, template_row):
    folder = template_row['cfg_name'].rsplit('/', 1)[0]
    return f"{folder}/eq046_{spec['id_069']}.options"


def surface_chunks(raw):
    """A skinned surface with only the chunks 0.26g reads (0.31a2 adds 110, 0.46c2 110 and 113)."""
    out, at = bytearray(), 0
    while at < len(raw):
        kind, size = struct.unpack_from('<II', raw, at)
        if at + 8 + size > len(raw):
            raise ValueError('truncated surface chunk')
        if kind in SURFACE_CHUNKS:
            out += raw[at:at + 8 + size]
        at += 8 + size
    if [struct.unpack_from('<I', out, 0)[0], struct.unpack_from('<I', out, 8 + struct.unpack_from('<I', out, 4)[0])[0]] != [3, 4]:
        raise ValueError('surface without its vertex and index chunks')
    return bytes(out)


def lod0_settings(settings):
    """Model settings drawing LOD0 only, as every 0.26g skinned model does."""
    lods = settings['lod_hierrarchy']
    keep = set(lods['LOD0']['surfaces'])
    for name in ('LOD1', 'LOD2'):
        lods[name] = {}
    settings['material_settings'] = {k: v for k, v in settings['material_settings'].items() if k in keep}
    return settings


def asset(path, raw, shipped, textures):
    """A donor resource as the pack ships it."""
    name = path.rsplit('/', 1)[1]
    if name == 'converted_model':
        return surface_chunks(raw)
    if name in CONFIG_NAMES or path.endswith('.options') or path.endswith('.material'):
        original = bcw.loads(raw)
        value = copy.deepcopy(original)
        if name == 'settings':
            value = lod0_settings(value)
        if path.endswith('.material'):
            for stage in value['material'].values():
                if isinstance(stage, dict):
                    for key in LATER_STAGE_KEYS:
                        stage.pop(key, None)
        out = raw if value == original and not bcw.is_compact(raw) else bcw.dump(value)
        if path.endswith('.material'):
            out = drawable_material(path, out, shipped, textures)
        if bcw.dump(bcw.loads(out)) != bcw.dump(bcw.loads(bcw.dump(bcw.loads(out)))):
            raise ValueError(f'{path}: config roundtrip failed')
        return out
    if bcw.is_compact(raw):
        raise ValueError(f'{path}: an unexpected 16-byte config')
    return raw


def shipped_textures(own):
    """The character and editor textures players have, and the pack's `own`."""
    listed = (HERE / 'shipped_textures.txt').read_text().splitlines()
    return set(listed) | {path for path in own if path.startswith('textures/') and path.endswith('.dds')}


def data_sections(config):
    return [value for key, value in config.items() if key == 'data' or key.startswith('data_')]


def name_surfaces(config, surfaces):
    """0.46c2 leaves out each part's surface (`part_name`); 0.26g needs it."""
    for section in data_sections(config):
        for part in [section] + section.get('additional_models', []):
            if 'part_name' not in part:
                found = surfaces[part['base_model']]
                if len(found) != 1:
                    raise ValueError(f"{part['base_model']}: cannot name its surface among {found}")
                part['part_name'] = found[0]


def card(params, kind):
    """The native card: a backpack's weight; else armour, isolation, weight."""
    props = [(15, f"{params['weight']:.1f}")]
    if kind != 'arm_back':
        props = [(6, f"{params['armor'] * 100:g}"), (8, f"{params['ano_armor'] * 100:g}")] + props
    return [{'prop_id': ident, 'prop_value': value} for ident, value in props]


def sounds(source, template, released):
    """The donor's sounds; one the released game lacks (0.31's crouch rustle of pants) takes
    the template's sound in its place."""
    out = []
    for index, sound in enumerate(source.get('sounds', [])):
        if released(sound):
            out.append(sound)
        elif index < len(template.get('sounds', [])):
            out.append(template['sounds'][index])
    return out


def convert(source, template, spec, released, plain=None):
    """Native parameters, hit vocabulary and card; the donor's clothing, models and sounds."""
    result = copy.deepcopy(source)
    if 'sounds' in source:
        result['sounds'] = sounds(source, template, released)
    params = copy.deepcopy((plain or template)['parameters'])
    if params['type'] != source['parameters']['type']:
        raise ValueError(f"{spec['id']}: template has a different equipment kind")
    params.update(faction=int(spec['faction']), item_level=int(spec['level']),
                  match_making_level=int(spec['level']), base_rarity_level=0)
    result['parameters'] = params
    if plain is None:
        params['weight'] = float(spec['weight'])
        params['drop_weight'] = int(spec['drop_weight'])
        for name in ('armor', 'arms_armor', 'back_armor'):
            if name in params:
                params[name] = float(spec['armour']) / 100
        if 'ano_armor' in params:
            params['ano_armor'] = float(spec['isolation']) / 100
        result['default_modifications'] = [int(v) for v in spec['default_modifications'].split(',') if v]
        if 'hit_params' in template:
            result['hit_params'] = {
                region: {'hit_types': {kind: {'armor': params['armor' if kind in PHYSICAL else 'ano_armor']}
                                       for kind in HIT_TYPES}}
                for region in source.get('hit_params', {})}
        else:
            result.pop('hit_params', None)
    else:
        params['drop_weight'] = 0
        result['default_modifications'] = plain.get('default_modifications', [])[:]
        result['srp_premium'] = {'plain_item': int(spec['plain_id']),
                                 'bonuses': [1240, 1240 + 10 * int(spec['faction'])]}
        result['hit_params'] = copy.deepcopy(plain['hit_params'])
    for section in data_sections(result):
        section.pop('damage_protection', None)  # 0.31's; 0.26g reads hit_params
        # Masks are worn over the player's native head (owner's rule for ported headwear).
        section.pop('remove_head_model', None)
    if result['data']['type'] != 5:
        raise ValueError(f"{spec['id']}: unsupported donor equipment data type {result['data']['type']}")
    result['data']['type'] = DATA_TYPE
    result['ui_desc']['props_list'] = card(params, params['type'])
    result['ui_desc']['icon'] = [int(spec['icon_row']), int(spec['icon_col'])]
    result['ui_desc']['text_descriptions'] = text_keys(spec)
    return result


def movie_metadata(rows):
    """The movie cannot read extra item-config fields; generate its mirror."""
    entries = [f"         [{row['id']},{row['plain_id']},[1240,{1240 + 10 * int(row['faction'])}]]"
               for row in rows if row['plain_id']]
    return ('package survarium.constants\n{\n'
            '   // Generated by scripts/equipment-046/make_pack.py from items.tsv.\n'
            '   // The server reads srp_premium from the item configs in game data.\n'
            '   public class Equipment046\n   {\n'
            '      private static const ITEMS:Array = [\n' + ',\n'.join(entries) + '\n      ];\n'
            '      public static function premium(dictId:int) : Array\n      {\n'
            '         for each(var row:Array in ITEMS)\n         {\n'
            '            if(row[0] == dictId) return row;\n         }\n'
            '         return null;\n      }\n   }\n}\n')


def icon(raw):
    """A donor icon in a 171x83 atlas cell: cropped to what it draws, shrunk to fit the
    cell's 81-pixel drawing height (0.46c2 pads some to 128x128), centred."""
    from PIL import Image
    image = Image.open(io.BytesIO(raw)).convert('RGBA')
    box = image.getchannel('A').getbbox()
    if box is None:
        raise ValueError('donor icon is empty')
    image = image.crop(box)
    scale = min(1.0, (CELL_W - 2) / image.width, (CELL_H - 2) / image.height)
    if scale < 1.0:
        size = (max(1, round(image.width * scale)), max(1, round(image.height * scale)))
        image = image.convert('RGBa').resize(size, Image.Resampling.LANCZOS).convert('RGBA')
    canvas = Image.new('RGBA', (CELL_W, CELL_H), (0, 0, 0, 0))
    canvas.paste(image, ((CELL_W - image.width) // 2, (CELL_H - image.height) // 2))
    return png(canvas.tobytes())


def donor_configs(inputs):
    by_name = {row['cfg_name'].rsplit('/', 1)[1].removesuffix('.options'): row['cfg_name']
               for row in inputs.config('donor031', DICTIONARY)['items_dict']}
    equipment = inputs.config('donor046', GAME_OPTIONS)['equipment']

    def source(spec):
        if spec['donor'] == '031':
            return inputs.config('donor031', by_name[spec['id_069']])
        if spec['donor'] == '046':
            return copy.deepcopy(equipment[spec['id_069']])
        raise ValueError(f"{spec['id']}: unknown donor {spec['donor']}")
    return source


def write_pack(inputs, out):
    base = inputs.config('base', DICTIONARY)['items_dict']
    base_by_id = {r['dict_id']: r for r in base}
    rows = specs()
    inputs.read('base', MODIFICATIONS)  # the balance audit's effect values
    ids = [int(r['id']) for r in rows]
    if len(ids) != len(set(ids)) or set(ids) & base_by_id.keys():
        raise ValueError('new equipment ids are duplicated or already occupied')
    source_of = donor_configs(inputs)
    base_atlas = atlas(inputs.read('base', MOVIE))
    base_text = e029.strings(inputs.read('base', 'localization/english/localization'))
    texts = {lang: catalog(source) for lang, source in LANGUAGES.items()}
    shipped = shipped_switches()
    asset_paths = {group: inputs.group(group) for group in ('assets031', 'assets046')}
    textures = shipped_textures(path for paths in asset_paths.values() for path in paths)
    surfaces = {}
    base_files = set(inputs.group('base'))

    def released(sound):
        return sound_file(*sound) in base_files
    files, operations, entries, icons, report = {}, [], [], [], []
    for group, paths in asset_paths.items():
        for path in paths:
            raw = asset(path, inputs.read(group, path), shipped, textures)
            if path.endswith('/settings'):
                model = path.removeprefix('models/').removesuffix('.skinned_model/settings')
                surfaces[model] = bcw.loads(raw)['lod_hierrarchy']['LOD0']['surfaces']
            files['files/' + path] = raw
            operations.append({'op': 'add_file', 'path': path})
    configs = {}
    occupied = set()
    for spec in sorted(rows, key=lambda r: (bool(r['plain_id']), int(r['id']))):
        ident = int(spec['id'])
        template_row = base_by_id[int(spec['template_id'])]
        template = inputs.config('base', template_row['cfg_name'])
        source = source_of(spec)
        name_surfaces(source, surfaces)
        plain = inputs.config('base', base_by_id[int(spec['plain_id'])]['cfg_name']) if spec['plain_id'] else None
        if plain is not None and base_by_id[int(spec['plain_id'])]['item_category'] != template_row['item_category']:
            raise ValueError(f'{ident}: plain item of another category')
        config = convert(source, template, spec, released, plain)
        configs[ident] = config
        path = config_path(spec, template_row)
        if any(r['cfg_name'] == path for r in base):
            raise ValueError(f'{path}: config path already taken')
        entries.append({'cfg_name': path, 'is_stack': False, 'name': f"eq046_{spec['id_069']}",
                        'item_category': template_row['item_category'], 'dict_id': ident,
                        'base_cost': int(spec['price']), 'is_premium': bool(spec['plain_id'])})
        row, col = config['ui_desc']['icon']
        if (row, col) in occupied or (col <= base_atlas[0] // 171 and any(cell(base_atlas, row, col)[3::4])):
            raise ValueError(f'occupied target icon cell {row},{col}')
        occupied.add((row, col))
        icon_file = f'icons/{ident}.png'
        files[icon_file] = icon(inputs.read('donor' + spec['donor'],
                                            'flash_movies/textures/ui_icons/items/equipment/'
                                            f"equipment_{source['ui_desc']['icon'][1]}_{source['ui_desc']['icon'][0]}.png"))
        icons.append({'row': row, 'col': col, 'source': icon_file})
        for key in config['ui_desc']['text_descriptions'].values():
            if key in base_text:
                raise ValueError(f'{key}: text key already in the released dictionaries')
        raw = bcw.dump(config)
        if bcw.dump(bcw.loads(raw)) != raw:
            raise ValueError(f'{ident}: config roundtrip failed')
        files['files/' + path] = raw
        operations.append({'op': 'add_file', 'path': path})
        report.append({'id': ident, 'name': spec['name'], 'level': config['parameters']['item_level'],
                       'armour': spec['armour'], 'isolation': spec['isolation'], 'price': int(spec['price']),
                       'premium': bool(spec['plain_id']), 'template_id': int(spec['template_id']),
                       'donor': spec['donor'], 'donor_item': spec['id_069']})
    names = {r['name'] for r in base}
    if names & {e['name'] for e in entries}:
        raise ValueError('dictionary name collision')
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    keys = sorted(k for spec in rows for k in text_keys(spec).values())
    for lang, table in texts.items():
        missing = [k for k in keys if k not in table]
        if missing:
            raise ValueError(f'{LANGUAGES[lang]}.tsv lacks {missing[:3]}')
        operations.append({'op': 'localization_add', 'languages': [lang],
                           'strings': {k: e029.porter_text(table[k]) for k in keys}})
    operations.append({'op': 'atlas_icons_add', 'icons': icons})
    manifest = {'format_version': 1, 'id': PACK, 'version': VERSION, 'target_build': '0.26g',
                'operations': operations,
                'notes': ['0.31a2 and 0.46c2 equipment, adapted to 0.26g by scripts/equipment-046/items.tsv.']}
    files['pack.json'] = (json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode()
    files['balance.json'] = (json.dumps(report, ensure_ascii=False, indent=2) + '\n').encode()
    files['ui/Equipment046.as'] = movie_metadata(rows).encode()
    out.mkdir(parents=True, exist_ok=False)
    for name, raw in sorted(files.items()):
        dest = out / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(raw)
    print(f'{PACK}@{VERSION}: {len(entries)} items, {len(operations)} operations; pack.json {sha(files["pack.json"])}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    write_pack(Inputs(args.inputs), args.out)


if __name__ == '__main__':
    main()
