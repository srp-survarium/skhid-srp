#!/usr/bin/env python3
"""Native inventory/weapon/ammo item cooks bypass the resource name cache.

Execute their registration setup up to register_cook (without registering
unrelated dependencies), then the original cook lookup and name-cache gate.
This checks their reuse policy; actual asset loading remains a gameplay gate.
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STACK, STOP
from unicorn import UcError
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EIP


def run(data):
    g = Game(data)
    thread_id = g.block(16)
    g.emu.mem_write(thread_id, bytes.fromhex("b807000000c3"))
    g.word(0x870118, thread_id)
    table = g.block(0x206 * 4)
    g.word(0xA86E00, table)
    g.word(0xA86E04, table + 0x206 * 4)
    for begin, end, cook, class_id in [
        (0x2C7170, 0x2C71C8, 0x9261E8C, 0x41),
        (0x30A800, 0x30A867, 0x2261AE4, 0x46),
        (0x31F2E0, 0x31F33D, 0x2261A60, 0x48),
    ]:
        g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
        g.emu.emu_start(begin, end, count=10000)
        assert g.emu.reg_read(UC_X86_REG_EIP) == end
        assert g.read(cook + 8) == class_id
        assert g.read(cook + 0xC) == 0  # no reuse
        assert g.read(cook + 0x18) & 8  # translate-query cook
        g.word(table + class_id * 4, cook)
        g.call(0x102CD0, edi=class_id)
        assert g.emu.reg_read(UC_X86_REG_EAX) == cook

        query = g.block(0x300)
        g.word(query + 0x84, class_id)
        # An inaccessible name proves the zero policy bypasses lookup
        # before even reading the name; repeat to exclude first-query state.
        g.word(query + 0xF8, 0x12345000)
        for _ in range(2):
            g.call(0x108A00, (query, 0))
            assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
            assert g.emu.reg_read(UC_X86_REG_EAX) == 1
        g.word(cook + 0xC, 1)
        try:
            g.call(0x108A00, (query, 0))
        except UcError:
            # Negative control: a reusable cook follows the name path.
            assert g.emu.reg_read(UC_X86_REG_EIP) == 0x108A30
        else:
            raise AssertionError("reuse=1 did not enter name lookup")
        g.word(cook + 0xC, 0)
    print("native pickup cooks: inventory, weapon and ammo item registration use no-reuse; "
          "original class lookup and repeated name-cache bypass passed, reusable negative controls rejected")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
