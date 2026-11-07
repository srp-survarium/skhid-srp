#!/usr/bin/env python3
"""The live scope variants on the remodels (docs/notes/remodel-069.md): pack remodel-069-variants and
content-edit/remodel-069-variants-1.json.

usage: python3 -I variants.py <players' files> <remodel bundle> <out pack folder> <version>

<players' files>  rifle-scopes/prepare_live.sh's folder (the dictionary, the scope items, every weapon model)
<remodel bundle>  make_pack.sh's bundle (overlay/: the remodels)
A scope variant of ours that sets its weapon's model to a copy of the gun's live model (its eye moved
onto the optic, or the Kedr's scope point added) drew the old gun on a remodelled one. Each such copy is
made again from the remodel: the remodel's files, with every locator the copy set otherwise than the
gun's live model (its aim, a scope point) set as the copy has it. The remodel keeps the live model's aim
and scope point (remodel-069d.json: live contract locators), so the copy's optic and eye stay where they
were. The copy goes to weapons/remodel_069/<0.69d0 item>/<the copy's name>; the edit list points the
variant there. A variant that keeps its gun's model follows the remodel by itself; one for a gun and its
premium draws the first gun's remodel on both, as it draws the first gun's copy today.
"""
import hashlib
import json
import shutil
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[0] / 'sight-alignment'))
import model as cfgs  # noqa: E402
from model import ce  # noqa: E402

TOLERANCE = 1e-6


def set_float3(buf, path, value):
    """A float3 at `path` in three numbers of its own (the format keeps equal values once)."""
    off = ce.find(buf, path)
    _, name, crc, kind, count = ce.rec(buf, off)
    buf.extend(bytes(-len(buf) % 4))
    ce.REC.pack_into(buf, off, ce.append(buf, struct.pack('<3f', *value)), name, crc, kind, count)


def locators(directory):
    """A model's locators by name: (index, position, rotation)."""
    found = cfgs.config(directory / 'render/export_properties')['locators']
    return {raw['name']: (i, raw['position'], raw.get('rotation', [0.0, 0.0, 0.0])) for i, raw in enumerate(found)}


def main():
    live, bundle, out, version = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4]
    if out.exists():
        sys.exit(f'{out}: already there')
    rows = [line.split('\t') for line in (HERE / 'items.tsv').read_text().splitlines()
            if line and not line.startswith('#')]
    remodel = {item: (live_model, w, model) for item, _, live_model, w, model in rows}
    items = cfgs.config(live / 'gameplay/db_static_dictionaries')['items_dict']
    name_of = {r['dict_id']: r['name'] for r in items}
    made, edits, report = {}, [], []
    for r in sorted(items, key=lambda r: r['name']):
        if r['item_category'] != 22:
            continue
        cfg = cfgs.config(live / r['cfg_name'])
        if 'srp_scope_fit' not in cfg:
            continue
        guns = [name_of.get(w, w) for w in cfg['srp_scope_fit']['weapons']]
        copies = [m['value'] for mod in cfg.get('modifications', []) for m in mod.get('modifiers', [])
                  if m.get('path', [None, None])[1:2] == ['model']]
        # A variant for a gun and its premium sets one copy for both (of the first gun's model, as today).
        if not copies or not all(g in remodel for g in guns):
            continue
        live_model, w, model = remodel[guns[0]]
        copy = copies[0]
        if copy == live_model:
            continue
        if copy not in made:
            new = f'weapons/remodel_069/{w}/{copy.rsplit("/", 1)[-1]}'
            src = bundle / 'overlay/models' / (model + '.skinned_model')
            dst = out / 'files/models' / (new + '.skinned_model')
            shutil.copytree(src, dst)
            base = locators(cfgs.model_dir(live, live_model))
            theirs = locators(cfgs.model_dir(live, copy))
            ours = locators(dst)
            props = dst / 'render/export_properties'
            buf = bytearray(props.read_bytes())
            changed = []
            for name, (_, position, rotation) in sorted(theirs.items()):
                b = base.get(name)
                if b and max(abs(x - y) for x, y in zip(position + rotation, b[1] + b[2])) < TOLERANCE:
                    continue  # as the gun's live model: the remodel has it so already
                if name not in ours:
                    sys.exit(f'{copy}: its locator {name} is none of the remodel {model}\'s')
                i = ours[name][0]
                set_float3(buf, f'locators[{i}]/position', position)
                set_float3(buf, f'locators[{i}]/rotation', rotation)
                changed.append(name)
            props.write_bytes(bytes(buf))
            made[copy] = new
            report.append(f'{copy}\t{new}\t{",".join(changed) or "-"}')
        edits.append({'files': r['cfg_name'], 'from': copy, 'to': made[copy]})
    files = sorted(str(p.relative_to(out / 'files')) for p in (out / 'files').rglob('*') if p.is_file())
    pack = {'format_version': 1, 'id': 'remodel-069-variants', 'version': version, 'target_build': '0.26g',
            'description': 'the live scope variants\' weapon models on the 0.69d0 remodels (srp-026g/docs/notes/remodel-069.md): '
                           f'{len(made)} copies of the remodels with their variants\' eyes',
            'operations': [{'op': 'add_file', 'path': p} for p in files],
            'notes': ['made by srp-026g/scripts/remodel-069/variants.py over pack remodel-069']}
    (out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    (HERE / 'variants.tsv').write_text(
        '# made by variants.py: each live variant weapon model copy, its remodelled copy, the locators it sets\n'
        '# copy\tremodelled\tlocators\n' + '\n'.join(report) + '\n')
    edit = {'_about': 'The live scope variants that set their weapon\'s model to a copy of it (an eye on the optic) '
                      'draw the remodel\'s copy instead (pack remodel-069-variants, remodel-069/variants.py; '
                      'docs/notes/remodel-069.md). After remodel-069-1.json.',
            'replace_string': edits}
    (HERE.parent / 'content-edit/remodel-069-variants-1.json').write_text(json.dumps(edit, indent=1) + '\n')
    print(f'remodel-069-variants@{version}: {len(made)} copies, {len(edits)} variants; pack.json sha256 '
          f'{hashlib.sha256((out / "pack.json").read_bytes()).hexdigest()}')


if __name__ == '__main__':
    main()
