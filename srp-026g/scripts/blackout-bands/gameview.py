"""The game's files as it mounts them (resources.db, then its patches; later wins), read with
vostok-port and cached under $BANDS_WORK/gamefiles."""
import json
import os
import subprocess
import bandsenv as E

PREFIXES = ['models/character/human/', 'models/ported/', 'material_instances/', 'textures/character/',
            'textures/ported/', 'gameplay/items/armour/', 'gameplay/items/upgrades/camouflages/']
CACHE = os.path.join(E.WORK, 'gamefiles')


def index():
    path = os.path.join(CACHE, 'index.json')
    if os.path.exists(path):
        return json.load(open(path))
    idx = {}
    for layer in E.layers():
        for prefix in PREFIXES:
            out = subprocess.run([E.VOSTOK_PORT, 'list', '--db', layer, '--prefix', prefix],
                                 capture_output=True, text=True).stdout
            for line in out.splitlines():
                if line.strip():
                    idx[line.strip()] = layer
    os.makedirs(CACHE, exist_ok=True)
    json.dump(idx, open(path, 'w'))
    return idx


IDX = index()


def read(path):
    """Bytes of a game file."""
    local = os.path.join(CACHE, 'files', path)
    if not os.path.exists(local):
        layer = IDX.get(path)
        if layer is None:
            raise KeyError(path)
        os.makedirs(os.path.dirname(local), exist_ok=True)
        subprocess.run([E.VOSTOK_PORT, 'extract', '--db', layer, '--path', path, '--output', local],
                       check=True, capture_output=True)
    return open(local, 'rb').read()


def local(path):
    read(path)
    return os.path.join(CACHE, 'files', path)


def config(path):
    """Leaves of a binary config as {key: 'type:value'}."""
    out = subprocess.run([E.VOSTOK_PORT, 'dump-config', '--leaves', '--file', local(path)],
                         capture_output=True, text=True, check=True).stdout
    d = {}
    for line in out.splitlines():
        if '\t' in line:
            k, v = line.split('\t', 1)
            d[k] = v
    return d


def string(v):
    return v.split(':', 1)[1].strip('"') if v.startswith('string:') else v


def children(prefix):
    return sorted(p for p in IDX if p.startswith(prefix))
