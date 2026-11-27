"""v021d-vertex-blend@4 against @3: 0.69d's cut of an alpha-tested blend by its second layer.

    check_blend_cut.py --packs PACKS_DIR

Version 4 is version 3 with two guarded blocks in each alpha-tested blend program
(resource-porter weather_fog_021::vertex_blend::blend_cut): same manifest operations,
every other file byte for byte, and in each changed program every @3 instruction in its
order, the temporaries raised to 10, and only two runs of new instructions, right before
and right after the blend-aware alpha `mad a, W, a - 1, 1`. The runs write r8 and r9,
and the weight and alpha only under their guard, whose selectors are the strengths
-21069 (linear) and (-2^26, -2^25] (masked).
"""
import argparse
import json
import struct
from pathlib import Path

ONE = struct.unpack('<I', struct.pack('<f', 1.0))[0]
SELECTORS = {struct.unpack('<I', struct.pack('<f', v))[0] for v in (-21069.0, -33554432.0, -67108864.0)}
PROGRAMS = 40
LINEAR = struct.unpack('<I', struct.pack('<f', -21069.0))[0]


def code(blob):
    at = 4 + struct.unpack_from('<I', blob, 0)[0] * 264
    return blob[:at], blob[at:]


def chunks(dxbc):
    assert dxbc[:4] == b'DXBC' and struct.unpack_from('<I', dxbc, 24)[0] == len(dxbc)
    out = []
    for i in range(struct.unpack_from('<I', dxbc, 28)[0]):
        at = struct.unpack_from('<I', dxbc, 32 + 4 * i)[0]
        size = struct.unpack_from('<I', dxbc, at + 4)[0]
        out.append((dxbc[at:at + 4], dxbc[at + 8:at + 8 + size]))
    return out


def instructions(shdr):
    words = struct.unpack(f'<{len(shdr) // 4}I', shdr)
    assert words[1] == len(words)
    out, at = [], 2
    while at < len(words):
        n = words[at + 1] if words[at] & 0x7ff == 0x35 else (words[at] >> 24) & 0x7f
        assert n > 0
        out.append(words[at:at + n])
        at += n
    return out


def is_mix(op):
    return len(op) == 9 and op[0] == 0x09000032 and op[7:] == (0x4001, ONE)


def temps_written(op):
    """Temporary registers an instruction's destination names (none for control flow)."""
    code_ = op[0] & 0x7ff
    if code_ in (0x15, 0x1f) or len(op) < 3:
        return set()
    tok = op[1]
    return {op[2]} if (tok >> 12) & 0xff == 0 and (tok >> 20) & 3 == 1 else set()


def check_program(old, new, path):
    (oh, oc), (nh, nc) = code(old), code(new)
    assert oh == nh, f'{path}: wrapper changed'
    oc_, nc_ = chunks(oc), chunks(nc)
    assert [t for t, _ in oc_] == [t for t, _ in nc_], path
    for (tag, a), (_, b) in zip(oc_, nc_):
        if tag not in (b'SHDR', b'SHEX'):
            assert a == b, f'{path}: {tag} changed'
    shdr = next(i for i, (t, _) in enumerate(oc_) if t in (b'SHDR', b'SHEX'))
    a, b = instructions(oc_[shdr][1]), instructions(nc_[shdr][1])
    # every old instruction, in order; dcl_temps raised to 10
    runs, i, current = [], 0, None
    for op in b:
        if i < len(a) and (op == a[i] or (op[0] & 0x7ff == 0x68 and a[i][0] & 0x7ff == 0x68)):
            if op[0] & 0x7ff == 0x68:
                assert a[i][1] <= 8 and op[1] == 10, f'{path}: temporaries'
            current = None
            i += 1
            continue
        if current is None:
            current = (i, [])
            runs.append(current)
        current[1].append(op)
    assert i == len(a), f'{path}: an @3 instruction is missing'
    mix = next(k for k, op in enumerate(a) if is_mix(op))
    assert [r[0] for r in runs] == [mix, mix + 1], f'{path}: new instructions outside the two places'
    w, alpha = a[mix][4], a[mix][2]
    found = []
    for _, ops in runs:
        depth, selectors = 0, set()
        for op in ops:
            c = op[0] & 0x7ff
            depth += c == 0x1f
            depth -= c == 0x15
            assert depth >= 0, path
            written = temps_written(op)
            assert written <= {8, 9} or (depth > 0 and written <= {w, alpha}), f'{path}: writes r{written} unguarded'
            if c in (0x18, 0x1d, 0x31):
                literals = {op[k + 1] for k in range(1, len(op) - 1) if op[k] == 0x4001}
                assert literals and literals <= SELECTORS, f'{path}: a guard compares with another value'
                selectors |= literals
        assert depth == 0, f'{path}: unbalanced guard'
        found.append(selectors)
    linear = {LINEAR}
    assert found[0] == found[1] and found[0] in (linear, SELECTORS), f'{path}: selectors {found}'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packs', type=Path, required=True)
    args = parser.parse_args()
    three, four = args.packs / 'v021d-vertex-blend-3', args.packs / 'v021d-vertex-blend-4'
    m3, m4 = (json.loads((p / 'pack.json').read_text()) for p in (three, four))
    assert (m4['id'], m4['version']) == ('v021d-vertex-blend', '4')
    assert m4['operations'] == m3['operations'] and m4['requires'] == m3['requires']
    changed = 0
    for op in m3['operations']:
        old = (three / 'files' / op['path']).read_bytes()
        new = (four / 'files' / op['path']).read_bytes()
        if old != new:
            check_program(old, new, op['path'])
            changed += 1
    assert changed == PROGRAMS, f'{changed} programs changed, {PROGRAMS} expected'
    print(f'v021d-vertex-blend@4: {changed} programs carry the 0.69d cut, every other file is @3\'s')


if __name__ == '__main__':
    main()
