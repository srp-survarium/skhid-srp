#!/usr/bin/env python3
"""DXBC's MD5 variant, for guarded offline shader edits without a Wine launch.

Padding layout reference: Wine/vkd3d libs/vkd3d-shader/checksum.c
https://github.com/wine-mirror/wine/blob/master/libs/vkd3d/libs/vkd3d-shader/checksum.c
The compression below follows the standard MD5 round equations.
"""
import math
import struct


def checksum(container):
    if container[:4] != b'DXBC':
        raise ValueError('not a DXBC container')
    payload = container[20:]
    bit_length = (len(payload) * 8) & 0xffffffff
    complete = len(payload) // 64 * 64
    tail = payload[complete:] + b'\x80'
    if len(tail) > 56:
        blocks = payload[:complete] + tail.ljust(64, b'\0')
        tail = bytes(60)
    else:
        blocks = payload[:complete]
        tail = bytes(4) + tail
        tail = tail.ljust(60, b'\0')
    final = bytearray(tail + struct.pack('<I', (bit_length >> 2) | 1))
    struct.pack_into('<I', final, 0, bit_length)
    blocks += final
    state = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476]
    shifts = [7, 12, 17, 22] * 4 + [5, 9, 14, 20] * 4 + [4, 11, 16, 23] * 4 + [6, 10, 15, 21] * 4
    constants = [int(abs(math.sin(i + 1)) * 2**32) for i in range(64)]
    mask = 0xffffffff
    for start in range(0, len(blocks), 64):
        words = struct.unpack_from('<16I', blocks, start)
        a, b, c, d = state
        for i in range(64):
            if i < 16:
                f, g = (b & c) | (~b & d), i
            elif i < 32:
                f, g = (d & b) | (~d & c), (5 * i + 1) % 16
            elif i < 48:
                f, g = b ^ c ^ d, (3 * i + 5) % 16
            else:
                f, g = c ^ (b | ~d), (7 * i) % 16
            value = (a + f + constants[i] + words[g]) & mask
            rotated = ((value << shifts[i]) | (value >> (32 - shifts[i]))) & mask
            a, b, c, d = d, (b + rotated) & mask, b, c
        state = [(x + y) & mask for x, y in zip(state, [a, b, c, d])]
    return struct.pack('<4I', *state)
