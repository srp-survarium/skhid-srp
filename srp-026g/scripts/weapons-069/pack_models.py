#!/usr/bin/env python3
"""Version a weapons pack with the model files a newer port makes, nothing else.

usage: pack_models.py --from <packs>/<id>-<n> --bundle <port bundle> --version <m> --note <text>

The bundle is port-weapons-069d's output (make_pack.sh step 1) from a porter whose
change touches models only. Every file of the source pack that the bundle's overlay
makes with other bytes is replaced; each must be a model file, and every other file
(the icons, which later steps drew, included) stays byte-identical. Writes
<packs>/<id>-<m> (refused if it is there) and prints its pack.json sha256 for
release/content.tsv.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--from', dest='src', type=Path, required=True)
    ap.add_argument('--bundle', type=Path, required=True)
    ap.add_argument('--version', required=True)
    ap.add_argument('--note', required=True)
    args = ap.parse_args()
    pack = json.loads((args.src / 'pack.json').read_text())
    dst = args.src.parent / f"{pack['id']}-{args.version}"
    assert not dst.exists(), f'{dst}: refusing to replace a pack'
    overlay = args.bundle / 'overlay'
    files = sorted(p.relative_to(args.src / 'files') for p in (args.src / 'files').rglob('*') if p.is_file())
    missing = [f for f in files if not (overlay / f).is_file()]
    assert not missing, f'the bundle lacks {missing[:5]}: not a port of this pack'
    changed = [f for f in files if (overlay / f).read_bytes() != (args.src / 'files' / f).read_bytes()]
    assert changed, 'the bundle makes every file as the pack has it'
    outside = [f for f in changed if f.parts[0] != 'models']
    assert not outside, f'not model files: {outside}'
    shutil.copytree(args.src, dst)
    for f in changed:
        shutil.copyfile(overlay / f, dst / 'files' / f)
        print(f'replaced {f}')
    pack['version'] = args.version
    pack['notes'].append(args.note)
    (dst / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    differ = {p.relative_to(dst) for p in dst.rglob('*')
              if p.is_file() and p.read_bytes() != (args.src / p.relative_to(dst)).read_bytes()}
    assert differ == {Path('pack.json')} | {Path('files') / f for f in changed}, differ
    print(f"{pack['id']}@{args.version}\t{hashlib.sha256((dst / 'pack.json').read_bytes()).hexdigest()}"
          f"\t{len(changed)} model files; other files identical")


if __name__ == '__main__':
    main()
