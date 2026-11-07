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
  bone_map where a live bone is named otherwise (the VSS's spare magazine); offset where 0.69d0
  placed the model otherwise on the rig (fit.py measures it); aim_069 where its iron sights
  stand otherwise than ours; optic where the live gun carries an optic by default (its config's rifle
  scope): its iron sights folded, as 0.69d0 folds them under an optic; else, if it has sights that
  fold, optic_model_path: the same model folded, at model_path + '_optic', for its scope variants;
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
# Where 0.69d0 placed its model otherwise on the same rig: the shift (metres, mesh space) that lays it
# on the live model (fit.py's; every other remodel lies on its live model to the millimetre).
UZI = [0.0, -0.0176, -0.0184]
OFFSET = {'a9_91': [0.0088, -0.0207, -0.0004], 'benelli_m4': [0.0, -0.0243, -0.0214],
          'mp_153': [0.0, -0.0222, -0.0012], 'spas_12': [0.0, -0.0203, -0.0044], 'toz_34': [0.0, -0.0022, -0.0008],
          'uzi': UZI, 'uzi_premium': UZI, 'uzi_premium_2': UZI}
# Where 0.69d0's iron sights stand otherwise than ours on the same body (the Uzi's taller rear sight):
# the aim on 0.69d0's own sight line (aim_069), as the renders compare them.
AIM_069 = {'uzi', 'uzi_premium', 'uzi_premium_2'}

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

    # What 0.69d0's optics (its _scope and _collimator modules) do (attach_params.slots_to_change_param):
    # the slots they fold (param 1, the iron sights) and those they take (their own slots: a sight there,
    # the M4A1's rear one, makes way).
    optics = {k.split('.')[1] for k in D if '.attach_params.slots_to_change_param[' in k
              and k.split('.')[1].endswith(('_scope', '_collimator'))}
    folding = {D[k[:-len('param')] + 'id'] for k, v in D.items()
               if k.split('.')[1] in optics and '.attach_params.slots_to_change_param[' in k
               and k.endswith('].param') and v == '1'}
    taken = {s for m in optics for s in listed(D, f'weapon_modules.{m}.slots_to_attach_to', 'slot')}

    def folds(mid):
        """Whether an optic changes the module: folds it (a model_param1 of its own) or takes its slot."""
        model = D.get(f'weapon_modules.{mid}.model')
        slots = listed(D, f'weapon_modules.{mid}.slots_to_attach_to', 'slot')
        return (D.get(f'weapon_modules.{mid}.model_param1', model) != model and any(s in folding for s in slots)
                or any(s in taken for s in slots))

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
        if w in OFFSET:
            guns[w]['offset'] = OFFSET[w]
        if w in AIM_069:
            guns[w]['aim_069'] = True
        if cfg.get('addons', {}).get('rifle_scope_dict_id') is not None:
            guns[w]['optic'] = True
        elif any(folds(m) for m in modules if m not in guns[w]['separable']):
            guns[w]['optic_model_path'] = model_path + '_optic'
    spec = {'format_version': 1, 'guns': guns, 'renames': {'clips': {}, 'sounds': {}}, 'clip_events': {}}
    (out / 'remodel-069d.json').write_text(json.dumps(spec, indent=1) + '\n')
    (out / 'items.tsv').write_text(
        '# made by sidecar.py: each live gun item, its config and model, its 0.69d0 item and its remodel\n'
        '# item\tconfig\tlive model\t0.69d0 item\tmodel\n' + '\n'.join(rows) + '\n')
    print(f'{len(guns)} 0.69d0 guns for {len(rows)} live items')


if __name__ == '__main__':
    main()
