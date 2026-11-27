"""Independent complete dirt-family alpha delta and native isolation check."""
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import tempfile

PREFIX = 'shaders/sm_4_0/gbuffer_pass.ps/'
FAMILY = re.compile(r'_([03])___1000_10001_00____(\d+)_____0__(0_021|2_121)00____00000____000__________0__$')

def chunks(blob):
    offset = 4 + struct.unpack_from('<I', blob)[0] * 264
    code = blob[offset:]
    assert code[:4] == b'DXBC' and struct.unpack_from('<I', code, 24)[0] == len(code)
    result = {}
    for i in range(struct.unpack_from('<I', code, 28)[0]):
        at = struct.unpack_from('<I', code, 32 + i * 4)[0]
        size = struct.unpack_from('<I', code, at + 4)[0]
        result[code[at:at + 4]] = code[at + 8:at + 8 + size]
    return blob[:offset], result

def program(raw):
    words = list(struct.unpack('<' + 'I' * (len(raw) // 4), raw))
    instructions, at = [], 2
    while at < len(words):
        n = (words[at] >> 24) & 127
        assert n
        instructions.append(words[at:at + n])
        at += n
    assert at == len(words)
    return words[:2], instructions

def alpha_reflection(raw):
    data = bytearray(raw)
    u = lambda at: struct.unpack_from('<I', data, at)[0]
    found = []
    for i in range(u(0)):
        at = u(4) + i * 24
        for j in range(u(at + 4)):
            var = u(at + 8) + j * 24
            name = u(var)
            if data[name:data.index(0, name)] == b'alpha_ref_parameter':
                assert (u(var + 4), u(var + 8), u(var + 12)) == (248, 4, 0)
                found.append(var)
    assert len(found) == 1
    struct.pack_into('<I', data, found[0] + 12, 2)
    return data

def check(porter, stock, patch, pack):
    pins = {p['vertex']: p for p in json.loads(Path(__file__).with_name('visibility-shader-inputs.json').read_text())}
    assert set(pins) == {0, 1, 2, 10, 12, 13, 14}
    manifest = json.loads((pack / 'pack.json').read_text())
    native_paths = set(subprocess.check_output([str(porter), 'list', '--db', str(stock)], text=True).splitlines())
    # Enumerate every shipped path for this feature family, not only paths
    # which happened to fail during a previous native map load.
    family = {p: FAMILY.fullmatch(p[len(PREFIX):]) for p in native_paths if p.startswith(PREFIX)}
    family = {p: m for p, m in family.items() if m}
    assert {int(m[2]) for m in family.values()} == set(pins)
    expected = {p[:len(PREFIX) + 8] + '1' + p[len(PREFIX) + 9:] for p in family}
    assert {op['path'] for op in manifest['operations']} == expected
    assert all(op['op'] == 'add_file' for op in manifest['operations'])
    assert expected.isdisjoint(native_paths), 'existing native shader would change'
    with tempfile.TemporaryDirectory() as tmp:
        cache = {}
        def read(db, path):
            key = (db, path)
            if key not in cache:
                target = Path(tmp) / str(len(cache))
                subprocess.run([str(porter), 'extract', '--db', str(db), '--path', path,
                                '--output', str(target)], check=True, stdout=subprocess.DEVNULL)
                cache[key] = target.read_bytes()
            return cache[key]
        for source, match in sorted(family.items()):
            quality, vertex, alias = match.groups()
            pin = pins[int(vertex)]
            key = source[len(PREFIX):]
            path = PREFIX + key[:8] + '1' + key[9:]
            actual = (pack / 'files' / path).read_bytes()
            if patch is not None:
                assert read(patch, path) == actual, 'composed shader differs from pack'
            native = read(stock, source)
            assert hashlib.sha256(native).hexdigest() == pin['low' if quality == '0' else 'high']
            plain = PREFIX + key[:10] + '0' + key[11:]
            low_alpha = PREFIX + key[:8] + '1' + key[9:10] + '0' + key[11:]
            if quality == '0':
                assert native == read(stock, plain), 'native low parallax changed'
                alpha = read(stock, low_alpha)
                assert hashlib.sha256(alpha).hexdigest() == pin['alpha'] and actual == alpha
                continue
            normal = PREFIX + key[:6] + '1' + key[7:8] + '1' + key[9:]
            reference = read(stock, normal)
            assert hashlib.sha256(reference).hexdigest() == pin['normal']
            _, ref_ops = program(chunks(reference)[1][b'SHDR'])
            old_header, old = chunks(native)
            new_header, new = chunks(actual)
            assert old_header == new_header and old.keys() == new.keys()
            assert all(old[k] == new[k] for k in old if k not in (b'SHDR', b'RDEF'))
            header, before = program(old[b'SHDR'])
            if pin['mask'] == 'vertex':
                cutoff = [0x07000000, 0x00100082, 0, 0x0010003a, 0, 0x00004001, 0xbecccccd]
                assert before.count(cutoff) == 1
                assert [0x0b000032, 0x00100042, 0, 0x0010002a, 0, 0x0010000a, 2, 0x8020802a, 0x41, 1, 15] in ref_ops
                before[before.index(cutoff)] = [0x0b000032, 0x00100082, 0, 0x0010003a, 0, 0x0010003a, 2, 0x8020802a, 0x41, 1, 15]
            else:
                assert pin['mask'] == 'zero'
                refs = [i for i, op in enumerate(ref_ops) if len(op) == 9 and op[0] == 0x09000031
                        and op[3:] == [0x8020802a, 0x41, 1, 15, 0x00004001, 0]]
                assert len(refs) == 1 and ref_ops[refs[0] + 1][0] == 0x0304000d
                cutoff = [0x0304000d, 0x00004001, 0xffffffff]
                assert before.count(cutoff) == 1
                at = before.index(cutoff)
                temp = next(op for op in before if op[0] & 2047 == 104)
                scratch = temp[1]
                temp[1] += 1
                before[at:at + 1] = [[0x09000031, 0x00100012, scratch, 0x8020802a, 0x41, 1, 15, 0x00004001, 0],
                                     [0x0304000d, 0x0010000a, scratch]]
            expected_code = header + [word for op in before for word in op]
            expected_code[1] = len(expected_code)
            assert new[b'SHDR'] == struct.pack('<' + 'I' * len(expected_code), *expected_code), 'unrelated native code changed'
            assert new[b'RDEF'] == alpha_reflection(old[b'RDEF']), 'unrelated binding changed'
    print('Visibility shader closure: all28 absent paths across7 native vertex classes; shipped alpha formulas, native low identities and unchanged unrelated instructions/bindings verified')
