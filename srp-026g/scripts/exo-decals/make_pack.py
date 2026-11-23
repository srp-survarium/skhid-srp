#!/usr/bin/env python3
"""make_pack.py: the content pack that lets the Duty and Freedom exoskeleton bodies take decals
(2026-11-20; ../../docs/notes/stk2-outfits.md, Decals).

usage: python3 -I make_pack.py --live <folder> --out <packs folder>/exo-decals-<version>

--live  the released files at their game paths (`srp-game-data get --db <stock> --patch <each listed
        patch>... --out <folder> --paths <a file of LIVE's paths>`), each checked by its sha256.
--out   the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the manifest's
        `pack` row (release/content.tsv).

A stock vest has `parameters/decal_available` true and an empty `data/decals` table; the two bodies
(torso items of packs duty-exo and freedom-exo, from STK2) were ported with the flag false and
without the table. The pack sets both.

It also gives the Freedom body's model a file name of its own. The game keeps an item's baked
textures (its diffuse, normal, fresnel and smoothness maps with the decals in them) in files named
after the decals and the model's file name, without its folder, and keeps them from game to game.
Freedom is Duty's mesh in other textures, ported under the same file name in another folder
(`ported/freedom_026/duty_03_torso_s1`, Duty's `ported/duty_026/duty_03_torso_s1`): with the same
decals at the same places the suit seen second would wear the other's baked textures. The pack adds
the Freedom model's ten files again, byte for byte, as `ported/freedom_026/freedom_exo_torso_s1`,
and points the Freedom body's config at it (its torso, frame and back unit in all four sleeve
tables; the legs are another model). The old files stay in players' archives, unused.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

PACK_ID = 'exo-decals'
VERSION = '1'
CONFIGS = 'gameplay/items/armour/torso'
OLD = 'ported/freedom_026/duty_03_torso_s1'
NEW = 'ported/freedom_026/freedom_exo_torso_s1'
# The Freedom body's model as released (patch 17): its files and their sha256.
LIVE = {
    'render/back_s/converted_model': 'dca0bc388d102c3751d037c22a0cf3f8fab6d77f9331ec91327aeee1f4cfcc2f',
    'render/back_s/export_properties': '2cf88996b97df1899d155dbeb37e085f988f3ae83ba4e46a94f8c266b720077e',
    'render/bind_pose': '72d6130cddfe7dfa66f46a5c9f709d4c0d068b57d2f53a9fb74757ed197c6e93',
    'render/exoskeleton_s/converted_model': '16e43baa71a55395d7a4f3624177982efb6ca1c3caafdd8b36bdaacdfe2ce75b',
    'render/exoskeleton_s/export_properties': '967f2355442a53fed17a751f7db019a582d451f296687c2bfcbe2026443f0a81',
    'render/export_properties': '4df0aafa9d3ad17587dfa5374d4abcfb2c97d8cfab0b094bf5edfedb8a3b8106',
    'render/torso_s/converted_model': '1b466d0c2d2071b5242bcdab61e7310c0bc84a770ff55e9f1c1dae4ab5a404d1',
    'render/torso_s/export_properties': 'a62b229a65ee8afa0d9892f10490f3f1bab7321bfa66a05be4396178d3aef935',
    'settings': '0374e55e982533800ca07aec8c53d43130869a45a3e4de28969e294399d79173',
    'skeleton': '2a952e1632dd4209fc310032c295ede5682f5e46488014d7837fcadf23acdd0c',
}
# The sleeve tables of a torso config (the gloves worn pick one), and which of the Freedom body's
# additional models are surfaces of the torso's model: the frame and the back unit (2 is the legs).
TABLES = ('data', 'data_full', 'data_short', 'data_long')
TORSO_PARTS = (0, 1)
DECALS = {'parameters.decal_available': True, 'data.decals': {'table': {}}}


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--live', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        sys.exit(f'{a.out} exists')
    operations = []
    files = {}
    for name, sha in LIVE.items():
        data = (a.live / f'models/{OLD}.skinned_model' / name).read_bytes()
        if hashlib.sha256(data).hexdigest() != sha:
            sys.exit(f'models/{OLD}.skinned_model/{name} is not the released file')
        path = f'models/{NEW}.skinned_model/{name}'
        files[path] = data
        operations.append({'op': 'add_file', 'path': path})
    models = {f'{table}.base_model': NEW for table in TABLES}
    models.update({f'{table}.additional_models.{i}.base_model': NEW for table in TABLES for i in TORSO_PARTS})
    operations += [
        {'op': 'config_set', 'path': f'{CONFIGS}/duty_exo_026.options', 'values': DECALS},
        {'op': 'config_set', 'path': f'{CONFIGS}/freedom_exo_026.options', 'values': {**DECALS, **models}},
    ]
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the Duty and Freedom exoskeleton bodies take decals as a stock vest does '
                       '(parameters/decal_available, an empty data/decals); the Freedom body\'s model under a file '
                       f'name of its own ({NEW}: the game names an item\'s baked textures after its model\'s file '
                       'name, which it shared with Duty\'s)',
        'operations': operations,
    }
    for path, data in files.items():
        target = a.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (a.out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(f'{a.out}: {len(files)} files, {len(operations)} operations')


if __name__ == '__main__':
    main()
