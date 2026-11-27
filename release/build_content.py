#!/usr/bin/env python3
"""The files of players' resources.db as release/content.tsv says they should be, against what
players have: release/build-content's work (run it, not this; --help there).

    python3 -I build_content.py --stock <resources.db> --patches <folder> --list <patches.tsv>
        --bin <srp-game-data> --tree <checkout> --manifest <content.tsv> --work <new folder>
        --stock-sha256 <its sha256> [--bin-source <the srp-026g it was built from>]
        [--tools <tools.json>] [--vostok-port <program>] [--packs <folder>] [--cache <folder>]
        [--quick] [--delta-over <patch>=<its sha256>...]

With --cache, a step whose inputs are those of an earlier build (a movie, the packs, an edit list,
the checks of a file, a map's lamps or a stats table) takes what it made or found then from there
(Cache): its key is the bytes it reads (a base or players' file's by its sha256, Layers) and its
code, as build-content's header lists. With --delta-over (the earlier builds' patches), patch/ holds
only what the game sees through them otherwise than through this build's own patch (delta).

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

Writes into --work: world/ (every file a movie, pack or edit made, at its path, for build-content's
check of players' files with the patch; not with --quick, which skips that check), patch/ (those of
them players do not have so: what the next patch carries; absent when there is none) and
files.tsv (`<status> <path> <why> <live in>`, a line a file; with --delta-over also delta.tsv,
`<path> <why>` of each file in patch/), digests.tsv (`<sha256> <path>` of each file in patch/) and
game-data-id (srp-game-data as the cache knows it).
Prints each file and why, and the
checks; exits 1 on any problem (nothing in patch/ then). A live file of no base row that no row
makes is a problem, unless an earlier version of a pack row added it (<packs>/<id>-<n>, n below
the row's version, adds the path with the live bytes): printed as left from it, not packed.
"""
import argparse
import ast
import bisect
import concurrent.futures
import fnmatch
import functools
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
# configs have both there: content_type_check.number_either).
# Rudnya v0.21 (pack maps-021d-rudnya): numbers 0.21d authored as floats where 0.26g's own projects
# happen to have whole numbers.
TYPE_ALLOWLIST = {
    **{(f'projects/v021d_level_01/day_{mode}/client_project',
        '/client_objects[environment]/properties/sky_clouds_color_multiplier'):
       "0.21d's 1.28 in its day environment (a 0.21d key 0.26g does not read; its environment loader "
       'load_environment_properties_impl 0x34E1E0 reads a float as it is and an int through cvtsi2ss)'
       for mode in (2, 4, 'ah')},
    **{(f'projects/v021d_level_01/day_{mode}/client_project', '/client_objects[damage_zone]/max_armor_piercing'):
       "0.21d's 0.02 on the swamp anomaly's fourth sensor: a fraction the hit takes with max_hit "
       '(damage_zone_core::hit_objects 0x319AF0, +1C4h beside the float +1C0h), as srp reads it '
       "(mapdata's f32_or); 0.26g's own zones have a whole 0 there"
       for mode in (2, 4, 'ah')},
    # London's evening (pack london-weathers): 0.31a2's stratosphere turns at 0.62, a fraction where
    # 0.26g's corpus has whole numbers; the reader takes either (stratosphere 0x34E84F). `ah`:
    # Artifact Hunt's copy of evening_4 (artefact-hunt-1.json), the same value.
    **{(f'projects/level_09/evening_{mode}/client_project',
        '/client_objects[environment]/properties/stratosphere_rotation_speed'):
       "0.31a2's London evening 0.62; load_environment_properties_impl 0x34E1E0 reads float/int "
       "numerically (stratosphere 0x34E84F)"
       for mode in (2, 3, 4, 'ah')},
    # `ah` in the rows below: Artifact Hunt's copies of the Team Deathmatch projects (artefact-hunt-maps-1).
    # London (v0.32) (pack london-v032): 0.32a4's evening stratosphere turns at a fraction too, and its
    # evening's camera track (only the console command demo_play flies it, game_world::demo_play
    # 0x1F9800) has the editor's whole key values as ints and fractions as floats, as 0.26g's own lobby
    # track mixes them channel by channel, which its curve reader takes alike.
    **{(f'projects/v032a_level_09/evening_{mode}/client_project',
        '/client_objects[environment]/properties/stratosphere_rotation_speed'):
       "0.32a4's London evening; load_environment_properties_impl 0x34E1E0 reads float/int "
       "numerically (stratosphere 0x34E84F)"
       for mode in (2, 3, 4, 'ah')},
    **{(f'projects/v032a_level_09/evening_{mode}/client_project',
        f'/client_objects[track]/path/channels/{channel}/keys[]/value'):
       "0.32a4's London evening track: whole key values as ints, fractions as floats, as the editor writes "
       "them and as 0.26g's lobby track (projects/lobby_scene/day) mixes them"
       for mode in (2, 3, 4, 'ah') for channel in ('translateX', 'translateY', 'translateZ', 'rotateX', 'rotateY', 'rotateZ')},
    # London (v0.42) (pack london-v042): 0.44a5's vignette power is 2.56, a fraction where 0.26g's
    # corpus has whole numbers; the reader takes either (0x34F2C2, the same read as the stratosphere's).
    **{(f'projects/v044a_level_09/{weather}_{mode}/client_project',
        '/client_objects[environment]/properties/vignette_effect_power'):
       "0.44a5's London 2.56; load_environment_properties_impl 0x34E1E0 reads float/int "
       'numerically (vignette 0x34F2C2)'
       for weather in ('day', 'rain', 'evening') for mode in (2, 3, 4, 'ah')},
    # Its evening's camera track (only the console command demo_play flies it, game_world::demo_play
    # 0x1F9800): the editor writes a whole key value as an int and a fraction as a float, and 0.26g's own
    # lobby track mixes them channel by channel (rotateZ's ints, rotateY's floats), which its curve
    # reader takes alike.
    **{(f'projects/v044a_level_09/evening_{mode}/client_project',
        f'/client_objects[track]/path/channels/{channel}/keys[]/value'):
       "0.44a5's London evening track: whole key values as ints, fractions as floats, as the editor writes "
       "them and as 0.26g's lobby track (projects/lobby_scene/day) mixes them"
       for mode in (2, 3, 4, 'ah') for channel in ('translateX', 'translateY', 'translateZ', 'rotateX', 'rotateY', 'rotateZ')},
    # Its rain and evening: 0.44a5's stratosphere turns at a fraction, as London's evening above.
    **{(f'projects/v044a_level_09/{weather}_{mode}/client_project',
        '/client_objects[environment]/properties/stratosphere_rotation_speed'):
       "0.44a5's London rain and evening; load_environment_properties_impl 0x34E1E0 reads float/int "
       'numerically (stratosphere 0x34E84F)'
       for weather in ('rain', 'evening') for mode in (2, 3, 4, 'ah')},
    # Original 0.21 weather scalar representations, scoped to the exact
    # projects/keys that differ from the 0.26 corpus. The native reader tests
    # binary-config type 2 then uses movss; otherwise it uses cvtsi2ss. Keep
    # fractional donor values instead of rounding them to the corpus's ints.
    # `ah`: Artifact Hunt's copies of the Team Deathmatch projects (artefact-hunt-maps-1.json).
    **{(f'projects/v021d_level_{level}/{weather}_{mode}/client_project',
        f'/client_objects[environment]/properties/{key}'):
       "Original 0.21d weather scalar; load_environment_properties_impl 0x34E1E0 "
       "reads float/int numerically (stratosphere 0x34E84F, cloud fog 0x34EA16, "
       "Mie height 0x34ED27, rain scale 0x34F0BB, wind 0x34F1F0, vignette 0x34F2DA); "
       "legacy sky_clouds_color_multiplier and mie_fog_falloff are not read by 0.26g"
       for level, weather, keys in (
           ('01', 'evening', ('mie_fog_base_height', 'sky_clouds_color_multiplier', 'stratosphere_rotation', 'vignette_effect_power')),
           ('01', 'rain', ('mie_fog_base_height', 'rain_u_scale', 'sky_clouds_color_multiplier', 'stratosphere_rotation')),
           ('02', 'evening', ('mie_fog_base_height', 'sky_clouds_color_multiplier', 'stratosphere_rotation')),
           ('02', 'rain', ('mie_fog_base_height', 'rain_u_scale', 'sky_clouds_color_multiplier', 'stratosphere_rotation', 'wind_strength')),
           ('03', 'evening', ('mie_fog_base_height', 'mie_fog_falloff', 'stratosphere_rotation')),
           ('03', 'rain', ('mie_fog_base_height', 'rain_u_scale', 'sky_clouds_color_multiplier', 'stratosphere_rotation')),
           ('04', 'evening', ('mie_fog_base_height', 'stratosphere_rotation')),
           ('04', 'rain', ('mie_fog_base_height', 'rain_u_scale', 'sky_clouds_color_multiplier', 'stratosphere_rotation')),
           ('05', 'evening', ('mie_fog_base_height', 'sky_clouds_color_multiplier', 'sky_clouds_fog_power', 'stratosphere_rotation', 'vignette_effect_power')),
           ('05', 'rain', ('mie_fog_base_height', 'rain_u_scale', 'sky_clouds_color_multiplier', 'stratosphere_rotation')),
           ('06', 'evening', ('mie_fog_base_height', 'stratosphere_rotation')),
           ('06', 'rain', ('mie_fog_base_height', 'sky_clouds_color_multiplier', 'stratosphere_rotation')),
       ) for mode in (2, 4, 'ah') for key in keys},
    **{(f'projects/v021d_level_01/{weather}_{mode}/client_project', '/client_objects[damage_zone]/max_armor_piercing'):
       "Same authored Rudnya day swamp sensor's 0.02; damage_zone_core::hit_objects 0x319AF0 "
       "reads float +1C4h beside max_hit +1C0h (mapdata f32_or), retained in both original weathers"
       for weather in ('rain', 'evening') for mode in (2, 4, 'ah')},
}

SIMULATION_CHANGES = 'srp-026g/crates/session/src/v026g/simulation_changes.tsv'

# The operations of a pack.json that vostok-port knows (resource-porter src/content_pack): a movie
# (a file the movie rows made, given to it as --replace) is read only by an operation that edits
# it, by its path (config_*: `path`; atlas_icons_add: `swf`, without one ATLAS_SWF); the others
# give it back as it was. An operation not named here may read any of them.
PACK_OPERATIONS = {'add_file', 'replace_file', 'localization_add', 'config_append', 'config_set',
                   'atlas_icons_add', 'dictionary_add'}
ATLAS_SWF = 'flash_movies/lobby_menu.swf'   # vostok-port's atlas::DEFAULT_SWF


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


SOURCE = Path(__file__).read_text(encoding='utf-8')    # as loaded: an edit during a build is not this code


@functools.cache
def function_sources():
    return {node.name: ast.get_source_segment(SOURCE, node)
            for node in ast.parse(SOURCE).body if isinstance(node, ast.FunctionDef)}


def code_digest(*parts):
    """The sha256 of the source of these functions of this file (and of any other values given):
    what a check computes with, so that an edit elsewhere in the file leaves its cached results valid."""
    return sha256(json.dumps([function_sources()[p.__name__] if callable(p) else p for p in parts],
                             default=str).encode())


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
        """({path: sha256}, said) of the step of that key, every file there, or None. The files'
        bytes are read when asked (blob)."""
        if not self.root:
            return None
        try:
            entry = json.loads((self.root / 'steps' / f'{key}.json').read_text())
            if not all(self.blob_path(digest).is_file() for digest in entry['files'].values()):
                return None
            return entry['files'], entry['said']
        except (OSError, ValueError, KeyError):
            return None

    def blob_path(self, digest):
        return self.root / 'blobs' / digest[:2] / digest

    def blob(self, digest):
        """The bytes of that sha256, which get named; a file changed since is a Problem (its step
        cannot run again this late), and is deleted, so that the next build makes it again."""
        path = self.blob_path(digest)
        data = path.read_bytes()
        if sha256(data) != digest:
            path.unlink(missing_ok=True)
            raise Problem(f'{path} in the cache is not the file of that sha256 (deleted): run the build again')
        return data

    def put(self, key, files, said, digests=None):
        """Keeps what the step of that key made (`digests`: the files' sha256s, when known)."""
        if not self.root:
            return
        names = {}
        for path, data in files.items():
            digest = digests[path] if digests else sha256(data)
            blob = self.blob_path(digest)
            if not (blob.is_file() and blob.stat().st_size == len(data)):
                write_new(blob, data)
            names[path] = digest
        write_new(self.root / 'steps' / f'{key}.json', json.dumps({'files': names, 'said': said}).encode())


class Layers:
    """Archives laid over one another as srp-game-data reads them (the base, or players' files):
    which archive gives each file, and each file's sha256 as it gives it, read once and then known
    from the cache by the archive's sha256 and the reader (`reader`), so a step's key can name the
    exact bytes it reads without reading them."""

    def __init__(self, game, cache, reader, archives, sha_of):
        self.game, self.cache, self.archives = game, cache, archives
        self.of = game.files(archives)                      # path: the name of the archive giving it
        self.key_of = {Path(a).name: Cache.key('file digests', reader, sha_of[Path(a).name]) for a in archives}
        self.known, self.new = {}, set()                    # archive name: {path: sha256}; names to save

    def digests(self, paths, keep=True):
        """({path: sha256}, {path: bytes} of those read now (none without `keep`)) of files the
        archives give."""
        out, todo, kept = {}, [], {}
        for path in paths:
            name = self.of[path]
            if name not in self.known:
                self.known[name] = (self.cache.get(self.key_of[name]) or ({}, {}))[1]
            if path in self.known[name]:
                out[path] = self.known[name][path]
            else:
                todo.append(path)
        for at in range(0, len(todo), 2000):
            got = self.game.get(self.archives, todo[at:at + 2000])
            for path, data in got.items():
                out[path] = self.known[self.of[path]][path] = sha256(data)
                self.new.add(self.of[path])
            if keep:
                kept.update(got)
        return out, kept

    def save(self):
        for name in sorted(self.new):
            self.cache.put(self.key_of[name], {}, self.known[name])
        self.new.clear()


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
        # `new <path>` or `<same|differs> <path> <archive>`, a new or differing file the server
        # reads followed by ` server`: a path may hold spaces (0.21d's `render/material #825364/`
        # folders), an archive name none.
        for line in text.splitlines():
            status, rest = line.split(' ', 1)
            if status != 'same':
                rest = rest.removesuffix(' server')
            path, name = (rest, '') if status == 'new' else rest.rsplit(' ', 1)
            out[path] = (status, name)
        return out


class World:
    """The files the sources make: path -> its sha256, why (the rows that made them, in order), and
    its bytes: held, or (a step taken from the cache) read from the cache when asked, so that a
    build reads only the files a step that runs needs."""

    def __init__(self, cache):
        self.cache, self.digests, self.why, self.held = cache, {}, {}, {}

    def __contains__(self, path):
        return path in self.digests

    def __iter__(self):
        return iter(self.digests)

    def __len__(self):
        return len(self.digests)

    def put(self, path, data, reason, digest=None):
        self._put(path, digest or sha256(data), reason, data)

    def put_cached(self, path, digest, reason):
        self._put(path, digest, reason, None)

    def _put(self, path, digest, reason, data):
        if self.digests.get(path) == digest:
            return
        self.digests[path] = digest
        self.held.pop(path, None)
        if data is not None:
            self.held[path] = data
        self.why.setdefault(path, []).append(reason)

    def sha256(self, path):
        return self.digests[path]

    def data(self, path):
        held = self.held.get(path)
        return held if held is not None else self.cache.blob(self.digests[path])


class Drawn:
    """The movies a pack draws into (vostok-port's lobby icons) without their code: a pack composes
    over a movie's graphics (flash_tree.py graphics: the tree's movie, a tag where its code goes), so
    that a change of a tree's code alone leaves the packs' runs as they were, and the code goes into
    the composed movie after them (flash_tree.py code): the movie the tree builds, with what the packs
    drew. Each cached by what it reads."""

    def __init__(self, tree, programs, stand_in, work, game, cache, world):
        self.tree, self.programs, self.stand_in = tree, programs, stand_in
        self.work, self.game, self.cache, self.world = work, game, cache, world
        self.movies, self.made = {}, {}             # path: (name, movie dir, its original's archive, sha256)

    def add(self, path, name, movie_dir, archive, archive_sha):
        self.movies[path] = (name, movie_dir, archive, archive_sha)

    def program(self, *args):
        if self.stand_in:
            return [self.stand_in, *args]
        return [sys.executable, self.tree / 'tools' / 'actionscript' / 'flash_tree.py', *args]

    def graphics(self, path):
        """(sha256, bytes or None when in the cache) of the movie at `path` without its code."""
        if path in self.made:
            return self.made[path]
        name, movie_dir, archive, archive_sha = self.movies[path]
        # Everything of the tree but its code (src/, tables/ and movie.json's data tables).
        drawing = sha256(json.dumps([(e.name, tree_digest(e) if e.is_dir() else sha256(e.read_bytes()))
                                     for e in sorted(movie_dir.iterdir()) if e.name not in ('src', 'tables')]).encode())
        key = Cache.key('movie graphics', name, archive_sha, drawing, self.programs, self.stand_in, sys.version)
        made = self.cache.get(key)
        if made:
            self.made[path] = (made[0][path], None)
            return self.made[path]
        folder = self.work / 'graphics' / name
        folder.mkdir(parents=True)
        (folder / 'original.swf').write_bytes(self.game.get([archive], [path])[path])
        out = folder / 'graphics.swf'
        run(self.program('graphics', movie_dir, folder / 'original.swf', out), f'the graphics of movie {name}')
        data = out.read_bytes()
        shutil.rmtree(folder)
        self.cache.put(key, {path: data}, '')
        self.made[path] = (sha256(data), data)
        return self.made[path]

    def graphics_data(self, path):
        digest, data = self.graphics(path)
        return data if data is not None else self.cache.blob(digest)

    def with_code(self, path, drawn, digest):
        """(bytes or None, sha256) of the movie the tree at `path` builds (in world now) with what the
        packs drew into its graphics (`drawn`: its bytes or None, `digest`)."""
        built = self.world.sha256(path)
        key = Cache.key('movie with its code', path, digest, built, self.programs, self.stand_in, sys.version)
        made = self.cache.get(key)
        if made:
            return None, made[0][path]
        folder = self.work / 'with-code' / self.movies[path][0]
        folder.mkdir(parents=True)
        (folder / 'drawn.swf').write_bytes(drawn if drawn is not None else self.cache.blob(digest))
        (folder / 'built.swf').write_bytes(self.world.data(path))
        out = folder / 'movie.swf'
        run(self.program('code', folder / 'drawn.swf', folder / 'built.swf', out), f'the code of movie {path}')
        data = out.read_bytes()
        shutil.rmtree(folder)
        self.cache.put(key, {path: data}, '')
        return data, sha256(data)


def build_movies(rows, tree, archive_of, sha_of, game, tools, work, world, cache):
    """Every movie row's movie, built from its tree, in `world`; returns their Drawn."""
    stand_in = os.environ.get('SKHID_TEST_BUILD_MOVIE')
    # What a movie's build reads besides its tree and its original: the build's programs and the tools.
    programs = files_digest(sorted((tree / 'tools' / 'actionscript').glob('*.py'))
                            + [tree / 'srp-026g' / 'client-patches' / 'build_lobby_026g.py']
                            + [Path(p) for p in (stand_in, tools) if p])
    drawn = Drawn(tree, files_digest(sorted((tree / 'tools' / 'actionscript').glob('*.py'))
                                     + [Path(p) for p in (stand_in,) if p]), stand_in, work, game, cache, world)
    for kind, name, _, _, at in rows:
        if kind != 'movie':
            continue
        movie_dir = tree / 'srp-026g' / 'flash' / name
        spec = json.loads((movie_dir / 'movie.json').read_text())
        archive = spec.get('archive')
        if archive not in archive_of:
            raise Problem(f'{at}: movie {name}: its original is in {archive} (movie.json), which is neither the '
                          'stock resources.db nor a base row')
        drawn.add(spec['movie'], name, movie_dir, archive_of[archive], sha_of[archive])
        # The repository's tables movie.json's `data` makes classes of (flash_tree.data_sources).
        sources = files_digest(sorted(tree / path for path in spec.get('data', {}).values()))
        key = Cache.key('movie', name, sha_of[archive], tree_digest(movie_dir), sources, programs, stand_in,
                        sys.version)
        made = cache.get(key)
        if made:
            digest = made[0][spec['movie']]
            world.put_cached(spec['movie'], digest, f'movie {name} (srp-026g/flash/{name})')
            print(f'  movie {name}: {spec["movie"]}, {cache.blob_path(digest).stat().st_size} bytes, '
                  f'sha256 {digest[:16]}… (as an earlier build of the same sources made it: the cache)')
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
    return drawn


def movies_packs_edit(specs, movies):
    """The movies (of `movies`, their paths) the packs' operations may read: every one an
    operation names anywhere in it, ATLAS_SWF for an atlas operation that names none, and all of
    them when an operation is not one of PACK_OPERATIONS."""
    def strings(value):
        if isinstance(value, str):
            yield value
        elif isinstance(value, dict):
            for item in value.values():
                yield from strings(item)
        elif isinstance(value, list):
            for item in value:
                yield from strings(item)
    edited = set()
    for spec in specs:
        for operation in spec.get('operations', []):
            kind = operation.get('op') if isinstance(operation, dict) else None
            if kind not in PACK_OPERATIONS:
                return set(movies)
            if kind == 'atlas_icons_add' and 'swf' not in operation:
                edited.add(ATLAS_SWF)
            edited.update(strings(operation))
    return edited & set(movies)


PACK_JOBS = 3                                         # vostok-port runs at once
NO_PATCH = 'the base already has every file as the packs make it: no patch'


def pack_runs(specs, links=()):
    """The packs' operations in runs of vostok-port composed apart: [[(pack index, [operation
    index])]], each pack in its order. vostok-port makes each path of the operations on it alone
    (its conflicts are of one path, a table or a string of one file), so operations on a path in
    common (a localization_add on every dictionary, a dictionary_add on the dictionaries' file, an
    atlas on its movie, an edit of a file a pack adds, a layered replace) are one group; and since
    it checks what the files it adds refer to (a model's materials) across them, the files a pack
    adds or replaces are one group with those of the packs it requires. The groups of one pack are
    its run, those of the same several packs theirs, so a pack's change recomposes the runs of its
    operations only. A pack without operations is a group of its own. None: an operation not in
    PACK_OPERATIONS (then the packs run whole, at once). `links`: pairs of pack ids whose files
    refer to each other's without a `requires` (learnt from vostok-port's refusals: Composer), one
    group as if one required the other."""
    nodes, touched = [], []
    ids = {spec.get('id'): i for i, spec in enumerate(specs)}
    linked = {}
    for a, b in links:
        linked.setdefault(a, set()).add(b)
        linked.setdefault(b, set()).add(a)
    for i, spec in enumerate(specs):
        operations = spec.get('operations', [])
        related = set(spec.get('requires', [])) | linked.get(spec.get('id'), set())
        for k, operation in enumerate(operations):
            kind = operation.get('op') if isinstance(operation, dict) else None
            if kind not in PACK_OPERATIONS:
                return None
            path = {'dictionary_add': 'gameplay/db_static_dictionaries', 'localization_add': 'localization/',
                    'atlas_icons_add': operation.get('swf', ATLAS_SWF)}.get(kind, operation.get('path'))
            if not isinstance(path, str):
                return None
            nodes.append((i, k))
            touched.append([path] + ([('files', i)] + [('files', ids[r]) for r in sorted(related) if r in ids]
                                     if kind in ('add_file', 'replace_file') else []))
        if not operations:
            nodes.append((i, None))
            touched.append([('pack', i)])
    first = {}                                        # what a path's group is known by: its first node
    parent = list(range(len(nodes)))

    def root(n):
        while parent[n] != n:
            parent[n] = parent[parent[n]]
            n = parent[n]
        return n

    for n, paths in enumerate(touched):
        for path in paths:
            a, b = root(n), root(first.setdefault(path, n))
            parent[max(a, b)] = min(a, b)
    packs_of = {}
    for n, (i, _) in enumerate(nodes):
        packs_of.setdefault(root(n), set()).add(i)
    runs = {}
    for n, (i, k) in enumerate(nodes):
        operations = runs.setdefault(tuple(sorted(packs_of[root(n)])), {}).setdefault(i, [])
        if k is not None:
            operations.append(k)
    return [sorted(packs.items()) for _, packs in sorted(runs.items())]


def compose_packs(rows, base, base_shas, vostok, packs_dir, work, world, cache, drawn):
    applied = [(name, source, at) for kind, name, source, _, at in rows if kind == 'pack' and SHA.match(source)]
    if not applied:
        return
    if not vostok:
        raise Problem('the manifest has packs to compose: SKHID_VOSTOK_PORT must name resource-porter\'s vostok-port')
    if not packs_dir:
        raise Problem('the manifest has packs to compose: SKHID_PACKS_DIR must name the folder of the packs')
    reason = 'pack ' + ', '.join(name for name, _, _ in applied)
    folders, specs = [], []
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
        folders.append(folder)
        specs.append(spec)
    # What vostok-port reads: the base archives (their sha256: it reads them itself, not through
    # srp-game-data), each pack's folder (its files by size and time: a folder is one version, made
    # once) and operations, the bytes of the movies they edit (their graphics, Drawn, given as
    # --replace; the others are the movie rows') and itself (the program and the files beside it
    # named after it: a wrapper and its binary). Each run (pack_runs) is composed and cached apart.
    program = Path(vostok).resolve()
    composer = Composer(vostok, base, base_shas, files_digest(sorted(program.parent.glob(program.name + '*'))),
                        applied, folders, specs, work, world, cache, drawn)
    # vostok-port checks what the files it adds refer to (a model's materials, a project's probes)
    # across every file it makes: a run whose files refer to another pack's (no `requires` says so)
    # is refused. The packs whose added paths its refusal names are linked to it (kept in the cache,
    # by their ids) and run with it; a refusal that names none: the packs composed whole, at once.
    links_key = Cache.key('the packs whose files refer to each other\'s')
    links = {tuple(pair) for pair in (cache.get(links_key) or ({}, []))[1]}
    adder = {operation['path']: spec.get('id') for spec in specs for operation in spec.get('operations', [])
             if isinstance(operation, dict) and operation.get('op') in ('add_file', 'replace_file')}
    while True:
        runs = pack_runs(specs, links)
        if runs is None:
            print('    an operation build_content.py does not know (PACK_OPERATIONS): the packs composed at once')
            composed = composer.compose(None)
            break
        try:
            composed = composer.compose(runs)
            break
        except RunsRefused as refused:
            found = {tuple(sorted((ident, adder[word])))
                     for packs, said in refused.runs for word in re.findall(r'[^\s,;:()"\']+', said)
                     if word in adder for ident in packs if adder[word] != ident}
            if not found - links:
                print(f'    the packs in runs: {refused}; composed at once')
                composed = composer.compose(None)
                break
            print(f'    the packs in runs: their files refer to each other\'s: {sorted(found - links)}; run together')
            links |= found
            cache.put(links_key, {}, sorted(links))
    print(f'    {len(composed)} file(s), {composer.fresh} run(s) of vostok-port now'
          + ('' if composer.fresh else ': as an earlier build of the same packs over the same movies composed them: '
             'the cache'))
    for path, (data, digest) in sorted(composed.items()):
        if path in drawn.movies:                # a movie's graphics, drawn into: its code goes in
            data, digest = drawn.with_code(path, data, digest)
        if data is None:
            world.put_cached(path, digest, reason)
        else:
            world.put(path, data, reason, digest)


class RunsRefused(Problem):
    """vostok-port refused runs: [(the ids of a run's packs, what it said)]."""

    def __init__(self, runs):
        self.runs = runs
        super().__init__('vostok-port pack patch failed:\n' + '\n'.join(
            '    ' + line for _, said in runs[:1] for line in said.splitlines()[-25:]))


class Composer:
    """vostok-port pack patch over the base, run by run (each a pack folder of the operations of
    the run: its files, a pack.json of those operations and of the packs it requires that the run
    has, so that vostok-port's checks of the order and of layered files hold within it), or (None)
    every pack whole at once."""

    def __init__(self, vostok, base, base_shas, program_id, applied, folders, specs, work, world, cache, drawn):
        self.vostok, self.base, self.base_shas, self.program_id = vostok, base, base_shas, program_id
        self.applied, self.folders, self.specs = applied, folders, specs
        self.work, self.world, self.cache, self.drawn = work, world, cache, drawn
        self.fresh, self.stats = 0, {}

    def folder_stats(self, i, operations):
        """The files of pack i (by size and time) its operations read (None: every file): an
        add_file's or replace_file's source, an atlas's icons; the others read none."""
        folder = self.folders[i]
        if operations is None or not self.specs[i].get('operations'):
            if i not in self.stats:
                self.stats[i] = [(p.relative_to(folder).as_posix(), p.stat().st_size, p.stat().st_mtime_ns)
                                 for p in sorted(folder.rglob('*')) if p.is_file()]
            return self.stats[i]
        read = []
        for k in operations:
            operation = self.specs[i]['operations'][k]
            if operation['op'] in ('add_file', 'replace_file'):
                read.append(operation.get('source') or f'files/{operation["path"]}')
            elif operation['op'] == 'atlas_icons_add':
                read += [icon.get('source') for icon in operation.get('icons', [])]
        out = []
        for name in read:
            file = folder / str(name)
            out.append((name, *((file.stat().st_size, file.stat().st_mtime_ns) if file.is_file() else (None,))))
        return out

    def requires(self, i):
        """The ids pack i requires, directly or through another."""
        ids, out, todo = {spec.get('id'): j for j, spec in enumerate(self.specs)}, set(), [i]
        while todo:
            for required in self.specs[todo.pop()].get('requires', []):
                if required not in out:
                    out.add(required)
                    if required in ids:
                        todo.append(ids[required])
        return out

    def compose(self, runs):
        """{path: (bytes, or None when in the cache, sha256)} of the runs (None: the packs whole)."""
        whole = runs is None
        composed, todo = {}, []
        for number, run in enumerate([[(i, None) for i in range(len(self.specs))]] if whole else runs):
            ids = {self.specs[i].get('id') for i, _ in run}
            manifests = {}
            for i, operations in run:
                if operations is None:
                    manifests[i] = None
                    continue
                spec = dict(self.specs[i], operations=[self.specs[i]['operations'][k] for k in operations])
                if 'requires' in spec:
                    spec['requires'] = [r for r in spec['requires'] if r in ids] + \
                        sorted(r for r in self.requires(i) - set(spec['requires']) if r in ids)
                manifests[i] = json.dumps(spec, indent=1, ensure_ascii=False)
            edited = movies_packs_edit([self.specs[i] if manifests[i] is None else json.loads(manifests[i])
                                        for i, _ in run], self.world)
            key = Cache.key('packs', self.base_shas, self.program_id,
                            [(self.folders[i].name, manifests[i], self.folder_stats(i, operations))
                             for i, operations in run],
                            sorted((path, self.drawn.graphics(path)[0]) for path in edited))
            made = self.cache.get(key)
            if made:
                composed.update({path: (None, digest) for path, digest in made[0].items()})
            else:
                todo.append(self.prepare(number, whole, run, manifests, edited)
                            + (key, edited, [self.specs[i].get('id') for i, _ in run]))
        self.fresh += len(todo)
        refused = []
        with concurrent.futures.ThreadPoolExecutor(PACK_JOBS) as jobs:
            for (runs_dir, label, command, key, edited, packs), result in zip(todo, jobs.map(self.run, todo)):
                made = self.collect(runs_dir, result, key, edited)
                if isinstance(made, str):
                    refused.append((packs, made))
                else:
                    composed.update(made)
        if refused:
            raise RunsRefused(refused)
        return composed

    def prepare(self, number, whole, run, manifests, edited):
        """(its folder, its label, the command) of a run to compose: the folders of its derived packs."""
        command = [self.vostok, 'pack', 'patch', '--base-db', self.base[0]]
        for patch in self.base[1:]:
            command += ['--base-patch', patch]
        runs_dir = self.work / 'pack-runs' / f'{number}{"-whole" if whole else ""}'
        shutil.rmtree(runs_dir, ignore_errors=True)
        for i, _ in run:
            folder = self.folders[i]
            if manifests[i] is not None:
                derived = runs_dir / folder.name
                derived.mkdir(parents=True)
                for entry in folder.iterdir():
                    if entry.name != 'pack.json':
                        (derived / entry.name).symlink_to(entry.resolve())
                (derived / 'pack.json').write_text(manifests[i], encoding='utf-8')
                folder = derived
            command += ['--pack', folder]
        for path in sorted(edited):
            target = self.work / 'movies-for-packs' / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(self.drawn.graphics_data(path))
            command += ['--replace', f'{path}={target}']
        label = f'packs {"whole" if whole else f"run {number}"} ({", ".join(self.applied[i][0] for i, _ in run)})'
        return runs_dir, label, command + ['--out-dir', runs_dir / 'out']

    @staticmethod
    def run(job):
        runs_dir, label, command, *_ = job
        start = time.monotonic()
        result = subprocess.run([str(c) for c in command], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        return result, f'time: {label} {time.monotonic() - start:.1f} s'

    def collect(self, runs_dir, ran, key, edited):
        result, took = ran
        print(took)
        out = runs_dir / 'out'
        if result.returncode and NO_PATCH not in result.stdout:
            shutil.rmtree(runs_dir, ignore_errors=True)
            return result.stdout
        files = {file.relative_to(out).as_posix(): file.read_bytes()
                 for file in sorted(p for p in out.rglob('*') if p.is_file())} if out.is_dir() else {}
        shutil.rmtree(runs_dir, ignore_errors=True)
        # A movie no operation edits is not given (the key leaves its bytes out).
        for path in sorted(set(files) & (set(self.world) - edited)):
            raise Problem(f'vostok-port changed {path}, which no operation of the packs names: '
                          'build_content.py (movies_packs_edit) does not know what reads it')
        digests = {path: sha256(data) for path, data in files.items()}
        self.cache.put(key, files, result.stdout, digests)
        return {path: (data, digests[path]) for path, data in files.items()}


def left_by_earlier_packs(rows, packs_dir, paths, base_files, live, game, cache, live_id):
    """{path: '<id>@<n>'} of the live files `paths` (no base row's, made by no row) that an earlier
    version of a composed pack left: its folder <packs>/<id>-<n>, n below the row's version, adds the
    path (add_file, not over a base file) and its files/<path> is the live file byte for byte, so it is
    what that version shipped. Nothing refers to it: a pack's new version puts what it changes at new
    paths (a patch only adds files), so it stays as it is, not packed. Cached by players' archives
    (`live_id`) and each such path's files of the earlier versions (by size and time)."""
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
            files = f'{manifest.parent}/files/'
            for op in spec.get('operations', []):
                if op.get('op') == 'add_file':
                    added.setdefault(op['path'], []).append((f'{ident}@{n}', files + op['path']))
    wanted = [p for p in paths if p in added]

    def stat(file):
        try:
            info = os.stat(file)
        except OSError:
            return None
        return info.st_size, info.st_mtime_ns

    key = Cache.key('left by earlier packs', code_digest(left_by_earlier_packs), live_id,
                    [(p, [(version, file, stat(file)) for version, file in added[p]]) for p in wanted])
    known = cache.get(key)
    if known:
        return known[1]
    got = game.get(live, wanted)
    left = {}
    for path in wanted:
        for version, file in added[path]:
            if os.path.isfile(file) and Path(file).read_bytes() == got[path]:
                left[path] = version
                break
    cache.put(key, {}, left)
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


def apply_edits(rows, tree, base, work, world, cache):
    tool = edits_dir(tree) / 'content_edit.py'
    # What an edit list's run reads besides the list and its files: content_edit.py and the modules
    # beside it (not their tests), and for a `localization` list the localization's modules of
    # client-patches and its folder.
    programs = files_digest(sorted(p for p in edits_dir(tree).glob('*.py') if not p.name.startswith('test_')))
    localization = files_digest(sorted(p for p in (tree / 'srp-026g' / 'client-patches').glob('*localization*.py')
                                       if not p.name.startswith('test_')))
    known = sorted(set(base.of) | set(world))
    present, count = set(known), len(world)
    for kind, name, source, _, at in rows:
        if kind != 'edit' or source != '-':
            continue
        for path in list(world)[count:] if count != len(world) else []:    # the paths added since (in order)
            if path not in present:
                present.add(path)
                bisect.insort(known, path)
        count = len(world)
        edits = {} if name.endswith('.tsv') else json.loads((edits_dir(tree) / name).read_text(encoding='utf-8'))
        if name.endswith('.tsv'):
            paths = own_content_edit().stats_paths(own_content_edit().stats_category(name), known)
        else:
            paths = edit_paths(edits, known)
        movies = [p for p in paths if p.startswith('flash_movies/')]
        if movies:
            raise Problem(f'{at}: edit {name} names {movies[0]}: a movie changes in its source tree')
        missing = [p for p in paths if p not in world and p not in base.of]
        if missing:
            raise Problem(f'{at}: edit {name}: {missing[0]} is in no archive of the base and made by no row above')
        # Its files by their bytes as the rows above (or the base) leave them.
        digests, got = base.digests([p for p in paths if p not in world])
        folder = edits.get('localization')
        key = Cache.key('edit', name, sha256((edits_dir(tree) / name).read_bytes()), programs, sys.version,
                        [(p, world.sha256(p) if p in world else digests[p]) for p in paths],
                        [localization, tree_digest(edits_dir(tree) / folder)] if isinstance(folder, str) else None)
        made = cache.get(key)
        if made:
            files, said = made
            print(f'  edit {name}: {said} (as an earlier build of the list over the same files made it: the cache)')
            for path, digest in files.items():
                world.put_cached(path, digest, f'edit {name}')
            continue
        live = work / 'edits' / name / 'live'
        out = work / 'edits' / name / 'out'
        got.update(base.game.get(base.archives, [p for p in paths if p not in world and p not in got]))
        for path in paths:
            target = live / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(world.data(path) if path in world else got[path])
        with timed(f'edit {name}'):
            said = run([sys.executable, '-I', tool, edits_dir(tree) / name, live, out], f'edit {name} (content_edit.py)')
        said = said.strip().splitlines()[-2] if len(said.strip().splitlines()) > 1 else said.strip()
        print(f'  edit {name}: {said}')
        files = {file.relative_to(out).as_posix(): file.read_bytes()
                 for file in sorted(p for p in out.rglob('*') if p.is_file())}
        shutil.rmtree(work / 'edits' / name)
        digests = {path: sha256(data) for path, data in files.items()}
        cache.put(key, files, said, digests)
        for path, data in files.items():
            world.put(path, data, f'edit {name}', digests[path])


def type_check(rows, tree, base, world, cache, whole):
    """Every file the manifest makes keeps the value type of each key path the
    base/live version has, and a new key uses a 0.26g type for its path or is
    allowlisted (content_type_check.py); an entry of a list the manifest's edit
    lists key (append_entry's unique_key) is weighed against the entry with
    its key, wherever an edit moved it. A Problem lists every offender with
    the file, the path, the old type and the new type. What the base gives
    (the corpus, its configs' problems) is cached by the bytes of the base's
    configs, each made file's problems by its path, its bytes, its previous
    version's bytes, its list keys and the corpus; `whole` (--full) checks
    every made file again."""
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        'content_type_check', str(Path(__file__).resolve().parent / 'content_type_check.py'))
    ctc = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(ctc)
    # The config reader is infrastructure of this checkout, not of --tree
    # (which supplies the content): use build-content's own content_edit.py.
    own = Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' / 'content-edit'
    walk = ctc.load_walk(str(own))
    sources = [files_digest([Path(spec.origin), own / 'content_edit.py']),
               code_digest(type_check, sorted(TYPE_ALLOWLIST.items())), sys.version]
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
    corpus_paths = sorted(p for p in base.of if p.endswith(ctc.CONFIG_SUFFIXES))
    digests, corpus = base.digests(corpus_paths)
    base_key = Cache.key('the types of the base', sources, [(p, digests[p]) for p in corpus_paths])
    known = cache.get(base_key)
    if known:
        corp = {path: set(types) for path, types in known[1]['corpus'].items()}
        base_problems = known[1]['problems']
    else:
        corpus.update(base.game.get(base.archives, [p for p in corpus_paths if p not in corpus]))
        corp = ctc.corpus_with(walk, corpus)
        base_problems = [p for p in (refused(path, corpus[path], 'base') for path in corpus_paths) if p]
        cache.put(base_key, {}, {'corpus': {path: sorted(types) for path, types in corp.items()},
                                 'problems': base_problems})
    del corpus
    # Each made file's problems: its own parse (a config path's), its previous version's (the base's
    # bytes, where the base has it; None for a file no base archive has: a pack's new file), its
    # value types against them.
    files_key = Cache.key('the types of made files', sources)
    stored = (cache.get(files_key) or ({}, {}))[1]
    earlier = {} if whole else stored
    previous_digest, _ = base.digests([p for p in world if p in base.of], keep=False)
    key_of = {path: Cache.key(path, world.sha256(path), previous_digest.get(path), keys.get(path, {}), base_key)
              for path in world}
    todo = [p for p in sorted(world) if key_of[p] not in earlier]
    previous = base.game.get(base.archives, [p for p in todo if p in base.of])
    found = {}
    for path in todo:
        config = path.endswith(ctc.CONFIG_SUFFIXES)
        data = world.data(path)
        found[key_of[path]] = {
            'made': refused(path, data, 'made') if config else None,
            'previous': refused(path, previous[path], 'previous') if config and path in previous else None,
            'types': ctc.violations(walk, previous.get(path), data, path, corp, TYPE_ALLOWLIST,
                                    keys.get(path, {}))}
    if found:
        cache.put(files_key, {}, stored | found)     # another tree's files kept for its next build
    each = [found.get(key_of[p]) or earlier[key_of[p]] for p in sorted(world)]
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
    print(f'value types: every key the base has keeps its type across {len(world)} file(s)'
          + ('' if whole else f' ({len(todo)} checked now, the others as an earlier build checked the same bytes)'))


@functools.cache
def own_content_edit():
    """This checkout's content_edit.py (not --tree's), loaded once."""
    import importlib.util
    path = Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' / 'content-edit' / 'content_edit.py'
    spec = importlib.util.spec_from_file_location('content_edit', str(path))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stats_checks(rows, base, players, world, tree, compared, cache):
    """Per content_edit.STATS category: each config (or list entry) is its table row, its cards follow
    it, and a changed match column needs the `next` row of SIMULATION_CHANGES. A category's findings
    are cached by the bytes of its configs, its table and what players have of them; within it, a
    row's (its cells against its configs) by the row and the bytes of its configs, a config's card
    lines by its bytes, so that a changed config checks its row and its cards again, not every one."""
    ce = own_content_edit()
    sources = [files_digest([Path(ce.__file__)]), code_digest(stats_checks), sys.version]
    tables = {name: edits_dir(tree) / name for kind, name, source, _, _ in rows
              if kind == 'edit' and source == '-' and name.endswith('.tsv')}
    problems, moved = [], []
    known = set(base.of) | set(world)
    for category_name, category in ce.STATS.items():
        paths = ce.stats_paths(category, known)
        table = tables.get(f'{category_name}.tsv')
        changed = [p for p in paths if p in world and compared[p][0] in ('differs', 'new')]
        digests, _ = base.digests([p for p in paths if p not in world])
        theirs, _ = players.digests([p for p in changed if compared[p][0] == 'differs'])
        key = Cache.key('stats', category_name, sources, sha256(table.read_bytes()) if table else None,
                        [(p, world.sha256(p) if p in world else digests[p]) for p in paths],
                        [(p, compared[p][0], theirs.get(p)) for p in changed])
        found = cache.get(key)
        if found:
            found = found[1]
        else:
            found = {'problems': []}
            configs = {p: bytearray(b) for p, b in base.game.get(base.archives, [p for p in paths if p not in world]).items()}
            configs.update({p: bytearray(world.data(p)) for p in paths if p in world})
            targets = ce.stats_targets(category, configs)
            _, columns, table_rows = ce.read_stats(str(table)) if table else (None, category.get('columns', {}), None)
            digest_of = {p: world.sha256(p) if p in world else digests[p] for p in paths}
            parts_key = Cache.key('stats rows and cards', category_name, sources)
            stored, fresh = (cache.get(parts_key) or ({}, {}))[1], {}

            def part(key, check):
                if key not in stored and key not in fresh:
                    fresh[key] = check()
                return fresh[key] if key in fresh else stored[key]

            if table:
                head = next(line for line in table.read_text(encoding='utf-8').splitlines()
                            if line.strip() and not line.startswith('#'))
                found['problems'] += ce.stats_unmatched(category, table_rows, targets, table.name)
                for row, places in targets.items():
                    if row in table_rows:
                        found['problems'] += part(
                            Cache.key('row', table.name, head, row, table_rows[row],
                                      [(p, off, digest_of[p]) for p, off in places]),
                            lambda: ce.stats_problems(category, columns, {row: table_rows[row]}, {row: places},
                                                      configs, table.name))
            if category.get('cards'):
                found['problems'] += [problem for p in paths for problem in part(
                    Cache.key('cards', p, digest_of[p]),
                    lambda: [f'{p}: card line {prop_id} says {text!r}, its config {want!r}'
                             for prop_id, text, want in ce.card_differences(configs[p], category['cards'])])]
            if fresh:
                cache.put(parts_key, {}, stored | fresh)
            # A config players lack (a new key/value table's) has no values: each of its match values moved.
            had = players.game.get(players.archives, [p for p in changed if compared[p][0] == 'differs'])
            try:
                found['moved'] = ce.stats_moved(category, columns, {p: bytearray(b) for p, b in had.items()},
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


def lamp_check(world, cache):
    """Every made client_project's breakable-lamp bindings name its own
    objects (scripts/breakable-lamps/make_edits.py check_built): a wrong
    index or a moved light, after any later edit, is a Problem. A project's
    problems are cached by its path and bytes and the check's sources."""
    import importlib.util
    path = Path(__file__).resolve().parent.parent / 'srp-026g' / 'scripts' / 'breakable-lamps' / 'make_edits.py'
    spec = importlib.util.spec_from_file_location('lamp_edits', str(path))
    lamps = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(lamps)
    projects = sorted(p for p in world if p.endswith('/client_project'))
    key = Cache.key('the lamps of made projects', files_digest([path, Path(lamps.EDIT)]), sys.version)
    earlier = (cache.get(key) or ({}, {}))[1]
    key_of = {p: f'{world.sha256(p)} {p}' for p in projects}
    found = {key_of[p]: lamps.check_built(p, bytearray(world.data(p))) for p in projects if key_of[p] not in earlier}
    each = {key_of[p]: found.get(key_of[p], earlier.get(key_of[p])) for p in projects}
    if found:
        cache.put(key, {}, each)
    problems = [problem for p in projects for problem in each[key_of[p]]]
    if problems:
        print()
        for problem in problems:
            print(f'LAMPS: {problem}')
        raise Problem(f'{len(problems)} lamp binding(s) name no object of their map: no patch made')
    print(f'lamps: every binding of {len(projects)} made project(s) names its own model or light')


KEEPER_EDIT = 'artefact-keeper-1.json'
KEEPER_POOL = b'srp/artefact_keeper/pool'


def keeper_check(rows, base, world):
    """With Keeper's edit list in the manifest, every map project (every client_project but the
    lobby scenes') has its hidden pool (the anomaly `srp/artefact_keeper/pool`): the list names
    its maps one by one, so a map added later without it would give Keepers nothing there."""
    if ('edit', KEEPER_EDIT) not in {(kind, name) for kind, name, *_ in rows}:
        return
    projects = sorted(p for p in set(base.of) | set(world)
                      if p.startswith('projects/') and p.endswith('/client_project')
                      and not p.startswith('projects/lobby_scene/'))
    missing = [p for p in projects if p not in world or KEEPER_POOL not in world.data(p)]
    if missing:
        print()
        for project in missing:
            print(f'KEEPER: {project}: no {KEEPER_POOL.decode()} (add the map to {KEEPER_EDIT}: '
                  'srp-026g/scripts/artefact-keeper/make_edit.py)')
        raise Problem(f'{len(missing)} map project(s) without the Keeper pool: no patch made')
    print(f'keeper: every one of {len(projects)} map project(s) has its pool')


def walker_twin_problems(path, buf):
    """A project's temporary collision objects with same_walker_collision whose `walker_<name>` the
    project does not have: 0.26g's battery rule (0x3153B0) looks the twin up for each one a battery
    place attaches (0x3156EC, 0x31595C) and uses it unchecked, so a missing one crashes the client
    when the mode starts (London (v0.42)'s mode 3, 2026-11-21). The same rule reads each battery
    place's attached_objects/collision_geometries unchecked (0x3155FB-0x315610): a place without the
    list crashed London (v0.42)'s mode 3 too."""
    ce = own_content_edit()

    def value(item, key):
        try:
            data, _, _, kind, count = ce.rec(buf, ce.find(buf, key, item))
        except KeyError:
            return None
        return bytes(buf[data:data + count - 1]).decode('utf-8', 'replace') if kind == ce.T_STR else data != 0

    def items(path):
        try:
            return ce.children(buf, ce.find(buf, path))
        except KeyError:
            return []

    problems = [f'{path}: battery place {value(place, "name")} has no attached_objects/collision_geometries'
                for place in items('server_objects/victory_item_spawn_points')
                if value(place, 'attached_objects/collision_geometries') is None]
    entries = [(value(item, 'name'), value(item, 'same_walker_collision')) for item in items('temporary_collision_objects')]
    names = {name for name, _ in entries}
    return problems + [f'{path}: {name} has same_walker_collision and the project no walker_{name}'
                       for name, twin in entries if twin is True and f'walker_{name}' not in names]


def walker_check(base, world, cache):
    """Every map project, as players will have it (made by a row or as the base has it), gives each
    temporary collision object with same_walker_collision its `walker_<name>` twin, and each battery
    place its attached_objects/collision_geometries (walker_twin_problems). A project's problems are
    cached by its path and bytes and the check's source."""
    projects = sorted(p for p in set(base.of) | set(world)
                      if p.startswith('projects/') and p.endswith('/client_project'))
    key = Cache.key('the walker twins of projects', code_digest(walker_twin_problems),
                    files_digest([Path(own_content_edit().__file__)]), sys.version)
    earlier = (cache.get(key) or ({}, {}))[1]
    digests, files = base.digests([p for p in projects if p not in world])
    key_of = {p: f'{world.sha256(p) if p in world else digests[p]} {p}' for p in projects}
    todo = [p for p in projects if key_of[p] not in earlier]
    files.update(base.game.get(base.archives, [p for p in todo if p not in world and p not in files]))
    files.update({p: world.data(p) for p in todo if p in world})
    found = {key_of[p]: walker_twin_problems(p, bytearray(files[p])) for p in todo}
    each = {key_of[p]: found.get(key_of[p], earlier.get(key_of[p])) for p in projects}
    if found:
        cache.put(key, {}, each)
    problems = [problem for p in projects for problem in each[key_of[p]]]
    if problems:
        print()
        for problem in problems:
            print(f'WALKER: {problem}')
        raise Problem(f'{len(problems)} battery place(s) or walker twin(s) missing '
                      '(the battery rule would crash the client): no patch made')
    print(f'walker twins: every same_walker_collision object and battery place of {len(projects)} project(s) '
          'has what the battery rule reads')


# The longest project name (a mode config's match_making_params/map_name) the client loads: it cuts
# a longer one to 31 characters, finds no project and crashes loading the map (create_game_objects
# 0x22C11E, a null project config; Training Camp's v010b_cs_training_camp/day_teams, 2026-11-22).
PROJECT_NAME_MAX = 31


def project_name_problems(path, buf):
    """The mode config's project name, when it is longer than the client takes."""
    ce = own_content_edit()
    try:
        data, _, _, kind, count = ce.rec(buf, ce.find(buf, 'mode/match_making_params/map_name'))
    except KeyError:
        return []
    name = bytes(buf[data:data + count - 1]).decode('utf-8', 'replace') if kind == ce.T_STR else ''
    return [f'{path}: map_name {name} is {len(name)} characters, over {PROJECT_NAME_MAX}: the client cuts it '
            'and crashes loading the map'] if len(name) > PROJECT_NAME_MAX else []


def project_name_check(base, world, cache):
    """Every mode config, as players will have it, names a project the client can load
    (project_name_problems). A config's problems are cached by its path and bytes and the check's
    source."""
    configs = sorted(p for p in set(base.of) | set(world)
                     if p.startswith('gameplay/game_modes/') and p.endswith('.options'))
    key = Cache.key('the project names of mode configs', code_digest(project_name_problems),
                    files_digest([Path(own_content_edit().__file__)]), sys.version)
    earlier = (cache.get(key) or ({}, {}))[1]
    digests, files = base.digests([p for p in configs if p not in world])
    key_of = {p: f'{world.sha256(p) if p in world else digests[p]} {p}' for p in configs}
    todo = [p for p in configs if key_of[p] not in earlier]
    files.update(base.game.get(base.archives, [p for p in todo if p not in world and p not in files]))
    files.update({p: world.data(p) for p in todo if p in world})
    found = {key_of[p]: project_name_problems(p, bytearray(files[p])) for p in todo}
    each = {key_of[p]: found.get(key_of[p], earlier.get(key_of[p])) for p in configs}
    if found:
        cache.put(key, {}, each)
    problems = [problem for p in configs for problem in each[key_of[p]]]
    if problems:
        print()
        for problem in problems:
            print(f'PROJECT NAME: {problem}')
        raise Problem(f'{len(problems)} mode config(s) name a project longer than the client loads: no patch made')
    print(f'project names: every one of {len(configs)} mode config(s) names a project of at most '
          f'{PROJECT_NAME_MAX} characters')


GRASS_MAX = 120_000


def grass_instances(path, data):
    """A .veg layer's instances: its positions chunk (0x58), 12 bytes each."""
    at = 0
    while at + 8 <= len(data):
        tag, size = int.from_bytes(data[at:at + 4], 'little'), int.from_bytes(data[at + 4:at + 8], 'little')
        if tag == 0x58:
            return size // 12
        at += 8 + size
    raise Problem(f'{path}: a vegetation layer without positions')


def grass_check(base, world, cache):
    """Every map's grass, as players will have it, is at most GRASS_MAX instances: 0.26g's grass
    cook loads all the .veg layers of a project folder at once with 16 bytes of stack an instance
    on a 2 MiB thread (grass_cook::on_layers_loaded 0x12A820, exe patch 128), and about 130,000
    overflow it whatever the settings (stalker-maps.md). Every port's maps pass here, made by a
    pack or a base row; resource-porter's importer thins to the same cap. A layer's count is
    cached by its bytes."""
    paths = sorted(p for p in set(base.of) | set(world)
                   if p.startswith('projects/') and p.endswith('.veg') and '/vegetation/' in p)
    key = Cache.key('the grass of layers', code_digest(grass_instances))
    earlier = (cache.get(key) or ({}, {}))[1]
    digests, files = base.digests([p for p in paths if p not in world])
    digests.update({p: world.sha256(p) for p in paths if p in world})
    todo = [p for p in paths if digests[p] not in earlier]
    files.update(base.game.get(base.archives, [p for p in todo if p not in world and p not in files]))
    count = {digests[p]: grass_instances(p, world.data(p) if p in world else files[p]) for p in todo}
    if count:
        cache.put(key, {}, earlier | count)
    count = earlier | count
    maps = {}
    for path in paths:
        folder = path.rsplit('/vegetation/', 1)[0]
        maps[folder] = maps.get(folder, 0) + count[digests[path]]
    over = {folder: n for folder, n in maps.items() if n > GRASS_MAX}
    if over:
        print()
        for folder, n in sorted(over.items()):
            print(f'GRASS: {folder}: {n} instances, over {GRASS_MAX}')
        raise Problem(f'{len(over)} map(s) with more grass than the grass cook holds: no patch made')
    folder, most = max(maps.items(), key=lambda kv: kv[1], default=('none', 0))
    print(f'grass: each of {len(maps)} map folder(s) at most {GRASS_MAX} instances (most: {folder}, {most})')


def pierce_check(rows, tree, base, world, cache):
    """Every pierce line (an add_entry with `pierce_line`, content_edit.pierce_lines) is what the final
    files make: a row after the list's that retunes a weapon's or a round's piercing, a material's
    armour or what fires what, or adds a gun or a round, leaves a line stale or missing (and a
    weapon the list's _inputs does not declare, a line computed without it), and is a Problem. Its
    findings are cached by the lines and the bytes of every file it reads."""
    ce = own_content_edit()
    lines = [(name, entry) for kind, name, source, _, _ in rows
             if kind == 'edit' and source == '-' and name.endswith('.json')
             for entry in json.loads((edits_dir(tree) / name).read_text(encoding='utf-8')).get('add_entry', [])
             if 'pierce_line' in entry]
    if not lines:
        print('pierce lines: none listed')
        return
    known = set(base.of) | set(world)
    # Every item config the dictionary can pair, read once.
    needed = sorted(p for p in known if p.startswith('gameplay/items/') and p.endswith('.options'))
    needed += [ce.GAME_MATERIALS, ce.DICTIONARY]
    digests, _ = base.digests([p for p in needed if p not in world], keep=False)
    modules = sorted(p for p in Path(ce.__file__).parent.glob('*.py') if not p.name.startswith('test_'))
    key = Cache.key('pierce lines', code_digest(pierce_check), files_digest(modules), sys.version, lines,
                    [(p, world.sha256(p) if p in world else digests[p]) for p in needed])
    found = cache.get(key)
    if found:
        problems, checked = found[1]
        report_pierce(problems, checked)
        return
    files = base.game.get(base.archives, [p for p in needed if p not in world])
    files.update({p: world.data(p) for p in needed if p in world})
    files = {p: bytearray(b) for p, b in files.items()}
    problems, checked = [], 0
    for name, entry in lines:
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
    cache.put(key, {}, [problems, checked])
    report_pierce(problems, checked)


def report_pierce(problems, checked):
    if problems:
        for problem in problems:
            print(f'PIERCE: {problem}')
        raise Problem(f'{len(problems)} pierce line(s) the final data does not make: no patch made')
    print(f'pierce lines: each of {checked} round(s) what its final data makes')


def clothing_check(base, world):
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
    paths = sorted(p for p in set(base.of) | set(world) if check.is_boot(p))
    if not paths:
        return  # Small release-builder fixtures can contain no equipment.
    game, stock = base.game, [base.archives[0]]
    native_paths = sorted(p for p in game.files(stock) if check.is_boot(p))
    native = game.get(stock, native_paths)
    native.update(game.get(stock, check.model_paths(native)))
    effective = game.get(base.archives, [p for p in paths if p not in world])
    effective.update({p: world.data(p) for p in paths if p in world})
    models = check.model_paths(effective)
    effective.update(game.get(base.archives, [p for p in models if p not in world]))
    effective.update({p: world.data(p) for p in models if p in world})
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


DIGESTS = 'digests.tsv'


def chain_layers(live, live_sha_of, chain, game, cache, reader):
    """Layers of players' files (`live`, `live_sha_of` their sha256s by name) with the earlier
    builds' patches (`chain`: [(patch, its sha256)]) over them. A patch's files' sha256s come from
    DIGESTS beside it (build-content writes it with the patch: its first line names the patch's
    sha256) or, without one, are read once and then known from the cache by the patch's sha256."""
    layers = Layers(game, cache, reader, live + [patch for patch, _ in chain],
                    live_sha_of | {patch.name: sha for patch, sha in chain})
    for patch, sha in chain:
        listing = patch.parent / DIGESTS
        lines = listing.read_text().splitlines() if listing.is_file() else []
        if lines and lines[0] == f'# {patch.name} {sha}':
            layers.known[patch.name] = dict(reversed(line.split('\t', 1)) for line in lines[1:])
    return layers


def delta(chain, players, world, changed):
    """{path: (its bytes of path, why)} of what a patch over `chain` (chain_layers: the earlier
    builds' patches over players' files) holds so that the game sees every file as this build's own
    patch over players' files would show it: a file this build packs that the chain gives
    otherwise, and a file the chain gives that this build does not pack, as players have it. A
    patch adds or replaces files and cannot take one away: a file the chain gives that neither
    players have nor this build packs is a Problem."""
    names = {Path(patch).name for patch in chain.archives[len(players.archives):]}
    given = sorted(p for p, name in chain.of.items() if name in names)
    theirs, _ = chain.digests(given, keep=False)
    packed = set(changed)
    back = [p for p in given if p not in packed]
    gone = [p for p in back if p not in players.of]
    if gone:
        raise Problem(f'{gone[0]} (and {len(gone) - 1} more): the earlier build\'s patch adds it, players do not '
                      'have it and this build does not make it; a patch cannot take a file away: build it whole '
                      '(without --delta-from)')
    live_digests, _ = players.digests(back, keep=False)
    out = {}
    for path in given:
        have = theirs[path]
        if path in packed:
            if have != world.sha256(path):
                out[path] = (world.data, 'made: the earlier build gives it otherwise')
        elif have != live_digests[path]:
            out[path] = (None, 'as players have it: the earlier build changed it, this one does not')
    for path in packed - set(given):
        out[path] = (world.data, 'made')
    had = players.game.get(players.archives, [p for p, (data_of, _) in out.items() if data_of is None])
    return {p: (data_of or had.__getitem__, why) for p, (data_of, why) in out.items()}


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
    parser.add_argument('--quick', action='store_true')
    parser.add_argument('--delta-over', nargs='*')
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
        # srp-game-data as the cache knows it: the files' sha256s it reads are kept by it (Layers),
        # with each archive's sha256 (build-content checked each). build-content keys the server
        # game data by it.
        reader = game_data_id(args.bin, args.bin_source)
        (work / 'game-data-id').write_text(reader + '\n')
        live_id = [reader, ('resources.db', args.stock_sha256)] + offered
        print(f'the base: the stock resources.db and {len(base_rows)} published patch(es): '
              + ' '.join(n.removeprefix('resources.db.') for n in base_names[1:]))
        if any(kind == 'movie' for kind, *_ in rows) and not os.environ.get('SKHID_TEST_BUILD_MOVIE') and \
                not (args.tools and args.tools.is_file()):
            raise Problem(f'the movies need a tools.json naming java and Flex (tools/actionscript/README.md): '
                          f'{args.tools} is not a file (SKHID_FLASH_TOOLS names another)')
        world = World(cache)
        print('the movies, from source:')
        with timed('the movies'):
            drawn = build_movies(rows, args.tree, archive_of, sha_of, game, args.tools, work, world, cache)
        with timed('the packs'):
            compose_packs(rows, base, [(n, sha_of[n]) for n in base_names], args.vostok_port, args.packs, work,
                          world, cache, drawn)
        with timed('the base files'):
            base_layers = Layers(game, cache, reader, base, sha_of)
        print('the edit lists, in order:')
        try:
            with timed('the edit lists'):
                apply_edits(rows, args.tree, base_layers, work, world, cache)

            # The value types every changed file keeps (content_type_check.py):
            # a key that exists in the base/live version keeps its type, a new key
            # uses a type 0.26g files already use at that path or is allowlisted.
            with timed('the value types'):
                type_check(rows, args.tree, base_layers, world, cache, whole=not args.quick)
            with timed('the lamps'):
                lamp_check(world, cache)
            with timed('the pierce lines'):
                pierce_check(rows, args.tree, base_layers, world, cache)
            with timed('the Keeper pool'):
                keeper_check(rows, base_layers, world)
            with timed('the grass'):
                grass_check(base_layers, world, cache)
            with timed('the walker twins'):
                walker_check(base_layers, world, cache)
            with timed('the project names'):
                project_name_check(base_layers, world, cache)
            with timed('the clothing fit'):
                clothing_check(base_layers, world)

            # What the sources make, against what players have (build-content compares world/ again,
            # with the patch, but not with --quick). A file's status is cached by its path and bytes,
            # for the same live files.
            with timed('the made files, written'):
                for path in [] if args.quick else world:
                    target = work / 'world' / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(world.data(path))
            with timed('the comparison with players\' files'):
                compared_key = Cache.key('players\' files', live_id)
                earlier = (cache.get(compared_key) or ({}, {}))[1]
                key_of = {path: f'{world.sha256(path)} {path}' for path in world}
                todo = work / 'to-compare'
                for path in (p for p in world if key_of[p] not in earlier):
                    (todo / path).parent.mkdir(parents=True, exist_ok=True)
                    (todo / path).write_bytes(world.data(path))
                found = game.compare(live, todo) if todo.is_dir() else {}
                if todo.is_dir():
                    shutil.rmtree(todo)
                compared = {p: found[p] if p in found else tuple(earlier[key_of[p]]) for p in world}
                if found:
                    cache.put(compared_key, {}, {key_of[p]: compared[p] for p in world})
                players = Layers(game, cache, reader, live,
                                 {'resources.db': args.stock_sha256} | dict(offered))
            with timed('the stats checks'):
                stats_checks(rows, base_layers, players, world, args.tree, compared, cache)
            players.save()
        finally:
            base_layers.save()
        problems = []
        with timed('the live files'):
            live_files = players.of
            unmade = sorted(p for p, name in live_files.items() if name not in base_names and p not in world)
            left = left_by_earlier_packs(rows, args.packs, unmade, base_layers.of, live, game, cache, live_id)
        for path in unmade:
            if path not in left:
                problems.append(f'{path}: live from {live_files[path]}, which is no base row, and made by no row '
                                'of the manifest nor left by an earlier version of a pack row (<packs>/<id>-<n>): '
                                'what made it? (a patch put together by hand, a row taken out, or an earlier '
                                'pack\'s folder missing)')
        changed = sorted(p for p, (status, _) in compared.items() if status != 'same')
        lines = []
        print()
        print(f'every file the movies, packs and edits make ({len(world)}), against what players have '
              f'(the stock resources.db and the {len(offered)} offered patches):')
        for path in sorted(world):
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
        if args.delta_over is not None:
            with timed('the delta'):
                chain = chain_layers(live, {'resources.db': args.stock_sha256} | dict(offered),
                                     [(Path(p), sha) for p, _, sha in (a.rpartition('=') for a in args.delta_over)],
                                     game, cache, reader)
                packed = delta(chain, players, world, changed)
            players.save()
            chain.save()
            (work / 'delta.tsv').write_text(''.join(f'{path}\t{why}\n' for path, (_, why) in sorted(packed.items())))
            if not packed:
                print('nothing to ship over the earlier build: the game sees every file through it as this build '
                      'makes it')
                return
        else:
            packed = {path: (world.data, 'made') for path in changed}
            if not packed:
                print('nothing to ship: players have every file as the manifest makes it')
                return
        patch = work / 'patch'
        size, digests = 0, []
        for path, (data_of, _) in sorted(packed.items()):
            target = patch / path
            target.parent.mkdir(parents=True, exist_ok=True)
            data = data_of(path)
            target.write_bytes(data)
            size += len(data)
            digests.append(f'{world.sha256(path) if data_of == world.data else sha256(data)}\t{path}\n')
        # Each file's sha256, for a later --delta-over this patch (build-content puts the patch's
        # sha256 above them, DIGESTS).
        (work / DIGESTS).write_text(''.join(digests))
        print(f'the next patch: {len(packed)} file(s), {size} bytes')
    except Problem as problem:
        sys.exit(f'build-content: {problem}')


if __name__ == '__main__':
    main()
