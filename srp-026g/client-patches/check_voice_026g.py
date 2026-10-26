#!/usr/bin/env python3
"""Run patch 66 (safe Rust patches::voice: voice chat) in the default
build on made-up game objects and a made-up skhid_voice.dll.

    python check_voice_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; every entry runs from the game's own site or slot, the
game's own console-variable code (cc_value<float> 0x1DD40, cc_value<bool>
0x1DDB0, cc_string 0xEC860, the lookup 0xEB7C0 and execute 0xEB980) and
key binder (get_binded_action 0x207D50, over the exe's action table) for
real. Stubs record their calls: kernel32's LoadLibraryA, GetProcAddress,
GetModuleHandleA and FreeLibrary (their import slots), user32's and
kernel32's focus functions, the DLL's sv_* exports (cdecl), Scaleform's
Invoke (0x511600), translate_text (0x1FF0F0) and the game's functions each
site calls on (game_world::game_world 0x1F67B0, show_player_list 0x2384C0,
game_options::activate 0x200F50, game_world::tick 0x1F77E0, game_options'
callback 0x1FF5D0). Cases:

- the variables (game::game's call 0x1E90A3): the world made, then
  s_chat_volume and s_mic_sens (cc_float, 0..100), s_use_microphone and
  g_hide_spam (cc_bool), srp_voice_mutes (cc_string) registered once, saved
  (kind 2); the game's lookup finds each; user.cfg lines set them;
- the action table: entry 37h is mute_last_speaker, kind 8, no default key;
  the binder answers 37h for its key with kind 8 (the original: 49h) and 47h
  (ptt) as before;
- the connect (match_client::connect 0x27A1B0): the DLL loaded once, its
  version asked, sv_start(host, port + 1, session); no DLL, or another
  version (unloaded): nothing started; back at 0x27A1B5 with the moved
  instructions' registers and stack;
- the frames (game_world::tick through the vtable slot 0x9FC4FC): the
  options to sv_settings on a change only; the muted slots (srp_voice_mutes'
  voice ids, sv_voice_id) to sv_mute; the far slots (25 m; a dead player or
  a dead self) to sv_radio; the hud's root.srp_voice_talking(name, flags...)
  on a change only, the player first, muted slots left out; "Disable voice
  chat": sv_settings(0, 0, ...) and the list empty; the tick reached with
  the world and its arguments;
- the keys (on_keyboard_action's call 0x25CD34): ptt down and up to sv_ptt,
  held nothing, not while voice is off or the microphone is; a ptt whose key
  is no longer held, or with the game out of focus, let go at the next
  frame; the mute key mutes and unmutes the last speaker by voice id with a
  hud notice, never the player; show_player_list called as before;
- the Esc menu (activate_main_menu's call 0x1EDBDC): over the match's
  world, root.srp_voice_panel(title, mute all, slot, name, muted...) of the
  teammates with a voice id (no enemy, not the player); over the lobby, the
  panel empty; activate's answer kept;
- the menu's calls (game_options' callback slot 0x9FD47C): srp_voice_mute
  and srp_voice_mute_all set the mutes (teammates only) and return `ret 10h`;
  any other call reaches the game's callback with its arguments;
- the end (on_finish_match 0x1F96F0, on_deactivate 0x1F9520): sv_stop once,
  back in the function with the moved instructions' registers.
The original exe, run the same way where it has the site, calls none of it.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

REGISTER_SITE, REGISTER_BACK, WORLD_CONSTRUCTOR = 0x1E90A3, 0x1E90B2, 0x1F67B0
CONNECT, CONNECT_BACK = 0x27A1B0, 0x27A1B5
FINISH, FINISH_BACK, DEACTIVATE, DEACTIVATE_BACK = 0x1F96F0, 0x1F96F9, 0x1F9520, 0x1F9525
KEY_SITE, KEY_BACK, SHOW_PLAYER_LIST = 0x25CD34, 0x25CD39, 0x2384C0
MENU_SITE, MENU_BACK, ACTIVATE = 0x1EDBDC, 0x1EDBE1, 0x200F50
TICK_SLOT, TICK = 0x9FC4FC, 0x1F77E0
CALLBACK_SLOT, CALLBACK = 0x9FD47C, 0x1FF5D0
INVOKE, TRANSLATE = 0x511600, 0x1FF0F0
SCANF = 0x422B46                       # the CRT's sscanf_s (cdecl), as cc_float's execute (0xECB70) reads a number
LOOKUP, EXECUTE, BINDED = 0xEB7C0, 0xEB980, 0x207D50
IAT = {0x87003C: "GetModuleHandleA", 0x870080: "LoadLibraryA", 0x870084: "GetProcAddress",
       0x870088: "FreeLibrary"}
ACTIONS, KEY_NAMES = 0xA74AF0, 0xA74FA0
COMMANDS = 0x1F1D66C

GAME = 0x20000000                      # the game; its world at +18h, its options at +57F4h
WORLD = GAME + 0x18
NETWORK = 0x20100000                   # network_client (+1F1C44h the local place, +1F1C48h the match)
MATCH = 0x20400000                     # the match client: +0Ch the static info
CORE = 0x20500000                      # [world+4D4Ch]; the core +1F8h, its places [+18458h]
PLACES = 0x20520000
PLAYERS = 0x20600000                   # a player each 1000h
HUD_HOLDER, HUD_UI, HUD_MOVIE = 0x20700000, 0x20700200, 0x20700400
MENU_HOLDER, MENU_UI, MENU_MOVIE = 0x20700600, 0x20700800, 0x20700A00
BINDER = 0x20800000
STUBS = 0x20900000                     # an address per stub
TEXT = 0x20A00000
INPUT, KEYBOARD = 0x20B00000, 0x20B00100
STACK, SENTINEL = 0x21000000, 0x22000000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
MARKS = dict(zip(REGS, (0x0A0A0A0A, 0x0C0C0C0C, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
DLL_EXPORTS = ("sv_version", "sv_start", "sv_stop", "sv_ptt", "sv_settings", "sv_radio", "sv_mute",
               "sv_talking", "sv_slot", "sv_last_speaker", "sv_voice_id")
WIN32 = ("GetForegroundWindow", "GetWindowThreadProcessId", "GetCurrentProcessId")
STUB_NAMES = list(IAT.values()) + list(DLL_EXPORTS) + list(WIN32) + ["keyboard_of", "pressed"]
STUB_AT = {name: STUBS + 0x10 * i for i, name in enumerate(STUB_NAMES)}
STUB_NAME = {at: name for name, at in STUB_AT.items()}
# stdcall pops (the Win32 functions'), thiscall pops (the keyboard's); cdecl 0 (the DLL's)
POPS = {"GetModuleHandleA": 4, "LoadLibraryA": 4, "GetProcAddress": 8, "FreeLibrary": 4,
        "GetForegroundWindow": 0, "GetWindowThreadProcessId": 8, "GetCurrentProcessId": 0,
        "keyboard_of": 0, "pressed": 4}
GAME_FUNCTIONS = {WORLD_CONSTRUCTOR: ("world", 8), SHOW_PLAYER_LIST: ("show_player_list", 4),
                  ACTIVATE: ("activate", 4), TICK: ("tick", 12), CALLBACK: ("callback", 16),
                  INVOKE: ("invoke", 16), TRANSLATE: ("translate", 8), SCANF: ("scanf", 0)}
MODULE, USER32, KERNEL32, WINDOW = 0x5000, 0x6000, 0x7000, 0x8000
PROCESS = 0x1234
DIK_P, DIK_O, DIK_B, DIK_W = 0x19, 0x18, 0x30, 0x11
# The match: places 0..4; the player is place 1 (team 0) with teammates 0
# (10 m away), 3 (30 m away) and 4 (dead), and an enemy at place 2 (50 m away).
NAMES = ["Alice", "Me", "Enemy", "Carol", "Dave"]
TEAMS = [0, 0, 1, 0, 0]
SELF = 1
POSITIONS = {0: (10.0, 0.0, 0.0), 1: (0.0, 0.0, 0.0), 2: (50.0, 0.0, 0.0), 3: (0.0, 30.0, 0.0)}
IDS = {0: 0xA11CE, 1: 0x3E, 2: 0xE0E0, 3: 0xCA201, 4: 0xDA4E}


def f32(value):
    return struct.unpack("<f", struct.pack("<I", value))[0]


class Run:
    def __init__(self, data, linked=None):
        self.data = data
        self.linked = linked
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((GAME, 0x10000), (NETWORK, 0x200000), (MATCH, 0x60000), (CORE, 0x40000),
                         (PLAYERS, 0x10000), (HUD_HOLDER, 0x1000), (BINDER, 0x1000), (STUBS, 0x1000),
                         (TEXT, 0x10000), (INPUT, 0x1000), (STACK, 0x10000), (SENTINEL, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        for slot, name in IAT.items():
            self.word(slot, STUB_AT[name])
        for at in list(STUB_AT.values()) + list(GAME_FUNCTIONS):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, self.stop)
        self.stops, self.calls = (), []
        # what the made-up DLL and Windows answer
        self.dll = True
        self.version = 1
        self.talking = 0
        self.last_speaker = -1
        self.ids = dict(IDS)
        self.held = set()
        self.foreground = WINDOW
        self.activate_answer = 0x77
        # the made-up DLL's push-to-talk as the last sv_ptt left it (a stop or a start does not
        # change it: the case of a key held across a match's end)
        self.dll_ptt = 0
        # the binder over the exe's action table: P ptt (and deserialize_player_state), O
        # (serialize_player_state; in the patch mute_last_speaker), W forward
        emu.mem_write(BINDER, bytes(0x360))
        for i in range((KEY_NAMES - ACTIONS) // 20):
            description = ACTIONS + 20 * i
            action = self.read(description + 4)
            self.word(BINDER + 12 * action, description)
        for action, key in ((0x47, DIK_P), (0x38, DIK_P), (0x37, DIK_O), (0x0B, DIK_W)):
            self.word(BINDER + 12 * action + 4, self.key_entry(key))
        self.match()

    def key_entry(self, dik):
        i = 0
        while True:
            entry = KEY_NAMES + 0x88 * i
            text, code = struct.unpack("<II", self.emu.mem_read(entry, 8))
            if text == 0:
                raise KeyError(dik)
            if code == dik:
                return entry
            i += 1

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 128)).split(b"\0")[0]

    def put_text(self, text):
        at = TEXT + 0x8000 + self.next_text
        self.emu.mem_write(at, text + b"\0")
        self.next_text = (self.next_text + len(text) + 16) & ~3
        return at

    # The made-up match.
    def match(self, alive=(0, 1, 2, 3)):
        self.next_text = 0
        self.word(WORLD + 0x9C, GAME)
        self.word(WORLD + 0x204, HUD_HOLDER)
        self.word(HUD_HOLDER + 0x108, HUD_UI)
        self.word(HUD_UI + 4, HUD_MOVIE)
        self.word(WORLD + 0x4D4C, CORE)
        self.word(GAME + 0x5344, BINDER)
        self.word(GAME + 0x53AC, NETWORK)
        options = GAME + 0x57F4
        self.word(options + 0xC, MENU_HOLDER)
        self.word(MENU_HOLDER + 0x108, MENU_UI)
        self.word(MENU_UI + 4, MENU_MOVIE)
        self.word(options + 0x34, GAME)
        self.emu.mem_write(NETWORK + 0x1F1C44, bytes([SELF]))
        self.word(NETWORK + 0x1F1C48, MATCH)
        info = MATCH + 0xC
        self.emu.mem_write(info + 0x577B5, bytes([len(NAMES)]))
        for place, (name, team) in enumerate(zip(NAMES, TEAMS)):
            profile = info + 0x45FC * place
            self.emu.mem_write(profile + 8, name.encode() + b"\0")
            self.word(profile + 0x45EC, team)
        self.word(CORE + 0x1F8 + 0x18458, PLACES)
        for place in range(len(NAMES)):
            entry = PLACES + 0x30 * place
            player = PLAYERS + 0x1000 * place
            self.emu.mem_write(entry, bytes(0x30))
            self.word(entry + 4, player)               # kit 1's player
            self.emu.mem_write(entry + 0xC, bytes([1]))
            self.emu.mem_write(player, bytes(0x1000))
            living = place in alive
            self.emu.mem_write(player + 0x33C, bytes([living]))
            self.emu.mem_write(player + 0x33F, bytes([living]))
            self.emu.mem_write(player + 0x43C, struct.pack("<3f", *POSITIONS.get(place, (0.0, 0.0, 0.0))))
        self.word(INPUT, INPUT + 0x40)                 # the input world's vtable: +24h the keyboard
        self.word(INPUT + 0x40 + 0x24, STUB_AT["keyboard_of"])
        self.word(KEYBOARD, KEYBOARD + 0x40)
        self.word(KEYBOARD + 0x40, STUB_AT["pressed"])

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 7)]
        result = 0
        if address in STUB_AT.values():
            name = STUB_NAME[address]
            pops = POPS.get(name, 0)
            if name == "LoadLibraryA":
                self.calls.append((name, self.text(args[0]).decode()))
                result = MODULE if self.dll else 0
            elif name == "GetModuleHandleA":
                result = {b"user32.dll": USER32, b"kernel32.dll": KERNEL32}.get(self.text(args[0]), 0)
            elif name == "GetProcAddress":
                wanted = self.text(args[1]).decode()
                result = STUB_AT.get(wanted, 0) if args[0] in (MODULE, USER32, KERNEL32) else 0
            elif name == "FreeLibrary":
                self.calls.append((name, args[0]))
            elif name == "GetForegroundWindow":
                result = self.foreground
            elif name == "GetWindowThreadProcessId":
                self.word(args[1], PROCESS if args[0] == WINDOW else PROCESS + 1)
            elif name == "GetCurrentProcessId":
                result = PROCESS
            elif name == "keyboard_of":
                result = KEYBOARD if emu.reg_read(UC_X86_REG_ECX) == INPUT else 0
            elif name == "pressed":
                result = int(args[0] in self.held)
            elif name == "sv_version":
                result = self.version
            elif name == "sv_start":
                self.calls.append((name, self.text(args[0]).decode(), args[1], args[2]))
            elif name == "sv_settings":
                self.calls.append((name, round(f32(args[0]), 3), args[1], round(f32(args[2]), 3)))
            elif name in ("sv_ptt", "sv_radio", "sv_mute"):
                self.calls.append((name, args[0]))
                if name == "sv_ptt":
                    self.dll_ptt = args[0]
            elif name == "sv_stop":
                self.calls.append((name,))
            elif name == "sv_talking":
                result = self.talking
            elif name == "sv_last_speaker":
                result = self.last_speaker & 0xFFFFFFFF
            elif name == "sv_voice_id":
                result = self.ids.get(args[0], 0)
            elif name == "sv_slot":
                result = SELF
        else:
            name, pops = GAME_FUNCTIONS[address]
            if name == "invoke":           # thiscall (name, result, arguments, count)
                values = []
                for i in range(args[3]):
                    kind, value = struct.unpack("<II", self.emu.mem_read(args[2] + 24 * i + 4, 8))
                    values.append(self.text(value).decode() if kind == 6 else bool(value & 0xFF) if kind == 2
                                  else value)
                self.calls.append(("invoke", emu.reg_read(UC_X86_REG_ECX), self.text(args[0]).decode(), values))
                result = 1
            elif name == "scanf":          # sscanf_s(text, "%f", place): one float
                if self.text(args[1]) != b"%f":
                    raise ValueError(f"sscanf with {self.text(args[1])!r}")
                try:
                    number = float(self.text(args[0]).split()[0])
                    self.emu.mem_write(args[2], struct.pack("<f", number))
                    result = 1
                except (ValueError, IndexError):
                    result = 0
            elif name == "translate":      # thiscall (key, buffer)
                self.emu.mem_write(args[1], b"T:" + self.text(args[0]) + b"\0")
            elif name == "world":
                self.calls.append(("world", emu.reg_read(UC_X86_REG_EBX), args[0], args[1]))
                result = 0x5151
            elif name == "show_player_list":
                self.calls.append(("show_player_list", emu.reg_read(UC_X86_REG_ESI), args[0] & 0xFF))
            elif name == "activate":
                self.calls.append(("activate", emu.reg_read(UC_X86_REG_EAX), args[0]))
                result = self.activate_answer
            elif name == "tick":
                self.calls.append(("tick", emu.reg_read(UC_X86_REG_ECX), *args[:3]))
            elif name == "callback":
                self.calls.append(("callback", emu.reg_read(UC_X86_REG_ECX), *args[:4]))
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        if address in self.stops:
            self.stopped = address
            emu.emu_stop()

    def run(self, start, stops, sp, regs=None):
        for reg, value in {**MARKS, **(regs or {})}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.calls = tuple(stops), None, []
        self.emu.emu_start(start, 0, count=2000000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def call(self, start, args, regs=None):
        """`start` called with `args` pushed: (registers, esp after its return - esp before the call)."""
        sp = STACK + 0x8000 - 4 * (len(args) + 1)
        self.word(sp, SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        regs, esp = self.run(start, (SENTINEL,), sp, regs)
        return regs, esp - (sp + 4)

    def site(self, start, back, args, regs=None):
        """From a `call` site with `args` on the stack, to `back`: (registers, esp after - esp before)."""
        sp = STACK + 0x8000 - 4 * len(args)
        for i, value in enumerate(args):
            self.word(sp + 4 * i, value)
        regs, esp = self.run(start, (back,), sp, regs)
        return regs, esp - sp

    def calls_of(self, *names):
        return [c for c in self.calls if c[0] in names]

    # The state of the patch.
    def symbol(self, name):
        return self.linked.symbols[name]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    stock, run = Run(original), Run(bytes(built), linked)
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    mutes = lambda: run.text(run.read(found[b"srp_voice_mutes"] + 0x40)).decode()

    print("the variables (game::game's call of game_world::game_world, 0x1E90A3)")
    for r in (stock, run):
        r.emu.mem_write(TEXT, b"minimap_position\0")
        r.emu.mem_write(TEXT + 0x100, bytes(0x48))
        r.word(TEXT + 0x100 + 0x10, TEXT)
        r.word(COMMANDS, TEXT + 0x100)
    a = stock.site(REGISTER_SITE, REGISTER_BACK, [GAME, 0x42])
    a_calls = stock.calls
    for _ in range(2):
        b = run.site(REGISTER_SITE, REGISTER_BACK, [GAME, 0x42])
    report(a_calls == run.calls == [("world", MARKS[UC_X86_REG_EBX], GAME, 0x42)] and a[1] == b[1] == 8
           and a[0][UC_X86_REG_EAX] == b[0][UC_X86_REG_EAX] == 0x5151
           and all(b[0][reg] == a[0][reg] for reg in REGS[3:]),
           "game_world::game_world called with its two arguments and ebx, its answer and `ret 8` kept, "
           "ebx esi edi ebp kept (twice)")
    lookup = lambda r, name: r.call(LOOKUP, [], {UC_X86_REG_ESI: r.put_text(name)})[0][UC_X86_REG_EAX]
    names = (b"s_chat_volume", b"s_mic_sens", b"s_use_microphone", b"g_hide_spam", b"srp_voice_mutes")
    found = {name: lookup(run, name) for name in names}
    report(all(found.values()) and not any(lookup(stock, name) for name in names)
           and lookup(run, b"minimap_position") == TEXT + 0x100,
           "the game's lookup (0xEB7C0) finds the five, the original none; the command before them still found")
    vtables = {name: run.read(found[name]) for name in names}
    float_fields = [struct.unpack("<ff", run.read(found[name] + 0x44, 8)) for name in names[:2]]
    report(vtables[b"s_chat_volume"] == vtables[b"s_mic_sens"] == 0x9E7C6C
           and vtables[b"s_use_microphone"] == vtables[b"g_hide_spam"] == 0x9E7C4C
           and vtables[b"srp_voice_mutes"] == 0x9E7C2C and float_fields == [(0.0, 100.0)] * 2
           and all(run.read(found[name] + 0x14) == 2 and run.read(found[name] + 0x1C, 2)[1] == 1 for name in names),
           "cc_float (0x9E7C6C, min 0, max 100) twice, cc_bool (0x9E7C4C) twice, cc_string (0x9E7C2C); "
           "kind 2, saved")
    values = lambda: ((f32(run.read(run.read(found[b"s_chat_volume"] + 0x40))),
                       f32(run.read(run.read(found[b"s_mic_sens"] + 0x40)))),
                      run.read(run.read(found[b"s_use_microphone"] + 0x40), 1) +
                      run.read(run.read(found[b"g_hide_spam"] + 0x40), 1))
    report(values() == ((100.0, 50.0), b"\x01\x00") and mutes() == "",
           "the defaults: volume 100, sensitivity 50, microphone on, voice not disabled, no mutes")
    execute = lambda line: run.call(EXECUTE, [run.put_text(line), 1, 2])
    for line in (b"s_chat_volume 40", b"s_mic_sens 75", b"s_use_microphone off", b"g_hide_spam on",
                 b"srp_voice_mutes 000ca201|"):
        execute(line)
    report(values() == ((40.0, 75.0), b"\x00\x01") and mutes() == "000ca201|",
           "user.cfg's lines (execute 0xEB980: filter 1, mask 2) set each")
    for line in (b"s_chat_volume 100", b"s_mic_sens 50", b"s_use_microphone on", b"g_hide_spam off",
                 b"srp_voice_mutes 000ca201|"):
        execute(line)

    print("the action table and the binder (get_binded_action 0x207D50)")
    entry = struct.unpack("<5I", run.read(0xA74E10, 20))
    binded = lambda r, key, kinds: r.call(BINDED, [TEXT + 0x400], {UC_X86_REG_EAX: BINDER, UC_X86_REG_EDI: kinds,
                                                                    UC_X86_REG_ESI: key})[0][UC_X86_REG_EAX]
    report(run.text(entry[0]) == b"mute_last_speaker" and entry[1:] == (0x37, 8, 0, 0)
           and binded(run, DIK_O, 8) == 0x37 and binded(stock, DIK_O, 8) == 0x49
           and binded(run, DIK_O, 1) == 0x49 and binded(stock, DIK_O, 1) == 0x37
           and binded(run, DIK_P, 8) == binded(stock, DIK_P, 8) == 0x47,
           "entry 0xA74E10 is mute_last_speaker, 37h, kind 8, no default key; the chat keys' lookup finds it "
           "(the original's: none), the game's no longer; ptt (47h) as before")

    print("the connect (match_client::connect 0x27A1B0)")

    def connect(r, port=2236, session=0xBEEF):
        host = r.put_text(b"203.0.113.9")
        sp = STACK + 0x8000 - 24
        for i, value in enumerate((SENTINEL, host, 0xFFFF0000 | port, session, 7, 8)):
            r.word(sp + 4 * i, value)
        regs, esp = r.run(CONNECT, (CONNECT_BACK,), sp, {UC_X86_REG_ECX: MATCH})
        return regs, esp - sp, r.stopped
    a, b = connect(stock), connect(run)
    moved = lambda regs, esp, at: at == CONNECT_BACK and esp == -12 and regs[UC_X86_REG_ESI] == MATCH \
        and all(regs[reg] == MARKS[reg] for reg in (UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EBP))
    report(moved(*a) and moved(*b) and stock.calls == []
           and run.calls == [("LoadLibraryA", "skhid_voice.dll"), ("sv_ptt", 0),
                             ("sv_start", "203.0.113.9", 2237, 0xBEEF)],
           "the DLL loaded, sv_start(host, 2236 + 1, session); back at 0x27A1B5 with three pushes and esi the "
           "match client, as the original")
    connect(run, 3000, 0x1111)
    report(run.calls == [("sv_ptt", 0), ("sv_stop",), ("sv_ptt", 0), ("sv_start", "203.0.113.9", 3001, 0x1111)],
           "a second connect: the old session stopped, no second load; push-to-talk released before the start")
    for dll, version, want in ((False, 1, [("LoadLibraryA", "skhid_voice.dll")]),
                               (True, 2, [("LoadLibraryA", "skhid_voice.dll"), ("FreeLibrary", MODULE)])):
        other = Run(bytes(built), linked)
        other.dll, other.version = dll, version
        connect(other)
        first = list(other.calls)
        connect(other)
        report(first == want and other.calls == [],
               f"{'no DLL' if not dll else 'a DLL of version 2'}: nothing started"
               + (", the DLL unloaded" if dll else "") + "; not tried again")

    print("the frames (game_world::tick through its vtable slot 0x9FC4FC)")
    slot = run.read(TICK_SLOT)
    report(stock.read(TICK_SLOT) == TICK and slot == linked.symbols["voice_tick"],
           "the slot names voice_tick (the original's: tick 0x1F77E0)")

    def frame(r, talking=0):
        r.talking = talking
        regs, popped = r.call(r.read(TICK_SLOT), [1, 2, 3], {UC_X86_REG_ECX: WORLD})
        return regs, popped, [c for c in r.calls if c[0] != "tick"], r.calls_of("tick")
    connect(run, 2236, 0xBEEF)
    regs, popped, calls, ticks = frame(run)
    report(ticks == [("tick", WORLD, 1, 2, 3)] and popped == 12 and all(regs[reg] == MARKS[reg] for reg in REGS[3:]),
           "tick reached with the world and its three arguments, `ret 0Ch`, the nonvolatile registers kept")
    report(calls == [("sv_settings", 1.0, 1, 0.5), ("sv_mute", 1 << 3), ("sv_radio", (1 << 2) | (1 << 3) | (1 << 4)),
                     ("invoke", HUD_MOVIE, "root.srp_voice_talking", [])],
           f"the first frame: the options (1.0, microphone on, 0.5), the mutes (Carol, 000ca201: slot 3), the far "
           f"slots (2, 3 at 30 m, 4 dead; not 0 at 10 m); got {calls}")
    report(frame(run)[2] == [], "the next frame, nothing changed: nothing sent")
    calls = frame(run, (1 << 0) | (1 << 3) | (1 << SELF))[2]
    report(calls == [("invoke", HUD_MOVIE, "root.srp_voice_talking", ["Me", 1, "Alice", 0])],
           f"Alice, Carol (muted) and the player talking: the hud's list is the player's (flag 1), then Alice "
           f"(near, 0); Carol left out; got {calls}")
    report(frame(run, (1 << 0) | (1 << 3) | (1 << SELF))[2] == [], "the same again: not sent again")
    calls = frame(run, (1 << 2) | (1 << 31))[2]
    report(calls == [("invoke", HUD_MOVIE, "root.srp_voice_talking", ["Me", 1, "Enemy", 2])],
           f"bit 31 (the player before its slot is known) is the player; slot 2 far (flag 2); got {calls}")
    calls = frame(run, 0)[2]
    report(calls == [("invoke", HUD_MOVIE, "root.srp_voice_talking", [])], "nobody: the list emptied")
    run.match(alive=(0, 2, 3))
    calls = frame(run, 0)[2]
    report(calls == [("sv_radio", (1 << 0) | (1 << 2) | (1 << 3) | (1 << 4))],
           f"the player dead: every other slot far; got {calls}")
    run.match()
    frame(run, 0)
    execute(b"g_hide_spam on")
    calls = frame(run, 1 << 0)[2]
    report(calls == [("sv_settings", 0.0, 0, 0.5)],
           f"Disable voice chat: sv_settings(0, 0, ...), Alice talking but no list; got {calls}")
    execute(b"g_hide_spam off")
    execute(b"s_chat_volume 25")
    calls = frame(run, 0)[2]
    report(calls == [("sv_settings", 0.25, 1, 0.5)], f"volume 25: sv_settings(0.25, ...); got {calls}")
    execute(b"s_chat_volume 100")
    frame(run, 0)

    print("the keys (on_keyboard_action's call of show_player_list, 0x25CD34)")
    handler = WORLD + 0x1F8

    def key(r, dik, kind):
        frame_at = STACK + 0x9000
        for i, value in enumerate((0x19FF00, SENTINEL, INPUT, dik, kind)):
            r.word(frame_at + 4 * i, value)
        sp = STACK + 0x8800
        r.word(sp, 0x1)                    # the shown byte
        regs, esp = r.run(KEY_SITE, (KEY_BACK,), sp,
                          {UC_X86_REG_EDI: handler, UC_X86_REG_EBP: frame_at, UC_X86_REG_ESI: GAME + 0x214})
        ok = r.stopped == KEY_BACK and esp == sp + 4 and r.calls_of("show_player_list") == [
            ("show_player_list", GAME + 0x214, 1)] \
            and regs[UC_X86_REG_EDI] == handler and regs[UC_X86_REG_EBP] == frame_at
        return ok, [c for c in r.calls if c[0] != "show_player_list"]
    sk = key(stock, DIK_P, 1)
    report(sk[0] and sk[1] == [], "the original: show_player_list as the site called it, nothing else")
    run.held = {DIK_P}
    steps = [key(run, DIK_P, 1), key(run, DIK_P, 2), key(run, DIK_W, 1), key(run, DIK_P, 3)]
    report(all(ok for ok, _ in steps) and [c for _, c in steps] == [[("sv_ptt", 1)], [], [], [("sv_ptt", 0)]],
           "P down: sv_ptt(1); held, or another key: nothing; P up: sv_ptt(0); show_player_list called as "
           "before each time, back at 0x25CD39")
    key(run, DIK_P, 1)
    run.held = set()
    report(frame(run)[2] == [("sv_ptt", 0)], "P down, then no longer held (its up lost): let go at the next frame")
    run.held = {DIK_P}
    key(run, DIK_P, 1)
    run.foreground = WINDOW + 1
    report(frame(run)[2] == [("sv_ptt", 0)], "P held, the game out of focus: let go at the next frame")
    run.foreground = WINDOW
    key(run, DIK_P, 3)
    execute(b"g_hide_spam on")
    frame(run)
    report(key(run, DIK_P, 1)[1] == [], "voice disabled: P sends nothing")
    execute(b"g_hide_spam off")
    execute(b"s_use_microphone off")
    frame(run)
    report(key(run, DIK_P, 1)[1] == [], "the microphone off: P sends nothing")
    execute(b"s_use_microphone on")
    frame(run)
    run.last_speaker = 0
    first = key(run, DIK_O, 1)[1]
    after_first = mutes()
    second = key(run, DIK_O, 1)[1]
    report(first == [("invoke", HUD_MOVIE, "root.srp_voice_notice", ["T:st_voice_muted", "Alice"])]
           and after_first == "000ca201|000a11ce|"
           and second == [("invoke", HUD_MOVIE, "root.srp_voice_notice", ["T:st_voice_unmuted", "Alice"])]
           and mutes() == "000ca201|",
           f"the mute key: Alice (the last speaker, id 000a11ce) muted with a notice, then unmuted; {first}")
    run.last_speaker = SELF
    report(key(run, DIK_O, 1)[1] == [] and mutes() == "000ca201|", "the player itself is never muted")
    run.last_speaker = -1
    report(key(run, DIK_O, 1)[1] == [], "no last speaker: nothing")
    report(frame(run)[2] == [("sv_mute", 1 << 3)] or frame(run)[2] == [],
           "the mutes changed: sv_mute again with Carol's slot")

    print("the Esc menu (activate_main_menu's call of game_options::activate, 0x1EDBDC)")
    options = GAME + 0x57F4

    def menu(r, state):
        regs, popped = r.site(MENU_SITE, MENU_BACK, [state], {UC_X86_REG_EAX: options})
        return regs, popped, r.calls
    a = menu(stock, WORLD)
    b = menu(run, WORLD)
    panel = [c for c in b[2] if c[0] == "invoke"]
    report(a[2] == [("activate", options, WORLD)] and b[2][0] == ("activate", options, WORLD)
           and a[0][UC_X86_REG_EAX] == b[0][UC_X86_REG_EAX] == 0x77 and a[1] == b[1]
           and panel == [("invoke", MENU_MOVIE, "root.srp_voice_panel",
                          ["T:st_voice_chat_volume_option", "T:st_voice_mute_all", 0, "Alice", False,
                           3, "Carol", True, 4, "Dave", False])],
           f"over the match: activate as before (its answer kept), then the panel: Alice, Carol (muted), Dave; no "
           f"enemy, not the player; {panel}")
    run.ids[4] = 0
    b = menu(run, WORLD)
    report([c for c in b[2] if c[0] == "invoke"][0][3][2:] == [0, "Alice", False, 3, "Carol", True],
           "a teammate whose voice id is not known yet is not listed")
    run.ids[4] = IDS[4]
    b = menu(run, GAME + 0x539C)
    report([c for c in b[2] if c[0] == "invoke"] == [("invoke", MENU_MOVIE, "root.srp_voice_panel",
                                                      ["T:st_voice_chat_volume_option", "T:st_voice_mute_all"])],
           "over the lobby: the panel empty")

    print("the menu's calls (game_options' callback slot 0x9FD47C)")
    report(stock.read(CALLBACK_SLOT) == CALLBACK and run.read(CALLBACK_SLOT) == linked.symbols["voice_menu_call"],
           "the slot names voice_menu_call (the original's: 0x1FF5D0)")

    def menu_call(r, name, values):
        arguments = TEXT + 0x2000
        for i, (kind, value) in enumerate(values):
            r.emu.mem_write(arguments + 24 * i, struct.pack("<II", 0, kind) + struct.pack("<d", value)
                            if kind == 5 else struct.pack("<IIII", 0, kind, value, 0) + bytes(8))
        regs, popped = r.call(r.read(CALLBACK_SLOT), [0x99, r.put_text(name), arguments, len(values)],
                              {UC_X86_REG_ECX: 0x4444})
        return popped, r.calls_of("callback"), all(regs[reg] == MARKS[reg] for reg in REGS[3:])
    out = menu_call(run, b"srp_voice_mute", [(4, 0), (2, 1)])
    report(out[:2] == (16, []) and out[2] and mutes() == "000ca201|000a11ce|",
           "srp_voice_mute(0, true): Alice's id kept; `ret 10h`, the game's callback not called")
    menu_call(run, b"srp_voice_mute", [(5, 3.0), (2, 0)])
    report(mutes() == "000a11ce|", "srp_voice_mute(3 as a Number, false): Carol unmuted")
    menu_call(run, b"srp_voice_mute", [(4, 2), (2, 1)])
    menu_call(run, b"srp_voice_mute", [(4, SELF), (2, 1)])
    report(mutes() == "000a11ce|", "the enemy and the player: refused")
    menu_call(run, b"srp_voice_mute_all", [(2, 1)])
    report(mutes() == "000a11ce|000ca201|0000da4e|", "srp_voice_mute_all(true): every teammate")
    menu_call(run, b"srp_voice_mute_all", [(2, 0)])
    report(mutes() == "", "srp_voice_mute_all(false): none")
    a = menu_call(stock, b"menu_button", [(6, 0)])
    b = menu_call(run, b"menu_button", [(6, 0)])
    given = lambda calls: [(c[1], c[2], c[4], c[5]) for c in calls]
    report(a[0] == b[0] == 16 and given(a[1]) == given(b[1]) == [(0x4444, 0x99, TEXT + 0x2000, 1)]
           and stock.text(a[1][0][3]) == run.text(b[1][0][3]) == b"menu_button",
           "another call (menu_button): the game's callback with the handler and the four arguments, as the "
           "original")

    print("the end (on_finish_match 0x1F96F0, on_deactivate 0x1F9520)")

    def finish(r):
        sp = STACK + 0x8800
        regs, esp = r.run(FINISH, (FINISH_BACK,), sp, {UC_X86_REG_EAX: WORLD})
        return r.stopped == FINISH_BACK and esp == sp - 4 and regs[UC_X86_REG_EDI] == WORLD \
            and regs[UC_X86_REG_EAX] == GAME and r.read(sp - 4) == MARKS[UC_X86_REG_EDI], r.calls

    def deactivate(r):
        sp = STACK + 0x8800
        regs, esp = r.run(DEACTIVATE, (DEACTIVATE_BACK,), sp, {UC_X86_REG_ECX: WORLD})
        return r.stopped == DEACTIVATE_BACK and esp == sp - 12 and regs[UC_X86_REG_EDI] == WORLD, r.calls
    a, b, c = finish(stock), finish(run), finish(run)
    report(a == (True, []) and b == (True, [("sv_ptt", 0), ("sv_stop",)]) and c == (True, [("sv_ptt", 0)]),
           "on_finish_match: push-to-talk released, sv_stop once (again only the release), back at 0x1F96F9 with "
           "edi the world, eax the game and edi pushed, as the original")
    connect(run)
    a, b, c = deactivate(stock), deactivate(run), deactivate(run)
    report(a == (True, []) and b == (True, [("sv_ptt", 0), ("sv_stop",)]) and c == (True, [("sv_ptt", 0)]),
           "on_deactivate: push-to-talk released, sv_stop once, back at 0x1F9525 with three pushes and edi the "
           "world")
    report(frame(run, 1)[2] == [] and key(run, DIK_P, 1)[1] == [],
           "stopped: no frame and no key reach the DLL")

    # The review's case: P held when the match ends, its release while stopped (lost), the next match.
    connect(run)
    frame(run, 0)
    run.held = {DIK_P}
    down = key(run, DIK_P, 1)[1]
    finished = finish(run)[1]
    up = key(run, DIK_P, 3)[1]
    connect(run, 2236, 0x2222)
    again = list(run.calls)
    run.held = set()
    later = frame(run, 0)[2] + frame(run, 0)[2]
    report(down == [("sv_ptt", 1)] and finished == [("sv_ptt", 0), ("sv_stop",)] and up == []
           and again == [("sv_ptt", 0), ("sv_start", "203.0.113.9", 2237, 0x2222)]
           and run.dll_ptt == 0 and not any(c[0] == "sv_ptt" and c[1] for c in later),
           f"P down, the match ends, P up while stopped (lost), the next match: the DLL's push-to-talk is "
           f"released at the end and again before the start, and nothing presses it; {finished} {again}")

    # A movie may appear after the first frame; retry an undelivered list.
    run.word(HUD_HOLDER + 0x108, 0)
    absent = frame(run, 1 << 0)[2]
    run.word(HUD_HOLDER + 0x108, HUD_UI)
    ready = frame(run, 1 << 0)[2]
    report(not any(c[0] == "invoke" for c in absent)
           and ("invoke", HUD_MOVIE, "root.srp_voice_talking", ["Alice", 0]) in ready,
           "HUD initially unavailable: unchanged talking list retried when the movie appears")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
