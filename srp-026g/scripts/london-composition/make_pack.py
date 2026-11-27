#!/usr/bin/env python3
"""make_pack.py <vostok-port> <stock resources.db> <donor db> <out folder>: the content pack
composition-v044a, what exe patch 178 (composition_v044a) draws the ported Londons' deferred
composition with: 0.44a5's 64 composition.ps programs as the family composition_v044a.ps at
0.26g's keys, and that family's mask (composition.ps's) in shaders/masks.

<donor db> is the 0.44a5 archive that holds shaders/sm_4_0/composition.ps/ (resources.05.db of
the unpacked 0.44a5; 0.32a4's 64 are the same bytes of code and resources). The programs are
copied byte for byte; only their keys change. A key has a character per macro of the build's
key order (resource-porter ports/near-build/shader-key-order-*.tsv), `_` where the family's mask
does not read it. Both builds' composition.ps reads the same five macros, at other places:

    macro                       0.26g  0.44a5 (and 0.32a4)
    GLOBAL_LIGHTING_QUALITY       3      3
    GLOBAL_SHADOWMAP_QUALITY      4      4
    CONFIG_TDIFFUSE               5      5
    CONFIG_TNORMAL                6     10
    CONFIG_TFRESNEL              13     15

The pack is refused unless the donor's keys, so moved, are exactly players' 64 composition.ps
keys: the twin then exists for every choice 0.26g's own programs do.
"""
import hashlib
import json
import os
import subprocess
import sys

FAMILY = 'shaders/sm_4_0/composition.ps/'
TWIN = 'shaders/sm_4_0/composition_v044a.ps/'
MASK = ['CONFIG_TDIFFUSE', 'CONFIG_TFRESNEL', 'CONFIG_TNORMAL', 'GLOBAL_LIGHTING_QUALITY',
        'GLOBAL_SHADOWMAP_QUALITY']
# 0.26g's place of each of the donor's (see above).
PLACES = {3: 3, 4: 4, 5: 5, 10: 6, 15: 13}
STOCK_SHA256 = 'c74078be702d17113b9ca23f59aa2b0087238481e7f01fb793620d51d875492c'


def keys(porter, db):
    out = subprocess.run([porter, 'list', '--db', db, '--prefix', FAMILY], check=True,
                         capture_output=True, text=True).stdout.split()
    return sorted(path[len(FAMILY):] for path in out)


def moved(key, length):
    out = ['_'] * length
    for donor, ours in PLACES.items():
        out[ours] = key[donor]
    assert all(c == '_' for i, c in enumerate(key) if i not in PLACES), key
    return ''.join(out)


def main():
    porter, stock, donor, out = sys.argv[1:]
    if os.path.exists(out):
        sys.exit(f'{out} exists')
    players = keys(porter, stock)
    length = len(players[0])
    donors = keys(porter, donor)
    pairs = {moved(key, length): key for key in donors}
    if sorted(pairs) != players or len(pairs) != len(donors):
        sys.exit(f'the donor keys moved are not players\' {len(players)}: {sorted(set(pairs) ^ set(players))[:4]}')
    operations = [{
        'op': 'config_set',
        'path': 'shaders/masks',
        'values': {'composition_v044a': {'table': {'ps': MASK}}},
    }]
    for key in players:
        target = TWIN + key
        path = os.path.join(out, 'files', target)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        subprocess.run([porter, 'extract', '--db', donor, '--path', FAMILY + pairs[key], '--output', path],
                       check=True, capture_output=True)
        operations.append({'op': 'add_file', 'path': target})
    pack = {
        'format_version': 1,
        'id': 'composition-v044a',
        'version': '1',
        'target_build': '0.26g',
        'description': "What exe patch 178 (composition_v044a) draws the ported Londons' deferred composition "
                       "with: 0.44a5's 64 composition.ps programs (0.32a4's are the same) as the family "
                       "composition_v044a.ps at 0.26g's keys, and its mask in shaders/masks; no existing file "
                       "changes",
        'requires': [],
        'base_db_sha256': STOCK_SHA256,
        'operations': operations,
    }
    with open(os.path.join(out, 'pack.json'), 'w') as f:
        json.dump(pack, f, indent=2)
        f.write('\n')
    digest = hashlib.sha256(open(os.path.join(out, 'pack.json'), 'rb').read()).hexdigest()
    print(f'{len(players)} programs; pack.json sha256 {digest}')


main()
