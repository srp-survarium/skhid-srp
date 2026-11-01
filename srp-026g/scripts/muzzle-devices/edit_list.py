#!/usr/bin/env python3
"""edit_list.py <bundle's muzzle-edits.json> <content-edit/muzzle-devices-1.json>

The edit list of the muzzle devices (docs/notes/muzzle-devices.md): each gun with a built-in
device gets it as its stock device, data/srp_muzzle_stock_model and data/srp_muzzle_stock_fire_mm
(client patch 90 draws it when the weapon kit names no device), as port-muzzles wrote them.
"""
import json
import sys

src, out = sys.argv[1:3]
edits = json.load(open(src))
assert set(edits) == {'add_value'}, set(edits)
doc = {
    '_about': "Muzzle devices (srp-026g/docs/notes/muzzle-devices.md; made by "
              "scripts/muzzle-devices/make_pack.sh from resource-porter's port-muzzles): each gun whose "
              "mesh had a built-in device (now cut out of it, pack muzzle-devices) gets that device back as "
              "its stock device, data/srp_muzzle_stock_model (a static model at the gun's muzzle_point) and "
              "data/srp_muzzle_stock_fire_mm (where its shot's flash goes); client patch 90 draws it while "
              "the weapon kit names no device.",
    'add_value': edits['add_value'],
}
with open(out, 'w', encoding='utf-8') as f:
    f.write(json.dumps(doc, indent=1, ensure_ascii=False) + '\n')
