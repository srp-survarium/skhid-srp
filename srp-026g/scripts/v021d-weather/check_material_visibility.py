#!/usr/bin/env python3
"""Check real composed materials against original visibility and native isolation."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from make_material_visibility import make
from check_visibility_shaders import check as check_shaders
from make_vertex_alpha import prior_material


def digest(values):
    return hashlib.sha256(json.dumps(values, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def read(porter, db, path):
    output = subprocess.check_output([str(porter), 'dump-db-config', '--db', str(db),
                                      '--path', path, '--leaves'], text=True)
    return {key: json.loads(value.split(':', 1)[1])
            for line in output.splitlines() for key, value in [line.split('\t', 1)]}


def check(porter, donor, patch):
    here = Path(__file__).resolve().parent
    source = json.loads((here / 'material-visibility-inputs.json').read_text())
    authored = json.loads((here.parent / 'content-edit/v021d-material-visibility-1.json').read_text())
    assert authored == make(source), 'authored visibility edit differs from donor provenance'
    blend = json.loads((here / 'vertex-blend-inputs.json').read_text())
    blend_changes = {r['target_material']: r['changes'] for r in blend['materials']}
    alpha = json.loads((here / 'alpha-texture-inputs.json').read_text())
    alpha_changes = {r['target_material']: r['changes'] for r in alpha['materials']}
    painted = json.loads((here / 'vertex-alpha-inputs.json').read_text())
    paths = set(subprocess.check_output([str(porter), 'list', '--db', str(patch)], text=True).splitlines())
    # The correction has no right to change a shared native material. All eight
    # remain absent from this patch; their original archived bytes stay mounted.
    models = {}
    flags = 0
    for row in source['materials']:
        original = read(porter, donor, f"material_instances/{row['original_material']}.material")
        actual = read(porter, patch, f"material_instances/{row['target_material']}.material")
        # The later painted-cutoff edit is checked separately. Reconstruct this
        # earlier state only after verifying every exact final selector value.
        actual = prior_material(actual, row['target_material'], painted)
        for key, values in row['changes'].items():
            assert original[key] == values['original'], (row['original_material'], key, 'donor')
            assert actual[key] == original[key], (row['target_material'], key, 'port')
            flags += 1
        rest = {k: v for k, v in actual.items() if k not in row['changes']}
        # The later original-blend edit has its own strict composed check.
        # Undo only its exact authored deltas when checking this earlier edit's
        # snapshot, so visibility restoration still cannot change other leaves.
        for key, values in blend_changes.get(row['target_material'], {}).items():
            assert key not in row['changes'] and rest[key] == values['to']
            rest[key] = values['from']
        # The independently checked later alpha edit changes only these exact
        # texture references, preserving original mipchains in private assets.
        for key, values in alpha_changes.get(row['target_material'], {}).items():
            assert key not in row['changes'] and rest[key] == values['to']
            rest[key] = values['from']
        assert digest(rest) == row['unchanged_leaves_sha256'], (row['target_material'], 'finish/schema drift')
        if row['current_material'] != row['target_material']:
            assert f"material_instances/{row['current_material']}.material" not in paths, 'native material modified'
            for binding in row['bindings']:
                path = binding['settings']
                if path not in models:
                    models[path] = read(porter, patch, path)
                assert models[path][binding['key']] == row['target_material'], (path, binding['key'])
    print(f"Original visibility: {len(source['materials'])} materials, {flags} flags; "
          f"{len(models)} rebound models; native shared materials and all other material leaves unchanged")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'donor', 'patch', 'stock', 'shader-pack'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    check(args.porter, args.donor, args.patch)
    check_shaders(args.porter, args.stock, args.patch, args.shader_pack)
