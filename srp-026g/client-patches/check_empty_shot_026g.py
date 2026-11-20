#!/usr/bin/env python3
"""Run weapon_core_fire_state_base::on_shot_event in the original and patch 148's exe.

    python check_empty_shot_026g.py <original survarium.exe>

Requires Python `unicorn`. on_shot_event (0x30EBF0) is the fire state's "shoot"
callback: with a round queued (+13Ch) it fires it (weapon_core::instant_fire
0x2E23A0); with the queue spent it fires nothing, after the assert
m_bullets_in_queue > 0 (its report, 0x1132D0). Patch 148 takes the assert's own
skip, so a spent queue returns at once: neither the report nor a shot. A queued
round is fired as in the original.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base

ON_SHOT_EVENT, INSTANT_FIRE, ASSERT_REPORT = 0x30EBF0, 0x2E23A0, 0x1132D0
STACK, HEAP, RETURN = 0x20000000, 0x20100000, 0x20400000   # above the image
STATE, CORE, PARAMS = HEAP + 0x1000, HEAP + 0x3000, HEAP + 0x6000
QUEUE, STATE_CORE = 0x13C, 0x120


def image(data):
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    headers = base.Headers(data)
    for _, rva, vsize, raw, rsize, _ in headers.sections(data):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
    emu.mem_map(HEAP, 0x10000, UC_PROT_ALL)
    emu.mem_map(RETURN, 0x1000, UC_PROT_ALL)
    return emu


def shoot(data, queued):
    """What on_shot_event does for a fire state with `queued` rounds: the first of
    `fires` (instant_fire), `asserts` (the report) or `returns` (with EAX)."""
    emu = image(data)
    emu.mem_write(STATE + STATE_CORE, struct.pack("<I", CORE))
    emu.mem_write(STATE + QUEUE, bytes([queued]))
    esp = STACK + 0x8000
    emu.mem_write(esp, struct.pack("<II", RETURN, PARAMS))   # thiscall: the params
    emu.reg_write(UC_X86_REG_ESP, esp)
    emu.reg_write(UC_X86_REG_ECX, STATE)
    seen = []

    def stop(uc, address, size, what):
        seen.append(what)
        uc.emu_stop()

    emu.hook_add(UC_HOOK_CODE, stop, "fires", INSTANT_FIRE, INSTANT_FIRE)
    emu.hook_add(UC_HOOK_CODE, stop, "asserts", ASSERT_REPORT, ASSERT_REPORT)
    try:
        emu.emu_start(ON_SHOT_EVENT, RETURN, count=10000)
    except UcError as error:
        return ("fault", emu.reg_read(UC_X86_REG_EIP), str(error))
    if seen:
        return (seen[0],)
    if emu.reg_read(UC_X86_REG_EIP) != RETURN or emu.reg_read(UC_X86_REG_ESP) != esp + 8:
        raise AssertionError("on_shot_event did not return (ret 4)")
    return ("returns", emu.reg_read(UC_X86_REG_EAX))


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    patched, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    va, old, new = base.EMPTY_SHOT_QUIET
    at = base.file_offset(va)
    assert original[at:at + len(old)] == old and patched[at:at + len(new)] == new
    cases = [
        (original, 0, ("asserts",)),
        (patched, 0, ("returns", 0)),
        (original, 1, ("fires",)),
        (patched, 1, ("fires",)),
    ]
    for data, queued, want in cases:
        got = shoot(data, queued)
        if got != want:
            which = "original" if data is original else "patched"
            raise AssertionError(f"the {which} exe, {queued} queued: {got}, not {want}")
    print("patch 148: a spent queue's shoot event: the original asserts m_bullets_in_queue > 0, "
          "the patched exe returns 0 without it; a queued round is fired by both")


if __name__ == "__main__":
    main()
