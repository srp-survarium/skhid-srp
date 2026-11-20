#!/usr/bin/env python3
"""Extend the pinned scope-fits donor with weapon meshes and complete mount chains.

Use extract_donor.rs built against the split-aware resource-porter. No archive
is modified. Materials use the existing 0.26 template and native texture slots;
the pack builder converts texture values for 0.26 rendering.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from audit import model
from inputs import fingerprint
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import retexture


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('base', 'live', 'archive', 'extractor', 'out'):
        parser.add_argument('--' + key, type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError('output must be a new directory')
    shutil.copytree(args.base, args.out)

    def extract(table, target):
        subprocess.run([str(args.extractor), str(args.archive), str(HERE / table), str(target)], check=True)

    extract('donor-paths.txt', args.out)
    extract('texture-paths.txt', args.out)
    with tempfile.TemporaryDirectory() as temporary:
        native = Path(temporary)
        extract('material-paths.txt', native)
        for source in native.rglob('*.material'):
            slots = {}

            def visit(value):
                if isinstance(value, dict):
                    for key, child in value.items():
                        if key in retexture.SLOTS.values():
                            slots[key] = child['value']
                        visit(child)

            visit(model.config(source))
            destination = args.out / source.relative_to(native)
            destination.parent.mkdir(parents=True, exist_ok=True)
            template = args.live / 'material_instances/weapons/loc2/picatini_medium.material'
            destination.write_bytes(retexture.made(template.read_bytes(), slots))
    print(json.dumps(fingerprint(args.out), sort_keys=True))


if __name__ == '__main__':
    main()
