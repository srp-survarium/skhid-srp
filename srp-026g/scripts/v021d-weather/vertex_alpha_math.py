"""Small independent scalar SM4 executor for original/generated cutout blocks.

Only operations actually used by the reviewed scalar alpha paths are accepted.
Every arithmetic result rounds to float32; MAD uses fused multiply/add.
"""
import math
import struct


def bits(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


def number(value):
    return struct.unpack('<f', struct.pack('<I', value))[0]


def run(ops, vertex_alpha, texture_alpha, params, marker, initial=None, output=(0, 0)):
    registers = {}
    if initial is not None:
        register, component = initial
        registers[register] = [0] * 4
        registers[register][component] = bits(texture_alpha)

    def source(op, at):
        token = op[at]
        at += 1
        negative = bool(token >> 31)
        if negative:
            assert op[at] == 0x41
            at += 1
        kind, component = (token >> 12) & 255, (token >> 4) & 3
        if kind == 0:
            value = registers[op[at]][component]
            at += 1
        elif kind == 4:
            value = op[at]
            at += 1
        elif kind == 1:
            assert op[at] in (7, 9) and component == 0
            value = bits(vertex_alpha)
            at += 1
        else:
            assert kind == 8
            slot, index = op[at:at + 2]
            at += 2
            if index == 11:
                assert slot == 0
                value = bits(params[component])
            else:
                assert index == 15 and component == 2 and slot in (0, 1)
                value = bits(marker)
        return (bits(-number(value)) if negative else value), at

    active, stack = True, []
    for op in ops:
        code = op[0] & 2047
        if code == 31:
            condition = False
            if active:
                value, end = source(op, 1)
                assert end == len(op)
                condition = bool(value)
            stack.append((active, condition))
            active = active and condition
            continue
        if code == 18:
            parent, condition = stack[-1]
            active = parent and not condition
            continue
        if code == 21:
            active = stack.pop()[0]
            continue
        if not active:
            continue
        mask, register = (op[1] >> 4) & 15, op[2]
        assert (op[1] >> 12) & 255 == 0
        dest = registers.setdefault(register, [0] * 4)
        if code == 69:
            resource = 5 if len(op) == 9 else 8
            assert op[resource + 1] == 0
            swizzle = (op[resource] >> 4) & 255
            for component in range(4):
                if mask & (1 << component):
                    dest[component] = bits(texture_alpha if ((swizzle >> (2 * component)) & 3) == 3 else .3)
            continue
        assert mask.bit_count() == 1
        args, raw_args, at = [], [], 3
        while at < len(op):
            value, at = source(op, at)
            raw_args.append(value)
            args.append(number(value))
        if code == 0:
            value = args[0] + args[1]
        elif code == 56:
            value = args[0] * args[1]
        elif code == 47:
            value = math.log2(args[0]) if args[0] else -math.inf
        elif code == 25:
            value = math.exp2(args[0])
        elif code == 50:
            value = math.fma(*args)
        elif code == 54:
            value = args[0]
        elif code == 55:
            dest[mask.bit_length() - 1] = raw_args[1] if raw_args[0] else raw_args[2]
            continue
        elif code == 24:
            dest[mask.bit_length() - 1] = 0xffffffff if args[0] == args[1] else 0
            continue
        else:
            raise AssertionError(code)
        if op[0] & 0x2000:
            value = min(1., max(0., value))
        dest[mask.bit_length() - 1] = bits(value)
    assert not stack
    return registers[output[0]][output[1]]
