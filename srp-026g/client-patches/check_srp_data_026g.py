#!/usr/bin/env python3
"""Run patch 94 (rust/patches/src/srp_data.rs, its hooks of srp_data.toml:
srp's tables for the lobby movie) in the default build on made-up game
objects, the original beside it.

    python check_srp_data_026g.py <original survarium.exe>

Requires Python `unicorn`. check_lobby_scene_026g.py's harness (the lobby
camera's tick 0x2840F2 with the lobby movie under the menu, Scaleform's
Invoke 0x511600 a stub that records each call, srp_patches.log's open
counted); besides, query_resources (0xE5B60) a stub that records its
request and its boost function, which the game's own bind (0x176D0) made;
the callback is run through the game's own invoker (0x3AAA20). The table is
rooms.options as content_edit.py makes it of scripts/content-edit/rooms.tsv,
laid in memory as the loader leaves a config: every table's, text's and
name's offset an address. Checked:

- configs.loaded (0x1EA312 to 0x1EA318): one query, of one request
  {"resources/gameplay/srp/rooms.options", class 1Fh}, its boost function
  0x176D0's bound to the entry srp_data_on_config and the word 0, the game's
  allocator, asynchronous; the registers and ESP the original's (which asks
  nothing); a second run asks nothing;
- a lobby frame before the config came tells the movie nothing;
- the config through the game's invoker: the next lobby frame invokes
  root.srp_data("rooms", key, value, ...) with every key of the config, its
  path (`waits[3]`, `pvb/sizes[1]`) and its value as the table's cell, in
  the config's order; the next frame nothing; a movie that refuses it is
  told again at the next frame until it takes it; another lobby movie is
  told again;
- a result without a config (a missing file): a line in srp_patches.log,
  and the movie never told.
"""
import os
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_lobby_scene_026g as lobby

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts", "content-edit"))
import content_edit as ce  # noqa: E402

LOADED, LOADED_END = 0x1EA312, 0x1EA318      # configs.loaded: mov ecx,[edi+5348h]
QUERY, INVOKER, BIND = 0xE5B60, 0x3AAA20, 0x176D0
FUNCTION_VTABLE, ALLOCATOR_SLOT = 0x9DB9E0, 0x1F1D9C4
GAME = 0x30500000                             # the game object (its +5348h the renderer, made up)
CONFIG, BINARY, RESULTS, FUNCTION = 0x30510000, 0x30520000, 0x30521000, 0x30522000
OTHER_MOVIE = lobby.MENU + 0x9800
TABLE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts", "content-edit", "rooms.tsv")


def rooms_config():
    """rooms.options as content_edit.py makes it, and its (key, cell) pairs in the config's order."""
    _, _, rows = ce.read_stats(TABLE)
    buf = ce.new_config(rows[""])
    pairs = [(path[1:], ce.cell_text(buf, ce.find(buf, path[1:]))) for path, kind, _ in ce.walk(buf)[1:]
             if kind not in (ce.T_NAMED, ce.T_INDEXED)]
    return bytes(buf), pairs


def loaded(buf, at):
    """The config as its loader leaves it at `at`: each record's offsets addresses (a table's
    children, a text, its name)."""
    out = bytearray(buf)

    def walk(off):
        data, name, crc, kind, count = ce.REC.unpack_from(buf, off)
        if kind in (ce.T_NAMED, ce.T_INDEXED, ce.T_STR) and (count or kind == ce.T_STR):
            data = (data & ~0xFFFFFFFF) | (at + (data & 0xFFFFFFFF))
        ce.REC.pack_into(out, off, data, at + name, crc, kind, count)
        if kind in (ce.T_NAMED, ce.T_INDEXED):
            for i in range(count):
                walk((ce.REC.unpack_from(buf, off)[0] & 0xFFFFFFFF) + 24 * i)
    walk(0)
    return bytes(out)


class Run(lobby.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(GAME, 0x40000, UC_PROT_ALL)
        self.queries = []
        self.refusing = False
        self.emu.hook_add(UC_HOOK_CODE, self.query_resources, begin=QUERY, end=QUERY)

    def query_resources(self, emu, address, size, _):
        """query_resources (eax the requests, ecx their count; &function, allocator, &user data,
        parent, flag; cdecl): recorded, not run."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 * i)
        requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
        user = self.read(arg(3))
        self.queries.append((self.text(self.read(requests)).decode(), self.read(requests + 4), count,
                             self.read(arg(1), 0x20), arg(2), user, arg(4), arg(5)))
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, arg(0))

    def stub(self, emu, address, size, _):
        super().stub(emu, address, size, _)
        if address == hide.INVOKE and self.refusing:
            emu.reg_write(UC_X86_REG_EAX, 0)

    def configs_loaded(self):
        """configs.loaded, EDI the game: the registers and ESP after, the queries asked."""
        self.queries = []
        sp = hide.STACK + 0x9000
        self.word(GAME + 0x5348, 0x31313131)
        regs, end = self.run(LOADED, (LOADED_END,), sp, {**hide.GAME, UC_X86_REG_EDI: GAME})
        assert self.stopped == LOADED_END, hex(self.stopped or 0)
        return regs, end, self.queries

    def arrive(self, function, config):
        """The query done: the game's invoker (the function's functor +8, the results) with a result
        whose first resource is `config` (0: none, the query failed)."""
        self.emu.mem_write(FUNCTION, function)
        self.emu.mem_write(RESULTS, bytes(0x400))
        self.word(RESULTS + 0x38, 1)
        self.word(RESULTS + 0x12C, config)
        sp = hide.STACK + 0x9000
        for i, value in enumerate((hide.SENTINEL, FUNCTION + 8, RESULTS)):
            self.word(sp + 4 * i, value)
        self.run(INVOKER, (hide.SENTINEL,), sp, hide.GAME)
        assert self.stopped == hide.SENTINEL, hex(self.stopped or 0)

    def frame(self, movie=lobby.MOVIE, refusing=False):
        """A lobby frame, the menu's movie `movie`: the root.srp_data invokes."""
        self.word(lobby.VIEW + 4, movie)
        self.refusing = refusing
        _, _, invokes = self.tick()
        self.refusing = False
        return [(at, values) for at, name, values in invokes if name == "root.srp_data"]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    entry = base.arena().symbols["srp_data_on_config"]
    buf, pairs = rooms_config()
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    def made(run):
        run.lobby()
        run.camera()
        run.emu.mem_write(CONFIG, loaded(buf, CONFIG))
        run.emu.mem_write(BINARY, bytes(0x200))
        run.word(BINARY + 0x108, CONFIG)
        return run

    stock, ours = made(Run(original)), made(Run(bytes(patched)))
    a, b = stock.configs_loaded(), ours.configs_loaded()
    q = b[2][0] if len(b[2]) == 1 else None
    vtable, functor, word = struct.unpack_from("<I4xII", q[3]) if q else (0, 0, 0)
    report(a[:2] == b[:2] and a[0][UC_X86_REG_ECX] == 0x31313131 and a[2] == [] and q is not None
           and q[:3] == ("resources/gameplay/srp/rooms.options", 0x1F, 1) and vtable == FUNCTION_VTABLE | 1
           and functor == entry and word == 0 and q[4] == ours.read(ALLOCATOR_SLOT) and q[5] == 0 and q[6:] == (0, 1),
           "configs.loaded: one class 1Fh request of resources/gameplay/srp/rooms.options, a boost function of "
           "0x176D0 bound to srp_data_on_config and word 0, the game's allocator, no user data; the registers "
           "and ESP the original's (which asks nothing)")
    report(ours.configs_loaded()[2] == [], "configs.loaded again: nothing asked")
    report(ours.frame() == [], "a lobby frame before the config came: nothing told")

    ours.arrive(q[3] if q else bytes(0x20), BINARY)
    want = ("rooms",) + tuple(text for pair in pairs for text in pair)
    told = ours.frame()
    report(told == [(lobby.MOVIE, want)] and len(want) <= 64,
           f"the config arrives; the next lobby frame: root.srp_data(\"rooms\", ...) with its {len(pairs)} keys "
           f"and values in the config's order ({', '.join(f'{k}={v}' for k, v in pairs[:4])}, ...)")
    report(ours.frame() == [], "the next frame: nothing")
    report(ours.frame(OTHER_MOVIE, refusing=True) == [(OTHER_MOVIE, want)]
           and ours.frame(OTHER_MOVIE) == [(OTHER_MOVIE, want)] and ours.frame(OTHER_MOVIE) == [],
           "another lobby movie refusing (no root yet): told again at the next frame, then not")

    missing = made(Run(bytes(patched)))
    q = missing.configs_loaded()[2][0]
    before = missing.logged
    missing.arrive(q[3], 0)
    report(missing.logged == before + 1 and missing.frame() == [] and missing.frame() == [],
           "a missing config: a line in srp_patches.log, the movie never told")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
