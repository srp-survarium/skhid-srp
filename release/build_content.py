#!/usr/bin/env python3
"""The files of players' resources.db as release/content.tsv says they should be, against what
players have: release/build-content's work (run it, not this; --help there).

    python3 -I build_content.py --stock <resources.db> --patches <folder> --list <patches.tsv>
        --bin <srp-game-data> --tree <checkout> --manifest <content.tsv> --work <new folder>
        --stock-sha256 <its sha256> [--bin-source <the srp-026g it was built from>]
        [--tools <tools.json>] [--vostok-port <program>] [--packs <folder>] [--cache <folder>]

With --cache, a step whose inputs are those of an earlier build (a movie, the packs, an edit list,
the checks of a file or a stats table) takes what it made or found then from there (Cache).

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
import bisect
import fnmatch
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

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


class timed:
    """Prints how long the step took (`time: <what> <s> s`), for the log."""

    def __init__(self, what):
        self.what = what

    def __enter__(self):
        self.start = time.monotonic()

    def __exit__(self, *_):
        print(f'time: {self.what} {time.monotonic() - self.start:.1f} s')


def write_new(path, data):
    """Writes a file whole or not at all (a reader sees the old one, or none, until it is there)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    part = path.with_name(f'.{path.name}.{os.getpid()}')
    part.write_bytes(data)
    os.replace(part, path)


def tree_digest(folder):
    """The sha256 of every file under a folder: its path there and its bytes."""
    digest = hashlib.sha256()
    for file in sorted(p for p in Path(folder).rglob('*') if p.is_file()):
        digest.update(f'{file.relative_to(folder).as_posix()}\0{sha256(file.read_bytes())}\0'.encode())
    return digest.hexdigest()


def files_digest(files):
    """The sha256 of the given files' names and bytes, in that order (a missing one counts as
    missing); not their folders, so another checkout's same files have the same digest."""
    return sha256(json.dumps([(f.name, sha256(f.read_bytes()) if f.is_file() else None) for f in files]).encode())


def game_data_id(binary, source):
    """srp-game-data as the cache knows it: with `source` (the srp-026g it was built from), its
    sources: each checkout's build differs by the paths in it, the same sources read the same;
    else the program's bytes."""
    if not source:
        return files_digest([binary])
    crates = [source / 'crates' / c for c in ('resources-db', 'binary-config-parser')]
    return sha256(json.dumps([files_digest([source / f for f in ('Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock')])]
                             + [tree_digest(c / 'src') + files_digest([c / 'Cargo.toml']) for c in crates]).encode())


class Cache:
    """What a step (a movie, the packs, an edit list) made in an earlier build, by the key of
    everything it reads: <root>/steps/<key>.json names the files by their sha256 and holds what the
    step said, <root>/blobs/<sha256> holds each file (checked when read). Anything under <root> may
    be deleted at any time: a step whose entry or a file is missing runs again. Without a root
    (build-content --no-cache), every step runs."""

    def __init__(self, root):
        self.root = root

    @staticmethod
    def key(*parts):
        return sha256(json.dumps(parts, sort_keys=True, default=str).encode())

    def get(self, key):
        """({path: bytes}, said) of the step of that key, or None."""
        if not self.root:
            return None
        try:
            entry = json.loads((self.root / 'steps' / f'{key}.json').read_text())
            files = {}
            for path, digest in entry['files'].items():
                data = (self.root / 'blobs' / digest[:2] / digest).read_bytes()
                if sha256(data) != digest:
                    return None
                files[path] = data
            return files, entry['said']
        except (OSError, ValueError, KeyError):
            return None

    def put(self, key, files, said):
        if not self.root:
            return
        names = {}
        for path, data in files.items():
            digest = sha256(data)
            blob = self.root / 'blobs' / digest[:2] / digest
            if not (blob.is_file() and blob.stat().st_size == len(data)):
                write_new(blob, data)
            names[path] = digest
        write_new(self.root / 'steps' / f'{key}.json', json.dumps({'files': names, 'said': said}).encode())


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
    """The files the sources make: path -> bytes, why (the rows that made them, in order), and each
    one's sha256 (hashed once)."""

    def __init__(self):
        self.bytes, self.why, self.digests = {}, {}, {}

    def put(self, path, data, reason):
        if self.bytes.get(path) == data:
            return
        self.bytes[path] = data
        self.digests.pop(path, None)
        self.why.setdefault(path, []).append(reason)

    def sha256(self, path):
        if path not in self.digests:
            self.digests[path] = sha256(self.bytes[path])
        return self.digests[path]


def build_movies(rows, tree, archive_of, sha_of, game, tools, work, world, cache):
    stand_in = os.environ.get('SKHID_TEST_BUILD_MOVIE')
    # What a movie's build reads besides its tree and its original: the build's programs and the tools.
    programs = files_digest(sorted((tree / 'tools' / 'actionscript').glob('*.py'))
                            + [tree / 'srp-026g' / 'client-patches' / 'build_lobby_026g.py']
                            + [Path(p) for p in (stand_in, tools) if p])
    for kind, name, _, _, at in rows:
        if kind != 'movie':
            continue
        movie_dir = tree / 'srp-026g' / 'flash' / name
        spec = json.loads((movie_dir / 'movie.json').read_text())
        archive = spec.get('archive')
        if archive not in archive_of:
            raise Problem(f'{at}: movie {name}: its original is in {archive} (movie.json), which is neither the '
                          'stock resources.db nor a base row')
        key = Cache.key('movie', name, sha_of[archive], tree_digest(movie_dir), programs, stand_in, sys.version)
        made = cache.get(key)
        if made:
            data = made[0][spec['movie']]
            world.put(spec['movie'], data, f'movie {name} (srp-026g/flash/{name})')
            print(f'  movie {name}: {spec["movie"]}, {len(data)} bytes, sha256 {sha256(data)[:16]}… '
                  '(as an earlier build of the same sources made it: the cache)')
            continue
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
        with timed(f'movie {name}'):
            said = run(command, f'the build of movie {name}')
        for line in said.strip().splitlines()[-2:]:
            print(f'    {line}')
        data = out.read_bytes()
        cache.put(key, {spec['movie']: data}, said)
        world.put(spec['movie'], data, f'movie {name} (srp-026g/flash/{name})')
        print(f'  movie {name}: {spec["movie"]}, {len(data)} bytes, sha256 {sha256(data)[:16]}…')


def compose_packs(rows, base, base_id, vostok, packs_dir, work, world, cache):
    applied = [(name, source, at) for kind, name, source, _, at in rows if kind == 'pack' and SHA.match(source)]
    if not applied:
        return
    if not vostok:
        raise Problem('the manifest has packs to compose: SKHID_VOSTOK_PORT must name resource-porter\'s vostok-port')
    if not packs_dir:
        raise Problem('the manifest has packs to compose: SKHID_PACKS_DIR must name the folder of the packs')
    reason = 'pack ' + ', '.join(name for name, _, _ in applied)
    command = [vostok, 'pack', 'patch', '--base-db', base[0]]
    for patch in base[1:]:
        command += ['--base-patch', patch]
    folders = []
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
        folders.append(folder)
    # What vostok-port reads: the base, each pack's folder (its files by size and time: a folder is
    # one version, made once), the movies it composes over, and itself (the program and the files
    # beside it named after it: a wrapper and its binary).
    program = Path(vostok).resolve()
    key = Cache.key('packs', base_id, files_digest(sorted(program.parent.glob(program.name + '*'))),
                    [(folder.name, [(p.relative_to(folder).as_posix(), p.stat().st_size, p.stat().st_mtime_ns)
                                    for p in sorted(folder.rglob('*')) if p.is_file()]) for folder in folders],
                    sorted((path, world.sha256(path)) for path in world.bytes))
    made = cache.get(key)
    if made:
        print('    as an earlier build of the same packs over the same movies composed them: the cache')
        for path, data in made[0].items():           # in the order composed
            world.put(path, data, reason)
        return
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
    composed = {file.relative_to(out).as_posix(): file.read_bytes()
                for file in sorted(p for p in out.rglob('*') if p.is_file())}
    cache.put(key, composed, said)
    for path, data in composed.items():
        world.put(path, data, reason)


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


def matching(known, pattern):
    """fnmatch.filter(known, pattern) of a sorted list, trying only the paths that begin with the
    pattern's head before its first wildcard."""
    head = re.split(r'[*?[]', pattern, maxsplit=1)[0]
    return fnmatch.filter(known[bisect.bisect_left(known, head):bisect.bisect_left(known, head + '\U0010ffff')], pattern)


def edit_paths(edits, known):
    """The game paths an edit list reads: its `file`s, its `files` lists and what its `files` globs match."""
    known = sorted(known)
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
                paths.update(matching(known, entry['files']))
    # What the list's own `copy` makes anew it then edits (an add_value of a copy): no read. A copy
    # onto a live file is refused by content_edit.py; it stays a read here.
    made = set()
    for e in edits.get('copy', []):
        for p in matching(known, e['files']):
            if not p.startswith(e['from']):
                raise Problem(f"copy: {p} does not start with {e['from']}")
            made.add(e['to'] + p[len(e['from']):])
    return sorted(paths - (made - set(known)))


def apply_edits(rows, tree, base, base_id, base_files, game, work, world, cache):
    tool = edits_dir(tree) / 'content_edit.py'
    # What an edit list's run reads besides the list and its files: content_edit.py and its modules
    # (the localization's of client-patches too), and for a `localization` list its folder.
    programs = files_digest(sorted(edits_dir(tree).glob('*.py'))
                            + sorted((tree / 'srp-026g' / 'client-patches').glob('*localization*.py')))
    for kind, name, source, _, at in rows:
        if kind != 'edit' or source != '-':
            continue
        known = sorted(set(base_files) | set(world.bytes))
        edits = {} if name.endswith('.tsv') else json.loads((edits_dir(tree) / name).read_text(encoding='utf-8'))
        if name.endswith('.tsv'):
            paths = own_content_edit().stats_paths(own_content_edit().stats_category(name), known)
        else:
            paths = edit_paths(edits, known)
        movies = [p for p in paths if p.startswith('flash_movies/')]
        if movies:
            raise Problem(f'{at}: edit {name} names {movies[0]}: a movie changes in its source tree')
        missing = [p for p in paths if p not in world.bytes and p not in base_files]
        if missing:
            raise Problem(f'{at}: edit {name}: {missing[0]} is in no archive of the base and made by no row above')
        # Its files: a made one by its bytes, a base one by the base (base_id).
        folder = edits.get('localization')
        key = Cache.key('edit', name, sha256((edits_dir(tree) / name).read_bytes()), programs, sys.version, base_id,
                        [(p, world.sha256(p) if p in world.bytes else None) for p in paths],
                        tree_digest(edits_dir(tree) / folder) if isinstance(folder, str) else None)
        made = cache.get(key)
        if made:
            files, said = made
            print(f'  edit {name}: {said} (as an earlier build of the list over the same files made it: the cache)')
            for path, data in files.items():
                world.put(path, data, f'edit {name}')
            continue
        live = work / 'edits' / name / 'live'
        out = work / 'edits' / name / 'out'
        got = game.get(base, [p for p in paths if p not in world.bytes])
        for path in paths:
            target = live / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(world.bytes[path] if path in world.bytes else got[path])
        with timed(f'edit {name}'):
            said = run([sys.executable, '-I', tool, edits_dir(tree) / name, live, out], f'edit {name} (content_edit.py)')
        said = said.strip().splitlines()[-2] if len(said.strip().splitlines()) > 1 else said.strip()
        print(f'  edit {name}: {said}')
        files = {file.relative_to(out).as_posix(): file.read_bytes()
                 for file in sorted(p for p in out.rglob('*') if p.is_file())}
        cache.put(key, files, said)
        for path, data in files.items():
            world.put(path, data, f'edit {name}')


def type_check(rows, tree, base, base_id, base_files, game, world, cache):
    """Every file the manifest makes keeps the value type of each key path the
    base/live version has, and a new key uses a 0.26g type for its path or is
    allowlisted (content_type_check.py); an entry of a list the manifest's edit
    lists key (append_entry's unique_key) is weighed against the entry with
    its key, wherever an edit moved it. A Problem lists every offender with
    the file, the path, the old type and the new type. What the base gives
    (the corpus, its configs' problems) and each made file's problems are
    cached: by the base, and by the file's path, bytes and list keys."""
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        'content_type_check', str(Path(__file__).resolve().parent / 'content_type_check.py'))
    ctc = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(ctc)
    # The config reader is infrastructure of this checkout, not of --tree
    # (which supplies the content): use build-content's own content_edit.py.
    own = Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' / 'content-edit'
    walk = ctc.load_walk(str(own))
    sources = files_digest([Path(__file__).resolve(), Path(spec.origin), own / 'content_edit.py'])
    try:
        keys = ctc.list_keys(json.loads((edits_dir(tree) / name).read_text(encoding='utf-8'))
                             for kind, name, *_ in rows if kind == 'edit' and name.endswith('.json'))
    except ValueError as why:
        raise Problem(f'the edit lists: {why}') from None

    def refused(path, data, label):
        try:
            ctc.parse(walk, data)
        except ctc.NotAConfig as why:
            return f'{path} ({label}): a config the reader refuses ({why})'

    # The corpus of the types 0.26g already uses at each key name: every base
    # config file (the stock db and the base patches), so a new key's type is
    # weighed against all of 0.26g, not only the files this patch touches.
    base_key = Cache.key('the types of the base', base_id, sources, sys.version)
    known = cache.get(base_key)
    if known:
        corp = {path: set(types) for path, types in known[1]['corpus'].items()}
        base_problems = known[1]['problems']
    else:
        corpus_paths = sorted(p for p in base_files if p.endswith(ctc.CONFIG_SUFFIXES))
        corpus = game.get(base, corpus_paths)
        corp = ctc.corpus_with(walk, corpus)
        base_problems = [p for p in (refused(path, corpus[path], 'base') for path in corpus_paths) if p]
        cache.put(base_key, {}, {'corpus': {path: sorted(types) for path, types in corp.items()},
                                 'problems': base_problems})
    # Each made file's problems: its own parse (a config path's), its previous version's (the base's
    # bytes, where the base has it; None for a file no base archive has: a pack's new file), its
    # value types against them.
    files_key = Cache.key('the types of made files', base_id, sources, sys.version)
    earlier = (cache.get(files_key) or ({}, {}))[1]
    key_of = {path: Cache.key(path, world.sha256(path), keys.get(path, {})) for path in world.bytes}
    todo = [p for p in sorted(world.bytes) if key_of[p] not in earlier]
    previous = game.get(base, [p for p in todo if p in base_files])
    found = {}
    for path in todo:
        config = path.endswith(ctc.CONFIG_SUFFIXES)
        found[key_of[path]] = {
            'made': refused(path, world.bytes[path], 'made') if config else None,
            'previous': refused(path, previous[path], 'previous') if config and path in previous else None,
            'types': ctc.violations(walk, previous.get(path), world.bytes[path], path, corp, TYPE_ALLOWLIST,
                                    keys.get(path, {}))}
    if found:
        cache.put(files_key, {}, {key_of[p]: found.get(key_of[p]) or earlier[key_of[p]] for p in world.bytes})
    each = [found.get(key_of[p]) or earlier[key_of[p]] for p in sorted(world.bytes)]
    # A file at a config path the reader refuses is a problem, never a file
    # the check skips (a table out of CRC order, a broken record): the made
    # files, their previous versions and every base config alike.
    problems = [e['made'] for e in each if e['made']] + [e['previous'] for e in each if e['previous']] \
        + base_problems + [p for e in each for p in e['types']]
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


def stats_checks(rows, base, base_id, base_files, live, live_id, game, world, tree, compared, cache):
    """Per content_edit.STATS category: each config (or list entry) is its table row, its cards follow
    it, and a changed match column needs the `next` row of SIMULATION_CHANGES. A category's findings
    are cached by its configs, its table and what players have of them."""
    ce = own_content_edit()
    sources = files_digest([Path(__file__).resolve(), Path(ce.__file__)])
    tables = {name: edits_dir(tree) / name for kind, name, source, _, _ in rows
              if kind == 'edit' and source == '-' and name.endswith('.tsv')}
    problems, moved = [], []
    for category_name, category in ce.STATS.items():
        paths = ce.stats_paths(category, set(base_files) | set(world.bytes))
        table = tables.get(f'{category_name}.tsv')
        changed = [p for p in paths if p in world.bytes and compared[p][0] in ('differs', 'new')]
        key = Cache.key('stats', category_name, sources, sys.version, base_id, live_id,
                        sha256(table.read_bytes()) if table else None,
                        [(p, world.sha256(p) if p in world.bytes else None) for p in paths],
                        [(p, compared[p][0]) for p in changed])
        found = cache.get(key)
        if found:
            found = found[1]
        else:
            found = {'problems': []}
            configs = {p: bytearray(b) for p, b in game.get(base, [p for p in paths if p not in world.bytes]).items()}
            configs.update({p: bytearray(world.bytes[p]) for p in paths if p in world.bytes})
            targets = ce.stats_targets(category, configs)
            _, columns, table_rows = ce.read_stats(str(table)) if table else (None, category.get('columns', {}), None)
            if table:
                found['problems'] += ce.stats_problems(category, columns, table_rows, targets, configs, table.name)
            if category.get('cards'):
                found['problems'] += [f'{p}: card line {prop_id} says {text!r}, its config {want!r}' for p in paths
                                      for prop_id, text, want in ce.card_differences(configs[p], category['cards'])]
            # A config players lack (a new key/value table's) has no values: each of its match values moved.
            players = game.get(live, [p for p in changed if compared[p][0] == 'differs'])
            try:
                found['moved'] = ce.stats_moved(category, columns, {p: bytearray(b) for p, b in players.items()},
                                                {p: configs[p] for p in changed})
            except ValueError as why:
                found['moved'] = []
                found['problems'].append(f'{category_name}: {why}')
            found['said'] = (f'{category_name}: {len(targets)} row(s)'
                             + (f', each its row of {table.name}' if table else '')
                             + (', every card line its number' if category.get('cards') else ''))
            cache.put(key, {}, found)
        problems += found['problems']
        moved += found['moved']
        print(found['said'])
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


KEEPER_EDIT = 'artefact-keeper-1.json'
KEEPER_POOL = b'srp/artefact_keeper/pool'


def keeper_check(rows, base_files, world):
    """With Keeper's edit list in the manifest, every map project (every client_project but the
    lobby scenes') has its hidden pool (the anomaly `srp/artefact_keeper/pool`): the list names
    its maps one by one, so a map added later without it would give Keepers nothing there."""
    if ('edit', KEEPER_EDIT) not in {(kind, name) for kind, name, *_ in rows}:
        return
    projects = sorted(p for p in set(base_files) | set(world.bytes)
                      if p.startswith('projects/') and p.endswith('/client_project')
                      and not p.startswith('projects/lobby_scene/'))
    missing = [p for p in projects if KEEPER_POOL not in world.bytes.get(p, b'')]
    if missing:
        print()
        for project in missing:
            print(f'KEEPER: {project}: no {KEEPER_POOL.decode()} (add the map to {KEEPER_EDIT}: '
                  'srp-026g/scripts/artefact-keeper/make_edit.py)')
        raise Problem(f'{len(missing)} map project(s) without the Keeper pool: no patch made')
    print(f'keeper: every one of {len(projects)} map project(s) has its pool')


def pierce_check(rows, tree, base, base_files, game, world):
    """Every pierce line (an add_entry with `pierce_line`, content_edit.pierce_lines) is what the final
    files make: a row after the list's that retunes a weapon's or a round's piercing, a material's
    armour or what fires what, or adds a gun or a round, leaves a line stale or missing (and a
    weapon the list's _inputs does not declare, a line computed without it), and is a Problem."""
    ce = own_content_edit()
    known = set(base_files) | set(world.bytes)
    # Every item config the dictionary can pair, read once.
    needed = sorted(p for p in known if p.startswith('gameplay/items/') and p.endswith('.options'))
    needed += [ce.GAME_MATERIALS, ce.DICTIONARY]
    files = game.get(base, [p for p in needed if p not in world.bytes])
    files.update({p: world.bytes[p] for p in needed if p in world.bytes})
    files = {p: bytearray(b) for p, b in files.items()}
    problems, checked = [], 0
    for kind, name, source, _, _ in rows:
        if kind != 'edit' or source != '-' or not name.endswith('.json'):
            continue
        for entry in json.loads((edits_dir(tree) / name).read_text(encoding='utf-8')).get('add_entry', []):
            if 'pierce_line' not in entry:
                continue
            try:
                want = ce.pierce_lines(files.__getitem__, set(files), entry['pierce_line'])
            except ValueError as why:
                raise Problem(f'{name}: {why}') from None
            prop_id = entry['values']['prop_id']
            paths = set(fnmatch.filter(files, entry['files']))
            problems += [f'{path}: a pierce line, but not one of {entry["files"]} ({name})'
                         for path in sorted(set(want) - paths)]
            for path in sorted(paths):
                have = ce.prop_text(files[path], prop_id)
                if have != want.get(path):
                    problems.append(f'{path}: line {prop_id} says {have!r}, its data makes {want.get(path)!r} '
                                    f'({name}: a row after it changes what it is computed from)')
            checked += len(want)
    if problems:
        for problem in problems:
            print(f'PIERCE: {problem}')
        raise Problem(f'{len(problems)} pierce line(s) the final data does not make: no patch made')
    print(f'pierce lines: each of {checked} round(s) what its final data makes')


def clothing_check(base, base_files, game, world):
    """Reject low boot shafts selecting the variants for medium/tall boots.
    Measure the stock classes, then check the final composed meshes/configs,
    after all edits (including exo-boots-1). No item ids or fixed heights."""
    import importlib.util
    path = (Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' /
            'clothing-fit' / 'check_boot_classes.py')
    spec = importlib.util.spec_from_file_location('check_boot_classes', path)
    check = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = check
    spec.loader.exec_module(check)
    paths = sorted(p for p in set(base_files) | set(world.bytes) if check.is_boot(p))
    if not paths:
        return  # Small release-builder fixtures can contain no equipment.
    stock = [base[0]]
    native_paths = sorted(p for p in game.files(stock) if check.is_boot(p))
    native = game.get(stock, native_paths)
    native.update(game.get(stock, check.model_paths(native)))
    effective = game.get(base, [p for p in paths if p not in world.bytes])
    effective.update({p: world.bytes[p] for p in paths if p in world.bytes})
    models = check.model_paths(effective)
    effective.update(game.get(base, [p for p in models if p not in world.bytes]))
    effective.update({p: world.bytes[p] for p in models if p in world.bytes})
    try:
        rows = check.audit(check.Resources(native), check.Resources(effective))
    except (ValueError, KeyError) as error:
        raise Problem(f'clothing fit: {error}') from error
    failures = [r for r in rows if r['status'] != 'ok']
    for row in failures:
        print(f"CLOTHING: {row['path']}: {row['class']} shaft ends at "
              f"{min(row['shaft_top_m']):.3f} m; native class begins at "
              f"{row['native_class_min_m']:.3f} m")
    if failures:
        raise Problem(f'{len(failures)} boot shaft/class mismatch(es): no patch made')
    print(f'clothing fit: {len(rows)} boot shafts cover their native trousers class')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for option in ('stock', 'patches', 'list', 'bin', 'tree', 'manifest', 'work'):
        parser.add_argument('--' + option, required=True, type=Path)
    parser.add_argument('--tools', type=Path)
    parser.add_argument('--vostok-port', type=Path)
    parser.add_argument('--packs', type=Path)
    parser.add_argument('--stock-sha256', required=True)
    parser.add_argument('--bin-source', type=Path)
    parser.add_argument('--cache', type=Path)
    args = parser.parse_args()
    work = args.work
    work.mkdir(parents=True)
    game = Game(args.bin, work)
    cache = Cache(args.cache)
    offered = read_list(args.list)
    try:
        rows = read_manifest(args.manifest, offered, args.tree)
        base_rows = [name for kind, name, _, _, _ in rows if kind == 'base']
        base_names = ['resources.db'] + sorted(base_rows, key=lambda n: int(PATCH.match(n).group(1)))
        archive_of = {'resources.db': args.stock} | {n: args.patches / n for n in base_rows}
        sha_of = {'resources.db': args.stock_sha256} | {name: source for kind, name, source, _, _ in rows
                                                        if kind == 'base'}
        base = [archive_of[n] for n in base_names]
        live = [args.stock] + [args.patches / name for name, _ in offered]
        # The cache's names of the archives as srp-game-data reads them (build-content checked each
        # sha256), with srp-game-data itself.
        reader = game_data_id(args.bin, args.bin_source)
        base_id = [reader] + [(n, sha_of[n]) for n in base_names]
        live_id = [reader, ('resources.db', args.stock_sha256)] + offered
        print(f'the base: the stock resources.db and {len(base_rows)} published patch(es): '
              + ' '.join(n.removeprefix('resources.db.') for n in base_names[1:]))
        if any(kind == 'movie' for kind, *_ in rows) and not os.environ.get('SKHID_TEST_BUILD_MOVIE') and \
                not (args.tools and args.tools.is_file()):
            raise Problem(f'the movies need a tools.json naming java and Flex (tools/actionscript/README.md): '
                          f'{args.tools} is not a file (SKHID_FLASH_TOOLS names another)')
        world = World()
        print('the movies, from source:')
        with timed('the movies'):
            build_movies(rows, args.tree, archive_of, sha_of, game, args.tools, work, world, cache)
        with timed('the packs'):
            compose_packs(rows, base, base_id, args.vostok_port, args.packs, work, world, cache)
        with timed('the base files'):
            base_files = game.files(base)
        print('the edit lists, in order:')
        with timed('the edit lists'):
            apply_edits(rows, args.tree, base, base_id, base_files, game, work, world, cache)

        # The value types every changed file keeps (content_type_check.py):
        # a key that exists in the base/live version keeps its type, a new key
        # uses a type 0.26g files already use at that path or is allowlisted.
        with timed('the value types'):
            type_check(rows, args.tree, base, base_id, base_files, game, world, cache)
        with timed('the lamps'):
            lamp_check(world)
        with timed('the pierce lines'):
            pierce_check(rows, args.tree, base, base_files, game, world)
        with timed('the Keeper pool'):
            keeper_check(rows, base_files, world)
        with timed('the clothing fit'):
            clothing_check(base, base_files, game, world)

        # What the sources make, against what players have (build-content compares world/ again,
        # with the patch). A file's status is cached by its path and bytes, for the same live files.
        with timed('the made files, written'):
            folder = work / 'world'
            for path, data in world.bytes.items():
                target = folder / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
        with timed('the comparison with players\' files'):
            compared_key = Cache.key('players\' files', live_id)
            earlier = (cache.get(compared_key) or ({}, {}))[1]
            key_of = {path: f'{world.sha256(path)} {path}' for path in world.bytes}
            todo = work / 'to-compare'
            for path in (p for p in world.bytes if key_of[p] not in earlier):
                (todo / path).parent.mkdir(parents=True, exist_ok=True)
                (todo / path).write_bytes(world.bytes[path])
            found = game.compare(live, todo) if todo.is_dir() else {}
            compared = {p: found[p] if p in found else tuple(earlier[key_of[p]]) for p in world.bytes}
            if found:
                cache.put(compared_key, {}, {key_of[p]: compared[p] for p in world.bytes})
        with timed('the stats checks'):
            stats_checks(rows, base, base_id, base_files, live, live_id, game, world, args.tree, compared, cache)
        problems = []
        with timed('the live files'):
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
