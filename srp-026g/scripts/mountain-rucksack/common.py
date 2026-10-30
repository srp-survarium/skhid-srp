"""What the mountain rucksack's scripts share: the pinned inputs, items.tsv, and the machinery of
the earlier equipment packs (equipment-069's loader, through it equipment-046's icon cell,
catalog reader and asset checks, and equipment-029's atlas and text helpers)."""
import csv
import importlib.util
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent


def _equipment069_common():
    name = 'equipment069_common'
    if name not in sys.modules:
        spec = importlib.util.spec_from_file_location(name, SCRIPTS / 'equipment-069' / 'common.py')
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
    return sys.modules[name]


_e069 = _equipment069_common()
bcw = _e069.bcw
sha = _e069.sha
sound_file = _e069.sound_file
equipment046 = _e069.equipment046
DICTIONARY, MODIFICATIONS, MOVIE, LOCALIZATIONS = (_e069.DICTIONARY, _e069.MODIFICATIONS, _e069.MOVIE,
                                                   _e069.LOCALIZATIONS)

PACK = 'mountain-rucksack'
# model.py's output: resource-porter builds the model from the mesh and maps; the icon is drawn
# into the lobby's equipment atlas.
MODEL_FILES = ('rucksack.glb', 'rucksack_diffuse.png', 'rucksack_normal.png', 'rucksack_gloss.png',
               'rucksack_fresnel.png', 'rucksack_icon.png')
ICON = 'rucksack_icon.png'
# resource-porter's build-armour-model writes these beside its overlay.
BUNDLE_RECORDS = ('port-manifest.json', 'fit-report.json')


def specs():
    with (HERE / 'items.tsv').open() as source:
        return list(csv.DictReader(source, delimiter='\t', quoting=csv.QUOTE_NONE))


def text_keys(spec):
    stem = 'st_srp_' + spec['key']
    return {'name': stem + '_name', 'description': stem + '_desc'}


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
