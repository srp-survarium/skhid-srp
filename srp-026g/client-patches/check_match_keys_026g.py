#!/usr/bin/env python3
"""Run patch 58 (the Rust patch match_keys: the kit keys and the match chat
option) in the default build on made-up game objects.

    python check_match_keys_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the patch's three entries run from the game's own sites:

- the keys: the game world's on_keyboard_action (0x25CD00) is run from its
  kind's test (0x25CD41) with a made-up input handler in edi (the hud at +4:
  its cursor flag +2DCh, its movie's holder +8 -> +108h -> +4) and the
  function's frame in ebp (the key, the kind), until it reaches 0x25CD4B (a
  key down) or 0x25CF0D (any other). Scaleform's Invoke (0x511600) is a stub
  that records its movie, name, result and argument. A case must invoke
  `root.select_kit_key` with the kit (an unsigned int GFx value) exactly for
  the keys 1, 2 and 3 going down while the cursor is shown and the movie is
  there, and arrive where the original exe does with every register, the
  frame and the stack as the original's;
- the variable: the game world's constructor's call of the hud's (0x1F6845)
  runs, to the instruction after it (0x1F684A, the patch's lohook, which
  replays it), with the hud's constructor a stub that pops its argument and
  leaves made-up registers; the game's own cc_value<bool> constructor (0x1DDB0)
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

KIND_TEST, BACKS = 0x25CD41, (0x25CD4B, 0x25CF0D)   # on_keyboard_action: a key down, any other
HUD_SITE, HUD_BACK, HUD_CONSTRUCTOR = 0x1F6845, 0x1F684A, 0x233810
HUD_PAST = 0x1F6850                    # past `mov [ebp+1D88h],ebx`, the lohook's span at HUD_BACK
CHAT_SITE, CHAT_BACK, ADD_MESSAGE = 0x26B916, 0x26B91B, 0x208AD0
INVOKE = 0x511600
LOOKUP, EXECUTE = 0xEB7C0, 0xEB980
COMMANDS = 0x1F1D66C                   # the console commands' list head
OPTION_NAME, CC_BOOL = 0xA064FC, 0x9E7C4C
HANDLER, HOLDER, UI, MOVIE = 0x20000000, 0x20001000, 0x20002000, 0x20003000
WORLD, STACK, SENTINEL, TEXT = 0x20004000, 0x21000000, 0x22000000, 0x23000000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
GAME = dict(zip(REGS, (0x0A0A0A0A, HANDLER, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
DOWN, HELD, UP = 1, 2, 3


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HANDLER, STACK, SENTINEL, TEXT):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.calls = []
        self.stub_regs = {}
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=INVOKE, end=INVOKE)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=HUD_CONSTRUCTOR, end=HUD_CONSTRUCTOR)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=ADD_MESSAGE, end=ADD_MESSAGE)
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

    def run(self, start, until, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(start, until, count=500000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    # The keys: on_keyboard_action from its kind's test, edi the handler, ebp its frame.
    def key(self, key, kind, cursor, movie=True):
        emu = self.emu
        emu.mem_write(HANDLER, bytes(0x4000))
        self.word(HANDLER + 0xC, HOLDER if movie else 0)
        self.word(HOLDER + 0x108, UI)
        self.word(UI + 4, MOVIE)
        emu.mem_write(HANDLER + 0x2E0, bytes([cursor]))
        frame = STACK + 0x8000
        for i, value in enumerate((0x19FF00, SENTINEL, 0x30303030, key, kind)):
            self.word(frame + 4 * i, value)
        sp = frame - 0x20                  # the frame's locals and three pushes
        self.calls = []
        self.stopped = None
        before = bytes(emu.mem_read(STACK, 0x10000))
        regs, esp = self.run(KIND_TEST, 0, sp, {**GAME, UC_X86_REG_EDI: HANDLER, UC_X86_REG_EBP: frame})
        after = bytes(emu.mem_read(STACK, 0x10000))
        changed = [i for i in range(len(before)) if before[i] != after[i] and not sp - 0x1000 <= STACK + i < sp]   # below esp: scratch (the Rust entry saves the machine there)
        return self.stopped, regs, esp, list(self.calls), bytes(emu.mem_read(HANDLER, 0x4000)), changed

    def stop(self, emu, address, size, _):
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
            regs, esp = self.run(HUD_SITE, HUD_PAST, sp + 4, {**GAME, UC_X86_REG_EAX: WORLD})
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
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the keys (on_keyboard_action from 0x25CD41 to 0x25CD4B or 0x25CF0D)")
    cases = [(key, kind, cursor, movie) for key in (1, 2, 3, 4, 5, 0x1C) for kind in (DOWN, HELD, UP)
             for cursor in (0, 1) for movie in (True, False)]
    for key, kind, cursor, movie in cases:
        a, b = stock.key(key, kind, cursor, movie), patched.key(key, kind, cursor, movie)
        want = kind == DOWN and 2 <= key <= 4 and cursor and movie
        invoked = b[3]
        ok = a[0] == b[0] == BACKS[kind != DOWN] and a[1:3] == b[1:3] and not a[3] and a[4] == b[4] \
            and not a[5] and not b[5]
        if want:
            value = struct.pack("<II", 0, 4) + struct.pack("<I", key - 2) + bytes(12)
            ok = ok and invoked == [("invoke", MOVIE, b"root.select_kit_key", 0, 1, value)]
        else:
            ok = ok and not invoked
        report(ok, f"key {key:#x} kind {kind} cursor {cursor} movie {'yes' if movie else 'no'}: "
                   + (f"root.select_kit_key({key - 2})" if invoked else "nothing invoked")
                   + f", at {b[0]:#x} with the registers, frame, stack and objects the original's")

    print("the variable (the hud's constructor call, 0x1F6845)")
    # the patch's static HIDE_CHAT (vostok::console::ConsoleBool): the
    # command (48h bytes), then its byte
    variable = next(va for name, va, _, _ in linked.every if name.endswith("10match_keys9HIDE_CHAT"))
    option = variable + 0x48
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
                # add_message is a stdcall function: ECX and EDX are its to clobber (the RUST_CALL
                # entry leaves them so too); EBX, ESI, EDI, EBP are the caller's
                keep = {reg: value for reg, value in GAME.items() if reg not in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX)}
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
