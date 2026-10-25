#!/usr/bin/env python3
"""Run the game's own effect code on the artefacts' first-person glow (the
content pack artefact-glow, srp-026g/scripts/artefact-glow/make_pack.py;
docs/notes/artefacts.md, "Your own glow").

    python check_artefact_glow_026g.py <original survarium.exe>

Requires Python `unicorn`. No exe patch makes the glow: an artefact's
effect config becomes a composite of the stock particle effect and a
first-person twin (`view` 0), and the stock code shows each to its own
presenters. This runs that code, in the original and in the build (the
build must not change it):

- the particle effect cook 0x25A730 from 0x25A86F to 0x25A8A2 (`view`,
  1 when absent) on the pack's own composite (make_pack.composite_effect,
  serialized by make_pack.serialize, loaded at an address): the stock child
  is 1, the first-person child 0;
- the presenters' constructors: first_person_game_effects_presenter
  0x2971C0 (its particle presenter +470h, view +518h) and
  third_person_game_effects_presenter 0x297570 (+8h, view +0B0h): 0 and 1;
- particle_game_effect_presenter's handler 0x2AC400 (the presenters'
  vtable +2Ch: [ebp+8] the particle presenter, [ebp+0Ch] the effect, edx
  the event) on an activation: it goes on to play the effect (0x2AC686)
  when the effect's view (+34h) is the presenter's (+0A8h) or 2 and the
  presenter has a player (+8h), and returns (0x2AC6B3) otherwise: the
  carrier's first-person presenter plays the twin, not the stock glow; any
  other player's third-person presenter plays the stock glow, not the twin;
- artefact_base::effect_calculator 0x313640 (vtable 0xA15A68 +0, the time
  calculator EFFECT 0x313350 gives the effect player): the effect is
  finished as soon as the artefact is not active (state 4), so both
  children end with the active.

Says nothing about how the particles look; the owner's test does.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, \
    UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_EBP

import patch_026g as base

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts", "artefact-glow"))
import make_pack  # noqa: E402

COOK_VIEW, COOK_VIEW_END = 0x25A86F, 0x25A8A2      # ebx = the effect's config; [esp+1Ch] = view
FP_PRESENTER, TP_PRESENTER = 0x2971C0, 0x297570    # edi = this, (world, hud); eax = this, edx = world
PRESENT = 0x2AC400                                 # cdecl-ish: (presenter, effect), edx = the event; `ret 8`
PLAY, SKIP = 0x2AC686, 0x2AC6B3                    # its activation, its return without one
CALCULATOR = 0x313640                              # thiscall (out, ...), `ret 10h`; out +8 = finished
ACTIVATE = 0

MEMORY, STACK, STOP = 0x20000000, 0x21008000, 0x22000000


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(MEMORY, 0x100000, UC_PROT_ALL)
        emu.mem_map(0x21000000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x1000, UC_PROT_ALL)
        self.next = MEMORY
        self.seen = None
        emu.hook_add(UC_HOOK_CODE, self.on_end, begin=PLAY, end=PLAY)
        emu.hook_add(UC_HOOK_CODE, self.on_end, begin=SKIP, end=SKIP)

    def on_end(self, emu, address, size, _):
        self.seen = address
        emu.emu_stop()

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at, size=4):
        return int.from_bytes(self.emu.mem_read(at, size), "little")

    def block(self, size):
        at, self.next = self.next, self.next + (size + 0xF) // 0x10 * 0x10
        self.emu.mem_write(at, bytes(size))
        return at

    def call(self, va, args=(), until=STOP, **regs):
        esp = STACK - 0x200
        for value in reversed(args):
            esp -= 4
            self.word(esp, value)
        esp -= 4
        self.word(esp, STOP)
        self.emu.reg_write(UC_X86_REG_ESP, esp)
        for name, value in regs.items():
            self.emu.reg_write(REGISTER[name], value)
        self.seen = None
        self.emu.emu_start(va, until, count=200_000)

    def load_config(self, data):
        """A config file as the game holds it loaded: its offsets made addresses."""
        at = self.block(len(data))
        out = bytearray(data)
        off, heap = 0, len(data)      # the records, then the names and strings (the first name opens them)
        while off < heap:
            dat, name, crc, t, cnt = struct.unpack_from("<QQIHH", data, off)
            if name:
                heap = min(heap, name)
            if t >= 3:                # a table, a string or a vector: an offset
                dat += at
            if name:
                name += at
            struct.pack_into("<QQIHH", out, off, dat, name, crc, t, cnt)
            off += 24
        self.emu.mem_write(at, bytes(out))
        return at

    def child(self, table, index):
        return self.read(table) + 24 * index


REGISTER = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "esi": UC_X86_REG_ESI,
            "edi": UC_X86_REG_EDI, "ebx": UC_X86_REG_EBX, "ebp": UC_X86_REG_EBP}

# The stock effect_artefact_onyx.options (time_to_finish 5, its particle system, two key frames).
STOCK_EFFECT = (make_pack.T_NAMED, [("effect", (make_pack.T_NAMED, [
    ("type", (make_pack.T_INT, make_pack.PARTICLE)),
    ("time_to_finish", (make_pack.T_INT, 5)),
    ("particle_system", (make_pack.T_STR, "fx/artifacts/human_onix")),
    ("key_frames", (make_pack.T_INDEXED, [
        (make_pack.T_NAMED, [("time", (make_pack.T_INT, t)), ("particle_system_time", (make_pack.T_INT, t))])
        for t in (0, 1)])),
]))])


def run(data):
    game = Game(data)
    checks = 0

    def expect(ok, what):
        nonlocal checks
        if not ok:
            sys.exit(f"FAIL: {what}")
        checks += 1

    # The cook's `view` on the pack's composite.
    composite = make_pack.composite_effect(STOCK_EFFECT, "fx/artifacts/human_onix_fp")
    config = game.load_config(make_pack.serialize(composite))
    effect = game.child(config, 0)                                 # the root's one child: `effect`
    kids = {}
    for k in range(2):                                             # effect's `type` and `effects`, CRC order
        rec = game.child(effect, k)
        if game.read(rec + 0x14, 2) == 4:
            kids["effects"] = rec
    views = []
    for k in range(2):
        child = game.child(kids["effects"], k)
        game.call(COOK_VIEW, until=COOK_VIEW_END, ebx=child)
        views.append(game.read(game.emu.reg_read(UC_X86_REG_ESP) + 0x1C))
    expect(views == [1, 0], f"the cook reads view 1 (absent) for the stock child, 0 for the twin: {views}")

    # The presenters' own views.
    fp = game.block(0x600)
    game.call(FP_PRESENTER, (0x12340000, 0), edi=fp)
    tp = game.block(0x300)
    game.call(TP_PRESENTER, edx=0x12340000, eax=tp)
    expect(game.read(fp + 0x470 + 0xA8) == 0 and game.read(fp) == 0xA0B5E4,
           "the first-person presenter's particle presenter shows view 0")
    expect(game.read(tp + 0x8 + 0xA8) == 1 and game.read(tp) == 0xA0B620,
           "a third-person presenter's particle presenter shows view 1")

    # The handler on an activation, per presenter and effect view.
    player = game.block(0x100)
    event = game.block(0x20)
    game.word(event + 4, ACTIVATE)

    def presents(presenter_view, effect_view, target=player):
        presenter = game.block(0x100)
        game.word(presenter + 8, target)
        game.word(presenter + 0xA8, presenter_view)
        fx = game.block(0x60)
        game.word(fx + 0x34, effect_view)
        game.call(PRESENT, (presenter, fx), edx=event)
        assert game.seen in (PLAY, SKIP), hex(game.seen or 0)
        return game.seen == PLAY

    expect(presents(0, 0), "the carrier's first-person presenter plays the first-person twin")
    expect(not presents(0, 1), "the carrier's first-person presenter skips the stock (third-person) glow")
    expect(presents(1, 1), "another player's third-person presenter plays the stock glow")
    expect(not presents(1, 0), "another player's third-person presenter skips the first-person twin")
    expect(presents(0, 2) and presents(1, 2), "view 2 would play in both")
    expect(not presents(0, 0, target=0), "a presenter with no player (the free camera) plays nothing")

    # The effect's end: the artefact's calculator.
    for state, finished in ((4, 0), (3, 1), (1, 1)):
        artefact = game.block(0x300)
        game.word(artefact + 0x280, state)
        calc = game.block(0x10)
        game.word(calc + 8, artefact)
        out = game.block(0x10)
        game.call(CALCULATOR, (out, 0, 0, 0), ecx=calc)
        expect(game.read(out + 8, 1) == finished,
               f"artefact state {state}: the effect {'ends' if finished else 'goes on'}")
    return checks


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    for data, name in ((original, "the original"), (patched, "the build")):
        print(f"artefact glow, {name}: {run(data)} checks as expected")


if __name__ == "__main__":
    main()
