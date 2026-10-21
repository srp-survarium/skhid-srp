#!/usr/bin/env python3
"""The game's Flash movies as committed ActionScript source trees (srp-026g/flash/<movie>/).

    flash_tree.py original <movie dir> <archive> <out.swf>
        the pinned original movie, cut from the archive the pin names
        (resources.db or a resources.db.patch_<n>), its sha256 checked
    flash_tree.py --tools T export <movie dir> <original.swf> --release R
        export the movie's complete source (FFDec, pinned), apply its
        reconstruction diffs, and write <movie dir>/src and movie.json
    flash_tree.py check <movie dir>
        is src/ the baseline (every file the hash movie.json pins)?
    flash_tree.py --tools T build <movie dir> <original.swf> <out.swf> [--work DIR]
        the movie of the tree as it is: every file that is not the baseline's
        is compiled whole; the others keep the original's compiled code
        (tools/actionscript/build_feature.py)
    flash_tree.py --tools T full <movie dir> <original.swf> <work dir> [--jobs N]
        compile EVERY script of src/ (the baseline tree), link and assemble
        them, and require the complete compressed movie to be byte-identical
        to the original

A tree holds the source that compiles to the release exactly: the FFDec export
with tools/actionscript/reconstruction's corrections applied, with LF line
ends (FFDec writes CRLF; nothing compiled depends on them: `full` proves it). movie.json pins the release (the original
movie's archive entry and sha256), the reconstruction manifest whose per-class
compiler settings the build uses, and the sha256 of every baseline source file.
The original movie is not committed: its non-code tags (graphics, fonts,
sounds) are not source, and the build takes them from the pinned original.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import movie_source as movie
import source_patches

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def lf(data):
    """A tree file's text: FFDec exports CRLF; the tree keeps LF, as every other source."""
    return data.replace(b'\r\n', b'\n')


def load(movie_dir):
    spec = json.loads((movie_dir / 'movie.json').read_text())
    if spec.get('schema') != 1 or not spec.get('sources'):
        raise ValueError(f'{movie_dir}: not a movie source tree')
    return spec


def pin(spec):
    path = HERE / 'releases' / (spec['release'] + '.json')
    return json.loads(path.read_text())


def tree(movie_dir):
    """Every source file of the tree, by its path relative to src/."""
    root = movie_dir / 'src'
    files = {}
    for path in sorted(root.rglob('*')):
        if path.is_file():
            if path.suffix != '.as':
                raise ValueError(f'Not ActionScript in a source tree: {path}')
            files[path.relative_to(root).as_posix()] = path.read_bytes()
    return files


def differences(movie_dir):
    """(changed, added, removed) relative paths against the pinned baseline."""
    spec, files = load(movie_dir), tree(movie_dir)
    baseline = spec['sources']
    changed = sorted(p for p in files if p in baseline and sha(files[p]) != baseline[p])
    added = sorted(p for p in files if p not in baseline)
    removed = sorted(p for p in baseline if p not in files)
    return changed, added, removed


def original(movie_dir, archive, output):
    spec = load(movie_dir)
    release = pin(spec)
    if archive.name != release['archive']:
        raise ValueError(f"The pinned original is in {release['archive']}, not {archive.name}")
    with archive.open('rb') as stream:
        stream.seek(release['archive_offset'])
        data = stream.read(release['movie_size'])
    if sha(data) != spec['movie_sha256']:
        raise ValueError(f'{archive} does not hold the pinned movie at offset {release["archive_offset"]}')
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('xb') as stream:
        stream.write(data)


def export(movie_dir, original_swf, tools, release, reconstruction):
    with tempfile.TemporaryDirectory(prefix='srp-flash-export-') as temp:
        project = Path(temp) / 'project'
        movie.init_project(original_swf, project, tools, release)
        pinned = movie.verify_project(project)
        corrected, _ = source_patches.load_sources(reconstruction, project, pinned)
        if (movie_dir / 'src').exists():
            raise ValueError(f'{movie_dir}/src exists; export writes a new tree')
        sources = {}
        for relative in sorted(pinned['sources']):
            data = lf(corrected.get(relative, (project / 'pristine' / relative).read_bytes()))
            target = movie_dir / 'src' / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            sources[relative] = sha(data)
    release_pin = json.loads((HERE / 'releases' / (release + '.json')).read_text())
    spec = {'schema': 1, 'release': release, 'movie': release_pin['movie'],
            'movie_sha256': pinned['movie_sha256'],
            'reconstruction': reconstruction.resolve().relative_to(REPO).as_posix(),
            'reconstruction_sha256': sha(reconstruction.read_bytes()),
            'sources': sources}
    (movie_dir / 'movie.json').write_text(json.dumps(spec, indent=2, sort_keys=True) + '\n')
    return spec


def compiler_profiles(spec, output):
    """The reconstruction's per-class compiler settings, without its diffs (the tree has them applied)."""
    recon = REPO / spec['reconstruction']
    raw = recon.read_bytes()
    if sha(raw) != spec['reconstruction_sha256']:
        raise ValueError('The reconstruction manifest differs from the one the tree was exported with')
    manifest = json.loads(raw)
    keep = ('class', 'compiler', 'composition', 'method_reservations')
    classes = [{k: entry[k] for k in keep if k in entry} for entry in manifest['classes']]
    output.write_text(json.dumps({'schema': 1, 'release': spec['release'], 'movie_sha256': spec['movie_sha256'],
                                  'classes': classes}, indent=2, sort_keys=True) + '\n')
    return output


def project_from_tree(movie_dir, original_swf, project, files=None):
    """A project (tools/actionscript layout) whose exported sources are the tree's files."""
    spec = load(movie_dir)
    if sha(original_swf.read_bytes()) != spec['movie_sha256']:
        raise ValueError('Not the pinned original movie')
    files = tree(movie_dir) if files is None else files
    project.mkdir(parents=True)
    shutil.copyfile(original_swf, project / 'original.swf')
    for relative, data in files.items():
        target = project / 'pristine' / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    movie.write_json(project / 'manifest.json', {'schema': 1, 'release': spec['release'],
                                                  'movie_sha256': spec['movie_sha256'],
                                                  'sources': {p: sha(d) for p, d in sorted(files.items())}})
    return project


def build(movie_dir, original_swf, tools, work=None, jobs=4, report=print):
    """The tree's movie (bytes) and the build report (None when the tree is the baseline)."""
    import build_feature
    spec = load(movie_dir)
    data = original_swf.read_bytes()
    if sha(data) != spec['movie_sha256']:
        raise ValueError(f"Not the pinned original of {spec['movie']} (sha256 {sha(data)})")
    if differences(movie_dir) == ([], [], []):
        report('The tree is the baseline: the movie is the original')
        return data, None
    with (tempfile.TemporaryDirectory(prefix='srp-flash-') if work is None else _new_dir(work)) as folder:
        output = Path(folder) / 'build'
        result = build_feature.build(tools, movie_dir, original_swf, output, jobs, log=report)
        return (output / 'feature.swf').read_bytes(), result


class _new_dir:
    def __init__(self, path):
        self.path = Path(path)

    def __enter__(self):
        self.path.mkdir(parents=True, exist_ok=False)
        return self.path

    def __exit__(self, *exc):
        return False


def full(movie_dir, original_swf, tools, work, jobs):
    changed, added, removed = differences(movie_dir)
    if changed or added or removed:
        raise ValueError('A full build proves the baseline tree; this tree has changes '
                         f'({len(changed)} changed, {len(added)} added, {len(removed)} removed)')
    spec = load(movie_dir)
    work.mkdir(parents=True, exist_ok=False)
    project = project_from_tree(movie_dir, original_swf, work / 'project')
    profiles = compiler_profiles(spec, work / 'compiler-profiles.json')
    subprocess.run([sys.executable, str(HERE / 'build_baseline.py'), '--tools', str(tools),
                    '--reconstruction', str(profiles), '--jobs', str(jobs), str(project), str(work / 'build')],
                   check=True)
    rebuilt = (work / 'build' / 'assembled' / 'rebuilt.swf').read_bytes()
    if rebuilt != original_swf.read_bytes():
        raise ValueError('The rebuilt movie is not byte-identical to the original')
    return sha(rebuilt)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--tools', type=Path, default=REPO / 'private' / 'actionscript' / 'tools.json')
    commands = parser.add_subparsers(dest='command', required=True)
    cut = commands.add_parser('original')
    cut.add_argument('movie_dir', type=Path)
    cut.add_argument('archive', type=Path)
    cut.add_argument('output', type=Path)
    out = commands.add_parser('export')
    out.add_argument('movie_dir', type=Path)
    out.add_argument('original', type=Path)
    out.add_argument('--release', required=True)
    check = commands.add_parser('check')
    check.add_argument('movie_dir', type=Path)
    make = commands.add_parser('build')
    make.add_argument('movie_dir', type=Path)
    make.add_argument('original', type=Path)
    make.add_argument('output', type=Path)
    make.add_argument('--work', type=Path, help='a new directory to keep the reports and compiler output in')
    make.add_argument('--jobs', type=int, default=4)
    whole = commands.add_parser('full')
    whole.add_argument('movie_dir', type=Path)
    whole.add_argument('original', type=Path)
    whole.add_argument('work', type=Path)
    whole.add_argument('--jobs', type=int, default=4)
    args = parser.parse_args()
    if args.command == 'original':
        original(args.movie_dir, args.archive, args.output)
        print(f'Wrote {args.output}')
    elif args.command == 'export':
        recon = HERE / 'reconstruction' / args.release / 'manifest.json'
        spec = export(args.movie_dir, args.original, args.tools.resolve(), args.release, recon)
        print(f"Exported {len(spec['sources'])} scripts of {spec['movie']} into {args.movie_dir / 'src'}")
    elif args.command == 'check':
        changed, added, removed = differences(args.movie_dir)
        for label, paths in (('changed', changed), ('added', added), ('removed', removed)):
            for path in paths:
                print(f'{label}\t{path}')
        print(f'{len(changed)} changed, {len(added)} added, {len(removed)} removed against the baseline')
    elif args.command == 'build':
        if args.output.exists():
            raise ValueError('Output already exists; choose a new path')
        data, _ = build(args.movie_dir, args.original.resolve(), args.tools.resolve(),
                        args.work.resolve() if args.work else None, args.jobs)
        with args.output.open('xb') as stream:
            stream.write(data)
        print(f'Created {args.output}: sha256 {sha(data)}')
    else:
        digest = full(args.movie_dir, args.original.resolve(), args.tools.resolve(), args.work.resolve(), args.jobs)
        print(f'Rebuilt from the tree, byte-identical to the original: sha256 {digest}')
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
