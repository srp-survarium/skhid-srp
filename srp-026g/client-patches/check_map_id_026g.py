#!/usr/bin/env python3
"""Run patch 154 (rust/patches/src/map_id.rs, its lohooks of map_id.toml, and
patch_026g.py's MAP_ID_READERS: a maps_dict id over a byte) in the default
build on made-up options, from the game's own code before each site to past
it.

    python check_map_id_026g.py <original survarium.exe>

Requires Python `unicorn`; check_blackout_026g.py's harness. The original runs
the same way and must do what it did:

- the options read: process_static_match_info from 0x27B917 through the game's
  own match_options::deserialize (0x1F0E70) to 0x27B932, and a replay's chunk 1
  from 0x1F094C to its `ret 8`: the options' +577B8h holds srp's block's tag 4
  (alone, after Blackout's entry or an unknown tag, before a free-for-all's)
  when it is over a byte and its low byte the options' byte, else the
  options' byte (no block, another magic, another low byte, an id of a byte,
  a cut entry), whatever was there before; +577B4h keeps the wire's byte; the
  registers and the stack as the game's code leaves them; the original
  leaves +577B8h as it was;
- the six readers of the map (each one instruction, 7 bytes): the register
  each loads is the options' +577B8h in the build, the byte +577B4h in the
  original, and every other register is kept.
"""
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base
import check_blackout_026g as blackout

OPTIONS = blackout.OPTIONS
MAP_BYTE, MAP_ID = 0x577B4, 0x577B8
STALE = 0x77777777
# The readers: (va, the base register, the register loaded).
READERS = (
    (0x234CFD, UC_X86_REG_EAX, UC_X86_REG_EDI),   # initialize_match: the hud's game type
    (0x27AFA7, UC_X86_REG_ESI, UC_X86_REG_EDI),   # query_players
    (0x27BC26, UC_X86_REG_ESI, UC_X86_REG_ECX),   # process_static_match_info: "Map : %s (%d)"
    (0x27BE8B, UC_X86_REG_ESI, UC_X86_REG_EDX),   # process_static_match_info: switch_to_level_loading
    (0x2A3603, UC_X86_REG_EAX, UC_X86_REG_EDI),   # refresh_quest_ui_data
    (0x2D01F3, UC_X86_REG_EAX, UC_X86_REG_EDI),   # game_world_core's constructor
)
OTHERS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
          UC_X86_REG_EBP)


def options(byte, block=b""):
    """check_blackout's options with the map `byte`, then `block`."""
    out = bytearray(blackout.empty_options())
    out[20 * 9] = byte
    return bytes(out) + block


def wide(entry_id):
    return (4, struct.pack("<H", entry_id))


class Run(blackout.Run):
    def with_stale(self, read, payload):
        """`read` (static_info or replay) of `payload` over options whose
        +577B8h held STALE: whether the code went on as the game's, and the
        options' byte and +577B8h after it."""
        real = self.options_payload

        def stale(payload_, stale_):
            real(payload_, stale_)
            self.emu.mem_write(OPTIONS + MAP_ID, struct.pack("<I", STALE))
        self.options_payload = stale
        try:
            back, _ = read(payload)
        finally:
            self.options_payload = real
        return back, self.emu.mem_read(OPTIONS + MAP_BYTE, 1)[0], self.read(OPTIONS + MAP_ID)

    def reader(self, va, base_reg, loaded):
        """The one instruction at `va` with its base register at the options
        (+577B4h 44, +577B8h 300): the loaded value, and whether every other
        register was kept."""
        self.emu.mem_write(OPTIONS, bytes(0x578A0))
        self.emu.mem_write(OPTIONS + MAP_BYTE, bytes([44]))
        self.emu.mem_write(OPTIONS + MAP_ID, struct.pack("<I", 300))
        given = {reg: 0x0BAD0000 + 0x100 * i for i, reg in enumerate(OTHERS)}
        given[base_reg] = OPTIONS
        for reg, value in given.items():
            self.emu.reg_write(reg, value)
        self.emu.emu_start(va, va + 7, count=1)
        kept = all(self.emu.reg_read(reg) == value for reg, value in given.items() if reg != loaded)
        return self.emu.reg_read(loaded), kept


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(("ok   " if ok else "FAIL ") + text)

    srpx = blackout.srpx
    cases = [
        ("no block: the byte", 61, b"", 61),
        ("tag 4, 300", 44, srpx(wide(300)), 300),
        ("tag 4 after Blackout's entry", 44, srpx((2, b"\x01"), wide(300)), 300),
        ("tag 4 before a free-for-all's", 44, srpx(wide(300), (3, b"\x1e\x00")), 300),
        ("tag 4 after an unknown tag", 0xFF, srpx((9, b"xyz"), wide(65535)), 65535),
        ("another low byte: the byte", 61, srpx(wide(300)), 61),
        ("an id of a byte: the byte", 61, srpx(wide(61)), 61),
        ("another magic: the byte", 44, b"SRPY" + bytes([4, 2]) + struct.pack("<H", 300), 44),
        ("a cut entry: the byte", 44, b"SRPX\x04\x02\x2c", 44),
        ("another length: the byte", 44, srpx((4, b"\x2c\x01\x00")), 44),
    ]
    for way in ("static_info", "replay"):
        print(f"the options' read ({way})")
        for name, byte, block, want in cases:
            payload = options(byte, block)
            back, got_byte, got = patched.with_stale(getattr(patched, way), payload)
            s_back, s_byte, s_got = stock.with_stale(getattr(stock, way), payload)
            report(back and got_byte == byte and got == want and s_back and s_byte == byte and s_got == STALE,
                   f"{name}: +577B8h {got} (the original leaves {s_got:#x}), +577B4h {got_byte}, "
                   "on as the game's")

    print("the readers of the map")
    for va, base_reg, loaded in READERS:
        b_value, b_kept = patched.reader(va, base_reg, loaded)
        a_value, a_kept = stock.reader(va, base_reg, loaded)
        report(b_value == 300 and a_value == 44 and b_kept and a_kept,
               f"{va:#x}: loads {b_value} (+577B8h; the original {a_value}, +577B4h), the rest kept")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
