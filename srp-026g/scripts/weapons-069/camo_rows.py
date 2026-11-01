#!/usr/bin/env python3
"""camo_rows.py: guns.tsv rows (weapon-camo's format) for the weapons of a resource-porter bundle.

usage: python3 -I camo_rows.py <bundle>  >guns.tsv
       python3 -I camo_rows.py <bundle> --add <weapon_camo.py out folder>

Every diffuse texture the bundle's weapon models draw, with the surfaces whose UVs use it, no
donors (0.69d's weapons have no 0.26g premium): weapon_camo.py paints the large parts of each
(its suggestion; a parts.tsv beside the table picks them by hand). The cartridges (`ammo/`) and a
pump's loose shell are left bare.
"""
import json
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'weapon-camo'))
from weapon_camo import Files, config_value  # noqa: E402

bundle = sys.argv[1]
if sys.argv[2:3] == ['--add']:
    # weapon_camo.py's _aoac files into the bundle's overlay and its manifest's files.
    import hashlib
    import shutil
    out, manifest_path = sys.argv[3], os.path.join(bundle, 'port-manifest.json')
    manifest = json.load(open(manifest_path))
    for dirpath, _, names in os.walk(out):
        for name in sorted(names):
            src = os.path.join(dirpath, name)
            path = os.path.relpath(src, out).replace(os.sep, '/')
            dst = os.path.join(bundle, 'overlay', *path.split('/'))
            assert not os.path.exists(dst), path
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copyfile(src, dst)
            data = open(src, 'rb').read()
            manifest['files'].append({'path': path, 'sha256': hashlib.sha256(data).hexdigest(),
                                      'size': len(data), 'source': 'weapon_camo.py'})
    manifest['files'].sort(key=lambda f: f['path'])
    open(manifest_path, 'w').write(json.dumps(manifest, indent=2) + '\n')
    sys.exit(0)
files = Files(os.path.join(bundle, 'overlay'))
rows = {}
for item in json.load(open(os.path.join(bundle, 'port-manifest.json')))['items']:
    root = item['model_root']
    model = root[len('models/'):-len('.skinned_model')]
    settings = files.read(root + '/settings')
    for surface in files.surfaces(model):
        if surface.startswith('shell'):
            continue
        material = config_value(settings, 'material_settings/%s/material_name' % surface)
        texture = config_value(files.read('material_instances/%s.material' % material),
                               'material/g_stage/effect/texture_diffuse/value')
        if '/ammo/' in texture:
            continue
        rows.setdefault(texture, []).append('%s:%s' % (model, surface))
print('# The 0.69d weapons\' textures that get an _aoac (weapon_camo.py), made by camo_rows.py')
for texture, surfaces in sorted(rows.items()):
    print('%s\t\t%s' % (texture, ','.join(surfaces)))
