#!/usr/bin/env python3
"""The game's Flash movies as ActionScript source trees (srp-026g/flash/<movie>/).

    flash_tree.py original <movie dir> <archive> <out.swf>
        the movie the tree's graphics come from, cut from the archive
        movie.json names (resources.db or a resources.db.patch_<n>)
    flash_tree.py [--tools T] build <movie dir> <original.swf> <out.swf>
        compile the whole tree (src/) in one Flex run and put the code into
        the original movie in place of all of its code

A tree is the movie's complete ActionScript; movie.json names the original
movie, whose graphics, fonts, sounds and symbol bindings (everything but the
code) the build keeps. A bitmap the original lacks is bitmaps/<class>.png
beside src/: the build adds it, bound to <class> (a BitmapData of the
tree, as the original's bitmaps are). The build is one `compc` run of Apache Flex 4.16.1
over src/ (strict, optimized, no debug information, the original's SWF
version), against the Flash Player API (playerglobal.swc) only: every class
the movie uses is in the tree.
"""
import argparse
import fcntl
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import swf

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
# compc's output depends on where the source lies (not on what it says), so
# every tree is compiled from one place: the same tree gives the same bytes
# from any checkout.
BUILD_ROOT = Path('/tmp/srp-flash-build')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(movie_dir):
    spec = json.loads((movie_dir / 'movie.json').read_text())
    if spec.get('schema') != 2:
        raise ValueError(f'{movie_dir}: not a movie source tree')
    return spec


def original(movie_dir, archive, output):
    spec = load(movie_dir)
    if archive.name != spec['archive']:
        raise ValueError(f"The original is in {spec['archive']}, not {archive.name}")
    if 'archive_offset' not in spec:
        raise ValueError(f"movie.json gives no offset: take {spec['movie']} out of {spec['archive']} with srp-game-data")
    with archive.open('rb') as stream:
        stream.seek(spec['archive_offset'])
        data = stream.read(spec['movie_size'])
    if sha(data) != spec['movie_sha256']:
        raise ValueError(f'{archive} does not hold the movie at offset {spec["archive_offset"]}')
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('xb') as stream:
        stream.write(data)


def compile_tree(source, tools, version, work):
    """Every class of `source`, compiled by one compc run: the DoABC tags of its library."""
    config = json.loads(tools.read_text())
    flex = Path(config['flex'])
    output = work / 'library'
    command = [config['java'], '-Xmx3g', '-Djava.awt.headless=true', '-jar', str(flex / 'lib' / 'compc.jar'),
               '+flexlib=' + str(flex / 'frameworks'), '+playerglobalHome=' + str(flex / 'frameworks/libs/player'),
               '-library-path=', '-external-library-path=' + str(flex / 'frameworks/libs/player/32.0/playerglobal.swc'),
               f'-swf-version={version}', '-strict=true', '-debug=false', '-optimize=true',
               '-source-path=' + str(source), '-include-sources=' + str(source),
               '-directory=true', '-output=' + str(output)]
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    (work / 'compc.log').write_text(result.stdout)
    if result.returncode:
        errors = [line for line in result.stdout.splitlines() if 'Error' in line]
        raise ValueError('The tree does not compile:\n' + '\n'.join(errors[:20] or result.stdout.splitlines()[-20:]))
    _, tags = swf.split((output / 'library.swf').read_bytes())
    # compc's library has a generated root class of its own (SymbolClass 0).
    roots = {name for cid, name in swf.symbol_classes(tags) if cid == 0}
    return [(kind, raw) for kind, raw in tags if kind == swf.DO_ABC and swf.do_abc(raw)[1] not in roots]


def bitmaps(movie_dir):
    """[(class name, width, height, RGBA)] of the tree's bitmaps/<class>.png, by name."""
    folder = movie_dir / 'bitmaps'
    if not folder.is_dir():
        return []
    out = []
    for path in sorted(folder.iterdir()):
        if path.suffix != '.png' or not path.stem.isidentifier():
            raise ValueError(f'{path}: a bitmap is <class name>.png')
        out.append((path.stem, *swf.png(path.read_bytes())))
    return out


def assemble(original_data, code, added=()):
    """The original movie with `code` (DoABC tags) in place of all of its code, where its code began.

    `added` (bitmaps()) are new bitmaps: each a DefineBitsLossless2 of a new character id before the
    code, bound to its class (a BitmapData of the tree) by one SymbolClass after it."""
    header, tags = swf.split(original_data)
    first = next(i for i, (kind, _) in enumerate(tags) if kind in (swf.DO_ABC, swf.DO_ABC_OLD))
    taken = swf.character_ids(tags)
    if {name for _, name in swf.symbol_classes(tags)} & {name for name, *_ in added}:
        raise ValueError('A bitmap of bitmaps/ is a class the original already binds')
    defines, bindings = [], []
    for i, (name, width, height, rgba) in enumerate(added):
        cid = max(taken, default=0) + 1 + i
        defines.append((swf.DEFINE_BITS_LOSSLESS2, swf.bitmap(cid, width, height, rgba)))
        bindings.append((cid, name))
    bound = [(swf.SYMBOL_CLASS, swf.symbol_class(bindings))] if bindings else []
    kept = tags[:first] + defines + code + bound \
        + [t for t in tags[first:] if t[0] not in (swf.DO_ABC, swf.DO_ABC_OLD)]
    return swf.compressed(swf.join(header, kept), original_data)


def check(movie):
    """What the movie needs to load: every ABC block parses, and every class a
    SymbolClass tag binds is defined, once. Returns the number of classes."""
    _, tags = swf.split(movie)
    classes = []
    for kind, raw in tags:
        if kind == swf.DO_ABC:
            classes += swf.abc_definitions(swf.do_abc(raw)[2])[0]
    seen, twice = set(), set()
    for name in classes:
        (twice if name in seen else seen).add(name)
    if twice:
        raise ValueError('Classes defined more than once: ' + ', '.join(sorted(twice)[:10]))
    missing = sorted({name for _, name in swf.symbol_classes(tags)} - seen)
    if missing:
        raise ValueError('Classes the movie binds to symbols but the tree does not have: ' + ', '.join(missing))
    return len(classes)


def build(movie_dir, original_swf, tools, report=print):
    """The tree's movie (bytes)."""
    spec = load(movie_dir)
    data = original_swf.read_bytes()
    if sha(data) != spec['movie_sha256']:
        raise ValueError(f"Not the original of {spec['movie']} (sha256 {sha(data)})")
    place = BUILD_ROOT / movie_dir.resolve().name
    place.mkdir(parents=True, exist_ok=True)
    with (BUILD_ROOT / (place.name + '.lock')).open('w') as lock, \
            tempfile.TemporaryDirectory(prefix='srp-flash-') as work:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if (place / 'src').exists():
            shutil.rmtree(place / 'src')
        shutil.copytree(movie_dir / 'src', place / 'src')
        try:
            code = compile_tree(place / 'src', tools, swf.uncompressed(data)[3], Path(work))
        finally:
            shutil.rmtree(place / 'src')
    added = bitmaps(movie_dir)
    movie = assemble(data, code, added)
    report(f'{movie_dir.name}: {check(movie)} classes in {len(code)} ABC blocks over the original\'s graphics'
           + (f', {len(added)} bitmaps added' if added else ''))
    return movie


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--tools', type=Path, default=REPO / 'private' / 'actionscript' / 'tools.json',
                        help='JSON naming java and flex (Apache Flex 4.16.1); default %(default)s')
    commands = parser.add_subparsers(dest='command', required=True)
    cut = commands.add_parser('original')
    cut.add_argument('movie_dir', type=Path)
    cut.add_argument('archive', type=Path)
    cut.add_argument('output', type=Path)
    make = commands.add_parser('build')
    make.add_argument('movie_dir', type=Path)
    make.add_argument('original', type=Path)
    make.add_argument('output', type=Path)
    args = parser.parse_args()
    if args.command == 'original':
        original(args.movie_dir, args.archive, args.output)
        print(f'Wrote {args.output}')
    else:
        if args.output.exists():
            raise ValueError('Output already exists; choose a new path')
        data = build(args.movie_dir, args.original, args.tools)
        with args.output.open('xb') as stream:
            stream.write(data)
        print(f'Created {args.output}: sha256 {sha(data)}')
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
