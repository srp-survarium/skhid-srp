#!/usr/bin/env python3
"""The remodel list: every live gun on 0.69d0's model (docs/notes/remodel-069.md).

usage: python3 -I sidecar.py <0.69d0 game leaves> <players' files> <out folder>

<0.69d0 game leaves>  `vostok-port dump-db-config --path gameplay/game.options --leaves` of the
                      0.69d0 split (one `key<TAB>type:value` a line)
<players' files>      the players' gameplay/db_static_dictionaries and gameplay/items/weapons/*
                      (rifle-scopes/prepare_live.sh's folder)
Writes <out folder>/remodel-069d.json (resource-porter's remodel sidecar) and items.tsv. Each live
gun item takes the 0.69d0 item of its name (0.69d0 has every one but the event MP7, ALIAS the
renamed ones); the guns of pack weapons-069 are 0.69d0's already. For each 0.69d0 item:
- rig: the live model's (its skeleton, bind pose and hand points: the live clips play on it);
  bone_map where a live bone is named otherwise (the VSS's spare magazine);
- bake: the default muzzle device or bipod the gun shows today, as a surface of its own (a later
  attachment leaves it out by name); separable: every other attachment (default optics, the
  premium items' bundled optics, lasers and other brakes): the kit's and the scopes';
- model_path: weapons/remodel_069/<item>/<item>, new to the players' files.
"""
import json
import struct
import sys
from pathlib import Path

ALIAS = {'a_545': 'a545', 'g3sg1': 'g3a3', 'ks_23m': 'ks_23', 'mauser_m712': 'mauser'}
ALREADY_069 = {'akmsu', 'famas', 'g3a3', 'mauser', 'ks_23', 'ppsh_drum'}  # pack weapons-069
KINDS = ('muzzlebrake', 'flashhider', 'silencer', 'suppressor', 'foregrip', 'scope', 'collimator')
SLOT_KINDS = ('muzzle', 'tactical', 'picatinny_grip', 'scope', 'collim', 'laser')
BAKED_KINDS = ('muzzlebrake', 'flashhider', 'barrelnut', 'firearrestor', 'suppressor', 'silencer', 'bipod')
BONE_MAP = {'vss': {'magazin3': 'magazin2'}, 'vss_premium': {'magazin3': 'magazin2'}}

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'sight-alignment'))
import model as cfgs  # noqa: E402


def leaves(path):
    out = {}
    for line in open(path, encoding='utf-8'):
        k, _, v = line.rstrip('\n').partition('\t')
        out[k] = v.partition(':')[2].strip('"')
    return out


def listed(D, prefix, field):
    out, i = [], 0
    while f'{prefix}[{i}].{field}' in D:
        out.append(D[f'{prefix}[{i}].{field}'])
        i += 1
    return out


def main():
    D = leaves(sys.argv[1])
    live, out = Path(sys.argv[2]), Path(sys.argv[3])
    weapons069 = {k.split('.')[1] for k in D if k.startswith('weapons.') and k.endswith('.object.model')}

    def attachment(mid):
        if 'ironsight' in mid:
            return False  # a sight stays baked, though it sits on a scope rail (the M4A1's rear one)
        model = D.get(f'weapon_modules.{mid}.model', '')
        slots = listed(D, f'weapon_modules.{mid}.slots_to_attach_to', 'slot')
        return (any(k in mid or f'/{k}' in model for k in KINDS)
                or bool(slots) and all(any(k in s for k in SLOT_KINDS) for s in slots))

    items = cfgs.config(live / 'gameplay/db_static_dictionaries')['items_dict']
    guns, rows = {}, []
    for r in sorted(items, key=lambda r: r['name']):
        if r['item_category'] not in (8, 13, 14, 15, 16, 17):
            continue
        name = r['name']
        w = name if name in weapons069 else ALIAS.get(name, ALIAS.get(name.rsplit('_premium', 1)[0]))
        if w not in weapons069:
            print(f'{name}: not in 0.69d0, kept', file=sys.stderr)
            continue
        if w in ALREADY_069:
            continue
        cfg = cfgs.config(live / r['cfg_name'])
        live_model = cfg['object']['model']
        model_path = f'weapons/remodel_069/{w}/{w}'
        rows.append(f"{name}\t{r['cfg_name']}\t{live_model}\t{w}\t{model_path}")
        if w in guns:
            continue
        defaults = listed(D, f'weapons.{w}.default_modules', 'module')
        modules = listed(D, f'weapons.{w}.bundled_modules', 'module') + defaults
        bake = [m for m in defaults if attachment(m) and any(k in m for k in BAKED_KINDS)]
        guns[w] = {'rig': {'live': f'models/{live_model}.skinned_model'},
                   'separable': [m for m in modules if attachment(m) and m not in bake],
                   'bake': bake, 'model_path': model_path}
        if w in BONE_MAP:
            guns[w]['bone_map'] = BONE_MAP[w]
    spec = {'format_version': 1, 'guns': guns, 'renames': {'clips': {}, 'sounds': {}}, 'clip_events': {}}
    (out / 'remodel-069d.json').write_text(json.dumps(spec, indent=1) + '\n')
    (out / 'items.tsv').write_text(
        '# made by sidecar.py: each live gun item, its config and model, its 0.69d0 item and its remodel\n'
        '# item\tconfig\tlive model\t0.69d0 item\tmodel\n' + '\n'.join(rows) + '\n')
    print(f'{len(guns)} 0.69d0 guns for {len(rows)} live items')


if __name__ == '__main__':
    main()
