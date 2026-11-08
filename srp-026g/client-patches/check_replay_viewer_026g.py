#!/usr/bin/env python3
"""Run patch 130 (the Rust patch `replay_viewer`: a replay played in the
running game as a spectated match) in the default build beside the original,
at its three sites of its own and its key handler, on made-up objects.

    python check_replay_viewer_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are. A
network client (vtable network_client's 0x9FFB14, its game +14h, pvp_match
+1F1C40h with the world core's time, local place +1F1C44h, match link
+1F1C48h with its packet pool, link state +1F1C64h), a game (its game world
at +18h, +18h+9Ch the game again, +53ACh the client), the game world's input
handler and hud (its hud and player icons movies), the platform input world
and keyboard, a packet and an 0xF6's reader are made up. The game's functions
the patch calls are stand-ins that keep what they were given: game::pause
and game::resume (0x1ED5C0 / 0x1ED650, the paused flag 0x1F1DA3C set and
cleared as theirs), new_packet (0x279FE0), buffer_writer::w (0x7AC10),
enqueue (0x27A2A0), close_current_match (0x27A790), Movie::Invoke (0x511600),
show_cursor (0x23A930) and text_translator::translate_text (0x1FF0F0, the key
itself as the text).

- The sites' bytes in the original and the build's jumps to the arena.
- A player's game and a spectator before any playback note run the game's
  code: the 0xF6 with the upload request's payload `00` (0x27A4F4), a key
  (0x25CD00: Space, Esc, H), a match's end (0x27A7F3): at the instruction
  after the span with every register, the flags and the made-up objects as
  the original has them there, and nothing called.
- A spectator's game through one replay, the patch's state kept between the
  runs (its arena's): the first playback note (paused, 0.5x) pauses the game
  and sets time_scale 0.5, then 0x27A5AB (the payload read to its end); an
  older one changes nothing; a newer one (running, 2x) resumes it at 2; an
  events note and a malformed one of ours are taken with nothing called;
  Space, Right, Shift+Right, `]`, `[` each send one 0xE0 alone in its
  datagram with the request's 14 bytes; H hides the hud and the icons
  (root.hide_interface true), H again shows them; M is taken with nothing
  sent (the Moments switch is the patch's own); Esc is
  close_current_match(2); a key held is taken with nothing done; a match's
  end (reason 4, a kick) resumes a paused game and sets time_scale 1, after
  which Space goes to the game again.
- The replay controls: a player's and a spectator's world both query and take
  replay_player.swf (a game queries its movies at its first world only); a
  hud with them gives a button (0x25CFF3) and the pointer (0x25D1E6) to its
  hud movie in a player's and a live spectator's game, to the controls (as the
  original) in a viewer's, and to the hud movie again after its match's end.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP,
                               UC_X86_REG_EFLAGS)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

NC_VTABLE, INPUT_WORLD_VTABLE, KEYBOARD_VTABLE = 0x9FFB14, 0xA252D4, 0xA25478
PAUSE, RESUME, NEW_PACKET, WRITE, ENQUEUE, CLOSE, INVOKE, SHOW_CURSOR = (
    0x1ED5C0, 0x1ED650, 0x279FE0, 0x7AC10, 0x27A2A0, 0x27A790, 0x511600, 0x23A930)
PAUSED_FLAG, TIME_SCALE = 0x1F1DA3C, 0xA70298
THREAD_SLOT = 0x870118
NC, GAME, OBJECTS, STUBS, STACK = 0x40000000, 0x48000000, 0x50000000, 0x51000000, 0x52000000
LINK, PVP, POOL = OBJECTS, OBJECTS + 0x10000, OBJECTS + 0x5A000
INPUT = OBJECTS + 0x60000                  # the game world's input handler; its +4 the hud
UI = INPUT + 4
HUD_HOLDER, HUD_VIEW, HUD_MOVIE = OBJECTS + 0x61000, OBJECTS + 0x62000, OBJECTS + 0x63000
ICONS_HOLDER, ICONS_VIEW, ICONS_MOVIE = OBJECTS + 0x64000, OBJECTS + 0x65000, OBJECTS + 0x66000
INPUT_WORLD, KEYBOARD = OBJECTS + 0x67000, OBJECTS + 0x68000
READER, PAYLOAD = OBJECTS + 0x69000, OBJECTS + 0x6A000
PACKET = OBJECTS + 0x70000
STACK_SIZE = 0x40000                       # patch 76's after_move entry probes 0x9000 below it
SP = STACK + 0x30000
FRAME = STACK + 0x38000                     # close_current_match's EBP
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
# site: (start, bytes, the instruction after the span, the patch's exit)
SITES = {
    "note": (0x27A4F4, bytes.fromhex("83be641c1f0002"), 0x27A4FB, 0x27A5AB),
    "key": (0x25CD00, bytes.fromhex("558bec83e4f883ec0c"), 0x25CD09, 0x25CF15),
    "close": (0x27A7F3, bytes.fromhex("8b5d0883fb05"), 0x27A7F9, None),
    "query": (0x1F8F9E, bytes.fromhex("39b9c8530000"), 0x1F8FA4, 0x1F8FA6),
    "take": (0x1F88CC, bytes.fromhex("3988c8530000"), 0x1F88D2, 0x1F88D4),
    "button": (0x25CFF3, bytes.fromhex("80bee002000000"), 0x25CFFA, 0x25D066),
    "pointer": (0x25D1E6, bytes.fromhex("8bb008010000"), 0x25D1EC, 0x25D1EE),
    "call": (0x233D30, bytes.fromhex("53568b742410"), 0x233D36, 0x233D73),
    "frame": (0x23B200, bytes.fromhex("53555657 8bf8".replace(" ", "")), 0x23B206, None),
    "raw": (0x27C10A, bytes.fromhex("81c1f8010000"), 0x27C110, None),
}
SHOW_CONTROLS, DETACH, HANDLE_EVENT = 0x23A9D0, 0x242CC0, 0x513580
CURSOR_HOLDER, CURSOR_VIEW, CURSOR_MOVIE = OBJECTS + 0x71000, OBJECTS + 0x72000, OBJECTS + 0x73000
MOUSE_BUTTONS = 0x1204                      # MovieImpl: mouse 0's buttons held
WORLD_UI = GAME + 0x18 + 0x1FC              # the game world's hud (game_world+1FCh)
REPLAY_HOLDER, REPLAY_VIEW, REPLAY_MOVIE = OBJECTS + 0x6C000, OBJECTS + 0x6D000, OBJECTS + 0x6E000
CALL_NAME, CALL_ARGS = OBJECTS + 0x6F000, OBJECTS + 0x6F100
SPACE, ESC, H, F11, LEFT, RIGHT, LBRACKET, RBRACKET, LSHIFT, M = 0x39, 0x01, 0x23, 0x57, 0xCB, 0xCD, 0x1A, 0x1B, 0x2A, 0x32
TRANSLATE = 0x1FF0F0
DOWN, HELD = 1, 3
CLOCK = 100_000                            # the world core's time in the made-up match

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
        emu.mem_map(NC, 0x200000, UC_PROT_ALL)
        emu.mem_map(GAME, 0x10000, UC_PROT_ALL)
        emu.mem_map(OBJECTS, 0x80000, UC_PROT_ALL)
        emu.mem_map(STUBS, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK, STACK_SIZE, UC_PROT_ALL)
        self.word(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        self.calls = []
        for va, how in ((PAUSE, self.pause), (RESUME, self.resume), (NEW_PACKET, self.new_packet),
                        (WRITE, self.write), (ENQUEUE, self.enqueue), (CLOSE, self.close),
                        (INVOKE, self.invoke), (SHOW_CURSOR, self.show_cursor),
                        (SHOW_CONTROLS, self.show_controls), (DETACH, self.detach), (TRANSLATE, self.translate),
                        (HANDLE_EVENT, self.handle_event)):
            emu.hook_add(UC_HOOK_CODE, how, begin=va, end=va)

    def word(self, at, value, size=4):
        self.emu.mem_write(at, (value & (1 << 8 * size) - 1).to_bytes(size, "little"))

    def read(self, at, size=4):
        return int.from_bytes(self.emu.mem_read(at, size), "little")

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0].decode()

    def time_scale(self):
        return struct.unpack("<f", self.emu.mem_read(TIME_SCALE, 4))[0]

    def ret(self, pop):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def arg(self, n):
        return self.read(self.emu.reg_read(UC_X86_REG_ESP) + 4 * n)

    def pause(self, emu, *_):
        self.calls.append(("pause", emu.reg_read(UC_X86_REG_ECX)))
        self.word(PAUSED_FLAG, 1)
        self.ret(0)

    def resume(self, emu, *_):
        self.calls.append(("resume", emu.reg_read(UC_X86_REG_ECX)))
        self.word(PAUSED_FLAG, 0)
        self.ret(0)

    def new_packet(self, emu, *_):
        self.calls.append(("new_packet", emu.reg_read(UC_X86_REG_ECX), self.arg(1)))
        emu.reg_write(UC_X86_REG_EAX, PACKET)
        self.ret(4)

    def write(self, emu, *_):
        n, writer, source = emu.reg_read(UC_X86_REG_EDI), self.arg(1), self.arg(2)
        self.calls.append(("write", writer, bytes(emu.mem_read(source, n))))
        self.ret(8)

    def enqueue(self, emu, *_):
        self.calls.append(("enqueue", emu.reg_read(UC_X86_REG_ECX), self.arg(1), self.read(PACKET + 0x98, 1)))
        self.ret(4)

    def close(self, emu, *_):
        self.calls.append(("close_current_match", emu.reg_read(UC_X86_REG_ECX), self.arg(1)))
        self.ret(4)

    def invoke(self, emu, *_):
        args, count = self.arg(3), self.arg(4)
        value = (self.read(args + 4), self.read(args + 8)) if count else None   # a GFx value: +4 its type, +8 a bool
        if value and value[0] & 0x3F == 6:
            value = (6, self.text(value[1]))
        self.calls.append(("invoke", emu.reg_read(UC_X86_REG_ECX), self.text(self.arg(1)), count, value))
        emu.reg_write(UC_X86_REG_EAX, 1)
        self.ret(16)

    def show_controls(self, emu, *_):
        self.calls.append(("show_replay_controls", emu.reg_read(UC_X86_REG_EDI), self.arg(1)))
        self.ret(4)

    def handle_event(self, emu, *_):
        kind, modifiers, x, y, scroll, button, index = struct.unpack("<IIfffII", emu.mem_read(self.arg(1), 0x1C))
        self.calls.append(("handle_event", emu.reg_read(UC_X86_REG_ECX), kind, x, y, button, index))
        emu.reg_write(UC_X86_REG_EAX, 3)
        self.ret(4)

    def detach(self, emu, *_):
        self.calls.append(("detach_from_player", emu.reg_read(UC_X86_REG_ECX)))
        self.word(emu.reg_read(UC_X86_REG_ECX) + 4, 0)
        self.ret(0)

    def translate(self, emu, *_):
        key, out = self.text(self.arg(1)), self.arg(2)
        self.calls.append(("translate", key))
        emu.mem_write(out, key.encode() + b"\0")
        self.ret(8)

    def show_cursor(self, emu, *_):
        self.calls.append(("show_cursor",))
        self.ret(4)

    def setup(self, local):
        emu = self.emu
        emu.mem_write(NC, bytes(0x200000))
        emu.mem_write(GAME, bytes(0x10000))
        emu.mem_write(OBJECTS, bytes(0x80000))
        self.word(NC, NC_VTABLE)
        self.word(NC + 0x14, GAME)
        self.word(NC + 0x1F1C40, PVP)
        self.word(NC + 0x1F1C44, local, 1)
        self.word(NC + 0x1F1C48, LINK)
        self.word(LINK, 0xA08864)                  # the match link's vtable: +18h new_packet, +1Ch enqueue
        self.word(NC + 0x1F1C64, 2)
        self.word(LINK + 0xC + 0x577B5, 4, 1)
        self.word(LINK + 0x578B0, POOL)
        self.word(POOL + 0x30, 40)
        self.word(POOL + 0x34, 256)
        self.word(PVP + 0x1F8 + 0x18FC0, CLOCK)
        self.word(GAME + 0x18 + 0x9C, GAME)
        self.word(GAME + 0x53AC, NC)
        self.word(UI + 0x18, GAME + 0x18)
        self.word(UI + 0x8, HUD_HOLDER)
        self.word(UI + 0x10, ICONS_HOLDER)
        for holder, view, movie in ((HUD_HOLDER, HUD_VIEW, HUD_MOVIE), (ICONS_HOLDER, ICONS_VIEW, ICONS_MOVIE)):
            self.word(holder + 0x108, view)
            self.word(view + 4, movie)
        self.word(INPUT_WORLD, INPUT_WORLD_VTABLE)
        self.word(INPUT_WORLD + 0x24, KEYBOARD)
        self.word(KEYBOARD, KEYBOARD_VTABLE)
        self.word(PACKET + 0x98, 0x60, 1)
        self.word(PAUSED_FLAG, 0)
        if local != 0xFF:
            self.word(LINK + 0xC + local * 0x45FC + 0x45F0, 1, 1)     # the options' local place (is_local)
        self.word(WORLD_UI + 0x14, REPLAY_HOLDER)
        self.word(WORLD_UI + 0x18, GAME + 0x18)                      # the hud's game world
        self.word(REPLAY_HOLDER + 0x108, REPLAY_VIEW)
        self.word(REPLAY_VIEW + 4, REPLAY_MOVIE)
        self.word(WORLD_UI + 0xC, CURSOR_HOLDER)
        self.word(CURSOR_HOLDER + 0x108, CURSOR_VIEW)
        self.word(CURSOR_VIEW + 4, CURSOR_MOVIE)
        self.calls = []

    def run(self, site, regs, stack=b""):
        start, _, past, exit_ = SITES[site]
        emu = self.emu
        emu.mem_write(STACK, bytes(STACK_SIZE))
        emu.mem_write(SP, stack)
        regs = dict(regs)
        reason = regs.pop("reason", None)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        if reason is not None:
            emu.mem_write(FRAME, struct.pack("<3I", 0, STUBS + 0xF00, reason))
            emu.reg_write(UC_X86_REG_EBP, FRAME)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        stops = {past, exit_}
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop() if a in stops else None)
        emu.emu_start(start, 0, count=2000000)
        emu.hook_del(hook)
        state = {r: emu.reg_read(r) for r in REGS + (UC_X86_REG_ESP, UC_X86_REG_EIP)}
        state["flags"] = emu.reg_read(UC_X86_REG_EFLAGS) & 0x8D5
        objects = (bytes(emu.mem_read(NC, 0x20)), bytes(emu.mem_read(NC + 0x1F1C40, 0x30)),
                   bytes(emu.mem_read(GAME, 0x10000)), bytes(emu.mem_read(OBJECTS, 0x80000)),
                   bytes(emu.mem_read(SP, 0x400)), self.read(PAUSED_FLAG), self.time_scale())
        calls, self.calls = self.calls, []
        return state, objects, calls


STEADY = dict(zip(REGS, (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))


def note_regs(game, payload):
    game.emu.mem_write(PAYLOAD, payload)
    game.word(READER, PAYLOAD)
    game.word(READER + 4, PAYLOAD)
    game.word(READER + 8, len(payload))
    regs = dict(STEADY)
    regs[UC_X86_REG_ESI] = NC
    regs[UC_X86_REG_EBX] = READER
    return regs


def key(game, dik, kind=DOWN, shift=False):
    game.word(KEYBOARD + 4 + 4 * LSHIFT, 0x80 if shift else 0)
    regs = dict(STEADY)
    regs[UC_X86_REG_ECX] = INPUT
    return game.run("key", regs, struct.pack("<4I", STUBS + 0xF00, INPUT_WORLD, dik, kind))


def close(game, reason):
    regs = dict(STEADY)
    regs[UC_X86_REG_EDI] = NC
    regs["reason"] = reason
    return game.run("close", regs)


def load(game, site):
    regs = dict(STEADY)
    regs[UC_X86_REG_ECX if site == "query" else UC_X86_REG_EAX] = GAME
    if site == "take":
        regs[UC_X86_REG_ECX] = 0
    return game.run(site, regs)


def mouse(game, site):
    """The game world's mouse at a hud with the replay controls: a button (ESI its input
    handler) or the pointer (EDI the game world)."""
    regs = dict(STEADY)
    regs[UC_X86_REG_ESI if site == "button" else UC_X86_REG_EDI] = INPUT if site == "button" else GAME + 0x18
    return game.run(site, regs)


def control(game, method, *args):
    """The replay controls' ExternalInterface call: (type, low, high) GFx values."""
    game.emu.mem_write(CALL_NAME, method.encode() + b"\0")
    game.emu.mem_write(CALL_ARGS, b"".join(struct.pack("<6I", 0, kind, low, high, 0, 0) for kind, low, high in args))
    regs = dict(STEADY)
    regs[UC_X86_REG_ECX] = UI
    return game.run("call", regs, struct.pack("<5I", STUBS + 0xF00, REPLAY_MOVIE, CALL_NAME, CALL_ARGS, len(args)))


def frame(game):
    regs = dict(STEADY)
    regs[UC_X86_REG_EAX] = WORLD_UI
    return game.run("frame", regs, bytes(0x20))


def world_load(game, site):
    """A world's load with its arguments pushed: the mode slot after it ran."""
    regs = dict(STEADY)
    regs[UC_X86_REG_EDI] = NC
    regs[UC_X86_REG_EBP] = FRAME
    state, _, calls = game.run(site, regs, struct.pack("<3I", READER, 1, 8))
    mode = game.read(SP + 4)
    return state, mode, calls


def playback(revision, paused, percent, start=10_000, end=600_000):
    return struct.pack("<4BH2I", 0x52, 1, revision, paused, percent, start, end)


def request(seq, op, value):
    return b"\xff" * 6 + struct.pack("<4BI", 0x52, 0, seq, op, value)   # replay_control::REQUEST_HEAD, seq, op, value


def events(first, total, items):
    return struct.pack("<2B2H", 0x52, 2, first, total) + b"".join(
        struct.pack("<I3B", t, kind, a, v) for t, kind, a, v in items)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    block = linked.blocks["patches.code"]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for site, (start, span, _, _) in SITES.items():
        va, old, symbol, opcode = installs[start]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and old == span and original[at:at + len(span)] == span
              and built[at:at + len(new)] == new and block.va <= linked.symbols[symbol] < block.va + block.size,
              f"{site}: {va:#x} {old.hex()} in the original, `jmp {symbol}` (patches.code) here")
    at = base.file_offset(0x27A5AB)
    check(original[at:at + 13] == bytes.fromhex("8b530803138953 04e944010000".replace(" ", "")),
          "0x27A5AB is the 0xF6 case's end: the reader to its end, then the common exit 0x27A6FC")
    at = TIME_SCALE - 0xA5E000 + 0xA4C400          # .data: VA 0xA5E000, file 0xA4C400
    check(struct.unpack("<f", original[at:at + 4])[0] == 1.0,
          "time_scale (0xA70298) is 1.0 in the image")

    stock, ours = Game(original), Game(built)

    # The game's own code where the patch has nothing to do.
    def both(case, local, run):
        stock.setup(local)
        ours.setup(local)
        s, o = run(stock), run(ours)
        check(s == o and not o[2] and s[0][UC_X86_REG_EIP] in {SITES[k][2] for k in SITES},
              f"{case}: past the span as the original, registers, flags and objects the same, nothing called")

    for local, who in ((2, "a player"), (0xFF, "a spectator before any playback note")):
        both(f"note, {who}, the upload request (00)", local, lambda g: g.run("note", note_regs(g, b"\x00")))
        for dik, name in ((SPACE, "Space"), (ESC, "Esc"), (H, "H"), (RIGHT, "Right")):
            both(f"key, {who}, {name}", local, lambda g, dik=dik: key(g, dik))
        both(f"close, {who}, reason 0", local, lambda g: close(g, 0))

    # A player's game given a playback note (never sent to one): taken, nothing done, its keys its own.
    stock.setup(2)
    ours.setup(2)
    state, _, calls = ours.run("note", note_regs(ours, playback(1, 1, 50)))
    check(state[UC_X86_REG_EIP] == SITES["note"][3] and not calls and ours.time_scale() == 1.0,
          "note, a player, a playback note: taken (0x27A5AB), nothing called, time_scale 1")
    note_regs(stock, playback(1, 1, 50))           # the same bytes in the stock's objects
    s, o = key(stock, SPACE), key(ours, SPACE)
    check(s == o and not o[2], "key, that player, Space: as the original")

    # A spectator's game through one replay.
    ours.setup(0xFF)
    exit_ = SITES["note"][3]
    state, _, calls = ours.run("note", note_regs(ours, playback(5, 1, 50)))
    check(state[UC_X86_REG_EIP] == exit_ and calls == [("pause", GAME)] and ours.time_scale() == 0.5,
          "note, the first playback (paused, 0.5x): game::pause, time_scale 0.5, then 0x27A5AB")
    state, _, calls = ours.run("note", note_regs(ours, playback(4, 0, 800)))
    check(state[UC_X86_REG_EIP] == exit_ and not calls and ours.time_scale() == 0.5 and ours.read(PAUSED_FLAG) == 1,
          "note, an older playback (revision 4 after 5): nothing")
    state, _, calls = ours.run("note", note_regs(ours, playback(6, 0, 200)))
    check(state[UC_X86_REG_EIP] == exit_ and calls == [("resume", GAME)] and ours.time_scale() == 2.0,
          "note, a newer playback (running, 2x): game::resume, time_scale 2")
    state, _, calls = ours.run("note", note_regs(ours, events(0, 3, [(30_000, 1, 1, 2), (104_500, 1, 2, 3),
                                                                    (150_000, 1, 3, 0xFF)])))
    check(state[UC_X86_REG_EIP] == exit_ and not calls, "note, the events (three kills): taken, nothing called")
    state, _, calls = ours.run("note", note_regs(ours, b"\x52\x03\x01"))
    check(state[UC_X86_REG_EIP] == exit_ and not calls,
          "note, a view of place 1 (no player there in the made-up match): taken, nothing called")
    for bad in (b"\x52", b"\x52\x01\x07", b"\x52\x09" + bytes(12)):
        state, _, calls = ours.run("note", note_regs(ours, bad))
        check(state[UC_X86_REG_EIP] == exit_ and not calls and ours.time_scale() == 2.0,
              f"note, a malformed one of ours ({bad.hex()}): taken, nothing done")

    def sent(calls, payload):
        return calls == [("new_packet", LINK, 0xE0), ("write", PACKET + 0x2A8, payload),
                         ("enqueue", LINK, PACKET, 0x20)]

    taken = SITES["key"][3]
    for case, dik, shift, payload in (
            ("Space: pause", SPACE, False, request(0, 1, 1)),
            ("Right: 10 s on", RIGHT, False, request(1, 3, CLOCK + 10_000)),
            ("Left: 10 s back", LEFT, False, request(2, 3, CLOCK - 10_000)),
            ("Shift+Right: the next kill, 4 s before it", RIGHT, True, request(3, 3, 146_000)),
            ("Shift+Left: the previous kill, 4 s before it (the one 0.5 s ahead is the one being watched)",
             LEFT, True, request(4, 3, 26_000)),
            ("]: faster (4x)", RBRACKET, False, request(5, 2, 400)),
            ("[: slower (back to 2x)", LBRACKET, False, request(6, 2, 200))):
        state, _, calls = key(ours, dik, shift=shift)
        check(state[UC_X86_REG_EIP] == taken and state[UC_X86_REG_EAX] & 0xFF == 1 and sent(calls, payload),
              f"key, a viewer, {case}: one 0xE0 alone in its datagram ({payload.hex()}), the key taken")
    state, _, calls = key(ours, H)
    check(state[UC_X86_REG_EIP] == taken and calls == [("invoke", HUD_MOVIE, "root.hide_interface", 1, (2, 1)),
                                                        ("invoke", ICONS_MOVIE, "root.hide_interface", 1, (2, 1))],
          f"key, a viewer, H: root.hide_interface(true) to the hud and the icons, the key taken")
    state, _, calls = key(ours, F11)
    check(state[UC_X86_REG_EIP] == taken and [c[4] for c in calls] == [(2, 0), (2, 0)],
          "key, a viewer, F11: root.hide_interface(false) to both, the key taken")
    state, _, calls = key(ours, M)
    check(state[UC_X86_REG_EIP] == taken and not calls, "key, a viewer, M: the Moments switch's next choice, taken, nothing sent")
    state, _, calls = key(ours, SPACE, kind=HELD)
    check(state[UC_X86_REG_EIP] == taken and not calls, "key, a viewer, Space held: taken, nothing done")
    state, _, calls = key(ours, 0x10)
    check(state[UC_X86_REG_EIP] == SITES["key"][2] and not calls, "key, a viewer, Q: on to the game")
    ours.word(POOL + 0x30, 240)
    state, _, calls = key(ours, SPACE)
    check(state[UC_X86_REG_EIP] == taken and not calls, "key, a viewer, Space with the packet pool near empty: nothing sent")
    ours.word(POOL + 0x30, 40)
    state, _, calls = key(ours, ESC)
    check(state[UC_X86_REG_EIP] == taken and calls == [("close_current_match", NC, 2)],
          "key, a viewer, Esc: close_current_match(2) (the game's leave), the key taken")

    ours.run("note", note_regs(ours, playback(7, 1, 100)))
    state, _, calls = close(ours, 4)
    check(state[UC_X86_REG_EIP] == SITES["close"][2] and state[UC_X86_REG_EDI] == NC
          and [c[2] if c[0] == "invoke" else c[0] for c in calls] == ["resume", "root.set_cursor_shown", "show_replay_controls"]
          and ours.time_scale() == 1.0,
          "close, a viewer, reason 4 (a kick, reason 1's end since patch 119): game::resume, the controls hidden, time_scale 1, "
          "on as the original")

    def released(calls):
        return [c[1:] for c in calls if c[0] == "handle_event"]

    def up(movie, button):
        return (movie, 3, -1.0, -1.0, button, 0)

    def ended_with(controls, cursor):
        ours.run("note", note_regs(ours, playback(8, 1, 100)))
        ours.word(REPLAY_MOVIE + MOUSE_BUTTONS, controls)
        ours.word(CURSOR_MOVIE + MOUSE_BUTTONS, cursor)
        return released(close(ours, 0)[2])

    check(ended_with(1, 1) == [up(REPLAY_MOVIE, 0), up(CURSOR_MOVIE, 0)],
          "close, a viewer, the left button held by both movies: each let go once, off the stage")
    check(not released(close(ours, 0)[2]), "close again, no viewer any more: nothing let go")
    check(ended_with(0, 0) == [], "close, a viewer, no button held: nothing sent to the movies")
    check(ended_with(1, 0) == [up(REPLAY_MOVIE, 0)], "close, a viewer, only the controls hold the left button: only they let go")
    check(ended_with(0b101, 2) == [up(REPLAY_MOVIE, 0), up(REPLAY_MOVIE, 2), up(CURSOR_MOVIE, 1)],
          "close, a viewer, several buttons held: each one let go")
    ours.word(CURSOR_HOLDER + 0x108, 0)
    check(ended_with(1, 1) == [up(REPLAY_MOVIE, 0)], "close, a viewer, the cursor's movie not there: the controls still let go")
    ours.word(CURSOR_HOLDER + 0x108, CURSOR_VIEW)
    state, _, calls = key(ours, SPACE)
    check(state[UC_X86_REG_EIP] == SITES["key"][2] and not calls, "key, after the end: Space goes to the game again")


    # (The unpacked load's site runs patch 87's check after ours, which a
    # made-up world does not pass; the game's own test runs it.)
    stock.setup(2)
    ours.setup(2)
    state, mode, calls = world_load(ours, "raw")
    check(mode == 1 and not calls and world_load(stock, "raw")[0] == state,
          "raw, a player's game: its world loads as a network one (mode 1), as the original")

    # The replay controls (patch 130's second part): every network game's first world loads
    # them, and their hud gives them the mouse only in a viewer's game.
    for local, who in ((2, "a player's"), (0xFF, "a spectator's (no place local)")):
        for site, name in (("query", "the query"), ("take", "the callback's take")):
            stock.setup(local)
            ours.setup(local)
            o = load(ours, site)
            want = SITES[site][3]
            check(o[0][UC_X86_REG_EIP] == want and not o[2], f"{site}, {who} world: {name} of replay_player.swf made ({want:#x})")
        for site, name in (("button", "a button"), ("pointer", "the pointer")):
            ours.setup(local)
            o = mouse(ours, site)
            check(o[0][UC_X86_REG_EIP] == SITES[site][3] and not o[2],
                  f"{site}, {who} game: {name} to the hud movie ({SITES[site][3]:#x}), not the hidden controls")
    ours.setup(0xFF)
    state, _, calls = control(ours, "update_position", (4, 5000, 0))
    check(state[UC_X86_REG_EIP] == SITES["call"][3] and not calls,
          "call, a live spectator, update_position: taken with nothing done (the game's would seek with no reader)")
    state, _, calls = control(ours, "profile_changed", (4, 1, 0))
    check(state[UC_X86_REG_EIP] == SITES["call"][2], "call, a live spectator, profile_changed (the hud's own): on as the game's")
    ours.run("note", note_regs(ours, playback(1, 0, 100)))
    for site, name in (("button", "a button"), ("pointer", "the pointer")):
        stock.setup(0xFF)
        s, o = mouse(stock, site), mouse(ours, site)
        check(o[0][UC_X86_REG_EIP] == SITES[site][2] and s[0] == o[0] and not o[2],
              f"{site}, a viewer: {name} to the controls, as the original")
    state, _, calls = control(ours, "update_position", (4, 5000, 0))
    check(state[UC_X86_REG_EIP] == SITES["call"][3] and sent(calls, request(0, 3, 15_000)),
          "call, a viewer, update_position(5000): a seek to the start + 5 s")
    state, _, calls = control(ours, "update_play_speed", (5,) + struct.unpack("<2I", struct.pack("<d", 2.2)))
    check(sent(calls, request(1, 2, 200)), "call, a viewer, update_play_speed(2.2): a speed request, 2x (the nearest step)")
    state, _, calls = control(ours, "button_play_clicked", (2, 0, 0))
    check(sent(calls, request(2, 1, 1)), "call, a viewer, button_play_clicked: a pause request")
    state, _, calls = control(ours, "select_free_view")
    check(state[UC_X86_REG_EIP] == SITES["call"][3] and calls == [("detach_from_player", NC)],
          "call, a viewer, select_free_view: patch 121's free view (detach_from_player), no vtable slot past the client's")
    ours.run("note", note_regs(ours, events(0, 1, [(70_000, 1, 1, 2)])))
    state, _, calls = frame(ours)
    names = [(c[2], c[4]) for c in calls if c[0] == "invoke"]
    check(state[UC_X86_REG_EIP] == SITES["frame"][2] and ("show_replay_controls", WORLD_UI, 1) in calls
          and ("root.set_match_time", (4, 590_000)) in names and any(n == "root.set_markers_text" and v[1].startswith("60000:1:") for n, v in names if v)
          and ("root.set_position", (4, CLOCK - 10_000)) in names and ("root.set_play_state", (2, 0)) in names,
          "frame, a viewer: the controls shown, the bar's length, the markers, its position and the play button")
    keys = ["st_srp_viewer_back", "st_srp_viewer_moments", "st_srp_viewer_moments_all", "st_srp_viewer_moments_mine",
            "st_srp_viewer_moments_followed", "st_srp_viewer_cursor", "st_srp_viewer_pause", "st_srp_viewer_filter",
            "st_srp_viewer_hide", "st_srp_viewer_cap_ctrl", "st_srp_viewer_cap_space"]
    check([c[1] for c in calls if c[0] == "translate"] == keys
          and ("root.start_viewer", (6, "st_srp_viewer_back")) in names
          and any(c[2] == "root.start_viewer" and c[3] == 11 for c in calls if c[0] == "invoke")
          and ("root.set_moments", (4, 0)) in names and ("root.set_cursor_shown", (2, 0)) in names,
          "frame, a viewer: the controls made a viewer's (root.start_viewer with its 11 texts translated), "
          "the Moments switch at All (the viewer did not play), the cursor hidden")
    state, _, calls = frame(ours)
    names = [c[2] for c in calls if c[0] == "invoke"]
    check(names == ["root.set_position", "root.set_play_state"] and not [c for c in calls if c[0] != "invoke"],
          "frame, a viewer, the next frame: only the position and the play button")
    state, _, calls = control(ours, "moments_clicked", (4, 2, 0))
    check(state[UC_X86_REG_EIP] == SITES["call"][3] and not calls, "call, a viewer, moments_clicked(2): taken, nothing sent")
    state, _, calls = frame(ours)
    names = [(c[2], c[4]) for c in calls if c[0] == "invoke"]
    check(("root.set_moments", (4, 2)) in names, "frame, after moments_clicked(2): the switch at Followed")
    state, mode, calls = world_load(ours, "raw")
    check(mode == 0 and state[UC_X86_REG_EIP] == SITES["raw"][2] and not calls,
          "raw, a viewer: its world loads as the replay's own seek (mode 0, its pending inputs cleared)")
    key(ours, H)
    state, _, calls = frame(ours)
    check(("show_replay_controls", WORLD_UI, 0) in calls, "frame, after H: the controls hidden with the interface")
    state, _, calls = control(ours, "back_clicked")
    check(state[UC_X86_REG_EIP] == SITES["call"][3] and sent(calls, request(3, 4, 0)),
          "call, a viewer, back_clicked: a leave request (the server's farewell ends the world), no close here")
    close(ours, 0)
    state, _, calls = frame(ours)
    check(not calls, "frame, after the match's end: nothing fed")
    state, _, calls = mouse(ours, "button")
    check(state[UC_X86_REG_EIP] == SITES["button"][3] and not calls, "button, after the match's end: to the hud movie again")

    print("all checks passed" if not failed else f"{failed} check(s) FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
