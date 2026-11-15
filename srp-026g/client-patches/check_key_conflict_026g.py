#!/usr/bin/env python3
"""Patch 66: push-to-talk's key and a game key take each other's key in the
options (voice-chat.md, "A push-to-talk key"; client_patch.md section 3.54).

    python check_key_conflict_026g.py <original survarium.exe>

game_options::process_key_input (0x2010E0) walks the controls' rows for one
that has the key just pressed for another action; at 0x20119A it tests the two
actions' kinds (their rows of the action table, through the binder's bindings)
and a row whose kinds meet the bound one's is taken (0x2011B2: the reassign
message names it), any other passed (0x2011D4). The test runs from 0x20119A,
in the original and in the built exe, with a binder whose bindings point at the
game's own action table rows (0xA74AF0), until it reaches one of the two:
- push-to-talk (ptt, kind 8) and a key of the player (spot, accel, delay
  breath; kinds 5, 1, 4), either way: passed in the original, taken in the
  built exe;
- push-to-talk and a chat key (send_message): taken in both; and a lobby key
  (friends, character): passed in both;
- two other actions as in the original: melee and accel taken, crouch_toggle
  and character (both C by default) passed, accel and delay_breath (both Left
  Shift) passed.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP

import patch_026g as base

TEST, TAKEN, PASSED = 0x20119A, 0x2011B2, 0x2011D4
ACTION_TABLE, ACTION_ROWS, ACTION_ROW = 0xA74AF0, 60, 20    # {name, id, kinds, toggle, default key}
BINDING = 0xC
STACK, BINDER = 0x21000000, 0x22000000
SP = STACK + 0x8000

PTT, SPOT, ACCEL, DELAY_BREATH, MELEE, CROUCH_TOGGLE = 0x47, 0x22, 0x07, 0x2A, 0x21, 0x06
CHARACTER, FRIENDS, SEND_MESSAGE = 0x40, 0x44, 0x45
# (the action the key is pressed for, the row that has it, the original's answer, the built exe's)
CASES = (
    (PTT, SPOT, "passed", "taken"),
    (SPOT, PTT, "passed", "taken"),
    (PTT, ACCEL, "passed", "taken"),
    (DELAY_BREATH, PTT, "passed", "taken"),
    (PTT, SEND_MESSAGE, "taken", "taken"),
    (PTT, FRIENDS, "passed", "passed"),
    (CHARACTER, PTT, "passed", "passed"),
    (MELEE, ACCEL, "taken", "taken"),
    (CROUCH_TOGGLE, CHARACTER, "passed", "passed"),
    (ACCEL, DELAY_BREATH, "passed", "passed"),
)


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_map(BINDER, 0x1000, UC_PROT_ALL)
        rows = {}
        for n in range(ACTION_ROWS):
            row = ACTION_TABLE + n * ACTION_ROW
            rows[struct.unpack("<I", emu.mem_read(row + 4, 4))[0]] = row
        for action, row in rows.items():                      # each binding's +0: its action's row
            emu.mem_write(BINDER + action * BINDING, struct.pack("<I", row))
        self.kinds = {a: struct.unpack("<I", emu.mem_read(row + 8, 4))[0] for a, row in rows.items()}
        emu.hook_add(UC_HOOK_CODE, self.end)

    def end(self, emu, address, size, _):
        if address in (TAKEN, PASSED):
            emu.emu_stop()

    def test(self, bound, row):
        emu = self.emu
        emu.mem_write(STACK, bytes(0x10000))
        emu.mem_write(SP + 0x1C, struct.pack("<I", BINDER))   # process_key_input's local: the binder
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.reg_write(UC_X86_REG_ESI, bound)
        emu.reg_write(UC_X86_REG_EDI, row)
        emu.reg_write(UC_X86_REG_EBX, 0x0B0B0B0B)
        emu.emu_start(TEST, 0, count=200000)
        eip = emu.reg_read(UC_X86_REG_EIP)
        assert eip in (TAKEN, PASSED), hex(eip)
        assert emu.reg_read(UC_X86_REG_ESP) == SP, hex(emu.reg_read(UC_X86_REG_ESP))
        assert (emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EDI)) == (bound, row)
        assert emu.reg_read(UC_X86_REG_EBX) == 0x0B0B0B0B
        return "taken" if eip == TAKEN else "passed"


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))
    assert (old.kinds[PTT], old.kinds[SPOT], old.kinds[ACCEL], old.kinds[DELAY_BREATH]) == (8, 5, 1, 4)
    for bound, row, want_old, want_new in CASES:
        got = old.test(bound, row), new.test(bound, row)
        assert got == (want_old, want_new), (hex(bound), hex(row), got)
        print(f"ok {bound:#04x} onto {row:#04x}'s key: the original {got[0]}, the built exe {got[1]}")
    print("patch 66: push-to-talk and the player's keys take each other's key in the options; "
          "the chat, lobby and other keys as in the original")


if __name__ == "__main__":
    main()
