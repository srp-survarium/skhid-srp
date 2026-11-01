#!/usr/bin/env python3
"""The 0.69d0 files the muzzle-device port reads: game.options, five languages' localization, every
muzzle-device module model (brakes, flash hiders, barrel nuts; no suppressors) with its materials and
textures, and every weapon's skinned model render folder (base meshes and locators).

    donor_paths.py <survey vostok-port> <full069.split> <paths069.txt> <out>
"""
import subprocess
import sys

P, SPLIT, PATHS, OUT = sys.argv[1:5]
paths = [p for p in open(PATHS).read().split('\n') if p]
have = set(paths)
need = {'gameplay/game.options'}
for lang in ('english', 'german', 'polish', 'russian', 'ukrainian'):
    need.add(f'localization/{lang}/localization')


def dump(p):
    r = subprocess.run([P, 'dump-db-config', '--db', SPLIT, '--path', p, '--leaves'],
                       capture_output=True, text=True)
    d = {}
    for line in r.stdout.splitlines():
        k, _, v = line.partition('\t')
        d[k] = v.split(':', 1)[1].strip('"') if ':' in v else v
    return d


modules = [p for p in paths if p.startswith(('models/weapons/modules/muzzlebrakes/',
                                             'models/weapons/modules/firearrestors/',
                                             'models/weapons/modules/barrels/'))]
need.update(modules)
materials = set()
for p in modules:
    if p.endswith('.model/settings'):
        for k, v in dump(p).items():
            if k.startswith('material_settings.') and k.endswith('.material_name') and v:
                materials.add(v)
for m in sorted(materials):
    path = f'material_instances/{m}.material'
    if path not in have:
        print('missing', path, file=sys.stderr)
        continue
    need.add(path)
    for k, v in dump(path).items():
        if k.startswith('material.') and k.endswith('.value') and v and '/' in v:
            for ext in ('.dds', '.options'):
                t = f'textures/{v}{ext}'
                if t in have:
                    need.add(t)
need.update(p for p in paths if p.startswith('models/weapons/') and '.skinned_model/render/' in p)
open(OUT, 'w').write('\n'.join(sorted(need)) + '\n')
print(len(need), 'paths;', len(materials), 'materials', file=sys.stderr)
