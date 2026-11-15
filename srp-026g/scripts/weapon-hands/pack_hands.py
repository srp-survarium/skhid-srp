#!/usr/bin/env python3
"""Version a weapons pack with the hands clips a newer port makes, nothing else.

usage: pack_hands.py --from <packs>/<id>-<n> --bundle <port bundle> --version <m> --note <text>

The bundle is the weapon port's output (`vostok-port port-weapons` / `port-weapons-069d`) from
a porter whose hands step (resource-porter `src/weapons_026/hands.rs`) makes the guns' hands
clips: the hand IK baked in, a pistol's clips re-based onto 0.26g's stance. Taken from the
bundle: every hands clip (a file under `animations/` of a `/player/` folder) it makes otherwise
than the pack or that the pack lacks, and every weapon config whose only difference from the
pack's is its states' `user_animations` (a gun playing its own hands clips). Every other file
of the pack stays byte-identical, whatever else the bundle makes otherwise (later porter
changes, files later steps drew). Writes <packs>/<id>-<m> (refused if it is there), prints each
file taken and the pack.json sha256 for release/content.tsv.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'content-edit'))
import content_edit as ce  # noqa: E402


def hands_clip(f):
    return f.parts[0] == 'animations' and 'player' in f.parts and f.suffix in ('.b-spline', '.bones_names')


def only_hands_differ(theirs, ours):
    """Two configs that differ in their states' user_animations lists, and nowhere else."""
    a, b = ce.walk(bytearray(theirs)), ce.walk(bytearray(ours))
    if [p for p, _, _ in a] != [p for p, _, _ in b]:
        return False
    return all(x == y or ('/states/' in x[0] and 'user_animations[' in x[0]) for x, y in zip(a, b))


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
    overlay, files = args.bundle / 'overlay', args.src / 'files'
    made = sorted(p.relative_to(overlay) for p in overlay.rglob('*') if p.is_file())
    have = {p.relative_to(files) for p in files.rglob('*') if p.is_file()}
    clips = [f for f in made if hands_clip(f)
             and (f not in have or (overlay / f).read_bytes() != (files / f).read_bytes())]
    configs = [f for f in made if f in have and f.parts[:3] == ('gameplay', 'items', 'weapons')
               and f.name.startswith('wpn_') and (overlay / f).read_bytes() != (files / f).read_bytes()
               and only_hands_differ((files / f).read_bytes(), (overlay / f).read_bytes())]
    assert clips, 'the bundle makes every hands clip as the pack has it'
    # Every hands clip a taken config names that the bundle makes is in the pack or taken (the
    # others are the players' files: the bundle reuses them).
    made_set = set(made)
    for f in configs:
        for path, t, v in ce.walk(bytearray((overlay / f).read_bytes())):
            if t == ce.T_STR and 'user_animations[' in path and '/states/' in path:
                clip = v.decode().removeprefix('resources/')
                for ext in ('.b-spline', '.bones_names'):
                    p = Path(clip + ext)
                    assert p in have or p in clips or p not in made_set, f'{f} names {p}, not shipped'
    shutil.copytree(args.src, dst)
    added = []
    for f in clips + configs:
        (dst / 'files' / f).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(overlay / f, dst / 'files' / f)
        if f not in have:
            added.append(f)
        print(f"{'added' if f not in have else 'replaced'} {f}")
    ops = pack['operations']
    for f in added:
        ops.append({'op': 'add_file', 'path': f.as_posix()})
    adds = sorted((o for o in ops if o['op'] == 'add_file'), key=lambda o: o['path'])
    rest = [o for o in ops if o['op'] != 'add_file']
    pack['operations'] = adds + rest
    pack['version'] = args.version
    pack['notes'].append(args.note)
    (dst / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    differ = {p.relative_to(dst) for p in dst.rglob('*') if p.is_file()
              and (not (args.src / p.relative_to(dst)).is_file()
                   or p.read_bytes() != (args.src / p.relative_to(dst)).read_bytes())}
    assert differ == {Path('pack.json')} | {Path('files') / f for f in clips + configs}, differ
    print(f"{pack['id']}@{args.version}\t{hashlib.sha256((dst / 'pack.json').read_bytes()).hexdigest()}"
          f"\t{len(clips)} hands clip files ({len(added)} new), {len(configs)} configs; other files identical")


if __name__ == '__main__':
    main()
