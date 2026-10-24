#!/usr/bin/env python3
"""Run patch 62 (hide_ui_026g.asm: one key hides the interface and shows it
again) in the default build on made-up game objects.

    python check_hide_ui_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the patch's three entries run from the game's own sites, with
the game's own get_binded_action (0x207D50). Scaleform's Invoke (0x511600)
is a stub that records each call (the movie, the method, its arguments) and
answers root.interface_hidden with a bool:

- the match: on_keyboard_action's case of action 19h (0x25CED1) runs with a
  made-up input handler at [esp+10h] (+18h the replay's movie; the hud at +4:
  its movie [[+8]+108h]+4, the player icons' [[+10h]+108h]+4) until
  0x25CEDB (the replay's toggle) or 0x25CF0D (the key not taken). With the
  replay's movie both builds reach 0x25CEDB with esi the handler and invoke
  nothing. Without it the original reaches 0x25CF0D and invokes nothing; the
  patch asks the hud movie root.interface_hidden and gives the hud movie and
  the icons' root.hide_interface(not that), whatever the cursor (the kits
  before the start too), and nothing without the hud movie; esp, ebp and the
  stack as the original's; the kits' show_cursor call (0x23845B) is the
  original's (a death keeps the interface hidden: no case of its own);
- the lobby: lobby_menu::on_keyboard_action's binder call (0x26CCD2) runs
  with a made-up key binder (the game's action table's descriptions, a few
  keys bound), edi 2, esi the key and ebx a made-up lobby menu (its movie
  [[+690h]+108h]+4), until 0x26CCD7 (back from the call), 0x26CCAE (the key
  taken) or 0x26CDD2 (the key to the movie, no action). Scaleform's Invoke
  (0x511600) is a stub that records it and answers root.interface_hidden
  with a bool. F11 (bound to toggle_replay_ui) invokes root.toggle_interface
  on the movie (no result, no argument) and is taken; without the movie it is
  not. While the movie answers that its interface is hidden, every other key
  (a window's, F6's Play, Enter) goes to 0x26CDD2; else it comes back at
  0x26CCD7 with the original's answer, toggle place and registers;
- the options' controls: game_options' own functions on a made-up options
  object (its movie, the game, the binder above), GFx objects, the strings'
  look-up, buffer_string's printf and the CRT's sscanf_s stubbed: the rows
  sent to the movie (0x2019E0) are the original's 40 and a 41st (19h,
  st_hide_interface_action, group 2, type 1); the keys read from the binder
  (0x201CF0) the original's 40 and (19h, kF11); a key kF9 set for 19h
  (0x2013A0) is kept in the 41st row and in no row of the original; the
  accept (0x201F20) runs `bind toggle_replay_ui kF9` through the console (the
  original nothing); the binder's own bind (0x207DC0) binds 19h to kF9, its
  save (0x207450) writes `bind toggle_replay_ui kF9`, and that line read
  back binds it again.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

CASE, REPLAY_BACK, KEY_OTHER = 0x25CED1, 0x25CEDB, 0x25CF0D
KITS_SITE, KITS_BACK, SHOW_CURSOR = 0x23845B, 0x238460, 0x23A930
LOBBY_SITE, LOBBY_BACK, LOBBY_TAKEN, LOBBY_TO_MOVIE = 0x26CCD2, 0x26CCD7, 0x26CCAE, 0x26CDD2
SHOW_MOVIE, HIDE_MOVIE, INVOKE = 0x23DC00, 0x23DC70, 0x511600
ACTIONS = 0xA74AF0                     # the action table: 20-byte {name, id, kinds, toggle, key name}
HANDLER, WORLD, HUD, LOBBY = 0x20000000, 0x20010000, 0x20020000, 0x20030000
HOLDER, UI, MOVIE, BINDER, KEYS = 0x20040000, 0x20041000, 0x20042000, 0x20050000, 0x20060000
STACK, SENTINEL = 0x21000000, 0x22000000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
GAME = dict(zip(REGS, (0x0A0A0A0A, 0x0C0C0C0C, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
HUD_HOLDER, HUD_UI, HUD_MOVIE = 0x20070000, 0x20071000, 0x20072000
ICONS_HOLDER, ICONS_UI, ICONS_MOVIE = 0x20073000, 0x20074000, 0x20075000
# The keys bound (DIK code: action id), as the defaults have them: F11
# toggle_replay_ui (kinds 5), C character (2) and crouch_toggle (5), F friends
# (2) and melee (5), W forward (5), F6 next_player (5; the lobby's Play at
# 0x26CDA2); H and Enter nothing.
F11, C, F, W, H, F6, ENTER = 0x57, 0x2E, 0x21, 0x11, 0x23, 0x40, 0x1C
BOUND = {0x19: F11, 0x40: C, 0x06: C, 0x44: F, 0x21: F, 0x0B: W, 0x17: F6}


KEY_NAMES = 0xA74FA0                   # the key names: 88h-byte {name, DIK code, ...}, to a null name


def file_at(data, va):
    """The file offset of a VA of the exe's sections."""
    for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
        if rva <= va - base.IMAGE_BASE < rva + rsize:
            return raw + va - base.IMAGE_BASE - rva
    raise KeyError(hex(va))


def key_entry(data, dik=None, name=None):
    """The key-name table's entry of a DIK code or a name (the exe's own)."""
    i = 0
    while True:
        entry = KEY_NAMES + 0x88 * i
        text, code = struct.unpack_from("<II", data, file_at(data, entry))
        if text == 0:
            raise KeyError(dik if name is None else name)
        if dik is not None and code == dik:
            return entry
        if name is not None and bytes(data[file_at(data, text):file_at(data, text) + len(name) + 1]) == name + b"\0":
            return entry
        i += 1


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HANDLER, WORLD, HUD, LOBBY, HOLDER, BINDER, KEYS, HUD_HOLDER, STACK, SENTINEL):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.calls = []
        self.invoked = []
        self.hidden = 0
        for stub in (SHOW_MOVIE, HIDE_MOVIE, INVOKE, SHOW_CURSOR):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)
        self.stops = ()
        emu.hook_add(UC_HOOK_CODE, self.stop)
        # the binder: 48h bindings {description, key, second key}, the bound ones' keys made up
        emu.mem_write(BINDER, bytes(0x360))
        for i in range((0xA74FA0 - ACTIONS) // 20):
            description = ACTIONS + 20 * i
            action = self.read(description + 4)
            self.word(BINDER + 12 * action, description)
            if action in BOUND:
                self.word(BINDER + 12 * action + 4, key_entry(data, BOUND[action]))

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0]

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        pops = 0
        if address == SHOW_MOVIE:          # eax the scene, esi the movie's place; ret
            self.calls.append(("show", emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ESI)))
        elif address == HIDE_MOVIE:        # edx the scene, eax the movie's place; ret
            self.calls.append(("hide", emu.reg_read(UC_X86_REG_EDX), emu.reg_read(UC_X86_REG_EAX)))
        elif address == SHOW_CURSOR:       # edi the hud, the byte; ret 4
            self.calls.append(("cursor", emu.reg_read(UC_X86_REG_EDI), args[0] & 0xFF))
            pops = 4
        else:                              # Invoke: thiscall (name, result, arguments, count), ret 10h; al 1
            self.calls.append(("invoke", emu.reg_read(UC_X86_REG_ECX), self.text(args[0]), args[1] != 0,
                               args[2], args[3]))
            values = [struct.unpack("<II", self.emu.mem_read(args[2] + 24 * i + 4, 8)) for i in range(args[3])]
            self.invoked.append(tuple(self.text(v).decode() if t == 6 else v & 0xFF if t == 2 else v
                                      for t, v in values))
            if args[1] and self.text(args[0]) == b"root.interface_hidden":
                emu.mem_write(args[1] + 4, struct.pack("<II", 2, self.hidden))
            emu.reg_write(UC_X86_REG_EAX, 1)
            pops = 16
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        if address in self.stops and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def run(self, start, stops, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.calls, self.invoked = stops, None, False, [], []
        self.emu.emu_start(start, 0, count=500000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def movies(self, hud, movie, icons):
        """A hud at `hud`: its movie [[+8]+108h]+4, the icons' [[+10h]+108h]+4."""
        self.word(hud + 8, HUD_HOLDER if movie else 0)
        self.word(HUD_HOLDER + 0x108, HUD_UI)
        self.word(HUD_UI + 4, HUD_MOVIE)
        self.word(hud + 0x10, ICONS_HOLDER if icons else 0)
        self.word(ICONS_HOLDER + 0x108, ICONS_UI)
        self.word(ICONS_UI + 4, ICONS_MOVIE)

    def invokes(self):
        calls = [call for call in self.calls if call[0] == "invoke"]
        return [(call[1], call[2].decode(), values) for call, values in zip(calls, self.invoked)]

    # The match: the case 19h, [esp+10h] the handler.
    def match(self, replay, movie, icons, hidden):
        emu = self.emu
        emu.mem_write(HANDLER, bytes(0x400))
        self.word(HANDLER + 0x18, 0x30303030 if replay else 0)
        self.movies(HANDLER + 4, movie, icons)
        self.hidden = hidden
        sp = STACK + 0x8000
        emu.mem_write(STACK, bytes(0x10000))
        for i, value in enumerate((0xE0E0E0E0, 0x50505050, 0xB0B0B0B0, 0x11111111, HANDLER, 0x19FF00, SENTINEL)):
            self.word(sp + 4 * i, value)
        before = self.read(sp, 0x40)
        regs, esp = self.run(CASE, (REPLAY_BACK, KEY_OTHER), sp, {**GAME, UC_X86_REG_EBP: sp + 0x14})
        return (self.stopped, esp, regs[UC_X86_REG_ESI] if self.stopped == REPLAY_BACK else None,
                regs[UC_X86_REG_EBP], self.read(sp, 0x40) == before, self.invokes())

    # The lobby: the site's push of the toggle's place done, then its call.
    def lobby(self, key, movie, hidden=0):
        emu = self.emu
        self.hidden = hidden
        emu.mem_write(LOBBY, bytes(0x1000))
        self.word(LOBBY + 0x690, HOLDER if movie else 0)
        self.word(HOLDER + 0x108, UI)
        self.word(UI + 4, MOVIE)
        sp = STACK + 0x8000 - 4
        toggle = STACK + 0x9000
        self.word(toggle, 0x7E7E7E7E)
        self.word(sp, toggle)
        emu.mem_write(STACK + 0x8000, bytes(0x40))
        regs, esp = self.run(LOBBY_SITE, (LOBBY_BACK, LOBBY_TAKEN, LOBBY_TO_MOVIE), sp,
                             {**GAME, UC_X86_REG_EAX: BINDER, UC_X86_REG_EDI: 2, UC_X86_REG_ESI: key,
                              UC_X86_REG_EBX: LOBBY})
        return self.stopped, esp - sp, regs, list(self.calls), self.read(toggle)



# The options' controls: game_options' functions on a made-up options object.
KEYBINDINGS, KEYS_FROM_BINDER, KEY_SET, APPLY = 0x2019E0, 0x201CF0, 0x2013A0, 0x201F20
LOCALIZE, CREATE_OBJECT, CREATE_ARRAY = 0x1FF0F0, 0x511700, 0x511710
BIND_KEY, BIND_SAVE, LOOKUP = 0x207DC0, 0x207450, 0xEB7C0
ASSIGN = 0xE4590                       # buffer_string's printf-like assign (eax the string; cdecl format, ...)
SCANF = 0x422B46                       # the CRT's sscanf_s (cdecl: text, format, then a place and its size each)
SENT = 0x201C8B                        # 0x2019E0 after root.set_keybindings, before the keys
COMMANDS = 0x1F1D66C                   # the console commands' list head
DEFAULTS_SET = 0x1F1D9D4               # bind_key's "the defaults were set" flag
OPTIONS, GAMEOBJ, GFX, STUBS, CONSOLE, OUT = 0x25000000, 0x25010000, 0x25030000, 0x25040000, 0x25050000, 0x25060000
OBJECT, ARRAY = 0x08, 0x09             # the made-up GFx values' types (no managed bit 40h)


class Options(Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        for at in (OPTIONS, GAMEOBJ, GFX, STUBS, CONSOLE, OUT):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        # the options: +0Ch the movie's holder, +34h the game (+5344h the binder, +52A8h the strings)
        self.word(OPTIONS + 0x0C, HOLDER)
        self.word(HOLDER + 0x108, UI)
        self.word(UI + 4, MOVIE)
        self.word(OPTIONS + 0x34, GAMEOBJ)
        self.word(GAMEOBJ + 0x5344, BINDER)
        # a GFx object interface: +8 release, +14h SetMember, +3Ch PushBack
        self.word(GFX, GFX + 0x100)
        for slot, stub in ((0x08, STUBS), (0x14, STUBS + 0x10), (0x3C, STUBS + 0x20)):
            self.word(GFX + 0x100 + slot, stub)
        # the console's bind and unbind (+0 the vtable, +4 execute; +0Ch the next, +10h the name)
        for i, name in enumerate((b"bind", b"unbind")):
            command = CONSOLE + 0x100 * i
            emu.mem_write(CONSOLE + 0x800 + 0x20 * i, name + b"\0")
            self.word(command, CONSOLE + 0x400)
            self.word(command + 0x0C, CONSOLE + 0x100 * (i + 1) if i == 0 else 0)
            self.word(command + 0x10, CONSOLE + 0x800 + 0x20 * i)
        self.word(CONSOLE + 0x404, STUBS + 0x30)
        self.word(COMMANDS, CONSOLE)
        self.word(DEFAULTS_SET, 1)
        for stub in (STUBS, STUBS + 0x10, STUBS + 0x20, STUBS + 0x30, STUBS + 0x40, LOCALIZE, CREATE_OBJECT,
                     CREATE_ARRAY, ASSIGN, SCANF):
            emu.hook_add(UC_HOOK_CODE, self.gfx, begin=stub, end=stub)
        self.objects, self.rows, self.lines, self.handles, self.heap = {}, [], [], 0x100, OUT + 0x1000

    def gfx(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address in (CREATE_OBJECT, CREATE_ARRAY):  # (&value, ...): a new object or array
            self.handles += 1
            self.objects[self.handles] = {}
            self.emu.mem_write(args[0], struct.pack("<III", GFX, OBJECT if address == CREATE_OBJECT else ARRAY,
                                                    self.handles))
            pops = 16 if address == CREATE_OBJECT else 4
        elif address == STUBS:                # release
            pops = 8
        elif address == STUBS + 0x10:         # SetMember(data, name, &value, is a display object)
            kind, value = struct.unpack("<II", self.emu.mem_read(args[2] + 4, 8))
            self.objects[args[0]][self.text(args[1]).decode()] = self.text(value).decode() if kind == 6 else value
            pops = 16
        elif address == STUBS + 0x20:         # PushBack(data, &value)
            self.rows.append(self.objects[self.read(args[1] + 8)])
            pops = 8
        elif address == STUBS + 0x30:         # a console command's execute(the arguments)
            self.lines.append((self.text(self.read(emu.reg_read(UC_X86_REG_ECX) + 0x10)), self.text(args[0])))
            pops = 4
        elif address == ASSIGN:               # buffer_string assign "%s" (the CRT's printf is not run)
            if self.text(args[0]) != b"%s":
                raise ValueError(f"assign with {self.text(args[0])!r}")
            string, text = emu.reg_read(UC_X86_REG_EAX), self.text(args[1])
            begin, capacity = self.read(string), self.read(string + 8)
            text = text[:capacity - begin - 1]
            emu.mem_write(begin, text + b"\0")
            self.word(string + 4, begin + len(text))
            pops = 0
        elif address == SCANF:                # sscanf_s(text, "%255s %255s", a, 256, b, 256): two words
            if self.text(args[1]) != b"%255s %255s":
                raise ValueError(f"sscanf with {self.text(args[1])!r}")
            words = self.text(args[0]).split()
            for i, word in enumerate(words[:2]):
                emu.mem_write(self.read(sp + 12 + 8 * i), word[:255] + b"\0")
            emu.reg_write(UC_X86_REG_EAX, min(len(words), 2))
            pops = 0
        elif address == STUBS + 0x40:         # the save's allocator: malloc(size, ...)
            emu.reg_write(UC_X86_REG_EAX, self.heap)
            self.heap += (args[0] + 15) & ~15
            pops = 20
        else:                                 # the strings' look-up (key, out): the key itself
            self.emu.mem_write(args[1], self.text(args[0]) + b"\0")
            pops = 8
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def call(self, start, args, regs=None, until=SENTINEL):
        sp = STACK + 0x8000 - 4 * (len(args) + 1)
        self.word(sp, SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        self.calls, self.invoked = [], []
        out, esp = self.run(start, (until,), sp, {**GAME, **(regs or {})})
        return out, esp - sp

    def table(self):
        """The rows of the options' controls as 0x2019E0 sends them (root.set_keybindings)."""
        self.objects, self.rows = {}, []
        self.call(KEYBINDINGS, [OPTIONS], until=SENT)
        return list(self.rows), list(self.calls)

    def set_keybind_calls(self):
        calls = [call for call in self.calls if call[0] == "invoke"]
        return [values for call, values in zip(calls, self.invoked) if call[2] == b"root.set_keybind"]

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

    asked = (HUD_MOVIE, "root.interface_hidden", ())
    hides = lambda flag, icons: [(HUD_MOVIE, "root.hide_interface", (flag,))] + \
        ([(ICONS_MOVIE, "root.hide_interface", (flag,))] if icons else [])
    print("the match (on_keyboard_action's case 19h, 0x25CED1, to 0x25CEDB or 0x25CF0D)")
    for replay in (0, 1):
        for movie in (0, 1):
            for icons in (0, 1):
                for hidden in (0, 1):
                    a = stock.match(replay, movie, icons, hidden)
                    b = patched.match(replay, movie, icons, hidden)
                    same = a[:5] == b[:5] and a[4] and not a[5]
                    if replay:
                        want = []
                        ok = same and b[0] == REPLAY_BACK and b[2] == HANDLER
                    else:
                        want = [asked] + hides(1 - hidden, icons) if movie else []
                        ok = same and b[0] == KEY_OTHER
                    ok = ok and b[5] == want
                    report(ok, f"replay {replay} hud movie {movie} icons {icons}, the interface "
                               f"{'hidden' if hidden else 'shown'}: "
                               + (f"root.hide_interface({1 - hidden}) to the hud" + (" and the icons" if icons else "")
                                  if want else "nothing") + f", at {b[0]:#x} as the original")

    print("the lobby (lobby_menu::on_keyboard_action's binder call, 0x26CCD2)")
    asked = ("invoke", MOVIE, b"root.interface_hidden", True, 0, 0)
    for key, name in ((F11, "F11"), (C, "C"), (F, "F"), (W, "W"), (H, "H"), (F6, "F6"), (ENTER, "Enter")):
        for movie in (True, False):
            for hidden in (0, 1):
                a = stock.lobby(key, movie, hidden)
                b = patched.lobby(key, movie, hidden)
                keep = {UC_X86_REG_EBP: GAME[UC_X86_REG_EBP], UC_X86_REG_EDI: 2, UC_X86_REG_ESI: key,
                        UC_X86_REG_EBX: LOBBY}
                kept = all(b[2][reg] == value for reg, value in keep.items())
                same = a[0] == LOBBY_BACK and a[1] == b[1] == 4 and kept and not a[3] and a[4] == b[4]
                if key == F11 and movie:
                    ok = same and a[2][UC_X86_REG_EAX] == 0x49 and b[0] == LOBBY_TAKEN \
                        and b[3] == [("invoke", MOVIE, b"root.toggle_interface", False, 0, 0)]
                    what = "root.toggle_interface invoked, the key taken (0x26ccae)"
                elif movie and hidden:
                    ok = same and b[0] == LOBBY_TO_MOVIE and b[3] == [asked]
                    what = "hidden: no action of the game, the key to the movie (0x26cdd2)"
                else:
                    ok = same and b[0] == LOBBY_BACK and b[3] == ([asked] if movie else []) \
                        and a[2][UC_X86_REG_EAX] == b[2][UC_X86_REG_EAX]
                    what = f"action {b[2][UC_X86_REG_EAX]:#x} as the original's"
                report(ok, f"{name}, movie {'yes' if movie else 'no'}, the interface "
                           f"{'hidden' if hidden else 'shown'}: {what}, its toggle place as the original's, "
                           "ebx esi edi ebp kept")

    print("the options' controls (game_options 0x2019E0, 0x201CF0, 0x2013A0, 0x201F20; the binder's bind "
          "0x207DC0 and its save 0x207450)")
    s_opt, p_opt = Options(original), Options(bytes(built))
    a_rows, _ = s_opt.table()
    b_rows, b_calls = p_opt.table()
    new = {"action_id": 0x19, "group_id": 2, "type": 1, "label": "st_hide_interface_action"}
    report(len(a_rows) == 40 and len(b_rows) == 41 and b_rows[:40] == a_rows and b_rows[40] == new
           and [c[2] for c in b_calls] == [b"root.set_keybindings"],
           f"the list sent to the movie (root.set_keybindings): the original's {len(a_rows)} rows, then "
           f"{b_rows[40] if len(b_rows) > 40 else None}")
    s_opt.call(KEYS_FROM_BINDER, [OPTIONS, 1])
    p_opt.call(KEYS_FROM_BINDER, [OPTIONS, 1])
    a_keys, b_keys = s_opt.set_keybind_calls(), p_opt.set_keybind_calls()
    report(len(a_keys) == 40 and b_keys[:40] == a_keys and b_keys[40:] == [(0x19, "kF11", 1)],
           f"the keys from the binder (root.set_keybind): the original's 40, then {b_keys[40:]}")
    for run in (s_opt, p_opt):
        run.emu.mem_write(OUT, b"kF9\0")
        run.call(KEY_SET, [OPTIONS, 0x19], {UC_X86_REG_EAX: OUT})
    a_set, b_set = s_opt.set_keybind_calls(), p_opt.set_keybind_calls()
    kept = [s_opt.text(s_opt.read(0xA9DB28 + 0x68 * i + 0x10)) for i in range(40)]
    report(a_set == b_set == [(0x19, "kF9", 0)] and b"kF9" not in kept
           and p_opt.text(p_opt.read(linked.symbols["controls"] + 40 * 0x68 + 0x10)) == b"kF9",
           f"a key set for 19h (kF9, 0x2013A0): shown {b_set} by both, kept in the 41st row; the original "
           "keeps it in no row")
    for run in (s_opt, p_opt):
        run.lines = []
        run.call(APPLY, [])
    report(not s_opt.lines and p_opt.lines == [(b"bind", b"toggle_replay_ui kF9")],
           f"the apply (0x201F20) runs {p_opt.lines}; the original's runs {s_opt.lines}")
    f9, f11 = key_entry(original, name=b"kF9"), key_entry(original, name=b"kF11")
    slot = BINDER + 12 * 0x19 + 4
    p_opt.emu.mem_write(OUT + 0x100, p_opt.lines[0][1] + b"\0" if p_opt.lines else b"\0")
    p_opt.call(BIND_KEY, [OUT + 0x100, 0], {UC_X86_REG_ECX: BINDER})
    bound = p_opt.read(slot)
    # the bind command's save: +10h its name, +60h its key (0 the first), +64h the binder; the list it
    # fills: +4 its end, +0Ch its capacity's end, +10h an allocator (+10h its malloc)
    command, saved, allocator = CONSOLE + 0x600, OUT + 0x2000, OUT + 0x3000
    p_opt.word(command + 0x10, CONSOLE + 0x800)
    p_opt.word(command + 0x60, 0)
    p_opt.word(command + 0x64, BINDER)
    p_opt.word(allocator, allocator + 0x20)
    p_opt.word(allocator + 0x20 + 0x10, STUBS + 0x40)
    for field, value in ((0, OUT + 0x2100), (4, OUT + 0x2100), (0xC, OUT + 0x2400), (0x10, allocator)):
        p_opt.word(saved + field, value)
    p_opt.call(BIND_SAVE, [saved], {UC_X86_REG_ECX: command})
    lines = [p_opt.text(p_opt.read(at)) for at in range(OUT + 0x2100, p_opt.read(saved + 4), 4)]
    p_opt.word(slot, f11)
    line = next((x for x in lines if x.startswith(b"bind toggle_replay_ui ")), b"")
    p_opt.emu.mem_write(OUT + 0x200, line[len(b"bind "):] + b"\0")
    p_opt.call(BIND_KEY, [OUT + 0x200, 0], {UC_X86_REG_ECX: BINDER})
    report(bound == f9 and line == b"bind toggle_replay_ui kF9" and p_opt.read(slot) == f9,
           f"the binder's bind (0x207DC0) binds 19h to kF9; its save (0x207450) writes {line!r} among "
           f"{len(lines)} lines; read back (bind, after F11 again) it binds kF9")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
