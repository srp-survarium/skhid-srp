#!/usr/bin/env python3
"""Export the authored day maps for the weather porter, through build-content.

This deliberately rebuilds the content manifest, including its ordered edit
lists. It never takes a project from server-game-data or a release patch alone.
The weather outputs are excluded so regeneration always starts with the day
sources; all other manifest rows retain their original order.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'release'))
import build_content as build


def file_sha256(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


WEATHER_MODE = re.compile(r'v021d_level_(01|02|03|05|06)_(rain|evn)_[24]\t')


def shift_map_counts(text, delta):
    """Adjust the checked map-table lengths without rewriting unrelated source."""
    expected = json.loads(text)
    for row in expected.get('append_entry', []):
        if row.get('list') == 'maps_dict':
            row['expected_count'] += delta
    pattern = r'("list"\s*:\s*"maps_dict"\s*,\s*"expected_count"\s*:\s*)(\d+)'
    changed = re.sub(pattern, lambda m: m[1] + str(int(m[2]) + delta), text)
    if json.loads(changed) != expected:
        raise ValueError('map dictionary count layout changed; update the source export')
    return changed


def day_edit_tree(work, root=ROOT, weather_rows=0):
    """View the original edit sources without rows belonging to the excluded pack."""
    tree = work / 'day-source'
    edits = tree / 'srp-026g/scripts/content-edit'
    edits.mkdir(parents=True)
    for source in (root / 'srp-026g/scripts/content-edit').iterdir():
        target = edits / source.name
        if source.name == 'modes.tsv':
            target.write_text(''.join(line for line in source.read_text().splitlines(keepends=True)
                                      if not WEATHER_MODE.match(line)))
        elif weather_rows and source.suffix == '.json':
            original = source.read_text()
            changed = shift_map_counts(original, -weather_rows)
            if changed == original:
                target.symlink_to(source)
            else:
                target.write_text(changed)
        else:
            target.symlink_to(source, target_is_directory=source.is_dir())
    (tree / 'srp-026g/client-patches').symlink_to(root / 'srp-026g/client-patches', target_is_directory=True)
    return tree


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ('stock', 'patches', 'game_data', 'porter', 'packs', 'flash_tools', 'cache', 'out'):
        p.add_argument('--' + arg.replace('_', '-'), type=Path, required=True)
    args = p.parse_args()
    if args.out.exists():
        p.error('output already exists')
    offered = build.read_list(ROOT / 'release/patches.tsv')
    rows = build.read_manifest(ROOT / 'release/content.tsv', offered, ROOT)
    weather_rows = 0
    for kind, name, pin, *_ in rows:
        if kind == 'pack' and name.startswith('maps-021d-weathers@'):
            pack_id, version = name.split('@')
            recipe = args.packs / f'{pack_id}-{version}/pack.json'
            if file_sha256(recipe) != pin:
                raise RuntimeError('weather pack source pin differs')
            weather_rows += sum(len(op['entries']) for op in json.loads(recipe.read_text())['operations']
                                if op['op'] == 'dictionary_add' and op['table'] == 'maps_dict')
    rows = [r for r in rows if not (r[0] == 'pack' and r[1].startswith('maps-021d-weathers@'))
            and not (r[0] == 'edit' and r[1].startswith('v021d-weather-'))]
    base_rows = sorted((r for r in rows if r[0] == 'base'),
                       key=lambda r: int(build.PATCH.match(r[1]).group(1)))
    archive_of = {'resources.db': args.stock} | {r[1]: args.patches / r[1] for r in base_rows}
    sha_of = {'resources.db': file_sha256(args.stock)} | {r[1]: r[2] for r in base_rows}
    base = list(archive_of.values())
    base_id = [build.game_data_id(args.game_data, ROOT / 'srp-026g')] + list(sha_of.items())
    with tempfile.TemporaryDirectory(prefix='v021-weather-source-') as tmp:
        work = Path(tmp)
        game = build.Game(args.game_data, work)
        world, cache = build.World(), build.Cache(args.cache)
        build.build_movies(rows, ROOT, archive_of, sha_of, game, args.flash_tools, work, world, cache)
        build.compose_packs(rows, base, base_id, args.porter, args.packs, work, world, cache)
        base_files = game.files(base)
        build.apply_edits(rows, day_edit_tree(work, weather_rows=weather_rows), base, base_id, base_files, game, work, world, cache)
        selected = {path: data for path, data in world.bytes.items()
                    if path.startswith('projects/v021d_level_') and '/day_' in path
                    or path.startswith('gameplay/game_modes/v021d_level_') and '_day_' in path
                    or path == 'gameplay/db_static_dictionaries'}
        levels = {'01', '02', '03', '05', '06'}
        required = {f'projects/v021d_level_{level}/day_{mode}/client_project'
                    for level in levels for mode in (2, 4)}
        if missing := required - selected.keys():
            raise RuntimeError(f'manifest did not make every day project: {sorted(missing)}')
        for path, data in selected.items():
            dest = args.out / path
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
        (args.out / 'source.json').write_text(json.dumps({
            'manifest_sha256': hashlib.sha256((ROOT / 'release/content.tsv').read_bytes()).hexdigest(),
            'files': {path: hashlib.sha256(data).hexdigest() for path, data in sorted(selected.items())},
        }, indent=2) + '\n')
        print(f'{len(selected)} authored resources exported to {args.out}')


if __name__ == '__main__':
    main()
