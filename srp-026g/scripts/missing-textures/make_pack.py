#!/usr/bin/env python3
"""make_pack.py: the content pack of textures the live data names but no archive of 0.26g has,
taken from the Survarium build that has them (2026-10-25; docs/notes/missing-textures.md).

usage: python3 -I make_pack.py --srp-game-data <program> --builds <folder> --out <packs folder>/missing-textures-<version>

--srp-game-data  srp-026g's srp-game-data (its `get` reads a build's resources.db)
--builds         the builds' resources.db files: <folder>/<build>/<archive> as SOURCES names them
                 (0.21d/resources.db, v031a2/resources.06.db, ...: /mnt/toshiba/surv-resources and the
                 weapon survey's builds; a 0.31+ build is split, each resources.NN.db its own archive)
--out            the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
                 manifest's `pack` row (release/content.tsv)

Each file is the source build's own, byte for byte (pinned by its sha256): the .dds as it shipped
and its .options, which are byte-identical to stock 0.26g files of the same kind.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

PACK_ID = 'missing-textures'
VERSION = '1'
BM05 = 'textures/character/human/black_market_05/black_market_05_'
# game path: (build, archive, sha256, why)
SOURCES = {
    BM05 + 'legs_camo_nmap.dds': ('v031a2', 'resources.06.db', '6792388931895b13a9b41aa0eea6878a6dc5f33e133d22f879596eccd05e9091',
                                  'the Black Market 5 pants\' camouflage layer normal map: the baker asks for it on a decal without a camouflage (0.26g has the layer\'s _camo and _camo_smap only)'),
    BM05 + 'legs_camo_nmap.options': ('v031a2', 'resources.06.db', '7861fedb73812d0047394f32cf5aee9543390f5d2e9a3d7426c60ee13500ab71', ''),
    BM05 + 'torso_camo_nmap.dds': ('v031a2', 'resources.06.db', '8a6961e91d7f6556b8c2135773e8536ace6a710604fc8e164b38357096f0078a',
                                   'the Black Market 5 jacket\'s camouflage layer normal map (its _camo and _camo_smap: missing-textures-1.json)'),
    BM05 + 'torso_camo_nmap.options': ('v031a2', 'resources.06.db', '7861fedb73812d0047394f32cf5aee9543390f5d2e9a3d7426c60ee13500ab71', ''),
    'textures/flora_2/saksaul_tile_bump.dds': ('v031a2', 'resources.10.db', '07eba3284cc5ac58f2b089bc96578d187cacc6e3635f8593b2677bd92698668f',
                                               'level_09\'s saxaul trees: material flora_2/saksaul_tile (parallax on) names it; 0.31a2\'s saksaul_tile has the same layout'),
    'textures/flora_2/saksaul_tile_bump.options': ('v031a2', 'resources.10.db', '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d', ''),
    'textures/tiled/metal/metal_column_05_fmap.dds': ('v046c2', 'resources.21.db', '0e27e9423c67af9e089c5a0ccaa256ed715e363868b80ac15e5db9d26861fd5b',
                                                      'the fresnel map of the v0.11e and v0.21d maps\' ported metal_column_05 materials (pipes, factories); 0.46c2\'s metal_column_05 has the same layout'),
    'textures/tiled/metal/metal_column_05_fmap.options': ('v046c2', 'resources.21.db', '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d', ''),
    'textures/tiled/stucco/stucco_ikona.dds': ('0.21d', 'resources.db', '016b22c2cd20874e8bb20a9263ade59b687849ca634dbfd56f55bf534cf61b04',
                                               'the icons painted on level_01\'s church walls (stucco_01_icon_vc, church_planks_icon_vc, wood_planks_02_icon_vc); 0.21d\'s stucco_01 is 0.26g\'s byte for byte'),
    'textures/tiled/stucco/stucco_ikona.options': ('0.21d', 'resources.db', '7b2180b296626209c22535d5320dac7c503e85975437364d2482ef518e9a9e75', ''),
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--srp-game-data', required=True)
    ap.add_argument('--builds', required=True)
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    out = Path(a.out)
    if out.exists():
        sys.exit(f'{out}: already there')
    by_archive = {}
    for path, (build, archive, _, _) in SOURCES.items():
        by_archive.setdefault((build, archive), []).append(path)
    got = {}
    with tempfile.TemporaryDirectory() as tmp:
        for (build, archive), paths in sorted(by_archive.items()):
            db = Path(a.builds) / build / archive
            folder = Path(tmp) / build / archive
            subprocess.run([a.srp_game_data, 'get', '--db', str(db), '--out', str(folder)] + paths, check=True,
                           stdout=subprocess.DEVNULL)
            for p in paths:
                got[p] = (folder / p).read_bytes()
    operations, notes = [], []
    for path in sorted(SOURCES):
        build, archive, want, why = SOURCES[path]
        data = got[path]
        if sha256(data) != want:
            sys.exit(f'{path} of {build}/{archive}: sha256 {sha256(data)}, not {want}')
        target = out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        operations.append({'op': 'add_file', 'path': path})
        if why:
            notes.append(f'{path}: {build} ({archive}): {why}')
    manifest = {'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
                'description': 'textures the live data names that no 0.26g archive has, from the Survarium build '
                               'that has them (srp-026g/scripts/missing-textures/make_pack.py)',
                'operations': operations, 'notes': notes}
    (out / 'pack.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'pack {PACK_ID}@{VERSION}: {len(operations)} files; pack.json sha256 {sha256((out / "pack.json").read_bytes())}')


if __name__ == '__main__':
    main()
