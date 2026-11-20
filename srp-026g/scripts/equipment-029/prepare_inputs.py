#!/usr/bin/env python3
"""Extract pinned inputs with srp-game-data (including directory hard links).

--donor names the directory of 0.29c resources.00.db .. resources.18.db from
Survarium_archives/Survarium29c/game in the owner's Survarium_archives.zip.
--stock is the untouched 0.26g resources.db. --patches holds the published
patches in base-patches.tsv. No server data or release working folder is used.
"""
import argparse
from collections import defaultdict
import hashlib
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def listing(binary, layers):
    result = subprocess.run([str(binary), 'files', *layers], check=True, capture_output=True, text=True)
    return {line.split('\t')[0] for line in result.stdout.splitlines()}


def extract(binary, layers, paths, destination):
    pathlist = destination.with_suffix('.paths')
    pathlist.parent.mkdir(parents=True, exist_ok=True)
    pathlist.write_text(''.join(path + '\n' for path in sorted(paths)))
    subprocess.run([str(binary), 'get', *layers, '--paths', str(pathlist), '--out', str(destination)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('donor', 'stock', 'patches', 'game-data-bin', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out must be a new directory')
    sums = {}
    groups = defaultdict(set)
    for line in (HERE / 'inputs.sha256').read_text().splitlines():
        checksum, key = line.split('  ', 1)
        group, path = key.split('/', 1)
        if '..' in Path(path).parts or Path(path).is_absolute():
            raise ValueError(f'unsafe resource path: {path}')
        sums[key] = checksum
        groups[group].add(path)
    volumes = [args.donor / f'resources.{i:02}.db' for i in range(19)]
    owners = {}
    for volume in volumes:
        for path in listing(args.game_data_bin, ['--db', str(volume)]):
            owners[path] = volume
    base_layers = ['--db', str(args.stock)]
    for line in (HERE / 'base-patches.tsv').read_text().splitlines():
        if not line or line.startswith('#'):
            continue
        name, checksum = line.split('\t')
        path = args.patches / name
        if digest(path) != checksum:
            raise ValueError(f'{path}: published patch checksum changed')
        base_layers += ['--patch', str(path)]
    args.out.mkdir(parents=True)
    try:
        for group, paths in groups.items():
            if group == 'base':
                extract(args.game_data_bin, base_layers, paths, args.out / group)
                continue
            by_volume = defaultdict(list)
            for path in sorted(paths):
                by_volume[owners[path]].append(path)
            for index, (volume, selected) in enumerate(sorted(by_volume.items())):
                stage = args.out / '.extract' / f'{group}-{index}'
                extract(args.game_data_bin, ['--db', str(volume)], selected, stage)
                for path in selected:
                    target = args.out / group / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(stage / path, target)
        for path, expected in sums.items():
            if digest(args.out / path) != expected:
                raise ValueError(f'{path}: extracted bytes differ from the pinned input')
        print(f'{len(sums)} pinned resource inputs verified in {args.out}')
    finally:
        shutil.rmtree(args.out / '.extract', ignore_errors=True)


if __name__ == '__main__':
    main()
