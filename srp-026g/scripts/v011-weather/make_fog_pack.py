"""make_fog_pack.py DONOR_DB STOCK_DB VOSTOK_PORT OUT: the pack v011e-radar-fog, 0.11e's additive
geometry fog in 0.26g's own additive fog program.

0.11e fogs the geometry in two passes of its effect_atmospheric_scattering: technique 6 multiplies the
frame by 1 - min(depth, D) * 0.1 / D (`atmospheric_scattering_on_geometry_mul.ps`), technique 7 adds the
in-scattered light read from the Rayleigh and Mie tables (`..._add.ps`), D the weather's
`atmosphere_inscatter_distance`. 0.26g compiles the same two techniques (3 and 4) but draws one pass of
its own (5, `..._temporary.ps`); exe patch v011w_render draws 3 and 4 on a 0.11 weather map's scene.
0.26g's multiplicative program is 0.11e's with 0.26g's constant layout; its additive one is another
equation (an anti-sun lobe, a fixed table texel). This pack gives 0.26g's additive file 0.11e's
program, moved to 0.26g's layout: nothing else draws technique 4, so no other map changes.

The move, every operand of 0.11e's program (nothing else changes):
- $Globals: to_sun_direction_parameter cb0[5] -> cb0[4], inscatter_parameters cb0[6] -> cb0[5],
  s_eye_ray_corner cb0[7] -> cb0[6] (0.26g's add.ps reflection, which the file keeps);
- static_globals: eye_position cb1[15] -> cb1[20], screen_res cb1[18] -> cb1[14] (m_V2W cb1[11..13]
  is at the same place);
- the tables: 0.11e binds t_rayleigh_scattering at t1 and t_mie_scattering at t2, 0.26g the reverse.
The container (header, RDEF, ISGN, OSGN, STAT) is 0.26g's file's; the checksum is made again.
"""
import hashlib, json, struct, subprocess, sys, tempfile
from pathlib import Path

FAMILY = 'shaders/sm_4_0/atmospheric_scattering_on_geometry_add.ps/'
DONOR_SHA = '4e925f474bb4d6d5ef8e540ac6ec077320874c990d4f8790b3a62fd28cee8d4d'
STOCK_SHA = 'd9334ae5a9c103f5a9b0569d2bb7f5d26705ce68451e02efc503b8d84a952a8a'
STOCK_DB_SHA = 'c74078be702d17113b9ca23f59aa2b0087238481e7f01fb793620d51d875492c'
CB_MOVES = {(0, 5): (0, 4), (0, 6): (0, 5), (0, 7): (0, 6), (1, 15): (1, 20), (1, 18): (1, 14)}
CB_SIZES = {0: 7, 1: 21}
TEXTURES = {1: 2, 2: 1}


def _compress(state, block):
    import math
    k = [int(abs(math.sin(i + 1)) * 2 ** 32) & 0xFFFFFFFF for i in range(64)]
    s = [7, 12, 17, 22] * 4 + [5, 9, 14, 20] * 4 + [4, 11, 16, 23] * 4 + [6, 10, 15, 21] * 4
    a, b, c, d = state
    x = struct.unpack('<16I', block)
    for i in range(64):
        if i < 16:
            f, g = (b & c) | (~b & d), i
        elif i < 32:
            f, g = (d & b) | (~d & c), (5 * i + 1) % 16
        elif i < 48:
            f, g = b ^ c ^ d, (3 * i + 5) % 16
        else:
            f, g = c ^ (b | ~d & 0xFFFFFFFF), (7 * i) % 16
        f = (f + a + k[i] + x[g]) & 0xFFFFFFFF
        a, d, c = d, c, b
        b = (b + (((f << s[i]) | (f >> (32 - s[i]))) & 0xFFFFFFFF)) & 0xFFFFFFFF
    return [(p + q) & 0xFFFFFFFF for p, q in zip(state, (a, b, c, d))]


def checksum(container):
    """The DXBC container's hash: MD5 of all after its first 20 bytes, with the format's own padding."""
    data = container[20:]
    bits = len(data) * 8
    state = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476]
    full = len(data) - len(data) % 64
    for i in range(0, full, 64):
        state = _compress(state, data[i:i + 64])
    last = data[full:]
    tail = struct.pack('<I', (bits >> 2) | 1)
    if len(last) >= 56:
        state = _compress(state, last + b'\x80' + b'\0' * (63 - len(last)))
        state = _compress(state, struct.pack('<I', bits) + b'\0' * 56 + tail)
    else:
        state = _compress(state, struct.pack('<I', bits) + last + b'\x80' + b'\0' * (55 - len(last)) + tail)
    return struct.pack('<4I', *state)


def split(blob):
    """(the resource's header, the container's chunks as [(tag, bytes)])"""
    n = struct.unpack_from('<I', blob)[0]
    head, code = blob[:4 + n * 264], blob[4 + n * 264:]
    assert code[:4] == b'DXBC' and checksum(code) == code[4:20], 'not a whole DXBC container'
    count = struct.unpack_from('<I', code, 28)[0]
    chunks = []
    for i in range(count):
        at = struct.unpack_from('<I', code, 32 + 4 * i)[0]
        size = struct.unpack_from('<I', code, at + 4)[0]
        chunks.append((code[at:at + 4], code[at + 8:at + 8 + size]))
    return head, chunks


def join(head, chunks):
    table = 32 + 4 * len(chunks)
    body, offsets = b'', []
    for tag, data in chunks:
        offsets.append(table + len(body))
        body += tag + struct.pack('<I', len(data)) + data
    code = bytearray(b'DXBC' + b'\0' * 16 + struct.pack('<III', 1, table + len(body), len(chunks))
                     + b''.join(struct.pack('<I', o) for o in offsets) + body)
    code[4:20] = checksum(bytes(code))
    return head + bytes(code)


def operands(ins, at, out, move):
    """Copy the operand at ins[at:] into out, moved; returns the index after it."""
    token = ins[at]
    kind, dims = (token >> 12) & 0xFF, (token >> 20) & 3
    out.append(token)
    at += 1
    if token & 0x80000000:
        out.append(ins[at])
        at += 1
    reps = [(token >> (22 + 3 * i)) & 7 for i in range(dims)]
    assert all(r == 0 for r in reps), 'only immediate indices'
    index = list(ins[at:at + dims])
    at += dims
    if kind == 8:                                   # a constant buffer element [cb, register]
        index = list(CB_MOVES.get(tuple(index), tuple(index)))
    elif kind == 7:                                 # a texture
        index = [TEXTURES.get(index[0], index[0])]
    out.extend(index)
    if kind in (4, 5):                              # immediates
        n = {0: 0, 1: 1, 2: 4}[token & 3]
        out.extend(ins[at:at + n])
        at += n
    return at


def move_program(shdr):
    words = list(struct.unpack('<%dI' % (len(shdr) // 4), shdr))
    out = words[:2]
    at = 2
    while at < len(words):
        op = words[at]
        length = (op >> 24) & 0x7F
        code = op & 0x7FF
        ins = words[at:at + length]
        assert length and code != 0x35, 'no custom data in this program'
        new = [op]
        i = 1
        while ins[i - 1] & 0x80000000:              # extended opcode tokens
            new.append(ins[i])
            i += 1
        if code == 0x59:                            # dcl_constantBuffer cbN[size]
            token, cb, size = ins[1], ins[2], ins[3]
            new = [op, token, cb, CB_SIZES[cb]]
        elif code == 0x58:                          # dcl_resource tN + its return type
            new = [op, ins[1], TEXTURES.get(ins[2], ins[2]), ins[3]]
        elif code in (0x5A, 0x64, 0x65, 0x62, 0x63, 0x68, 0x6A):  # samplers, inputs, outputs, temps
            new = list(ins)
        else:
            while i < length:
                i = operands(ins, i, new, None)
        assert len(new) == length, (hex(op), new, ins)
        out.extend(new)
        at += length
    assert out[1] == len(out)
    return struct.pack('<%dI' % len(out), *out)


def only(port, db):
    """The family's one file in db (0.11e's key is a character longer than 0.26g's)."""
    paths = subprocess.run([str(port), 'list', '--db', str(db), '--prefix', FAMILY], check=True,
                           capture_output=True, text=True).stdout.split()
    assert len(paths) == 1, paths
    return paths[0]


def extract(port, db, path):
    with tempfile.TemporaryDirectory() as tmp:
        dest = Path(tmp) / 'x'
        subprocess.run([str(port), 'extract', '--db', str(db), '--path', path, '--output', str(dest)],
                       check=True, stdout=subprocess.DEVNULL)
        return dest.read_bytes()


def build(donor_db, stock_db, port):
    donor = extract(port, donor_db, only(port, donor_db))
    path = only(port, stock_db)
    stock = extract(port, stock_db, path)
    assert hashlib.sha256(donor).hexdigest() == DONOR_SHA, 'not 0.11e\'s additive fog program'
    assert hashlib.sha256(stock).hexdigest() == STOCK_SHA, 'not 0.26g\'s additive fog program'
    _, original = split(donor)
    head, native = split(stock)
    program = move_program(dict(original)[b'SHDR'])
    chunks = [(tag, program if tag == b'SHDR' else data) for tag, data in native]
    assert [t for t, _ in chunks] == [t for t, _ in original], 'the same chunks'
    return path, stock, join(head, chunks)


def main():
    donor_db, stock_db, port, out = map(Path, sys.argv[1:5])
    assert not out.exists(), 'the output folder must be new'
    path, stock, new = build(donor_db, stock_db, port)
    (out / 'files' / path).parent.mkdir(parents=True)
    (out / 'files' / path).write_bytes(new)
    pack = {
        'format_version': 1, 'id': 'v011e-radar-fog', 'version': '1', 'target_build': '0.26g',
        'description': "0.11e's additive geometry fog in 0.26g's own additive fog program (technique 4 of "
                       "effect_atmospheric_scattering, which only exe patch v011w_render draws, on Vostok v0.11's "
                       "evening and rain)",
        'requires': [], 'base_db_sha256': STOCK_DB_SHA,
        'operations': [{'op': 'replace_file', 'path': path, 'base_sha256': hashlib.sha256(stock).hexdigest()}],
        'server': None,
        'notes': ['Built by srp-026g/scripts/v011-weather/make_fog_pack.py from 0.11e\'s and 0.26g\'s files '
                  '(both pinned): 0.11e\'s instructions with every operand moved to 0.26g\'s constant and '
                  'texture places, in 0.26g\'s container; check_fog_pack.py checks it.'],
    }
    (out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(hashlib.sha256((out / 'pack.json').read_bytes()).hexdigest(), out)


if __name__ == '__main__':
    main()
