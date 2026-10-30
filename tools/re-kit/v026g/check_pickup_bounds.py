#!/usr/bin/env python3
"""Verify weapon targeting's model-bound coordinate conversion in original code.

Runs the renderer's authored-box copy/rotation and bone-origin expansion on
synthetic retained model resources. The pi rotation singleton is initialized
as native initialization would leave it, avoiding imported CRT trig calls.
Stops before renderer publication. Does not render meshes or test interaction.
"""
from pathlib import Path
import math
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STACK, STOP
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EIP


IDENTITY = (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)


def run(data):
    for authored in [(.2, -.1, .3, .8, .1, .9), (-.12, -.3, -.8, .2, .15, .6),
                     (-1, -1, -1, 1, 1, 1)]:
        for bone_origin in [None, (3, 4, 5)]:
            g = Game(data)
            instance, original = g.block(0x6300), g.block(0x144)
            bones, inverse, previous, current, interpolated = [g.block(64) for _ in range(5)]
            g.word(instance + 0x62B4, original)
            encoded = struct.pack('<6f', *authored)
            g.emu.mem_write(original + 0x108, encoded)
            g.word(original + 0x140, inverse)
            g.word(instance + 0x290, previous)
            g.word(instance + 0x229C, current)
            g.word(instance + 0x42A8, interpolated)
            matrix = list(IDENTITY)
            if bone_origin:
                matrix[12:15] = bone_origin
            g.emu.mem_write(bones, struct.pack('<16f', *matrix))
            g.emu.mem_write(inverse, struct.pack('<16f', *IDENTITY))
            # Actual native rotation singleton: rotY(f32(pi)), row-vector.
            angle = struct.unpack('<f', struct.pack('<f', math.pi))[0]
            s, c = math.sin(angle), math.cos(angle)
            g.word(0xA264CC8, 1)
            g.emu.mem_write(0xA264C88, struct.pack('<16f', c, 0, s, 0, 0, 1, 0, 0,
                                                 -s, 0, c, 0, 0, 0, 0, 1))
            esp = STACK - 0x100
            for i, value in enumerate([STOP, instance, bones, bones, int(bone_origin is not None)]):
                g.word(esp + i * 4, value)
            g.emu.reg_write(UC_X86_REG_ESP, esp)
            g.emu.emu_start(0x15A590, 0x15A950, count=100_000)
            assert g.emu.reg_read(UC_X86_REG_EIP) == 0x15A950
            actual = struct.unpack('<6f', g.emu.mem_read(instance + 0x62B8, 24))
            expected = [-authored[3], authored[1], -authored[5],
                        -authored[0], authored[4], -authored[2]]
            if authored == (-1, -1, -1, 1, 1, 1):
                expected = [-2, -2, -2, 2, 2, 2]
            if bone_origin:
                for i in range(3):
                    expected[i] = min(expected[i], bone_origin[i])
                    expected[i + 3] = max(expected[i + 3], bone_origin[i])
            assert all(abs(a - b) < 1e-6 for a, b in zip(actual, expected)), (actual, expected)
            assert bytes(g.emu.mem_read(original + 0x108, 24)) == encoded
    print('pickup bounds: original resource layout, Y(pi) conversion, generic fallback, '
          'bone-origin expansion and immutable authored bounds passed')


if __name__ == '__main__':
    run(Path(sys.argv[1]).read_bytes())
