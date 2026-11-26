#!/usr/bin/env python3
"""Version the weapon-modules pack with the files a newer porter makes differently, nothing else.

usage: pack_over.py --from <packs>/weapon-modules-<n> --bundle <port-modules bundle> --version <m> --note <text>
       pack_over.py --from <packs>/weapon-modules-<n> --made <pack> --record <modules-069d.json> --version <m> --note <text>

The bundle is port-modules' output (make_pack.sh step 2, or a quick port of some guns with --only)
from a porter whose change touches what every gun shares (a module's model or material), not the
table or the masks. Every file of the source pack that the bundle makes with other bytes is
replaced; a file the bundle makes that the pack lacks is added, unless a pack of
release/content.tsv has it with the same bytes (pack from-bundle leaves out what players have).
The modules table stays the pack's (a quick port's holds its guns alone; a change to the table is
make_pack.sh's), as do the camouflage masks (made after the port). Writes <packs>/weapon-modules-<m>
(refused if it is there; the files that stay are hard links to the source's) with the source's
record of the port beside it, and prints its pack.json sha256 for release/content.tsv.

With --made, a change to the table too: <pack> is `vostok-port pack from-bundle` of a full port's
bundle (every gun, make_pack.sh's steps 2 and 4 without the masks: what players lack of it, against
the files make_pack.sh packs against), --record that port's record. Every file of <pack> the source
pack has with other bytes, the table among them, is replaced, every one it lacks is added; the
camouflage masks and every other file stay the source's (a patch takes no file off), and the record
beside the new version is --record.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil

TABLE = Path('gameplay/weapon_modules.srpm')
ROOT = Path(__file__).resolve().parents[3]


def players_have(packs, path, data):
    """A pack of the manifest (other than weapon-modules) holds `path` with these bytes."""
    for line in (ROOT / 'release/content.tsv').read_text().splitlines():
        kind, _, rest = line.partition('\t')
        if kind != 'pack' or rest.startswith('weapon-modules@'):
            continue
        pack_id, version = rest.split('\t', 1)[0].split('@')
        f = packs / f'{pack_id}-{version}' / 'files' / path
        if f.is_file() and f.read_bytes() == data:
            return True
    return False


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--from', dest='src', type=Path, required=True)
    made_by = ap.add_mutually_exclusive_group(required=True)
    made_by.add_argument('--bundle', type=Path)
    made_by.add_argument('--made', type=Path)
    ap.add_argument('--record', type=Path)
    ap.add_argument('--version', required=True)
    ap.add_argument('--note', required=True)
    args = ap.parse_args()
    pack = json.loads((args.src / 'pack.json').read_text())
    assert pack['id'] == 'weapon-modules', pack['id']
    dst = args.src.parent / f"{pack['id']}-{args.version}"
    assert not dst.exists(), f'{dst}: refusing to replace a pack'
    assert (args.made is None) == (args.record is None), '--made goes with --record'
    have = {Path(op['path']) for op in pack['operations']}
    assert all(op['op'] == 'add_file' for op in pack['operations'])
    if args.made:
        overlay = args.made / 'files'
        made_pack = json.loads((args.made / 'pack.json').read_text())
        assert all(op['op'] == 'add_file' for op in made_pack['operations'])
        made = sorted(Path(op['path']) for op in made_pack['operations'])
        assert TABLE in made, f'{args.made} has no table: no full port'
    else:
        overlay = args.bundle / 'overlay'
        made = sorted(p.relative_to(overlay) for p in overlay.rglob('*') if p.is_file())
    changed = [f for f in made if f in have and (args.made or f != TABLE)
               and (overlay / f).read_bytes() != (args.src / 'files' / f).read_bytes()]
    added = [f for f in made if f not in have and not players_have(args.src.parent, f, (overlay / f).read_bytes())]
    assert changed or added, 'the bundle makes every file as the pack has it'
    shutil.copytree(args.src / 'files', dst / 'files', copy_function=os.link)
    for f in changed + added:
        target = dst / 'files' / f
        target.unlink(missing_ok=True)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(overlay / f, target)
        print(('replaced ' if f in have else 'added ') + str(f))
    pack['version'] = args.version
    pack['description'] = f"{pack['description']}; @{args.version}: {args.note}"
    pack['operations'] = sorted(pack['operations'] + [{'op': 'add_file', 'path': str(f)} for f in added],
                                key=lambda op: op['path'])
    (dst / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    record = args.record or args.src.parent / f"{args.src.name}.modules-069d.json"
    if record.is_file():
        shutil.copyfile(record, args.src.parent / f'{dst.name}.modules-069d.json')
    print(f"{pack['id']}@{args.version}\t{hashlib.sha256((dst / 'pack.json').read_bytes()).hexdigest()}"
          f"\t{len(changed)} files replaced, {len(added)} added; every other file the source's")


if __name__ == '__main__':
    main()
