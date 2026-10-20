#!/usr/bin/env python3
"""Show a class's source as a build list's features leave it, and record a feature's diff.

    feature_source.py --reconstruction R PROJECT BUILD_LIST show CLASS [--before FEATURE] [-o FILE]
    feature_source.py --reconstruction R PROJECT BUILD_LIST diff FEATURE CLASS EDITED_FILE

`show` writes the class's source after every feature of the build list (or
only the features before FEATURE): the file to edit. `diff` writes the
feature's patch of the class, from the source the earlier features leave to
EDITED_FILE, as features/FEATURE/<Class>.patch, and adds the class to the
feature's feature.json if it is not there yet (name its edited `methods`, or
the `members` it adds or redeclares, before building). A later feature's patch
of the same class may then need recording again: patches apply exactly.
Only game classes take patches; repository-authored classes are whole files.
"""
import argparse
import difflib
import json
from pathlib import Path
import sys

import build_feature
import movie_source as movie
import source_patches as patches


def source_before(build_list, project, reconstruction, cls, stop=None):
    """The class's source after the build list's features before `stop` (all of them without it)."""
    project = project.resolve()
    pinned = movie.verify_project(project)
    reconstructed, _ = patches.load_sources(reconstruction, project, pinned)
    relative = cls.replace('.', '/') + '.as'
    if relative not in pinned['sources']:
        raise ValueError(f'{cls} is not a class of the release (a new class is a whole file)')
    source = reconstructed.get(relative, (project / 'pristine' / relative).read_bytes())
    spec = json.loads(build_list.read_bytes())
    if stop is not None and stop not in spec['features']:
        raise ValueError(f'The build list has no feature {stop}')
    for name in spec['features']:
        if name == stop:
            break
        folder = build_list.parent / 'features' / name
        for entry in json.loads((folder / 'feature.json').read_bytes())['classes']:
            if entry['class'] != cls:
                continue
            if 'patch' not in entry:
                raise ValueError(f'{cls} is a whole repository file in {name}, not patched')
            source = patches.apply_patch(source, build_feature.contained(folder, entry['patch'], 'Feature patch').read_bytes(), relative)
    return source


def diff(before, after, relative):
    old = before.decode('utf-8').replace('\r\n', '\n').splitlines(keepends=True)
    new = after.decode('utf-8').replace('\r\n', '\n').splitlines(keepends=True)
    if not old or not new or not old[-1].endswith('\n') or not new[-1].endswith('\n'):
        raise ValueError('Sources must end with a newline')
    text = ''.join(difflib.unified_diff(old, new, f'a/{relative}', f'b/{relative}', n=3))
    if not text:
        raise ValueError('The edited source is the same as before the feature')
    # difflib puts a tab-less date nowhere, but its headers have no newline on
    # an empty date: normalize to the applicator's exact two header lines.
    lines = text.splitlines(keepends=True)
    lines[0], lines[1] = f'--- a/{relative}\n', f'+++ b/{relative}\n'
    return ''.join(lines).encode('utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--reconstruction', required=True, type=Path)
    parser.add_argument('project', type=Path)
    parser.add_argument('build_list', type=Path)
    commands = parser.add_subparsers(dest='command', required=True)
    show = commands.add_parser('show')
    show.add_argument('cls')
    show.add_argument('--before')
    show.add_argument('-o', '--output', type=Path)
    record = commands.add_parser('diff')
    record.add_argument('feature')
    record.add_argument('cls')
    record.add_argument('edited', type=Path)
    args = parser.parse_args()
    if args.command == 'show':
        source = source_before(args.build_list, args.project, args.reconstruction, args.cls, args.before)
        if args.output:
            args.output.write_bytes(source)
        else:
            sys.stdout.buffer.write(source)
        return 0
    relative = args.cls.replace('.', '/') + '.as'
    before = source_before(args.build_list, args.project, args.reconstruction, args.cls, args.feature)
    patch = diff(before, args.edited.read_bytes(), relative)
    if patches.apply_patch(before, patch, relative).replace(b'\r\n', b'\n') != args.edited.read_bytes().replace(b'\r\n', b'\n'):
        raise ValueError('The recorded patch does not reproduce the edited source')
    folder = args.build_list.parent / 'features' / args.feature
    folder.mkdir(parents=True, exist_ok=True)
    name = args.cls.rsplit('.', 1)[-1] + '.patch'
    (folder / name).write_bytes(patch)
    manifest_path = folder / 'feature.json'
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {'schema': 2, 'classes': []}
    if not any(entry['class'] == args.cls for entry in manifest['classes']):
        manifest['classes'].append({'class': args.cls, 'patch': name, 'methods': []})
        manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
        print(f'Added {args.cls} to {manifest_path}: name its edited methods (or members) there')
    print(f'Wrote {folder / name}')
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError) as error:
        sys.exit(str(error))
