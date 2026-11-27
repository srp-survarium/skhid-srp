"""Check composed original-linear materials and every native shader fallback."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from check_visibility_shaders import chunks
from check_material_visibility import read as leaves, digest
from make_vertex_blend import make, masked_marker, PREFIX, STRENGTH, TECHNIQUE, NORMAL, ALPHA_CONSIDER
import check_masked_blend

def instructions(code):
    words = list(struct.unpack('<' + 'I' * (len(code) // 4), code))
    assert words[1] == len(words)
    result = []
    at = 2
    while at < len(words):
        size = (words[at] >> 24) & 127
        assert size and at + size <= len(words)
        result.append(words[at:at + size])
        at += size
    return words[0], result


def check_program(native, actual, path):
    marker = struct.unpack("<I", struct.pack("<f", -21021))[0]
    old_header,old=chunks(native);new_header,new=chunks(actual)
    assert old_header==new_header and old.keys()==new.keys()
    assert all(old[k]==new[k] for k in old if k!=b'SHDR'), (path,'resource/signature delta')
    old_version,before=instructions(old[b'SHDR']);new_version,after=instructions(new[b'SHDR'])
    assert old_version==new_version
    # Discover the original weight's source from native arithmetic, independently
    # of the catalog's register annotations used by the builder.
    weight=[(i,op) for i,op in enumerate(before) if op[0]==0x08002038 and op[-3:]==[0x0020801a,0,14]]
    assert len(weight)==1
    weight_at,weight_op=weight[0]
    logs=[op for op in before[weight_at-5:weight_at] if op[0]&2047==47]
    assert len(logs)==1 and logs[0][3:5]==[0x8010100a,0x81]
    alpha_register=logs[0][5]
    declarations=[(i,op) for i,op in enumerate(before) if op[0]&2047==104]
    assert len(declarations)==1
    temps_at,temps_op=declarations[0];scratch=temps_op[1]
    branches=[i for i,op in enumerate(after) if op[0]&2047==24 and op[-2:]==[0x00004001,marker]]
    assert len(branches)==1
    at=branches[0]
    assert at==weight_at+1 and after[at-1]==weight_op
    assert after[at:at+4]==[
        [0x08000018,0x00100012,scratch,0x0020801a,0,14,0x00004001,marker],
        [0x0304001f,0x0010000a,scratch],
        [0x05000036,*weight_op[1:3],0x0010100a,alpha_register],
        [0x01000015]], (path,'original-alpha override differs')
    check_masked_blend.verify(after[at+4:at+24], before, scratch, weight_op[1:3], alpha_register)
    del after[at:at+24]
    assert after[temps_at]==[0x02000068,scratch+1]
    after[temps_at]=temps_op
    assert before==after, (path,'unmarked native arithmetic changed')


def check(porter, stock, donor, patch, pack, composed=None):
    # `composed`: the version content.tsv lists, when it is a later version built on `pack`
    # (v021d-vertex-blend@4 = @3 with 0.69d's blend cut, its own check v069d-blend-cut).
    composed = composed or pack
    here = Path(__file__).resolve().parent
    source = json.loads((here / 'vertex-blend-inputs.json').read_text())
    catalog = [line.split('\t') for line in (here / 'vertex-blend-programs.tsv').read_text().splitlines()
               if line and not line.startswith('#')]
    paths = {row[0]: row[1] for row in catalog}
    manifest = json.loads((pack / 'pack.json').read_text())
    assert len(paths) == len(catalog) == len(manifest['operations'])
    assert all(op['op'] == 'replace_file' for op in manifest['operations'])
    assert {op['path']: op['base_sha256'] for op in manifest['operations']} == paths
    originals = {}
    with tempfile.TemporaryDirectory() as tmp:
        def read(db, path):
            output = Path(tmp) / 'shader'
            subprocess.run([str(porter), 'extract', '--db', str(db), '--path', path,
                            '--output', str(output)], check=True, stdout=subprocess.DEVNULL)
            return output.read_bytes()
        assert hashlib.sha256(read(donor, check_masked_blend.SOURCE)).hexdigest() == check_masked_blend.SOURCE_SHA256
        # Reviewed retail programs copy base alpha into both blend operands.
        # Their interpolation therefore cannot change the alpha-test cutoff,
        # for either original linear or original masked blending.
        assert set(source['alpha_cutoff_proofs']) == {'linear', 'masked'}
        for proof in source['alpha_cutoff_proofs'].values():
            assert hashlib.sha256(read(donor, proof['path'])).hexdigest() == proof['sha256']
        for path, expected in paths.items():
            # Extract every composed alias, including ones sharing bytecode.
            assert read(patch, path) == (composed / 'files' / path).read_bytes(), path
            actual = (pack / 'files' / path).read_bytes()
            if expected not in originals:
                native = read(stock, path)
                assert hashlib.sha256(native).hexdigest() == expected, path
                originals[expected] = native
            check_program(originals[expected], actual, path)
    alpha = json.loads((here / 'alpha-texture-inputs.json').read_text())
    alpha_changes = {r['target_material']: r['changes'] for r in alpha['materials']}
    authored = json.loads((here.parent / 'content-edit/v021d-vertex-blend-1.json').read_text())
    assert authored == make(source), 'material edit differs from provenance'
    classifications = {row['material']: row['mode'] for row in source['classifications']}
    assert {r['current_material'] for r in source['materials']} == {
        material for material, mode in classifications.items() if mode in ('linear', 'masked')}
    patch_paths = set(subprocess.check_output([str(porter), 'list', '--db', str(patch)], text=True).splitlines())
    models = {}
    for row in source['materials']:
        original = leaves(porter, donor, f"material_instances/{row['original_material']}.material")
        for key, value in row['original_blend'].items():
            assert original.get(key, False) == value, (row['original_material'], key, 'donor')
        actual = leaves(porter, patch, f"material_instances/{row['target_material']}.material")
        for key, value in row['changes'].items():
            assert actual[key] == value['to'], (row['target_material'], key)
        assert row['mode'] == classifications[row['current_material']]
        marker = -21021 if row['mode'] == 'linear' else masked_marker(original[PREFIX + 'constant_vertex_blended_factor.value'])
        assert actual[STRENGTH] == marker and actual[TECHNIQUE] == 1
        if row['mode'] == 'masked':
            assert actual[PREFIX + 'constant_vertex_blended_falloff.value'] == original[PREFIX + 'constant_vertex_blended_falloff.value']
        assert actual[NORMAL] == original[NORMAL]
        if original[PREFIX + 'use_alpha_test.value']:
            assert row['alpha_cutoff'] == 'base_texture'
            assert actual[ALPHA_CONSIDER] is False
        else:
            assert ALPHA_CONSIDER not in row['changes']
        assert actual[PREFIX + 'texture_vertex_blended_diffuse.value']
        assert actual[PREFIX + 'constant_vertex_blended_textures_tile.value'] == 1
        rest = {k: v for k, v in actual.items() if k not in row['changes']}
        # The later donor-mip correction owns only these exact texture leaves.
        # Its own checker verifies every donor payload and final material value.
        for key, values in alpha_changes.get(row['target_material'], {}).items():
            assert key not in row['changes'] and rest[key] == values['to']
            rest[key] = values['from']
        assert digest(rest) == row['unchanged_leaves_sha256'], (row['target_material'], 'unrelated material change')
        if row['current_material'] != row['target_material']:
            assert f"material_instances/{row['current_material']}.material" not in patch_paths
            for binding in row['bindings']:
                path = binding['settings']
                if path not in models:
                    models[path] = leaves(porter, patch, path)
                assert models[path][binding['key']] == row['target_material'], (path, binding['key'])
    print(f'Original vertex blend: {len(paths)} composed paths, {len(originals)} native programs; '
          f'{len(source["materials"])} original materials, {len(models)} rebound models; '
          'native instructions, signatures, shared materials and unrelated leaves preserved')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'stock', 'donor', 'patch', 'pack'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--composed-pack', type=Path, help='the version content.tsv lists, built on --pack')
    args = parser.parse_args()
    check(args.porter, args.stock, args.donor, args.patch, args.pack, args.composed_pack)

