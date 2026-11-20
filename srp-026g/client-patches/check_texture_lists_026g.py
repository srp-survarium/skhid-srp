#!/usr/bin/env python3
"""Run the game's own pushes into the render's texture lists with patch 163,
in the original and in the build (README "The render's texture lists").

    python check_texture_lists_026g.py <original survarium.exe>

Requires Python `unicorn`. Each case runs the game's code from the patched
sites:

- the resource manager's texture names (+1Ch, 2048 fixed_string<260> of
  110h): remove_texture_predicate 0x3DF780 for a new name, the manager its
  global 0x226047C. With 2047 names it pushes in both. With 2048 the
  original goes on to push_back 0x1326F0's assert (the branch 0x132701),
  the build moves the 2048 names to a heap room of 4096 (every name's
  three pointers into its own entry there) and pushes the new one; a name
  already held is found in the room; with 4096 in the room the build
  reaches the assert too;
- a scene's streamable textures (+126158h, 2048 of 148h), with the
  candidates (+98054Ch, 2048 of 64h) and the requests (+1CA164h, 1024 of
  118h): gather_streamable_textures from its site 0x155205 to after its
  push_back (0x15521C). Full, the original reaches the assert's branch
  0x155F6E; the build moves the three lists into rooms of 4096, 4096 and
  2048 (each entry as it was, the names' pointers into their entries) and
  pushes; with 4096 the build reaches the assert;
- a texture pool's textures (+64h, 1024 slots of 1Ch): add_texture from
  its site 0x3D319D to after its push_back (0x3D31A9). Full, the original
  reaches the assert's branch 0x3D35B9; the build moves the slots into a
  heap room of 2048 (each as it was) and pushes; with 2048 it reaches the
  assert;
- the scene's destructor from its site 0x14FDE6: the build gives the three
  rooms back to the heap and the lists are their inline rooms again; a
  scene without rooms frees nothing.

At four stack alignments. Says nothing about what a match draws; the
owner's test does.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EFLAGS

import patch_026g as base

MANAGER_GLOBAL = 0x226047C
PREDICATE = 0x3DF780                   # eax = the texture_query_desc (its name pointer at +110h); al 1: held
NAMES, NAME, OWN_NAMES = 0x1C, 0x110, 2048
NAMES_FULL = 0x132701                  # push_back 0x1326F0's branch to the assert
GATHER_PUSH, GATHER_PUSHED = 0x155205, 0x15521C
STREAMING_FULL = 0x155F6E              # push_back 0x155F60's branch to the assert
DESTROYED, DESTROYED_NEXT = 0x14FDE6, 0x14FDEC
POOL_PUSH, POOL_PUSHED = 0x3D319D, 0x3D31A9
POOL_FULL = 0x3D35B9                   # push_back 0x3D35B0's branch to the assert
SLOTS, SLOT, OWN_SLOTS = 0x64, 0x1C, 1024
SCENE_SIZE = 0x9B76A8
WORKER = 0x9B7598
LISTS = {                              # offset in the scene: (entry, the game's room, named)
    "streamable": (0x126158, 0x148, 2048, True),
    "requested": (0x1CA164, 0x118, 1024, True),
    "candidates": (0x98054C, 0x64, 2048, False),
}
MARK = {"streamable": 0x140, "requested": 0x114, "candidates": 0x60}   # a dword each entry's copy keeps
OURS = {"streamable": 4096, "requested": 2048, "candidates": 4096}
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT = 0x870048, 0x870108, 0x870040, 0x870090
PROCESS_ID, THREAD_ID = 0x8701B8, 0x870118
HEAP_ALLOC, PROCESS_HEAP, HEAP_FREE = 0x87004C, 0x870050, 0x870054
MEMORY, HEAP, STACK, STUBS, STOP, SCENE = 0x20000000, 0x30000000, 0x21000000, 0x22000000, 0x23000000, 0x40000000
REGISTER = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "esi": UC_X86_REG_ESI,
            "edi": UC_X86_REG_EDI, "ebx": UC_X86_REG_EBX, "ebp": UC_X86_REG_EBP}


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((MEMORY, 0x200000), (HEAP, 0x1000000), (STACK, 0x10000), (STUBS, 0x1000),
                         (STOP, 0x1000), (SCENE, (SCENE_SIZE + 0xFFF) & ~0xFFF)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.slots = (CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT, PROCESS_ID, THREAD_ID, HEAP_ALLOC,
                      PROCESS_HEAP, HEAP_FREE)
        for n, slot in enumerate(self.slots):
            self.word(slot, STUBS + 0x10 * n)
            emu.hook_add(UC_HOOK_CODE, self.kernel32, begin=STUBS + 0x10 * n, end=STUBS + 0x10 * n)
        for at in (STOP, NAMES_FULL, GATHER_PUSHED, STREAMING_FULL, DESTROYED_NEXT, POOL_PUSHED, POOL_FULL):
            emu.hook_add(UC_HOOK_CODE, self.on_stop, begin=at, end=at)
        self.next, self.heap, self.ticks, self.allocations, self.freed, self.lines = MEMORY, HEAP, 0, [], [], []

    def kernel32(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        slot = self.slots[(address - STUBS) // 0x10]
        pops, result = 0, 0
        if slot == CREATE_FILE:
            pops, result = 28, 0x1234
        elif slot == WRITE_FILE:
            self.lines.append(bytes(emu.mem_read(args[1], args[2])))
            self.word(args[3], args[2])
            pops, result = 20, 1
        elif slot == CLOSE_HANDLE:
            pops, result = 4, 1
        elif slot == TICK_COUNT:
            self.ticks += 1000
            result = self.ticks
        elif slot == PROCESS_ID:
            result = 4242
        elif slot == THREAD_ID:
            result = 77
        elif slot == PROCESS_HEAP:
            result = 0x5EA0
        elif slot == HEAP_FREE:
            assert args[0] == 0x5EA0, args
            self.freed.append(args[2])
            pops, result = 12, 1
        else:                              # HeapAlloc(heap, flags, size), zeroed
            assert args[0] == 0x5EA0 and args[1] & 8, args
            result, self.heap = self.heap, self.heap + (args[2] + 0xF) // 0x10 * 0x10
            emu.mem_write(result, bytes(args[2]))
            self.allocations.append((result, args[2]))
            pops = 12
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def on_stop(self, emu, address, size, _):
        self.stopped = address
        emu.emu_stop()

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def block(self, size):
        at, self.next = self.next, self.next + (size + 0xF) // 0x10 * 0x10
        self.emu.mem_write(at, bytes(size))
        return at

    def run(self, at, sp, **regs):
        self.stopped, self.fault = None, None
        for name, value in regs.items():
            self.emu.reg_write(REGISTER[name], value)
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            self.emu.emu_start(at, 0, count=20_000_000)
        except UcError as error:
            self.fault = (hex(self.emu.reg_read(UC_X86_REG_EIP)), str(error))

    def call(self, va, align, **regs):
        sp = STACK + 0x8000 - align - 4
        self.word(sp, STOP)
        self.run(va, sp, **regs)

    def list(self, at):
        """(begin, end, capacity end) of the list at `at`."""
        return tuple(self.read(at + 4 * i) for i in range(3))

    def set_list(self, at, begin, held, room, size):
        self.word(at, begin)
        self.word(at + 4, begin + held * size)
        self.word(at + 8, begin + room * size)

    def string(self, entry, text):
        """A fixed_string<260> at `entry` holding `text` (its pointers into its own room)."""
        body = entry + 0xC
        self.emu.mem_write(body, text + b"\0")
        self.word(entry, body)
        self.word(entry + 4, body + len(text))
        self.word(entry + 8, body + 0x104)


def name_of(n):
    return b"character/human/test_%05d" % n


def names_held(game, at):
    """The names of the list at `at`, each checked to point into its own entry."""
    begin, end, _ = game.list(at)
    out = []
    for entry in range(begin, end, NAME):
        b, e, c = (game.read(entry + 4 * i) for i in range(3))
        assert b == entry + 0xC and c == entry + 0xC + 0x104 and b <= e < c, (hex(entry), hex(b), hex(e), hex(c))
        out.append(bytes(game.emu.mem_read(b, e - b)))
    return out


def manager(game, held):
    """A resource manager with `held` names in its own room, its global set."""
    rm = game.block(0x88100)
    room = rm + NAMES + 0xC
    for n in range(held):
        game.string(room + n * NAME, name_of(n))
    game.set_list(rm + NAMES, room, held, OWN_NAMES, NAME)
    game.word(MANAGER_GLOBAL, rm)
    return rm


def predicate(game, align, name):
    """remove_texture_predicate for a query naming `name`: (stopped, AL)."""
    query = game.block(0x228)
    text = game.block(0x110)
    game.emu.mem_write(text, name + b"\0")
    game.word(query + 0x110, text)
    game.call(PREDICATE, align, eax=query, ebx=0x1BB, esi=0x51, edi=0xD1)
    return game.stopped, game.emu.reg_read(UC_X86_REG_EAX) & 0xFF


def fresh(game):
    game.next = MEMORY
    game.emu.mem_write(MEMORY, bytes(0x200000))
    game.emu.mem_write(STACK, bytes(0x10000))
    game.allocations, game.freed, game.lines = [], [], []


def scene_with(game, held):
    """A scene whose streamable textures hold `held` (each a name and a mark), with as many candidates
    and requests up to theirs, each in its own room."""
    game.emu.mem_write(SCENE, bytes(0x2000))
    for name, (at, size, room, named) in LISTS.items():
        begin = SCENE + at + 0xC
        count = min(held, room)
        for n in range(count):
            entry = begin + n * size
            if named:
                game.string(entry, b"%s_%05d" % (name.encode(), n))
            game.word(entry + MARK[name], 0xC0DE0000 + n)
        game.set_list(SCENE + at, begin, count, room, size)
    game.emu.mem_write(SCENE + WORKER + 0xC4, b"\0")
    return SCENE


def entries(game, name):
    """The list's entries as (its name, its mark), the names checked to point into their entries."""
    at, size, _, named = LISTS[name]
    begin, end, _ = game.list(SCENE + at)
    out = []
    for entry in range(begin, end, size):
        text = None
        if named:
            b, e, c = (game.read(entry + 4 * i) for i in range(3))
            assert b == entry + 0xC and c == entry + 0xC + 0x104 and b <= e < c, (name, hex(entry))
            text = bytes(game.emu.mem_read(b, e - b))
        out.append((text, game.read(entry + MARK[name])))
    return out


def gather_push(game, align):
    """gather's push from its site, its frame's entry at ESP+28h (a name, mark 0xFEED)."""
    sp = STACK + 0x8000 - align - 0x180
    entry = sp + 0x28
    game.emu.mem_write(entry, bytes(0x148))
    game.string(entry, b"pushed")
    game.word(entry + MARK["streamable"], 0xFEED)
    game.run(GATHER_PUSH, sp, ebx=SCENE, esi=SCENE + WORKER, ebp=0xEB9)


def pool_with(game, held):
    """A texture pool (named "64x64 dxt1") whose slots hold `held` in its own room, each marked."""
    pool = game.block(0x7070)
    game.emu.mem_write(pool + 0xC, b"64x64 dxt1\0")
    game.word(pool, pool + 0xC)
    for n in range(held):
        game.word(pool + 0x70 + n * SLOT + 8, 0x7E000000 + n)
    game.set_list(pool + SLOTS, pool + 0x70, held, OWN_SLOTS, SLOT)
    return pool


def slots_held(game, pool):
    begin, end, _ = game.list(pool + SLOTS)
    return [game.read(at + 8) for at in range(begin, end, SLOT)]


def pool_push(game, align, pool):
    """add_texture's push from its site, its frame's slot at ESP+18h (marked 0xFEED)."""
    sp = STACK + 0x8000 - align - 0x100
    game.emu.mem_write(sp + 0x18, bytes(SLOT))
    game.word(sp + 0x18 + 8, 0xFEED)
    game.run(POOL_PUSH, sp, esi=pool, ebp=0xEB9)


def check(stock_data, built_data):
    stock, ours = Game(stock_data), Game(built_data)
    checks = 0

    def expect(ok, what):
        nonlocal checks
        if not ok:
            sys.exit(f"FAIL: {what}")
        checks += 1

    for align in (0, 4, 8, 12):
        # The names: a push with room, in both.
        for game in (stock, ours):
            fresh(game)
            rm = manager(game, OWN_NAMES - 1)
            stopped, al = predicate(game, align, b"new")
            expect(stopped == STOP and al == 0 and names_held(game, rm + NAMES)[-1] == b"new"
                   and not game.allocations, "a new name with room is pushed, nothing allocated")
        # Full: the original asserts, the build moves to a room of 4096 and pushes.
        fresh(stock)
        manager(stock, OWN_NAMES)
        expect(predicate(stock, align, b"new")[0] == NAMES_FULL, "the original's 2049th name reaches the assert")
        fresh(ours)
        rm = manager(ours, OWN_NAMES)
        stopped, al = predicate(ours, align, b"new")
        begin, end, capacity = ours.list(rm + NAMES)
        held = names_held(ours, rm + NAMES)
        expect(stopped == STOP and al == 0 and ours.allocations == [(begin, 2 * OWN_NAMES * NAME)]
               and capacity == begin + 2 * OWN_NAMES * NAME and held == [name_of(n) for n in range(OWN_NAMES)] + [b"new"],
               f"the build moves 2048 names into a heap room of 4096 and pushes the new one ({ours.allocations})")
        expect(any(b"moved to a room of 4096" in line for line in ours.lines), "one line says so")
        stopped, al = predicate(ours, align, name_of(1234))
        expect(stopped == STOP and al == 1 and len(names_held(ours, rm + NAMES)) == OWN_NAMES + 1,
               "a name the room holds is found there, nothing pushed")
        # Our room full: the assert.
        for n in range(OWN_NAMES + 1, 2 * OWN_NAMES):
            ours.string(begin + n * NAME, name_of(n))
        ours.word(rm + NAMES + 4, begin + 2 * OWN_NAMES * NAME)
        allocated = len(ours.allocations)
        expect(predicate(ours, align, b"newer")[0] == NAMES_FULL and len(ours.allocations) == allocated,
               "with 4096 in the room the build reaches the assert, nothing allocated")

        # A texture pool: a push with room, in both; full, the original asserts and the build moves.
        for game in (stock, ours):
            fresh(game)
            pool = pool_with(game, 100)
            pool_push(game, align, pool)
            expect(game.stopped == POOL_PUSHED and slots_held(game, pool)[-1] == 0xFEED and not game.allocations,
                   "a pool's slot with room is pushed, nothing allocated")
        fresh(stock)
        pool = pool_with(stock, OWN_SLOTS)
        pool_push(stock, align, pool)
        expect(stock.stopped == POOL_FULL, "the original's 1025th slot reaches the assert")
        fresh(ours)
        pool = pool_with(ours, OWN_SLOTS)
        pool_push(ours, align, pool)
        begin, end, capacity = ours.list(pool + SLOTS)
        expect(ours.stopped == POOL_PUSHED and ours.allocations == [(begin, 2 * OWN_SLOTS * SLOT)]
               and capacity == begin + 2 * OWN_SLOTS * SLOT
               and slots_held(ours, pool) == [0x7E000000 + n for n in range(OWN_SLOTS)] + [0xFEED],
               f"the build moves 1024 slots into a heap room of 2048 and pushes ({ours.allocations})")
        expect(any(b"texture pool 64x64 dxt1, full at 1024, moved to a room of 2048" in line for line in ours.lines),
               "one line names the pool")
        ours.word(pool + SLOTS + 4, begin + 2 * OWN_SLOTS * SLOT)
        pool_push(ours, align, pool)
        expect(ours.stopped == POOL_FULL and len(ours.allocations) == 1, "with 2048 in its room the build reaches the assert")

        # A scene's streamable textures: a push with room, in both.
        for game in (stock, ours):
            fresh(game)
            scene_with(game, 100)
            gather_push(game, align)
            expect(game.stopped == GATHER_PUSHED and game.fault is None and entries(game, "streamable")[-1] == (b"pushed", 0xFEED)
                   and not game.allocations, f"a push with room goes in, nothing allocated ({game.stopped and hex(game.stopped)}, {game.fault}, {game.allocations}, {entries(game, 'streamable')[-1:]})")
        # Full: the original asserts; the build moves the three lists and pushes.
        fresh(stock)
        scene_with(stock, 2048)
        gather_push(stock, align)
        expect(stock.stopped == STREAMING_FULL, "the original's 2049th streamable texture reaches the assert")
        fresh(ours)
        scene_with(ours, 2048)
        before = {name: entries(ours, name) for name in LISTS}
        gather_push(ours, align)
        expect(ours.stopped == GATHER_PUSHED and ours.fault is None, f"the build pushes ({ours.stopped}, {ours.fault})")
        for name, (at, size, room, _) in LISTS.items():
            begin, end, capacity = ours.list(SCENE + at)
            pushed = [(b"pushed", 0xFEED)] if name == "streamable" else []
            expect(HEAP <= begin and capacity == begin + OURS[name] * size and (begin, OURS[name] * size) in ours.allocations
                   and entries(ours, name) == before[name] + pushed,
                   f"the build's {name} are in a heap room of {OURS[name]}, as they were")
        expect(len(ours.allocations) == 3, "three rooms")
        rooms = sorted(begin for begin, _ in ours.allocations)
        # Our room full: the assert.
        at, size, _, _ = LISTS["streamable"]
        begin = ours.read(SCENE + at)
        ours.word(SCENE + at + 4, begin + OURS["streamable"] * size)
        gather_push(ours, align)
        expect(ours.stopped == STREAMING_FULL and len(ours.allocations) == 3,
               "with 4096 in its room the build reaches the assert")
        # The destructor, the lists emptied: the rooms back, the lists inline.
        for name, (at, size, _, _) in LISTS.items():
            ours.word(SCENE + at + 4, ours.read(SCENE + at))
        ours.run(DESTROYED, STACK + 0x8000 - align - 0x100, ebp=SCENE)
        expect(ours.stopped == DESTROYED_NEXT and sorted(ours.freed) == rooms
               and all(ours.list(SCENE + at) == (SCENE + at + 0xC, SCENE + at + 0xC, SCENE + at + 0xC + room * size)
                       for at, size, room, _ in LISTS.values()),
               f"the destructor gives the three rooms back, the lists inline again ({ours.freed})")
        fresh(ours)
        scene_with(ours, 10)
        ours.run(DESTROYED, STACK + 0x8000 - align - 0x100, ebp=SCENE)
        expect(ours.stopped == DESTROYED_NEXT and not ours.freed, "a scene without rooms frees nothing")
    print(f"{checks} texture-list cases passed (the original asserts at 2049 names, 2049 streamable textures and "
          f"1025 slots of a pool; the build moves them into rooms twice the size when full and asserts past those)")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(),
                          None, open(base.late_join_bin(), 'rb').read())
    check(original, bytes(built))


if __name__ == "__main__":
    main()
