#!/usr/bin/env python3
"""Build a movie's ActionScript source tree (srp-026g/flash/<movie>) over its release movie.

An ordinary build with a cache: every file of the tree that is not the
baseline's (movie.json) is compiled whole, with the compiler settings the
release is rebuilt with (the tree's reconstruction manifest), and goes in as
compiled. A file that is the baseline's is not compiled: the release movie's
code for it is what it compiles to (flash_tree.py full proves it), so its ABC
is kept. Graphics, fonts and sounds come from the release movie.

- A changed file of the release: its class is compiled into its own ABC.
- A new file: a new class, compiled into the ABC of the class of the release
  srp.json names for it (`new_classes`, in that order).
- A file of the release gone from the tree: its class is taken out of the
  movie (it must be the last script of its ABC, and nothing may name it).

The compilers compile one class at a time, against the movie compiled so far.
Pass 1 compiles every changed and new class, a class that does not compile yet
(it uses a member another one adds or changes) after the others, until every
declaration of the tree is in the movie. Pass 2 compiles each class again,
against that movie, into its ABC as the build left it. The result does not
depend on the order of the classes.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import zlib

import movie_source as movie
import source_patches as patches

CLASS_NAME = re.compile(r'[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*')
REPO = Path(__file__).resolve().parents[2]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def full_movie(original, code, output):
    movie.transplant(original, code, output)
    raw = output.read_bytes()
    if original.read_bytes()[:3] == b'CWS':
        output.write_bytes(b'CWS' + raw[3:8] + zlib.compress(raw[8:], 9))


def verify_container(original, built):
    """The movie's header and every non-code tag are the release's; only ABC blocks change."""
    old_header, old_tags = movie.split_movie(original)
    new_header, new_tags = movie.split_movie(built)
    if old_header[:4] + old_header[8:] != new_header[:4] + new_header[8:] or len(old_tags) != len(new_tags):
        raise ValueError('The build changed the movie header or tag count')
    for (old_kind, old), (new_kind, new) in zip(old_tags, new_tags):
        if old_kind != new_kind:
            raise ValueError('The build changed tag ordering or kinds')
        if old_kind not in (72, 82):
            if old != new:
                raise ValueError('The build changed non-code movie data')
        elif old_kind == 82:
            def attributes(raw):
                word, = struct.unpack_from('<H', raw)
                payload = raw[6 if word & 63 == 63 else 2:]
                return payload[:payload.index(b'\0', 4) + 1]
            if attributes(old) != attributes(new):
                raise ValueError('The build changed DoABC flags or names')


def abc_blocks(path):
    """The ABC tags of a code movie, in order (raw tag bytes)."""
    return [raw for kind, raw in movie.split_movie(path)[1] if kind in (72, 82)]


def with_block(base, block, raw, output):
    """`base` (a code movie) with its ABC block number `block` replaced by the tag `raw`."""
    header, tags = movie.split_movie(base)
    out, number = [], 0
    for kind, tag in tags:
        if kind in (72, 82):
            tag = raw if number == block else tag
            number += 1
        out.append((kind, tag))
    output.write_bytes(movie.join_movie(header, out))


def changed_block(before, after):
    """The one ABC block that differs between two code movies (None if none does)."""
    old, new = abc_blocks(before), abc_blocks(after)
    if len(old) != len(new):
        raise ValueError('The ABC block count changed')
    differ = [i for i, (a, b) in enumerate(zip(old, new)) if a != b]
    if len(differ) > 1:
        raise ValueError(f'More than one ABC block changed: {differ}')
    return differ[0] if differ else None


def duplicate_functions(source):
    """FFDec's compiler keeps the last of two methods of one name: refuse that."""
    names = {}
    for name in re.findall(r'\bfunction\s+(?!get\b|set\b)(\w+)\s*\(', source):
        names[name] = names.get(name, 0) + 1
    return sorted(name for name, count in names.items() if count > 1)


def stub_source(source):
    """A new class's declaration alone (its package, the imports its header
    names, an empty body): every new class is added so first, then compiled
    whole, so that new classes may name each other (FFDec compiles a class
    only against types the movie has)."""
    text = source.decode('utf-8')
    package = re.search(r'^\s*package\b([^{]*)\{', text, re.M)
    header = re.search(r'^\s*(public\s+(?:(?:final|dynamic)\s+)*class\s+([\w$]+)([^{]*))\{', text, re.M)
    if package is None or header is None:
        raise ValueError('A new class must be one public class in a package block')
    named = set(re.findall(r'[\w$]+', header.group(3)))
    imports = [m.group(0).strip() for m in re.finditer(r'^\s*import\s+([\w$.]+)\s*;', text, re.M)
               if m.group(1).split('.')[-1] in named]
    body = ''.join(f'    {line}\n' for line in imports)
    return (f'package {package.group(1).strip()}\n{{\n{body}    {" ".join(header.group(1).split())}\n'
            '    {\n    }\n}\n').replace('package \n', 'package\n').encode('utf-8')


def compiler_plans(reconstruction, spec):
    """Each class's compiler settings, from the reconstruction manifest the tree was exported with."""
    raw = reconstruction.read_bytes()
    if sha(raw) != spec['reconstruction_sha256']:
        raise ValueError('The tree was exported with another reconstruction manifest')
    return {entry['class']: (entry.get('compiler', 'flex-strict'), patches.composition_plan(entry),
                             patches.reservation_plan(entry))
            for entry in json.loads(raw)['classes']}


def tree_inputs(movie_dir, plans):
    """The tree's removed, new and changed classes against its baseline, in build order."""
    spec = json.loads((movie_dir / 'movie.json').read_bytes())
    srp_path = movie_dir / 'srp.json'
    srp = json.loads(srp_path.read_bytes()) if srp_path.exists() else {}
    if set(srp) - {'description', 'new_classes'}:
        raise ValueError('srp.json holds "new_classes" (and a "description") only')
    anchors = {}
    for entry in srp.get('new_classes', []):
        if not CLASS_NAME.fullmatch(entry.get('class', '')) or entry['class'] in anchors \
                or set(entry) != {'class', 'abc_with'}:
            raise ValueError(f'srp.json: invalid or repeated entry {entry}')
        anchors[entry['class']] = entry['abc_with']
    root = movie_dir / 'src'
    files = {path.relative_to(root).as_posix(): path.read_bytes() for path in sorted(root.rglob('*')) if path.is_file()}
    baseline = spec['sources']
    inputs = []
    # A class of the release gone from the tree is taken out of the movie.
    for relative in sorted(set(baseline) - set(files)):
        cls = relative[:-3].replace('/', '.')
        word = re.compile(rf'(?<![\w$]){re.escape(cls.split(".")[-1])}(?![\w$])')
        users = sorted(p for p, data in files.items() if word.search(data.decode('utf-8')))
        if users:
            raise ValueError(f'{cls} is gone from the tree, but the tree still names it: ' + ', '.join(users))
        inputs.append({'class': cls, 'kind': 'removed'})
    new = {relative[:-3].replace('/', '.'): relative for relative in set(files) - set(baseline)}
    unnamed = sorted(new.keys() - anchors.keys())
    if unnamed:
        raise ValueError(f'{new[unnamed[0]]} is new: name {unnamed[0]} in srp.json\'s new_classes, '
                         'with abc_with, the class of the release whose ABC takes it')
    for cls, anchor in anchors.items():
        if cls not in new:
            raise ValueError(f'srp.json names {cls}, which is not a new file of the tree')
        if anchor.replace('.', '/') + '.as' not in files or anchor in new:
            raise ValueError(f'srp.json: {cls}\'s abc_with must be a class of the release the tree has')
        twice = duplicate_functions(files[new[cls]].decode('utf-8'))
        if twice:
            raise ValueError(f'{cls} defines these more than once (FFDec keeps the last): ' + ', '.join(twice))
        inputs.append({'class': cls, 'kind': 'new', 'source': files[new[cls]], 'source_name': Path(new[cls]).name,
                       'anchor': anchor, 'compiler': 'ffdec', 'composition': None, 'reservations': None})
    for relative, data in files.items():
        if relative in baseline and sha(data) != baseline[relative]:
            cls = relative[:-3].replace('/', '.')
            compiler, composition, reservations = plans.get(cls, ('flex-strict', None, None))
            inputs.append({'class': cls, 'kind': 'changed', 'source': data, 'compiler': compiler,
                           'composition': composition, 'reservations': reservations})
    return spec, inputs


def build(tools_path, movie_dir, original, output, jobs=4, log=print):
    """Build the tree into `output` (a new directory); the movie is output/feature.swf."""
    output = output.resolve()
    spec = json.loads((movie_dir / 'movie.json').read_bytes())
    if sha(original.read_bytes()) != spec['movie_sha256']:
        raise ValueError(f"Not the pinned original of {spec['movie']}")
    spec, inputs = tree_inputs(movie_dir, compiler_plans(REPO / spec['reconstruction'], spec))
    if not inputs:
        raise ValueError('The tree is the baseline: nothing to build')
    output.mkdir(parents=True, exist_ok=False)
    tools = movie.Toolchain(tools_path, output / 'bridge')
    reference = output / 'original-code.swf'
    movie.code_movie(original, reference)
    report = {'schema': 'tree', 'release': spec['release'], 'movie_sha256': spec['movie_sha256'],
              'classes': [{'class': item['class'], 'kind': item['kind']} for item in inputs], 'complete': False}
    movie.write_json(output / 'report.json', report)
    folders = {}
    for number, item in enumerate(inputs):
        folder = folders[item['class']] = output / f'class-{number:04d}'
        folder.mkdir()
        if item['kind'] == 'removed':
            continue
        (folder / 'source.as').write_bytes(item['source'])
        if item['composition'] is not None:
            (folder / 'compiler-composition.tsv').write_bytes(item['composition'])
        if item['reservations'] is not None:
            (folder / 'method-reservations.tsv').write_bytes(item['reservations'])

    def run(folder, cls, *arguments, log_name):
        if tools.run(*arguments, log=folder / log_name).returncode:
            raise ValueError(f'{cls}: {arguments[0]} failed; see {folder / log_name}')

    def compile_link(item, context, name):
        """Compile the class's source into `context` (a code movie) and link it: the linked movie."""
        cls, folder = item['class'], folders[item['class']]
        composition = [folder / 'compiler-composition.tsv'] if item['composition'] is not None else []
        reserved = item['reservations'] is not None
        compiled = folder / f'{name}-{"unreserved" if reserved else "compiled"}.swf'
        run(folder, cls, 'compile', context, compiled, cls, folder / 'source.as', folder / f'{name}-snapshot',
            item['compiler'], tools.config.get('flex', ''), *composition, log_name=f'{name}-compile.log')
        if reserved:
            run(folder, cls, 'reserve', compiled, folder / f'{name}-compiled.swf', cls,
                folder / 'method-reservations.tsv', folder / f'{name}-reserved-snapshot', log_name=f'{name}-reserve.log')
        linked = folder / f'{name}-linked.swf'
        run(folder, cls, 'link-feature', context, folder / f'{name}-compiled.swf', linked, cls,
            log_name=f'{name}-link.log')
        return linked

    # Classes gone from the tree, then every new class as its declaration
    # alone (`stub_source`): each new class is a type of the movie before any
    # is compiled whole.
    current = reference
    for item in inputs:
        cls, folder = item['class'], folders[item['class']]
        if item['kind'] == 'removed':
            run(folder, cls, 'remove-script', current, folder / 'removed.swf', cls, folder / 'removed.tsv',
                folder / 'removed-snapshot', log_name='remove.log')
            current = folder / 'removed.swf'
            log(f'{cls}: taken out of the movie')
        elif item['kind'] == 'new':
            stub = folder / 'stub' / item['source_name']
            stub.parent.mkdir()
            stub.write_bytes(stub_source(item['source']))
            run(folder, cls, 'add-script', current, folder / 'stub-compiled.swf', item['anchor'],
                stub, folder / 'stub-snapshot', log_name='stub-compile.log')
            run(folder, cls, 'link-feature', current, folder / 'stub-compiled.swf', folder / 'stub-linked.swf',
                item['anchor'], log_name='stub-link.log')
            current = folder / 'stub-linked.swf'
    declared = current
    compiled = [item for item in inputs if item['kind'] != 'removed']

    # Pass 1: every class compiled once, a class that does not compile yet
    # after the others, until all of them are in the movie.
    pending, errors = list(compiled), {}
    while pending:
        done = []
        for item in pending:
            before, item['attempts'] = current, item.get('attempts', 0) + 1
            try:
                current = compile_link(item, current, f'pass1-{item["attempts"]}')
            except (ValueError, OSError, subprocess.TimeoutExpired) as error:
                errors[item['class']] = error
                continue
            item['block'] = changed_block(before, current)
            done.append(item)
        if not done:
            raise ValueError('These classes do not compile against the rest of the tree: '
                             + '; '.join(f'{item["class"]}: {errors[item["class"]]}' for item in pending))
        pending = [item for item in pending if item not in done]
        if pending:
            log(f'Pass 1: {len(compiled) - len(pending)} of {len(compiled)} classes compiled; again: '
                + ', '.join(item['class'] for item in pending))
    everything = current
    log(f'Pass 1: {len(compiled)} classes compiled')

    # Pass 2: each class compiled again against every declaration of the tree
    # (pass 1's movie), into its ABC as the build left it: the classes of one
    # ABC one after another (in build order), those of different ABCs at once.
    groups = {}
    for item in compiled:
        if item['block'] is not None:
            groups.setdefault(item['block'], []).append(item)
    start = abc_blocks(declared)

    def rebuild(block, items):
        raw = start[block]
        for item in items:
            context = folders[item['class']] / 'pass2-context.swf'
            with_block(everything, block, raw, context)
            linked = compile_link(item, context, 'pass2')
            if changed_block(context, linked) not in (None, block):
                raise ValueError(f'{item["class"]}: pass 2 changed another ABC block than pass 1')
            raw = abc_blocks(linked)[block]
        return block, raw

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        results = list(pool.map(lambda group: rebuild(*group), groups.items()))
    final = output / 'built-code.swf'
    shutil.copyfile(declared, final)
    for block, raw in results:
        with_block(final, block, raw, final)
    log(f'Pass 2: {len(compiled)} classes compiled against every declaration of the tree')

    pending = output / 'feature.tmp'
    full_movie(original, final, pending)
    verify_container(original, pending)
    pending.rename(output / 'feature.swf')
    report.update(complete=True, feature_movie_sha256=movie.digest(output / 'feature.swf'))
    movie.write_json(output / 'report.json', report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--tools', required=True, type=Path)
    parser.add_argument('--jobs', type=int, default=4, help='ABC blocks compiled at once in pass 2 (1-8)')
    parser.add_argument('tree', type=Path, help='a source tree (srp-026g/flash/<movie>)')
    parser.add_argument('original', type=Path, help='the release movie the tree pins')
    parser.add_argument('output', type=Path, help='a new directory')
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        raise ValueError('--jobs must be between 1 and 8')
    build(args.tools, args.tree, args.original, args.output, args.jobs)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        sys.exit(str(error))
