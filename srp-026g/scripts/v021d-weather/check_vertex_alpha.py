"""Check original painted coverage, native fallback and final private materials."""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from check_material_visibility import read as leaves, digest
from check_vertex_blend import chunks, instructions
from make_vertex_alpha import make, PROFILES, PREFIX, ALPHA_REF, INACTIVE
from vertex_alpha_math import run, bits, number

SAMPLES = tuple(itertools.product((0., .001, .1, .25, .5, .75, .9, .999, 1.), repeat=2))
FALLBACK = (0., .1, .25, .5, 1., -21021., -21209., -21214., -21216.)
DEPTH_CUTOFF = [0x09000000, 0x00100012, 0, 0x0010003a, 0, 0x8020802a, 0x41, 0, 15]


def original_block(blob):
    _, cs = chunks(blob)
    _, ops = instructions(cs[b'SHDR'])
    at = next(i for i, o in enumerate(ops) if o[0] & 2047 == 49)
    assert ops[at][3:5] == [0x0010000a, 0]
    return [o for o in ops[:at] if o[0] & 2047 == 69 or
            (o[0] & 2047 < 88 and (o[1] >> 12) & 255 == 0 and ((o[1] >> 4) & 15).bit_count() == 1)]


def geometry(native, actual, vertex, references):
    old_header, old_cs = chunks(native)
    header, cs = chunks(actual)
    assert header == old_header and set(cs) == set(old_cs)
    assert all(cs[k] == v for k, v in old_cs.items() if k not in (b'SHDR', b'ISGN'))
    _, old_ops = instructions(old_cs[b'SHDR'])
    _, ops = instructions(cs[b'SHDR'])
    register = {1: 7, 2: 9}.get(vertex)
    signature = bytearray(old_cs[b'ISGN'])
    if register is not None:
        matches = [8 + i*24 for i in range(struct.unpack_from('<I', signature)[0])
                   if struct.unpack_from('<I', signature, 8 + i*24 + 16)[0] == register]
        assert len(matches) == 1 and signature[matches[0] + 20] & 1
        signature[matches[0] + 21] |= 1
    assert bytes(signature) == cs[b'ISGN']
    starts = [i for i, o in enumerate(ops) if o[0] & 2047 == 24 and o[-2:] == [0x4001, bits(-21210.)]]
    assert len(starts) == 1
    start = starts[0]
    stop = start + 86
    block = ops[start:stop]
    cutoff = block[80]
    assert ops[stop][0] & 2047 == 49 and all(o[0] & 2047 == 21 for o in block[-5:])
    recovered = [o.copy() for o in ops[:start] + [cutoff] + ops[stop:]]
    old_declarations = {o[2]: o for o in old_ops if o[0] & 2047 == 98}
    fixed = []
    for o in recovered:
        if o[0] == 0x02000068:
            o[1] -= 1
        if o[0] & 2047 == 98 and o[2] == register:
            if register not in old_declarations:
                assert o[1] & 0xf0 == 0x10
                continue
            previous = old_declarations[register]
            assert o == [previous[0], previous[1] | 0x10, previous[2]]
            o = previous
        fixed.append(o)
    assert fixed == old_ops, 'unmarked native instructions changed'
    constant = cutoff[3] == 0x8020802a
    initial = None if constant else (cutoff[4], (cutoff[3] >> 4) & 3)
    output = (cutoff[2], ((cutoff[1] >> 4) & 15).bit_length() - 1)
    for profile, marker in PROFILES.items():
        expected = references[vertex, marker, constant]
        for (v, a), value in zip(SAMPLES, expected):
            assert run(block, v, a, profile[:3], marker, initial, output) == value, (vertex, marker, v, a)
    for marker in FALLBACK:
        for v, a in SAMPLES:
            expected = bits((1. if constant else number(bits(a))) - number(bits(marker)))
            assert run(block, v, a, (1., 1., False), marker, initial, output) == expected


def depth(native, actual):
    old_header, old_cs = chunks(native)
    header, cs = chunks(actual)
    assert header == old_header and set(cs) == set(old_cs)
    assert all(cs[k] == v for k, v in old_cs.items() if k != b'SHDR')
    _, old_ops = instructions(old_cs[b'SHDR'])
    _, ops = instructions(cs[b'SHDR'])
    start = next(i for i, o in enumerate(ops) if o[0] & 2047 == 54)
    stop = next(i for i, o in enumerate(ops) if o[0] & 2047 == 49)
    assert stop - start == 12
    recovered = [o.copy() for o in ops[:start] + [DEPTH_CUTOFF] + ops[stop:]]
    temporary = next(o for o in recovered if o[0] == 0x02000068)
    assert temporary[1] == 2
    temporary[1] = 1
    assert recovered == old_ops, 'unmarked shadow instructions changed'
    thresholds = {marker: profile[3] for profile, marker in PROFILES.items()}
    for marker in tuple(thresholds) + FALLBACK:
        for _, alpha in SAMPLES:
            actual = run(ops[start:stop], 0., alpha, (), marker, (0, 3))
            assert actual == bits(number(bits(alpha)) - number(bits(thresholds.get(marker, marker))))


def check(porter, donor, stock, patch, pack):
    here = Path(__file__).resolve().parent
    source = json.loads((here/'vertex-alpha-inputs.json').read_text())
    catalog = json.loads((here/'vertex-alpha-programs.json').read_text())
    assert json.loads((here.parent/'content-edit/v021d-vertex-alpha-1.json').read_text()) == make(source)
    paths = {path: (kind, row) for kind, family in catalog.items() for row in family['native'] for path in row['paths']}
    assert len(paths) == 1442
    manifest = json.loads((pack/'pack.json').read_text())
    assert len(manifest['operations']) == 1442
    assert {r['path']: r['base_sha256'] for r in manifest['operations']} == {p: r['sha256'] for p, (_, r) in paths.items()}
    assert all(r['op'] == 'replace_file' for r in manifest['operations'])
    stock_paths = subprocess.check_output([str(porter), 'list', '--db', str(stock),
        '--prefix', 'shaders/sm_4_0/gbuffer_pass.ps/'], text=True).splitlines()
    def ordinary(path):
        parts = [p for p in path.rsplit('/', 1)[-1].split('_') if p]
        return (len(parts) == 11 and parts[1].endswith('1') and parts[7] == '00000'
                and parts[4] in ('0', '1', '2', '10', '12', '13', '14'))
    expected_ordinary = {p for p in stock_paths if ordinary(p)}
    geometry_paths = {p for p, (kind, _) in paths.items() if kind == 'geometry'}
    assert len(expected_ordinary) == 854
    assert {p for p in geometry_paths if ordinary(p)} == expected_ordinary
    ordinary_hashes = {paths[p][1]['sha256'] for p in expected_ordinary}
    for alias in geometry_paths - expected_ordinary:
        parts = [p for p in alias.rsplit('/', 1)[-1].split('_') if p]
        assert parts[7] == '10000' and alias in stock_paths and paths[alias][1]['sha256'] in ordinary_hashes
    with tempfile.TemporaryDirectory(prefix='check-painted-alpha-') as tmp:
        def read(db, path):
            output = Path(tmp)/'resource'
            subprocess.run([str(porter), 'extract', '--db', str(db), '--path', path, '--output', str(output)],
                           check=True, stdout=subprocess.DEVNULL)
            return output.read_bytes()
        references = {}
        assert len(catalog['geometry']['original']) == 7
        for original in catalog['geometry']['original']:
            blob = read(donor, original['path'])
            assert hashlib.sha256(blob).hexdigest() == original['sha256']
            block = original_block(blob)
            for profile, marker in PROFILES.items():
                for constant in (False, True):
                    references[original['vertex'], marker, constant] = [
                        run(block, v, 1. if constant else a, profile[:3], 0.) for v, a in SAMPLES]
        assert len(catalog['depth']['original']) == 14
        for original in catalog['depth']['original']:
            blob = read(donor, original['paths'][0])
            assert hashlib.sha256(blob).hexdigest() == original['sha256']
            _, cs = chunks(blob)
            _, ops = instructions(cs[b'SHDR'])
            at = next(i for i, o in enumerate(ops) if o[0] & 2047 == 13)
            assert ops[at - 2] == DEPTH_CUTOFF
            assert [o[0] & 2047 for o in ops if o[0] & 2047 < 88] == [72, 0, 49, 13, 54, 62]
        seen = set()
        for path, (kind, contract) in paths.items():
            actual = read(patch, path)
            assert actual == (pack/'files'/path).read_bytes(), (path, 'composed shader differs')
            identity = (kind, contract['sha256'], hashlib.sha256(actual).hexdigest())
            if identity in seen:
                continue
            seen.add(identity)
            native = read(stock, path)
            assert hashlib.sha256(native).hexdigest() == contract['sha256']
            if kind == 'geometry':
                geometry(native, actual, contract['vertex'], references)
            else:
                assert kind == 'depth'
                depth(native, actual)
        assert len(seen) == 280
    patch_paths = set(subprocess.check_output([str(porter), 'list', '--db', str(patch)], text=True).splitlines())
    for row in source['materials']:
        original = leaves(porter, donor, f"material_instances/{row['original_material']}.material")
        assert digest(original) == row['original_leaf_sha256']
        if row['state'] != 'active':
            assert original[PREFIX+'use_alpha_test.value'] is False
            path = f"material_instances/{row['target_material']}.material"
            if path in patch_paths:
                assert digest(leaves(porter, patch, path)) == row['effective_leaf_sha256'], (path, 'inactive material changed')
            continue
        assert original[PREFIX+'use_alpha_test.value'] is True and original[PREFIX+'use_vertex_alpha_test.value'] is True
        profile = tuple(original[PREFIX+k+'.value'] for k in ('dark_border', 'light_border', 'texture_alpha_multiply_vertex_alpha', 'alpha_ref'))
        assert row['original_blend']['master'] == original[PREFIX+'use_vertex_blended_textures.value']
        assert all(original[PREFIX+k+'.value'] == v for k, v in row['original_blend']['channels'].items())
        actual = leaves(porter, patch, f"material_instances/{row['target_material']}.material")
        assert actual[ALPHA_REF] == PROFILES[profile]
        assert actual['material.g_stage.is_cast_shadow.value'] == row['original_cast_shadow']
        for name, value in INACTIVE.items():
            assert actual[PREFIX+name+'.value'] == value
        for key, change in row['changes'].items():
            assert actual[key] == change['to']
        assert digest({k:v for k,v in actual.items() if k not in row['changes']}) == row['unchanged_leaves_sha256']
    print('Original painted alpha: 1364 visible/78 depth paths, 280 instruction inverses, 7 donor layouts, 5 visible/shadow profiles, 28 private materials; original caster flags and unrelated leaves preserved')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'donor', 'stock', 'patch', 'pack'):
        parser.add_argument('--'+name, required=True, type=Path)
    args = parser.parse_args()
    check(args.porter, args.donor, args.stock, args.patch, args.pack)
