#!/usr/bin/env python3
"""The live files scope_fits.py reads, out of the stock resources.db and the patches over it (the
last archive that has a file wins, as the game mounts them).

    python3 -I live_files.py --porter <vostok-port> --stock <resources.db> \
        [--target-patch <resources.db.patch_n>]... --out <folder>

The dictionary, every scope item's config, and of the AK-74M's variants their idle and aimed
models and their weapon models.
"""
import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model as M  # noqa: E402


class Layers:
    def __init__(self, porter, archives):
        self.porter = porter
        self.layers = [(a, set(subprocess.run([porter, 'list', '--db', a], check=True, capture_output=True,
                                              text=True).stdout.splitlines())) for a in archives]

    def prefix(self, prefix):
        return sorted({p for _, paths in self.layers for p in paths if p.startswith(prefix)})

    def get(self, path, out):
        db = next((db for db, paths in reversed(self.layers) if path in paths), None)
        if db is None:
            raise FileNotFoundError(path)
        dest = out / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run([self.porter, 'extract', '--db', db, '--path', path, '--output', str(dest)],
                       check=True, stdout=subprocess.DEVNULL)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--porter', required=True)
    ap.add_argument('--stock', required=True)
    ap.add_argument('--target-patch', action='append', default=[])
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    layers = Layers(args.porter, [args.stock] + args.target_patch)
    layers.get('gameplay/db_static_dictionaries', args.out)
    for path in layers.prefix('gameplay/items/scopes/'):
        layers.get(path, args.out)
    for path in sorted((args.out / 'gameplay/items/scopes').glob('*.options')):
        cfg = M.config(path)
        fit = cfg.get('srp_scope_fit')
        if not fit or ('ak_74m' not in fit['weapons'] and 3 not in fit['weapons']):
            continue
        models = [f'models/{cfg["data"]["idle_model"]}.model/', f'models/{cfg["data"]["aimed_model"]}.model/']
        models += [f'models/{m["value"]}.skinned_model/' for x in cfg['modifications'] for m in x['modifiers']
                   if m['path'] == ['object', 'model']]
        for prefix in models:
            for p in layers.prefix(prefix):
                layers.get(p, args.out)


if __name__ == '__main__':
    main()
