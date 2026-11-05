"""What equipment-069's scripts share: the pinned inputs, items.tsv, and the machinery of the
earlier equipment packs (equipment-029's atlas and text helpers, equipment-046's icon cell,
catalog reader and the shader combinations the released characters draw)."""
import csv
import hashlib
import importlib.util
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent
sys.path.insert(0, str(SCRIPTS / 'research-mode'))
import bcw  # noqa: E402

PACK = 'equipment-069'
DICTIONARY = 'gameplay/db_static_dictionaries'
MODIFICATIONS = 'gameplay/items/modifications/modifications.options'
MOVIE = 'flash_movies/lobby_menu.swf'
LOCALIZATIONS = tuple(f'localization/{lang}/localization'
                      for lang in ('english', 'german', 'original', 'polish', 'russian', 'ukrainian'))
SOUNDS = {257: 'sounds/composite/{}.composite_sound', 258: 'sounds/collections/{}.sound_collection'}
# 0.69d keeps each item icon as its own picture, column first.
DONOR_ICONS = 'flash_movies/textures/ui_icons/items/equipment/equipment_{col}_{row}.png'
# resource-porter's build-armour-069d writes these beside its overlay.
BUNDLE_RECORDS = ('port-manifest.json', 'fit-report.json')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def sound_file(kind, name):
    return SOUNDS[kind].format(name)


def specs():
    with (HERE / 'items.tsv').open() as source:
        return list(csv.DictReader(source, delimiter='\t', quoting=csv.QUOTE_NONE))


def text_keys(spec):
    stem = 'st_srp_equipment069_' + spec['id_069']
    return {'name': stem + '_name', 'description': stem + '_desc'}


def _load(name, path, folder):
    """A sibling pack's script under its own module name; its own imports from its folder."""
    if name in sys.modules:
        return sys.modules[name]
    shadowed = {key: sys.modules.pop(key) for key in ('donors', 'prepare_inputs', 'make_pack')
                if key in sys.modules}
    sys.path.insert(0, str(folder))
    try:
        spec = importlib.util.spec_from_file_location(name, path)
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
    finally:
        sys.path.remove(str(folder))
        for key in ('donors', 'prepare_inputs', 'make_pack'):
            sys.modules.pop(key, None)
        sys.modules.update(shadowed)
    return module


def equipment046():
    """equipment-046's make_pack: its icon cell, catalog reader, shader combinations (and,
    through it, equipment-029's atlas and text helpers)."""
    folder = SCRIPTS / 'equipment-046'
    return _load('equipment046_make_pack', folder / 'make_pack.py', folder)


class Inputs:
    """prepare_inputs.py's folder; every read checked against inputs.sha256."""

    def __init__(self, root):
        self.root = Path(root)
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
