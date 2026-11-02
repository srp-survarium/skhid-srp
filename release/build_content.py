#!/usr/bin/env python3
"""The files of players' resources.db as release/content.tsv says they should be, against what
players have: release/build-content's work (run it, not this; --help there).

    python3 -I build_content.py --stock <resources.db> --patches <folder> --list <patches.tsv>
        --bin <srp-game-data> --tree <checkout> --manifest <content.tsv> --work <new folder>
        [--tools <tools.json>] [--vostok-port <program>] [--packs <folder>]

The manifest's rows, in this order of kinds (each kind's rows in their order):
  base   resources.db.patch_<n>  <its sha256>  a published patch made outside these sources, taken
         as it is: laid over the stock resources.db in ascending n, it is the base
  movie  <name>                  -             srp-026g/flash/<name> compiled over its original's
         graphics (the original from the archive its movie.json names: the stock db or a base row)
  pack   <id>@<version>          <sha256 of its pack.json> | resources.db.patch_<n>
         a resource-porter content pack, composed over the base and the movies by
         `vostok-port pack patch` (its folder <packs>/<id>-<version>); or, with a base row's name,
         a record: the pack is in that published patch, nothing is applied
  edit   <file>.json             - | resources.db.patch_<n>
         srp-026g/scripts/content-edit/<file>.json applied by content_edit.py to the files as the
         rows above (and the edits before it) leave them; or, with a base row's name, a record;
         (a <table>.tsv too: content_edit.py STATS)
Every Flash tree of srp-026g/flash needs a movie row and every edit list of content-edit a row.

Writes into --work: world/ (every file a movie, pack or edit made, at its path), patch/ (those of
them players do not have so: what the next patch carries; absent when there is none) and
files.tsv (`<status> <path> <why> <live in>`, a line a file). Prints each file and why, and the
checks; exits 1 on any problem (nothing in patch/ then). A live file of no base row that no row
makes is a problem, unless an earlier version of a pack row added it (<packs>/<id>-<n>, n below
the row's version, adds the path with the live bytes): printed as left from it, not packed.
"""
import argparse
import fnmatch
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

KINDS = ('base', 'movie', 'pack', 'edit')
PATCH = re.compile(r'^resources\.db\.patch_([1-9][0-9]{0,2})$')
SHA = re.compile(r'^[0-9a-f]{64}$')
LOBBY = 'lobby_menu'

# The value-type check's allowlist (content_type_check.py): a NEW key whose
# type is not one 0.26g uses at that structural path, with the reason it is
# right: {(game path, structural path): reason}. It never lets a key that
# exists change its type (a number between int and float only where the base
# configs have both there: content_type_check.number_either). Empty: today's content needs none.
TYPE_ALLOWLIST = {}

SIMULATION_CHANGES = 'srp-026g/crates/session/src/v026g/simulation_changes.tsv'


class Problem(Exception):
    pass


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def run(command, what):
    """Runs a command; its output is printed indented; a failure is a Problem."""
    result = subprocess.run([str(c) for c in command], stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True)
    if result.returncode:
        raise Problem(f'{what} failed:\n' + '\n'.join('    ' + line for line in result.stdout.splitlines()[-25:]))
    return result.stdout


def read_list(path):
    """The offered-patch list: [(name, sha256)] in its order (release/lib.sh checked it)."""
    rows = []
    for line in Path(path).read_text().splitlines():
        if line and not line.startswith('#'):
            name, sha = line.split('\t')[:2]
            rows.append((name, sha))
    return rows


def read_manifest(path, offered, tree):
    """The rows, checked: [(kind, name, source, what, line)]."""
    rows, seen, rank = [], set(), 0
    for number, line in enumerate(Path(path).read_text(encoding='utf-8').splitlines(), 1):
        at = f'{path}:{number}'
        if not line.strip() or line.startswith('#'):
            continue
        fields = line.split('\t')
        if len(fields) != 4 or not all(fields[:3]):
            raise Problem(f'{at}: not <kind> <name> <source> <what> with tabs between')
        kind, name, source, what = fields
        if kind not in KINDS:
            raise Problem(f'{at}: {kind} is not a kind ({", ".join(KINDS)})')
        if KINDS.index(kind) < rank:
            raise Problem(f'{at}: a {kind} row after the {KINDS[rank]} rows (the kinds go {", ".join(KINDS)})')
        rank = KINDS.index(kind)
        if (kind, name) in seen:
            raise Problem(f'{at}: {kind} {name} is there twice')
        seen.add((kind, name))
        if kind == 'base':
            if not PATCH.match(name):
                raise Problem(f'{at}: a base row is a published resources.db.patch_<n>, not {name}')
            if (name, source) not in offered:
                raise Problem(f'{at}: {name} with sha256 {source} is not on the offered-patch list: '
                              'a base row is a published patch players have')
        elif kind == 'movie' and source != '-':
            raise Problem(f'{at}: movie {name}: the source is `-` (the tree)')
        elif kind == 'edit' and not (source == '-' or PATCH.match(source)):
            raise Problem(f'{at}: edit {name}: the source is `-` (applied) or the base row that holds it')
        elif kind == 'pack' and not (SHA.match(source) or PATCH.match(source)):
            raise Problem(f'{at}: pack {name}: the source is the sha256 of its pack.json (composed) or the '
                          'base row that holds it')
        if kind in ('pack', 'edit') and PATCH.match(source) and ('base', source) not in seen:
            raise Problem(f'{at}: {kind} {name} is in {source}, which is no base row above')
        if kind == 'pack' and not re.match(r'^[a-z0-9._-]+@[A-Za-z0-9._-]+$', name):
            raise Problem(f'{at}: a pack is named <id>@<version>, not {name}')
        if kind == 'movie' and not (tree / 'srp-026g' / 'flash' / name / 'movie.json').is_file():
            raise Problem(f'{at}: movie {name}: no srp-026g/flash/{name}/movie.json in {tree}')
        if kind == 'edit' and not (edits_dir(tree) / name).is_file():
            raise Problem(f'{at}: edit {name}: no srp-026g/scripts/content-edit/{name} in {tree}')
        rows.append((kind, name, source, what, at))
    trees = sorted(p.parent.name for p in (tree / 'srp-026g' / 'flash').glob('*/movie.json'))
    lists = sorted(p.name for p in edits_dir(tree).glob('*.json')) + \
        sorted(p.name for p in edits_dir(tree).glob('*.tsv'))
    for kind, names in (('movie', trees), ('edit', lists)):
        for name in names:
            if (kind, name) not in seen:
                where = f'srp-026g/flash/{name}' if kind == 'movie' else f'srp-026g/scripts/content-edit/{name}'
                raise Problem(f'{where} has no {kind} row in {path}: everything the sources make is live, '
                              f'so add its row (or take it out of the tree)')
    return rows


def edits_dir(tree):
    return tree / 'srp-026g' / 'scripts' / 'content-edit'


class Game:
    """srp-game-data on archives."""

    def __init__(self, binary, work):
        self.binary, self.work, self.count = binary, work, 0

    def layers(self, archives):
        out = ['--db', archives[0]]
        for archive in archives[1:]:
            out += ['--patch', archive]
        return out

    def files(self, archives):
        """{path: archive name} of every file the archives give."""
        text = run([self.binary, 'files'] + self.layers(archives), 'srp-game-data files')
        return {path: name for path, _, _, name in (line.split('\t') for line in text.splitlines())}

    def get(self, archives, paths):
        """{path: bytes} of those files as the archives give them."""
        if not paths:
            return {}
        self.count += 1
        folder = self.work / f'get-{self.count}'
        listing = self.work / f'get-{self.count}.txt'
        listing.write_text(''.join(p + '\n' for p in paths))
        run([self.binary, 'get'] + self.layers(archives) + ['--out', folder, '--paths', listing], 'srp-game-data get')
        got = {path: (folder / path).read_bytes() for path in paths}
        shutil.rmtree(folder)
        listing.unlink()
        return got

    def compare(self, archives, folder):
        """{path: (status, archive or '')}: same, differs, new."""
        text = run([self.binary, 'compare'] + self.layers(archives) + ['--dir', folder], 'srp-game-data compare')
        out = {}
        for line in text.splitlines():
            status, path, *name = line.split(' ')
            out[path] = (status, name[0] if name else '')
        return out


class World:
    """The files the sources make: path -> bytes, and why (the rows that made them, in order)."""

    def __init__(self):
        self.bytes, self.why = {}, {}

    def put(self, path, data, reason):
        if self.bytes.get(path) == data:
            return
        self.bytes[path] = data
        self.why.setdefault(path, []).append(reason)


def build_movies(rows, tree, archive_of, game, tools, work, world):
    stand_in = os.environ.get('SKHID_TEST_BUILD_MOVIE')
    for kind, name, _, _, at in rows:
        if kind != 'movie':
            continue
        movie_dir = tree / 'srp-026g' / 'flash' / name
        spec = json.loads((movie_dir / 'movie.json').read_text())
        archive = spec.get('archive')
        if archive not in archive_of:
            raise Problem(f'{at}: movie {name}: its original is in {archive} (movie.json), which is neither the '
                          'stock resources.db nor a base row')
        original = game.get([archive_of[archive]], [spec['movie']])[spec['movie']]
        if sha256(original) != spec.get('movie_sha256'):
            raise Problem(f'{at}: movie {name}: {spec["movie"]} of {archive} is not the original movie.json names')
        folder = work / 'movies' / name
        folder.mkdir(parents=True)
        (folder / 'original.swf').write_bytes(original)
        out = folder / 'built.swf'
        if stand_in:
            command = [stand_in, name, movie_dir, folder / 'original.swf', out]
        elif name == LOBBY:
            command = [sys.executable, '-I', tree / 'srp-026g' / 'client-patches' / 'build_lobby_026g.py',
                       folder / 'original.swf', out, '--tools', tools]
        else:
            command = [sys.executable, tree / 'tools' / 'actionscript' / 'flash_tree.py', '--tools', tools,
                       'build', movie_dir, folder / 'original.swf', out]
        said = run(command, f'the build of movie {name}')
        for line in said.strip().splitlines()[-2:]:
            print(f'    {line}')
        data = out.read_bytes()
        world.put(spec['movie'], data, f'movie {name} (srp-026g/flash/{name})')
        print(f'  movie {name}: {spec["movie"]}, {len(data)} bytes, sha256 {sha256(data)[:16]}…')


def compose_packs(rows, base, vostok, packs_dir, work, world):
    applied = [(name, source, at) for kind, name, source, _, at in rows if kind == 'pack' and SHA.match(source)]
    if not applied:
        return
    if not vostok:
        raise Problem('the manifest has packs to compose: SKHID_VOSTOK_PORT must name resource-porter\'s vostok-port')
    if not packs_dir:
        raise Problem('the manifest has packs to compose: SKHID_PACKS_DIR must name the folder of the packs')
    command = [vostok, 'pack', 'patch', '--base-db', base[0]]
    for patch in base[1:]:
        command += ['--base-patch', patch]
    for name, source, at in applied:
        ident, version = name.split('@')
        folder = Path(packs_dir) / f'{ident}-{version}'
        manifest = folder / 'pack.json'
        if not manifest.is_file():
            raise Problem(f'{at}: pack {name}: no {manifest}')
        if sha256(manifest.read_bytes()) != source:
            raise Problem(f'{at}: pack {name}: {manifest} has sha256 {sha256(manifest.read_bytes())}, not {source}')
        spec = json.loads(manifest.read_text())
        if (spec.get('id'), str(spec.get('version'))) != (ident, version):
            raise Problem(f'{at}: pack {name}: {manifest} says {spec.get("id")}@{spec.get("version")}')
        command += ['--pack', folder]
    movies = work / 'movies-for-packs'
    for path, data in world.bytes.items():
        target = movies / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        command += ['--replace', f'{path}={target}']
    out = work / 'packs'
    said = run(command + ['--out-dir', out], 'vostok-port pack patch')
    for line in said.strip().splitlines()[-3:]:
        print(f'    {line}')
    reason = 'pack ' + ', '.join(name for name, _, _ in applied)
    for file in sorted(p for p in out.rglob('*') if p.is_file()):
        world.put(file.relative_to(out).as_posix(), file.read_bytes(), reason)


def left_by_earlier_packs(rows, packs_dir, paths, base_files, live, game):
    """{path: '<id>@<n>'} of the live files `paths` (no base row's, made by no row) that an earlier
    version of a composed pack left: its folder <packs>/<id>-<n>, n below the row's version, adds the
    path (add_file, not over a base file) and its files/<path> is the live file byte for byte, so it is
    what that version shipped. Nothing refers to it: a pack's new version puts what it changes at new
    paths (a patch only adds files), so it stays as it is, not packed."""
    paths = [p for p in paths if p not in base_files]
    if not paths or not packs_dir:
        return {}
    added = {}                                    # path: [('<id>@<n>', its file)], highest n first
    for kind, name, source, _, at in rows:
        ident, _, version = name.partition('@')
        if kind != 'pack' or not SHA.match(source) or not version.isdigit():
            continue
        numbers = (f.name[len(ident) + 1:] for f in Path(packs_dir).glob(f'{ident}-*'))
        for n in sorted((int(n) for n in numbers if n.isdigit() and int(n) < int(version)), reverse=True):
            manifest = Path(packs_dir) / f'{ident}-{n}' / 'pack.json'
            if not manifest.is_file():
                raise Problem(f'{at}: pack {name}: {manifest.parent}, an earlier version, has no pack.json')
            spec = json.loads(manifest.read_text())
            if (spec.get('id'), str(spec.get('version'))) != (ident, str(n)):
                raise Problem(f'{at}: pack {name}: {manifest} says {spec.get("id")}@{spec.get("version")}')
            for op in spec.get('operations', []):
                if op.get('op') == 'add_file':
                    added.setdefault(op['path'], []).append((f'{ident}@{n}', manifest.parent / 'files' / op['path']))
    wanted = [p for p in paths if p in added]
    got = game.get(live, wanted)
    left = {}
    for path in wanted:
        for version, file in added[path]:
            if file.is_file() and file.read_bytes() == got[path]:
                left[path] = version
                break
    return left


def edit_paths(edits, known):
    """The game paths an edit list reads: its `file`s, its `files` lists and what its `files` globs match."""
    paths = set()
    if 'localization' in edits:
        # The catalog compiler reads every existing dictionary, the UI mapping,
        # and support files copied into newly registered language directories.
        paths.update(p for p in known if p.startswith('localization/'))
    if 'promote_rolled_defaults' in edits:
        # Computed from the data (content_edit.py promoted_defaults): the modifications, the item
        # dictionary and every equipment config; an item of the categories elsewhere stops the edit.
        paths.update({'gameplay/items/modifications/modifications.options', 'gameplay/db_static_dictionaries'})
        paths.update(p for p in known if p.startswith('gameplay/items/armour/'))
    for entries in edits.values():
        if not isinstance(entries, list):
            continue
        for entry in entries:
            if 'file' in entry:
                paths.add(entry['file'])
            if isinstance(entry.get('files'), list):      # add_value: the game paths themselves
                paths.update(entry['files'])
            elif 'files' in entry:
                paths.update(fnmatch.filter(known, entry['files']))
    return sorted(paths)


def apply_edits(rows, tree, base, base_files, game, work, world):
    tool = edits_dir(tree) / 'content_edit.py'
    for kind, name, source, _, at in rows:
        if kind != 'edit' or source != '-':
            continue
        known = sorted(set(base_files) | set(world.bytes))
        if name.endswith('.tsv'):
            paths = own_content_edit().stats_paths(own_content_edit().stats_category(name), known)
        else:
            paths = edit_paths(json.loads((edits_dir(tree) / name).read_text(encoding='utf-8')), known)
        movies = [p for p in paths if p.startswith('flash_movies/')]
        if movies:
            raise Problem(f'{at}: edit {name} names {movies[0]}: a movie changes in its source tree')
        missing = [p for p in paths if p not in world.bytes and p not in base_files]
        if missing:
            raise Problem(f'{at}: edit {name}: {missing[0]} is in no archive of the base and made by no row above')
        live = work / 'edits' / name / 'live'
        out = work / 'edits' / name / 'out'
        got = game.get(base, [p for p in paths if p not in world.bytes])
        for path in paths:
            target = live / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(world.bytes[path] if path in world.bytes else got[path])
        said = run([sys.executable, '-I', tool, edits_dir(tree) / name, live, out], f'edit {name} (content_edit.py)')
        print(f'  edit {name}: {said.strip().splitlines()[-2] if len(said.strip().splitlines()) > 1 else said.strip()}')
        for file in sorted(p for p in out.rglob('*') if p.is_file()):
            world.put(file.relative_to(out).as_posix(), file.read_bytes(), f'edit {name}')


def type_check(base, base_files, game, world):
    """Every file the manifest makes keeps the value type of each key path the
    base/live version has, and a new key uses a 0.26g type for its path or is
    allowlisted (content_type_check.py). A Problem lists every offender with
    the file, the path, the old type and the new type."""
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        'content_type_check', str(Path(__file__).resolve().parent / 'content_type_check.py'))
    ctc = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(ctc)
    # The config reader is infrastructure of this checkout, not of --tree
    # (which supplies the content): use build-content's own content_edit.py.
    own = Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' / 'content-edit'
    walk = ctc.load_walk(str(own))
    # The corpus of the types 0.26g already uses at each key name: every base
    # config file (the stock db and the base patches), so a new key's type is
    # weighed against all of 0.26g, not only the files this patch touches.
    corpus_paths = sorted(p for p in base_files if p.endswith(ctc.CONFIG_SUFFIXES))
    corpus = game.get(base, corpus_paths)
    corp = ctc.corpus_with(walk, corpus)
    # The previous version of each changed file: the base's bytes, where the
    # base has it; None for a file no base archive has (a pack's new file).
    had = [p for p in sorted(world.bytes) if p in base_files]
    previous = game.get(base, had)
    problems = []
    # A file at a config path the reader refuses is a problem, never a file
    # the check skips (a table out of CRC order, a broken record): the made
    # files, their previous versions and every base config alike.
    for label, files in (('made', world.bytes), ('previous', previous), ('base', corpus)):
        for path in sorted(p for p in files if p.endswith(ctc.CONFIG_SUFFIXES)):
            try:
                ctc.parse(walk, files[path])
            except ctc.NotAConfig as why:
                problems.append(f'{path} ({label}): a config the reader refuses ({why})')
    for path in sorted(world.bytes):
        problems += ctc.violations(walk, previous.get(path), world.bytes[path], path, corp, TYPE_ALLOWLIST)
    if problems:
        print()
        for problem in problems:
            print(f'TYPE: {problem}')
        raise Problem(f'{len(problems)} value-type problem(s) the manifest does not account for: no patch made '
                      '(a key keeps its type; a new key uses a 0.26g type or an allowlist row in build_content.py; '
                      'a config parses)')
    print(f'value types: every key the base has keeps its type across {len(world.bytes)} file(s)')


def own_content_edit():
    """This checkout's content_edit.py (not --tree's)."""
    import importlib.util
    path = Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' / 'content-edit' / 'content_edit.py'
    spec = importlib.util.spec_from_file_location('content_edit', str(path))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stats_checks(rows, base, base_files, live, game, world, tree, compared):
    """Per content_edit.STATS category: each config (or list entry) is its table row, its cards follow
    it, and a changed match column needs the `next` row of SIMULATION_CHANGES."""
    ce = own_content_edit()
    tables = {name: edits_dir(tree) / name for kind, name, source, _, _ in rows
              if kind == 'edit' and source == '-' and name.endswith('.tsv')}
    problems, moved = [], []
    for category_name, category in ce.STATS.items():
        paths = ce.stats_paths(category, set(base_files) | set(world.bytes))
        configs = {p: bytearray(b) for p, b in game.get(base, [p for p in paths if p not in world.bytes]).items()}
        configs.update({p: bytearray(world.bytes[p]) for p in paths if p in world.bytes})
        targets = ce.stats_targets(category, configs)
        table = tables.get(f'{category_name}.tsv')
        _, columns, table_rows = ce.read_stats(str(table)) if table else (None, category.get('columns', {}), None)
        if table:
            problems += ce.stats_problems(category, columns, table_rows, targets, configs, table.name)
        if category.get('cards'):
            problems += [f'{p}: card line {prop_id} says {text!r}, its config {want!r}'
                         for p in paths for prop_id, text, want in ce.card_differences(configs[p], category['cards'])]
        # A config players lack (a new key/value table's) has no values: each of its match values moved.
        changed = [p for p in paths if p in world.bytes and compared[p][0] in ('differs', 'new')]
        players = game.get(live, [p for p in changed if compared[p][0] == 'differs'])
        try:
            moved += ce.stats_moved(category, columns, {p: bytearray(b) for p, b in players.items()},
                                    {p: configs[p] for p in changed})
        except ValueError as why:
            problems.append(f'{category_name}: {why}')
        print(f'{category_name}: {len(targets)} row(s)' + (f', each its row of {table.name}' if table else '')
              + (', every card line its number' if category.get('cards') else ''))
    if problems:
        print()
        for problem in problems:
            print(f'STATS: {problem}')
        raise Problem(f'{len(problems)} stats problem(s): no patch made')
    if not moved:
        return
    changes = tree / SIMULATION_CHANGES
    marked = changes.is_file() and any(line.startswith('next\t') for line in changes.read_text().splitlines())
    for line in moved:
        print(f'  match value: {line}')
    if not marked:
        raise Problem(f'{len(moved)} value(s) a match computes with differ from what players have, and '
                      f'{SIMULATION_CHANGES} has no `next` row')
    print(f'{len(moved)} value(s) a match computes with differ from players\': {SIMULATION_CHANGES} has '
          'its `next` row')


def lamp_check(world):
    """Every made client_project's breakable-lamp bindings name its own
    objects (scripts/breakable-lamps/make_edits.py check_built): a wrong
    index or a moved light, after any later edit, is a Problem."""
    import importlib.util
    path = Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' / 'breakable-lamps' / 'make_edits.py'
    spec = importlib.util.spec_from_file_location('lamp_edits', str(path))
    lamps = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(lamps)
    projects = sorted(p for p in world.bytes if p.endswith('/client_project'))
    problems = [problem for p in projects for problem in lamps.check_built(p, bytearray(world.bytes[p]))]
    if problems:
        print()
        for problem in problems:
            print(f'LAMPS: {problem}')
        raise Problem(f'{len(problems)} lamp binding(s) name no object of their map: no patch made')
    print(f'lamps: every binding of {len(projects)} made project(s) names its own model or light')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for option in ('stock', 'patches', 'list', 'bin', 'tree', 'manifest', 'work'):
        parser.add_argument('--' + option, required=True, type=Path)
    parser.add_argument('--tools', type=Path)
    parser.add_argument('--vostok-port', type=Path)
    parser.add_argument('--packs', type=Path)
    args = parser.parse_args()
    work = args.work
    work.mkdir(parents=True)
    game = Game(args.bin, work)
    offered = read_list(args.list)
    try:
        rows = read_manifest(args.manifest, offered, args.tree)
        base_rows = [name for kind, name, _, _, _ in rows if kind == 'base']
        base_names = ['resources.db'] + sorted(base_rows, key=lambda n: int(PATCH.match(n).group(1)))
        archive_of = {'resources.db': args.stock} | {n: args.patches / n for n in base_rows}
        base = [archive_of[n] for n in base_names]
        live = [args.stock] + [args.patches / name for name, _ in offered]
        print(f'the base: the stock resources.db and {len(base_rows)} published patch(es): '
              + ' '.join(n.removeprefix('resources.db.') for n in base_names[1:]))
        if any(kind == 'movie' for kind, *_ in rows) and not os.environ.get('SKHID_TEST_BUILD_MOVIE') and \
                not (args.tools and args.tools.is_file()):
            raise Problem(f'the movies need a tools.json naming java and Flex (tools/actionscript/README.md): '
                          f'{args.tools} is not a file (SKHID_FLASH_TOOLS names another)')
        world = World()
        print('the movies, from source:')
        build_movies(rows, args.tree, archive_of, game, args.tools, work, world)
        compose_packs(rows, base, args.vostok_port, args.packs, work, world)
        base_files = game.files(base)
        print('the edit lists, in order:')
        apply_edits(rows, args.tree, base, base_files, game, work, world)

        # The value types every changed file keeps (content_type_check.py):
        # a key that exists in the base/live version keeps its type, a new key
        # uses a type 0.26g files already use at that path or is allowlisted.
        type_check(base, base_files, game, world)
        lamp_check(world)

        # What the sources make, against what players have.
        folder = work / 'world'
        for path, data in world.bytes.items():
            target = folder / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        compared = game.compare(live, folder) if world.bytes else {}
        stats_checks(rows, base, base_files, live, game, world, args.tree, compared)
        problems = []
        live_files = game.files(live)
        unmade = sorted(p for p, name in live_files.items() if name not in base_names and p not in world.bytes)
        left = left_by_earlier_packs(rows, args.packs, unmade, base_files, live, game)
        for path in unmade:
            if path not in left:
                problems.append(f'{path}: live from {live_files[path]}, which is no base row, and made by no row '
                                'of the manifest nor left by an earlier version of a pack row (<packs>/<id>-<n>): '
                                'what made it? (a patch put together by hand, a row taken out, or an earlier '
                                'pack\'s folder missing)')
        changed = sorted(p for p, (status, _) in compared.items() if status != 'same')
        lines = []
        print()
        print(f'every file the movies, packs and edits make ({len(world.bytes)}), against what players have '
              f'(the stock resources.db and the {len(offered)} offered patches):')
        for path in sorted(world.bytes):
            status, name = compared[path]
            why = ', then '.join(world.why[path])
            word = {'same': 'unchanged', 'differs': 'CHANGED', 'new': 'NEW'}[status]
            where = f'live in {name}' if name else 'not in players\' files'
            print(f'  {word:9} {path}: {why}; {where}')
            lines.append(f'{status}\t{path}\t{why}\t{name or "-"}\n')
        (work / 'files.tsv').write_text(''.join(lines))
        if left:
            print()
            print(f'left from earlier versions of the packs ({len(left)}): live, no longer made, nothing refers '
                  'to them; not packed:')
            for path in sorted(left):
                print(f'  left      {path}: pack {left[path]}; live in {live_files[path]}')
        if problems:
            print()
            for problem in problems:
                print(f'PROBLEM: {problem}')
            raise Problem(f'{len(problems)} live file(s) the manifest does not account for: no patch made')
        print()
        if not changed:
            print('nothing to ship: players have every file as the manifest makes it')
            return
        patch = work / 'patch'
        for path in changed:
            target = patch / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(world.bytes[path])
        print(f'the next patch: {len(changed)} file(s), {sum(len(world.bytes[p]) for p in changed)} bytes')
    except Problem as problem:
        sys.exit(f'build-content: {problem}')


if __name__ == '__main__':
    main()
