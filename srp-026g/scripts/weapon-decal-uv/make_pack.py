#!/usr/bin/env python3
"""Build a content pack of independent decal UVs from the source meshes and shader programs."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from hashlib import sha256
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from shaders import FAMILIES

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'content-edit'))
import content_edit


def weapon_shader(path):
    parts = path.split('/')
    return (len(parts) == 4 and parts[:2] == ['shaders', 'sm_4_0'] and parts[2] in FAMILIES
            and len(parts[3]) == 67 and parts[3][22:].split('_', 1)[0] == '6')


def shader_sources(game_data, archives, packs, folder):
    command = [game_data, 'files', *archives]
    catalog = subprocess.check_output(command, text=True)
    base = sorted(path for line in catalog.splitlines()
                  if weapon_shader(path := line.partition('\t')[0]))
    paths = folder / 'shader-paths'
    paths.write_text('\n'.join(base) + '\n')
    original = folder / 'stock'
    subprocess.run([game_data, 'get', *archives, '--paths', paths, '--out', original], check=True)
    sources = {path: (None, original/path) for path in base}
    for pack in packs:
        manifest = json.loads((pack/'pack.json').read_text())
        for op in manifest['operations']:
            path = op.get('path', '')
            if weapon_shader(path):
                if op['op'] not in ('add_file', 'replace_file'):
                    raise ValueError(f'unexpected shader operation in {pack}: {op}')
                sources[path] = manifest['id'], pack/op.get('source', 'files/'+path)
    return sources


class Pack:
    def __init__(self, directory):
        self.directory = directory
        self.operations = []
        self.requires = set()

    def replace(self, path, source, data, owner):
        if data == source:
            return
        if owner is not None:
            self.requires.add(owner)
        target = self.directory/'files'/path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        self.operations.append({'op': 'replace_file', 'path': path, 'base_sha256': sha256(source).hexdigest()})

    def finish(self, version):
        manifest = {'format_version': 1, 'id': 'weapon-decal-uv', 'version': version,
                    'target_build': '0.26g', 'description': 'Independent texture coordinates for weapon decal bakes',
                    'requires': sorted(self.requires), 'operations': sorted(self.operations, key=lambda op: op['path'])}
        (self.directory/'pack.json').write_text(json.dumps(manifest, indent=2) + '\n')


def weapon_meshes(pack, source, unwrap, directory, jobs):
    manifest = json.loads((source/'pack.json').read_text())
    if manifest['id'] != 'weapon-modules':
        raise ValueError('expected the weapon-modules source pack')
    files = source/'files'
    root = files/'models/weapons/remodel_069'
    unique = {}
    for path in sorted(root.glob('*/*.skinned_model/render/*/converted_model')):
        key = sha256(path.read_bytes()).hexdigest()
        unique.setdefault(key, []).append(path)
    if not unique:
        raise ValueError(f'no skinned weapon surfaces under {root}')

    def convert(entry):
        key, paths = entry
        output = directory/key
        subprocess.run([unwrap, paths[0], output, '2048'], check=True)
        return paths, output.read_bytes()

    with ThreadPoolExecutor(max_workers=jobs) as workers:
        for paths, mesh in workers.map(convert, unique.items()):
            for path in paths:
                old = path.read_bytes()
                pack.replace(path.relative_to(files).as_posix(), old, mesh, manifest['id'])
                before, after = (struct.unpack_from('<I', data, 8)[0] for data in (old, mesh))
                if before != after:
                    props = path.with_name('export_properties')
                    original = props.read_bytes()
                    updated = bytearray(original)
                    content_edit.set_number(updated, 'source/vertices', before, after)
                    pack.replace(props.relative_to(files).as_posix(), original, updated, manifest['id'])
    return len(unique)


def main(args):
    args.out.parent.mkdir(parents=True, exist_ok=True)
    if args.out.exists():
        raise ValueError(f'{args.out} already exists')
    archives = ['--db', args.stock]
    for base in args.base:
        archives.extend(['--patch', base])
    with tempfile.TemporaryDirectory(dir=args.out.parent) as scratch:
        tmp = Path(scratch)
        output = tmp/'pack'
        output.mkdir()
        pack = Pack(output)
        sources = shader_sources(args.game_data, archives, args.shader_pack, tmp)
        for path, (owner, file) in sorted(sources.items()):
            src = file.read_bytes()
            data = FAMILIES[path.split('/')[2]](src)
            pack.replace(path, src, data, owner)
        count = weapon_meshes(pack, args.weapons, args.unwrap, tmp, args.jobs)
        pack.finish(args.version)
        output.rename(args.out)
    print(f'{count} distinct weapon surfaces, {len(pack.operations)} file replacements')
    print(sha256((args.out/'pack.json').read_bytes()).hexdigest())


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stock', type=Path, required=True, help='stock 0.26g resources.db')
    parser.add_argument('--base', type=Path, action='append', default=[], help='published base row from content.tsv')
    parser.add_argument('--weapons', type=Path, required=True, help='weapon-modules source pack')
    parser.add_argument('--shader-pack', type=Path, action='append', default=[], help='upstream shader pack, in manifest order')
    parser.add_argument('--game-data', type=Path, required=True)
    parser.add_argument('--unwrap', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--version', default='1')
    parser.add_argument('--jobs', type=int, default=min(4, os.cpu_count() or 1))
    main(parser.parse_args())
