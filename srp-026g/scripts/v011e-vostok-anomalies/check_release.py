#!/usr/bin/env python3
"""Extract a normal builder's candidate and verify the complete Vostok maps."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

import make_pack


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tool', required=True)
    parser.add_argument('--db', required=True)
    parser.add_argument('--patch', action='append', default=[])
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--packs', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    layers = ['--db', args.db]
    for patch in args.patch:
        layers += ['--patch', patch]
    listing = subprocess.check_output([args.tool, 'files', *layers], text=True)
    paths = {line.split()[1] for line in (make_pack.HERE / 'inputs.sha256').read_text().splitlines()}
    paths.update(('gameplay/game_modes/level_03_day_5.options', 'gameplay/db_static_dictionaries'))
    for ident in ('radar-v011e-seams-1', 'radar-v011e-textures-1'):
        manifest = json.loads((args.packs / ident / 'pack.json').read_text())
        paths.update(op['path'] for op in manifest['operations'])
    paths.update(line.split('\t')[0] for line in listing.splitlines()
                 if line.startswith('models/ported/v011e/level/03/terrain_p_')
                 and line.split('\t')[0].endswith('/settings'))
    reference = args.work / 'reference'
    composed = args.work / 'composed'
    path_file = args.work / 'paths.txt'
    path_file.write_text('\n'.join(sorted(paths)) + '\n')
    subprocess.run([args.tool, 'get', *layers, '--paths', str(path_file), '--out', str(reference)], check=True)
    paths.update(('projects/v011e_level_03/day_5/client_project',
                  'gameplay/game_modes/v011e_level_03_day_5.options'))
    path_file.write_text('\n'.join(sorted(paths)) + '\n')
    subprocess.run([args.tool, 'get', *layers, '--patch', args.candidate,
                    '--paths', str(path_file), '--out', str(composed)], check=True)
    subprocess.run([sys.executable, str(make_pack.HERE / 'check_composed.py'),
                    '--reference', str(reference), '--composed', str(composed),
                    '--packs', str(args.packs)], check=True)


if __name__ == '__main__':
    main()
