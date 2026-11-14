#!/usr/bin/env python3
"""Patch 63 (team_damage): team damage turned back on its dealer past a
threshold, as 0.28a had it (docs/notes/team-damage.md).

    python check_team_damage_026g.py <original survarium.exe>

Requires Python `unicorn`. The patched exe and the original run side by side
on made-up objects, each hook from the game's own code:

- the rule: process_static_match_info and a replay's chunk 1 read options
  with and without srp's block (check_blackout_026g.py's harness, the whole
  chain of the site); the rule is what the match then does below;
- the hit: base_player::hit_impl (0x2D9B90) entered as its callers do, its
  body past `player.hit_start` (0x2D9BBC) a stub that records the player and
  the hit and leaves by the function's own way out (0x2D9F1A). A teammate's
  hit with its dealer past both thresholds reaches the body once, for the
  dealer, its damage times return_ratio, the hit itself untouched; any other
  reaches it for the victim, as the original does every one;
- the pain lost: hit_impl's subss at 0x2D9EA9: the dealer's counts grow by
  the loss (the teammates' only for a teammate), xmm0 and every register as
  the original leaves them;
- the world: serialize at 0x2D2716 with the game's writers appends the
  counts' record after the effects; deserialize at 0x2D2C75 takes a world's
  own counts (none without the record), the reader past it.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDI, \
    UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_XMM0

import patch_026g as base
import check_blackout_026g as blackout
import check_hide_ui_026g as hide

HIT_IMPL, HIT_BODY, HIT_OUT = 0x2D9B90, 0x2D9BBC, 0x2D9F1A
LOST_SITE, LOST_BACK = 0x2D9EA9, 0x2D9EB1
SAVE_SITE, SAVE_BACK = 0x2D2716, 0x2D271D
LOAD_SITE, LOAD_BACK = 0x2D2C75, 0x2D2C7B
CORE, PLAYERS, PROFILES, PLACES = 0x33000000, 0x34100000, 0x34200000, 0x34400000
HIT, ITEM, BUF, SENTINEL = 0x34500000, 0x35000000, 0x36000000, 0x37000000
PLACE_COUNT = 4
TEAMS = (0, 0, 1, 1)                   # places 0 and 1 against 2 and 3
f32 = lambda x: struct.unpack("<f", struct.pack("<f", x))[0]
RULE = struct.pack("<3f", 3.0, 0.67, 2.0)


def record(counts):
    """The record of `counts` [(all, team)] (none when all are 0)."""
    last = max((i + 1 for i, c in enumerate(counts) if c[0]), default=0)
    if not last:
        return b""
    return b"SRTD" + bytes([last]) + b"".join(struct.pack("<2f", *c) for c in counts[:last])


class Run(blackout.Run):
    def __init__(self, data):
        super().__init__(data)
        for at, size in ((CORE, 0x20000), (PLAYERS, 0x10000), (PROFILES, 0x20000), (PLACES, 0x1000),
                         (HIT, 0x1000), (ITEM, 0xC1000), (BUF, 0x10000), (SENTINEL, 0x1000)):
            self.emu.mem_map(at, size, UC_PROT_ALL)
        self.bodies = []
        self.emu.hook_add(UC_HOOK_CODE, self.body, begin=HIT_BODY, end=HIT_BODY)

    def body(self, emu, address, size, _):
        ebp = emu.reg_read(UC_X86_REG_EBP)
        self.bodies.append((self.read(ebp + 8), self.read(self.read(ebp + 12), 0x48)))
        emu.reg_write(UC_X86_REG_EIP, HIT_OUT)

    def rule(self, block, replay=False):
        ok, _ = (self.replay if replay else self.static_info)(blackout.empty_options(block))
        assert ok, "the options read"

    def world(self, alive=(1,) * PLACE_COUNT, left=()):
        """The core, its places and players (kit 1 each); the places of `left` out of its places' mask."""
        emu = self.emu
        emu.mem_write(CORE, bytes(0x20000))
        self.word(CORE + 0x18458, PLACES)
        self.word(CORE + 0x1845C, PLACES + 0x30 * PLACE_COUNT)
        self.word(CORE + 0x18FD8, sum(1 << i for i in range(PLACE_COUNT) if i not in left))
        self.word(CORE + 0x19034, 0x19034ABC)
        emu.mem_write(PLACES, bytes(0x30 * PLACE_COUNT))
        for i in range(PLACE_COUNT):
            player, profile = PLAYERS + 0x1000 * i, PROFILES + 0x5000 * i
            emu.mem_write(player, bytes(0x1000))
            self.word(player + 0xD0, 1)
            emu.mem_write(player + 0x114, bytes([i]))
            self.word(player + 0x120, CORE)
            emu.mem_write(player + 0x33C, bytes([alive[i], 0]))
            self.word(player + 0x6B8, profile)
            self.word(profile + 0x45EC, TEAMS[i])
            emu.mem_write(PLACES + 0x30 * i + 0xC, b"\x01")
            self.word(PLACES + 0x30 * i + 4, player)

    def hit(self, victim, dealer, damage=0.5, source=True):
        """hit_impl entered with (victim, hit): the bodies reached, the hit after."""
        hit = bytearray(range(0x48))
        struct.pack_into("<I", hit, 0x20, BUF + 0xE000 if source else 0)
        struct.pack_into("<f", hit, 0x34, damage)
        hit[0x44] = dealer
        self.emu.mem_write(HIT, bytes(hit))
        sp = hide.STACK + 0x8000 - 12
        for i, value in enumerate((SENTINEL, PLAYERS + 0x1000 * victim, HIT)):
            self.word(sp + 4 * i, value)
        self.bodies = []
        regs, esp = self.run(HIT_IMPL, (SENTINEL,), sp, dict(hide.GAME))
        kept = esp == sp + 12 and all(regs[r] == hide.GAME[r] for r in
                                      (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))
        return kept, self.bodies, bytes(hit), self.read(HIT, 0x48)

    def lost(self, victim, dealer, before, after, source=True):
        """hit_impl's subss: xmm0 after, the registers kept."""
        pool = BUF + 0xF100
        self.emu.mem_write(pool, bytes(0x100))
        self.emu.mem_write(pool + 0xA0, struct.pack("<f", after))
        hit = bytearray(0x48)
        struct.pack_into("<I", hit, 0x20, BUF + 0xE000 if source else 0)
        hit[0x44] = dealer
        self.emu.mem_write(HIT, bytes(hit))
        self.emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", before))[0])
        regs = {**hide.GAME, UC_X86_REG_EAX: pool, UC_X86_REG_EBX: PLAYERS + 0x1000 * victim, UC_X86_REG_EDI: HIT}
        sp = hide.STACK + 0x8000
        after_regs, esp = self.run(LOST_SITE, (LOST_BACK,), sp, regs)
        xmm0 = struct.unpack("<f", struct.pack("<I", self.emu.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF))[0]
        return after_regs == regs and esp == sp, xmm0

    def save(self):
        """serialize at 0x2D2716 (ebp the core, edi the writer, esi the item): the effects' bytes."""
        writer, buffer, data = BUF + 0xB000, BUF + 0xB100, BUF + 0xC000
        self.emu.mem_write(writer, bytes(0x40))
        self.word(writer + 0x14, buffer)
        for i, value in enumerate((0, data, 0xC00, 7)):
            self.word(buffer + 4 * i, value)
        self.emu.mem_write(data, b"EFFECTS" + bytes(0xC00 - 7))
        self.emu.mem_write(ITEM + 0xC0BBA, b"\xAA")
        regs = {**hide.GAME, UC_X86_REG_EBP: CORE, UC_X86_REG_EDI: writer, UC_X86_REG_ESI: ITEM}
        sp = hide.STACK + 0x8000
        after, esp = self.run(SAVE_SITE, (SAVE_BACK,), sp, regs)
        ok = after == regs and esp == sp and self.read(ITEM + 0xC0BBA, 1) == b"\0"
        return ok, self.read(data, self.read(buffer + 12))

    def load(self, blob, cursor=7):
        """deserialize at 0x2D2C75, the reader at [esp+3Ch] over `blob`: how far it moved."""
        self.emu.mem_write(BUF + 0xD000, blob)
        sp = hide.STACK + 0x8000
        for i, value in enumerate((BUF + 0xD000, BUF + 0xD000 + cursor, len(blob))):
            self.word(sp + 0x3C + 4 * i, value)
        regs = {**hide.GAME, UC_X86_REG_EBP: CORE}
        after, esp = self.run(LOAD_SITE, (LOAD_BACK,), sp, regs)
        ok = esp == sp and after[UC_X86_REG_EAX] == 0x19034ABC and \
            all(after[r] == v for r, v in regs.items() if r != UC_X86_REG_EAX)
        return ok, self.read(sp + 0x40) - (BUF + 0xD000) - cursor


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    on = blackout.srpx((1, RULE))

    def counts_of(run):
        """The counts the patch holds: what its save writes."""
        ok, data = run.save()
        assert ok and data.startswith(b"EFFECTS")
        rest = data[7:]
        out = [(0.0, 0.0)] * PLACE_COUNT
        if rest:
            assert rest[:4] == b"SRTD" and len(rest) == 5 + 8 * rest[4], rest.hex()
            for i in range(rest[4]):
                out[i] = struct.unpack_from("<2f", rest, 5 + 8 * i)
        return out

    def given(run, counts):
        """The patch's counts set by loading a world that has them."""
        ok, moved = run.load(b"EFFECTS" + record(counts))
        assert ok and moved == len(record(counts)), moved

    print("the hit (base_player::hit_impl 0x2D9B90)")
    hot = [(4.0, 3.5), (6.0, 3.5), (3.0, 2.9), (3.0, 3.0)]   # 0 past both; 1 ratio low; 2, 3 amount not over
    cases = [
        # name, victim, dealer, rule, source, alive, the dealer in the world, turned
        ("a teammate past both thresholds", 1, 0, on, True, 1, True, True),
        ("a teammate whose share is 0.58", 0, 1, on, True, 1, True, False),
        ("a teammate at 2.9 of team damage", 3, 2, on, True, 1, True, False),
        ("a teammate at exactly 3.0", 2, 3, on, True, 1, True, False),
        ("an enemy past both", 2, 0, on, True, 1, True, False),
        ("one's own hit", 0, 0, on, True, 1, True, False),
        ("no dealer (0xFF)", 1, 0xFF, on, True, 1, True, False),
        ("no source (a fall's)", 1, 0, on, False, 1, True, False),
        ("a dead victim", 1, 0, on, True, 0, True, False),
        ("no rule (a match not of team play)", 1, 0, b"", True, 1, True, False),
        ("the free-for-all's entry alone", 1, 0, blackout.srpx((3, b"\x1e\x00")), True, 1, True, False),
        ("a dealer who left (not in the world's places)", 1, 0, on, True, 1, False, False),
    ]
    for name, victim, dealer, block, source, alive, present, turned in cases:
        results = []
        for run in (stock, patched):
            run.rule(block)
            alive_ = [1] * PLACE_COUNT
            alive_[victim] = alive
            run.world(alive_, () if present else (dealer,))
            if run is patched and block == on:
                given(run, hot)
            results.append(run.hit(victim, dealer, source=source))
        a, b = results
        victim_player, dealer_player = PLAYERS + 0x1000 * victim, PLAYERS + 0x1000 * dealer
        stock_body = [(victim_player, a[2])] if alive else []
        if turned:
            want = bytearray(b[2])
            struct.pack_into("<f", want, 0x34, f32(0.5 * 2.0))
            body = [(dealer_player, bytes(want))]
        else:
            body = [(victim_player, b[2])] if alive else []
        ok = a[0] and b[0] and a[1] == stock_body and b[1] == body and b[3] == b[2]
        report(ok, f"{name}: the body for " + ("the dealer, damage 0.5 x 2.0" if turned else
                                               "the victim" if alive else "nobody")
               + ", the hit untouched, `ret 8`, ebx esi edi ebp kept; the original's alike for the victim")
    patched.rule(on, replay=True)
    patched.world()
    given(patched, hot)
    report(patched.hit(1, 0)[1][0][0] == PLAYERS, "the rule from a replay's options: turned as well")

    print("the pain lost (hit_impl 0x2D9EA9)")
    for name, victim, dealer, source, want in (
            ("a teammate's", 1, 0, True, (0.25, 0.25)),
            ("an enemy's", 2, 0, True, (0.25, 0.0)),
            ("one's own", 0, 0, True, None),
            ("no dealer", 1, 0xFF, True, None),
            ("no source", 1, 0, False, None)):
        expect = [(0.0, 0.0)] * PLACE_COUNT
        if want:
            expect[dealer] = want
        results = []
        for run in (stock, patched):
            run.rule(on)
            run.world()
            results.append(run.lost(victim, dealer, 0.75, 0.5, source))
        (a_ok, a_x), (b_ok, b_x) = results
        counts = counts_of(patched)
        report(a_ok and b_ok and a_x == b_x == f32(0.25) and counts == expect,
               f"{name}: counts {counts}, xmm0 0.25, every register the original's")
    patched.rule(b"")
    patched.world()
    patched.lost(1, 0, 0.75, 0.5)
    ok, data = patched.save()
    report(ok and data == b"EFFECTS", "no rule: nothing counted, nothing saved")

    print("the world (serialize 0x2D2716, deserialize 0x2D2C75)")
    counts = [(4.0, 3.5), (0.25, 0.0), (1.0, 0.5), (0.0, 0.0)]
    patched.rule(on)
    patched.world()
    given(patched, counts)
    a, b = stock.save(), patched.save()
    report(a[0] and b[0] and a[1] == b"EFFECTS" and b[1] == b"EFFECTS" + record(counts),
           f"the save appends the record ({len(record(counts))} bytes: three places, the last that dealt any), "
           "clears the item's byte as the original, every register kept")
    patched.rule(on)
    ok, data = patched.save()
    report(ok and data == b"EFFECTS", "a new match's options: no counts, no record")
    for name, blob, moved, want in (
            ("the record", b"EFFECTS" + record(counts), len(record(counts)), counts),
            ("no record", b"EFFECTS", 0, [(0.0, 0.0)] * PLACE_COUNT),
            ("a short record", (b"EFFECTS" + record(counts))[:-1], 0, [(0.0, 0.0)] * PLACE_COUNT),
            ("another magic", b"EFFECTS" + b"XRTD" + record(counts)[4:], 0, [(0.0, 0.0)] * PLACE_COUNT),
            ("too many places", b"EFFECTS" + b"SRTD" + bytes([21]) + bytes(8 * 21), 0, [(0.0, 0.0)] * PLACE_COUNT)):
        patched.rule(on)
        patched.world()
        given(patched, [(9.0, 9.0)] * PLACE_COUNT)
        a, b = stock.load(blob), patched.load(blob)
        got = counts_of(patched)
        report(a[0] and b[0] and a[1] == 0 and b[1] == moved and got == want,
               f"{name}: the counts {got}, the reader {moved} bytes on, eax [ebp+19034h], the rest kept")

    print("a match: hits of a teammate until the turn")
    patched.rule(on)
    patched.world()
    seen = []
    for _ in range(5):
        bodies = patched.hit(1, 0, damage=1.0)[1]
        seen.append(bodies[0][0] == PLAYERS + 0x1000)
        if seen[-1]:
            patched.lost(1, 0, 1.0, 0.0)
    report(seen == [True, True, True, True, False] and counts_of(patched)[0] == (4.0, 4.0),
           f"dealt 4 times (3.0 is not past 3), then turned (counts {counts_of(patched)[0]})")
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
