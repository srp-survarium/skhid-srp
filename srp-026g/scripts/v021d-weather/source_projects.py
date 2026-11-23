#!/usr/bin/env python3
"""Export the authored day maps for the weather porter, through build-content.

This deliberately rebuilds the content manifest, including its ordered edit
lists. It never takes a project from server-game-data or a release patch alone.
The weather outputs are excluded so regeneration always starts with the day
sources; all other manifest rows retain their original order.
"""
import argparse
import fnmatch
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


# The weather packs: the five maps' and Chemical Plant's, made from this export.
WEATHER_PACK = re.compile(r'maps-021d-(chemical-)?weathers@')


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


def without_files(edits, removed):
    """An edit list's entries without those naming a file of `removed` (the weather packs' files, and
    the copies an edit list makes of them, which this grows): a path, or each path of a list (with
    its count); a glob's file count less the files of `removed` it matches. None when unchanged."""
    def drop(entry):
        files = entry.get('file', entry.get('files'))
        if isinstance(files, list):
            kept = [i for i, path in enumerate(files) if path not in removed]
            if len(kept) == len(files):
                return entry
            if not kept:
                return None
            entry = dict(entry, files=[files[i] for i in kept])
            if 'expected_counts' in entry:
                entry['expected_counts'] = [entry['expected_counts'][i] for i in kept]
            return entry
        if not isinstance(files, str):
            return entry
        if files in removed:
            return None
        matched = len(fnmatch.filter(removed, files)) if any(c in files for c in '*?[') else 0
        if matched and isinstance(entry.get('expected_files'), int):
            entry = dict(entry, expected_files=entry['expected_files'] - matched)
        return entry
    for e in edits.get('copy', []):         # the list's copies are made first
        for path in fnmatch.filter(sorted(removed), e['files']):
            if path.startswith(e['from']):
                removed.add(e['to'] + path[len(e['from']):])
    changed = False
    out = {}
    for op, entries in edits.items():
        if not isinstance(entries, list) or op.startswith('_'):
            out[op] = entries
            continue
        kept = [e for e in (drop(e) if isinstance(e, dict) else e for e in entries) if e is not None]
        changed |= kept != entries
        out[op] = kept
    return out if changed else None


def day_edit_tree(work, rows, removed, root=ROOT, weather_rows=0):
    """View the original edit sources without the weather packs: no row of modes.tsv for their modes,
    no entry of an edit list for their files (without_files), the map table's counts less their rows."""
    tree = work / 'day-source'
    edits = tree / 'srp-026g/scripts/content-edit'
    edits.mkdir(parents=True)
    removed = set(removed)
    lists = [name for kind, name, *_ in rows if kind == 'edit' and name.endswith('.json')]
    made = {}
    for name in lists:                     # in the manifest's order: a copy of a removed file is removed too
        source = root / 'srp-026g/scripts/content-edit' / name
        original = source.read_text()
        kept = without_files(json.loads(original), removed)
        text = original if kept is None else json.dumps(kept, indent=1, ensure_ascii=False) + '\n'
        made[name] = shift_map_counts(text, -weather_rows) if weather_rows else text
    for source in (root / 'srp-026g/scripts/content-edit').iterdir():
        target = edits / source.name
        if source.name == 'modes.tsv':
            lines = source.read_text().splitlines(keepends=True)
            target.write_text(''.join(line for line in lines
                                      if 'gameplay/game_modes/' + line.split('\t')[0] + '.options' not in removed))
        elif source.name in made and made[source.name] != source.read_text():
            target.write_text(made[source.name])
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
    weather_rows, removed = 0, set()
    for kind, name, pin, *_ in rows:
        if kind == 'pack' and WEATHER_PACK.match(name):
            pack_id, version = name.split('@')
            recipe = args.packs / f'{pack_id}-{version}/pack.json'
            if file_sha256(recipe) != pin:
                raise RuntimeError('weather pack source pin differs')
            operations = json.loads(recipe.read_text())['operations']
            weather_rows += sum(len(op['entries']) for op in operations
                                if op['op'] == 'dictionary_add' and op['table'] == 'maps_dict')
            removed |= {op['path'] for op in operations if 'path' in op}
    # The renderer keys (exe patch 130) are laid over the finished projects, the weather packs' too:
    # they are no part of the authored day source the weather packs are made from.
    rows = [r for r in rows if not (r[0] == 'pack' and WEATHER_PACK.match(r[1]))
            and not (r[0] == 'edit' and r[1].startswith(('v021d-weather-', 'v021d-renderer-')))]
    base_rows = sorted((r for r in rows if r[0] == 'base'),
                       key=lambda r: int(build.PATCH.match(r[1]).group(1)))
    archive_of = {'resources.db': args.stock} | {r[1]: args.patches / r[1] for r in base_rows}
    sha_of = {'resources.db': file_sha256(args.stock)} | {r[1]: r[2] for r in base_rows}
    base = list(archive_of.values())
    reader = build.game_data_id(args.game_data, ROOT / 'srp-026g')
    with tempfile.TemporaryDirectory(prefix='v021-weather-source-') as tmp:
        work = Path(tmp)
        game = build.Game(args.game_data, work)
        cache = build.Cache(args.cache)
        world = build.World(cache)
        drawn = build.build_movies(rows, ROOT, archive_of, sha_of, game, args.flash_tools, work, world, cache)
        build.compose_packs(rows, base, list(sha_of.items()), args.porter, args.packs, work, world, cache, drawn)
        base_layers = build.Layers(game, cache, reader, base, sha_of)
        build.apply_edits(rows, day_edit_tree(work, rows, removed, weather_rows=weather_rows), base_layers, work, world, cache)
        selected = {path: world.data(path) for path in world
                    if path.startswith('projects/v021d_level_') and '/day_' in path
                    or path.startswith('gameplay/game_modes/v021d_level_') and '_day_' in path
                    or path == 'gameplay/db_static_dictionaries'}
        levels = {'01', '02', '03', '04', '05', '06'}
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
