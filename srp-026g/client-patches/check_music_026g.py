#!/usr/bin/env python3
"""Run patch 88 (rust/patches/src/music.rs, its hooks of music.toml: the
music, docs/notes/music.md) in the default build on made-up game objects:
each of its seven lohooks from its site to the end of its span, and its query
callback through the game's own boost invoker.

    python check_music_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them. Run as the game: the boost function bind (0x176D0), its
invoker (0x3AAA20), type_to_int (0x630F0), the config lookups (0xF6B10,
0xF6C00) and add_scene_to_active (0x59FB0) are the game's own code. Stubs
that record their arguments: query_resources (0xE5B60), emit_hud_sound
(0x65630: a new proxy into its out pointer), play (0x5ABA0), the proxy's free_object
at its count's 0 (0x5B390) and the resource's release (0x11540); QueryPerformanceCounter /
-Frequency (10 MHz, the time the test sets) and GetCurrentThreadId (one
thread). Checked:

- each lohook in place (`jmp` to its entry in patches.code, the original's
  bytes), the callback entry in patches.code; every hook returns to its
  span's end with the registers and ESP as they were;
- the bus: asked at the first frame with a sound world (a class 38 request
  "sound_scene", its user data the creation params variant of the lobby's
  form: helper 0x9FC7C0, 8/16/2, its own address, the type's id), a boost
  function of 0x176D0's (vtable 0x9DB9E0 | 1) bound to the entry and tag 0;
  the scene taken (+1 reference) and then kept among the active scenes by
  the sound thread's tick (the world's list, +2F9h set, fade-in state 1 with
  an endless time, the general factor as add_scene_to_active gives it);
- a screen asked before the bus is open plays once it is: the main theme;
- a track: a class 256 request of its path, a tag of its own; its sound
  taken and played as a looping hud sound in the bus (emit_hud_sound's
  emitter, the bus's scene, the logic world user; play mode 1); the bus's
  volume 0 at the start, 0.5 a second in, 1 at two;
- a change of track (login -> lobby): the next asked only once the first is
  silent (its voice and sound let go then); a mode's `ui.loading_music`
  (a config table made here) is its theme; one without plays nothing;
- the announcement: the map's theme full until 15 s, then a 2 s fade-out, let
  go when silent, nothing asked after;
- a callback whose tag nobody waits for: its sound taken and let go at once;
- the unload: the voice and the sound let go at once;
- a bus the engine refuses (a null first result): logged, asked again once
  at the next screen, a second refusal logged and nothing asked after;
- the Music slider: `s_music_volume` registered at on_configs_loaded's start
  (after patches 79's and 83's), found by the console's lookup (0xEB7C0, the
  stock option's), a saved kind-2 cc_float of 0 to 100 at 100; the game's
  own setter (0xECB70) takes a line's value within its range; the bus gets
  fade x slider / 100 from the next frame.
"""
import pathlib
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

import patch_026g as base

SITES = {  # the site, its span, its handler
    "frame": (0x1EC8EC, 6), "login": (0x21EAC8, 6), "lobby": (0x21D31D, 6), "loading": (0x2204B3, 6),
    "announced": (0x23B36A, 7), "unload": (0x1ED030, 10), "sound": (0x59AB1, 6), "configs": (0x1EA210, 6),
    "replay": (0x1F0971, 5),
}
LIST_HEAD, FIND, SET_FLOAT, CC_FLOAT_VTABLE = 0x1F1D66C, 0xEB7C0, 0xECB70, 0x9E7C6C
SSCANF = 0x422B46                       # the CRT's sscanf (cdecl text, format, place): its "%f" made here
INVALID = 0xEC530                       # on_invalid_syntax (eax the variable; the text, ret 4): the console's message
QUERY, EMIT, PLAY, FREE_VOICE, RELEASE = 0xE5B60, 0x65630, 0x5ABA0, 0x5B390, 0x11540
INVOKER = 0x3AAA20                       # the bound cdecl function's invoker (vtable 0x9DB9E0 slot 1)
FUNCTION_VTABLE, PARAMS_HELPER = 0x9DB9E0, 0x9FC7C0
QPC_SLOT, QPF_SLOT, THREAD_SLOT = 0x870158, 0x870150, 0x870118
CREATE_FILE_SLOT, WRITE_FILE_SLOT, CLOSE_SLOT = 0x870048, 0x870108, 0x870040   # the log's kernel32
FREQUENCY = 10_000_000
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
GAME, WORLD, USER, SCENE, RESULTS, UI = (HEAP + 0x10000 * i for i in range(6))
SOUNDS = HEAP + 0x80000                 # the sound emitters made here, 0x1000 apart
PROXIES = HEAP + 0xC0000                # the proxies emit_hud_sound makes
TEXTS = HEAP + 0xF0000
DONE = STUBS + 0xF00                    # a return address that stops a run
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
STEADY = dict(zip(REGS, (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HEAP, STUBS, STACK - 0x100000):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.ticks = 0
        for slot, stub in ((QPC_SLOT, STUBS), (QPF_SLOT, STUBS + 0x10)):
            self.word(slot, stub)
            emu.mem_write(stub, b"\xC2\x04\x00")
        emu.hook_add(UC_HOOK_CODE, self.counter, begin=STUBS, end=STUBS)
        emu.hook_add(UC_HOOK_CODE, self.frequency, begin=STUBS + 0x10, end=STUBS + 0x10)
        self.word(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        # the log: CreateFileA (a handle, stdcall 7), WriteFile (the bytes kept, stdcall 5), CloseHandle (1)
        self.logged = b""
        for slot, stub, code in ((CREATE_FILE_SLOT, STUBS + 0x90, b"\xB8\x07\x00\x00\x00\xC2\x1C\x00"),
                                 (WRITE_FILE_SLOT, STUBS + 0xA0, b"\xB8\x01\x00\x00\x00\xC2\x14\x00"),
                                 (CLOSE_SLOT, STUBS + 0xB0, b"\xB8\x01\x00\x00\x00\xC2\x04\x00")):
            self.word(slot, stub)
            emu.mem_write(stub, code)
        emu.hook_add(UC_HOOK_CODE, self.write_file, begin=STUBS + 0xA0, end=STUBS + 0xA0)
        self.calls = []
        self.proxies = 0
        for stub in (QUERY, EMIT, PLAY, FREE_VOICE, RELEASE, SSCANF, INVALID):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)
        # the game: its sound world (+5350h) and the world's logic user (+0B0h);
        # the world's active list empty, its general volume byte 50
        self.word(GAME + 0x5350, WORLD)
        self.word(WORLD + 0xB0, USER)
        emu.mem_write(WORLD + 0x48F0, bytes([50]))
        # the scene the cook would make: no submix (no device), volume 0, inactive
        self.texts = TEXTS

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def float(self, at):
        return struct.unpack("<f", self.emu.mem_read(at, 4))[0]

    def text(self, at):
        return bytes(self.emu.mem_read(at, 96)).split(b"\0")[0].decode()

    def put_text(self, text):
        at = self.texts
        self.emu.mem_write(at, text.encode() + b"\0")
        self.texts += (len(text) + 16) & ~15
        return at

    def write_file(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.logged += self.read(self.read(sp + 8), self.read(sp + 12))

    def counter(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.mem_write(self.read(sp + 4), struct.pack("<Q", self.ticks))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def frequency(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.mem_write(self.read(sp + 4), struct.pack("<Q", FREQUENCY))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def stub(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 * i)
        reg = emu.reg_read
        pops = 0
        if address == QUERY:              # eax requests, ecx count; (&function, allocator, &user data, parent, flag)
            requests, count = reg(UC_X86_REG_EAX), reg(UC_X86_REG_ECX)
            function = self.read(arg(1), 0x20)
            user = self.read(arg(3))                        # the first request's user data: a variant or 0
            variant = self.read(user, 0x30) if user else None
            self.calls.append(("query", self.text(self.read(requests)), self.read(requests + 4), count, function,
                               arg(2), user, variant, arg(4), arg(5)))
        elif address == EMIT:             # esi emitter, eax &scene, ecx user, edi out: eax out
            out = reg(UC_X86_REG_EDI)
            self.proxies += 1
            proxy = PROXIES + 0x100 * self.proxies
            self.emu.mem_write(proxy, bytes(0x100))
            self.word(proxy + 0x28, 1)              # the out pointer's reference
            self.word(out, proxy)
            self.calls.append(("emit", reg(UC_X86_REG_ESI), self.read(reg(UC_X86_REG_EAX)), reg(UC_X86_REG_ECX),
                               proxy))
            emu.reg_write(UC_X86_REG_EAX, out)
        elif address == PLAY:             # thiscall (mode, producer, receiver), ret 0Ch
            self.calls.append(("play", reg(UC_X86_REG_ECX), arg(1), arg(2), arg(3)))
            pops = 12
        elif address == FREE_VOICE:       # thiscall, ecx the proxy whose count went to 0
            self.calls.append(("release voice", reg(UC_X86_REG_ECX), self.read(reg(UC_X86_REG_ECX) + 0x28)))
        elif address == INVALID:
            self.calls.append(("invalid", self.text(arg(1))))
            pops = 4
        elif address == SSCANF:           # cdecl (text, "%f", &float): 1 when the text is a number
            assert self.text(arg(2)) == "%f"
            try:
                number = float(self.text(arg(1)))
                emu.mem_write(arg(3), struct.pack("<f", number))
                emu.reg_write(UC_X86_REG_EAX, 1)
            except ValueError:
                emu.reg_write(UC_X86_REG_EAX, 0)
        else:                             # ecx &resource pointer
            self.calls.append(("release", self.read(reg(UC_X86_REG_ECX))))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def go(self, start, stop, regs, sp):
        emu = self.emu
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            emu.emu_start(start, stop, count=5_000_000)
        except Exception as error:
            sys.exit(f"FAIL: {start:#x}: {error} at {emu.reg_read(UC_X86_REG_EIP):#x}")
        return {reg: emu.reg_read(reg) for reg in REGS}, emu.reg_read(UC_X86_REG_ESP)

    def hook(self, name, regs):
        """One hook's site to its span's end; the calls it made. The registers
        and ESP after are the site's own (the span replayed: lea/mov)."""
        site, span = SITES[name]
        sp = STACK - 0x200
        given = {**STEADY, **regs}
        self.calls = []
        after, esp = self.go(site, site + span, given, sp)
        return self.calls, after, esp, given, sp

    def call(self, start, regs, args=()):
        """A function of the game run to its return (stdcall/thiscall/cdecl alike: DONE stops it)."""
        sp = STACK - 0x600
        self.emu.mem_write(sp, struct.pack("<I", DONE) + b"".join(struct.pack("<I", a) for a in args))
        after, _ = self.go(start, DONE, {**STEADY, **regs}, sp)
        return after

    def at(self, ms):
        self.ticks = ms * (FREQUENCY // 1000)

    def callback(self, function, results):
        """The query ends: the game's invoker with the function's buffer (+8) and the results."""
        sp = STACK - 0x400
        self.emu.mem_write(sp, struct.pack("<III", DONE, function, results))
        self.calls = []
        self.go(INVOKER, DONE, STEADY, sp)
        return self.calls


def replayed(name, after, given, esp, sp, game):
    """The registers after a hook: the span's own effect on them, nothing else."""
    expect = dict(given)
    if name == "frame":                     # lea esi,[ebx+5308h]
        expect[UC_X86_REG_ESI] = (given[UC_X86_REG_EBX] + 0x5308) & 0xFFFFFFFF
    elif name in ("login", "lobby"):        # mov ecx,[esi/edi+9Ch]
        base_reg = UC_X86_REG_ESI if name == "login" else UC_X86_REG_EDI
        expect[UC_X86_REG_ECX] = game.read(given[base_reg] + 0x9C)
    elif name == "loading":                 # mov eax,esi / mov [esp+0Ch],esi
        expect[UC_X86_REG_EAX] = given[UC_X86_REG_ESI]
    elif name == "unload":                  # push ebx / push esi / mov esi,ecx / mov ecx,[esi+535Ch]
        expect[UC_X86_REG_ESI] = given[UC_X86_REG_ECX]
        expect[UC_X86_REG_ECX] = game.read(given[UC_X86_REG_ECX] + 0x535C)
        return after == expect and esp == sp - 8
    elif name == "configs":                 # push ebp / mov ebp,esp / and esp,-8 (a chain: patches 79, 83, 88)
        expect[UC_X86_REG_EBP] = sp - 4
        return {r: v for r, v in after.items() if r != UC_X86_REG_EBP} == \
            {r: v for r, v in expect.items() if r != UC_X86_REG_EBP} and after[UC_X86_REG_EBP] == sp - 4 \
            and esp == (sp - 4) & ~7
    elif name == "sound":                   # mov esi,[edi+48E4h]
        expect[UC_X86_REG_ESI] = game.read(given[UC_X86_REG_EDI] + 0x48E4)
    return after == expect and esp == sp


def cfg_table(game, entries):
    """A loaded binary config table (records of 24 bytes: data, name, CRC, type, count; children sorted
    by CRC, as cfg_child's binary search needs): {name: text}."""
    kids = sorted(((zlib.crc32(k.encode()), k, v) for k, v in entries.items()))
    first = UI + 0x100
    for i, (crc, key, value) in enumerate(kids):
        record = first + 24 * i
        game.emu.mem_write(record, struct.pack("<QQIHH", game.put_text(value), game.put_text(key), crc, 5,
                                               len(value) + 1))
    game.emu.mem_write(UI, struct.pack("<QQIHH", first if kids else 0, 0, 0, 3, len(kids)))
    return UI


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    block = linked.blocks["patches.code"]
    reader = base.hooks_026g.original_reader(original)
    ours = [h for h in base.hooks_026g.hooks(exe=reader) if any(m["patch"] == "music" for m in h.get("chain", [h]))]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for h in ours:
        if h["kind"] == "callback":
            target = linked.symbols[h["entry"]]
            check(h["entry"] == "music_on_query_done" and block.va <= target < block.va + block.size
                  and h["site"] not in installs, f"the callback entry {h['entry']} in patches.code, no site rewritten")
            continue
        va, old, symbol, opcode = installs[h["site"]]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old and built[at:at + len(new)] == new
              and any(va == site for site, _ in SITES.values()), f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")
    check(len(ours) == 10, f"music's ten hooks ({len(ours)})")

    game = Game(bytes(built))
    clean = True

    def run(name, **named):
        """A hook run with registers by name (EDI=...)."""
        nonlocal clean
        regs = {getattr(sys.modules[__name__], "UC_X86_REG_" + k): v for k, v in named.items()}
        calls, after, esp, given, sp = game.hook(name, regs)
        clean &= replayed(name, after, given, esp, sp, game)
        return calls

    def sound(ms):
        """The sound thread's ticks of 10 ms for `ms`, the clock with them."""
        for _ in range(ms // 10):
            game.at(game.ticks // (FREQUENCY // 1000) + 10)
            run("sound", EDI=WORLD, EBX=10)

    def frames(ms):
        for _ in range(ms // 20):
            sound(20)
            run("frame", EBX=GAME)

    def results(resource):
        game.emu.mem_write(RESULTS, bytes(0x400))
        game.word(RESULTS + 0x38, 1)
        game.word(RESULTS + 0x12C, resource)
        return RESULTS

    def volume():
        return game.float(SCENE + 0x2E4)

    game.at(1000)
    # the Music slider's variable, registered at on_configs_loaded's start (after patches 79 and 83)
    head = game.read(LIST_HEAD)
    run("configs")
    name = game.put_text("s_music_volume")
    node = game.call(FIND, {UC_X86_REG_ESI: name})[UC_X86_REG_EAX]
    music_value = game.read(node + 0x40) if node else 0
    check(node and game.read(LIST_HEAD) == node and game.read(node) == CC_FLOAT_VTABLE
          and game.text(game.read(node + 0x10)) == "s_music_volume" and game.read(node + 0x14) == 2
          and game.read(node + 0x1C, 2)[1] == 1 and (game.float(node + 0x44), game.float(node + 0x48)) == (0.0, 100.0)
          and game.float(music_value) == 100.0 and head != node,
          "configs.start: s_music_volume registered last of the chain, found by the console's lookup (0xEB7C0, the "
          "Settings' Music option's): a cc_float, saved (user.cfg), kind 2, 0 to 100, 100 at first")
    run("configs")
    check(game.read(LIST_HEAD) == node, "configs.start again: registered once")

    def slider(text):
        game.call(SET_FLOAT, {UC_X86_REG_ECX: node}, (game.put_text(text),))
        return game.float(music_value)

    check((slider("40"), slider("150"), slider("abc"), slider("62.50"), slider("100")) == (40.0, 0.0, 0.0, 62.5, 100.0),
          "the game's own setter (0xECB70, user.cfg's line and the slider's Apply; sscanf's \"%f\" made here): 40; out of range or not a number "
          "the least; 62.50; 100")
    check(run("sound", EDI=WORLD, EBX=10) == [] and game.read(WORLD + 0x48E4) == 0,
          "the sound thread's tick before the bus: the world untouched")
    check(run("login", ESI=HEAP + 0x200) == [], "the login screen before the first frame: the theme waits")
    calls = run("frame", EBX=GAME)
    q = calls[0] if calls else None
    function = q[4] if q else bytes(0x20)
    vtable, f, value = struct.unpack_from("<I4xII", function)
    variant = q[7] if q else bytes(0x30)
    helper, proxies, propagators, receivers = struct.unpack_from("<I4xIII", variant)
    itself, type_id = struct.unpack_from("<II", variant, 0x28)
    check(len(calls) == 1 and q[:4] == ("query", "sound_scene", 38, 1) and vtable == FUNCTION_VTABLE | 1
          and f == linked.symbols["music_on_query_done"] and value == 0 and q[5] == game.read(0x1F1D9C4)
          and (helper, proxies, propagators, receivers) == (PARAMS_HELPER, 8, 16, 2) and itself == q[6]
          and type_id != 0 and q[8:] == (0, 1),
          "the first frame asks for the bus: sound_scene (class 38), the creation params' variant (8/16/2), "
          "a boost function of 0x176D0 bound to the entry and tag 0, the game's allocator")
    game.emu.mem_write(SCENE, bytes(0x400))
    game.word(SCENE + 0xD0, 1)
    bus_function = STUBS + 0x400
    game.emu.mem_write(bus_function, function)
    calls = game.callback(bus_function + 8, results(SCENE))
    main_q = calls[0] if calls else None
    check(game.read(SCENE + 0xD0) == 2 and len(calls) == 1 and main_q[1:4] == ("music/main_theme", 256, 1)
          and struct.unpack_from("<4xI", main_q[4])[0] == 0 and struct.unpack_from("<8xII", main_q[4])[1] == 2
          and main_q[6] == 0,
          "the bus arrives (its scene taken: one reference more); the waiting main theme asked: "
          "music/main_theme (class 256), tag 2, no user data")
    run("sound", EDI=WORLD, EBX=10)
    check(game.read(WORLD + 0x48E4) == SCENE and game.read(WORLD + 0x48DC) == 1 and game.read(SCENE + 0x2F9) & 0xFF == 1
          and game.read(SCENE + 0x2FC) == 1 and game.read(SCENE + 0x2EC) == 0xFFFFFFFF and volume() == 0.0
          and game.float(SCENE + 0x2E8) == 0.5,
          "the sound tick keeps the bus active: in the world's list, +2F9h set, fading in for ever, volume 0, "
          "the general factor 0.5 (add_scene_to_active's)")
    run("sound", EDI=WORLD, EBX=10)
    check(game.read(WORLD + 0x48DC) == 1, "the next tick adds it no more")

    def arrive(query, resource):
        at = STUBS + 0x500
        game.emu.mem_write(at, query[4])
        return game.callback(at + 8, results(resource))

    def sound_at(i):
        at = SOUNDS + 0x1000 * i
        game.emu.mem_write(at, bytes(0x100))
        game.word(at + 0xD0, 1)
        return at

    main = sound_at(1)
    calls = arrive(main_q, main)
    check(game.read(main + 0xD0) == 2 and [c[0] for c in calls] == ["emit", "play"]
          and calls[0][1:4] == (main, SCENE, USER) and calls[1][1:] == (calls[0][4], 1, 0, 0),
          "the main theme arrives: its sound taken, emitted as a hud sound in the bus with the logic user, "
          "played looped")
    main_voice = calls[0][4]
    sound(1000)
    v1 = volume()
    sound(1000)
    check(abs(v1 - 0.5) < 0.011 and volume() > 0.999, f"fading in: {v1:.3f} at 1 s, {volume():.4f} at 2 s")

    slider("40")
    frames(100)
    at_40 = volume()
    slider("0")
    frames(100)
    at_0 = volume()
    slider("100")
    frames(100)
    check(abs(at_40 - 0.4) < 1e-6 and at_0 == 0.0 and volume() == 1.0,
          f"the Music slider: the bus at fade x slider (40: {at_40:.3f}, 0: {at_0}, 100: {volume()}) from the next frame "
          "(General is the engine's, +2E8h)")

    calls = run("lobby", EDI=HEAP + 0x300)
    check(calls == [], "the lobby: nothing asked while the main theme fades out")
    frames(1000)
    check(abs(volume() - 0.5) < 0.02 and not any(c[0] == "query" for c in game.calls),
          f"half way out a second later ({volume():.3f})")
    asked = []
    for _ in range(60):
        sound(20)
        calls = run("frame", EBX=GAME)
        if calls:
            asked = calls
            break
    check([c[0] for c in asked] == ["release voice", "release", "query"] and asked[0][1:] == (main_voice, 0)
          and asked[1][1] == main and asked[2][1:4] == ("music/lobby_theme", 256, 1) and volume() == 0.0,
          "silent: the main theme's voice and sound let go, only then the lobby theme asked")
    lobby_q = asked[2]
    lobby = sound_at(2)
    calls = arrive(lobby_q, lobby)
    check([c[0] for c in calls] == ["emit", "play"], "the lobby theme plays")
    sound(2000)

    level = cfg_table(game, {"loading_music": "music/level_04_loading", "mode_name": "st_mode_02_name"})
    check(run("loading", ESI=level) == [], "a loading: the lobby theme fades out first")
    asked = []
    for _ in range(150):
        sound(20)
        calls = run("frame", EBX=GAME)
        if calls:
            asked = calls
            break
    check(asked and asked[-1][1:4] == ("music/level_04_loading", 256, 1), "then the mode's ui.loading_music asked")
    map_sound = sound_at(3)
    calls = arrive(asked[-1], map_sound)
    check([c[0] for c in calls] == ["emit", "play"], "the map's theme plays")
    frames(30_000)
    check(volume() == 1.0, "through the loading and the countdown")
    run("announced", EDI=HEAP + 0x400)
    frames(14_800)
    full = volume()
    frames(1200)
    fading = volume()
    released = []
    for _ in range(150):
        sound(20)
        calls = run("frame", EBX=GAME)
        if calls:
            released = calls
            break
    check(full == 1.0 and 0.0 < fading < 1.0 and [c[0] for c in released] == ["release voice", "release"],
          f"announced: full at 14.8 s ({full}), fading at 16 s ({fading:.2f}), let go when silent, nothing asked")

    quiet = cfg_table(game, {"mode_name": "st_mode_02_name"})
    check(run("loading", ESI=quiet) == [] and all(run("frame", EBX=GAME) == [] for _ in range(5)),
          "a mode without loading_music: nothing")
    calls = run("lobby", EDI=HEAP + 0x300)
    stale = sound_at(4)
    late = arrive(lobby_q, stale)
    check(late == [("release", stale)] and game.read(stale + 0xD0) == 2,
          "a sound nobody waits for (an old tag): taken and let go at once")
    lobby2 = sound_at(5)
    arrive(calls[0], lobby2)
    calls = run("unload", ECX=GAME)
    check([c[0] for c in calls] == ["release voice", "release"] and calls[1][1] == lobby2,
          "the unload: the voice and the sound let go at once")
    check(clean, "every hook returned at its span's end, the registers and ESP the span's own")

    # a bus the engine refuses: logged, asked again at the next screen, then given up (logged too)
    refused = Game(bytes(built))
    refused.at(1000)

    def hook(name, **named):
        regs = {getattr(sys.modules[__name__], "UC_X86_REG_" + k): v for k, v in named.items()}
        return refused.hook(name, regs)[0]

    first = hook("frame", EBX=GAME)
    function = STUBS + 0x600
    refused.emu.mem_write(function, first[0][4])
    refused.emu.mem_write(RESULTS, bytes(0x400))
    refused.word(RESULTS + 0x38, 1)
    calls = refused.callback(function + 8, RESULTS)
    log_first = refused.logged
    quiet = [hook("frame", EBX=GAME) for _ in range(3)]
    retry = hook("login", ESI=HEAP + 0x200)
    refused.emu.mem_write(function, retry[0][4] if retry else bytes(0x20))
    refused.callback(function + 8, RESULTS)
    log_both = refused.logged
    after = hook("lobby", EDI=HEAP + 0x300) + hook("frame", EBX=GAME)
    check(calls == [] and b"did not load; asked again at the next screen" in log_first and quiet == [[], [], []]
          and retry and retry[0][1:3] == ("sound_scene", 38) and len(retry) == 1
          and b"did not load again; no music this session" in log_both and after == [],
          "a bus the engine refuses: logged to srp_patches.log, no query until the next screen (the login), "
          "asked again then, refused again: logged, and no music asked after")
    tabs = (pathlib.Path(__file__).resolve().parent / ".." / "flash" / "main_menu" / "src" / "main_menu_fla"
            / "settingsTabs_15.as").read_text(encoding="utf-8")
    sound_tab = tabs[tabs.index("internal function frame4()"):]
    check("this._sound_volume." not in sound_tab.split("}")[0] and "this._game_volume.slider.enabled = false;" in sound_tab,
          "the main menu's sound tab leaves the Music slider (_sound_volume) enabled, In-game and Voice chat greyed out")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
