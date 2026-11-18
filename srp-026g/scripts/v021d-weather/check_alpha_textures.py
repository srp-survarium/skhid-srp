"""Independently verify exact donor mipchains and isolated composed bindings."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from make_alpha_textures import make
from check_material_visibility import digest, read
from make_vertex_alpha import prior_material


def check(porter, donor, stock, pack, patch=None, patches=None, patch_cache=None):
    here = Path(__file__).resolve().parent
    source = json.loads((here / 'alpha-texture-inputs.json').read_text())
    painted = json.loads((here / 'vertex-alpha-inputs.json').read_text())
    authored = json.loads((here.parent / 'content-edit/v021d-alpha-textures-1.json').read_text())
    assert authored == make(source), 'authored alpha references differ from provenance'
    manifest = json.loads((pack / 'pack.json').read_text())
    expected = {'textures/ported/v021d/original_alpha_mips/' + row['texture'] + '.' + ext
                for row in source['textures'] for ext in ('dds', 'options')}
    assert {r['path'] for r in manifest['operations']} == expected
    assert all(r['op'] == 'add_file' for r in manifest['operations'])
    assert len(manifest['operations']) == len(expected)
    paths = set(subprocess.check_output([str(porter), 'list', '--db', str(patch)], text=True).splitlines()) if patch else set()
    with tempfile.TemporaryDirectory(prefix='check-original-alpha-') as temporary:
        output = Path(temporary) / 'resource'

        def extract(db, name):
            subprocess.run([str(porter), 'extract', '--db', str(db), '--path', name,
                            '--output', str(output)], check=True, stdout=subprocess.DEVNULL)
            return output.read_bytes()

        for row in source['textures']:
            for ext, key in [('dds', 'original_sha256'), ('options', 'options_sha256')]:
                origin = 'textures/' + row['texture'] + '.' + ext
                target = 'textures/ported/v021d/original_alpha_mips/' + row['texture'] + '.' + ext
                original = extract(donor, origin)
                assert hashlib.sha256(original).hexdigest() == row[key], origin
                assert (pack / 'files' / target).read_bytes() == original, target
                native = extract(stock, origin)
                expected_native = row['native_sha256'] if ext == 'dds' else row['options_sha256']
                assert hashlib.sha256(native).hexdigest() == expected_native, origin
                if patch:
                    assert origin not in paths, ('shared native texture modified', origin)
                    assert extract(patch, target) == original, ('composed texture', target)
        if patch:
            assert patches is not None and patch_cache is not None
            layers = [stock] + [patch_cache / line.split('\t')[0] for line in patches.read_text().splitlines()
                                if line.startswith('resources.db.patch_')] + [patch]
            for row in source['masked_base_textures']:
                prefix = 'textures/' + row['current_texture']
                found = {}
                for layer in layers:
                    entries = subprocess.check_output([str(porter), 'list', '--db', str(layer), '--prefix', prefix], text=True).splitlines()
                    for path in (prefix + '.dds', prefix + '.options'):
                        if path in entries:
                            found[path] = layer
                dds = extract(found[prefix + '.dds'], prefix + '.dds')
                original = extract(donor, 'textures/' + row['original_texture'] + '.dds')
                assert dds == original and hashlib.sha256(dds).hexdigest() == row['original_dds_sha256']
                # Older private options may be reserialized, but every typed
                # value must equal the original; the complete DDS stays exact.
                expected = read(porter, donor, 'textures/' + row['original_texture'] + '.options')
                assert expected == row['options_values']
                assert read(porter, found[prefix + '.options'], prefix + '.options') == expected
                for material in row['materials']:
                    leaves = read(porter, patch, f'material_instances/{material}.material')
                    assert leaves['material.g_stage.effect.texture_diffuse.value'] == row['current_texture']
    if patch:
        models = {}
        for row in source['materials']:
            original = read(porter, donor, f"material_instances/{row['original_material']}.material")
            actual = read(porter, patch, f"material_instances/{row['target_material']}.material")
            actual = prior_material(actual, row['target_material'], painted)
            for key, change in row['changes'].items():
                assert original[key] == change['source_texture'], (row['original_material'], key)
                assert original[change['selector']] is True and actual[change['selector']] is True
                assert actual[key] == change['to'], (row['target_material'], key)
            assert digest({k: v for k, v in actual.items() if k not in row['changes']}) == row['unchanged_leaves_sha256'], (row['target_material'], 'unrelated material drift')
            if row['current_material'] != row['target_material']:
                assert f"material_instances/{row['current_material']}.material" not in paths, 'native material modified'
                for binding in row['bindings']:
                    name = binding['settings']
                    if name not in models:
                        models[name] = read(porter, patch, name)
                    assert models[name][binding['key']] == row['target_material'], (name, binding['key'])
        print(f"Original alpha mipchains: {len(source['textures'])} exact donor DDS/options pairs, {len(source['materials'])} isolated materials; native resources and unrelated leaves unchanged")
        print('Original masked blend: all13 materials bind4 complete original base DDS mipchains and matching options values')
    else:
        print(f"Original alpha pack: {len(source['textures'])} exact donor DDS/options pairs; composed material verification pending")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'donor', 'stock', 'pack'):
        parser.add_argument('--' + name, required=True, type=Path)
    parser.add_argument('--patch', type=Path)
    parser.add_argument('--patches', type=Path)
    parser.add_argument('--patch-cache', type=Path)
    args = parser.parse_args()
    check(args.porter, args.donor, args.stock, args.pack, args.patch, args.patches, args.patch_cache)
