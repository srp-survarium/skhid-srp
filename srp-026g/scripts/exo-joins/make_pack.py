#!/usr/bin/env python3
"""make_pack.py: the content pack that closes the exoskeleton pieces' joins with the other armour
(the owner, 2026-11-27: "there is a hole when I wear exoskeleon helment and some armor", "there is
a hole between legs and boots"; ../../docs/notes/exo-joins.md).

usage: python3 -I make_pack.py --live <folder> --out <packs folder>/exo-joins-<version>

--live  the released files at their game paths (`srp-game-data get --db <stock> --patch <each listed
        patch>... --out <folder> --paths <a file of LIVE's paths>`), each checked by its sha256.
--out   the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the manifest's
        `pack` row (release/content.tsv).

The STK2 exoskeleton was one suit; Skhid wears its helmet, body and boots as pieces of their own,
with any other armour. Each piece still ends where its exo partner took over:

- The helmets (Duty and Freedom: one mesh, their own textures) hide the head
  (`remove_head_model`), and with it the native neck every stock collar is made around; the exo
  body's own collar was their neck. The pack gives each helmet a neck of its own, an additional
  model (`exo_helmet_neck`, surface `neck_s`): the native head's neck from its bottom (the stock
  collar line) up into the helmet's shell, without the eyes, kept inside the shell, in the dark
  ribbed rubber of the gas mask's hood (the helmet's texture), skinned as the native neck.
- The legs (the exo body's additional model; Duty's and Freedom's are one mesh under two names)
  end in a closed bottom at 0.21 m, inside the exo boot's 0.22 m shaft: over every lower boot the
  ankle was empty. The pack moves that bottom 0.105 m down (to 0.11 m, under the lowest boot's
  top, 0.151 m) and stretches the trousers above it, as far as 0.42 m, to meet it: the same
  vertices, triangles and UVs (the camouflage masks and decals stay where they are), the skin
  near the ankle the native full-length trousers' Leg/Foot blend; below 0.24 m they narrow towards
  the native tucked trousers (`_s3`), so they go into tall shafts as stock trousers do. Duty's
  file in place; Freedom's, which pack exo-camo added (a later pack may not replace an earlier
  one's file), as a model of its own, `freedom_exo_legs_s2`, which edit list exo-joins-1.json
  points the Freedom body at.

Each file it replaces is checked against the released one (LIVE); the configs are set by path.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402  (in-place edits of the models' binary configs)
from headwear import bind_records  # noqa: E402
import numpy as np  # noqa: E402
import joins  # noqa: E402

PACK_ID = 'exo-joins'
VERSION = '1'
SUITS = ('duty', 'freedom')
HELMETS = 'gameplay/items/armour/helmet'
HEAD = 'models/character/human/base/base_character_head.skinned_model/render/black_market_01_head_s/converted_model'
ANKLE = 'models/character/human/base/base_character_legs_s3.skinned_model/render/base_character_legs_s/converted_model'
BIND = 'models/character/human/base/base_character_legs_s3.skinned_model/render/bind_pose'
# The shins in the render bind: {side: (Leg, Foot)}, side 1 the +x leg.
SHIN_BONES = {1: (15, 16), -1: (10, 11)}
EXO_HEAD = 'models/ported/{suit}_026/duty_03_head.skinned_model'
NECK = 'ported/{suit}_026/exo_helmet_neck'
NECK_PART = 'neck_s'
LEGS = {'duty': 'models/ported/duty_026/duty_03_legs_s1.skinned_model',
        'freedom': 'models/ported/freedom_026/freedom_exo_legs_s1.skinned_model'}
# Where each suit's lowered legs go: Duty's in place (a file of patch 17's duty-exo); Freedom's,
# which pack exo-camo added (a later pack may not replace an earlier one's file), as a model of
# their own that edit list exo-joins-1.json points the Freedom body at.
LOWERED = {'duty': LEGS['duty'], 'freedom': 'models/ported/freedom_026/freedom_exo_legs_s2.skinned_model'}
# The model's files that the lowering leaves as they are.
KEPT = ('render/bind_pose', 'settings', 'skeleton')
# The released files read (patch 17's packs duty-exo and freedom-exo, exo-camo's Freedom legs).
LIVE = {
    ANKLE:
        '9fd60c4cbec122e519fcae763275c16b5c3d77d83bc97eed44245eee413d63f2',
    BIND:
        '72d6130cddfe7dfa66f46a5c9f709d4c0d068b57d2f53a9fb74757ed197c6e93',
    HEAD:
        'b1cbdc0283005d97a0e4cc2e21b52895550ba06f74ebc3cdb4ed03d0f25c7263',
    'models/ported/duty_026/duty_03_head.skinned_model/render/bind_pose':
        '72d6130cddfe7dfa66f46a5c9f709d4c0d068b57d2f53a9fb74757ed197c6e93',
    'models/ported/duty_026/duty_03_head.skinned_model/render/export_properties':
        'f14d5078051e311e022019c6140e99b810f768b22ce2f57e787623286e1414c2',
    'models/ported/duty_026/duty_03_head.skinned_model/render/helmet_s/converted_model':
        'be5d253d400caab32e95a8e15f7b4b105a701a6070800466f1f24c11d0a929b4',
    'models/ported/duty_026/duty_03_head.skinned_model/render/helmet_s/export_properties':
        'f1f572d45ca30bef9095bc29c4594e2014bc0ebf159817d4d38354deb7952092',
    'models/ported/duty_026/duty_03_head.skinned_model/settings':
        '4121a0a04a976792f324c40a212d2eec6c93b821483c86dda2b4211c8402c793',
    'models/ported/duty_026/duty_03_head.skinned_model/skeleton':
        '2a952e1632dd4209fc310032c295ede5682f5e46488014d7837fcadf23acdd0c',
    'models/ported/freedom_026/duty_03_head.skinned_model/render/bind_pose':
        '72d6130cddfe7dfa66f46a5c9f709d4c0d068b57d2f53a9fb74757ed197c6e93',
    'models/ported/freedom_026/duty_03_head.skinned_model/render/export_properties':
        'f14d5078051e311e022019c6140e99b810f768b22ce2f57e787623286e1414c2',
    'models/ported/freedom_026/duty_03_head.skinned_model/render/helmet_s/converted_model':
        'be5d253d400caab32e95a8e15f7b4b105a701a6070800466f1f24c11d0a929b4',
    'models/ported/freedom_026/duty_03_head.skinned_model/render/helmet_s/export_properties':
        'f1f572d45ca30bef9095bc29c4594e2014bc0ebf159817d4d38354deb7952092',
    'models/ported/freedom_026/duty_03_head.skinned_model/settings':
        '194abbb0847ddeff877d4f4bcdb969272bc0a22a63cd0b358b5d1602e6409d4f',
    'models/ported/freedom_026/duty_03_head.skinned_model/skeleton':
        '2a952e1632dd4209fc310032c295ede5682f5e46488014d7837fcadf23acdd0c',
    'models/ported/duty_026/duty_03_legs_s1.skinned_model/render/legs_s/converted_model':
        '2aa237d776d2ce625777fcacf2f0fb774e697e50e32066fc4af20eb3ada0c1a8',
    'models/ported/duty_026/duty_03_legs_s1.skinned_model/render/legs_s/export_properties':
        'ef66e7e0d6cf485f370c41414a8300aa6f2ebf4fa4df859e31f97cbfb8057ec3',
    'models/ported/duty_026/duty_03_legs_s1.skinned_model/render/export_properties':
        '6922840be31cab7a87413c409493ffb368e7d8cc45ee922a4ed1efd45de1470e',
    'models/ported/freedom_026/freedom_exo_legs_s1.skinned_model/render/legs_s/converted_model':
        '2aa237d776d2ce625777fcacf2f0fb774e697e50e32066fc4af20eb3ada0c1a8',
    'models/ported/freedom_026/freedom_exo_legs_s1.skinned_model/render/legs_s/export_properties':
        'ef66e7e0d6cf485f370c41414a8300aa6f2ebf4fa4df859e31f97cbfb8057ec3',
    'models/ported/freedom_026/freedom_exo_legs_s1.skinned_model/render/export_properties':
        '6922840be31cab7a87413c409493ffb368e7d8cc45ee922a4ed1efd45de1470e',
    'models/ported/freedom_026/freedom_exo_legs_s1.skinned_model/render/bind_pose':
        '72d6130cddfe7dfa66f46a5c9f709d4c0d068b57d2f53a9fb74757ed197c6e93',
    'models/ported/freedom_026/freedom_exo_legs_s1.skinned_model/settings':
        '83dfb88d2232d0b49f5dcf98e6b8256aee81dd595de14f5d53cfea1bbfdc88e2',
    'models/ported/freedom_026/freedom_exo_legs_s1.skinned_model/skeleton':
        '2a952e1632dd4209fc310032c295ede5682f5e46488014d7837fcadf23acdd0c',
    f'{HELMETS}/duty_exo_026.options':
        'fd0aebde0ee811b607529446085ff27a7b23ce980980832b15a66828e5eabfa2',
    f'{HELMETS}/freedom_exo_026.options':
        '90bc1cddafcccd7b4e05732f78f7ba6fc991671ac83b5e2b6ccb4709126ff260',
}
# The neck: how far up into the helmet it goes above the rim (metres), and its place in the
# helmet's 2048 px texture (u0, v0, u1, v1): the ribbed rubber of the gas mask's hood, ridges along u.
NECK_RISE = 0.04
NECK_UV = (50 / 2048, 372 / 2048, 380 / 2048, 500 / 2048)
# The legs: the closed bottom (below CAP_TOP) moved DROP down, the trousers up to BLEND_TOP
# stretched to meet it; below TUCK_TOP narrowed, over TUCK_SPAN, to TUCK_GAP outside the native
# tucked trousers.
DROP, CAP_TOP, BLEND_TOP = 0.105, 0.245, 0.42
TUCK_TOP, TUCK_SPAN, TUCK_GAP = 0.24, 0.05, 0.004


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def bounds(mesh):
    return mesh.pos.min(0).astype('f4').tolist(), mesh.pos.max(0).astype('f4').tolist()


def vector(buf, path):
    return list(ce.value_of(buf, ce.find(buf, path)))


def with_bounds(data, mesh, counts=False):
    """A model's or surface's export_properties with the mesh's bounding box (and, for a
    surface, its vertex, index and triangle counts)."""
    buf = bytearray(data)
    lo, hi = bounds(mesh)
    ce.set_vector(buf, 'bounding_box/min', vector(buf, 'bounding_box/min'), lo)
    ce.set_vector(buf, 'bounding_box/max', vector(buf, 'bounding_box/max'), hi)
    if counts:
        for key, value in (('vertices', len(mesh.pos)), ('indices', 3 * len(mesh.idx)),
                           ('primitives', len(mesh.idx))):
            path = f'source/{key}'
            ce.set_number(buf, path, ce.value_of(buf, ce.find(buf, path)), value)
    return bytes(buf)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--live', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        sys.exit(f'{a.out} exists')
    live = {}
    for path, sha in LIVE.items():
        data = (a.live / path).read_bytes()
        if sha and sha256(data) != sha:
            sys.exit(f'{path} is not the released file')
        live[path] = data
    files, operations = {}, []

    head = joins.read_mesh(live[HEAD])
    for suit in SUITS:
        exo_head = EXO_HEAD.format(suit=suit)
        helmet = joins.read_mesh(live[f'{exo_head}/render/helmet_s/converted_model'])
        neck, _ = joins.neck_piece(head, helmet, NECK_RISE, NECK_UV)
        model = f'models/{NECK.format(suit=suit)}.skinned_model'
        settings = bytearray(live[f'{exo_head}/settings'])
        assert ce.rename_key(settings, 'material_settings', 'helmet_s', NECK_PART)
        ce.set_string(settings, 'lod_hierrarchy/LOD0/surfaces[0]', 'helmet_s', NECK_PART)
        surface = bytearray(with_bounds(live[f'{exo_head}/render/helmet_s/export_properties'], neck, True))
        ce.set_string(surface, 'source/mesh_name',
                      '|duty_03_head|vis|duty_03_head_LOD0|duty_03_head_LOD0Shape', '|exo_helmet_neck|neck_s')
        made = {
            'render/bind_pose': live[f'{exo_head}/render/bind_pose'],
            'render/export_properties': with_bounds(live[f'{exo_head}/render/export_properties'], neck),
            f'render/{NECK_PART}/converted_model': joins.write_mesh(neck),
            f'render/{NECK_PART}/export_properties': bytes(surface),
            'settings': bytes(settings),
            'skeleton': live[f'{exo_head}/skeleton'],
        }
        for name, data in made.items():
            files[f'{model}/{name}'] = data
            operations.append({'op': 'add_file', 'path': f'{model}/{name}'})
        operations.append({'op': 'config_set', 'path': f'{HELMETS}/{suit}_exo_026.options', 'values': {
            'data.additional_models': [{'table': {'base_model': NECK.format(suit=suit), 'part_name': NECK_PART}}],
        }})

    ankle = joins.read_mesh(live[ANKLE])
    frames = [np.frombuffer(frame, '<f4', 3) for _, frame in bind_records(live[BIND])]
    shins = {side: (frames[leg], frames[foot]) for side, (leg, foot) in SHIN_BONES.items()}
    for suit in SUITS:
        model = LEGS[suit]
        mesh_path = f'{model}/render/legs_s/converted_model'
        legs = joins.read_mesh(live[mesh_path])
        if joins.write_mesh(legs) != live[mesh_path]:
            sys.exit(f'{mesh_path}: not written back byte for byte')
        lowered = joins.lower_hem(legs, DROP, CAP_TOP, BLEND_TOP, ankle, shins, TUCK_TOP, TUCK_SPAN, TUCK_GAP)
        made = {
            'render/legs_s/converted_model': joins.write_mesh(lowered),
            'render/legs_s/export_properties':
                with_bounds(live[f'{model}/render/legs_s/export_properties'], lowered, True),
            'render/export_properties': with_bounds(live[f'{model}/render/export_properties'], lowered),
        }
        target = LOWERED[suit]
        if target == model:
            for name, data in made.items():
                path = f'{model}/{name}'
                files[path] = data
                operations.append({'op': 'replace_file', 'path': path, 'base_sha256': sha256(live[path])})
        else:
            for name in KEPT:
                made[name] = live[f'{model}/{name}']
            for name, data in sorted(made.items()):
                files[f'{target}/{name}'] = data
                operations.append({'op': 'add_file', 'path': f'{target}/{name}'})

    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the Duty and Freedom exoskeleton helmets with a neck of their own (exo_helmet_neck: '
                       'the native neck, in the gas mask hood\'s rubber), so they no longer float over '
                       'other bodies; the exoskeleton legs\' closed bottom moved 0.105 m down into every boot '
                       '(the trousers stretched to meet it and narrowed into the ankle, UVs kept), so no gap shows '
                       'over low boots (Freedom\'s as freedom_exo_legs_s2: edit list exo-joins-1.json)',
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
