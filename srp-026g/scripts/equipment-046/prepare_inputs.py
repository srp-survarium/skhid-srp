#!/usr/bin/env python3
"""Extract the pack's pinned inputs (inputs.sha256) from the donor builds and the released game.

--donor031 and --donor046 are the 0.31a2 and 0.46c2 game folders (their
resources.NN.db volumes, unpacked from the builds' installers). --stock is the
untouched 0.26g resources.db, --patches holds the published patches of
base-patches.tsv. No server data or release working folder is used.

--pin computes what the pack needs from items.tsv instead (the dependency
closure of the donor models: model folders, materials, textures with their
camouflage `_aoac` maps; assets the released game has are reused, not
listed) and writes inputs.sha256 and shipped_textures.txt.
"""
import argparse
import csv
from pathlib import Path
import shutil
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402
from donors import Layers, digest  # noqa: E402

LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')
DICTIONARY = 'gameplay/db_static_dictionaries'
GAME_OPTIONS = 'gameplay/game.options'
MODIFICATIONS = 'gameplay/items/modifications/modifications.options'
MOVIE = 'flash_movies/lobby_menu.swf'
TEXTURE_FOLDERS = ('textures/character/', 'textures/editor/')
# An item config's sounds: [kind, name], the kind naming the resource.
SOUND_FILES = {257: 'sounds/composite/{}.composite_sound', 258: 'sounds/collections/{}.sound_collection'}


def sound_file(kind, name):
    return SOUND_FILES[kind].format(name)


def specs():
    with (HERE / 'items.tsv').open() as source:
        return list(csv.DictReader(source, delimiter='\t', quoting=csv.QUOTE_NONE))


def icon_path(row, col):
    """The donor builds keep each item icon as its own file, column first."""
    return f'flash_movies/textures/ui_icons/items/equipment/equipment_{col}_{row}.png'


def models_of(config):
    """The skinned models an item config draws (every data variant, hud hands, extra parts)."""
    found = set()

    def walk(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key in ('base_model', 'base_model_hud') and isinstance(child, str):
                    found.add(child)
                else:
                    walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)
    for key, value in config.items():
        if key == 'data' or key.startswith('data_'):
            walk(value)
    return found


def lod0(settings):
    return settings['lod_hierrarchy']['LOD0']['surfaces']


def textures_of(material):
    for body in material['material'].values():
        effect = body.get('effect') if isinstance(body, dict) else None
        if isinstance(effect, dict):
            for key, field in effect.items():
                if key.startswith('texture_') and isinstance(field, dict) and field.get('value'):
                    yield field['value']


class Closure:
    def __init__(self, donor, base, work):
        self.donor, self.base, self.work = donor, base, work
        self.paths = set()

    def config(self, path):
        target = self.work / path
        if not target.exists():
            self.donor.extract([path], self.work, self.work / '.stage')
        return bcw.loads(target.read_bytes())

    def add(self, path):
        if path not in self.base:
            self.paths.add(path)

    def model(self, model):
        folder = f'models/{model}.skinned_model/'
        files = sorted(path for path in self.donor.paths() if path.startswith(folder))
        if not files:
            raise ValueError(f'{model}: no donor model')
        shared = [path for path in files if path in self.base]
        if shared:
            raise ValueError(f'{model}: partly in the released game: {shared[:3]}')
        settings = self.config(folder + 'settings')
        surfaces = lod0(settings)
        skeleton = self.config(folder + 'skeleton')['skeleton'].removeprefix('resources/') + '.skeleton'
        if skeleton not in self.base:
            raise ValueError(f'{model}: skeleton {skeleton} is not in the released game')
        for path in files:
            part = path[len(folder):].split('/')
            # Only the surfaces 0.26g draws: LOD0 (0.46c2 adds LOD1 and LOD2).
            if len(part) == 3 and part[0] == 'render' and part[1] not in surfaces:
                continue
            self.add(path)
        for surface in surfaces:
            self.material(settings['material_settings'][surface]['material_name'])

    def material(self, name):
        path = f'material_instances/{name}.material'
        if path in self.base:
            return
        if path not in self.donor:
            raise ValueError(f'missing material {path}')
        self.add(path)
        for texture in textures_of(self.config(path)):
            for variant in (texture, texture + '_aoac'):
                for suffix in ('.dds', '.options'):
                    candidate = f'textures/{variant}{suffix}'
                    if candidate in self.donor:
                        self.add(candidate)
                    elif suffix == '.dds' and variant == texture and candidate not in self.base:
                        # make_pack turns such a map's switch off (no texture anywhere).
                        print(f'{name}: texture {texture} in neither build', file=sys.stderr)


def pin(args, donors, base):
    rows = specs()
    work = args.out / '.pin'
    inputs = {'base': {DICTIONARY, MODIFICATIONS, MOVIE}, 'donor031': {DICTIONARY}, 'donor046': {GAME_OPTIONS}}
    for lang in LANGUAGES:
        for group in inputs:
            inputs[group].add(f'localization/{lang}/localization')
    base.extract([DICTIONARY], work / 'base', work / '.stage')
    native = bcw.loads((work / 'base' / DICTIONARY).read_bytes())['items_dict']
    inputs['base'] |= {row['cfg_name'] for row in native if '/armour/' in row['cfg_name']}
    donors['031'].extract([DICTIONARY], work / 'donor031', work / '.stage')
    by_name = {row['cfg_name'].rsplit('/', 1)[1].removesuffix('.options'): row['cfg_name']
               for row in bcw.loads((work / 'donor031' / DICTIONARY).read_bytes())['items_dict']}
    donors['046'].extract([GAME_OPTIONS], work / 'donor046', work / '.stage')
    equipment = bcw.loads((work / 'donor046' / GAME_OPTIONS).read_bytes())['equipment']
    closures = {name: Closure(donors[name], base, work / f'assets{name}') for name in donors}
    for row in rows:
        if row['donor'] == '031':
            path = by_name[row['id_069']]
            inputs['donor031'].add(path)
            donors['031'].extract([path], work / 'donor031', work / '.stage')
            config = bcw.loads((work / 'donor031' / path).read_bytes())
        else:
            config = equipment[row['id_069']]
        inputs['donor' + row['donor']].add(icon_path(*config['ui_desc']['icon']))
        # The sounds the released game has (make_pack replaces the others with the template's).
        inputs['base'] |= {sound_file(*sound) for sound in config.get('sounds', []) if sound_file(*sound) in base}
        for model in sorted(models_of(config)):
            closures[row['donor']].model(model)
    for name, closure in closures.items():
        inputs['assets' + name] = closure.paths
    textures = sorted(path for path in base.paths() if path.startswith(TEXTURE_FOLDERS) and path.endswith('.dds'))
    (HERE / 'shipped_textures.txt').write_text(''.join(path + '\n' for path in textures))
    return {f'{group}/{path}' for group, paths in inputs.items() for path in paths}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('donor031', 'donor046', 'stock', 'patches', 'game-data-bin', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--pin', action='store_true', help='compute the inputs from items.tsv and pin them')
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out must be a new directory')
    donors = {name: Layers.donor(args.game_data_bin, getattr(args, 'donor' + name)) for name in ('031', '046')}
    base = Layers.base(args.game_data_bin, args.stock, args.patches)
    args.out.mkdir(parents=True)
    if args.pin:
        keys = pin(args, donors, base)
    else:
        sums = dict(reversed(line.split('  ', 1)) for line in (HERE / 'inputs.sha256').read_text().splitlines())
        keys = set(sums)
    layers = {'base': base, 'donor031': donors['031'], 'donor046': donors['046'],
              'assets031': donors['031'], 'assets046': donors['046']}
    groups = {}
    for key in keys:
        group, path = key.split('/', 1)
        groups.setdefault(group, []).append(path)
    for group, paths in sorted(groups.items()):
        layers[group].extract(paths, args.out / group, args.out / '.stage')
    if args.pin:
        lines = sorted(f'{digest(args.out / key)}  {key}' for key in keys)
        (HERE / 'inputs.sha256').write_text(''.join(line + '\n' for line in lines))
        shutil.rmtree(args.out / '.pin')
        print(f'{len(keys)} inputs pinned in inputs.sha256')
    else:
        for key, expected in sums.items():
            if digest(args.out / key) != expected:
                raise ValueError(f'{key}: extracted bytes differ from the pinned input')
        print(f'{len(keys)} pinned resource inputs verified in {args.out}')


if __name__ == '__main__':
    main()
