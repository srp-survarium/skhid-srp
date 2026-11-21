#!/usr/bin/env python3
"""make_pack.py: the content pack of the muzzle devices (docs/notes/attachments-muzzles.md).

usage: python3 -I make_pack.py --inputs <folder> --modules <folder> --record <file> [--icons <folder>]
                               --out <packs folder>/muzzle-devices-<version> [--write-scale]

--inputs   prepare.sh's folder: 0.69d0's game.options, 0.46c2's weapon shot effects, the players' files.
--modules  the weapon-modules pack's folder (its devices' models and the modules table).
--record   port-modules' record of that pack (modules-069d.json): every device has a gun that takes
           it, and every muzzle fit of the table has its item; every key a device's modification
           touches is in the config of every gun that takes it; an effect's family is its fits'.
--icons    render_icons.py's folder, <item>.png for every device. Without it the pack draws none
           (a local test: its id is muzzle-devices-noicons).
--out      the pack's folder (a new one).

What the pack holds, for each device of devices.tsv (devices.py makes it again and refuses a stale one):
- its item: a dictionary row (category 22, as the scopes; its price as base_cost) and a config of a
  shop scope item's shape (its `data` names the device's model; `ui_desc/props_list` empty) whose
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
  a whole number where every live material has a float is written as that float;
- a texture such a material names that players lack: 0.46c2's .dds as it is, its .options in
  0.26g's layout.
The effects are then put on 0.26g's scale (effect_scale.tsv, made here and refused when stale; the
note's "The scale"): 0.46c2's own stock flashes are dimmer than 0.26g's, so an effect copied as it
is would be dimmer against a gun's stock flash than 0.46c2 made it. Each effect's family is the
stock flash most of its fits replace (--record); its lights are multiplied by that family's light
peak in 0.26g over 0.46c2's, and the fire materials the pack brings by the one factor the
families' own fire materials share. An effect without a light or fire gets none.
The texts are in client-patches/localization/*.tsv (texts.py).
"""
import argparse
import collections
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
VERSION = '3'
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
# Why rules.tsv's `as` rows are right: 0.69d0's key, 0.26g's key for it and the value from which more
# of it changes nothing. The slow from a hit is -min(the last 0.5 s' stopping power, 0.5) (the
# stagger 0x2DBCD0, docs/notes/client_patch.md 3.51).
DEAD = {'parameters/movement_speed_damage': ('parameters/stopping_power', 0.5)}
# A shader switch 0.46c2 has on that no live material of the base has at all: 0.26g's fire effect has
# no angle attenuation (0.46c2 added it to weapon_fire). Any other such switch that is on stops the build.
LATER_SWITCHES = {('weapon_fire', 'use_angle_depended_attenuation')}
# The scale: a particle's light action, the base material of a flash's fire, its brightness
LIGHT, FIRE, EMISSIVE = 34, 'weapon_fire', 'constant_emissive_multiplier'


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
        self.effects, self.switches, self.keys, self.values = {}, set(), set(), {}
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
                        self.values[f'{stage}/{prefix}{key}'] = value
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
    # a number 0.46c2 writes whole where every live material has a float there is that float
    floats = []
    for key, value in sorted(shape.values.items()):
        mine = type(value['value'])
        theirs = {type(other.values[key]['value']) for other in kin if key in other.values}
        if mine is int and theirs == {float}:
            value['value'] = float(value['value'])
            floats.append(key.split('/')[-1])
        elif mine not in theirs:
            sys.exit(f'material {name}: {key} is a {mine.__name__}, the live materials\' a '
                     f'{" or ".join(sorted(t.__name__ for t in theirs))}')
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
    log.append(f'material {name} ({shape.base}): 0.46c2\'s' + (f' without {", ".join(dropped)}' if dropped else '')
               + (f'; as floats {", ".join(floats)}' if floats else '') + taken)
    textures = sorted({value for key, value in strings(material) if key == 'value' and '/' in value})
    return bcw.dump(material), textures


def enabled_nodes(value):
    """Every enabled particle node of a tree; nothing under a disabled one."""
    if isinstance(value, dict):
        if 'type_index' in value:
            if value.get('properties', {}).get('Enabled') is False:
                return
            yield value
        for key, child in value.items():
            if key != 'properties':
                yield from enabled_nodes(child)


def curve_keys(value):
    """The keys of a property's curve (each has a position and two tangents, x and the value)."""
    if isinstance(value, dict):
        if 'position' in value:
            yield value
        else:
            for child in value.values():
                yield from curve_keys(child)


def light_peak(tree):
    """The largest intensity of a particle's lights, or None when it has no light."""
    peaks = [key['position'][1] for node in enabled_nodes(tree) if node['type_index'] == LIGHT
             for key in curve_keys(node['properties']['Intensity'])]
    return max(peaks) if peaks else None


def scale_lights(tree, factor):
    for node in enabled_nodes(tree):
        if node['type_index'] == LIGHT:
            for key in curve_keys(node['properties']['Intensity']):
                for point in ('position', 'left_tangent', 'right_tangent'):
                    key[point] = (key[point][0], key[point][1] * factor)


def fire_materials(tree):
    """The materials an effect's enabled emitters name."""
    return sorted({node['properties']['Material'] for node in enabled_nodes(tree) if node['properties'].get('Material')})


def emissive(material):
    """A fire material's brightness entry ({'value': ...}), or None for another kind of material."""
    stages = material['material']
    if stages['instance']['base_material'] != FIRE:
        return None
    found = [table[EMISSIVE] for body in stages.values() if isinstance(body, dict)
             for table in (body, body.get('effect')) if isinstance(table, dict) and EMISSIVE in table]
    if len(found) != 1:
        sys.exit(f'a {FIRE} material with {len(found)} {EMISSIVE}')
    return found[0]


def family_of(effect, replaced):
    """The stock flash most of an effect's fits replace; `replaced` counts them by stock flash."""
    (family, most), *others = replaced.most_common()
    if others and others[0][1] == most:
        sys.exit(f'{effect}: as many of its fits replace {others[0][0]} as {family}: no family (the scale)')
    return family, most


def scale_of(effects, replaced, stock, donor_stock, stock_material, donor_material, own):
    """The scale of 0.46c2's effects on 0.26g's.

    effects: {effect: its 0.46c2 tree}; replaced: {effect: Counter of the stock flashes its fits
    replace}; stock, donor_stock: a stock flash's tree here and in 0.46c2; stock_material,
    donor_material: a material here and in 0.46c2, or None; own: whether players have a material.
    Returns (families, by_effect, fire): {family: (light here, light there, {fire material:
    (brightness here, there)})}, {effect: (family, its fits, all fits, light factor or None)}, and
    the one factor of the fire materials.
    """
    by_effect, families, ratios = {}, {}, set()
    # a fire material the effects take as the players' own is on 0.26g's scale already: no measure of it
    taken = {name for tree in effects.values() for name in fire_materials(tree) if own(name)}
    for effect, tree in sorted(effects.items()):
        family, most = family_of(effect, replaced[effect])
        if family not in families:
            here, there = stock(family), donor_stock(family)
            fire = {}
            for name in sorted(set(fire_materials(here)) & set(fire_materials(there))):
                ours, theirs = stock_material(name), donor_material(name)
                if ours is None or theirs is None or emissive(ours) is None or emissive(theirs) is None:
                    continue
                fire[name] = (emissive(ours)['value'], emissive(theirs)['value'])
            families[family] = (light_peak(here), light_peak(there), fire)
        here, there, fire = families[family]
        factor = None
        if light_peak(tree) is not None:
            if not here or not there:
                sys.exit(f'{effect} has a light and its family {family} has none in both builds: no factor')
            factor = here / there
        by_effect[effect] = (family, most, sum(replaced[effect].values()), factor)
        ratios |= {ours / theirs for name, (ours, theirs) in fire.items() if name not in taken}
    if len(ratios) != 1:
        sys.exit(f'the families\' fire materials are not brighter here by one factor: {sorted(ratios)} '
                 '(the pack\'s fire materials are shared between families)')
    return families, by_effect, ratios.pop()


def scale_table(families, by_effect, fire, peaks, scaled):
    """effect_scale.tsv: the families' numbers, each effect's family and light, the fire materials."""
    lines = ['# The scale of 0.46c2\'s shot effects on 0.26g\'s (make_pack.py --write-scale; never edited by hand;',
             '# docs/notes/attachments-muzzles.md, "The scale"). family: a stock flash, its light\'s peak in 0.26g and in',
             '# 0.46c2, their ratio, its fire materials both builds have (brightness here/there). effect: its family (the',
             '# stock flash most of its fits replace), those fits of all, its light\'s peak in 0.46c2 and in the pack.',
             '# material: a fire material the pack brings, its brightness in 0.46c2 and in the pack.']
    for family, (here, there, materials) in sorted(families.items()):
        lines.append('\t'.join(['family', family, f'{here:.4f}' if here else '-', f'{there:.4f}' if there else '-',
                                f'{here / there:.4f}' if here and there else '-',
                                ', '.join(f'{name} {ours:g}/{theirs:g}' for name, (ours, theirs) in materials.items()) or '-']))
    for effect, (family, most, total, factor) in sorted(by_effect.items()):
        lines.append('\t'.join(['effect', effect, family, f'{most} of {total}',
                                f'{peaks[effect]:.4f}' if factor else '-', f'{peaks[effect] * factor:.4f}' if factor else '-']))
    for name, (before, after) in sorted(scaled.items()):
        lines.append('\t'.join(['material', name, f'{before:g}', f'{after:g}']))
    return '\n'.join(lines) + f'\n# fire materials: x{fire:g}\n'


def at(config, key):
    """The value at a key path of a config, or None."""
    for part in key.split('/'):
        config = config.get(part) if isinstance(config, dict) else None
    return config


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
                    'icon': list(cell), 'props_list': {}},
        # a shop scope item's shape, as the grips' items; props_list is read of every item with a ui_desc at
        # the client's start, and a missing one crashes it (docs/notes/attachments-muzzles.md)
        'data': {'idle_model': device['model'], 'aimed_model': device['model'], 'type': 5, 'fov_factor': 1.0,
                 'near_plane_factor': 0.5, 'change_scope_factor': 1.0, 'hide_weapon_on_aim': False},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--inputs', required=True, type=Path)
    parser.add_argument('--modules', required=True, type=Path)
    parser.add_argument('--record', required=True, type=Path)
    parser.add_argument('--icons', type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--write-sums', action='store_true')
    parser.add_argument('--write-scale', action='store_true')
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

    # the modules table's muzzle fits and each gun's config
    dictionary = bcw.loads((live / 'gameplay/db_static_dictionaries').read_bytes())
    configs = {row['dict_id']: row['cfg_name'] for row in dictionary['items_dict']}
    record = json.loads(args.record.read_text())
    if record['table_bytes'] != (args.modules / 'files/gameplay/weapon_modules.srpm').stat().st_size:
        sys.exit(f'{args.record} is not the record of {args.modules} (the table\'s size)')
    fits = {}
    for gun, entry in record['guns'].items():
        for fit in entry['plan']['fits']:
            if fit['kind'] == 'muzzle':
                fits.setdefault(fit['module'], []).append(gun)
    weapons = {}
    for item, (dict_id, gun) in record['items'].items():
        weapons[gun] = (item, bcw.loads((live / configs[dict_id]).read_bytes()))

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

    # the scale: the effects' lights and fire on 0.26g's stock flashes' scale
    brought = {path[len('particles/'):-len('.particle')]: bcw.loads(data)
               for path, data in files.items() if path.endswith('.particle')}
    replaced = {effect: collections.Counter(at(weapons[gun][1], 'particles/shoot') for device in found
                                            if device['shoot'] == effect for gun in fits.get(device['module'], []))
                for effect in brought}

    def loaded(root, name, read):
        return bcw.loads(read(name)) if (root / name).is_file() else None
    families, by_effect, fire = scale_of(
        brought, replaced,
        lambda family: bcw.loads((live / f'particles/{family}.particle').read_bytes()),
        lambda family: bcw.loads(donor.read(f'd046/particles/{family}.particle')),
        lambda name: loaded(live, f'material_instances/{name}.material', lambda path: (live / path).read_bytes()),
        lambda name: loaded(args.inputs, f'd046/material_instances/{name}.material', donor.read),
        lambda name: f'material_instances/{name}.material' in have)
    peaks = {effect: light_peak(tree) for effect, tree in brought.items()}
    for effect, (family, _, _, factor) in by_effect.items():
        if factor is None:
            continue
        scale_lights(brought[effect], factor)
        files[f'particles/{effect}.particle'] = bcw.dump(brought[effect])
        written = light_peak(bcw.loads(files[f'particles/{effect}.particle']))
        if abs(written - peaks[effect] * factor) > 1e-6 * written:
            sys.exit(f'{effect}: its light is {written:g} as written, {peaks[effect] * factor:g} by the scale')
    scaled = {}
    for path in sorted(path for path in files if path.endswith('.material')):
        material = bcw.loads(files[path])
        entry = emissive(material)
        if entry is None:
            continue
        before, after = entry['value'], entry['value'] * fire
        entry['value'] = int(after) if isinstance(before, int) and float(after).is_integer() else after
        files[path] = bcw.dump(material)
        scaled[path[len('material_instances/'):-len('.material')]] = (before, entry['value'])
    text = scale_table(families, by_effect, fire, peaks, scaled)
    table = HERE / 'effect_scale.tsv'
    if args.write_scale:
        table.write_text(text, encoding='utf-8')
    elif not table.exists() or table.read_text(encoding='utf-8') != text:
        sys.exit('effect_scale.tsv is not what the inputs give (make_pack.py --write-scale)')
    log.append('the scale (effect_scale.tsv): lights ' + ', '.join(
        f'{effect.rsplit("/", 1)[-1]} x{factor:.2f} ({family.rsplit("/", 1)[-1]})' if factor else f'{effect.rsplit("/", 1)[-1]} none'
        for effect, (family, _, _, factor) in sorted(by_effect.items())) + f'; {len(scaled)} fire materials x{fire:g}')

    # the items
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
    left_out = {module for module, _ in dev.rows('left_out.tsv')}
    for device in found:
        if device['module'] not in fits:
            sys.exit(f'{device["item"]}: no gun of the modules table takes it')
        touched = [key for key, _ in dev.modifiers(device['stats'])] + (['particles/shoot'] if device['shoot'] else [])
        for item, config in (weapons[gun] for gun in fits[device['module']]):
            for key in touched + ['data']:
                if at(config, key) is None:
                    sys.exit(f'{device["item"]} on {item}: the weapon has no {key}')
            for key in device['instead']:
                ours, cap = DEAD[key]
                if at(config, ours) < cap:
                    sys.exit(f'{device["item"]} on {item}: {ours} {at(config, ours):g} is under {cap:g}: '
                             f'{key} is not dead there (rules.tsv)')
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
                       '(particles, materials, textures) from 0.46c2, on 0.26g\'s scale',
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
