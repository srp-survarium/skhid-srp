"""What the weapon-variants scripts share: variants.tsv, the pinned inputs, the two builds' meshes,
and the machinery of earlier packs (research-mode's config reader and writer, exo-gear's DXT1 codec,
weapon-camo's camouflage masks)."""
import collections
import hashlib
import importlib.util
from pathlib import Path
import struct
import sys

import numpy as np

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent
ROOT = SCRIPTS.parent.parent
sys.path.insert(0, str(SCRIPTS / 'research-mode'))
import bcw  # noqa: E402

PACK = 'weapon-variants'
DICTIONARY = 'gameplay/db_static_dictionaries'
MOVIE = 'flash_movies/lobby_menu.swf'
SYMBOL = 'LobbyMenu_WeaponIcons'
WEAPONS = 'gameplay/items/weapons'
SCOPES = 'gameplay/items/scopes'
# Everything the pack adds lives under these folders, so no path is shared with another pack.
FOLDER = 'weapons/variants'
# The camouflage mask template weapon_camo.py copies beside each _aoac (client patch 44).
AOAC_OPTIONS = 'textures/character/human/black_market_01/black_market_01_torso_aoac.options'
LANGUAGES = ('english', 'russian', 'ukrainian', 'belarusian', 'polish', 'german', 'czech', 'finnish',
             'hungarian', 'serbian')
SILVER_BONUS = 1230  # golden_weapons.md: a golden weapon's one bonus, its first own modification

# The two premium textures both builds ship, the check of paint.py (check_pack.py): (0.26g's, 0.69d0's,
# its gun's 0.26g diffuse, 0.69d0's albedo and metalness of the gun), texture paths.
CALIBRATION = [
    ('weapons/akmn/akmn_camo_st', 'weapons/assault/akmn/akmn_camo_st', 'weapons/akmn/akmn',
     'weapons/assault/akmn/akmn', 'weapons/assault/akmn/akmn_fmap'),
    ('weapons/sks/sks_camo_st', 'weapons/dmr/sks/sks_camo_st', 'weapons/sks/sks',
     'weapons/dmr/sks/sks', 'weapons/dmr/sks/sks_fmap'),
]

Variant = collections.namedtuple('Variant', 'name dict_id base icon gold donor_model donor_base_model')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def variants():
    rows = []
    with (HERE / 'variants.tsv').open(encoding='utf-8') as source:
        for line in source:
            if line.startswith('#') or not line.strip():
                continue
            name, dict_id, base, row, col, gold, model, base_model = line.rstrip('\n').split('\t')
            rows.append(Variant(name, int(dict_id), base, (int(row), int(col)), int(gold), model, base_model))
    return rows


def name_key(variant):
    return f'st_srp_variant_{variant.name}_name'


def _load(name, path):
    """A sibling script as a module of its own name (its folder first on sys.path while it loads)."""
    if name in sys.modules:
        return sys.modules[name]
    sys.path.insert(0, str(path.parent))
    try:
        spec = importlib.util.spec_from_file_location(name, path)
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
    finally:
        sys.path.remove(str(path.parent))
    return module


def exo_gear():
    """exo-gear's make_pack: the DXT1 codec (dds_levels, decode, encode) and the mesh renderer."""
    return _load('exo_gear_make_pack', SCRIPTS / 'exo-gear' / 'make_pack.py')


def weapon_camo():
    """weapon-camo's weapon_camo.py: the camouflage masks of guns.tsv and parts.tsv."""
    return _load('weapon_camo_tool', SCRIPTS / 'weapon-camo' / 'weapon_camo.py')


# --- the meshes: a vertex chunk (3), the 16-bit triangles (4), 0.69d0's chunks after them ---

def _chunks(data):
    at = 0
    while at + 8 <= len(data):
        kind, size = struct.unpack_from('<II', data, at)
        yield kind, at + 8, size
        at += 8 + size


def mesh(data, build):
    """(positions [n,3], uv [n,2], triangles [m,3]) of a converted_model: 0.26g's 36-byte vertices end
    with a float2 UV; 0.69d0's 32-byte ones hold a half2 UV at byte 28 (resource-porter's
    weapons_069d)."""
    kind, size, count = struct.unpack_from('<III', data, 0)
    assert kind == 3 and (size - 4) % count == 0, 'no vertex chunk'
    stride = (size - 4) // count
    raw = np.frombuffer(data, np.uint8, count=count * stride, offset=12).reshape(count, stride)
    pos = raw[:, :12].copy().view('<f4').reshape(count, 3).astype(np.float64)
    if build == '0.26g':
        uv = raw[:, stride - 8:].copy().view('<f4').reshape(count, 2).astype(np.float64)
    else:
        assert stride == 32, f'0.69d0 vertex of {stride} bytes'
        uv = raw[:, 28:32].copy().view('<f2').reshape(count, 2).astype(np.float64)
    tris = None
    for kind, at, size in _chunks(data):
        if kind == 4:
            n = struct.unpack_from('<I', data, at)[0]
            tris = np.frombuffer(data, '<u2', count=n, offset=at + 4).reshape(-1, 3).astype(np.int64)
    assert tris is not None, 'no index chunk'
    return pos, uv, tris


class Inputs:
    """prepare_inputs.py's folder; every read checked against inputs.sha256."""

    def __init__(self, root):
        self.root = Path(root)
        self.sums = dict(reversed(line.split('  ', 1))
                         for line in (HERE / 'inputs.sha256').read_text().splitlines() if line)

    def has(self, group, path):
        return f'{group}/{path}' in self.sums

    def read(self, group, path):
        key = f'{group}/{path}'
        raw = (self.root / key).read_bytes()
        if self.sums.get(key) != sha(raw):
            raise ValueError(f'{key}: input missing from inputs.sha256 or changed')
        return raw

    def config(self, group, path):
        return bcw.loads(self.read(group, path))

    def paths(self, group, prefix=''):
        return sorted(key.split('/', 1)[1] for key in self.sums
                      if key.split('/', 1)[0] == group and key.split('/', 1)[1].startswith(prefix))
