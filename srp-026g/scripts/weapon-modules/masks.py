#!/usr/bin/env python3
"""The camouflage masks into a weapon-modules bundle (make_pack.sh, step 3).

usage: python3 -I masks.py <masks folder> <bundle>

<masks folder>  weapon_camo.py's out folder: textures/<texture>_aoac.dds and .options
<bundle>        port-modules' bundle: each mask is copied into its overlay and listed in its
                port-manifest.json, as the files the porter made are (pack from-bundle refuses a
                file the manifest does not list)
"""
import hashlib
import json
import shutil
import sys
from pathlib import Path


def main():
    masks, bundle = Path(sys.argv[1]), Path(sys.argv[2])
    manifest = json.loads((bundle / 'port-manifest.json').read_text())
    listed = {f['path'] for f in manifest['files']}
    added = 0
    for src in sorted(p for p in masks.rglob('*') if p.is_file()):
        path = src.relative_to(masks).as_posix()
        dst = bundle / 'overlay' / path
        assert path not in listed and not dst.exists(), f'{path}: the bundle has it'
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, dst)
        data = dst.read_bytes()
        manifest['files'].append({'path': path, 'sha256': hashlib.sha256(data).hexdigest(), 'size': len(data),
                                  'source': 'weapon_camo.py'})
        added += 1
    (bundle / 'port-manifest.json').write_text(json.dumps(manifest, indent=2))
    print(f'{added} mask files into the bundle')


if __name__ == '__main__':
    main()
