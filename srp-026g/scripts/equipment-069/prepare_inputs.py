#!/usr/bin/env python3
"""Gather the pack's pinned inputs (inputs.sha256): resource-porter's models, the 0.69d icons,
the released game's files the items start from and the equipment-046 pieces copied whole.

--bundle is resource-porter's `vostok-port build-armour-069d` output (its overlay of models,
materials and textures, port-manifest.json, fit-report.json; README.md says how it is built).
--source069 holds the 0.69d files extracted at their archive paths (the icons are read from it).
--stock is the untouched 0.26g resources.db and --patches holds the published patches of
base-patches.tsv. --equipment046 is equipment-046's built pack (its folder, as its make_pack.py
writes it): the configs of the pieces copied whole and the surfaces they draw. No server data or
release working folder is used.

--pin computes the inputs from items.tsv and the bundle and writes inputs.sha256; without it
the extracted bytes are checked against it.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

from common import (BUNDLE_RECORDS, DICTIONARY, DONOR_ICONS, E046_MANIFEST, HERE, LOCALIZATIONS, MODIFICATIONS, MOVIE,
                    bcw, e046_entries, model_parts, sha, sound_file, specs, template)


def base_layers(stock, patches):
    """srp-game-data's layers of the released game: the stock db and base-patches.tsv's patches."""
    layers = ['--db', str(stock)]
    for line in (HERE / 'base-patches.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            name, checksum = line.split('\t')
            path = Path(patches) / name
            if sha(path.read_bytes()) != checksum:
                raise ValueError(f'{path}: published patch checksum changed')
            layers += ['--patch', str(path)]
    return layers


def extract(binary, layers, paths, out):
    """`paths` of the released game under `out`."""
    out.parent.mkdir(parents=True, exist_ok=True)
    listing = out.parent / f"{out.name}.paths"
    listing.write_text(''.join(p + '\n' for p in sorted(set(paths))))
    subprocess.run([str(binary), 'get', *layers, '--paths', str(listing), '--out', str(out)],
                   check=True, capture_output=True)
    listing.unlink()


def bundle_files(bundle):
    """The overlay's files, each as port-manifest.json records it."""
    record = json.loads((bundle / 'port-manifest.json').read_text())
    files = {}
    for entry in record['files']:
        raw = (bundle / 'overlay' / entry['path']).read_bytes()
        if sha(raw) != entry['sha256']:
            raise ValueError(f"{entry['path']}: not the bytes port-manifest.json records")
        files[entry['path']] = raw
    on_disk = {str(p.relative_to(bundle / 'overlay')) for p in (bundle / 'overlay').rglob('*') if p.is_file()}
    if on_disk != set(files):
        raise ValueError(f'overlay and port-manifest.json differ: {sorted(on_disk ^ set(files))[:3]}')
    return files


def pin(args, layers, work):
    """Every input the pack reads, by group."""
    rows = specs()
    base = {DICTIONARY, MODIFICATIONS, MOVIE, *LOCALIZATIONS}
    extract(args.game_data_bin, layers, [DICTIONARY], work / 'dictionary')
    native = bcw.loads((work / 'dictionary' / DICTIONARY).read_bytes())['items_dict']
    # Every armour config: the templates, and the balance audit's peers.
    armour = {row['cfg_name'] for row in native if '/armour/' in row['cfg_name']}
    base |= armour
    extract(args.game_data_bin, layers, sorted(armour), work / 'armour')
    by_id = {row['dict_id']: row for row in native}
    e046 = e046_entries(json.loads((args.equipment046 / E046_MANIFEST).read_bytes()))
    copied = {E046_MANIFEST}
    for row in rows:
        start = template(row)
        if start.copy:
            path = 'files/' + e046[start.ident]['cfg_name']
            config = bcw.loads((args.equipment046 / path).read_bytes())
            copied.add(path)
            # The surfaces it draws: the copy's are checked to be the same meshes.
            copied |= {f"files/models/{part['base_model']}.skinned_model/render/{part['part_name']}/converted_model"
                       for part in model_parts(config)}
        else:
            config = bcw.loads((work / 'armour' / by_id[start.ident]['cfg_name']).read_bytes())
        base |= {sound_file(*sound) for sound in config.get('sounds', [])}
    donor = {DONOR_ICONS.format(row=row['icon_069'].split(',')[0], col=row['icon_069'].split(',')[1]) for row in rows}
    bundle = set(bundle_files(args.bundle)) | {f'.records/{name}' for name in BUNDLE_RECORDS}
    return {'base': base, 'donor069': donor, 'bundle': bundle, 'equipment046': copied}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('bundle', 'source069', 'stock', 'patches', 'game-data-bin', 'equipment046', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--pin', action='store_true', help='compute the inputs and pin them')
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out must be a new directory')
    args.out.mkdir(parents=True)
    layers = base_layers(args.stock, args.patches)
    if args.pin:
        groups = pin(args, layers, args.out / '.pin')
        shutil.rmtree(args.out / '.pin')
    else:
        groups = {}
        for line in (HERE / 'inputs.sha256').read_text().splitlines():
            group, path = line.split('  ', 1)[1].split('/', 1)
            groups.setdefault(group, set()).add(path)
    extract(args.game_data_bin, layers, groups['base'], args.out / 'base')
    for group, root in (('donor069', args.source069), ('equipment046', args.equipment046)):
        for path in groups[group]:
            target = args.out / group / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(root / path, target)
    files = bundle_files(args.bundle)
    for path in groups['bundle']:
        target = args.out / 'bundle' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if path.startswith('.records/'):
            shutil.copyfile(args.bundle / path.removeprefix('.records/'), target)
        else:
            target.write_bytes(files[path])
    keys = {f'{group}/{path}' for group, paths in groups.items() for path in paths}
    if args.pin:
        lines = sorted(f'{sha((args.out / key).read_bytes())}  {key}' for key in keys)
        (HERE / 'inputs.sha256').write_text(''.join(line + '\n' for line in lines))
        print(f'{len(keys)} inputs pinned in inputs.sha256')
    else:
        sums = dict(reversed(line.split('  ', 1)) for line in (HERE / 'inputs.sha256').read_text().splitlines())
        for key, expected in sums.items():
            if sha((args.out / key).read_bytes()) != expected:
                raise ValueError(f'{key}: extracted bytes differ from the pinned input')
        print(f'{len(keys)} pinned inputs verified in {args.out}')


if __name__ == '__main__':
    main()
