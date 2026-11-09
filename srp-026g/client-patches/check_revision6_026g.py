#!/usr/bin/env python3
"""Runs the three changes of patch revision 6 with Unicorn (2.1.4).

Usage: check_revision6_026g.py ORIGINAL

The exe is built as patch_026g.py builds it (the checked-in late-join code).
The game's own instructions run against made-up objects; the functions they
call are taken at their entry and written down. It shows the calling
convention, the frames and the order of the calls: not a game on a screen.

1. The hud's lists of players, made again (rebuild_lists, lists_hook): the
   game's own instructions of 0x234880 run from 0x234E71 on, in the frame
   rebuild_lists makes.
   - a run that rebuild_lists begins makes the three lists and nothing
     else, releases every value it made exactly once, restores
     ebx/ebp/esi/edi and the stack, and returns to its caller;
   - it reads no slot of the frame that neither it nor rebuild_lists wrote;
   - it writes nothing into the hud or the options;
   - a hud that is not this match's, or has no movie, is left alone;
   - the game's own run of 0x234880 passes lists_hook unchanged.
2. A box that is no box (box_hook, in loose_oct_tree::insert 0x39E30): a
   box of six finite floats is kept as it is; one with a not-a-number or an
   infinity anywhere becomes the box at the origin; the object and the tree
   reach insert_impl as before.
3. scene::remove_model 0x14FF60: for a model that is not in the scene it
   lets go of the model it was given (one count less; the model's end is
   called when that was the last) and returns, calling nothing else; for
   one that is, it goes on as before.
"""
import struct
import sys
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
    UC_X86_REG_ESI, UC_X86_REG_EIP, UC_X86_REG_ESP, UC_X86_REG_EBP,
)

import patch_026g as P

# late_join_026g.asm's entry, in the arena: the lists made again
# (stdcall hud, options; its run ends at the jump of 0x23572B).
REBUILD = P.arena().symbols["late_join_rebuild_lists"]
HUD, OPTIONS, HEAP, STACK, STOP = 0x1000000, 0x4000000, 0x1100000, 0x2008000, 0x3000000
MOVIES = {8: HUD + 0x2000, 0x10: HUD + 0x3000, 0x14: HUD + 0x4000}
INTERFACE, VTABLE = HEAP, HEAP + 0x100
FRAME = 0x5D4 + 16                      # 0x234880's locals and its four saved registers
PROFILE, COUNT_AT, TEAM_AT, SQUAD_AT = 0x45FC, 0x577B5, 0x45EC, 0x5782C
LIST_SET_PLAYERS, SET_PLAYERS_LIST, SET_ENEMY_ICON_IDS = 0xA0348C, 0xA034A4, 0xA034BC
# The game's functions the lists are made with: address -> (name, bytes of
# arguments the callee takes off the stack).
NATIVE = {0x511710: ("array", 4), 0x511700: ("object", 16), 0x511600: ("invoke", 16),
          0x4257D8: ("format", 0), 0x424580: ("copy", 0)}
# The value interface's methods the lists use, by vtable offset.
METHODS = {0x08: ("release", 8), 0x14: ("member", 16), 0x34: ("element", 12), 0x3C: ("append", 8)}
STUBS = HEAP + 0x1000                   # one made-up address for each method


def build(original):
    code = True
    upload = open(P.default_bin("section"), "rb").read()
    return P.build(original, "section", upload, None, code)[0]


class Machine:
    def __init__(self, data, teams, own_team=1, second_movie=True):
        uc = self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        self.written = set()
        for name, rva, size, raw, raw_size, flags in P.Headers(data).sections(data):
            # The code, and the read-only data for the names it passes on.
            if flags & 0x20000000 or name.startswith(b".rdata"):
                uc.mem_map(P.IMAGE_BASE + rva, (max(size, raw_size) + 4095) & ~4095)
                uc.mem_write(P.IMAGE_BASE + rva, bytes(data[raw:raw + raw_size]))
        uc.mem_map(HUD, 0x10000)
        uc.mem_map(HEAP, 0x10000)
        uc.mem_map(OPTIONS, 0x60000)
        uc.mem_map(0x2000000, 0x10000)
        uc.mem_map(STOP, 4096)
        # The late join's state page (.srpd, zeroed), where patch 39 keeps
        # the places the scoreboard was made with.
        uc.mem_map(P.LATE_JOIN_STATE_VA, 0x1000)
        # Every place in the match (kd_stats' online in the hud's state,
        # +0C0h + 8 x place): patch 39 gives a row to those alone
        # (check_tab_list_026g.py has the others).
        for place in range(len(teams)):
            uc.mem_write(HUD + 0xC0 + 8 * place, b"\x01")
        self.w(HUD + 0x1C, OPTIONS)
        self.w(HUD + 0x344, own_team)
        for at, movie in MOVIES.items():
            if at == 0x14 and not second_movie:
                continue
            self.w(HUD + at, movie)
            self.w(movie + 0x108, movie + 0x200)
            self.w(movie + 0x204, movie + 0x300)   # what every interface call gets in ecx
        self.w(INTERFACE, VTABLE)
        for offset in METHODS:
            self.w(VTABLE + offset, STUBS + offset)
        uc.mem_write(OPTIONS + COUNT_AT, bytes([len(teams)]))
        for place, team in enumerate(teams):
            profile = OPTIONS + place * PROFILE
            self.w(profile + TEAM_AT, team)
            uc.mem_write(profile + 8, b"player%d\0" % place)
            if place % 2:
                uc.mem_write(profile + 0x48, b"CLAN\0")
            uc.mem_write(OPTIONS + SQUAD_AT + place, bytes([place % 3]))
        self.teams, self.own_team = teams, own_team
        self.events, self.made, self.released, self.strays, self.outside = [], [], [], [], []
        self.frame = None
        uc.hook_add(UC_HOOK_CODE, self.code)
        uc.hook_add(UC_HOOK_MEM_READ, self.read_hook)
        uc.hook_add(UC_HOOK_MEM_WRITE, self.write_hook)

    def r(self, address):
        return struct.unpack("<I", self.uc.mem_read(address, 4))[0]

    def w(self, address, value):
        self.put(address, struct.pack("<I", value))

    def put(self, address, data):
        """A write the game's own function would have made: it counts as written."""
        self.uc.mem_write(address, data)
        self.written.update(range(address, address + len(data)))

    def text(self, address):
        return bytes(self.uc.mem_read(address, 80)).split(b"\0")[0].decode("latin1")

    def value(self, address):
        """A value of the interface: (its interface, type and flags, what it holds)."""
        return self.r(address), self.r(address + 4), self.r(address + 8)

    def leave(self, taken, result=None):
        esp = self.uc.reg_read(UC_X86_REG_ESP)
        if result is not None:
            self.uc.reg_write(UC_X86_REG_EAX, result)
        self.uc.reg_write(UC_X86_REG_EIP, self.r(esp))
        self.uc.reg_write(UC_X86_REG_ESP, esp + 4 + taken)

    def make(self, at, kind):
        handle = HEAP + 0x8000 + 16 * len(self.made)
        self.made.append(handle)
        self.w(at, INTERFACE)
        self.w(at + 4, 0x40 | (8 if kind == "array" else 7))   # managed: released through the interface
        self.w(at + 8, handle)
        return handle

    def code(self, uc, address, size, _):
        if address in NATIVE:
            name, taken = NATIVE[address]
            esp = uc.reg_read(UC_X86_REG_ESP)
            args = [self.r(esp + 4 + 4 * i) for i in range(5)]
            if name in ("array", "object"):
                self.events.append((name, self.make(args[0], name)))
            elif name == "invoke":
                held = self.value(args[2]) if args[2] else None
                assert uc.reg_read(UC_X86_REG_ECX) in [m + 0x300 for m in MOVIES.values()]
                self.events.append(("invoke", args[0], uc.reg_read(UC_X86_REG_ECX) - 0x300, held and held[2], args[3]))
            elif name == "format":      # (buffer, size, format, tag, name)
                made = b"[" + self.text(args[3]).encode() + b"] " + self.text(args[4]).encode() + b"\0"
                self.put(args[0], made[:args[1]])
            else:                       # (to, from, count)
                self.put(args[0], bytes(uc.mem_read(args[1], args[2])))
            self.leave(taken, 0)
        elif STUBS <= address < STUBS + 0x100:
            name, taken = METHODS[address - STUBS]
            esp = uc.reg_read(UC_X86_REG_ESP)
            args = [self.r(esp + 4 + 4 * i) for i in range(4)]
            assert uc.reg_read(UC_X86_REG_ECX) == INTERFACE, "a method of something that is not a value"
            if name == "release":       # (the value's flags, what it holds)
                self.released.append(args[1])
                self.events.append(("release", args[1]))
            elif name == "member":      # (the object, the member's name, the value, ...)
                _, kind, held = self.value(args[2])
                shown = self.text(held) if (kind & 0x8F) == 6 else held
                self.events.append(("member", args[0], self.text(args[1]), shown))
            elif name == "element":     # (the array, the index, the value)
                self.events.append(("element", args[0], args[1], self.value(args[2])[2]))
            else:                       # (the array, the value)
                self.events.append(("append", args[0], self.value(args[1])[2]))
            self.leave(taken, 1)

    def read_hook(self, uc, access, address, size, value, _):
        if self.frame is not None and self.frame <= address < self.frame + FRAME:
            if any(a not in self.written for a in range(address, address + size)):
                self.strays.append((uc.reg_read(UC_X86_REG_EIP), address - self.frame))

    def write_hook(self, uc, access, address, size, value, _):
        self.written.update(range(address, address + size))
        if HUD <= address < HUD + 0x2000 or OPTIONS <= address < OPTIONS + 0x60000:
            self.outside.append((uc.reg_read(UC_X86_REG_EIP), address))

    def call(self, hud=HUD, options=OPTIONS):
        saved = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222,
                 UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EDI: 0x44444444}
        for register, was in saved.items():
            self.uc.reg_write(register, was)
        # The late join's install and patch 39 call it as a stdcall
        # function (the return address lists_hook knows is its own).
        back = STOP
        self.uc.reg_write(UC_X86_REG_ESP, STACK)
        self.uc.mem_write(STACK - 0x1000, b"\xCC" * 0x1000)
        self.uc.mem_write(STACK, struct.pack("<III", back, hud, options))
        self.frame = STACK - FRAME
        self.uc.emu_start(REBUILD, back, count=400000)
        assert self.uc.reg_read(UC_X86_REG_EIP) == back, "it did not come back to its caller"
        assert self.uc.reg_read(UC_X86_REG_ESP) == STACK + 12, "the two arguments were not taken off the stack"
        for register, was in saved.items():
            assert self.uc.reg_read(register) == was, "a register the caller keeps was changed"


def check(original):
    changed = build(original)
    assert changed[P.file_offset(0x23572B)] == 0xE9, "the hook is not in this build"
    for teams, own_team, second_movie in (([1, 2], 1, True), ([1, 2, 1], 1, True), ([1, 2, 1], 2, True),
                                          ([2, 1, 1, 2, 1], 1, False), ([1] * 10 + [2] * 10, 2, True)):
        m = Machine(changed, teams, own_team, second_movie)
        m.call()
        invoked = [(event[1], event[2]) for event in m.events if event[0] == "invoke"]
        want = [(LIST_SET_PLAYERS, MOVIES[8])] + ([(SET_PLAYERS_LIST, MOVIES[0x14])] if second_movie else []) + \
            [(SET_ENEMY_ICON_IDS, MOVIES[0x10])]
        assert invoked == want, f"other invocations than the three lists: {[(hex(a), hex(b)) for a, b in invoked]}"
        assert sorted(m.released) == sorted(m.made), \
            f"{len(m.made)} values made, {len(m.released)} released ({len(set(m.released))} different)"
        assert not m.strays, "a slot of the frame read before anything wrote it: " + \
            ", ".join(f"{eip:#x} reads +{slot:#x}" for eip, slot in m.strays[:6])
        assert not m.outside, "the hud or the options written: " + \
            ", ".join(f"{eip:#x} writes {address:#x}" for eip, address in m.outside[:6])
        # The rows: every player once, in order, with its name and its side.
        rows = [event for event in m.events if event[0] == "member"]
        names = [event[3] for event in rows if event[2] == "name"]
        per_list = len(teams)
        expected = [f"[CLAN] player{i}" if i % 2 else f"player{i}" for i in range(per_list)]
        assert names[:per_list] == expected, names[:per_list]
        if second_movie:
            assert names[per_list:] == [f"player{i}" for i in range(per_list)], names[per_list:]
        sides = [event[3] for event in rows if event[2] == "team"][:per_list]
        assert sides == [1 if team == own_team else 2 for team in teams], sides
        # The enemies' ids are the second array made (the scoreboard's rows,
        # the first, are appended too since patch 39).
        arrays = [event[1] for event in m.events if event[0] == "array"]
        enemies = [event[2] for event in m.events if event[0] == "append" and event[1] == arrays[1]]
        scoreboard = [event for event in m.events if event[0] in ("append", "element") and event[1] == arrays[0]]
        assert len(scoreboard) == per_list and all(event[0] == "append" for event in scoreboard), \
            "the scoreboard's rows are not appended, one for each place in the match"
        assert enemies == [i for i, team in enumerate(teams) if team != own_team], enemies

    # A hud that is not the one this match's start made, or has lost a movie.
    for spoil in ("other options", "no first movie", "no third movie"):
        m = Machine(changed, [1, 2, 1])
        if spoil == "other options":
            m.w(HUD + 0x1C, OPTIONS + 4)
        else:
            m.w(HUD + (8 if spoil == "no first movie" else 0x10), 0)
        m.outside.clear()
        m.call()
        assert not m.events and not m.outside, spoil

    # The game's own run of 0x234880 (any other return address): the hook
    # does what the seven bytes it replaced did and goes on at 0x235732.
    m = Machine(changed, [1, 2])
    m.uc.reg_write(UC_X86_REG_ESP, STACK - FRAME)
    m.w(STACK, 0x1F8562)                               # the return into 0x1F83D0
    m.w(STACK - FRAME + 0x98, 0x5A5A5A5A)
    m.uc.reg_write(UC_X86_REG_ESI, 0x33333333)
    m.uc.emu_start(0x23572B, 0x235732, count=100)
    assert m.uc.reg_read(UC_X86_REG_EIP) == 0x235732 and m.uc.reg_read(UC_X86_REG_ECX) == 0x5A5A5A5A
    assert m.uc.reg_read(UC_X86_REG_ESP) == STACK - FRAME and m.uc.reg_read(UC_X86_REG_ESI) == 0x33333333
    print("PASS (the lists): made again for 2, 3, 5 and 20 players, from either side, with and without the second "
          "movie; every value released once; registers and stack as before; no stray read of the frame; the hud and "
          "the options not written; a foreign hud left alone; the game's own run unchanged")


INSERT, INSERT_IMPL = 0x39E30, 0x39D50
REMOVE_MODEL, FIND, GOES_ON = 0x14FF60, 0x3C5510, 0x15006C
SCENE = 0x5000000
SAVED = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222,
         UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EDI: 0x44444444}


def floats(*values):
    return struct.pack("<%df" % len(values), *values)


def check_box(changed):
    inf, nan = float("inf"), float("nan")
    good = (10.5, -3.0, 250.0, 12.5, 1.0, 251.25)
    cases = [(good, good), ((0.0,) * 6, (0.0,) * 6)]
    for at in range(6):
        for bad in (nan, inf, -inf):
            cases.append((good[:at] + (bad,) + good[at + 1:], (-1.0, -1.0, -1.0, 1.0, 1.0, 1.0)))
    for given, kept in cases:
        m = Machine(changed, [1, 2])
        tree, obj, box, method = HEAP + 0x2000, HEAP + 0x3000, HEAP + 0x4000, STUBS + 0x200
        m.w(obj, HEAP + 0x3800)
        m.w(HEAP + 0x3800 + 0x20, method)
        m.uc.mem_write(box, floats(*given))
        seen = []

        def visit(uc, address, size, _, m=m, obj=obj, box=box, tree=tree, method=method, seen=seen):
            if address == method:        # the object's box: (a place for it, the second argument)
                assert uc.reg_read(UC_X86_REG_ECX) == obj
                m.leave(8, box)
            elif address == INSERT_IMPL:
                assert uc.reg_read(UC_X86_REG_EAX) == tree and uc.reg_read(UC_X86_REG_EDI) == obj
                seen.append(bytes(uc.mem_read(obj + 4, 24)))
                m.leave(0)

        m.uc.hook_add(UC_HOOK_CODE, visit)
        for register, was in SAVED.items():
            m.uc.reg_write(register, was)
        m.uc.reg_write(UC_X86_REG_ECX, tree)
        m.uc.reg_write(UC_X86_REG_ESP, STACK)
        m.uc.mem_write(STACK, struct.pack("<III", STOP, obj, 0x77))
        m.uc.emu_start(INSERT, STOP, count=2000)
        assert m.uc.reg_read(UC_X86_REG_EIP) == STOP and m.uc.reg_read(UC_X86_REG_ESP) == STACK + 12
        for register, was in SAVED.items():
            assert m.uc.reg_read(register) == was
        assert seen == [floats(*kept)], (given, struct.unpack("<6f", seen[0]) if seen else None)
    print(f"PASS (the box): {len(cases)} boxes; a finite one is kept, one with a not-a-number or an infinity in any "
          "of its six numbers is the box at the origin; insert_impl gets the tree and the object as before")


def check_remove_model(changed):
    END_OF_MODEL = 0xE0210               # ecx = the model, eax = its count: called when the count is 0
    # (in the scene, how many hold the model) -> (the count afterwards, whether the model's end is called)
    for inside, held, left, ended in ((False, 2, 1, False), (False, 1, 0, True), (True, 2, 2, False)):
        m = Machine(changed, [1, 2])
        m.uc.mem_map(SCENE, 0xA00000)
        model, begin = HEAP + 0x5000, HEAP + 0x6000
        end = begin + 8
        m.w(SCENE + 0x9B358C, begin)
        m.w(SCENE + 0x9B3590, end)
        m.w(model + 0xD0, held)
        called, ends = [], []

        def visit(uc, address, size, _, m=m, called=called, ends=ends, inside=inside, begin=begin, end=end,
                  model=model):
            if address == FIND:          # eax = the list's begin, esi = its end -> where the model is
                assert uc.reg_read(UC_X86_REG_EAX) == begin and uc.reg_read(UC_X86_REG_ESI) == end
                m.leave(0, begin if inside else end)
            elif address == END_OF_MODEL:
                assert uc.reg_read(UC_X86_REG_ECX) == model and uc.reg_read(UC_X86_REG_EAX) == model + 0xD0
                ends.append(address)
                m.leave(0)
            elif address in (0x155D00, 0x15EE30, 0x111ED0, 0x111860, 0xF7200, 0x3A1B0, 0x3A600):
                called.append(address)

        m.uc.hook_add(UC_HOOK_CODE, visit)
        for register, was in SAVED.items():
            m.uc.reg_write(register, was)
        m.uc.reg_write(UC_X86_REG_ESP, STACK)
        m.uc.mem_write(STACK, struct.pack("<III", STOP, SCENE, model))
        m.uc.emu_start(REMOVE_MODEL, GOES_ON if inside else STOP, count=2000)
        assert not called and m.r(model + 0xD0) == left and bool(ends) == ended and len(ends) <= 1
        if inside:
            assert m.uc.reg_read(UC_X86_REG_EIP) == GOES_ON
        else:
            assert m.uc.reg_read(UC_X86_REG_EIP) == STOP
            assert m.uc.reg_read(UC_X86_REG_ESP) == STACK + 12
            for register, was in SAVED.items():
                assert m.uc.reg_read(register) == was
    print("PASS (remove_model): a model that is not in the scene: the function lets go of the model it was given "
          "(one count less; the model's end is called when that was the last, and only then), calls nothing "
          "else, registers and stack as before; a model that is: it goes on at 0x15006C as before")



if __name__ == "__main__":
    exe = Path(sys.argv[1]).read_bytes()
    check(exe)
    built = build(exe)
    check_box(built)
    check_remove_model(built)
