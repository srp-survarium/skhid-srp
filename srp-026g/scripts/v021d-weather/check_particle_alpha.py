"""Check exact original particle alpha assets and narrowly scoped composed edits."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from make_particle_alpha import PREFIX, make


def digest(values):
    return hashlib.sha256(json.dumps(values, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def read(porter, db, path):
    output = subprocess.check_output([str(porter), 'dump-db-config', '--db', str(db),
                                      '--path', path, '--leaves'], text=True)
    return {key: json.loads(value.split(':', 1)[1])
            for line in output.splitlines() for key, value in [line.split('\t', 1)]}


def check(porter, donor, stock, pack, patch=None):
    here = Path(__file__).resolve().parent
    source = json.loads((here / 'particle-alpha-inputs.json').read_text())
    authored_path = here.parent / 'content-edit/v021d-particle-alpha-1.json'
    assert json.loads(authored_path.read_text()) == make(source)
    manifest = json.loads((pack / 'pack.json').read_text())
    expected = {'textures/' + PREFIX + row['texture'] + '.' + extension
                for row in source['textures'] for extension in ('dds', 'options')}
    assert {r['path'] for r in manifest['operations']} == expected
    assert len(manifest['operations']) == len(expected)
    assert all(r['op'] == 'add_file' for r in manifest['operations'])
    paths = set(subprocess.check_output([str(porter), 'list', '--db', str(patch)], text=True).splitlines()) if patch else set()
    with tempfile.TemporaryDirectory(prefix='particle-alpha-check-') as temporary:
        output = Path(temporary) / 'resource'
        def extract(db, path):
            subprocess.run([str(porter), 'extract', '--db', str(db), '--path', path,
                            '--output', str(output)], check=True, stdout=subprocess.DEVNULL)
            return output.read_bytes()
        for row in source['textures']:
            for extension, key in [('dds', 'original_sha256'), ('options', 'options_sha256')]:
                origin = 'textures/' + row['texture'] + '.' + extension
                target = 'textures/' + PREFIX + row['texture'] + '.' + extension
                original = extract(donor, origin)
                assert hashlib.sha256(original).hexdigest() == row[key], origin
                assert (pack / 'files' / target).read_bytes() == original, target
                native = extract(stock, origin)
                native_hash = row['native_sha256'] if extension == 'dds' else row['options_sha256']
                assert hashlib.sha256(native).hexdigest() == native_hash, origin
                if patch:
                    assert origin not in paths, ('shared native texture changed', origin)
                    assert extract(patch, target) == original, ('composed original texture', target)
    for row in source['materials']:
        origin = 'material_instances/' + row['original_material'] + '.material'
        original = read(porter, donor, origin)
        assert digest(original) == row['original_leaves_sha256'], origin
        for key, change in row['changes'].items():
            assert original[key] == change['source_texture'] and original[change['selector']] is True
        if patch:
            current_path = 'material_instances/' + row['current_material'] + '.material'
            # Shared native sources cannot be overridden. Older imported
            # source materials may already be present in the composed patch;
            # if present, every leaf must retain its original value.
            if row['current_material'].startswith('ported/v021d/'):
                if current_path in paths:
                    assert read(porter, patch, current_path) == original, ('imported source material changed', current_path)
            else:
                assert current_path not in paths, ('native particle material changed', current_path)
            actual = read(porter, patch, 'material_instances/' + row['target_material'] + '.material')
            for key, change in row['changes'].items():
                assert actual[key] == change['to'] and actual[change['selector']] is True
            assert digest({k: v for k, v in actual.items() if k not in row['changes']}) == row['unchanged_leaves_sha256'], row['target_material']
    if patch:
        for row in source['particles']:
            actual = read(porter, patch, row['path'])
            for key, change in row['changes'].items():
                assert actual[key] == change['to'], (row['path'], key)
            assert digest({k: v for k, v in actual.items() if k not in row['changes']}) == row['unchanged_leaves_sha256'], ('particle settings drift', row['path'])
    print(f"Original particle alpha: {len(source['textures'])} exact DDS/options pairs, {len(source['materials'])} private material copies, {len(source['particles'])} particle definitions; " + ('composed isolation passed' if patch else 'composed isolation pending'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'donor', 'stock', 'pack'):
        parser.add_argument('--' + name, required=True, type=Path)
    parser.add_argument('--patch', type=Path)
    args = parser.parse_args()
    check(args.porter, args.donor, args.stock, args.pack, args.patch)
