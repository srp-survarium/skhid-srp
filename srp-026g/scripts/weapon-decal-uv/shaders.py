"""Patch the 0.26g one-weight SM4 shaders while preserving their existing program and reflection."""
import struct

from dxbc_checksum import checksum


def words(*values):
    return struct.pack(f'<{len(values)}I', *values)


def uint(data, offset):
    return struct.unpack_from('<I', data, offset)[0]


def store(data, offset, value):
    struct.pack_into('<I', data, offset, value)


def string(data, offset):
    return data[offset:data.index(0, offset)].decode('ascii')


def float_bits(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


class Program:
    def __init__(self, resource):
        pos = 4 + 264 * uint(resource, 0)
        self.header, code = resource[:pos], resource[pos:]
        if code[:4] != b'DXBC' or uint(code, 24) != len(code) or checksum(code) != code[4:20]:
            raise ValueError('invalid game shader container')
        self.chunks = {}
        for i in range(uint(code, 28)):
            offset = uint(code, 32 + 4*i)
            tag, length = struct.unpack_from('<4sI', code, offset)
            self.chunks[tag] = bytearray(code[offset+8:offset+8+length])
        data = self.chunks[b'SHDR']
        tokens = list(struct.unpack(f'<{len(data)//4}I', data))
        self.version = tokens[0]
        if tokens[1] != len(tokens):
            raise ValueError('invalid token count')
        self.ops = []
        i = 2
        while i < len(tokens):
            length = (tokens[i] >> 24) & 127
            if not length or i+length > len(tokens):
                raise ValueError('invalid instruction length')
            self.ops.append(tokens[i:i+length])
            i += length
        self.original_ops = len(self.ops)

    def temp(self):
        declaration = next(op for op in self.ops if op[0] == 0x02000068)
        scratch = declaration[1]
        declaration[1] += 1
        return scratch

    def declare(self, instruction):
        i = next(i for i, op in enumerate(self.ops) if op[0] == 0x02000068)
        self.ops.insert(i, instruction)

    def varying(self, register, output):
        tag = b'OSGN' if output else b'ISGN'
        sig = self.chunks[tag]
        count, start = struct.unpack_from('<II', sig)
        if start != 8:
            raise ValueError('expected an SM4 signature')
        records_end = start + 24*count
        records = bytearray(sig[start:records_end])
        texcoord = None
        for off in range(0, len(records), 24):
            name = uint(records, off)
            if uint(records, off+16) == register or (
                    string(sig, name) == 'TEXCOORD' and uint(records, off+4) == register):
                raise ValueError(f'varying {register} already in use')
            if string(sig, name) == 'TEXCOORD':
                texcoord = name + 24
            store(records, off, name+24)
        if texcoord is None:
            raise ValueError('expected TEXCOORD semantics')
        records.extend(struct.pack('<5IBBH', texcoord, register, 0, 3, register, 3, 12 if output else 3, 0))
        self.chunks[tag] = words(count+1, start) + records + sig[records_end:]
        if output:
            self.declare(ins(101, dest(register, 3, OUTPUT)))
        else:
            self.declare(ins(98 | (2 << 11), dest(register, 3, INPUT)))  # linear interpolation

    def vertex_input(self, name, needed):
        sig = self.chunks[b'ISGN']
        for i in range(uint(sig, 0)):
            off = 8+i*24
            if string(sig, uint(sig, off)) != name:
                continue
            register = uint(sig, off+16)
            if sig[off+20] & needed != needed:
                raise ValueError(f'{name} lacks the requested components')
            previous = sig[off+21]
            decl = ins(95, dest(register, previous, INPUT))
            self.ops[self.ops.index(decl)] = ins(95, dest(register, previous | needed, INPUT))
            sig[off+21] |= needed
            return register
        raise ValueError(f'missing input {name}')

    def finish(self):
        tokens = [word for op in self.ops for word in op]
        self.chunks[b'SHDR'] = words(self.version, 2+len(tokens), *tokens)
        stats = self.chunks[b'STAT']
        store(stats, 0, uint(stats, 0) + len(self.ops) - self.original_ops)
        store(stats, 4, next(op[1] for op in self.ops if op[0] == 0x02000068))
        code = bytearray(b'DXBC' + bytes(16) + words(1, 0, len(self.chunks)) + bytes(4*len(self.chunks)))
        for i, (tag, data) in enumerate(self.chunks.items()):
            store(code, 32+4*i, len(code))
            code.extend(tag + words(len(data)) + data)
        store(code, 24, len(code))
        code[4:20] = checksum(code)
        return self.header + code


# SM4 tokenized operands. Masks are XYZW bits; source swizzles below are used
# only on 2D coordinates or a single, explicitly selected component.
TEMP, INPUT, OUTPUT = 0, 1, 2


def ins(opcode, *operands):
    args = [word for operand in operands for word in operand]
    return [(1+len(args)) << 24 | opcode, *args]


def dest(register, mask, kind=TEMP):
    return [0x100002 | kind << 12 | mask << 4, register]


def scalar(register, component, kind=TEMP):
    return [0x10000a | kind << 12 | component << 4, register]


def pair(register, kind=TEMP):
    return [0x100046 | kind << 12, register]


def value(number):
    return [0x4001, number]


def atlas(program, scratch, tag):
    """Decode exactly the six bytes that unwrap.cpp writes. tag.x is true for a tagged vertex."""
    bone = program.vertex_input('BLENDINDICES', 14)
    normal = program.vertex_input('NORMAL', 8)
    tangent = program.vertex_input('TANGENT', 8)
    binormal = program.vertex_input('BINORMAL', 8)
    # The three frame .w inputs are UNORM8. Round their decoded floats to
    # the nearest integer before comparing the tag or reconstructing V.
    body = []
    for component, register in enumerate((normal, tangent, binormal)):
        body.append(ins(50, dest(tag, 1 << component), scalar(register, 3, INPUT),
                        value(float_bits(255)), value(float_bits(.5))))
    body.extend([
        ins(28, dest(tag, 7), [0x100e46, tag]),  # ftou XYZ
        ins(35, dest(scratch, 1), scalar(bone, 2, INPUT), value(256), scalar(bone, 1, INPUT)),
        ins(35, dest(scratch, 2), scalar(tag, 1), value(256), scalar(bone, 3, INPUT)),
        ins(32, dest(tag, 1), scalar(tag, 0), value(ord('S'))),
        ins(32, dest(tag, 4), scalar(tag, 2), value(ord('D'))),
        ins(1, dest(tag, 1), scalar(tag, 0), scalar(tag, 2)),
        ins(86, dest(scratch, 3), pair(scratch)),
        ins(56, dest(scratch, 3), pair(scratch), value(float_bits(1 / 65535))),
    ])
    return body


def bake_vertex(resource):
    p = Program(resource)
    p.varying(8, output=True)
    scratch, tag = p.temp(), p.temp()
    decode = atlas(p, scratch, tag)
    # Native: clip.xy = 2 * frac(material_uv) - 1. The native Y flip follows
    # this block; o7.xy carries the source UV for material and camouflage.
    indices = [i for i, op in enumerate(p.ops) if op[:5] == [0xf000032, 0x100032, 1, 0x100086, 0]
               and op[-4:] == [float_bits(-1), float_bits(-1), 0, 0]]
    if len(indices) != 1:
        raise ValueError('unrecognized native unwrap projection')
    at = indices[0]
    material = p.vertex_input('TEXCOORD', 3)
    old_projection, old_uv = p.ops[at:at+2]
    if old_uv != [0x5000036, 0x102032, 7, 0x100086, 0]:
        raise ValueError('unrecognized material coordinates')
    p.ops[at:at+2] = decode + [
        ins(55, dest(scratch, 3), scalar(tag, 0), pair(scratch), old_uv[3:]),
        ins(54, dest(8, 3, OUTPUT), pair(scratch)),
        old_projection[:3] + pair(scratch) + old_projection[5:],
        # Preserve unwrapped source coordinates on a triangle crossing a
        # sampler wrap boundary. Untagged assets get the stock shader result.
        ins(55, old_uv[1:3], scalar(tag, 0), pair(material, INPUT), old_uv[3:]),
    ]
    return p.finish()


def bake_pixel(resource):
    p = Program(resource)
    p.varying(8, output=False)
    # Only the accumulated decal uses atlas_uv * viewport_size_scale.
    # Camouflage, AOAC and all four base texture channels use material_uv.
    matches = [op for op in p.ops if len(op) == 8 and op[0] & 2047 == 56
               and op[3:5] == pair(7, INPUT) and op[5:] == [0x208046, 0, 8]]
    if len(matches) != 1:
        raise ValueError('unrecognized decal accumulation lookup')
    matches[0][4] = 8
    return p.finish()


def draw_vertex(resource):
    p = Program(resource)
    p.varying(15, output=True)
    scratch, tag = p.temp(), p.temp()
    if p.ops[-1] != [0x0100003e]:
        raise ValueError('unexpected end of vertex shader')
    p.ops[-1:-1] = atlas(p, scratch, tag) + [
        ins(55, dest(15, 3, OUTPUT), scalar(tag, 0), pair(scratch), value(float_bits(-1)))]
    return p.finish()


CHANNELS = ('t_base', 't_normal', 't_fresnel', 't_roughness')


def resources(reflection):
    count, start = struct.unpack_from('<II', reflection, 8)
    for i in range(count):
        offset = start + i*32
        name, kind, _, _, _, slot, length, _ = struct.unpack_from('<8I', reflection, offset)
        yield string(reflection, name), name, kind, slot, length


# The float4 of deferred_material_parameters that carries the channel flags:
# unused by every gbuffer_pass.ps program the pack patches. Every patched
# program renames the same field, so the one buffer name has one layout.
CHANNEL_FIELD = 'sequence_parameters'
CHANNEL_HOST = b'srp_weapon_bake_channels'


def channel_constant(program):
    """Give the shader its own material buffer and the named four-channel host.

    CHANNEL_FIELD becomes srp_weapon_bake_channels. A distinct buffer name
    (as long as the original) keeps updates out of the engine's ordinary
    material buffer.
    """
    r = program.chunks[b'RDEF']
    source = 'deferred_material_parameters'
    target = b'srp_dual_material_parameters'
    bindings = list(resources(r))
    slot, = (slot for name, _, kind, slot, _ in bindings if name == source and kind == 0)
    count, start = struct.unpack_from('<II', r)
    buffer, = (start+i*24 for i in range(count) if string(r, uint(r, start+i*24)) == source)
    name, fields, field_start = struct.unpack_from('<3I', r, buffer)
    field, = (field_start+j*24 for j in range(fields) if string(r, uint(r, field_start+j*24)) == CHANNEL_FIELD)
    _, offset, size, flags, type_info, _ = struct.unpack_from('<6I', r, field)
    if flags & 2 or size != 16 or offset % 16 or struct.unpack_from('<4H', r, type_info) != (1, 3, 1, 4):
        raise ValueError(f'{CHANNEL_FIELD} is no unused float4')
    store(r, field, len(r))
    store(r, field+12, flags | 2)
    r.extend(CHANNEL_HOST + b'\0')
    r.extend(bytes(-len(r) % 4))
    for at in {name} | {at for text, at, kind, _, _ in bindings if kind == 0 and text == source}:
        r[at:at+len(target)] = target
    declaration, = (op for op in program.ops if op[0] & 2047 == 89 and op[2] == slot)
    index = offset // 16
    declaration[3] = max(declaration[3], index+1)
    return slot, index


def channel_offset(resource):
    """The byte offset of srp_weapon_bake_channels in a patched program's buffer."""
    r = Program(resource).chunks[b'RDEF']
    count, start = struct.unpack_from('<II', r)
    offsets = set()
    for i in range(count):
        _, fields, field_start = struct.unpack_from('<3I', r, start+i*24)
        for j in range(fields):
            field = field_start+j*24
            if string(r, uint(r, field)) == CHANNEL_HOST.decode():
                offsets.add(uint(r, field+4))
    offset, = offsets
    return offset


def draw_pixel(resource):
    p = Program(resource)
    slots = {slot: CHANNELS.index(name) for name, _, kind, slot, count in resources(p.chunks[b'RDEF'])
             if kind == 2 and name in CHANNELS and count == 1}
    if not slots:
        return resource
    samples = []
    for op in p.ops:
        # Every native gbuffer read of these texture slots is a 2D sample
        # from TEXCOORD6.xy (v7). Refuse a changed material program rather
        # than silently overriding e.g. a later parallax displacement.
        if op[0] & 2047 in (69, 70, 71, 72, 73, 74) and len(op) > 6 and (op[5] >> 12) & 255 == 7 and op[6] in slots:
            if op[0] & 2047 != 69 or op[3:5] != pair(7, INPUT):
                raise ValueError('unrecognized native material sample')
            samples.append((op, slots[op[6]]))
    if not samples:
        return resource
    p.varying(15, output=False)
    cb, index = channel_constant(p)
    mask = p.temp()
    prefix = [ins(29, dest(mask, 1), scalar(15, 0, INPUT), value(0)),  # ge
              ins(1, dest(mask, 15), scalar(mask, 0), [0x208e46, cb, index])]
    coordinates = {}
    for channel in sorted({channel for _, channel in samples}):
        uv = p.temp()
        coordinates[channel] = uv
        prefix.append(ins(55, dest(uv, 3), scalar(mask, channel), pair(15, INPUT), pair(7, INPUT)))
    for op, channel in samples:
        op[3:5] = pair(coordinates[channel])
    at = next(i for i, op in enumerate(p.ops) if op[0] == 0x02000068) + 1
    while at < len(p.ops) and p.ops[at][0] & 2047 in (105, 106):
        at += 1
    p.ops[at:at] = prefix
    return p.finish()


FAMILIES = {
    'vertex_base_unwrap.vs': bake_vertex,
    'bake_decal_composition.ps': bake_pixel,
    'vertex_base.vs': draw_vertex,
    'gbuffer_pass.ps': draw_pixel,
}
