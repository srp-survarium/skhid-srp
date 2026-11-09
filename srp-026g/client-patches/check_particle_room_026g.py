#!/usr/bin/env python3
"""Run the game's own particle presenters with patch 122's room, in the
original and in the build (README "Room for a presenter's particle effects").

    python check_particle_room_026g.py <original survarium.exe>

Requires Python `unicorn`. A particle presenter's list (buffer_vector
<effect_data>, 18h an entry) has room for six inside the presenter;
push_back 0x2ACC20 asserts at the seventh (the branch 0x2ACC2B, the int3
0x2ACC7F: the rev52/rev53 crashes). Each case runs from the patched sites
with the presenters the game's constructors build:

- a third-person presenter (0x297570), then game_world's loop from its site
  0x1F6B14 (`add eax,220h`): the original keeps the inline six, the build
  points the list at a heap room of 32; both go on with EAX the next holder;
- pushes through push_back: the original's seventh reaches the assert, the
  build holds 32 and its 33rd reaches it (the game's own limit, at 32);
- the same presenter built again at its address gets the same room; another
  presenter its own;
- a first-person presenter (0x2971C0) at the game world's +1D98h, then
  game_world's site 0x1F6884 (EBP the world): the same room of 32 for its
  list (+470h);
- deserialize 0x2AC870's local list from its site 0x2AC8A6: a presenter with
  a room reads into the room's second half (capacity 32); one without keeps
  the frame's six, as the original;
- the swap 0x2ACD00 (the presenter's list and the local): six and four
  entries swap to the byte as the original's; twenty and ten swap in the
  build in place, while the original writes past its stack room of six.

At four stack alignments. Says nothing about what a match plays; the owner's
test does.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EFLAGS

import patch_026g as base

TP_PRESENTER = 0x297570                # eax = this, edx = the world; keeps eax
FP_PRESENTER = 0x2971C0                # edi = this, (world, hud); ret 8
TP_MADE, TP_MADE_NEXT = 0x1F6B14, 0x1F6B19    # add eax,220h / sub esi,edi
FP_MADE, FP_MADE_NEXT = 0x1F6884, 0x1F688A    # mov [ebp+4D38h],eax / mov [ebp+4D3Ch],ebx
FP_IN_WORLD = 0x1D98                  # game_world's first-person presenter
PUSH_BACK = 0x2ACC20                   # esi = the list, (the entry); ret 4
PUSH_FULL = 0x2ACC2B                   # its branch to the assert (end == capacity end)
READ_ROOM, READ_ROOM_NEXT = 0x2AC8A6, 0x2AC8AE    # mov [esp+4Ch],edx / mov [esp+58h],eax / cmp edx,edi
SWAP = 0x2ACD00                        # edi = the presenter's list, esi = the local; ret
ASSIGN, ASSIGN_END = 0x2AD360, 0x2AD3E0
ENTRY = 0x18
TP_LIST, FP_LIST = 0x8 + 0xC, 0x470 + 0xC     # the particle list in each presenter
ROOM = 32
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT = 0x870048, 0x870108, 0x870040, 0x870090
PROCESS_ID, THREAD_ID = 0x8701B8, 0x870118
HEAP_ALLOC, PROCESS_HEAP = 0x87004C, 0x870050
MEMORY, HEAP, STACK, STUBS, STOP = 0x20000000, 0x24000000, 0x21000000, 0x22000000, 0x23000000
WORLD = 0x12340000
REGISTER = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "esi": UC_X86_REG_ESI,
            "edi": UC_X86_REG_EDI, "ebx": UC_X86_REG_EBX, "ebp": UC_X86_REG_EBP}
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP,
        UC_X86_REG_ESP)


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((MEMORY, 0x100000), (HEAP, 0x100000), (STACK, 0x10000), (STUBS, 0x1000),
                         (STOP, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        # srp_patches.log, the refusals' clock, the locks' thread and the heap, as kernel32 would.
        self.slots = (CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT, PROCESS_ID, THREAD_ID, HEAP_ALLOC,
                      PROCESS_HEAP)
        for n, slot in enumerate(self.slots):
            self.word(slot, STUBS + 0x10 * n)
            emu.hook_add(UC_HOOK_CODE, self.kernel32, begin=STUBS + 0x10 * n, end=STUBS + 0x10 * n)
        self.stops = {STOP, PUSH_FULL, TP_MADE_NEXT, FP_MADE_NEXT, READ_ROOM_NEXT}
        for at in self.stops:
            emu.hook_add(UC_HOOK_CODE, self.on_stop, begin=at, end=at)
        self.next, self.heap, self.ticks, self.allocations = MEMORY, HEAP, 0, []

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
        self.stopped, self.lines, self.fault = None, [], None
        fixed = {name: 0x1000 * (n + 1) for n, name in enumerate(REGISTER)}
        for name, value in {**fixed, **regs}.items():
            self.emu.reg_write(REGISTER[name], value)
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            self.emu.emu_start(at, 0, count=2_000_000)
        except UcError as error:
            self.fault = (self.emu.reg_read(UC_X86_REG_EIP), str(error))

    def call(self, va, args, align, **regs):
        sp = STACK + 0x8000 - align
        for value in reversed(args):
            sp -= 4
            self.word(sp, value)
        sp -= 4
        self.word(sp, STOP)
        self.run(va, sp, **regs)

    def regs(self):
        return tuple(self.emu.reg_read(r) for r in KEPT)

    def list(self, at):
        """(begin, end, capacity end) of the list at `at`."""
        return tuple(self.read(at + 4 * i) for i in range(3))


def fresh(game):
    game.next = MEMORY
    game.emu.mem_write(MEMORY, bytes(0x100000))
    game.emu.mem_write(STACK, bytes(0x10000))


def third_person(game, align, at=None):
    """A third-person presenter built by its constructor, then game_world's
    site: (the presenter, EAX after the site)."""
    tp = at if at is not None else game.block(0x300)
    game.call(TP_PRESENTER, (), align, eax=tp, edx=WORLD)
    assert game.stopped == STOP and game.fault is None, (game.stopped, game.fault)
    game.run(TP_MADE, STACK + 0x8000 - align, eax=tp)
    assert game.stopped == TP_MADE_NEXT and game.fault is None, (game.stopped, game.fault)
    return tp, game.emu.reg_read(UC_X86_REG_EAX)


def pushes(game, vec, count, align):
    """Pushes `count` blank entries: how many returned before one reached the assert's branch."""
    entry = game.block(ENTRY)
    for n in range(count):
        game.call(PUSH_BACK, (entry,), align, esi=vec)
        if game.stopped != STOP:
            return n, game.stopped
    return count, STOP


def fill(game, begin, count, mark):
    """`count` entries without references (+0 and +0Ch 0), told apart by `mark`."""
    for n in range(count):
        at = begin + n * ENTRY
        game.emu.mem_write(at, struct.pack("<IIIIII", 0, mark + n, mark ^ n, 0, mark * 3 + n, mark * 5 + n))


def entries(game, vec):
    begin, end, _ = game.list(vec)
    return bytes(game.emu.mem_read(begin, end - begin))


def list_in(game, slots, count, mark):
    """A list struct over a block of `slots` entries, holding `count` of them."""
    vec, room = game.block(0xC), game.block(slots * ENTRY)
    game.word(vec, room)
    game.word(vec + 4, room + count * ENTRY)
    game.word(vec + 8, room + slots * ENTRY)
    fill(game, room, count, mark)
    return vec


def check(stock_data, built_data):
    stock, ours = Game(stock_data), Game(built_data)
    checks = 0

    def expect(ok, what):
        nonlocal checks
        if not ok:
            sys.exit(f"FAIL: {what}")
        checks += 1

    for align in (0, 4, 8, 12):
        # A third-person presenter: inline six in the original, a room of 32 in the build.
        results = {}
        for name, game in (("stock", stock), ("ours", ours)):
            fresh(game)
            tp, next_eax = third_person(game, align)
            results[name] = (tp, next_eax, game.list(tp + TP_LIST), game.regs())
        tp, next_eax, (begin, end, capacity), regs = results["stock"]
        expect(next_eax == tp + 0x220 and begin == end == tp + 0x20 and capacity == tp + 0x20 + 6 * ENTRY,
               f"the original keeps the inline six and goes on to the next holder ({results['stock'][:3]})")
        tp, next_eax, (begin, end, capacity), ours_regs = results["ours"]
        expect(next_eax == tp + 0x220 and HEAP <= begin == end and capacity == begin + ROOM * ENTRY,
               f"the build's list is a heap room of {ROOM} ({results['ours'][:3]})")
        expect(ours_regs == regs, "the build's site leaves the other registers as the original's")
        expect(ours.allocations[-1][1] == 2 * ROOM * ENTRY, f"one room: the list's and its reads' ({ours.allocations})")

        # Pushes: the seventh asserts in the original; 32 hold in the build, the 33rd asserts.
        made, stopped = pushes(stock, results["stock"][0] + TP_LIST, 8, align)
        expect((made, stopped) == (6, PUSH_FULL), f"the original's seventh push reaches the assert ({made}, {stopped})")
        made, stopped = pushes(ours, results["ours"][0] + TP_LIST, ROOM + 1, align)
        expect((made, stopped) == (ROOM, PUSH_FULL),
               f"the build holds {ROOM} and its next push reaches the assert ({made}, {stopped})")

        # Built again at the same address: the same room; another presenter: its own.
        room = results["ours"][2][0]
        allocated = len(ours.allocations)
        third_person(ours, align, at=results["ours"][0])
        expect(ours.list(results["ours"][0] + TP_LIST)[0] == room and len(ours.allocations) == allocated,
               "the same presenter built again gets its room back, allocated once")
        # (The build's table outlives a case: from the second alignment on,
        # this address is one it has seen, and its room comes back.)
        other, _ = third_person(ours, align)
        begin, end, capacity = ours.list(other + TP_LIST)
        expect(HEAP <= begin == end != room and capacity == begin + ROOM * ENTRY
               and len(ours.allocations) <= allocated + 1, "another presenter gets its own room")

        # A first-person presenter.
        for name, game in (("stock", stock), ("ours", ours)):
            fresh(game)
            world = game.block(0x5000)
            fp = world + FP_IN_WORLD
            game.call(FP_PRESENTER, (WORLD, 0), align, edi=fp)
            expect(game.stopped == STOP and game.fault is None, f"{name}: the first-person constructor returns")
            game.run(FP_MADE, STACK + 0x8000 - align, eax=0xFFFFFFFF, ebp=world)
            expect(game.stopped == FP_MADE_NEXT and game.fault is None, f"{name}: the site goes on ({game.fault})")
            begin, end, capacity = game.list(fp + FP_LIST)
            if name == "stock":
                expect(begin == end == fp + 0x488 and capacity == begin + 6 * ENTRY, "the original's six inline")
            else:
                expect(HEAP <= begin == end and capacity == begin + ROOM * ENTRY, "the build's first-person room")

        # deserialize's local list, from its site.
        for name, game in (("stock", stock), ("ours", ours)):
            fresh(game)
            given, _ = third_person(game, align)
            bare = game.block(0x300)
            game.call(TP_PRESENTER, (), align, eax=bare, edx=WORLD)
            for presenter in (given, bare):
                sp = STACK + 0x6000 - align
                game.emu.mem_write(sp, bytes(0x100))
                game.word(sp + 0x18, presenter + 0x8)
                game.word(sp + 0x50, sp + 0x5C)
                game.word(sp + 0x54, sp + 0x5C)
                game.run(READ_ROOM, sp, eax=sp + 0xEC, edx=7, edi=0)
                expect(game.stopped == READ_ROOM_NEXT and game.fault is None, f"{name}: the read site goes on")
                read = game.list(sp + 0x50)
                expect(game.read(sp + 0x4C) == 7, f"{name}: the site's own store of the count")
                if name == "ours" and presenter == given:
                    room = game.list(given + TP_LIST)[0]
                    expect(read == (room + ROOM * ENTRY, room + ROOM * ENTRY, room + 2 * ROOM * ENTRY),
                           f"a presenter with a room reads into its second half ({read})")
                else:
                    expect(read == (sp + 0x5C, sp + 0x5C, sp + 0xEC), f"{name}: the frame's own six ({read})")

        # The swap: six and four to the byte as the original's.
        outcome = []
        for game in (stock, ours):
            fresh(game)
            kept, read = list_in(game, 6, 6, 0x100), list_in(game, 6, 4, 0x200)
            want_kept, want_read = entries(game, read), entries(game, kept)
            game.call(SWAP, (), align, edi=kept, esi=read)
            expect(game.stopped == STOP and game.fault is None, f"the swap returns ({game.stopped}, {game.fault})")
            expect(entries(game, kept) == want_kept and entries(game, read) == want_read, "six and four swapped")
            outcome.append((entries(game, kept), entries(game, read), game.list(kept), game.list(read)))
        expect(outcome[0] == outcome[1], "the build's swap is the original's, to the byte")

        # Twenty and ten in rooms of 32: the build swaps; the original writes past its stack room.
        fresh(ours)
        kept, read = list_in(ours, ROOM, 20, 0x300), list_in(ours, ROOM, 10, 0x400)
        want_kept, want_read = entries(ours, read), entries(ours, kept)
        ours.call(SWAP, (), align, edi=kept, esi=read)
        expect(ours.stopped == STOP and ours.fault is None and entries(ours, kept) == want_kept
               and entries(ours, read) == want_read, "the build swaps twenty and ten in place")
        fresh(stock)
        kept, read = list_in(stock, ROOM, 20, 0x300), list_in(stock, ROOM, 10, 0x400)
        entry_sp = STACK + 0x8000 - align - 4
        past = []

        def on_write(emu, access, address, size, value, _):
            if ASSIGN <= emu.reg_read(UC_X86_REG_EIP) < ASSIGN_END and entry_sp - 4 <= address < entry_sp + 0x400:
                past.append(address)
        hook = stock.emu.hook_add(UC_HOOK_MEM_WRITE, on_write)
        stock.call(SWAP, (), align, edi=kept, esi=read)
        stock.emu.hook_del(hook)
        expect(past, "the original's swap writes past its stack room of six")
    print(f"{checks} particle-room cases passed (the original asserts at its seventh entry and overruns its "
          f"swap's stack room; the build holds {ROOM} and swaps in place)")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(),
                          None, open(base.late_join_bin(), 'rb').read())
    check(original, bytes(built))


if __name__ == "__main__":
    main()
