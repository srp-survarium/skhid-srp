#!/usr/bin/env python3
"""Run the game's own presenters on events for an effect they hold and for
one they hold no entry of, in the original and in the build (patch 117,
README "A presenter ignores an effect it does not hold").

    python check_effect_presenters_026g.py <original survarium.exe>

Requires Python `unicorn`. The presenters are a third-person
game-effects presenter's, made by its constructor 0x297570: the sound
presenter at +0B4h (sound_game_effect_presenter, vtable 0xA0D1D0) and the
particle presenter at +8h (particle_game_effect_presenter, 0xA0D2E4), each
given an event by its `on_event` (0x2A9EF0: presenter, effect, event;
0x2AC400: presenter, effect, EDX the event):

- a sound stop on an empty list (the 2026-11-06 crash: base_player::remove
  cleared the presenter, a damage zone stopped its effect the tick after):
  the original erases from one entry before the list, so it takes the
  presenter's own fields for an entry, counts down the word at its capacity
  pointer's +28h and frees its seed (+10h) when the word at the seed's +4 is
  1 (it reaches the free at 0x1FC20F); the build returns, the presenter
  unchanged, and logs one line;
- a sound update on an empty list: the original reads its entry 0 (+10h at
  0x2AA0BD); the build returns, the presenter unchanged;
- a particle stop on an empty list: the original calls the erase 0x2ACD80
  with the list's begin as the end of the range (one entry before it); the
  build returns at 0x2AC6B3 without it;
- a sound stop and a sound update of an effect the presenter holds, and a
  particle stop at the erase of a held entry (from the site, the search's
  index 0, 2 and -1 of three): the build does what the original does, to
  the byte and register.

At four stack alignments (the generated entries realign). Says nothing about
what a match plays; the owner's test does.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EFLAGS

import patch_026g as base

TP_PRESENTER = 0x297570                # eax = this, edx = the world
SOUND_EVENT = 0x2A9EF0                 # (presenter, effect, event), ret 0Ch
PARTICLE_EVENT = 0x2AC400              # (presenter, effect), edx = the event; ret 8
SOUND_FREE = 0x1FC20F                  # the entries' destroy, an effect reference at 0: its free
SOUND_UPDATE_ENTRY = 0x2AA0BD          # mov ecx,[esi+10h]: the update's entry
PARTICLE_SEARCHED = 0x2AC656           # the particle stop after its search ([esp+14h] the index)
PARTICLE_ERASE = 0x2AC678              # its call of the erase 0x2ACD80
PARTICLE_RETURN = 0x2AC6B3             # its return without one
SOUND_VTABLE, PARTICLE_VTABLE = 0xA0D1D0, 0xA0D2E4
STOP_KIND, UPDATE_KIND = 1, 2
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT = 0x870048, 0x870108, 0x870040, 0x870090
# GetCurrentProcessId, a constant: a clock read from it would stand still and
# drop every refusal line after the first (each case expects one).
PROCESS_ID = 0x8701B8
# GetCurrentThreadId: the log's one writer asks whose line it is.
THREAD_ID = 0x870118
MEMORY, STACK, STUBS, STOP = 0x20000000, 0x21000000, 0x22000000, 0x23000000
WORLD = 0x12340000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
        UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP)


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((MEMORY, 0x100000), (STACK, 0x10000), (STUBS, 0x1000), (STOP, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        # srp_patches.log and the refusals' clock, as kernel32 would.
        for n, slot in enumerate((CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT, PROCESS_ID, THREAD_ID)):
            self.word(slot, STUBS + 0x10 * n)
            emu.hook_add(UC_HOOK_CODE, self.kernel32, begin=STUBS + 0x10 * n, end=STUBS + 0x10 * n)
        for at in (STOP, SOUND_FREE, PARTICLE_ERASE, PARTICLE_RETURN):
            emu.hook_add(UC_HOOK_CODE, self.on_stop, begin=at, end=at)
        self.next = MEMORY
        self.ticks = 0

    def kernel32(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        n = (address - STUBS) // 0x10
        if n == 0:                         # CreateFileA: 7 arguments, a handle
            pops, result = 28, 0x1234
        elif n == 1:                       # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])))
            self.word(args[3], args[2])
            pops, result = 20, 1
        elif n == 2:                       # CloseHandle
            pops, result = 4, 1
        elif n == 3:                       # GetTickCount: a second on, each refusal a line
            self.ticks += 1000
            pops, result = 0, self.ticks
        elif n == 4:                       # GetCurrentProcessId
            pops, result = 0, 4242
        else:                              # GetCurrentThreadId: one thread
            pops, result = 0, 1
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
        for name, value in regs.items():
            self.emu.reg_write(REGISTER[name], value)
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
        # The same machine in both games, whatever the case before left.
        fixed = {name: 0x1000 * (n + 1) for n, name in enumerate(REGISTER)}
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.run(va, sp, **{**fixed, **regs})

    def snapshot(self, at, size):
        return bytes(self.emu.mem_read(at, size))


REGISTER = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "esi": UC_X86_REG_ESI,
            "edi": UC_X86_REG_EDI, "ebx": UC_X86_REG_EBX, "ebp": UC_X86_REG_EBP}


class Scene:
    """A third-person presenter, its player, one sound effect and one
    particle effect, fresh in one game's memory."""

    def __init__(self, game):
        self.game = g = game
        g.next = MEMORY
        g.emu.mem_write(MEMORY, bytes(0x100000))
        g.emu.mem_write(STACK, bytes(0x10000))
        self.tp = g.block(0x300)
        g.call(TP_PRESENTER, (), 0, eax=self.tp, edx=WORLD)
        assert g.stopped == STOP and g.fault is None, (g.stopped, g.fault)
        self.sound, self.particle = self.tp + 0xB4, self.tp + 0x8
        assert g.read(self.sound) == SOUND_VTABLE and g.read(self.particle) == PARTICLE_VTABLE
        self.player = g.block(0x100)
        g.word(self.particle + 0x8, self.player)        # a presenter with a player shows its effects
        # The sound effect an update reads: its emitter (+0Ch, +114h 0: no
        # length), its start (+20h) and its volume keys (+38h -> -> 0.0s).
        self.effect = g.block(0x60)
        emitter, keys, holder = g.block(0x120), g.block(0x40), g.block(0x10)
        g.word(self.effect + 0xC, emitter)
        g.word(holder, keys)
        g.word(self.effect + 0x38, holder)
        g.word(self.effect + 0x34, 1)                   # a particle effect's view: third person
        self.event = g.block(0x20)
        # Its seed taken for an effect reference whose count is 1.
        self.seed = g.block(0x20)
        g.word(self.seed + 4, 1)
        g.word(self.sound + 0x10, self.seed)

    def hold_sound(self):
        """The sound presenter's entry 0 plays the effect: a loose pointer to
        it (count 2, so no free), no sound object."""
        g, node = self.game, self.game.block(0x10)
        g.word(node, self.effect + 4)
        g.word(node + 4, 2)
        begin = g.read(self.sound + 0x18)
        g.word(begin, node)
        g.word(self.sound + 0x1C, begin + 0x14)
        return node

    def sound_event(self, kind, align):
        self.game.word(self.event + 4, kind)
        self.game.call(SOUND_EVENT, (self.sound, self.effect, self.event), align)

    def particle_event(self, kind, align):
        self.game.word(self.event + 4, kind)
        self.game.call(PARTICLE_EVENT, (self.particle, self.effect), align, edx=self.event)


def check(stock_data, built_data):
    stock, ours = Game(stock_data), Game(built_data)
    checks = 0

    def expect(ok, what):
        nonlocal checks
        if not ok:
            sys.exit(f"FAIL: {what}")
        checks += 1

    for align in (0, 4, 8, 12):
        # A sound stop on an empty list.
        s, o = Scene(stock), Scene(ours)
        before = o.game.snapshot(o.tp, 0x300)
        s.sound_event(STOP_KIND, align)
        expect(stock.stopped == SOUND_FREE and stock.emu.reg_read(UC_X86_REG_EAX) == s.seed,
               f"the original frees the sound presenter's seed on a stop it holds no entry of ({stock.stopped})")
        capacity = stock.read(s.sound + 0x20)
        expect(stock.read(capacity + 0x28) == 0xFFFFFFFF,
               "the original counts down the word at its capacity pointer's +28h (0 -> -1, as in the dump)")
        o.sound_event(STOP_KIND, align)
        expect(ours.stopped == STOP and ours.fault is None, f"the build returns ({ours.stopped}, {ours.fault})")
        expect(ours.snapshot(o.tp, 0x300) == before and ours.read(o.seed + 4) == 1,
               "the build leaves the presenter and the seed's word as they were")
        # A run of one line before it may be counted first (vostok::log: "the line above: N more times").
        said = [l for l in ours.lines if b"(the line above: " not in l]
        expect(len(said) == 1 and b"a sound's stop" in said[0], f"one line logged: {ours.lines}")

        # A sound update on an empty list.
        s, o = Scene(stock), Scene(ours)
        before = o.game.snapshot(o.tp, 0x300)
        s.sound_event(UPDATE_KIND, align)
        expect(stock.fault is not None and stock.fault[0] == SOUND_UPDATE_ENTRY,
               f"the original reads entry 0's +10h on an update it holds no entry of ({stock.fault})")
        o.sound_event(UPDATE_KIND, align)
        expect(ours.stopped == STOP and ours.fault is None and ours.snapshot(o.tp, 0x300) == before,
               f"the build returns, the presenter unchanged ({ours.stopped}, {ours.fault})")

        # A sound stop and update of a held effect: the same in both.
        for kind in (STOP_KIND, UPDATE_KIND):
            results = []
            for scene in (Scene(stock), Scene(ours)):
                node = scene.hold_sound()
                scene.sound_event(kind, align)
                g = scene.game
                results.append((g.stopped, g.fault, g.snapshot(scene.tp, 0x300), g.snapshot(node, 0x10),
                                tuple(g.emu.reg_read(r) for r in (UC_X86_REG_EBX, UC_X86_REG_ESI,
                                                                   UC_X86_REG_EDI, UC_X86_REG_EBP,
                                                                   UC_X86_REG_ESP))))
                if kind == STOP_KIND:
                    expect(g.read(scene.sound + 0x1C) == g.read(scene.sound + 0x18) and g.read(node + 4) == 1,
                           "the held stop empties the list and lets its effect reference go")
            expect(results[0] == results[1] and results[0][0] == STOP,
                   f"a held effect's {'stop' if kind == STOP_KIND else 'update'} runs as the original's")

        # A particle stop on an empty list.
        s, o = Scene(stock), Scene(ours)
        before = o.game.snapshot(o.tp, 0x300)
        s.particle_event(STOP_KIND, align)
        begin = stock.read(s.particle + 0xC)
        expect(stock.stopped == PARTICLE_ERASE and stock.emu.reg_read(UC_X86_REG_EAX) == begin,
               f"the original erases the entry before the particle list ({stock.stopped})")
        o.particle_event(STOP_KIND, align)
        expect(ours.stopped == PARTICLE_RETURN and ours.fault is None and ours.snapshot(o.tp, 0x300) == before,
               f"the build returns without the erase ({ours.stopped}, {ours.fault})")

        # The particle stop after its search, from the site: indices 0 and 2
        # of three go to the erase as in the original; -1 returns.
        for index in (0, 2, 0xFFFFFFFF):
            results = []
            for game in (stock, ours):
                scene = Scene(game)
                entries = game.block(0x48)
                game.word(scene.particle + 0xC, entries)
                game.word(scene.particle + 0x10, entries + 0x48)
                frame = STACK + 0x8000 - 0x100 - align            # the function's EBP
                game.word(frame, 0)
                game.word(frame + 4, STOP)
                sp = frame - 0x60
                game.word(sp + 0x14, index)
                game.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
                game.run(PARTICLE_SEARCHED, sp, ebx=scene.particle, edi=scene.particle + 0xC, ebp=frame,
                         esi=0, eax=0x11111111, ecx=0x22222222, edx=0x33333333)
                stack = game.snapshot(sp, 0x20)
                results.append((game.stopped, game.fault, tuple(game.emu.reg_read(r) for r in REGS),
                                game.snapshot(game.emu.reg_read(UC_X86_REG_ESP), 0x10), stack))
            if index == 0xFFFFFFFF:
                expect(results[0][0] == PARTICLE_ERASE and results[1][0] == PARTICLE_RETURN and results[1][1] is None,
                       f"index -1: the original erases, the build returns ({results[0][0]}, {results[1][:2]})")
            else:
                expect(results[0] == results[1] and results[0][0] == PARTICLE_ERASE,
                       f"index {index}: the build erases as the original")
    print(f"{checks} presenter cases passed (the original's miss erases or faults, the build ignores it; "
          f"held effects as the original)")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(),
                          None, open(base.late_join_bin(), 'rb').read())
    check(original, bytes(built))


if __name__ == "__main__":
    main()
