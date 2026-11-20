"""Decode the masked compatibility block and its original texture contract."""
import struct

SOURCE = 'shaders/sm_4_0/gbuffer_pass.ps/_3___1100000001__00____2_____0__0_0111____00000____000__________0__'
SOURCE_SHA256 = 'fcf8789de938e808ea75c899a220f88a725679601e75dbe1b107fda796c8ce9e'


def verify(block, before, scratch, weight, alpha):
    def op(code, *parts):
        words = [word for part in parts for word in part]
        return [code | ((len(words) + 1) << 24)] + words
    def literal(value):
        return [0x4001, struct.unpack('<I', struct.pack('<f', value))[0]]
    def dst(c):
        return [0x100002 | (1 << (c + 4)), scratch]
    def src(c):
        return [0x10000a | (c << 4), scratch]
    def neg(c):
        return [0x8010000a | (c << 4), 0x41, scratch]
    samples = [entry for entry in before if entry[0] == 0x09000045
               and entry[3:5] == [0x101046, 7] and entry[6] == 0
               and entry[5] in (0x107e46, 0x107936)]
    assert len(samples) == 1 and samples[0][7] == 0x106000 and samples[0][8] in (0, 1)
    strength, falloff = [0x20801a, 0, 14], [0x20800a, 0, 14]
    expected = [
        op(29, dst(0), strength, literal(-16777216)),
        op(29, dst(1), literal(-8388608), strength),
        op(1, dst(0), src(0), src(1)), op(0x4001f, src(0)),
        op(69, dst(3), [0x101046, 7], [0x107ff6, 0], samples[0][7:]),
        op(0, dst(0), [0x8010100a, 0x41, alpha], literal(1)),
        op(47, dst(1), src(0)), op(56, dst(1), src(1), falloff), op(25, dst(1), src(1)),
        op(0, dst(2), neg(3), literal(1)), op(50, dst(0), src(0), src(2), src(3)),
        op(56, dst(0), src(0), src(1)),
        op(56, dst(1), strength, literal(-1 / 8388608)),
        op(0, dst(2), src(1), literal(1)), op(0x2038, dst(0), src(0), src(2)),
        op(47, dst(0), src(0)), op(56, dst(0), src(0), src(1)), op(25, dst(0), src(0)),
        op(0, weight, neg(0), literal(1)), op(21),
    ]
    assert block == expected, 'original masked equation, selector, alpha input or texture contract differs'
