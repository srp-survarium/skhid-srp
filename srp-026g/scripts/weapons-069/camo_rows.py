#!/usr/bin/env python3
"""camo_rows.py: camo.tsv (weapon-camo's guns.tsv format) for the guns of a weapons-069 bundle.

usage: python3 -I camo_rows.py <bundle>  >camo.tsv

Every diffuse texture the bundle's gun models draw (its donor-069d.json lists the models), with
the surfaces whose UVs use it, no donors (0.69d0's guns have no 0.26g premium): weapon_camo.py
paints the large parts of each (its suggestion; parts.tsv beside the table picks them by hand). The
cartridges (`ammo/`) and a pump's loose shell are rows too: parts.tsv paints none of them, and
weapon_camo.py --bare-masks gives each a mask that paints nothing, so a camouflaged gun's bake finds
every surface's _aoac (weapons-031's lesson: a surface without one fails to load).
"""
import json
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'weapon-camo'))
from weapon_camo import Files, config_value  # noqa: E402

bundle = sys.argv[1]
files = Files(os.path.join(bundle, 'overlay'))
rows = {}
for item in json.load(open(os.path.join(bundle, 'donor-069d.json'))).values():
    root = item['model_root']
    model = root[len('models/'):-len('.skinned_model')]
    settings = files.read(root + '/settings')
    for surface in files.surfaces(model):
        material = config_value(settings, 'material_settings/%s/material_name' % surface)
        texture = config_value(files.read('material_instances/%s.material' % material),
                               'material/g_stage/effect/texture_diffuse/value')
        rows.setdefault(texture, []).append('%s:%s' % (model, surface))
print('# The weapons-069 guns\' textures that get an _aoac (weapon_camo.py), made by camo_rows.py')
for texture, surfaces in sorted(rows.items()):
    print('%s\t\t%s' % (texture, ','.join(surfaces)))
