#!/usr/bin/env python3
"""Run patch 63 (team_damage_026g.asm and .c: team damage turned back on its
dealer past a threshold) in the default build on made-up game objects.

    python check_team_damage_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; each entry runs from the game's own site, the original exe
the same way:

- the static info: the game's match_options::deserialize (0x1F0E70) reads
  made-up options (twenty empty profiles, the map, the roster, the id, the
  level, the weights, the squads) with and without srp's block after them.
  The options' +57860h must hold 1 and the three numbers exactly for a good
  block, zeros for none, a wrong one, a short one or numbers out of range,
  every field the original reads read as the original reads it, and the
  function return `ret 8` with ebx esi edi ebp kept;
- the rule: kd_stats_rule_cook's create calls the game's constructor
  (0x323790) from 0x2CCE4A for a rule of 268h bytes full of junk. The rule
  must be the game's (its vtable, its places), its team damage part the
  options' numbers or zeros and no counts, back at 0x2CCE4F with eax the
  rule and the query popped; the original leaves the junk;
- the world: game_world_core::serialize at 0x2D2716 with the game's own
  field writers on a buffer, then deserialize at 0x2D2C75 with the reader
  at [esp+3Ch]. The save must append "SRTD", the places and each place's
  two counts and clear the item's byte as the original; the load must take
  them back into another rule, the reader past them, eax [ebp+19034h], and
  a world without them (or a short or wrong one) must leave no counts and
  the reader where it was;
- the hit: base_player::hit_impl (0x2D9B90) is entered as its callers do;
  its body (0x2D9B96) is a stub that records the player and the hit and
  returns as the function. A teammate's hit with its dealer past both
  thresholds must reach the body once, for the dealer, with the hit's
  damage times return_ratio and nothing else changed, and never for the
  victim; any other must reach it for the victim alone, as the original
  does every one; back at the caller with the two arguments popped and ebx
  esi edi ebp kept;
- the pain lost: hit_impl's `subss` at 0x2D9EA9 runs with a value on the
  x87 stack. The dealer's counts must grow by the loss (the team count for a
  teammate only), xmm0 and every register as the original leaves them, the
  x87 value still there.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_XMM0

import patch_026g as base

DESERIALIZE, OPTIONS_END = 0x1F0E70, 0x1F0F30
KD_SITE, KD_BACK = 0x2CCE4A, 0x2CCE4F
SAVE_SITE, SAVE_BACK = 0x2D2716, 0x2D271D
LOAD_SITE, LOAD_BACK = 0x2D2C75, 0x2D2C7B
HIT_IMPL, HIT_BODY = 0x2D9B90, 0x2D9B96
LOST_SITE, LOST_BACK = 0x2D9EA9, 0x2D9EB1
KD_VTABLE = 0xA177D4
THREAD_ID = 0x870118                   # [import] GetCurrentThreadId

OPTIONS, WORK, STACK, CORE = 0x30000000, 0x31000000, 0x32000000, 0x33000000
KD, PLAYERS, PROFILES, PLACES = 0x34000000, 0x34100000, 0x34200000, 0x34400000
HIT, ITEM, SENTINEL, STUB = 0x34500000, 0x35000000, 0x37000000, 0x38000000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
MARKS = dict(zip(REGS, (0x0A0A0A0A, 0x0C0C0C0C, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0F0F0F0F)))
TD_ON, TD_COUNTS, TD_SIZE, KD_SIZE = 0x1B8, 0x1C8, 0xB0, 0x268
OPTIONS_TEAM_DAMAGE = 0x57860
PLACE_COUNT = 4
TEAMS = (0, 0, 1, 1)                   # places 0 and 1 against 2 and 3
f32 = lambda x: struct.unpack("<f", struct.pack("<f", x))[0]


def empty_options(block=b""):
    """Twenty empty profiles, map 61, four places, the rest, then `block`."""
    out = bytearray()
    for _ in range(20):
        out += bytes(1) + bytes(1) + bytes(1) + struct.pack("<I", 10) + bytes(2)
    out += bytes([61, PLACE_COUNT]) + struct.pack("<IH", 0x80001234, 3)
    out += struct.pack("<28f", *[float(i) for i in range(28)]) + bytes(range(20))
    return bytes(out) + block


def srpx(*entries):
    return b"SRPX" + b"".join(bytes([tag, len(body)]) + body for tag, body in entries)


def numbers(amount, ratio, turn):
    return struct.pack("<3f", amount, ratio, turn)


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((OPTIONS, 0x60000), (WORK, 0x10000), (STACK, 0x10000), (CORE, 0x20000), (KD, 0x10000),
                         (PLAYERS, 0x10000), (PROFILES, 0x20000), (PLACES, 0x10000), (HIT, 0x10000),
                         (ITEM, 0xC1000), (SENTINEL, 0x1000), (STUB, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        # GetCurrentThreadId (the import 0x870118, which the rules' base constructors call): a thread
        emu.mem_write(STUB + 0x200, b"\xB8" + struct.pack("<I", 0x1234) + b"\xC3")
        self.word(THREAD_ID, STUB + 0x200)
        self.bodies = []
        emu.hook_add(UC_HOOK_CODE, self.body, begin=HIT_BODY, end=HIT_BODY)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        data = bytes(self.emu.mem_read(at, size))
        return struct.unpack("<I", data)[0] if size == 4 else data

    def body(self, emu, address, size, _):
        # hit_impl past its prologue: [ebp+8] the player, [ebp+0Ch] the hit; return as `mov esp,ebp /
        # pop ebp / ret 8` would
        ebp = emu.reg_read(UC_X86_REG_EBP)
        player, hit = self.read(ebp + 8), self.read(ebp + 12)
        self.bodies.append((player, self.read(hit, 0x48)))
        emu.reg_write(UC_X86_REG_EBP, self.read(ebp))
        emu.reg_write(UC_X86_REG_EIP, self.read(ebp + 4))
        emu.reg_write(UC_X86_REG_ESP, ebp + 8 + 8)

    def run(self, start, until, sp, regs):
        for reg, value in {**MARKS, **regs}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        trail = []
        handle = self.emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: trail.append(a) if len(trail) < 100000 else None)
        try:
            self.emu.emu_start(start, until, count=2000000)
        except Exception:
            print(f"  stopped at {self.emu.reg_read(UC_X86_REG_EIP):#x}, started at {start:#x}; last "
                  + " ".join(f"{a:#x}" for a in trail[-30:]))
            raise
        finally:
            self.emu.hook_del(handle)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    # match_options::deserialize on `payload`, the options' +57860h `stale` before.
    def options(self, payload, stale=b"\x77" * 16):
        emu = self.emu
        emu.mem_write(OPTIONS, bytes(0x578A0))
        emu.mem_write(OPTIONS + OPTIONS_TEAM_DAMAGE, stale)
        emu.mem_write(WORK, payload)
        reader = WORK + 0x8000
        for i, value in enumerate((WORK, WORK, len(payload))):
            self.word(reader + 4 * i, value)
        sp = STACK + 0x8000 - 12
        for i, value in enumerate((SENTINEL, OPTIONS, 0)):
            self.word(sp + 4 * i, value)
        regs, esp = self.run(DESERIALIZE, SENTINEL, sp, {UC_X86_REG_EAX: reader})
        return regs, esp - (sp + 12), self.read(OPTIONS + OPTIONS_TEAM_DAMAGE, 16), \
            self.read(OPTIONS + 0x577B0, 0x578A0 - 0x577B0 - 0x60)

    # kd_stats_rule_cook's create at its constructor call: esi the rule, the query pushed.
    def rule(self, rule=KD):
        emu = self.emu
        emu.mem_write(rule, b"\xEE" * KD_SIZE)
        query = WORK + 0x9000
        self.word(query, OPTIONS)
        sp = STACK + 0x8000 - 4
        self.word(sp, query)
        regs, esp = self.run(KD_SITE, KD_BACK, sp, {UC_X86_REG_EAX: rule, UC_X86_REG_ESI: rule})
        return regs, esp - (sp + 4), self.read(rule, KD_SIZE)

    def world(self, counts, on=True, numbers_=(3.0, 0.67, 2.0), places=PLACE_COUNT, kits=None):
        """The core, its places and players, the kd rule with `counts` [(all, team)]."""
        emu = self.emu
        emu.mem_write(CORE, bytes(0x20000))
        emu.mem_write(KD, bytes(KD_SIZE))
        self.word(KD, KD_VTABLE)
        emu.mem_write(KD + 0x1B0, bytes([places]))
        self.word(KD + TD_ON, 1 if on else 0)
        emu.mem_write(KD + TD_ON + 4, struct.pack("<3f", *numbers_))
        for i, (dealt, team) in enumerate(counts):
            emu.mem_write(KD + TD_COUNTS + 8 * i, struct.pack("<2f", dealt, team))
        other = WORK + 0xA000
        self.word(other, 0x12345678)
        rules = WORK + 0xA100
        self.word(rules, other)
        self.word(rules + 4, KD)
        self.word(CORE + 0x18824, rules)
        self.word(CORE + 0x18828, rules + 8)
        self.word(CORE + 0x18458, PLACES)
        self.word(CORE + 0x1845C, PLACES + 0x30 * PLACE_COUNT)
        self.word(CORE + 0x19034, 0x19034ABC)
        emu.mem_write(PLACES, bytes(0x30 * PLACE_COUNT))
        for i in range(PLACE_COUNT):
            player, profile = PLAYERS + 0x1000 * i, PROFILES + 0x5000 * i
            emu.mem_write(player, bytes(0x1000))
            emu.mem_write(player + 0x114, bytes([i]))
            self.word(player + 0x120, CORE)
            emu.mem_write(player + 0x33C, b"\x01\x00")
            self.word(player + 0x6B8, profile)
            self.word(profile + 0x45EC, TEAMS[i])
            kit = 1 if kits is None else kits[i]
            emu.mem_write(PLACES + 0x30 * i + 0xC, bytes([kit]))
            if kit < 3:
                self.word(PLACES + 0x30 * i + 4 * kit, player)

    def counts(self, rule=KD):
        return [struct.unpack("<2f", self.read(rule + TD_COUNTS + 8 * i, 8)) for i in range(PLACE_COUNT)]

    # serialize at 0x2D2716: ebp the core, edi the writer, esi the item.
    def save(self):
        emu = self.emu
        writer, buffer, data = WORK + 0xB000, WORK + 0xB100, WORK + 0xC000
        emu.mem_write(writer, bytes(0x40))
        self.word(writer + 0x14, buffer)
        for i, value in enumerate((0, data, 0xC00, 7)):
            self.word(buffer + 4 * i, value)
        emu.mem_write(data, b"EFFECTS" + bytes(0xC00 - 7))
        emu.mem_write(ITEM + 0xC0BBA, b"\xAA")
        sp = STACK + 0x8000
        regs, esp = self.run(SAVE_SITE, SAVE_BACK, sp, {UC_X86_REG_EBP: CORE, UC_X86_REG_EDI: writer,
                                                        UC_X86_REG_ESI: ITEM})
        size = self.read(buffer + 12)
        return regs, esp - sp, self.read(data, size), self.read(ITEM + 0xC0BBA, 1)

    # deserialize at 0x2D2C75: the reader at [esp+3Ch] over `blob`, at `cursor`.
    def load(self, blob, cursor):
        self.emu.mem_write(WORK + 0xD000, blob)
        sp = STACK + 0x8000
        for i, value in enumerate((WORK + 0xD000, WORK + 0xD000 + cursor, len(blob))):
            self.word(sp + 0x3C + 4 * i, value)
        regs, esp = self.run(LOAD_SITE, LOAD_BACK, sp, {UC_X86_REG_EBP: CORE})
        return regs, esp - sp, self.read(sp + 0x40) - (WORK + 0xD000)

    # hit_impl entered with (victim, hit), as base_player::hit calls it.
    def hit(self, victim, dealer, damage=0.5, source=True):
        emu = self.emu
        hit = bytearray(range(0x48))
        struct.pack_into("<I", hit, 0x20, WORK + 0xE000 if source else 0)
        struct.pack_into("<f", hit, 0x34, damage)
        struct.pack_into("<f", hit, 0x38, 0.3)
        hit[0x44] = dealer
        emu.mem_write(HIT, bytes(hit))
        sp = STACK + 0x8000 - 12
        for i, value in enumerate((SENTINEL, PLAYERS + 0x1000 * victim, HIT)):
            self.word(sp + 4 * i, value)
        self.bodies = []
        regs, esp = self.run(HIT_IMPL, SENTINEL, sp, {})
        return regs, esp - (sp + 12), list(self.bodies), bytes(hit), self.read(HIT, 0x48)

    # hit_impl at 0x2D9EA9 with a value on the x87 stack: fld, the site, then fstp at 0x2D9EB1.
    def lost(self, victim, dealer, before, after, source=True):
        emu = self.emu
        mark, out, pool = WORK + 0xF000, WORK + 0xF010, WORK + 0xF100
        emu.mem_write(mark, struct.pack("<f", 1234.5))
        emu.mem_write(out, bytes(4))
        emu.mem_write(pool, bytes(0x100))
        emu.mem_write(pool + 0xA0, struct.pack("<f", after))
        hit = bytearray(0x48)
        struct.pack_into("<I", hit, 0x20, WORK + 0xE000 if source else 0)
        hit[0x44] = dealer
        emu.mem_write(HIT, bytes(hit))
        stub = b"\xD9\x05" + struct.pack("<I", mark) + b"\xE9" + struct.pack("<i", LOST_SITE - (STUB + 11))
        emu.mem_write(STUB, stub)
        emu.mem_write(STUB + 0x100, b"\xD9\x1D" + struct.pack("<I", out) + b"\x90")
        handle = emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: e.reg_write(UC_X86_REG_EIP, STUB + 0x100),
                              begin=LOST_BACK, end=LOST_BACK)
        emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", before))[0])
        sp = STACK + 0x8000
        regs, esp = self.run(STUB, STUB + 0x106, sp, {UC_X86_REG_EAX: pool, UC_X86_REG_EBX: PLAYERS + 0x1000 * victim,
                                                      UC_X86_REG_EDI: HIT})
        emu.hook_del(handle)
        xmm0 = emu.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF
        return regs, esp - sp, struct.unpack("<f", struct.pack("<I", xmm0))[0], struct.unpack("<f", bytes(emu.mem_read(out, 4)))[0]


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

    print("the static info (match_options::deserialize 0x1F0E70)")
    good = numbers(3.0, 0.67, 2.0)
    cases = [
        ("no block", b"", None),
        ("0.28's numbers", srpx((1, good)), (3.0, 0.67, 2.0)),
        ("other numbers after an unknown tag", srpx((9, b"xyz"), (1, numbers(1.5, 0.5, 1.0))), (1.5, 0.5, 1.0)),
        ("a wrong magic", b"SRPY" + bytes([1, 12]) + good, None),
        ("a short block", srpx((1, good))[:-1], None),
        ("a wrong length", srpx((1, good + b"\0")), None),
        ("a ratio over 1", srpx((1, numbers(3.0, 1.5, 2.0))), None),
        ("a negative amount", srpx((1, numbers(-1.0, 0.67, 2.0))), None),
        ("a NaN", srpx((1, struct.pack("<I", 0x7FC00000) + good[4:])), None),
        ("a return ratio over 16", srpx((1, numbers(3.0, 0.67, 17.0))), None),
    ]
    for name, block, want in cases:
        payload = empty_options(block)
        a, b = stock.options(payload), patched.options(payload)
        expected = bytes(16) if want is None else struct.pack("<I3f", 1, *want)
        ok = a[1] == b[1] == 0 and all(a[0][r] == b[0][r] == MARKS[r] for r in KEPT if r != UC_X86_REG_EBX) \
            and a[3] == b[3] and a[2] == b"\x77" * 16 and b[2] == expected
        report(ok, f"{name}: +57860h {b[2].hex()}, the original's fields read alike and its +57860h left, "
                   "`ret 8`, esi edi ebp kept")

    print("the rule (kd_stats_rule_cook's create, 0x2CCE4A)")
    for name, block in (("with the numbers", srpx((1, good))), ("without", b"")):
        stock.options(empty_options(block))
        patched.options(empty_options(block))
        a, b = stock.rule(), patched.rule()
        part = struct.pack("<I3f", 1, 3.0, 0.67, 2.0) if block else bytes(16)
        ok = a[1] == b[1] == 0 and a[0][UC_X86_REG_EAX] == b[0][UC_X86_REG_EAX] == KD \
            and a[2][:0x1B0] == b[2][:0x1B0] and a[2][0x1B1:0x1B8] == b[2][0x1B1:0x1B8] \
            and struct.unpack_from("<I", b[2], 0)[0] == KD_VTABLE \
            and a[2][0x1B0] == PLACE_COUNT and b[2][0x1B0] == 20 and b[2][0x110:0x1B0] == bytes(0xA0) \
            and a[2][0x1B8:] == b"\xEE" * TD_SIZE and b[2][0x1B8:] == part + bytes(TD_SIZE - 16)
        report(ok, f"{name}: the game's rule (vtable, 20 places as the late join makes it, {PLACE_COUNT} in "
                   f"the original; records zero), its team damage part "
                   f"{b[2][0x1B8:0x1C8].hex()} and no counts; back with eax the rule and the query popped; the "
                   "original leaves the junk")

    print("the world (serialize 0x2D2716, deserialize 0x2D2C75)")
    counts = [(4.0, 3.5), (0.25, 0.0), (1.0, 0.5), (0.0, 0.0)]
    stock.world(counts)
    patched.world(counts)
    a, b = stock.save(), patched.save()
    record = b"SRTD" + bytes([PLACE_COUNT]) + b"".join(struct.pack("<2f", *c) for c in counts)
    report(a[1] == b[1] == 0 and a[2] == b"EFFECTS" and b[2] == b"EFFECTS" + record and a[3] == b[3] == b"\0"
           and all(b[0][r] == MARKS[r] for r in REGS if r not in (UC_X86_REG_EBP, UC_X86_REG_EDI, UC_X86_REG_ESI)),
           f"the save appends {len(record)} bytes after the effects' (\"SRTD\", the places, each place's two "
           "counts) with the game's writers, clears the item's byte as the original, every register kept")
    blob = b"EFFECTS" + record + b"NEXT"
    for name, data, cursor, want, moved in (
            ("the record", blob, 7, counts, 5 + 8 * PLACE_COUNT),
            ("no record", b"EFFECTS", 7, [(0.0, 0.0)] * PLACE_COUNT, 0),
            ("a short record", (b"EFFECTS" + record)[:-1], 7, [(0.0, 0.0)] * PLACE_COUNT, 0),
            ("another magic", b"EFFECTS" + b"XRTD" + record[4:], 7, [(0.0, 0.0)] * PLACE_COUNT, 0),
            ("too many places", b"EFFECTS" + b"SRTD" + bytes([21]) + bytes(8 * 21), 7,
             [(0.0, 0.0)] * PLACE_COUNT, 0)):
        stock.world([(9.0, 9.0)] * PLACE_COUNT)
        patched.world([(9.0, 9.0)] * PLACE_COUNT)
        a, b = stock.load(data, cursor), patched.load(data, cursor)
        ok = a[1] == b[1] == 0 and a[0][UC_X86_REG_EAX] == b[0][UC_X86_REG_EAX] == 0x19034ABC \
            and a[2] == cursor and b[2] == cursor + moved and patched.counts() == want \
            and stock.counts() == [(9.0, 9.0)] * PLACE_COUNT \
            and all(b[0][r] == MARKS[r] for r in REGS if r not in (UC_X86_REG_EAX, UC_X86_REG_EBP))
        report(ok, f"{name}: the counts {patched.counts()}, the reader {moved} bytes on, eax [ebp+19034h], the "
                   "other registers kept; the original reads nothing")

    print("the hit (base_player::hit_impl 0x2D9B90)")
    hot = [(4.0, 3.5), (6.0, 3.5), (3.0, 2.9), (3.0, 3.0)]   # 0: past both; 1: ratio low; 2, 3: amount not over
    #        name, victim, dealer, counts, on, source, alive, kit of the dealer, turned
    cases = [
        ("a teammate past both thresholds", 1, 0, hot, True, True, 1, 1, True),
        ("a teammate whose share is 0.58", 0, 1, hot, True, True, 1, 1, False),
        ("a teammate at 2.9 of team damage", 3, 2, hot, True, True, 1, 1, False),
        ("a teammate at exactly 3.0", 2, 3, hot, True, True, 1, 1, False),
        ("an enemy past both", 2, 0, hot, True, True, 1, 1, False),
        ("one's own hit", 0, 0, hot, True, True, 1, 1, False),
        ("no dealer (0xFF)", 1, 0xFF, hot, True, True, 1, 1, False),
        ("no source (a fall's)", 1, 0, hot, True, False, 1, 1, False),
        ("a dead victim", 1, 0, hot, True, True, 0, 1, False),
        ("the server's numbers off", 1, 0, hot, False, True, 1, 1, False),
        ("a dealer without a player", 1, 0, hot, True, True, 1, 3, False),
        ("a dealer with no counts (0 / 0)", 1, 0, [(0.0, 0.0)] * 4, True, True, 1, 1, False),
    ]
    for name, victim, dealer, cnt, on, source, alive, kit, turned in cases:
        results = []
        for run in (stock, patched):
            kits = [1] * PLACE_COUNT
            if dealer < PLACE_COUNT:
                kits[dealer] = kit
            run.world(cnt, on=on, kits=kits)
            run.emu.mem_write(PLAYERS + 0x1000 * victim + 0x33C, bytes([alive]))
            results.append(run.hit(victim, dealer, source=source))
        a, b = results
        victim_player, dealer_player = PLAYERS + 0x1000 * victim, PLAYERS + 0x1000 * dealer
        if turned:
            want = bytearray(b[3])
            struct.pack_into("<f", want, 0x34, f32(0.5 * 2.0))
            body = [(dealer_player, bytes(want))]
        else:
            body = [(victim_player, b[3])]
        ok = a[1] == b[1] == 0 and a[2] == [(victim_player, a[3])] and b[2] == body and b[4] == b[3] \
            and all(b[0][r] == MARKS[r] for r in KEPT) and all(a[0][r] == MARKS[r] for r in KEPT)
        report(ok, f"{name}: the body for the " + ("dealer, damage 0.5 x 2.0" if turned else "victim")
                   + ", the hit itself untouched, `ret 8`, ebx esi edi ebp kept; the original's for the victim")
    patched.world(hot)
    patched.emu.mem_write(KD + 0x1B0, bytes([2]))
    b = patched.hit(1, 0)
    report([player for player, _ in b[2]] == [PLAYERS], "a dealer past both, the rule of two places (the "
                                                        "dealer's 0 of 2): turned")
    patched.world(hot)
    patched.emu.mem_write(KD + 0x1B0, bytes([0]))
    b = patched.hit(1, 0)
    report(b[2] == [(PLAYERS + 0x1000, b[3])], "the dealer's place not one of the rule's (0 places): dealt")

    print("the pain lost (hit_impl 0x2D9EA9)")
    for name, victim, dealer, source, want in (
            ("a teammate's", 1, 0, True, [(0.25, 0.25)]),
            ("an enemy's", 2, 0, True, [(0.25, 0.0)]),
            ("one's own", 0, 0, True, [(0.0, 0.0)]),
            ("no dealer", 1, 0xFF, True, None),
            ("no source", 1, 0, False, [(0.0, 0.0)])):
        stock.world([(0.0, 0.0)] * PLACE_COUNT)
        patched.world([(0.0, 0.0)] * PLACE_COUNT)
        a, b = stock.lost(victim, dealer, 0.75, 0.5, source), patched.lost(victim, dealer, 0.75, 0.5, source)
        counts_after = patched.counts()
        expect = [(0.0, 0.0)] * PLACE_COUNT
        if want and dealer < PLACE_COUNT:
            expect[dealer] = want[0]
        ok = a[1] == b[1] == 0 and a[2] == b[2] == f32(0.75 - 0.5) and a[3] == b[3] == 1234.5 \
            and a[0] == b[0] and counts_after == expect
        report(ok, f"{name}: counts {counts_after}, xmm0 0.25, the x87 value kept, every register the original's")

    print("a match: four hits of a teammate, then the fifth")
    patched.world([(0.0, 0.0)] * PLACE_COUNT)
    seen = []
    for _ in range(5):
        b = patched.hit(1, 0, damage=1.0)
        seen.append(b[2][0][0] == PLAYERS + 0x1000)
        if seen[-1]:
            patched.lost(1, 0, 1.0, 0.0)
    report(seen == [True, True, True, True, False] and patched.counts()[0] == (4.0, 4.0),
           f"dealt 4 times (3.0 is not past 3.0), then turned (counts {patched.counts()[0]}): past 3.0 of team "
           "damage, all of it the team's")
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
