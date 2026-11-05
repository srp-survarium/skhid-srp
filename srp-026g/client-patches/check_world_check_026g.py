#!/usr/bin/env python3
"""Run patch 87 (rust/patches/src/world_check.rs, its two lohooks of
world_check.toml: a game parses every world it is sent before it loads it;
../docs/notes/world_frame_check.md) in the default build, the original
beside it.

    python check_world_check_026g.py <original survarium.exe> [--drift VERDICTS.tsv]

Requires Python `unicorn`. The 0xE8 handler (process_dynamic_match_info
0x27BFF0) runs from before each of its two loads to past it, on a made-up
network client, game and frame: the load (game_world_core::deserialize
0x2D0B70) and close_current_match (0x27A790) are stubs that record their
arguments; kernel32's CreateFileA, WriteFile and CloseHandle (the log's
import slots) stubs that record the line.

- `world_load.raw` (0x27C0FF, the packet's reader in ESI) and
  `world_load.unpacked` (0x27C0D0, the world in the frame's mutable buffer
  at EBP-24h, the four arguments pushed): a world a game can load is loaded
  in both builds with the same arguments, the reader the load gets the same,
  EBX, ESI, EDI, EBP and ESP the same at 0x27C116; one it cannot is loaded by
  the original and never by the patch, which calls close_current_match(the
  client, 2) once, reads the packet's reader to its end (the raw world),
  writes one line to srp_patches.log and reaches the handler's epilogue
  0x27C1E0 with EBX, ESI, EDI and EBP as they were. The worlds: no players,
  two at slots 0 and 19 (one animated), each section at its cap and one byte
  over, a state at 0x1000 and one over, slot 20, cut short, a byte too many,
  shorter than a time;
- --drift: the drift test, the client build against the server build. The
  file is the host build's verdicts (crate world-frame's example `verdicts`,
  run on kept worlds: `cargo run --release -p world-frame --example verdicts
  -- DIR... > VERDICTS.tsv` in srp-026g). Each world, as a game has it (its
  time, then the body), and its 16 mutated copies (the body's byte at
  len * k / 16 XOR 0xFF) go through the patched build's raw load, the world
  through the unpacked one too, and each verdict must be the host's.
"""
import argparse
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

RAW_SITE, UNPACKED_SITE = 0x27C0FF, 0x27C0D0
LOAD, LOADED, EPILOGUE, CLOSE = 0x2D0B70, 0x27C116, 0x27C1E0, 0x27A790
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE = 0x870048, 0x870108, 0x870040   # the import slots
CLIENT, GAME, READER = 0x40000000, 0x40400000, 0x40500000
WORLD, STACK, STUBS = 0x41000000, 0x42000000, 0x43000000
WORLD_ROOM, STACK_ROOM = 0x20000, 0x10000
FRAME = STACK + 0x8000                     # the handler's EBP
CORE = GAME + 0x1F8                        # [client+1F1C40h] + 1F8h
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
MUTATIONS = 16
# Section caps (world-frame's Section::cap) and the state's.
CAPS = (0x2400, 0xC00, 0x2000, 0x4000, 0x800, 0xC00)
STATE_CAP = 0x1000


class Run:
    """One exe's image under Unicorn, kept for every case (its translated
    code with it)."""

    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((CLIENT, 0x200000), (GAME, 0x1000), (READER, 0x1000), (WORLD, WORLD_ROOM),
                         (STACK, STACK_ROOM), (STUBS, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(CLIENT + 0x1F1C40, GAME)
        for slot, stub in ((CREATE_FILE, STUBS), (WRITE_FILE, STUBS + 0x10), (CLOSE_HANDLE, STUBS + 0x20)):
            self.word(slot, stub)
        for address in (LOAD, CLOSE, STUBS, STUBS + 0x10, STUBS + 0x20):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=address, end=address)
        for address in (LOADED, EPILOGUE):
            emu.hook_add(UC_HOOK_CODE, self.stop, begin=address, end=address)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address == LOAD:                 # (core, reader, 1, 8), ret 10h
            reader = args[1]
            self.calls.append(("load", tuple(args[:4]), tuple(self.read(reader + 4 * i) for i in range(3))))
            pops, result = 16, 1
        elif address == CLOSE:              # thiscall (reason), ret 4
            self.calls.append(("close", emu.reg_read(UC_X86_REG_ECX), args[0]))
            pops, result = 4, 0
        elif address == STUBS:              # CreateFileA: 7 arguments, a handle
            pops, result = 28, 0x1234
        elif address == STUBS + 0x10:       # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])))
            self.word(args[3], args[2])
            pops, result = 20, 1
        else:                               # CloseHandle
            pops, result = 4, 1
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        self.stopped = address
        emu.emu_stop()

    def start(self, at, regs, sp):
        self.calls, self.lines, self.stopped = [], [], None
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(at, 0, count=20_000_000)
        kept = {reg: self.emu.reg_read(reg) for reg in KEPT}
        return self.stopped, kept, self.emu.reg_read(UC_X86_REG_ESP)

    def raw(self, state):
        """The raw load: the packet's reader at the world (after the
        generation and the flag)."""
        assert len(state) + 2 <= WORLD_ROOM
        self.emu.mem_write(WORLD, b"\x05\x00" + state)
        for i, value in enumerate((WORLD, WORLD + 2, len(state) + 2)):
            self.word(READER + 4 * i, value)
        regs = {UC_X86_REG_EDI: CLIENT, UC_X86_REG_ESI: READER, UC_X86_REG_EBP: FRAME,
                UC_X86_REG_EBX: WORLD + 2, UC_X86_REG_EAX: 0, UC_X86_REG_ECX: 0, UC_X86_REG_EDX: 0}
        stopped, kept, sp = self.start(RAW_SITE, regs, FRAME - 0x30)
        return stopped, kept, sp, self.read(READER + 4)

    def unpacked(self, state):
        """The unpacked load: the world in the frame's mutable buffer
        {allocator 0, begin, capacity 8000h, size}, ESI its begin, EDX its
        size, ECX the core, the arguments pushed (8, 1, the frame's reader,
        the core)."""
        assert len(state) <= WORLD_ROOM
        self.emu.mem_write(WORLD, state)
        for at, value in ((-0x24, 0), (-0x20, WORLD), (-0x1C, 0x8000), (-0x18, len(state)),
                          (-0x14, 0), (-0x10, 0), (-0xC, 0)):
            self.word(FRAME + at, value)
        sp = FRAME - 0x30 - 16
        for i, value in enumerate((CORE, FRAME - 0x14, 1, 8)):
            self.word(sp + 4 * i, value)
        regs = {UC_X86_REG_EDI: CLIENT, UC_X86_REG_ESI: WORLD, UC_X86_REG_EBP: FRAME,
                UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_EAX: FRAME - 0x14, UC_X86_REG_ECX: CORE,
                UC_X86_REG_EDX: len(state)}
        stopped, kept, sp = self.start(UNPACKED_SITE, regs, sp)
        return stopped, kept, sp, None


def body(blocks, sections, mask=None):
    """A body of `blocks` (slot, state, animation bytes or None) and six
    sections."""
    out = bytearray(struct.pack("<I", sum(1 << s for s, _, _ in blocks) if mask is None else mask))
    for _, state, animation in blocks:
        out += struct.pack("<H", len(state)) + state
        out += b"\x01" + animation if animation else b"\x00"
    for section in sections:
        out += struct.pack("<H", len(section)) + section
    return bytes(out)


def state(world, time=4500):
    return struct.pack("<I", time) + world


# T = 0, no times, one float, not indexed: 69 bits, 9 bytes (world-frame's
# test of animation_len).
ANIMATION = bytes([0, 0, 0, 0, 0x08, 0, 0, 0, 0])
EMPTY = [b""] * 6


def cases():
    """(what, the state, whether a game can load it)."""
    small = [b"\x01" * 7, b"\x02" * 9, b"\x03" * 3, b"", b"\x05" * 11, b"\x06" * 2]
    two = [(0, b"\xAB" * 33, None), (19, b"\xCD" * 40, ANIMATION)]
    yield "no players", state(body([], small)), True
    yield "slots 0 and 19, one animated", state(body(two, small)), True
    for i, name in enumerate(("bullets", "statistics", "objects", "physics", "rules", "effects")):
        for extra, ok in ((0, True), (1, False)):
            sections = list(EMPTY)
            sections[i] = bytes(CAPS[i] + extra)
            yield f"{name} {'at its cap' if ok else 'one over'}", state(body(two[:1], sections)), ok
    yield "a state at 0x1000", state(body([(3, bytes(STATE_CAP), None)], EMPTY)), True
    yield "a state one over", state(body([(3, bytes(STATE_CAP + 1), None)], EMPTY)), False
    yield "slot 20", state(body([(0, b"\xAB" * 33, None)], EMPTY, mask=1 | 1 << 20)), False
    whole = state(body(two, small))
    yield "cut short", whole[:-1], False
    yield "a byte too many", whole + b"\x00", False
    yield "the body without its time", body(two, small), False
    yield "shorter than a time", b"\x01\x02\x03", False


def compare(stock, ours, path, world, can_load):
    """Failures of `ours` against `stock` on one world at one load."""
    load = getattr(stock, path)(world)
    calls = stock.calls
    fails = []
    if load[0] != LOADED or [c[0] for c in calls] != ["load"]:
        fails.append(f"the original does not load it ({load[0]}, {calls})")
    got = getattr(ours, path)(world)
    if can_load:
        if got[:3] != load[:3] or ours.calls != calls or got[3] != load[3]:
            fails.append(f"loaded otherwise: {got}, {ours.calls} against {load}, {calls}")
        if ours.lines:
            fails.append(f"a log line: {ours.lines}")
        return fails
    stopped, kept, _, cursor = got
    if stopped != EPILOGUE:
        fails.append(f"not at the epilogue: {stopped}")
    if ours.calls != [("close", CLIENT, 2)]:
        fails.append(f"calls {ours.calls}")
    entry = {UC_X86_REG_EDI: CLIENT, UC_X86_REG_EBP: FRAME,
             UC_X86_REG_ESI: READER if path == "raw" else WORLD,
             UC_X86_REG_EBX: WORLD + 2 if path == "raw" else 0x0B0B0B0B}
    if kept != entry:
        fails.append(f"registers {kept}")
    if path == "raw" and cursor != WORLD + 2 + len(world):
        fails.append(f"the reader not at its end: {cursor:#x}")
    if len(ours.lines) != 1 or b"world_check: a world refused" not in ours.lines[0]:
        fails.append(f"the log: {ours.lines}")
    return fails


def drift(ours, path):
    """(worlds, checks, the rows that differ)."""
    worlds = checks = 0
    differ = []
    for row in open(path):
        file, time, wanted = row.rstrip("\n").split("\t")
        world = open(file, "rb").read()
        bodies = [world]
        for k in range(MUTATIONS):
            mutated = bytearray(world)
            mutated[len(world) * k // MUTATIONS] ^= 0xFF
            bodies.append(bytes(mutated))
        worlds += 1
        for k, (one, verdict) in enumerate(zip(bodies, wanted)):
            loads = ["raw", "unpacked"] if k == 0 else ["raw"]
            for load in loads:
                stopped = getattr(ours, load)(state(one, int(time)))[0]
                checks += 1
                got = "0" if stopped == LOADED else "1" if stopped == EPILOGUE else "?"
                if got != verdict:
                    differ.append(f"{file} copy {k} ({load}): the game's build {got}, the host's {verdict}")
    return worlds, checks, differ


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("exe")
    parser.add_argument("--drift")
    args = parser.parse_args()
    original = open(args.exe, "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(bytes(built))
    failed = 0
    for what, world, can_load in cases():
        for path in ("raw", "unpacked"):
            fails = compare(stock, ours, path, world, can_load)
            failed += bool(fails)
            verdict = "loaded" if can_load else "refused, the match left"
            print(f"  {'FAIL' if fails else 'ok'}  {path}: {what}: {verdict}" + "".join(f"\n      {f}" for f in fails))
    if args.drift:
        worlds, checks, differ = drift(ours, args.drift)
        failed += len(differ)
        for line in differ[:20]:
            print(f"  FAIL  drift: {line}")
        print(f"  {'FAIL' if differ else 'ok'}  drift: {worlds} worlds, {checks} checks with the mutated copies, "
              f"{len(differ)} differ")
    print("all cases passed" if not failed else f"{failed} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
