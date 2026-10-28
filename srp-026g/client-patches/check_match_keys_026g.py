#!/usr/bin/env python3
"""Run patch 58 (match_keys_026g.asm: the kit keys and the match chat option)
in the default build on made-up game objects.

    python check_match_keys_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the patch's four entries run from the game's own sites:

- the poll: the game world's input handler's slot +14h (0x9FC59C), as the
  input world's tick calls it (thiscall, the input world, `ret 4`), with
  GetTickCount (its import 0x870090) a stub answering the case's time, from
  each state of kit_keys with the cursor shown and not: CLOSED -> OPENING
  with the time, OPENING -> OPEN from 300 ms (KIT_KEYS_DELAY) after it, also
  over GetTickCount's wrap, CLOSED when not shown; the input world popped,
  ebx esi edi ebp kept. The original's slot is `ret 4` (0x75AF0);
- the keys: the game world's on_keyboard_action (0x25CD00) is run from its
  kind's test (0x25CD41) with a made-up input handler in edi (the hud at +4:
  its cursor flag +2DCh, its movie's holder +8 -> +108h -> +4) and the
  function's frame in ebp (the key, the kind), in each state of kit_keys,
  until it reaches 0x25CD4B (a key down) or 0x25CF0D (any other).
  Scaleform's Invoke (0x511600) is a stub that records its movie, name,
  result and argument. A case must invoke `root.select_kit_key` with the kit
  (an unsigned int GFx value) exactly for the keys 1, 2 and 3 going down in
  OPEN while the cursor is shown and the movie is there, and arrive where the
  original exe does with every register, the frame and the stack as the
  original's;
- the input ticks: the game's own input world tick (0x3B7BD0) and keyboard
  process (0x3B9AF0) run over a made-up input world whose keyboard holds the
  poll's DirectInput events and keys down (its execute a `ret`), its one
  handler the game world's (vtable 0x9FC588; on_keyboard_action goes on at
  0x25CF0D from 0x25CD4B: the key's actions are not this patch's). Before
  each tick the hud's cursor is what the game tick before it left (1: the
  kits shown), and GetTickCount the tick's time (from 1000, and from just
  before its wrap). For each of the keys 1 to 4: pressed 100 and 250 ms
  after the kits appear, 350 ms after, pressed at 100 ms and held into OPEN,
  held from play into the kits, pressed in the frame they came, in play, and
  a respawn then a death: a kit is picked only by a press handed out 300 ms
  or more after the first poll that saw the kits, never by a key held or
  pressed before; every tick hands on_keyboard_action the same keys and kinds as the
  original exe's, which picks nothing (the kits are its movie's clicks), and
  comes back with its argument popped and ebx esi edi ebp kept;
- the variable: the game world's constructor's call of the hud's (0x1F6845)
  runs with the hud's constructor a stub that pops its argument and leaves
  made-up registers; the game's own cc_value<bool> constructor (0x1DDB0)
  runs. The call must reach the hud's constructor with the game world and the
  hud, come back with the stub's registers and its stack balanced, and put a
  cc_bool (0x9E7C4C) named "g_private_messages_in_game" (0xA064FC) on the
  option's byte, saved, at the head of the commands' list (0x1F1D66C), once
  over two calls; the game's own lookup (0xEB7C0, as options_item_base
  0x260E90 calls it) finds it, and the game's own execute (0xEB980, as a
  user.cfg line is run: filter 1, mask 2) sets and clears the byte;
- the chat: process_incoming_text_message's call of add_message (0x26B916)
  runs with add_message a stub that records it and `ret 14h`. A line must
  be dropped exactly with the option on, in a match (the chat handler's
  +0Dh) and on a channel 3 or 5 to 9; otherwise add_message gets it with the
  site's five arguments; both ways back at 0x26B91B with the five arguments
  popped and every register but eax as before. The original exe, run the
  same way, invokes nothing and gives add_message every line.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

KIND_TEST, BACKS = 0x25CD41, base.MATCH_KEYS_BACKS
KEYBOARD_ACTION = 0x25CD00
HUD_SITE, HUD_BACK, HUD_CONSTRUCTOR = 0x1F6845, 0x1F684A, 0x233810
CHAT_SITE, CHAT_BACK, ADD_MESSAGE = 0x26B916, 0x26B91B, 0x208AD0
INVOKE = 0x511600
LOOKUP, EXECUTE = 0xEB7C0, 0xEB980
COMMANDS = 0x1F1D66C                   # the console commands' list head
OPTION_NAME, CC_BOOL = 0xA064FC, 0x9E7C4C
POLL_SLOT, STOCK_SLOT = base.MATCH_KEYS_POLL_SLOT[0], 0x75AF0
INPUT_TICK, PROCESS = 0x3B7BD0, 0x3B9AF0  # input_world's tick, the keyboard's process
INPUT_VTABLE, KEYBOARD_VTABLE = 0xA252D4, 0xA25478
HANDLER_VTABLE = 0x9FC588
PLAIN_RET = 0x1AF330                   # a `ret`: the made-up keyboard's execute (the poll is the harness's)
GET_TICK_COUNT = 0x870090              # GetTickCount's import: the harness's clock
HANDLER, HOLDER, UI, MOVIE = 0x20000000, 0x20001000, 0x20002000, 0x20003000
WORLD, STACK, SENTINEL, TEXT = 0x20004000, 0x21000000, 0x22000000, 0x23000000
INPUT, KEYBOARD, KEYBOARD_VT, HANDLERS = 0x24000000, 0x24001000, 0x24003000, 0x24004000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
GAME = dict(zip(REGS, (0x0A0A0A0A, HANDLER, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
DOWN, UP, HELD = 1, 2, 3               # the keyboard's process (0x3B9AF0): an event down, up; a key down
CLOSED, OPENING, OPEN = 0, 1, 2        # kit_keys (match_keys_026g.asm)
DELAY = 300                            # KIT_KEYS_DELAY: ms the keys pick nothing after the kits appear
PHASES = {CLOSED: "CLOSED", OPENING: "OPENING", OPEN: "OPEN"}


class Run:
    def __init__(self, data, kit_keys=None, since=None):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HANDLER, STACK, SENTINEL, TEXT, INPUT):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.kit_keys = kit_keys           # the state byte's address (the patched build's)
        self.since = since                 # OPENING's time's address
        self.now = 0                       # what GetTickCount answers
        self.clock = SENTINEL + 0x100
        self.word(GET_TICK_COUNT, self.clock)
        emu.hook_add(UC_HOOK_CODE, self.tick_count, begin=self.clock, end=self.clock)
        self.calls = []
        self.actions = []
        self.ticking = False
        self.stub_regs = {}
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=INVOKE, end=INVOKE)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=HUD_CONSTRUCTOR, end=HUD_CONSTRUCTOR)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=ADD_MESSAGE, end=ADD_MESSAGE)
        emu.hook_add(UC_HOOK_CODE, self.action, begin=KEYBOARD_ACTION, end=KEYBOARD_ACTION)
        for back in BACKS:
            emu.hook_add(UC_HOOK_CODE, self.stop, begin=back, end=back)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0]

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address == INVOKE:              # thiscall (name, result, arguments, count), ret 10h; al 1
            value = self.read(args[2], 24) if args[2] else b""
            self.calls.append(("invoke", emu.reg_read(UC_X86_REG_ECX), self.text(args[0]), args[1], args[3],
                               value))
            pops, result = 16, 1
        elif address == HUD_CONSTRUCTOR:   # eax the game world, the hud; ret 4
            self.calls.append(("hud", emu.reg_read(UC_X86_REG_EAX), args[0]))
            pops, result = 4, 0x1234
            for reg, value in self.stub_regs.items():
                emu.reg_write(reg, value)
        else:                              # add_message, stdcall five, ret 14h
            self.calls.append(("add_message", *args))
            pops, result = 20, 0x5151
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def tick_count(self, emu, address, size, _):   # GetTickCount: stdcall, no arguments
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, self.now & 0xFFFFFFFF)
        emu.reg_write(UC_X86_REG_ECX, 0x0C1C1C1C)       # what a call may leave
        emu.reg_write(UC_X86_REG_EDX, 0x0D1D1D1D)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def run(self, start, until, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(start, until, count=500000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def phase(self, value=None, since=None):
        """kit_keys: set (a value, OPENING's time) or read; None in the original."""
        if self.kit_keys is None:
            return None
        if value is not None:
            self.emu.mem_write(self.kit_keys, bytes([value]))
        if since is not None:
            self.word(self.since, since)
        return self.read(self.kit_keys, 1)[0]

    def handler(self, cursor, movie=True):
        """The game world's input handler: its vtable, the hud's movie holder and cursor."""
        emu = self.emu
        emu.mem_write(HANDLER, bytes(0x4000))
        self.word(HANDLER, HANDLER_VTABLE)
        self.word(HANDLER + 0xC, HOLDER if movie else 0)
        self.word(HOLDER + 0x108, UI)
        self.word(UI + 4, MOVIE)
        emu.mem_write(HANDLER + 0x2E0, bytes([cursor]))

    # The poll: the handler's slot +14h as the input world's tick calls it.
    def poll(self, cursor, now=0):
        self.now = now
        self.emu.mem_write(HANDLER + 0x2E0, bytes([cursor]))
        slot = self.read(POLL_SLOT)
        sp = STACK + 0x8000 - 8
        self.word(sp, SENTINEL)
        self.word(sp + 4, INPUT)
        regs, esp = self.run(slot, SENTINEL, sp, {**GAME, UC_X86_REG_ECX: HANDLER})
        return slot, regs, esp - sp

    # A whole input tick: the game's input world tick and keyboard process over
    # a made-up input world, its keyboard's poll `events` (key, down) and the
    # keys down after them `held`, its one handler the game world's.
    def tick(self, cursor, events, held, now=1000):
        emu = self.emu
        self.now = now
        emu.mem_write(INPUT, bytes(0x10000))
        self.word(INPUT, INPUT_VTABLE)
        self.word(INPUT + 0xC, HANDLERS)          # the handlers: begin, end
        self.word(INPUT + 0x10, HANDLERS + 4)
        self.word(INPUT + 0x24, KEYBOARD)         # the keyboard (+20h and +28h: none)
        self.word(INPUT + 0x30, 2)                # activated
        emu.mem_write(INPUT + 0x34, b"\1")        # active
        self.word(HANDLERS, HANDLER)
        emu.mem_write(KEYBOARD_VT, bytes(self.emu.mem_read(KEYBOARD_VTABLE, 0x28)))
        self.word(KEYBOARD_VT + 0x20, PLAIN_RET)  # execute: the poll is below
        self.word(KEYBOARD, KEYBOARD_VT)
        self.word(KEYBOARD + 0x918, INPUT)
        self.word(KEYBOARD + 0x404, len(events))
        for i, (key, down) in enumerate(events):  # DIDEVICEOBJECTDATA: the key, its state
            self.word(KEYBOARD + 0x408 + 0x14 * i, key)
            self.word(KEYBOARD + 0x40C + 0x14 * i, 0x80 if down else 0)
        for key in held:
            self.word(KEYBOARD + 4 + 4 * key, 0x80)
        emu.mem_write(HANDLER + 0x2E0, bytes([cursor]))
        sp = STACK + 0x8000 - 8
        self.word(sp, SENTINEL)
        self.word(sp + 4, now)                    # the game's time
        self.calls, self.actions, self.ticking = [], [], True
        try:
            regs, esp = self.run(INPUT_TICK, SENTINEL, sp, {**GAME, UC_X86_REG_ECX: INPUT})
        finally:
            self.ticking = False
        return list(self.calls), list(self.actions), regs, esp - sp

    def action(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.actions.append((self.read(sp + 8), self.read(sp + 12)))   # the key, the kind

    # The keys: on_keyboard_action from its kind's test, edi the handler, ebp its frame.
    def key(self, key, kind, cursor, movie=True):
        emu = self.emu
        self.handler(cursor, movie)
        frame = STACK + 0x8000
        for i, value in enumerate((0x19FF00, SENTINEL, 0x30303030, key, kind)):
            self.word(frame + 4 * i, value)
        sp = frame - 0x20                  # the frame's locals and three pushes
        self.calls = []
        self.stopped = None
        before = bytes(emu.mem_read(STACK, 0x10000))
        regs, esp = self.run(KIND_TEST, 0, sp, {**GAME, UC_X86_REG_EDI: HANDLER, UC_X86_REG_EBP: frame})
        after = bytes(emu.mem_read(STACK, 0x10000))
        changed = [i for i in range(len(before)) if before[i] != after[i] and not sp - 0x200 <= STACK + i < sp]
        return self.stopped, regs, esp, list(self.calls), bytes(emu.mem_read(HANDLER, 0x4000)), changed

    def stop(self, emu, address, size, _):
        if self.ticking:                   # a key down not ours: the function's end, al 0
            if address == BACKS[0]:
                emu.reg_write(UC_X86_REG_EIP, BACKS[1])
            return
        self.stopped = address
        emu.emu_stop()

    # The variable: the constructor's call of the hud's, twice.
    def hud(self):
        self.stub_regs = {UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0E}
        out = []
        for _ in range(2):
            sp = STACK + 0x8000 - 8
            self.word(sp, HUD_BACK)
            self.word(sp + 4, WORLD + 0x1FC)
            self.calls = []
            regs, esp = self.run(HUD_SITE, HUD_BACK, sp + 4, {**GAME, UC_X86_REG_EAX: WORLD})
            out.append((regs, esp - (sp + 8), list(self.calls), self.read(COMMANDS)))
        return out

    def lookup(self, name):
        self.emu.mem_write(TEXT, name + b"\0")
        sp = STACK + 0x8000 - 4
        self.word(sp, SENTINEL)
        regs, _ = self.run(LOOKUP, SENTINEL, sp, {**GAME, UC_X86_REG_ESI: TEXT})
        return regs[UC_X86_REG_EAX]

    def execute(self, line):
        self.emu.mem_write(TEXT, line + b"\0")
        sp = STACK + 0x8000 - 16
        for i, value in enumerate((SENTINEL, TEXT, 1, 2)):
            self.word(sp + 4 * i, value)
        self.run(EXECUTE, SENTINEL, sp, GAME)

    # The chat: the site's five pushes, then its call.
    def chat(self, hide, in_match, channel, option_byte):
        emu = self.emu
        if option_byte is not None:
            emu.mem_write(option_byte, bytes([hide]))
        chat = HANDLER + 0x100
        emu.mem_write(chat, bytes(0x40))
        emu.mem_write(chat + 0xD, bytes([in_match]))
        sp = STACK + 0x8000 - 20
        arguments = (chat, channel, TEXT + 0x100, TEXT + 0x200, TEXT + 0x300)
        for i, value in enumerate(arguments):
            self.word(sp + 4 * i, value)
        self.calls = []
        regs, esp = self.run(CHAT_SITE, CHAT_BACK, sp, GAME)
        return regs, esp - (sp + 20), list(self.calls), arguments


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    stock = Run(original)
    patched = Run(bytes(built), linked.symbols["kit_keys"], linked.symbols["kit_keys_since"])
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    def pick(kit):
        value = struct.pack("<II", 0, 4) + struct.pack("<I", kit) + bytes(12)
        return [("invoke", MOVIE, b"root.select_kit_key", 0, 1, value)]

    print(f"the poll (the game world's input handler's slot +14h, 0x9FC59C; KIT_KEYS_DELAY {DELAY} ms)")
    wrap = 0xFFFFFF00
    # (state, OPENING's time, the kits shown, GetTickCount) -> (state, OPENING's time or None)
    polls = [
        ((CLOSED, 0, 0, 5000), (CLOSED, 0)),
        ((CLOSED, 0, 1, 5000), (OPENING, 5000)),
        ((OPENING, 5000, 1, 5000 + DELAY - 1), (OPENING, 5000)),
        ((OPENING, 5000, 1, 5000 + DELAY), (OPEN, None)),
        ((OPENING, wrap, 1, (wrap + DELAY - 1) & 0xFFFFFFFF), (OPENING, wrap)),
        ((OPENING, wrap, 1, (wrap + DELAY) & 0xFFFFFFFF), (OPEN, None)),
        ((OPENING, 5000, 0, 5100), (CLOSED, 0)),
        ((OPEN, 0, 1, 9999), (OPEN, None)),
        ((OPEN, 0, 0, 9999), (CLOSED, 0)),
    ]
    for (phase, since, cursor, now), (want, want_since) in polls:
        patched.handler(cursor)
        patched.phase(phase, since)
        slot, regs, popped = patched.poll(cursor, now)
        since_now = patched.read(patched.since)
        ok = slot == linked.symbols["match_keys_poll"] and patched.phase() == want and popped == 8 \
            and (want_since is None or since_now == want_since) and all(regs[reg] == GAME[reg] for reg in KEPT)
        report(ok, f"{PHASES[phase]}" + (f"({since:#x})" if phase == OPENING else "")
                   + f", the kits {'shown' if cursor else 'not shown'} at {now:#x}: {PHASES[patched.phase()]}"
                   + (f"({since_now:#x})" if want == OPENING else "")
                   + ", the input world popped, ebx esi edi ebp kept")
    stock.handler(1)
    slot, regs, popped = stock.poll(1)
    report(slot == STOCK_SLOT and popped == 8 and all(regs[reg] == GAME[reg] for reg in KEPT),
           f"the original's slot is {STOCK_SLOT:#x}, `ret 4`")

    print("the keys (on_keyboard_action from 0x25CD41 to 0x25CD4B or 0x25CF0D)")
    cases = [(key, kind, cursor, movie, phase) for key in (1, 2, 3, 4, 5, 0x1C) for kind in (DOWN, UP, HELD)
             for cursor in (0, 1) for movie in (True, False) for phase in (CLOSED, OPENING, OPEN)]
    for key, kind, cursor, movie, phase in cases:
        patched.phase(phase)
        a, b = stock.key(key, kind, cursor, movie), patched.key(key, kind, cursor, movie)
        want = kind == DOWN and 2 <= key <= 4 and cursor and movie and phase == OPEN
        invoked = b[3]
        ok = a[0] == b[0] == BACKS[kind != DOWN] and a[1:3] == b[1:3] and not a[3] and a[4] == b[4] \
            and not a[5] and not b[5] and patched.phase() == phase
        ok = ok and invoked == (pick(key - 2) if want else [])
        report(ok, f"key {key:#x} kind {kind} cursor {cursor} movie {'yes' if movie else 'no'} {PHASES[phase]}: "
                   + (f"root.select_kit_key({key - 2})" if invoked else "nothing invoked")
                   + f", at {b[0]:#x} with the registers, frame, stack and objects the original's")

    print("the input ticks (input_world's tick 0x3B7BD0, the keyboard's process 0x3B9AF0, on_keyboard_action; "
          "ms from the first poll that sees the kits)")
    for key in (2, 3, 4, 5):           # the keys 1, 2, 3 and 4 above the letters
        kit = key - 2 if key <= 4 else None
        d, u = (key, True), (key, False)
        # (what, [(ms, the kits shown at the tick, the poll's events)], the tick that picks or None)
        scenarios = [
            ("pressed 100 ms after the kits appear, let go",
             [(-16, 0, []), (0, 1, []), (100, 1, [d]), (116, 1, [u]), (400, 1, [])], None),
            ("pressed 250 ms after the kits appear, let go",
             [(-16, 0, []), (0, 1, []), (250, 1, [d]), (266, 1, [u]), (400, 1, [])], None),
            ("pressed 350 ms after the kits appear",
             [(-16, 0, []), (0, 1, []), (350, 1, [d])], 2),
            ("pressed 100 ms after the kits appear, held into OPEN, then let go and pressed",
             [(-16, 0, []), (0, 1, []), (100, 1, [d]), (200, 1, []), (316, 1, []), (400, 1, []), (450, 1, [u]),
              (500, 1, [d])], 7),
            ("held from play into the kits, then let go and pressed",
             [(-32, 0, []), (-16, 0, [d]), (0, 1, []), (350, 1, []), (400, 1, [u]), (450, 1, [d])], 5),
            ("pressed in the frame the kits came, let go, pressed at 350 ms",
             [(-16, 0, []), (0, 1, [d]), (16, 1, [u]), (350, 1, [d])], 3),
            ("in play", [(-16, 0, []), (0, 0, [d]), (100, 0, []), (400, 0, [u]), (800, 0, [d, u])], None),
            ("on the kits, a respawn, a death, a press 100 ms after it, then one 384 ms after",
             [(-16, 0, []), (0, 1, []), (350, 1, []), (400, 0, []), (416, 1, []), (516, 1, [d]), (532, 1, [u]),
              (800, 1, [d])], 7),
        ]
        for start in (1000, 0xFFFFFF80):  # GetTickCount, and over its wrap
            for what, ticks, picks in scenarios:
                held, seen, ok = set(), [], True
                stock.handler(0)
                patched.handler(0)
                for at, (ms, cursor, events) in enumerate(ticks):
                    now = (start + ms) & 0xFFFFFFFF
                    for k, down in events:
                        (held.add if down else held.discard)(k)
                    a = stock.tick(cursor, events, held, now)
                    b = patched.tick(cursor, events, held, now)
                    kinds = [(k, DOWN if down else UP) for k, down in events] + [(k, HELD) for k in sorted(held)]
                    want = pick(kit) if at == picks and kit is not None else []
                    ok = ok and a[1] == b[1] == kinds and a[0] == [] and b[0] == want \
                        and a[3] == b[3] == 8 and all(a[2][reg] == b[2][reg] == GAME[reg] for reg in KEPT)
                    seen.append(f"{ms} {'kits' if cursor else 'play'} {PHASES[patched.phase()]}"
                                + (" down" if d in events else "") + (" up" if u in events else "")
                                + (f" picks kit {kit + 1}" if b[0] else ""))
                report(ok, f"key {key - 1}, {what} (GetTickCount from {start:#x}): " + ", ".join(seen))


    print("the variable (the hud's constructor call, 0x1F6845)")
    option = linked.symbols["hide_chat"]
    variable = linked.symbols["chat_variable"]
    # A command the game registered before: the list's head, "minimap_position".
    for run in (stock, patched):
        run.emu.mem_write(TEXT + 0x800, b"minimap_position\0")
        run.emu.mem_write(HANDLER + 0x5000, bytes(0x48))
        run.word(HANDLER + 0x5000 + 0x10, TEXT + 0x800)
        run.word(COMMANDS, HANDLER + 0x5000)
    head = patched.read(COMMANDS)
    runs = patched.hud()
    first, second = runs
    stub_regs = {**GAME, UC_X86_REG_EAX: 0x1234, **patched.stub_regs}
    report(all(calls == [("hud", WORLD, WORLD + 0x1FC)] and popped == 0 and regs == stub_regs
               for regs, popped, calls, _ in runs),
           "both calls reach the hud's constructor with the game world (eax) and the hud, and come back with "
           "its registers and its argument popped")
    fields = patched.read(variable, 0x48)
    word = lambda at: struct.unpack_from("<I", fields, at)[0]
    report(first[3] == second[3] == variable and word(0) == CC_BOOL and word(0xC) == head
           and word(0x10) == OPTION_NAME and word(0x40) == option and word(0x14) == 2 and fields[0x1D] == 1
           and fields[0x1C] == 1 and patched.text(OPTION_NAME) == b"g_private_messages_in_game",
           f"one cc_bool at {variable:#x} heads the commands' list (the old head {head:#x} after it), named "
           f"g_private_messages_in_game, on the byte {option:#x}, saved (+1Dh 1), kind 2")
    report(patched.lookup(b"g_private_messages_in_game") == variable and stock.lookup(b"g_private_messages_in_game") == 0
           and patched.lookup(b"minimap_position") == stock.lookup(b"minimap_position") == head,
           "the game's lookup (0xEB7C0, options_item_base's) finds it, the original's finds nothing, "
           "the command before it found as before")
    seen = []
    for line, want in ((b"g_private_messages_in_game on", 1), (b"g_private_messages_in_game off", 0),
                       (b"g_private_messages_in_game 1", 1), (b"g_private_messages_in_game 0", 0)):
        patched.execute(line)
        seen.append(patched.read(option, 1)[0] == want)
    report(all(seen), "the game's execute (0xEB980, a user.cfg line: filter 1, mask 2) sets the byte for "
                      "`on` and `1` and clears it for `off` and `0`")

    print("the chat (add_message's call, 0x26B916)")
    for hide in (0, 1):
        for in_match in (0, 1):
            for channel in range(10):
                a = stock.chat(hide, in_match, channel, None)
                b = patched.chat(hide, in_match, channel, option)
                drop = hide and in_match and (channel == 3 or 5 <= channel <= 9)
                given = [("add_message", *b[3])]
                keep = {reg: value for reg, value in GAME.items() if reg != UC_X86_REG_EAX}
                ok = a[2] == given and a[1] == 0 and b[1] == 0 \
                    and all(b[0][reg] == value for reg, value in keep.items()) \
                    and (b[2] == [] if drop else b[2] == given)
                report(ok, f"option {hide}, in a match {in_match}, channel {channel}: "
                           + ("dropped" if not b[2] else "given to add_message")
                           + ", back at 0x26b91b with the five arguments popped, ebx esi edi ebp kept")
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
