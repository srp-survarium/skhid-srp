#!/usr/bin/env python3
"""Audit the packed meshes and replay the shader edits against their source programs."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from hashlib import sha256
import json
from pathlib import Path
import struct
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from make_pack import FAMILIES, content_edit, shader_sources


def versioned_packs(manifest, directory):
    packs = {}
    for line in manifest.read_text().splitlines():
        fields = line.split('\t')
        if fields[0] == 'pack':
            name, version = fields[1].rsplit('@', 1)
            packs[name] = directory / f'{name}-{version}', fields[2]
    return packs


def audit(args):
    packs = versioned_packs(args.manifest, args.packs)
    target, expected_hash = packs['weapon-decal-uv']
    manifest = (target / 'pack.json').read_bytes()
    if sha256(manifest).hexdigest() != expected_hash:
        raise ValueError(f'{target}: pack manifest differs from release/content.tsv')
    operations = json.loads(manifest)['operations']
    files = target / 'files'
    source = packs['weapon-modules'][0] / 'files'
    shaders = shader_sources(args.game_data, ['--db', args.stock], [packs['london-v042'][0]], args.work)
    declared = {op['path']: op for op in operations}
    if len(declared) != len(operations):
        raise ValueError('duplicate pack operation')
    actual = {p.relative_to(files).as_posix() for p in files.rglob('*') if p.is_file()}
    if actual != declared.keys():
        raise ValueError('payload and manifest file lists differ')
    pairs = {}
    surfaces = set()
    metadata = set()
    for original in (source / 'models/weapons/remodel_069').glob('*/*.skinned_model/render/*/converted_model'):
        path = original.relative_to(source).as_posix()
        surfaces.add(path)
        output = files / path
        old, new = original.read_bytes(), output.read_bytes()
        key = sha256(old).digest(), sha256(new).digest()
        before, after = (struct.unpack_from('<I', data, 8)[0] for data in (old, new))
        if before != after:
            props = original.with_name('export_properties').relative_to(source)
            metadata.add(props.as_posix())
            expected = bytearray((source/props).read_bytes())
            content_edit.set_number(expected, 'source/vertices', before, after)
            if (files/props).read_bytes() != expected:
                raise ValueError(f'{props}: vertex count or other metadata changed unexpectedly')
        pairs[key] = original, output
    if not surfaces or surfaces != {p for p in declared if p.endswith('/converted_model')}:
        raise ValueError('pack does not cover the source remodels')
    if metadata != {p for p in declared if p.endswith('/export_properties')}:
        raise ValueError('unexpected mesh metadata replacements')
    for path, (_, original) in shaders.items():
        before = original.read_bytes()
        after = FAMILIES[path.split('/')[2]](before)
        if after != before:
            if (files / path).read_bytes() != after:
                raise ValueError(f'{path}: packed shader differs from generator')
        elif path in declared:
            raise ValueError(f'{path}: redundant shader override')
    for path, op in declared.items():
        upstream = shaders[path][1] if path.startswith('shaders/') else source / path
        if op['op'] != 'replace_file' or op['base_sha256'] != sha256(upstream.read_bytes()).hexdigest():
            raise ValueError(f'{path}: upstream bytes differ')

    def check(pair):
        result = subprocess.run([args.checker, *pair], stdout=subprocess.PIPE, text=True)
        result.check_returncode()

    with ThreadPoolExecutor(max_workers=4) as pool:
        for _ in pool.map(check, pairs.values()):
            pass
    print(f'{len(surfaces)} mesh paths, {len(pairs)} distinct layouts, {len(shaders)} input shaders verified')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('packs', 'manifest', 'stock', 'game-data', 'checker', 'work'):
        parser.add_argument('--' + name, type=Path, required=True)
    audit(parser.parse_args())
