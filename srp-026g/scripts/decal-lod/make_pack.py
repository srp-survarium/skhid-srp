#!/usr/bin/env python3
"""make_pack.py: the content pack of the decal shader whose mip choice no longer flickers
(2026-10-25; ../../docs/notes/decal-lod.md).

usage: python3 -I make_pack.py --stock <folder> --out <packs folder>/decal-lod-<version>

--stock  the stock resources.db's files at their game paths (no Skhid patch carries them;
         `srp-game-data get --db <resources.db> --out <folder> <path>` or `vostok-port extract`):
         the shader of SHADER.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

A projected decal (a map's graffiti, newspapers, dirt) is drawn by `deferred_decal.ps`: it reads
the pixel's depth from the G-buffer's `$user$position` target, a DXGI_FORMAT_R16_FLOAT
(renderer_context_targets::create_targets 0x162DA0: rt 9, format 0x36), rebuilds the point as
depth x eye ray, takes it into the decal's box and samples the decal's texture with `sample`, so
the GPU picks the mip from the screen differences of that rebuilt point. A half float keeps 11
bits: the depth of a wall seen at an angle comes in steps a few pixels wide (1.6 cm from 16 m,
3.1 cm from 32 m), every step edge makes a jump in the differences and the mip changes there: the
texture is drawn in bands of two mip levels, which crawl over it as the player moves, and every
depth edge inside the box (a pipe, a plank, a window frame) takes the smallest mip. On Cologne's
graffiti, thin high-contrast lines on walls, that reads as flickering.

The pack's shader is the stock blob with the gradients given: `sample` becomes `sample_d` with the
texture-coordinate differences of the decal plane met by the pixel's and its neighbour's eye rays,
times the pixel's depth (a smooth function of the screen position; the depth only scales it, so
its steps no longer show). Inserted after the depth is read, in registers r4-r9 (the stock code
uses r0-r3; dcl_temps 4 -> 10):

    Rw = V2W rotation (cb1[11..13].xyz) x R             R: the eye ray, z 1 (r2)
    Rd = world_to_decal (cb0[0..2].xyz) x Rw            the ray in the decal's box frame
    gx = world_to_decal x V2W x (1, 0, 0), gy = ... x (0, 1, 0)
    s  = depth x eye_ray_corner.xy / (screen_res.xy x eye_ray_corner.z)   one pixel's ray step
    ddx(tc) = s.x (gx.xy - Rd.xy gx.z / Rd.z),  ddy(tc) = s.y (gy.xy - Rd.xy gy.z / Rd.z), u negated

(tc is 0.5 x the box's x and -y, and a pixel's ray step is -2 corner / (screen_res corner.z): the
0.5 and the 2 cancel, the two minus signs of v cancel, u keeps one.) The gradients equal the
differences of tc between neighbouring pixels on a plane: checked by running the patched program
on a synthetic camera and decal. The rest of the shader is byte for byte the stock one; the container's
checksum is recomputed (the DXBC MD5 variant, checked against every stock blob read here).
Only the permutation every graffiti material uses is replaced (diffuse texture only: no normal,
fresnel or roughness texture), `_____10______00`; it also draws the other decals made the same
way (dirt, moss, bullet marks of the maps).
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

PACK_ID = 'decal-lod'
VERSION = '1'
SHADER = 'shaders/sm_4_0/deferred_decal.ps/_____10______00____________________________________________________'
STOCK_SHA256 = '65edeb5e023425f11be719d4f2457afd2f29c700049ff7b31032a4a430e4b602'

# --- the DXBC container -------------------------------------------------------------------------

_S = [7, 12, 17, 22] * 4 + [5, 9, 14, 20] * 4 + [4, 11, 16, 23] * 4 + [6, 10, 15, 21] * 4
_K = [int(abs(math.sin(i + 1)) * 2 ** 32) & 0xffffffff for i in range(64)]


def _md5_block(state, block):
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
            f, g = c ^ (b | ~d), (7 * i) % 16
        f = (f + a + _K[i] + x[g]) & 0xffffffff
        a, d, c = d, c, b
        b = (b + ((f << _S[i]) | (f >> (32 - _S[i])))) & 0xffffffff
    return [(p + q) & 0xffffffff for p, q in zip(state, (a, b, c, d))]


def dxbc_checksum(blob):
    """The checksum D3D checks at bytes 4..20: MD5 of everything after them, with the length
    moved to the front of the last block and (bits >> 2) | 1 at its end."""
    data = blob[20:]
    bits = len(data) * 8
    state = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476]
    full = len(data) // 64 * 64
    for o in range(0, full, 64):
        state = _md5_block(state, data[o:o + 64])
    rest = data[full:]
    tail = struct.pack('<I', (bits >> 2) | 1)
    if len(rest) >= 56:
        state = _md5_block(state, rest + b'\x80' + bytes(63 - len(rest)))
        state = _md5_block(state, struct.pack('<I', bits) + bytes(52) + tail)
    else:
        block = struct.pack('<I', bits) + rest + b'\x80'
        state = _md5_block(state, block + bytes(60 - len(block)) + tail)
    return struct.pack('<4I', *state)


def chunks(blob):
    assert blob[:4] == b'DXBC'
    n = struct.unpack_from('<I', blob, 28)[0]
    out = []
    for i in range(n):
        off = struct.unpack_from('<I', blob, 32 + 4 * i)[0]
        size = struct.unpack_from('<I', blob, off + 4)[0]
        out.append((blob[off:off + 4], blob[off + 8:off + 8 + size]))
    return out


def container(parts, like):
    """A DXBC of these chunks, the header (version) of `like`, its checksum."""
    head = 32 + 4 * len(parts)
    offs, body = [], b''
    for tag, data in parts:
        offs.append(head + len(body))
        body += tag + struct.pack('<I', len(data)) + data
    blob = bytearray(b'DXBC' + bytes(16) + like[20:24] + struct.pack('<II', head + len(body), len(parts)))
    blob += struct.pack('<%dI' % len(parts), *offs) + body
    blob[4:20] = dxbc_checksum(bytes(blob))
    return bytes(blob)


def instructions(shdr):
    """The SM4 program as (version, [token lists])."""
    ver, length = struct.unpack_from('<II', shdr, 0)
    out, t = [], 8
    while t < length * 4:
        tok = struct.unpack_from('<I', shdr, t)[0]
        n = struct.unpack_from('<I', shdr, t + 4)[0] if tok & 0x7ff == 0x35 else (tok >> 24) & 0x7f
        out.append(list(struct.unpack_from('<%dI' % n, shdr, t)))
        t += 4 * n
    assert t == length * 4
    return ver, out


def program(ver, ins):
    body = b''.join(struct.pack('<%dI' % len(i), *i) for i in ins)
    return struct.pack('<II', ver, 2 + len(body) // 4) + body


# --- SM4 tokens ---------------------------------------------------------------------------------

MOV, MUL, MAD, DP3, DIV, MAX, SAMPLE, SAMPLE_D, DCL_TEMPS = 0x36, 0x38, 0x32, 0x10, 0x0E, 0x34, 0x45, 0x49, 0x68
COMP = 'xyzw'


def op(code, *operands):
    toks = [t for o in operands for t in o]
    return [code | ((1 + len(toks)) << 24)] + toks


def _swz(s):
    s = (s * 4)[:4] if len(s) < 4 else s
    return sum(COMP.index(c) << (2 * k) for k, c in enumerate(s))


def r(n, sel, dst=False, neg=False):
    """A temp: as a destination `sel` is the write mask, as a source a swizzle."""
    if dst:
        tok = 2 | (sum(1 << COMP.index(c) for c in sel) << 4) | (1 << 20)
    else:
        tok = 2 | (1 << 2) | (_swz(sel) << 4) | (1 << 20)
    if neg:
        return [tok | 0x80000000, 0x41, n]
    return [tok, n]


def cb(b, reg, sel):
    return [2 | (1 << 2) | (_swz(sel) << 4) | (8 << 12) | (2 << 20), b, reg]


def imm(*v):
    return [2 | (4 << 12)] + [struct.unpack('<I', struct.pack('<f', x))[0] for x in (v * 4)[:4]]


def gradients():
    """ddx(tc) in r4.xy, ddy(tc) in r5.xy; needs r1.z = depth, r2.xyz = the eye ray (z 1)."""
    code = []
    for k, c in enumerate('xyz'):                       # Rw: the ray in the world
        code.append(op(DP3, r(6, c, dst=True), cb(1, 11 + k, 'xyzx'), r(2, 'xyzx')))
    for k, c in enumerate('xyz'):                       # Rd: in the decal's box
        code.append(op(DP3, r(7, c, dst=True), cb(0, k, 'xyzx'), r(6, 'xyzx')))
    for col, g in (('x', 6), ('y', 9)):                 # gx, gy: the screen axes in the box
        for k, c in enumerate('xyz'):
            code.append(op(MOV, r(8, c, dst=True), cb(1, 11 + k, col)))
        for k, c in enumerate('xyz'):
            code.append(op(DP3, r(g, c, dst=True), cb(0, k, 'xyzx'), r(8, 'xyzx')))
    code += [
        op(MUL, r(8, 'x', dst=True), r(7, 'z'), r(7, 'z')),
        op(MAX, r(8, 'x', dst=True), r(8, 'x'), imm(1e-8)),
        op(DIV, r(8, 'x', dst=True), r(7, 'z'), r(8, 'x')),             # 1 / Rd.z, kept finite
        op(DIV, r(8, 'yz', dst=True), cb(0, 8, 'xxyx'), cb(1, 14, 'xxyx')),
        op(DIV, r(8, 'yz', dst=True), r(8, 'xyzx'), cb(0, 8, 'z')),
        op(MUL, r(8, 'yz', dst=True), r(8, 'xyzx'), r(1, 'z')),         # one pixel's step x depth
        op(MUL, r(8, 'w', dst=True), r(6, 'z'), r(8, 'x')),
        op(MAD, r(4, 'xy', dst=True), r(7, 'xyxx', neg=True), r(8, 'w'), r(6, 'xyxx')),
        op(MUL, r(4, 'xy', dst=True), r(4, 'xyxx'), r(8, 'y')),
        op(MUL, r(4, 'x', dst=True), r(4, 'x', neg=True), imm(1.0)),
        op(MUL, r(8, 'w', dst=True), r(9, 'z'), r(8, 'x')),
        op(MAD, r(5, 'xy', dst=True), r(7, 'xyxx', neg=True), r(8, 'w'), r(9, 'xyxx')),
        op(MUL, r(5, 'xy', dst=True), r(5, 'xyxx'), r(8, 'z')),
        op(MUL, r(5, 'x', dst=True), r(5, 'x', neg=True), imm(1.0)),
    ]
    return code


def patch(blob):
    parts = chunks(blob)
    tags = [t for t, _ in parts]
    assert tags == [b'RDEF', b'ISGN', b'OSGN', b'SHDR', b'STAT'], tags
    ver, ins = instructions(dict(parts)[b'SHDR'])
    codes = [i[0] & 0x7ff for i in ins]
    temps = [k for k, c in enumerate(codes) if c == DCL_TEMPS]
    samples = [k for k, c in enumerate(codes) if c == SAMPLE]
    assert len(temps) == 1 and ins[temps[0]] == [DCL_TEMPS | (2 << 24), 4], 'stock dcl_temps 4'
    assert len(samples) == 1, 'one diffuse sample'
    # the eye ray and the point: div r2.xy, r0.zw, cb0[8].z / mov r2.z, 1 / mul r1.xyz, r1.x, r2.xyz
    at = codes.index(MUL, codes.index(MOV))
    assert ins[at] == op(MUL, r(1, 'xyz', dst=True), r(1, 'x'), r(2, 'xyzx')), 'stock point rebuild'
    s = ins[samples[0]]
    assert s == [SAMPLE | (9 << 24), 0x001000f2, 0, 0x00100046, 0, 0x00107e46, 0, 0x00106000, 0], 'stock sample'
    new = list(ins)
    new[samples[0]] = op(SAMPLE_D, s[1:3], s[3:5], s[5:7], s[7:9], r(4, 'xyxx'), r(5, 'xyxx'))
    new[temps[0]] = [DCL_TEMPS | (2 << 24), 10]
    new[at + 1:at + 1] = gradients()
    out = [(t, program(ver, new) if t == b'SHDR' else d) for t, d in parts]
    return container(out, blob), len(ins), len(new)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--stock', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    stock = (args.stock / SHADER).read_bytes()
    if sha256(stock) != STOCK_SHA256:
        sys.exit(f'{SHADER}: not the stock shader (sha256 {sha256(stock)})')
    at = stock.index(b'DXBC')
    head, blob = stock[:at], stock[at:]   # the dependency table, then the container to the end
    assert struct.unpack_from('<I', blob, 24)[0] == len(blob), 'the container ends the file'
    assert blob[4:20] == dxbc_checksum(blob), 'the checksum reads the stock blob'
    assert container(chunks(blob), blob) == blob, 'the container writer rebuilds the stock blob'
    ver, ins = instructions(chunks(blob)[3][1])
    assert program(ver, ins) == chunks(blob)[3][1], 'the program reader rebuilds the stock program'
    patched, n0, n1 = patch(blob)
    data = head + patched
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'projected decals choose their mip from the decal plane, not from the half-float '
                       'depth: no crawling mip bands on graffiti (deferred_decal.ps, diffuse-only permutation)',
        'operations': [{'op': 'replace_file', 'path': SHADER, 'base_sha256': STOCK_SHA256}],
        'notes': ['made by srp-026g/scripts/decal-lod/make_pack.py from the stock resources.db\'s shader'],
    }
    target = args.out / 'files' / SHADER
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{SHADER}: {n0} -> {n1} instructions, {len(stock)} -> {len(data)} bytes, sha256 {sha256(data)}')
    print(f'{args.out}: pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
