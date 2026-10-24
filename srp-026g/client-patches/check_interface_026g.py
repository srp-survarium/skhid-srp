#!/usr/bin/env python3
"""Run patch 64 (interface_026g.asm and .c: the interface settings, the
first the interface's level) in the default build on made-up game objects.

    python check_interface_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the patch's four entries run from the game's own sites, with
the game's own console functions. Scaleform's Invoke (0x511600), the hud's
update_ui (0x23B200), the allocators, buffer_string's printf (0xE4590), the
CRT's sscanf_s (0x422B46) and the console's error line (0xEC530) are stubs:

- the variable: the hud's constructor's registration of show_quests_progress
  (0x2339DE) runs with its four arguments pushed and eax its object; the
  game's cc_value<bool> (0x1DDB0) and cc_value<unsigned int> (0x1DCD0) run.
  show_quests_progress is made as the original makes it; then a cc_u32
  (0x9E7CCC) named "g_interface_level" heads the commands' list (0x1F1D66C),
  on the setting's value, 0 to 2, saved (+1Dh 1), kind 2; back at 0x2339E3
  with the four arguments popped and the registers the original's. The
  game's lookup (0xEB7C0, options_item_base's) finds it (the original's
  nothing); the game's execute (0xEB980, a user.cfg line: filter 1, mask 2)
  sets it to 0, 1 and 2, and a value out of range gives 0 (Full);
- the options' row: options_tab::options_tab from the gameplay tab's option
  15 stored (0x262731, ebx a made-up tab of 17 slots, edi option 15) to
  0x262736: option 15 in its slot as the original's; then slot 16 an
  options_item_int (0x261100: vtable 0xA0670C) of the variable, its kind 1,
  its tab, its index 16, the three keys st_interface_full, st_interface_minimal,
  st_interface_disabled (the original leaves slot 16 empty); the game's
  options_item_int initialize (0x261130) reads the value, and its apply
  (0x2612A0) of the choice 1 sets the variable to 1 through the console;
- the labels: fill_settings_data's call making the gameplay labels (0x2005F2,
  the allocator and the count 11h pushed) runs the game's 0x202150; label 16
  is "st_interface_level_option", the 16 before it empty as the original's;
- the match: the game world's call of update_ui (0x1F8023, eax a made-up hud
  with the hud movie [[+8]+108h]+4 and the player icons' [[+10h]+108h]+4,
  three arguments pushed) calls update_ui once with eax the hud and the three
  arguments, comes back at 0x1F8028 with them popped, eax ecx edx update_ui's
  and ebx esi edi ebp kept; the original invokes nothing. The first frame
  gives both movies root.set_interface_level(0) (an unsigned int); a frame
  with nothing changed invokes nothing; the setting set to 2 (Disabled) or 1
  (Minimal) reaches both in the next frame, once; a mode's least
  (interface_force(0, 2)) gives them 2 with the setting 0, and 0 again when
  lifted; a new player icons' movie gets the value, the hud's nothing new;
  without the movies nothing is invoked.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

REGISTER_SITE, REGISTER_BACK = 0x2339DE, 0x2339E3
OPTIONS_SITE, OPTIONS_BACK = 0x262731, 0x262736
LABELS_SITE, LABELS_BACK = 0x2005F2, 0x2005F7
UI_SITE, UI_BACK, UPDATE_UI = 0x1F8023, 0x1F8028, 0x23B200
INVOKE, MALLOC, PRINTF, SCANF, ERROR = 0x511600, 0xE4F90, 0xE4590, 0x422B46, 0xEC530
LOOKUP, EXECUTE = 0xEB7C0, 0xEB980
INT_INITIALIZE, INT_APPLY = 0x261130, 0x2612A0
COMMANDS = 0x1F1D66C                   # the console commands' list head
QUESTS_OBJECT, QUESTS_NAME = 0xA2AE160, 0xA03170   # show_quests_progress's command and name
ITEMS_ALLOCATOR = 0xAF30E0             # options_item_base's helper allocator (its vtable +28h)
CC_U32, INT_ITEM = 0x9E7CCC, 0xA0670C
HUD, HOLDERS, MOVIES, TAB, ITEMS = 0x20000000, 0x20001000, 0x20002000, 0x20003000, 0x20004000
ALLOC, HEAP, STACK, SENTINEL, TEXT = 0x20005000, 0x24000000, 0x21000000, 0x22000000, 0x23000000
HUD_MOVIE, ICONS_MOVIE, ICONS_MOVIE_2 = MOVIES, MOVIES + 0x100, MOVIES + 0x200
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
GAME = dict(zip(REGS, (0x0A0A0A0A, 0x0C0C0C0C, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
UI_OUT = {UC_X86_REG_EAX: 0x1A1A1A1A, UC_X86_REG_ECX: 0x1C1C1C1C, UC_X86_REG_EDX: 0x1D1D1D1D}


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HUD, HOLDERS, MOVIES, TAB, ITEMS, ALLOC, STACK, SENTINEL, TEXT):
            emu.mem_map(at, 0x1000 if at < STACK else 0x10000, UC_PROT_ALL)
        emu.mem_map(HEAP, 0x100000, UC_PROT_ALL)
        self.heap = HEAP
        self.calls = []
        # options_item_base's helper allocator: an object whose vtable +28h allocates
        self.word(ALLOC, ALLOC + 0x100)
        self.word(ALLOC + 0x100 + 0x28, ALLOC + 0x200)
        self.word(ITEMS_ALLOCATOR, ALLOC)
        for stub in (INVOKE, UPDATE_UI, MALLOC, PRINTF, SCANF, ERROR, ALLOC + 0x200):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0]

    def alloc(self, size):
        at = self.heap
        self.heap += (size + 15) & ~15
        return at

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        out = {}
        if address == INVOKE:              # thiscall (name, result, arguments, count), ret 10h; al 1
            values = tuple(struct.unpack("<II", self.read(args[2] + 24 * i + 4, 8)) for i in range(args[3]))
            self.calls.append(("invoke", emu.reg_read(UC_X86_REG_ECX), self.text(args[0]).decode(), values))
            pops, out[UC_X86_REG_EAX] = 16, 1
        elif address == UPDATE_UI:         # eax the hud, three arguments; ret 0Ch
            self.calls.append(("update_ui", emu.reg_read(UC_X86_REG_EAX), *args[:3]))
            pops, out = 12, dict(UI_OUT)
        elif address == MALLOC:            # esi the allocator, eax the size, the name pushed; ret 4
            pops, out[UC_X86_REG_EAX] = 4, self.alloc(emu.reg_read(UC_X86_REG_EAX))
        elif address == ALLOC + 0x200:     # the helper allocator's +28h (size, flags); ret 8
            pops, out[UC_X86_REG_EAX] = 8, self.alloc(args[0])
        elif address == PRINTF:            # buffer_string's printf (eax the string; cdecl format, value)
            if self.text(args[0]) != b"%d":
                raise ValueError(f"printf with {self.text(args[0])!r}")
            string, text = emu.reg_read(UC_X86_REG_EAX), str(args[1]).encode()
            begin = self.read(string)
            emu.mem_write(begin, text + b"\0")
            self.word(string + 4, begin + len(text))
            pops = 0
        elif address == SCANF:             # sscanf_s(text, "%d", &value): one number
            if self.text(args[1]) != b"%d":
                raise ValueError(f"sscanf with {self.text(args[1])!r}")
            words = self.text(args[0]).split()
            try:
                self.word(args[2], int(words[0]))
                out[UC_X86_REG_EAX] = 1
            except (IndexError, ValueError):
                out[UC_X86_REG_EAX] = 0
            pops = 0
        else:                              # the console's error line (eax the command, the text); ret 4
            pops = 4
        for reg, value in out.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def run(self, start, until, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.calls = []
        self.emu.emu_start(start, until, count=2000000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP) - sp

    def call(self, start, args, regs=None):
        sp = STACK + 0x8000 - 4 * (len(args) + 1)
        self.word(sp, SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        return self.run(start, SENTINEL, sp, {**GAME, **(regs or {})})

    # The variable: the site's four pushes done, eax the object.
    def register(self):
        sp = STACK + 0x8000 - 16
        for i, value in enumerate((QUESTS_NAME, HUD + 0x35C, 1, 2)):
            self.word(sp + 4 * i, value)
        return self.run(REGISTER_SITE, REGISTER_BACK, sp, {**GAME, UC_X86_REG_EAX: QUESTS_OBJECT})

    def lookup(self, name):
        self.emu.mem_write(TEXT, name + b"\0")
        regs, _ = self.call(LOOKUP, [], {UC_X86_REG_ESI: TEXT})
        return regs[UC_X86_REG_EAX]

    def execute(self, line):
        self.emu.mem_write(TEXT, line + b"\0")
        self.call(EXECUTE, [TEXT, 1, 2])

    # The options' row: options_tab::options_tab at option 15's store, ebx the tab, edi option 15.
    def options(self):
        self.emu.mem_write(ITEMS, bytes(0x100))
        self.emu.mem_write(TAB, bytes(0x100))
        self.word(TAB, ITEMS)
        self.emu.mem_write(TAB + 4, bytes([0x11]))
        sp = STACK + 0x8000
        return self.run(OPTIONS_SITE, OPTIONS_BACK, sp, {**GAME, UC_X86_REG_EBX: TAB, UC_X86_REG_EDI: 0x15151515})

    # The labels: the site's two pushes done (the allocator, the count).
    def labels(self, count):
        sp = STACK + 0x8000 - 8
        self.word(sp, ALLOC)
        self.word(sp + 4, count)
        regs, popped = self.run(LABELS_SITE, LABELS_BACK, sp, GAME)
        return regs[UC_X86_REG_EAX], popped

    # The match: the site's three pushes done, eax the hud.
    def frame(self, movie=True, icons=ICONS_MOVIE):
        self.word(HUD + 8, HOLDERS if movie else 0)
        self.word(HOLDERS + 0x108, HOLDERS + 0x200)
        self.word(HOLDERS + 0x200 + 4, HUD_MOVIE)
        self.word(HUD + 0x10, HOLDERS + 0x400 if icons else 0)
        self.word(HOLDERS + 0x400 + 0x108, HOLDERS + 0x600)
        self.word(HOLDERS + 0x600 + 4, icons or 0)
        sp = STACK + 0x8000 - 12
        for i, value in enumerate((0x31313131, 0x32323232, 0x33)):
            self.word(sp + 4 * i, value)
        return self.run(UI_SITE, UI_BACK, sp, {**GAME, UC_X86_REG_EAX: HUD})


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    setting = linked.symbols["interface_settings"]
    value_at, least_at = setting + 0x4C, setting + 0x50
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the variable (the hud's constructor's registration of show_quests_progress, 0x2339DE)")
    for run in (stock, patched):
        run.emu.mem_write(TEXT + 0x800, b"some_command\0")
        run.emu.mem_write(TEXT + 0x900, bytes(0x48))
        run.word(TEXT + 0x900 + 0x10, TEXT + 0x800)
        run.word(COMMANDS, TEXT + 0x900)
    a, b = stock.register(), patched.register()
    quests_a, quests_b = stock.read(QUESTS_OBJECT, 0x48), patched.read(QUESTS_OBJECT, 0x48)
    fields = patched.read(setting, 0x4C)
    word = lambda at: struct.unpack_from("<I", fields, at)[0]
    report(a == b and a[1] == 16 and quests_a[:8] == quests_b[:8] and quests_a[12:] == quests_b[12:]
           and stock.read(COMMANDS) == QUESTS_OBJECT and quests_a[8:12] == bytes(4)
           and struct.unpack_from("<I", quests_b, 8)[0] == setting,
           "show_quests_progress made as the original makes it (but its +8, the list's previous: the setting's "
           "variable); back at 0x2339e3 with the four arguments popped and every register the original's")
    report(patched.read(COMMANDS) == setting and word(0) == CC_U32 and word(0xC) == QUESTS_OBJECT
           and patched.text(word(0x10)) == b"g_interface_level" and word(0x14) == 2 and fields[0x1D] == 1
           and word(0x40) == value_at and word(0x44) == 0 and word(0x48) == 2,
           f"then a cc_u32 at {setting:#x} heads the commands' list, named g_interface_level, on its value "
           f"{value_at:#x}, 0 to 2, saved (+1Dh 1), kind 2")
    report(patched.lookup(b"g_interface_level") == setting and stock.lookup(b"g_interface_level") == 0
           and patched.lookup(b"some_command") == stock.lookup(b"some_command") == TEXT + 0x900,
           "the game's lookup (0xEB7C0) finds it, the original's finds nothing, the command before it found as before")
    seen = []
    for line, want in ((b"g_interface_level 2", 2), (b"g_interface_level 1", 1), (b"g_interface_level 0", 0),
                       (b"g_interface_level 2", 2), (b"g_interface_level 7", 0)):
        patched.execute(line)
        seen.append(patched.read(value_at) == want)
    report(all(seen), "the game's execute (0xEB980, a user.cfg line) sets it to 2, 1, 0 and 2; a value out of "
                      "range gives 0 (Full)")

    print("the options' row (options_tab::options_tab, the gameplay tab, 0x262731 to 0x262736)")
    a, b = stock.options(), patched.options()
    items_a, items_b = stock.read(ITEMS, 0x44), patched.read(ITEMS, 0x44)
    item = struct.unpack_from("<I", items_b, 0x40)[0]
    item_fields = patched.read(item, 0x20) if item else bytes(0x20)
    iw = lambda at: struct.unpack_from("<I", item_fields, at)[0]
    keys = [patched.text(patched.read(iw(0x18) + 4 * i)) for i in range(3)] if item else []
    keep = {reg: GAME[reg] for reg in (UC_X86_REG_EBP, UC_X86_REG_ESI)}
    report(a[1] == b[1] == 0 and items_a[:0x3C] == items_b[:0x3C] == bytes(0x3C)
           and struct.unpack_from("<I", items_a, 0x3C)[0] == struct.unpack_from("<I", items_b, 0x3C)[0] == 0x15151515
           and items_a[0x40:] == bytes(4)
           and all(b[0][reg] == v for reg, v in keep.items()) and b[0][UC_X86_REG_EBX] == TAB
           and b[0][UC_X86_REG_EDI] == 0x15151515,
           "option 15 stored in its slot as the original stores it; the original leaves slot 16 empty; back at "
           "0x262736 with the stack, ebx (the tab), edi, esi and ebp as they were")
    report(item and iw(0) == INT_ITEM and iw(8) == setting and iw(0xC) == 1 and iw(0x10) == TAB
           and item_fields[0x14] == 16 and item_fields[0x1C] == 3
           and keys == [b"st_interface_full", b"st_interface_minimal", b"st_interface_disabled"],
           f"slot 16 an options_item_int of g_interface_level, index 16, the tab, the keys {keys}")
    patched.word(value_at, 2)
    patched.call(INT_INITIALIZE, [], {UC_X86_REG_ECX: item})
    initial = patched.read(item + 0x1D, 2)
    patched.emu.mem_write(item + 0x1D, bytes([1]))
    patched.call(INT_APPLY, [], {UC_X86_REG_ECX: item})
    report(initial == bytes([2, 2]) and patched.read(value_at) == 1,
           "the game's initialize (0x261130) reads the value 2; its apply (0x2612A0) of the choice 1 sets "
           "g_interface_level to 1 through the console")

    print("the labels (fill_settings_data's gameplay labels, 0x2005F2)")
    labels_a, popped_a = stock.labels(0x10)
    labels_b, popped_b = patched.labels(0x11)
    label = lambda run, at, i: run.text(run.read(at + 0x4C * i))
    report(popped_a == popped_b == 0 and stock.read(labels_a - 8) == 0x10 and patched.read(labels_b - 8) == 0x11
           and all(label(stock, labels_a, i) == label(patched, labels_b, i) == b"" for i in range(16))
           and label(patched, labels_b, 16) == b"st_interface_level_option",
           "17 labels (the count pushed), the game's 16 empty as the original's, label 16 "
           f"{label(patched, labels_b, 16)!r}; nothing popped")

    print("the match (the game world's call of update_ui, 0x1F8023)")
    level = lambda movie, value: ("invoke", movie, "root.set_interface_level", ((4, value),))
    both = lambda value: [level(HUD_MOVIE, value), level(ICONS_MOVIE, value)]
    patched.word(value_at, 0)
    a = stock.frame()
    report(a[1] == 12 and stock.calls == [("update_ui", HUD, 0x31313131, 0x32323232, 0x33)],
           "the original calls update_ui with the hud and the three arguments and invokes nothing")
    steps = []

    def step(text, want, **frame):
        regs, popped = patched.frame(**frame)
        calls = patched.calls
        ok = calls[:1] == [("update_ui", HUD, 0x31313131, 0x32323232, 0x33)] and calls[1:] == want \
            and popped == 12 and all(regs[r] == v for r, v in UI_OUT.items()) \
            and all(regs[r] == GAME[r] for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))
        report(ok, f"{text}: update_ui, then {[c[1:] for c in calls[1:]] or 'nothing'}; back at 0x1f8028 with "
                   "the arguments popped, eax ecx edx update_ui's, ebx esi edi ebp kept")

    step("the first frame (Full)", both(0))
    step("nothing changed", [])
    patched.word(value_at, 2)
    step("set to Disabled", both(2))
    step("nothing changed", [])
    patched.word(value_at, 1)
    step("set to Minimal", both(1))
    patched.word(value_at, 0)
    patched.call(linked.symbols["interface_force"], [0, 2])
    step("Full with a mode's least Disabled (interface_force(0, 2))", both(2))
    patched.call(linked.symbols["interface_force"], [0, 0])
    step("the least lifted", both(0))
    step("a new player icons' movie", [level(ICONS_MOVIE_2, 0)], icons=ICONS_MOVIE_2)
    step("no movies", [], movie=False, icons=None)
    step("the movies back", both(0))

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
