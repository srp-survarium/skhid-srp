#!/usr/bin/env python3
"""make_pack.py: the content pack of the muzzle devices (docs/notes/attachments-muzzles.md).

usage: python3 -I make_pack.py --inputs <folder> --modules <folder> [--record <file>] [--icons <folder>]
                               --out <packs folder>/muzzle-devices-<version>

--inputs   prepare.sh's folder: 0.69d0's game.options, 0.46c2's weapon shot effects, the players' files.
--modules  the weapon-modules pack's folder (its devices' models and the modules table).
--record   port-modules' record of that pack (modules-069d.json): every device has a gun that takes
           it, and every muzzle fit of the table has its item.
--icons    render_icons.py's folder, <item>.png for every device. Without it the pack draws none
           (a local test: its id is muzzle-devices-noicons).
--out      the pack's folder (a new one).

What the pack holds, for each device of devices.tsv (devices.py makes it again and refuses a stale one):
- its item: a dictionary row (category 22, as the scopes; its price as base_cost) and a config whose
  modification, on the weapon it is fitted to, adds data/srp_muzzle = the item's own dict id (what
  client patch 161 draws by, docs/notes/attachments.md), sets particles/shoot to its shot effect when
  0.69d0 gives it one, and scales the weapon's own values by its stats (type 2);
- its lobby icon, LobbyMenu_AmmoIcons columns 5 to 7 (22 a column, in the table's order).
And the shot effects the devices name that players' files lack, from 0.46c2 (the same effects under
the same names, in 0.26g's own material vocabulary; 0.69d0's use renamed material parameters):
- a particle: 0.46c2's tree in 0.26g's 24-byte layout; an action of a type 0.26g's loader has no
  case for (construct_action 0x364F10: 14, 16, 19..22) is taken out when 0.46c2 has it disabled,
  and stops the build when not; every node has the properties every live node of its type has;
- a material a particle names that players lack: 0.46c2's, without the parameters no live material
  of the same base material and effect has (all at their neutral values but weapon_fire's angle
  attenuation, which 0.26g's fire effect lacks); its shader switches must then be a live material's
  of that base and effect (the permutation exists), with two_sided taken on when only that is missing;
- a texture such a material names that players lack: 0.46c2's .dds as it is, its .options in
  0.26g's layout.
The texts are in client-patches/localization/*.tsv (texts.py).
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402
import devices as dev  # noqa: E402

PACK_ID = 'muzzle-devices'
VERSION = '2'
CATEGORY = 22
CONFIGS = 'gameplay/items/muzzle_devices/'
ICON_ATLAS = 'LobbyMenu_AmmoIcons'
# an item's own modification: this and its dict id (the attachments' one formula, attachments.md)
MODIFICATIONS = 994000
ICON_COLUMNS, ICON_ROWS = (5, 6, 7), 22
# modifier types (the modification's apply 0x358560): the value set, a share of the config's own
# added (base x max(0, 1 + sum)), a key added to a table
SET, SHARE, ADD_KEY = 0, 2, 3
# construct_action 0x364F10 has no case for these type indexes: the action is left out silently
NO_CASE = {14, 16, 19, 20, 21, 22}
DONOR_EFFECTS = 'fx/weapons/attach/'
# A shader switch 0.46c2 has on that no live material of the base has at all: 0.26g's fire effect has
# no angle attenuation (0.46c2 added it to weapon_fire). Any other such switch that is on stops the build.
LATER_SWITCHES = {('weapon_fire', 'use_angle_depended_attenuation')}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class Donor:
    """The donor files of prepare.sh's folder (its d069 and d046), each checked against inputs.sha256."""

    def __init__(self, root, write):
        self.root, self.write, self.read_sums = Path(root), write, {}
        self.sums = {}
        path = HERE / 'inputs.sha256'
        if path.exists():
            for line in path.read_text().splitlines():
                if line and not line.startswith('#'):
                    sha, name = line.split(None, 1)
                    self.sums[name.strip()] = sha

    def read(self, name):
        file = self.root / name
        if not file.is_file():
            sys.exit(f'the donor has no {name} ({self.root})')
        data = file.read_bytes()
        self.read_sums[name] = sha256(data)
        if not self.write and self.sums.get(name) != self.read_sums[name]:
            sys.exit(f'{name} is not the file this pack is made of (inputs.sha256; --write-sums takes the folder\'s)')
        return data

    def save(self):
        lines = ['# The donor files make_pack.py reads (prepare.sh\'s folder): 0.69d0\'s game.options, 0.46c2\'s effects.']
        lines += [f'{sha}  {name}' for name, sha in sorted(self.read_sums.items())]
        (HERE / 'inputs.sha256').write_text('\n'.join(lines) + '\n')


def nodes(value):
    """Every particle node (a table with a type_index) of a tree."""
    if isinstance(value, dict):
        if 'type_index' in value:
            yield value
        for child in value.values():
            yield from nodes(child)
    elif isinstance(value, list):
        for child in value:
            yield from nodes(child)


def strings(value, key=None):
    if isinstance(value, dict):
        for name, child in value.items():
            yield from strings(child, name)
    elif isinstance(value, list):
        for child in value:
            yield from strings(child, key)
    elif isinstance(value, str):
        yield key, value


def without_uncased(node, name, log):
    """A particle's tree without the actions 0.26g's loader has no case for, which must be disabled."""
    children = node.get('children')
    if not isinstance(children, dict):
        return
    for key in [k for k, child in children.items() if isinstance(child, dict) and child.get('type_index') in NO_CASE]:
        child = children[key]
        if child['properties'].get('Enabled') is not False:
            sys.exit(f'{name}: an enabled {child["particle_entity_type"]} (type {child["type_index"]}), '
                     'which 0.26g has no case for')
        del children[key]
        order = node.get('children_order')
        if isinstance(order, list) and key in order:
            order.remove(key)
        log.append(f'particle {name}: its disabled {child["particle_entity_type"]} (type {child["type_index"]}, '
                   'no case in 0.26g) taken out')
    for child in children.values():
        if isinstance(child, dict):
            without_uncased(child, name, log)


def live_properties(live):
    """{type_index: the properties every live node of that type has}."""
    common = {}
    for path in sorted((live / 'particles').rglob('*.particle')):
        for node in nodes(bcw.loads(path.read_bytes())):
            have = set(node.get('properties', {}))
            index = node['type_index']
            common[index] = have if index not in common else common[index] & have
    return common


def convert_particle(name, data, common, log):
    tree = bcw.loads(data)
    for lod in [key for key in tree if key.startswith('LOD ')]:
        without_uncased(tree[lod], name, log)
    for node in nodes(tree):
        index = node['type_index']
        if index not in common:
            sys.exit(f'{name}: a {node.get("particle_entity_type")} (type {index}); no live particle has one')
        lacks = common[index] - set(node.get('properties', {}))
        if lacks:
            sys.exit(f'{name}: its {node.get("particle_entity_type")} lacks {sorted(lacks)}, which every live one has')
    return bcw.dump(tree), sorted({value for key, value in strings(tree) if key == 'Material'})


def is_switch(name):
    """A material parameter that selects the shader permutation (resource-porter's
    material_conversion.rs is_shader_feature)."""
    return name.startswith(('use_', 'with_', 'is_')) or name.endswith('_type') or \
        name in ('wind_motion', 'lighting_model', 'decal_type', 'two_sided')


class Shape:
    """What of a material picks its shader: the base material, each stage's effect, the switches
    that are on; and every parameter it has."""

    def __init__(self, material):
        stages = material['material']
        self.base = stages['instance']['base_material']
        self.effects, self.switches, self.keys = {}, set(), set()
        for stage, body in stages.items():
            if not isinstance(body, dict):
                continue
            tables = [('', body)]
            if isinstance(body.get('effect'), dict):
                self.effects[stage] = body['effect'].get('effect_id')
                tables.append(('effect/', body['effect']))
            for prefix, table in tables:
                for key, value in table.items():
                    if isinstance(value, dict) and 'value' in value:
                        self.keys.add(f'{stage}/{prefix}{key}')
                        if is_switch(key) and value['value'] is not False:
                            self.switches.add(f'{stage}/{prefix}{key}')

    def kin(self, other):
        return (self.base, self.effects) == (other.base, other.effects)


def live_shapes(live):
    return {str(path.relative_to(live / 'material_instances'))[:-len('.material')]: Shape(bcw.loads(path.read_bytes()))
            for path in sorted((live / 'material_instances').rglob('*.material'))}


def convert_material(name, data, shapes, log):
    material = bcw.loads(data)
    kin = [shape for shape in shapes.values() if shape.kin(Shape(material))]
    if not kin:
        sys.exit(f'material {name}: no live material of its base material and effects')
    known = set().union(*(shape.keys for shape in kin))
    dropped = []
    for key in sorted(Shape(material).keys - known):
        stage, *rest = key.split('/')
        table = material['material'][stage]
        if rest[0] == 'effect':
            table = table['effect']
        value = table.pop(rest[-1])['value']
        if is_switch(rest[-1]) and value is not False and (Shape(material).base, rest[-1]) not in LATER_SWITCHES:
            sys.exit(f'material {name}: its switch {rest[-1]} is on and no live material of its base has it')
        dropped.append(f'{rest[-1]} {value:g}' if isinstance(value, float) else f'{rest[-1]} {value}')
    shape = Shape(material)
    taken = ''
    if not any(shape.switches == other.switches for other in kin):
        stage = material['material'].get('forward', {})
        if 'two_sided' in stage and any(shape.switches | {'forward/two_sided'} == other.switches for other in kin):
            stage['two_sided']['value'] = True
            shape = Shape(material)
            taken = '; two_sided on, as every live material of its other switches'
        else:
            sys.exit(f'material {name}: no live material of {shape.base} has its shader switches {sorted(shape.switches)}')
    lacks = set.intersection(*(other.keys for other in kin if other.switches == shape.switches)) - shape.keys
    if lacks:
        sys.exit(f'material {name}: lacks {sorted(lacks)}, which every live material of its shader has')
    log.append(f'material {name} ({shape.base}): 0.46c2\'s' + (f' without {", ".join(dropped)}' if dropped else '') + taken)
    textures = sorted({value for key, value in strings(material) if key == 'value' and '/' in value})
    return bcw.dump(material), textures


def item_config(device, cell):
    mods = [{'path': ['data'], 'value': ['srp_muzzle', device['dict_id']], 'type': ADD_KEY}]
    if device['shoot']:
        mods.append({'path': ['particles', 'shoot'], 'value': device['shoot'], 'type': SET})
    mods += [{'path': key.split('/'), 'value': float(value), 'type': SHARE} for key, value in dev.modifiers(device['stats'])]
    modification = MODIFICATIONS + device['dict_id']
    stem = device['item'].removeprefix('srp_')
    return {
        'parameters': {'weight': 0.0, 'modification': modification},
        'modifications': [{'id': modification, 'modifiers': mods}],
        'ui_desc': {'text_descriptions': {'name': f'st_{stem}_name', 'description': f'st_{stem}_description'},
                    'icon': list(cell)},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--inputs', required=True, type=Path)
    parser.add_argument('--modules', required=True, type=Path)
    parser.add_argument('--record', type=Path)
    parser.add_argument('--icons', type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--write-sums', action='store_true')
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    live = args.inputs / 'live'
    have = set((live / 'paths.txt').read_text().split('\n'))
    files, log = {}, []

    donor = Donor(args.inputs, args.write_sums)
    found = dev.devices(dev.Game(donor.read('d069/gameplay/game.options')))
    table = HERE / 'devices.tsv'
    if not table.exists() or table.read_text(encoding='utf-8') != dev.table(found):
        sys.exit('devices.tsv is not what 0.69d0\'s data and the rules give (devices.py --write)')

    # the shot effects
    common, shapes = live_properties(live), live_shapes(live)
    materials = {}
    for effect in sorted({d['shoot'] for d in found if d['shoot']}):
        path = f'particles/{effect}.particle'
        if path in have:
            if effect.startswith(DONOR_EFFECTS):
                sys.exit(f'{path}: players have it already')
            log.append(f'particle {effect}: the players\' own')
            continue
        if not effect.startswith(DONOR_EFFECTS):
            sys.exit(f'{effect}: neither the players\' nor one of 0.46c2\'s {DONOR_EFFECTS}')
        files[path], named = convert_particle(effect, donor.read(f'd046/{path}'), common, log)
        log.append(f'particle {effect}: 0.46c2\'s, in 0.26g\'s layout')
        for material in named:
            materials.setdefault(material, []).append(effect)
    textures = {}
    for material, users in sorted(materials.items()):
        path = f'material_instances/{material}.material'
        if path in have:
            continue
        files[path], named = convert_material(material, donor.read(f'd046/{path}'), shapes, log)
        for texture in named:
            textures.setdefault(texture, []).append(material)
    for texture, users in sorted(textures.items()):
        if f'textures/{texture}.dds' in have:
            continue
        files[f'textures/{texture}.dds'] = donor.read(f'd046/textures/{texture}.dds')
        files[f'textures/{texture}.options'] = bcw.write(bcw.read(donor.read(f'd046/textures/{texture}.options')))
        log.append(f'texture {texture} (of {", ".join(users)}): 0.46c2\'s')

    # the items
    dictionary = bcw.loads((live / 'gameplay/db_static_dictionaries').read_bytes())
    taken = {key: {row[key] for row in dictionary['items_dict']} for key in ('dict_id', 'name', 'cfg_name')}
    stock = bcw.loads((live / 'gameplay/items/modifications/modifications.options').read_bytes())
    modification_ids = {entry['id'] for entry in stock['modifications'] + stock.get('defects', [])}
    if len(found) > len(ICON_COLUMNS) * ICON_ROWS:
        sys.exit(f'{len(found)} devices: more than the icon columns {ICON_COLUMNS} hold')
    entries, icons = [], []
    for index, device in enumerate(found):
        cell = (1 + index % ICON_ROWS, ICON_COLUMNS[index // ICON_ROWS])
        config = item_config(device, cell)
        path = f'{CONFIGS}{device["item"]}.options'
        row = {'base_cost': device['price'], 'cfg_name': path, 'dict_id': device['dict_id'], 'is_premium': False,
               'is_stack': False, 'item_category': CATEGORY, 'name': device['item']}
        for key in taken:
            if row[key] in taken[key]:
                sys.exit(f'{device["item"]}: {key} {row[key]} is taken')
        if config['parameters']['modification'] in modification_ids:
            sys.exit(f'{device["item"]}: modification {config["parameters"]["modification"]} is taken')
        if not (args.modules / 'files/models' / f'{device["model"]}.model/settings').is_file():
            sys.exit(f'{device["item"]}: the modules pack has no model {device["model"]}')
        files[path] = bcw.dump(config)
        entries.append(row)
        if args.icons:
            if not (args.icons / f'{device["item"]}.png').is_file():
                sys.exit(f'{args.icons}: no {device["item"]}.png')
            icons.append({'row': cell[0], 'col': cell[1], 'source': f'icons/{device["item"]}.png'})
    log.append(f'{len(found)} devices: ' + ', '.join(
        f'{sum(d["family"] == family for d in found)} {family}' for family in sorted({d['family'] for d in found})))

    for path, data in files.items():
        if path in have:
            sys.exit(f'{path}: players have it already (the pack only adds files)')
        if path.endswith('.material'):
            for key, value in strings(bcw.loads(data)):
                if key == 'value' and '/' in value and f'textures/{value}.dds' not in have | set(files):
                    sys.exit(f'{path}: its texture {value} is neither the players\' nor the pack\'s')
        if path.endswith('.particle'):
            for key, value in strings(bcw.loads(data)):
                if key == 'Material' and f'material_instances/{value}.material' not in have | set(files):
                    sys.exit(f'{path}: its material {value} is neither the players\' nor the pack\'s')
    if args.record:
        record = json.loads(args.record.read_text())
        if record['table_bytes'] != (args.modules / 'files/gameplay/weapon_modules.srpm').stat().st_size:
            sys.exit(f'{args.record} is not the record of {args.modules} (the table\'s size)')
        fits = {}
        for gun, entry in record['guns'].items():
            for fit in entry['plan']['fits']:
                if fit['kind'] == 'muzzle':
                    fits.setdefault(fit['module'], []).append(gun)
        left_out = {module for module, _ in dev.rows('left_out.tsv')}
        for device in found:
            if device['module'] not in fits:
                sys.exit(f'{device["item"]}: no gun of the modules table takes it')
        stray = sorted(set(fits) - {d['module'] for d in found} - left_out)
        if stray:
            sys.exit(f'the modules table fits muzzle devices without an item: {stray}')
        log.append(f'{sum(len(fits[d["module"]]) for d in found)} fits on '
                   f'{len({gun for d in found for gun in fits[d["module"]]})} guns in the modules table; '
                   f'left out, in the table without an item: {", ".join(sorted(left_out & set(fits))) or "none"}')
    if args.write_sums:
        donor.save()

    contents = dict(files)
    for icon in icons:
        contents[icon['source']] = (args.icons / Path(icon['source']).name).read_bytes()
    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    if icons:
        operations.append({'op': 'atlas_icons_add', 'symbol': ICON_ATLAS, 'icons': icons})
    pack = {
        'format_version': 1,
        'id': PACK_ID if args.icons else PACK_ID + '-noicons',
        'version': VERSION,
        'target_build': '0.26g',
        'description': f'the muzzle devices, {len(found)} items (dict ids {found[0]["dict_id"]} to {found[-1]["dict_id"]}): '
                       'configs whose modification fits the device (data/srp_muzzle), sets its shot effect and '
                       'scales the weapon by its stats; dictionary rows; lobby icons; the shot effects '
                       '(particles, materials, textures) from 0.46c2',
        'operations': operations,
        'notes': ['made by srp-026g/scripts/muzzle-devices/make_pack.py from devices.tsv and inputs.sha256',
                  'devices and stats: Survarium 0.69d0; shot effects: Survarium 0.46c2',
                  # the manifest's row names pack.json's sha256: so it names the files' bytes too
                  'files: ' + sha256('\n'.join(f'{sha256(data)} {path}' for path, data in sorted(contents.items())).encode())]
                 + log,
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    for icon in icons:
        target = args.out / icon['source']
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(contents[icon['source']])
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print('\n'.join(log))
    print(f'{args.out}: {len(files)} files, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{pack["id"]}@{VERSION}\t{digest}')


if __name__ == '__main__':
    main()
