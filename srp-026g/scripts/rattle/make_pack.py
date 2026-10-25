#!/usr/bin/env python3
"""make_pack.py: the content pack of the Rattle (0.20e's artefact_rattle, items_dict 759).

usage: python3 -I make_pack.py --stock <folder> --donor <folder> --out <packs folder>/rattle-<version>

--stock  0.26g's files the pack is made from, at their game paths (`vostok-port extract --db
         <0.26g resources.db> --path <path> --output <folder>/<path>`, or `srp-game-data get` of
         the stock db and the base rows' patches): every `stock/` path of inputs.sha256.
--donor  0.20e's files, at their game paths (`vostok-port extract --db <0.20e resources.db>
         --path <path> --output <folder>/<path>`): every `donor/` path of inputs.sha256.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

Every input is checked against inputs.sha256. What the pack holds (docs/notes/rattle.md):
- items_dict row 759 (artefact_rattle, category 12, a stack, base cost 0, as Spring's 74);
- gameplay/items/artefacts/artefact_rattle.options: 0.20e's file, which exe patch 59 (the Rattle's
  artefact_rattle_core) reads, made 0.26g's (CONFIG below): the duration under data/active, the
  owner's numbers for the sound factors in place of 0.20e's unread step volumes, cooldown 40 s,
  amount -1 (0.20e's 1 would use it up at its first active's end: artefact_base::decrement_amount
  0x313530), 0.26g's lobby cell [21, 1], Lifebone's spawn particle (0.20e's fx/explosions/grenate
  is not in 0.26g), spawn_sec 0.26g's Spring's (1, an int as every 0.26g artefact's; 0.20e's 0.5
  is a float, which release/build-content's value-type check refuses at that path: @2); its effect
  0.20e's name artefact_rattle, made here (@3);
- the active's glow, grey (@3; the owner, 2026-10-25): gameplay/effects/effect_artefact_rattle.options,
  0.26g's effect_artefact_lifebone (0.20e's effect_artefact_rattle was Lifebone's particle too) with
  the particle fx/artifacts/human_rattle: 0.26g's human_zhivokost with its materials' grey copies
  (GLOW_MATERIALS: up_green, stroke_scroll_green and rays_green, their constant_emissive GREY in
  place of green, alpha kept) and its point light's colour (LightColor and its data source) GREY.
  The green is the materials' constant_emissive (the particles' ColorLife is white; 0.26g's plain
  up, stroke_scroll and rays give the same textures an orange one): the copies change that colour
  alone. Lifebone keeps its green;
- the model item/artefacts/artefact_pogremushka: 0.20e's files (0.26g's model format: 0.20e's and
  0.26g's Spring are byte for byte the same), its settings' LOD switching Spring's in 0.26g
  (200, 300, 400; 0.20e said default). Its materials item/artefacts_02 and
  item/artefacts_pogremushka and texture item/artefacts/arttfacts02 are 0.26g's own. No collision:
  0.26g's Spring and Onyx have none either (an artefact is taken through its anomaly's container);
- the texts in 0.26g's six languages (texts.tsv): the name and the description new keys; the pickup
  and activation hints 0.26g has, set to ours (localization_add cannot change a text: config_set).
"""
import argparse
import copy
import hashlib
import json
import struct
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

PACK_ID = 'rattle'
VERSION = '3'
DICT_ID = 759
CONFIG = 'gameplay/items/artefacts/artefact_rattle.options'
MODEL = 'models/item/artefacts/artefact_pogremushka.model/'
SETTINGS = MODEL + 'settings'
LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')
NEW_TEXTS = ('st_artefact_rattle_name', 'st_artefact_rattle_description')
SET_TEXTS = ('st_rattle_pickup_hint', 'st_rattle_activation_hint')
# The owner's rules (rattle.md, 2026-10-23): others hear the carrier's steps and movement noise at
# 70 % a Rattle; active (10 s, cooldown 40 s) none, and his shots at 50 % a Rattle fired.
PASSIVE = {'steps_sound_volume_3rd_person': 0.7}
ACTIVE = {'duration': 10, 'steps_sound_volume_3rd_person': 0, 'shots_sound_volume_3rd_person': 0.5}
COOLDOWN_SEC = 40
ICON = [21, 1]
# 0.20e's spawn particle 0.26g has not
SPAWN_PARTICLE = ('fx/explosions/grenate', 'fx/artifacts/emergence_zhivokost_single')
# The active's glow (@3): 0.20e's effect name, made of 0.26g's Lifebone's effect and particle in grey
EFFECT = 'artefact_rattle'
EFFECT_FILE = f'gameplay/effects/effect_{EFFECT}.options'
LIFEBONE_EFFECT = 'gameplay/effects/effect_artefact_lifebone.options'
GLOW_PARTICLE = ('fx/artifacts/human_zhivokost', 'fx/artifacts/human_rattle')
GLOW_MATERIALS = {'fx/artifacts/up_green': 'fx/artifacts/up_grey',
                  'fx/artifacts/stroke_scroll_green': 'fx/artifacts/stroke_scroll_grey',
                  'fx/artifacts/rays_green': 'fx/artifacts/rays_grey'}
# a light-to-mid grey, about the green's brightness (its luminance 0.57..0.59), not white
GREY = (0.5, 0.5, 0.5)
# 0.26g's Spring's LOD switching (its 0.20e settings said default_params true, as the Rattle's)
LOD_SWITCHING = {'default_params': False, 'type': 0, 'param0': 200, 'param1': 300, 'param2': 400}


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


def porter_text(text):
    """The game keeps texts as UTF-8 bytes; the porter reads and writes a config's strings as
    Windows-1251 (WHATWG's: byte 0x98 is U+0098), so a pack names the UTF-8 bytes so read
    (the Sponge's make_pack.py)."""
    raw = text.encode('utf-8')
    return ''.join('\x98' if b == 0x98 else bytes([b]).decode('cp1251') for b in raw)


def load_localization(data):
    """A localization file: its texts are UTF-8 (a few other strings are not: kept as bytes; the
    Sponge's make_pack.py)."""
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


def rattle_config(donor, spring):
    """0.20e's artefact_rattle.options as exe patch 59 reads it, 0.26g's way (the docstring)."""
    data = donor['data']
    want = {'type': 8, 'amount': 1, 'cooldown_sec': 0, 'duration': 10, 'effect': EFFECT,
            'pickup_hint': 'st_rattle_pickup_hint', 'activation_hint': 'st_rattle_activation_hint'}
    for key, value in want.items():
        if data.get(key) != value:
            sys.exit(f'donor {CONFIG}: data/{key} is {data.get(key)!r}, not {value!r}')
    if set(data['passive']) != {'steps_sound_volume_3rd_person', 'steps_sound_volume_1st_person'} \
            or set(data['active']) != {'steps_sound_volume_3rd_person', 'steps_sound_volume_1st_person'}:
        sys.exit(f'donor {CONFIG}: its passive and active are not the four step volumes')
    if donor['object']['model'] != 'item/artefacts/artefact_pogremushka' \
            or donor['object']['particle_on_spawn'] != SPAWN_PARTICLE[0] or donor['ui_desc']['icon'] != 19 \
            or donor['parameters'] != {'weight': bcw.loads(bcw.dump({'w': 0.2}))['w']}:
        sys.exit(f'donor {CONFIG}: not 0.20e\'s model, particle, icon 19 and weight 0.2')
    config = copy.deepcopy(donor)
    config['ui_desc']['icon'] = list(ICON)
    config['object']['particle_on_spawn'] = SPAWN_PARTICLE[1]
    out = config['data']
    del out['duration']
    out['passive'] = dict(PASSIVE)
    out['active'] = dict(ACTIVE)
    out['cooldown_sec'] = COOLDOWN_SEC
    out['amount'] = -1
    out['spawn_sec'] = spring['data']['spawn_sec']
    # 0.26g's artefacts as Spring has them: the same keys of the same types (the passive and the
    # active are the Rattle's own, read by the exe's patch alone)
    for key in ('type', 'amount', 'cooldown_sec', 'effect', 'pickup_hint', 'activation_hint', 'spawn_sec'):
        if key not in spring['data'] or type(spring['data'][key]) is not type(out[key]):
            sys.exit(f'0.26g\'s Spring has no data/{key} of the type the Rattle\'s is')
    if set(config) != set(spring) or set(config['object']) != set(spring['object']) \
            or set(config['ui_desc']) != set(spring['ui_desc']):
        sys.exit('the Rattle\'s config has not the sections and keys of 0.26g\'s Spring')
    return config


def model_settings(donor):
    settings = copy.deepcopy(donor)
    if settings.get('lod_switching') != {'default_params': True, 'type': 0}:
        sys.exit(f'donor {SETTINGS}: its LOD switching is not 0.20e\'s default {settings.get("lod_switching")}')
    if settings['material_settings'] != {
            'arttfacts02_s': {'material_name': 'item/artefacts_02', 'used_in_portals': False},
            'arttfacts02_glasssg': {'material_name': 'item/artefacts_pogremushka', 'used_in_portals': False}}:
        sys.exit(f'donor {SETTINGS}: not the materials item/artefacts_02 and item/artefacts_pogremushka')
    settings['lod_switching'] = dict(LOD_SWITCHING)
    return settings


def edit(node, path, change):
    """The node tree with change(path, node) applied to every node (None: kept); path the keys."""
    got = change(path, node)
    if got is not None:
        return got
    kind = node[0]
    if kind == bcw.NAMED:
        return (kind, [(key, edit(child, path + (key.decode('cp1251'),), change)) for key, child in node[1]],
                node[2])
    if kind == bcw.INDEXED:
        return (kind, [edit(child, path + (str(i),), change) for i, child in enumerate(node[1])], node[2])
    return node


def float4(node):
    return struct.unpack('<4f', node[1]) if node[0] == 8 else None


def greyed(node):
    """A float4 colour node in GREY, its alpha kept."""
    return (8, struct.pack('<4f', *GREY, float4(node)[3]), node[2])


def is_green(rgba):
    return rgba is not None and rgba[1] > 0.7 and rgba[0] < 0.15 and rgba[2] < 0.05


def glow(stock, files, log):
    """The active's glow in grey (the docstring): the effect, its particle and the materials."""
    effect = bcw.read(stock.read(LIFEBONE_EFFECT))
    seen = []

    def particle_name(path, node):
        if path == ('effect', 'particle_system'):
            if node[1].decode('cp1251') != GLOW_PARTICLE[0]:
                sys.exit(f'stock {LIFEBONE_EFFECT}: its particle is not {GLOW_PARTICLE[0]}')
            seen.append(path)
            return (bcw.STRING, GLOW_PARTICLE[1].encode('cp1251'), len(GLOW_PARTICLE[1]) + 1)
        return None
    files[EFFECT_FILE] = bcw.write(edit(effect, (), particle_name))
    if len(seen) != 1:
        sys.exit(f'stock {LIFEBONE_EFFECT}: no effect/particle_system')
    log.append(f'{EFFECT_FILE}: 0.26g\'s {LIFEBONE_EFFECT}, its particle {GLOW_PARTICLE[0]} -> {GLOW_PARTICLE[1]}')

    source = f'particles/{GLOW_PARTICLE[0]}.particle'
    particle = bcw.read(stock.read(source))
    materials, lights = [], []

    def recolour(path, node):
        if path and path[-1] == 'Material' and node[0] == bcw.STRING:
            name = node[1].decode('cp1251')
            if name in GLOW_MATERIALS:
                materials.append(name)
                new = GLOW_MATERIALS[name]
                return (bcw.STRING, new.encode('cp1251'), len(new) + 1)
            if name.endswith('_green'):
                sys.exit(f'stock {source}: a green material {name} this pack has no grey of')
            return None
        # the point light's colour: the Light action's LightColor ramp and the data source it names
        if path and path[-1] == 'value' and ('LightColor' in path or path[:3] == ('data_sources', 'color', 'color0')):
            if not is_green(float4(node)):
                sys.exit(f'stock {source}: {"/".join(path)} is not Lifebone\'s green {float4(node)}')
            lights.append('/'.join(path))
            return greyed(node)
        return None
    files[f'particles/{GLOW_PARTICLE[1]}.particle'] = bcw.write(edit(particle, (), recolour))
    if set(materials) != set(GLOW_MATERIALS) or len(lights) != 2:
        sys.exit(f'stock {source}: not the materials {sorted(GLOW_MATERIALS)} and one light '
                 f'({sorted(set(materials))}, {lights})')
    log.append(f'particle {GLOW_PARTICLE[1]}: 0.26g\'s {GLOW_PARTICLE[0]}, its {len(materials)} emitters\' materials '
               f'the grey copies, its light\'s colour (LightColor, data source color0) green -> {GREY}')

    for green, grey in GLOW_MATERIALS.items():
        found = []

        def emissive(path, node):
            if path == ('material', 'forward', 'effect', 'constant_emissive', 'value'):
                if not is_green(float4(node)):
                    sys.exit(f'stock {green}: constant_emissive {float4(node)} is not green')
                found.append(float4(node))
                return greyed(node)
            return None
        data = edit(bcw.read(stock.read(f'material_instances/{green}.material')), (), emissive)
        if len(found) != 1:
            sys.exit(f'stock {green}: no forward constant_emissive')
        files[f'material_instances/{grey}.material'] = bcw.write(data)
        log.append(f'material {grey}: 0.26g\'s {green}, constant_emissive '
                   f'{tuple(round(v, 4) for v in found[0][:3])} -> {GREY}')


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
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
    files, log = {}, []

    spring = bcw.loads(stock.read('gameplay/items/artefacts/artefact_spring.options'))
    files[CONFIG] = bcw.dump(rattle_config(bcw.loads(donor.read(CONFIG)), spring))
    log.append(f'{CONFIG}: 0.20e\'s, data/duration into data/active, passive {PASSIVE}, active {ACTIVE}, '
               f'cooldown_sec 0 -> {COOLDOWN_SEC}, amount 1 -> -1, spawn_sec 0.5 -> {spring["data"]["spawn_sec"]} '
               f'(Spring\'s int), icon 19 -> {ICON}, '
               f'particle_on_spawn {SPAWN_PARTICLE[0]} -> {SPAWN_PARTICLE[1]}, effect {EFFECT} (0.20e\'s, made below)')
    glow(stock, files, log)
    spring_settings = bcw.loads(stock.read('models/item/artefacts/artefact_spring.model/settings'))
    if spring_settings['lod_switching'] != LOD_SWITCHING:
        sys.exit(f'0.26g\'s Spring\'s LOD switching is not {LOD_SWITCHING}')
    for path in sorted(p[len('donor/'):] for p in sums if p.startswith('donor/' + MODEL)):
        data = donor.read(path)
        files[path] = bcw.dump(model_settings(bcw.loads(data))) if path == SETTINGS else data
    log.append(f'model item/artefacts/artefact_pogremushka: 0.20e\'s {len([p for p in files if p.startswith(MODEL)])} '
               f'files, its settings\' LOD switching 0.26g\'s Spring\'s {LOD_SWITCHING}')

    header, *table = rows('texts.tsv')
    if tuple(header) != ('key',) + LANGUAGES:
        sys.exit(f'texts.tsv: the columns are key and {", ".join(LANGUAGES)}')
    texts = {}
    for row in table:
        if len(row) != 1 + len(LANGUAGES) or not all(row):
            sys.exit(f'texts.tsv: {row[0]}: a text in each language')
        texts[row[0]] = dict(zip(LANGUAGES, row[1:]))
    if set(texts) != set(NEW_TEXTS) | set(SET_TEXTS):
        sys.exit(f'texts.tsv: the keys are {NEW_TEXTS + SET_TEXTS}')
    for language in LANGUAGES:
        strings = load_localization(stock.read(f'localization/{language}/localization'))['strings']
        for key in NEW_TEXTS:
            if key in strings:
                sys.exit(f'stock {language}: {key} is there already')
        for key in SET_TEXTS:
            if not isinstance(strings.get(key), str):
                sys.exit(f'stock {language}: no {key}')
            log.append(f'{language} {key}: {strings[key]!r} -> {texts[key][language]!r}')

    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [{
        'base_cost': 0, 'cfg_name': CONFIG, 'dict_id': DICT_ID, 'is_premium': False, 'is_stack': True,
        'item_category': 12, 'name': 'artefact_rattle'}]})
    for language in LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {key: porter_text(texts[key][language]) for key in NEW_TEXTS}})
        operations.append({'op': 'config_set', 'path': f'localization/{language}/localization',
                           'values': {f'strings.{key}': porter_text(texts[key][language]) for key in SET_TEXTS}})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the Rattle (items_dict 759, 0.20e\'s artefact_rattle): its config (the owner\'s sound '
                       'factors, read by exe patch 59), 0.20e\'s model and texts, its active\'s glow '
                       '(Lifebone\'s effect and particle in grey)',
        'operations': operations,
        'notes': ['made by srp-026g/scripts/rattle/make_pack.py from inputs.sha256 and texts.tsv',
                  'donor: Survarium 0.20e'] + log,
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
