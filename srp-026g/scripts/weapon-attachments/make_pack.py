#!/usr/bin/env python3
"""make_pack.py: the content pack of the weapon attachments, the weapon kit's muzzle devices and
grips (srp-026g/docs/notes/weapon-kit.md; client patch 68 draws them).

usage: python3 -I make_pack.py --stock <folder> --donor <folder> --out <packs folder>/weapon-attachments-<version>

--stock  0.26g's template, at its game path, from the stock resources.db:
         gameplay/items/upgrades/camouflages/black_market/camo_black_leather.options.
--donor  the files prepare_donor.sh makes from Survarium 0.46c2 (every path of donor.sha256).
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

What the pack holds, for each device of devices.tsv:
- its model (0.46c2's: the static mesh as it is, 0.26g's own layout of 28-byte vertices; its
  configs in 0.26g's 24-byte layout, each material entry of its settings given 0.26g's
  `used_in_portals` false), its material (made again on 0.26g's pp9_klin template) and its
  textures (0.46c2's, their .options in 0.26g's layout);
- its item config, made from 0.26g's camouflage camo_black_leather by content_edit.py's edits: the
  type `srp_muzl` (the server's: slot 4, survarium::v026g::modifications), the modification id
  (260000 + its dict id's last three digits) whose modifier adds `srp_muzzle_model` = the model
  (where the camouflage's adds `camouflage` = its texture) to the weapon config's `data` (client
  patch 68 reads it), the name and description keys, the icon's cell (a shop scope's for now);
  read back, nothing else differs;
- its dictionary row (category 22, the scopes': the lobby's attachments filter) and its texts in
  the six languages (texts.tsv).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

PACK_ID = 'weapon-attachments'
VERSION = '1'
LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')
TEMPLATE = 'gameplay/items/upgrades/camouflages/black_market/camo_black_leather.options'
STOCK = {TEMPLATE: '101a46acb6cb7e974777dbaeff20cb9f041aa0d02a00622f43f609d3c4762b74'}
TEMPLATE_VALUES = {'type': 'stk_camo', 'modification': 100001, 'key': 'camouflage',
                   'texture': 'character/human/camo/black_market/black_leather',
                   'name': 'st_camo_black_leather_name', 'description': 'st_camo_generic_desc', 'icon': (1, 1)}
ITEM_CONFIGS = 'gameplay/items/attachments/'
CATEGORY = 22
KINDS = {'flash_hider': {'type': 'srp_muzl', 'key': 'srp_muzzle_model', 'description': 'st_srp_flash_hider_desc'}}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def porter_text(text):
    """The game keeps texts as UTF-8 bytes; the porter reads and writes a config's strings as
    Windows-1251 (WHATWG's: byte 0x98 is U+0098), so a pack names the UTF-8 bytes so read."""
    raw = text.encode('utf-8')
    return ''.join('\x98' if b == 0x98 else bytes([b]).decode('cp1251') for b in raw)


def read_inputs(folder, wanted, what):
    found = {}
    for root, _, names in os.walk(folder):
        for name in names:
            rel = os.path.relpath(os.path.join(root, name), folder).replace(os.sep, '/')
            found[ce.game_path(rel)] = os.path.join(root, name)
    out = {}
    for path, sha in wanted.items():
        if path not in found:
            sys.exit(f'{what}: no {path} in {folder}')
        data = Path(found[path]).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'{what}: {path} is not the file this pack is made of (sha256 {sha})')
        out[path] = data
    return out


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def read_devices():
    devices = []
    for dict_id, name, kind, model, english, icon_row, icon_col in rows('devices.tsv'):
        if kind not in KINDS:
            sys.exit(f'devices.tsv: {name}: no kind {kind}')
        stem = model.removeprefix('models/').removesuffix('.model')
        devices.append({'dict_id': int(dict_id), 'name': name, 'kind': kind, 'model': model, 'english': english,
                        'model_name': stem, 'icon': (int(icon_row), int(icon_col)),
                        'config': ITEM_CONFIGS + name + '.options',
                        'modification': 260000 + int(dict_id) % 1000,
                        'key': f'st_{name}_name'})
    return devices


def read_texts():
    texts = {}
    header, *table = rows('texts.tsv')
    if tuple(header) != ('key',) + LANGUAGES:
        sys.exit(f'texts.tsv: the columns are key and {", ".join(LANGUAGES)}')
    for row in table:
        texts[row[0]] = dict(zip(LANGUAGES, row[1:]))
    return texts


def item_config(template, device):
    """The template with the device's values; read back, every other value as it was."""
    kind = KINDS[device['kind']]
    t = TEMPLATE_VALUES
    buf = bytearray(template)
    ce.set_number(buf, 'modifications[0]/id', t['modification'], device['modification'])
    ce.set_number(buf, 'parameters/modification', t['modification'], device['modification'])
    ce.set_number(buf, 'ui_desc/icon[0]', t['icon'][0], device['icon'][0])
    ce.set_number(buf, 'ui_desc/icon[1]', t['icon'][1], device['icon'][1])
    for frm, to in ((t['type'], kind['type']), (t['key'], kind['key']), (t['texture'], device['model_name']),
                    (t['name'], device['key']), (t['description'], kind['description'])):
        if ce.replace_string(buf, frm, to) != 1:
            sys.exit(f'{TEMPLATE} does not hold {frm!r} once')
    old = {p: (ty, v) for p, ty, v in ce.walk(bytearray(template))}
    new = {p: (ty, v) for p, ty, v in ce.walk(buf)}
    want = {
        '/modifications[0]/id': (ce.T_INT, device['modification']),
        '/parameters/modification': (ce.T_INT, device['modification']),
        '/ui_desc/icon[0]': (ce.T_INT, device['icon'][0]),
        '/ui_desc/icon[1]': (ce.T_INT, device['icon'][1]),
        '/parameters/type': (ce.T_STR, kind['type'].encode()),
        '/modifications[0]/modifiers[0]/value[0]': (ce.T_STR, kind['key'].encode()),
        '/modifications[0]/modifiers[0]/value[1]': (ce.T_STR, device['model_name'].encode()),
        '/ui_desc/text_descriptions/name': (ce.T_STR, device['key'].encode()),
        '/ui_desc/text_descriptions/description': (ce.T_STR, kind['description'].encode()),
    }
    if set(old) != set(new) or not set(want) <= set(new):
        sys.exit(f'{device["name"]}: the item config lost or gained a value')
    for path in old:
        expected = want.get(path, old[path])
        if new[path] != expected:
            sys.exit(f'{device["name"]}: {path} is {new[path]}, not {expected}')
    return bytes(buf)


def model_settings(data, surfaces):
    """The model's settings with 0.26g's `used_in_portals` (false) in each material entry."""
    buf = bytearray(data)
    for surface in surfaces:
        ce.add_value(buf, f'material_settings/{surface}', 'used_in_portals', False)
    return bytes(buf)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    devices = read_devices()
    texts = read_texts()
    donor_sums = {}
    for line in (HERE / 'donor.sha256').read_text().splitlines():
        sha, path = line.split(None, 1)
        donor_sums[path.strip()] = sha
    stock = read_inputs(args.stock, STOCK, '--stock')
    donor = read_inputs(args.donor, donor_sums, '--donor')

    files = {}
    for device in devices:
        files[device['config']] = item_config(stock[TEMPLATE], device)
        model = device['model']
        surface = Path(device['model_name']).name + '_s'
        prefix = model + '/'
        for path, data in donor.items():
            if path.startswith(prefix):
                files[path] = model_settings(data, [surface]) if path == model + '/settings' else data
        stem = device['model_name']
        for path, data in donor.items():
            if path == f'material_instances/{stem}.material' or path.startswith(f'textures/{stem}/'):
                files[path] = data

    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [
        {'base_cost': 0, 'cfg_name': d['config'], 'dict_id': d['dict_id'], 'is_premium': False, 'is_stack': False,
         'item_category': CATEGORY, 'name': d['name']} for d in devices]})
    keys = sorted({d['key'] for d in devices} | {KINDS[d['kind']]['description'] for d in devices})
    for language in LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {key: porter_text(texts[key][language]) for key in keys}})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the weapon kit\'s attachments from 0.46c2 (srp-026g/docs/notes/weapon-kit.md): '
                       + ', '.join(f'{d["dict_id"]} {d["english"]}' for d in devices),
        'operations': operations,
        'server': {'items': [{'dict_id': d['dict_id']} for d in devices]},
        'notes': [
            'made by srp-026g/scripts/weapon-attachments/make_pack.py from devices.tsv, texts.tsv and donor.sha256',
            'donor: Survarium 0.46c2 (build 4349), prepare_donor.sh',
            'item configs: 0.26g camo_black_leather with the type, the modification and its modifier\'s key and '
            'value, the texts and the icon cell',
        ] + [f'{d["dict_id"]} {d["name"]}: 0.46c2 {d["model"]} ({d["english"]})' for d in devices],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files)} files, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
