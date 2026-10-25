#!/usr/bin/env python3
"""make_pack.py: the content pack of the Sponge (0.28d0's artefact_sponge, items_dict 758, as
retail's: thrown, exploding on contact, 0.69d0's numbers and particles).

usage: python3 -I make_pack.py --stock <folder> --donor <folder> --retail <folder>
                               [--base-files <files.tsv>] --out <packs folder>/sponge-<version>

--stock       0.26g's files the pack is made from, at their game paths, as the manifest's packs
              are composed over them (the stock resources.db and the base rows' patches:
              `srp-game-data get --db <resources.db> --patch <patch>... --out <folder> <path>...`):
              every `stock/` path of inputs.sha256.
--donor       0.28d0's files, at their game paths (`vostok-port extract --db <0.28d0 resources.db>
              --path <path> --output <folder>/<path>`): every `donor/` path of inputs.sha256.
--retail      0.69d0's files (the last build; its archive's volumes as one split, `vostok-port
              extract --db <0.69d0 .split>`), at their game paths: every `retail/` path of
              inputs.sha256.
--base-files  optional: `srp-game-data files` of the same archives; with it, every material and
              texture the pack's particles and materials name must be in 0.26g or in the pack.
--out         the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
              manifest's `pack` row (release/content.tsv).

Every input is checked against inputs.sha256. What the pack holds:
- items_dict row 758 (artefact_sponge, category 12, a stack, base cost 0, as Lifebone's 54);
- gameplay/items/artefacts/artefact_sponge.options: 0.28d0's file, which exe patch 59 (0.28d0's
  artefact_sponge_core written again for 0.26g, thrown and exploding as retail's) reads, with
  retail's fuse, contact and numbers (RETAIL: 0.69d0's game.options, ammo_grenades_etc/
  artefact_sponge, in the keys and places 0.26g and the patch read), two numbers of the owner's
  static toxin rule (PASSIVE: the passive keeps the toxin pool `infection` full while the Sponge
  is owned) and the description key srp's own text (0.28d0 names Spring's);
- the model item/artefacts/artefact_sponge (0.28d0's files as they are: the model format is
  0.26g's; its material item/artefacts_sponge is 0.26g's own);
- the particles of the Sponge: the spawn's and the cloud's 0.69d0's (RETAIL_PARTICLES: retail's,
  0.46c2 on; read from the later builds' 16-byte values, read_compact, and written in 0.26g's
  24-byte layout), the others 0.28d0's (PARTICLES), all without MarginMultiplier (0.26g's
  particle loader has no such property);
- the materials of those particles 0.26g has not (MATERIALS): 0.28d0's without the parameters
  0.26g's effect has not (LATER_PARAMETERS), scroll directions as floats (0.26g's type), and
  checked to select a shader 0.26g ships: their shader switches are those of the 0.26g material
  named beside them;
- the textures of those materials 0.26g has not (TEXTURES), as 0.28d0 has them (DXT1, 0.26g's
  .options format);
- the texts in 0.26g's six languages: the name and the pick-up hint as 0.28d0 has them
  (CORRECTIONS aside), the description and the activation hint srp's own (texts.tsv).
The lobby icon is atlas cell [22, 1], which 0.26g's lobby movie already holds. The combat-log
icon (17) is drawn by the hud movie's source (srp-026g/flash/hud, CombatLogItem).
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

PACK_ID = 'sponge'
VERSION = '4'
DICT_ID = 758
CONFIG = 'gameplay/items/artefacts/artefact_sponge.options'
# The owner's static toxin rule (2026-10-22): while a Sponge is owned its passive keeps infection
# full. The passive is a regeneration modifier of the pool (exe patch 59, Lifebone's callback
# 0x312590): speed (s + regen_add) x regen_mul, timeout (t + timeout_add s) x timeout_mul,
# threshold (th + threshold_add) x threshold_mul. 0.28d0 has {0, 1, 0, 1, 1.5, 1}: regen_add
# 1000 refills the pool at the next regeneration tick, timeout_mul 0 takes the delay after a hit
# away; the threshold stays 0.28d0's (+1.5: past full).
PASSIVE = {'regen_add': (0, 1000), 'timeout_mul': (1, 0)}
# Retail's Sponge (0.69d0's game.options, ammo_grenades_etc/artefact_sponge; the owner chose its
# numbers, 2026-10-24) over 0.28d0's file: {path: (0.28d0's value, ours)}, None for a key one of
# them has not. In 0.26g's keys and places: the fuse, safe time and contact rule are
# grenade_core's (data/ttl_ms, data/safe_time, data/explode_on_contact: 0.26g's grenade cook
# 0x2E9290, the RGD-5's keys), read by exe patch 59 (as retail its own grenade_core, 0.31a2 on);
# the splash and the zone stay where 0.28d0 has them (data/active). The hit types are retail's
# (the owner, 2026-10-25: "exactly the same logic as in retail"): the explosion's and the
# cloud's damage type "fire" (0.26g has it: 0x9257E0's seventh); the cloud on retail's "health",
# the pool every part of 0.69d0's damage model feeds (its hit_params[16]), which is 0.26g's
# "body" (hit_params[0], its life: nothing feeds it, it takes "fire" itself); the explosion on
# retail's body list [18], 0.69d0's hit_params[18] "stamina", which takes only "stamina" hits
# (0.31a2's [18] was its "health"; 0.46c2 dropped two parts and [18] became "stamina"): 0.26g
# has no such part, so the list is empty and the explosion hits no one, as retail's fire on
# stamina (its stamina damage 0) does nothing. The fuse starts at the pin's pull (0.69d0's
# start_timer_on_throw false); 0.26g has no exploded_ttl_ms.
RETAIL = {
    ('data', 'activation_timeout_s'): (3, None),           # 0.28's fuse from the release
    ('data', 'ttl_ms'): (None, 3000),                       # ttl_ms 3000
    ('data', 'safe_time'): (None, 1500),                    # safe_time 1500
    ('data', 'explode_on_contact'): (None, True),           # explode_on_contact true
    ('data', 'active', 'duration_s'): (10, 15),             # duration_s 15
    ('data', 'active', 'splash_min_damage'): (0.05, 0.3),   # explosive.min_damage 0.3
    ('data', 'active', 'splash_max_damage'): (1.5, 0.3),    # explosive.max_damage 0.3
    ('data', 'active', 'splash_pierce'): (0, 0.1),          # explosive.player_pierce 0.1
    ('data', 'active', 'splash_damage_type'): ('toxic', 'fire'),           # explosive.damage_type
    ('data', 'active', 'splash_affected_bodies'): ([6, 0, 1, 7, 11, 12], []),  # explosive.affected_bodies [18]
    ('data', 'active', 'damage_zone', 'damage_type'): ('toxic', 'fire'),
    ('data', 'active', 'damage_zone', 'hit_parts_filter'): (['face'], ['body']),  # retail's ["health"]
    ('data', 'active', 'damage_zone', 'max_hit'): (1.5, 0.2),
    ('data', 'active', 'damage_zone', 'idle_particle'): ('fx/test/idle_particle', 'fx/artifacts/mochalka_explosion'),
    ('data', 'active', 'damage_zone', 'activated_particle'): ('fx/artifacts/mochalka_explosion', None),
}
# the cloud's cylinder: 5.5 m across (0.28d0 3.5)
ZONE_MESH = ((3.5, 1.5, 3.5), (5.5, 1.5, 5.5))
# 0.28d0's ui_desc names Spring's description (st_artefact_spring_description); the Sponge's own
# is texts.tsv's.
DESCRIPTION = ('st_artefact_spring_description', 'st_artefact_sponge_description')
MODEL = 'models/item/artefacts/artefact_sponge.model/'
LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')
# 0.28d0's texts, in its six languages; the description and activation hint are texts.tsv's.
DONOR_TEXTS = ('st_artefact_sponge_name', 'st_sponge_pickup_hint')
# 0.28d0's texts put right: (language, key): (its text's part, ours). English as 0.26g's other
# pick-up hints begin ("Larkspur artifact."; the site's /maps reads the name from it,
# scripts/maps-tab/build.py artefact_names); a Ukrainian typo.
CORRECTIONS = {('english', 'st_sponge_pickup_hint'): ('Artifact Sponge.', 'Sponge artifact.'),
               ('ukrainian', 'st_sponge_pickup_hint'): ('ядовидих', 'отруйних')}
PARTICLES = ('fx/artifacts/pre_mochalka2', 'fx/artifacts/pre_mochalka2_indoor', 'fx/artifacts/mochalka_idle')
# retail's (0.69d0's): the spawn's (object/particle_on_spawn; 0.46c2's change: its 1000-particle
# emitter on, a second glow) and the cloud's, the zone's idle particle for its whole time (0.31a2's
# change: 0.28d0's played it once at the zone's start and looped the test billboard
# fx/test/idle_particle; its emitters last 20 s, 0.28d0's 10)
RETAIL_PARTICLES = ('fx/artifacts/pre_mochalka1', 'fx/artifacts/mochalka_explosion')
# The materials the particles name that 0.26g has not, each with the 0.26g material whose shader
# switches (use_*, two_sided: the shader permutation) it must have.
MATERIALS = {
    'fx/artifacts/dust_hi': 'fx/artifacts/dust',
    'fx/artifacts/dust_tiled2': 'fx/artifacts/dust_tiled',
    'fx/artifacts/glow_spher_distortion': 'fx/artifacts/glow_spher',
    'fx/artifacts/glow_spher_half': 'fx/artifacts/caustic',
    'fx/artifacts/sponge_exp': 'fx/artifacts/flow_smoke',
    'fx/artifacts/sponge_exp_part': 'fx/artifacts/flow_smoke',
}
# Shader switches 0.26g ships no permutation for, taken from the template: glow_spher_half is
# one-sided in 0.28d0; 0.26g's materials of its other switches are all two-sided.
SWITCHES_FROM_TEMPLATE = {('fx/artifacts/glow_spher_half', 'two_sided')}
# Parameters of 0.28d0's forward_default that no 0.26g material has (0.26g's effect does not
# tile the maps: the tiling factors are dropped, the maps are drawn once over the particle).
LATER_PARAMETERS = ('apply_distortion_to_emissive_only', 'emissive_map_tiling_u_factor',
                    'emissive_map_tiling_v_factor', 'transparency_map_tiling_u_factor',
                    'transparency_map_tiling_v_factor', 'use_ttransparency_dxt1', 'use_uv_tiling')
FLOAT_PARAMETERS = ('emissive_map_scroll_u_direction', 'emissive_map_scroll_v_direction',
                    'transparency_map_scroll_u_direction', 'transparency_map_scroll_v_direction')
# The textures of those materials that 0.26g has not (0.26g's own dust, dust_tile, sphere and
# uv_dist1_nmap differ from 0.28d0's and stay 0.26g's).
TEXTURES = ('fx/artifacts/sphere_half', 'fx/artifacts/sponge_explosive', 'fx/artifacts/sponge_explosive_part',
            'fx/explosions/smoke_tiled')
# Textures 0.26g's own materials name that neither its resources.db nor any patch holds, named by
# the retail particles: fx/artifacts/magic_smoke (material magic_smoke) and magic_smoke1
# (magic_smoke2), the transparency of the spawn's emitters fluid1 and fluid2. Without them the
# game drew those billboards untextured, their colour ramps (red, white, blue) as squares
# (2026-10-25). 0.69d0's (DXT5; its .options of the 16-byte values written in 0.26g's 24, every
# leaf 0.69d0's); the stock materials checked to name them.
RETAIL_TEXTURES = {'fx/artifacts/magic_smoke': 'fx/artifacts/magic_smoke',
                   'fx/artifacts/magic_smoke1': 'fx/artifacts/magic_smoke2'}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class Folder:
    """Files at their game paths, checked against inputs.sha256."""

    def __init__(self, root, sums, what):
        self.root, self.sums, self.what = Path(root), sums, what

    def read(self, path):
        want = self.sums.get(f'{self.what}/{path}')
        if want is None:
            sys.exit(f'inputs.sha256 has no {self.what}/{path}')
        file = self.root / path
        if not file.is_file():
            sys.exit(f'{self.what}: no {path} in {self.root}')
        data = file.read_bytes()
        if sha256(data) != want:
            sys.exit(f'{self.what}: {path} is not the file this pack is made of (sha256 {want})')
        return data


def load_localization(data):
    """A localization file: its texts are UTF-8 (a few other strings are not: kept as bytes)."""
    def walk(node):
        kind, payload = node[0], node[1]
        if kind == bcw.STRING:
            try:
                return payload.decode('utf-8')
            except UnicodeDecodeError:
                return payload
        if kind == bcw.NAMED:
            return {key.decode('utf-8'): walk(child) for key, child in payload}
        if kind == bcw.INDEXED:
            return [walk(child) for child in payload]
        return bcw.to_py(node)
    return walk(bcw.read(data))


def porter_text(text):
    """The game keeps texts as UTF-8 bytes; the porter reads and writes a config's strings as
    Windows-1251 (WHATWG's: byte 0x98 is U+0098), so a pack names the UTF-8 bytes so read
    (research-mode's and decals-069d's make_pack.py)."""
    raw = text.encode('utf-8')
    return ''.join('\x98' if b == 0x98 else bytes([b]).decode('cp1251') for b in raw)


def kind_of(value):
    if isinstance(value, bool):
        return 'bool'
    if isinstance(value, int):
        return 'int'
    if isinstance(value, float):
        return 'float'
    if isinstance(value, str):
        return 'string'
    if isinstance(value, tuple):
        return f'float{len(value)}'
    return 'table' if isinstance(value, dict) else 'list'


def check_types(made, model, where, skip=()):
    """Every value of `made` at a path `model` has is of `model`'s type there (a float where the
    model has an int is allowed: 0.26g's loaders read numbers either way, 0x36EB01)."""
    def walk(a, b, path):
        if path in skip:
            return
        ka, kb = kind_of(a), kind_of(b)
        if ka != kb and not (ka == 'float' and kb == 'int'):
            sys.exit(f'{where}: {"/".join(path)} is {ka}; 0.26g has {kb} there')
        if isinstance(a, dict) and isinstance(b, dict):
            for key in a.keys() & b.keys():
                walk(a[key], b[key], path + (key,))
        elif isinstance(a, list) and isinstance(b, list) and b:
            # an item against the model's item in its place (a sound is [id, name]), or its first
            for i, item in enumerate(a):
                walk(item, b[i] if i < len(b) else b[0], path + (f'[{i}]',))
    walk(made, model, ())


def at_path(config, path):
    for key in path[:-1]:
        config = config[key]
    return config


def sponge_config(donor, lifebone):
    """0.28d0's artefact_sponge.options as exe patch 59 reads it: the file but RETAIL, ZONE_MESH,
    PASSIVE and DESCRIPTION."""
    config = copy.deepcopy(donor)
    for path, (was, now) in RETAIL.items():
        table = at_path(config, path)
        have = table.get(path[-1])
        if (have is None) != (was is None) or (was is not None and abs(have - was) > 1e-6 if isinstance(was, float)
                                               else have != was):
            sys.exit(f'donor {CONFIG}: {"/".join(path)} is {have!r}, not {was!r}')
        if now is None:
            del table[path[-1]]
        else:
            table[path[-1]] = now
    mesh = config['data']['active']['damage_zone']['collision_geometries'][0]['meshes'][0]
    if tuple(mesh['scale']) != ZONE_MESH[0]:
        sys.exit(f'donor {CONFIG}: the zone\'s mesh scale is {mesh["scale"]!r}')
    mesh['scale'] = ZONE_MESH[1]
    passive = config['data']['passive']
    for key, (was, now) in PASSIVE.items():
        if passive.get(key) != was or type(passive[key]) is not int:
            sys.exit(f'donor {CONFIG}: data/passive/{key} is {passive.get(key)!r}, not {was}')
        passive[key] = now
    names = config['ui_desc']['text_descriptions']
    if names.get('description') != DESCRIPTION[0]:
        sys.exit(f'donor {CONFIG}: ui_desc/text_descriptions/description is {names.get("description")!r}')
    names['description'] = DESCRIPTION[1]
    # What 0.28d0 says the Sponge is, so a later donor cannot slip in unnoticed.
    for key, want in (('type', 12), ('amount', 1), ('spawn_sec', 2), ('effect', 'artefact_spring'),
                      ('cooldown_sec', 0)):
        if donor['data'][key] != want:
            sys.exit(f'donor {CONFIG}: data/{key} is {donor["data"][key]!r}, not {want!r}')
    active = donor['data']['active']
    for key, want in (('throw_power', 15), ('splash_radius', 10), ('splash_damage_type', 'toxic')):
        if active[key] != want:
            sys.exit(f'donor {CONFIG}: data/active/{key} is {active[key]!r}, not {want!r}')
    if donor['ui_desc']['icon'] != [22, 1] or donor['ui_desc']['combat_log_icon'] != 17:
        sys.exit(f'donor {CONFIG}: not the icons [22, 1] and 17')
    # the keys every 0.26g artefact reader shares with Lifebone's are of Lifebone's types (the
    # passive and the active are the Sponge's own, read by the exe's patch alone)
    check_types(config, lifebone, 'artefact_sponge.options', skip={('data', 'passive'), ('data', 'active')})
    return config


COMPACT = 16
COMPACT_DATA = (1 << 44) - 1


def read_compact(data):
    """A binary config of the later builds (0.46c2 on, 0.69d0's: 16-byte values, resource-porter's
    binary_config.rs parse_compact) as bcw's nodes, which bcw.write lays out as 0.26g reads them:
    a value's u64 bits 0..44 its data (a scalar, or a table's, string's or vector's offset), 44..48
    its type (bcw's codes), 48..64 its count; a named child's second u64 its key's offset (bits
    0..48). The root is a table whose children start at byte 16."""
    def value(at):
        a = struct.unpack_from('<Q', data, at)[0]
        payload, kind, count = a & COMPACT_DATA, (a >> 44) & 0xF, a >> 48
        if kind == bcw.BOOL:
            return (kind, 1 if payload else 0, 4)
        if kind in (bcw.INT, bcw.FLOAT):
            return (kind, payload & 0xFFFFFFFF, 4)
        if kind == bcw.STRING:
            raw = bytes(data[payload:payload + count])
            if not raw.endswith(b'\0'):
                raise ValueError(f'a string at {at:#x} not NUL-terminated')
            return (kind, raw[:-1], count)
        if kind in (6, 7, 8):
            return (kind, bytes(data[payload:payload + 4 * (kind - 4)]), 4 * (kind - 4))
        if kind == bcw.NAMED:
            children = []
            for i in range(count):
                child = payload + COMPACT * i
                key_at = struct.unpack_from('<Q', data, child + 8)[0] & ((1 << 48) - 1)
                children.append((bytes(data[key_at:data.index(b'\0', key_at)]), value(child)))
            return (kind, children, count)
        if kind == bcw.INDEXED:
            return (kind, [value(payload + COMPACT * i) for i in range(count)], count)
        raise ValueError(f'value type {kind} at {at:#x}')
    head = struct.unpack_from('<Q', data, 0)[0]
    if head & COMPACT_DATA != COMPACT or (head >> 44) & 0xF not in (bcw.NAMED, bcw.INDEXED):
        raise ValueError('not a compact (16-byte) binary config')
    return value(0)


def strip_margin(node, counted):
    """A 0.28d0 or 0.69d0 particle without MarginMultiplier (0.26g has no such property: #225's
    convert_particle; nothing else of the particles is unknown to 0.26g's loader)."""
    kind = node[0]
    if kind == bcw.NAMED:
        children = []
        for key, child in node[1]:
            if key == b'MarginMultiplier':
                counted[0] += 1
                continue
            children.append((key, strip_margin(child, counted)))
        return (kind, children, len(children))
    if kind == bcw.INDEXED:
        return (kind, [strip_margin(child, counted) for child in node[1]], node[2])
    return node


def strings(value, key=None):
    if isinstance(value, dict):
        for name, child in value.items():
            yield from strings(child, name)
    elif isinstance(value, list):
        for child in value:
            yield from strings(child, key)
    elif isinstance(value, str):
        yield key, value


def is_switch(name):
    """A material parameter that selects the shader permutation (resource-porter's
    material_conversion.rs is_shader_feature)."""
    return name.startswith(('use_', 'with_', 'is_')) or name.endswith('_type') or \
        name in ('wind_motion', 'lighting_model', 'decal_type', 'two_sided')


def switches(material):
    """{path: value} of the shader switches that are on (off and absent are the same)."""
    out = {}

    def walk(value, path):
        if isinstance(value, dict):
            for key, child in value.items():
                walk(child, path + (key,))
        elif path[-1] == 'value' and is_switch(path[-2]) and value is not False:
            out['/'.join(path[:-1])] = value
    walk(material, ())
    return out


def convert_material(name, donor, template, log):
    material = copy.deepcopy(donor)
    stages = material['material']
    if stages['instance']['base_material'] != template['material']['instance']['base_material']:
        sys.exit(f'{name}: base material {stages["instance"]["base_material"]}, not the template\'s')
    dropped = []
    for stage, body in stages.items():
        effect = body.get('effect') if isinstance(body, dict) else None
        if effect is None:
            continue
        if effect.get('effect_id') != template['material'][stage]['effect']['effect_id']:
            sys.exit(f'{name}: {stage} effect {effect.get("effect_id")}, not the template\'s')
        for key in LATER_PARAMETERS:
            if key in effect:
                del effect[key]
                dropped.append(key)
        for key in FLOAT_PARAMETERS:
            if key in effect and isinstance(effect[key]['value'], int):
                effect[key]['value'] = float(effect[key]['value'])
    taken = []
    for path, key in sorted(SWITCHES_FROM_TEMPLATE):
        if path != name:
            continue
        for stage, body in stages.items():
            if isinstance(body, dict) and key in body:
                want = template['material'][stage][key]['value']
                if body[key]['value'] != want:
                    body[key]['value'] = want
                    taken.append(f'{stage}/{key} {want}')
    mine, theirs = switches(material), switches(template)
    if mine != theirs:
        sys.exit(f'{name}: shader switches {mine}, not those of a 0.26g material ({theirs})')
    log.append(f'material {name}: 0.28d0\'s without {", ".join(sorted(set(dropped)))}'
               + (f'; {", ".join(taken)} as {MATERIALS[name]}' if taken else '')
               + f'; the shader of {MATERIALS[name]}')
    return material


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--retail', required=True, type=Path)
    parser.add_argument('--base-files', type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    sums = {}
    for line in (HERE / 'inputs.sha256').read_text().splitlines():
        if line and not line.startswith('#'):
            sha, path = line.split(None, 1)
            sums[path.strip()] = sha
    stock = Folder(args.stock, sums, 'stock')
    donor = Folder(args.donor, sums, 'donor')
    retail = Folder(args.retail, sums, 'retail')
    files, log = {}, []

    files[CONFIG] = bcw.dump(sponge_config(bcw.loads(donor.read(CONFIG)),
                                           bcw.loads(stock.read('gameplay/items/artefacts/artefact_lifebone.options'))))
    spring = bcw.loads(stock.read('gameplay/items/artefacts/artefact_spring.options'))
    if spring['data']['effect'] != 'artefact_spring':
        sys.exit('0.26g has no effect artefact_spring (artefact_spring.options data/effect)')
    log.append(f'{CONFIG}: 0.28d0\'s but retail\'s (0.69d0) '
               + ', '.join(f'{"/".join(path)} {a} -> {b}' for path, (a, b) in RETAIL.items())
               + f', the zone\'s mesh {ZONE_MESH[0]} -> {ZONE_MESH[1]}; '
               + ', '.join(f'data/passive/{k} {a} -> {b}' for k, (a, b) in PASSIVE.items())
               + f', the description {DESCRIPTION[1]}')

    for path in sorted(p[len('donor/'):] for p in sums if p.startswith('donor/' + MODEL)):
        files[path] = donor.read(path)
    log.append(f'model item/artefacts/artefact_sponge: 0.28d0\'s {len([p for p in files if p.startswith(MODEL)])} files')

    named = {}
    for particle, source, build in [(p, donor, '0.28d0') for p in PARTICLES] + \
            [(p, retail, '0.69d0') for p in RETAIL_PARTICLES]:
        path = f'particles/{particle}.particle'
        counted = [0]
        tree = read_compact(source.read(path)) if source is retail else bcw.read(source.read(path))
        data = bcw.write(strip_margin(tree, counted))
        files[path] = data
        log.append(f'particle {particle}: {build}\'s' + (' (16-byte values, laid out in 0.26g\'s 24)' if source is retail
                                                          else '') + f', {counted[0]} MarginMultiplier taken out')
        for key, value in strings(bcw.loads(data)):
            if key == 'Material':
                named.setdefault(value, set()).add(particle)
    for name, template in MATERIALS.items():
        path = f'material_instances/{name}.material'
        material = convert_material(name, bcw.loads(donor.read(path)),
                                    bcw.loads(stock.read(f'material_instances/{template}.material')), log)
        files[path] = bcw.dump(material)
    for texture in TEXTURES:
        for ext in ('dds', 'options'):
            path = f'textures/{texture}.{ext}'
            files[path] = donor.read(path)
    for texture, material in RETAIL_TEXTURES.items():
        named_by = {v for k, v in strings(bcw.loads(stock.read(f'material_instances/{material}.material')))
                    if k == 'value'}
        if texture not in named_by:
            sys.exit(f'0.26g\'s material {material} does not name {texture}')
        files[f'textures/{texture}.dds'] = retail.read(f'textures/{texture}.dds')
        files[f'textures/{texture}.options'] = bcw.write(read_compact(retail.read(f'textures/{texture}.options')))
        log.append(f'texture {texture} (0.26g\'s material {material} names it, no archive of 0.26g has it): 0.69d0\'s')

    if args.base_files:
        base = {line.split('\t')[0] for line in args.base_files.read_text().splitlines() if '\t' in line}
        for material in sorted(named):
            path = f'material_instances/{material}.material'
            if path not in base and path not in files:
                sys.exit(f'{material}: a material of {sorted(named[material])} that neither 0.26g nor the pack has')
        for path in [p for p in files if p.startswith('material_instances/')]:
            for key, value in strings(bcw.loads(files[path])):
                if key == 'value' and '/' in value and value != 'test/test' and \
                        f'textures/{value}.dds' not in base and f'textures/{value}.dds' not in files:
                    sys.exit(f'{path}: its texture {value} is neither in 0.26g nor in the pack')
        for path in files:
            if path in base:
                sys.exit(f'{path}: 0.26g has it already (the pack only adds files)')
        log.append('every material and texture named is in 0.26g or in the pack')
    missing = sorted(set(MATERIALS) - set(named))
    if missing:
        sys.exit(f'materials no particle names: {missing}')

    texts = {key: {} for key in DONOR_TEXTS}
    for language in LANGUAGES:
        strings_of = load_localization(donor.read(f'localization/{language}/localization'))['strings']
        for key in DONOR_TEXTS:
            text = strings_of.get(key)
            if not isinstance(text, str):
                sys.exit(f'donor {language}: no {key} in UTF-8')
            if (language, key) in CORRECTIONS:
                was, now = CORRECTIONS[(language, key)]
                if text.count(was) != 1:
                    sys.exit(f'donor {language} {key}: not one {was!r}')
                text = text.replace(was, now)
            texts[key][language] = text
    header, *table = rows('texts.tsv')
    if tuple(header) != ('key',) + LANGUAGES:
        sys.exit(f'texts.tsv: the columns are key and {", ".join(LANGUAGES)}')
    for row in table:
        if len(row) != 1 + len(LANGUAGES) or not all(row):
            sys.exit(f'texts.tsv: {row[0]}: a text in each language')
        texts[row[0]] = dict(zip(LANGUAGES, row[1:]))

    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [{
        'base_cost': 0, 'cfg_name': CONFIG, 'dict_id': DICT_ID, 'is_premium': False, 'is_stack': True,
        'item_category': 12, 'name': 'artefact_sponge'}]})
    for language in LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {key: porter_text(by[language]) for key, by in texts.items()}})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the Sponge (items_dict 758, 0.28d0\'s artefact_sponge as retail\'s): its config (0.28d0\'s '
                       'with 0.69d0\'s fuse, contact and numbers, the passive keeping infection full), model, '
                       'particles (the spawn\'s and the cloud\'s 0.69d0\'s), materials, textures and texts',
        'operations': operations,
        'notes': ['made by srp-026g/scripts/sponge/make_pack.py from inputs.sha256 and texts.tsv',
                  'donor: Survarium 0.28d0; retail: Survarium 0.69d0'] + log,
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print('\n'.join(log))
    print(f'{args.out}: {len(files)} files, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
