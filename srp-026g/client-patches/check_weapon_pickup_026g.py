#!/usr/bin/env python3
"""Patch 83 (spare_weapons, the dropped-weapon pickup spike): each match kit
player's cook also cooks a spare inventory of its kit's weapons and ammo.

Runs the three installed Rust hooks of the patched exe with Unicorn, the
game's own profile code (player_profile::player_profile 0x2F73F0,
operator= 0x284D10, ~player_profile 0x26F3B0) unstubbed, on a made-up player
cook frame, stage, profile and queries_result:

- the player cook's query (0x202A18 to 0x202A43): the native batch is queried
  once with one more request, an inventory (41h) whose user data is a copy of
  the native inventory variant with a recipe of ours: the native recipe's
  fields, the profile a deep copy of the player's whose kit keeps slots 7, 10,
  8, 9, 11, 12 and 19..22 (the others FFFFh), the source and the other kits
  untouched; EAX the callback's first word as 0x202A3D loads it, the stack
  and the callee-saved registers kept;
- the lobby's character (+B1h 1), a request list with no inventory request,
  one too long, and SRP_SPARE_WEAPONS=off: the native bytes run on and
  nothing is allocated (or what was is given back);
- the batch's completion (0x202DD0): the spare's result kept (its count +1)
  and the recipe freed; an empty result or another count: skipped, the
  recipe and the profile given back, a FAILED line logged;
- player::~player (0x1F2100) of that kit player: the spare's reference given
  back (0x0E0210 when it was the last), the profile destroyed and freed;
- every allocation is freed exactly once.

Usage: python check_spare_weapons_026g.py <original survarium.exe>
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide

QUERY_SITE, QUERIED = 0x202A18, 0x202A43
QUERY_REPLAY_END = 0x202A1E                 # after the six replayed bytes
READY_SITE, READY_AFTER = 0x202DD0, 0x202DD6
DESTROY_SITE, DESTROY_AFTER = 0x1F2100, 0x1F2106
MALLOC, FREE, QUERY, RELEASE = 0xE4F90, 0x1539D0, 0xE5B60, 0xE0210
IAT = {"env": 0x8701B4, "memory": 0x870164, "pid": 0x8701B8, "qpc": 0x870158, "qpf": 0x870150, "thread": 0x870118,
       "create": 0x870048, "write": 0x870108, "close": 0x870040}
ALLOCATOR_SLOT = 0x1F1D9C4
ORIGINAL = {QUERY_SITE: "8b55d48b45a4", READY_SITE: "558bec83e4f8", DESTROY_SITE: "558bec83e4f8"}

DATA = 0x30000000
HEAP = 0x31000000
STUBS = 0x32000000
EBP = DATA + 0x8000                         # the player cook's frame
STAGE, NATIVE_RECIPE, PROFILE, RESULT, PLAYER = (DATA + 0x10000, DATA + 0x11000, DATA + 0x20000,
                                                DATA + 0x40000, DATA + 0x50000)
REQUESTS, USERS, PATHS, ALLOCATOR = DATA + 0x12000, DATA + 0x13000, DATA + 0x14000, DATA + 0x15000
INVENTORY = DATA + 0x60000
PROFILE_SIZE, KIT_STRIDE, SLOTS, ITEMS = 0x4634, 0x704, 0x98, 0x1568
KEEP = {7, 10, 8, 9, 11, 12, 19, 20, 21, 22}
VARIANT_VTABLE, TYPE_ID, CALLBACK_WORD, PARENT = 0x9FD808, 0x1234, 0x9FD781, 0x0BADF00D
INVENTORY_AT = 4


class Run(hide.Run):
    def __init__(self, data, off=False):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(DATA, 0x100000, UC_PROT_ALL)
        emu.mem_map(HEAP, 0x400000, UC_PROT_ALL)
        emu.mem_map(STUBS, 0x1000, UC_PROT_ALL)
        self.heap, self.live, self.frees, self.queries, self.releases, self.log = HEAP, {}, [], [], [], []
        self.off = off
        self.used, self.ticks = 900 << 20, 0
        self.stub_at = {}
        for n, name in enumerate(IAT):
            at = STUBS + 0x10 * n
            emu.mem_write(at, b"\xC3")
            self.word(IAT[name], at)
            self.stub_at[at] = name
        self.word(ALLOCATOR_SLOT, ALLOCATOR)
        for stub in [MALLOC, FREE, QUERY, RELEASE, *self.stub_at]:
            emu.hook_add(UC_HOOK_CODE, self.engine, begin=stub, end=stub)

    def ret(self, pops, eax=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def engine(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 * n)
        reg = emu.reg_read
        if address == MALLOC:                       # ESI allocator, EAX size, [esp+4] description; ret 4
            assert reg(UC_X86_REG_ESI) == ALLOCATOR
            size = reg(UC_X86_REG_EAX)
            block, self.heap = self.heap, self.heap + ((size + 0xF) & ~0xF) + 0x10
            emu.mem_write(block, b"\xCD" * size)    # not zeroed, as the game's
            self.live[block] = size
            self.ret(4, block)
        elif address == FREE:                       # ECX allocator, EAX memory, [esp+4] &pointer; ret 0
            assert reg(UC_X86_REG_ECX) == ALLOCATOR
            memory = reg(UC_X86_REG_EAX)
            assert memory in self.live, hex(memory)
            del self.live[memory]
            self.frees.append(memory)
            self.word(arg(1), 0)
            self.ret(0)
        elif address == QUERY:                      # EAX requests, ECX count; callback, allocator, users, parent, flag
            count, requests, users = reg(UC_X86_REG_ECX), reg(UC_X86_REG_EAX), arg(3)
            batch = [(self.read(requests + 8 * n), self.read(requests + 8 * n + 4), self.read(users + 4 * n))
                     for n in range(count)]
            last = self.read(users + 4 * (count - 1))
            variant = self.read(last, 0x30) if last else None
            self.queries.append({"batch": batch, "args": [arg(n) for n in range(1, 6)], "variant": variant})
            self.ret(0)
        elif address == RELEASE:                    # ECX the object, EAX &its count
            self.releases.append((reg(UC_X86_REG_ECX), reg(UC_X86_REG_EAX)))
            self.ret(0)
        else:
            name = self.stub_at[address]
            if name == "env":                       # (name, buffer, size): length
                value = b"off" if self.off else b""
                emu.mem_write(arg(2), value + b"\0")
                self.ret(12, len(value))
            elif name == "memory":                  # (status): MEMORYSTATUSEX
                self.used += 3 << 20       # each look 3 MiB more in use
                status = struct.pack("<II7Q", 64, 37, 0, 0, 0, 0, 4 << 30, (4 << 30) - self.used, 0)
                emu.mem_write(arg(1), status)
                self.ret(4, 1)
            elif name in ("qpc", "qpf"):
                if name == "qpc":
                    self.ticks += 2_500_000         # a quarter of a second a look
                emu.mem_write(arg(1), struct.pack("<Q", 10_000_000 if name == "qpf" else self.ticks))
                self.ret(4, 1)
            elif name == "thread":
                self.ret(0, 7)
            elif name == "pid":
                self.ret(0, 4242)
            elif name == "create":
                self.ret(28, 0x44)
            elif name == "write":                   # (file, bytes, count, &written, overlapped)
                self.log.append(bytes(emu.mem_read(arg(2), arg(3))).decode().strip())
                self.ret(20, 1)
            else:
                self.ret(4, 1)

    # ---- the made-up cook -------------------------------------------------------------------
    def profile(self):
        """A constructed profile (the game's constructor) with three kits' slot words."""
        self.emu.mem_write(PROFILE, bytes(PROFILE_SIZE + 0x100))
        self.call(0x2F73F0, ecx=PROFILE)
        self.emu.mem_write(PROFILE + 0x57, b"\x03")
        for kit in range(3):
            for slot in range(23):
                self.emu.mem_write(PROFILE + SLOTS + kit * KIT_STRIDE + 4 * slot,
                                   struct.pack("<HH", kit * 23 + slot, 100 + slot))
        for item in range(69):
            self.emu.mem_write(PROFILE + ITEMS + 0xB4 * item + 4, struct.pack("<H", item))
        return self.read(PROFILE, PROFILE_SIZE)

    def call(self, va, ecx=0):
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        self.emu.reg_write(UC_X86_REG_ECX, ecx)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.started = (hide.SENTINEL,), False
        self.emu.emu_start(va, hide.SENTINEL, count=2_000_000)

    def cook(self, kit=1, place=5, lobby=0, count=15, inventory_at=INVENTORY_AT):
        self.profile()
        self.word(STAGE + 0x9C, PROFILE)
        self.emu.mem_write(STAGE + 0xA0, bytes([place]))
        self.word(STAGE + 0xA4, 0x0A0B0C0D)
        self.emu.mem_write(STAGE + 0xB0, bytes([kit, lobby]))
        for n, word in enumerate([PROFILE, kit, 0xD1C7, 0xF1515, 0x0A0B0C0D, 0x5EED]):
            self.word(NATIVE_RECIPE + 4 * n, word)
        self.emu.mem_write(NATIVE_RECIPE + 0x18, bytes([lobby]))
        variant = EBP - 0xC0
        self.emu.mem_write(variant, bytes(0x30))
        self.word(variant, VARIANT_VTABLE)
        self.word(variant + 8, NATIVE_RECIPE)
        self.word(variant + 0x28, variant)
        self.word(variant + 0x2C, TYPE_ID)
        for n in range(count):
            inventory = n == inventory_at
            self.word(REQUESTS + 8 * n, PATHS + 0x40 * n)
            self.word(REQUESTS + 8 * n + 4, 0x41 if inventory else 0x50 + n)
            self.word(USERS + 4 * n, variant if inventory else (0 if n % 2 else DATA + 0x16000 + 0x40 * n))
        self.word(EBP - 0x24, REQUESTS)
        self.word(EBP - 0x20, REQUESTS + 8 * count)
        self.word(EBP - 0x5C, USERS)
        self.word(EBP - 0x50, CALLBACK_WORD)
        self.word(EBP - 0x2C, PARENT)
        self.word(EBP - 0x10, NATIVE_RECIPE)
        self.word(EBP - 0x14, STAGE)
        return count

    def query(self):
        regs = {**hide.GAME, UC_X86_REG_EBP: EBP}
        self.queries = []
        after, sp = self.run(QUERY_SITE, (QUERIED, QUERY_REPLAY_END), hide.STACK + 0x9000, regs)
        assert sp == hide.STACK + 0x9000
        for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP):
            assert after[r] == regs[r], (r, hex(after[r]))
        return self.stopped, after

    def ready(self, count, inventory, references=1):
        self.word(RESULT + 0x38, count)
        for n in range(count):
            self.word(RESULT + 0x12C + 0x2E0 * n, 0)
        if count:
            self.word(RESULT + 0x12C + 0x2E0 * (count - 1), inventory)
        if inventory:
            self.word(inventory + 0xD0, references)
        sp = hide.STACK + 0x9000
        self.word(sp, 0x202A62)             # a return address, unread
        self.word(sp + 4, RESULT)
        self.word(sp + 8, STAGE)
        self.word(sp + 12, NATIVE_RECIPE)
        _, after_sp = self.run(READY_SITE, (READY_AFTER,), sp, dict(hide.GAME))
        assert self.stopped == READY_AFTER and after_sp == (sp - 4) & ~7

    def destroy(self, kit):
        self.word(PLAYER + 0x6B8, PROFILE)
        self.emu.mem_write(PLAYER + 0x115, bytes([kit]))
        _, sp = self.run(DESTROY_SITE, (DESTROY_AFTER,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_ECX: PLAYER})
        assert self.stopped == DESTROY_AFTER


def slot_word(snapshot, kit, slot):
    at = SLOTS + kit * KIT_STRIDE + 4 * slot
    return struct.unpack_from("<HH", snapshot, at)


def main():
    original = open(sys.argv[1], 'rb').read()
    for va, hexbytes in ORIGINAL.items():
        at = hide.file_at(original, va)
        assert original[at:at + 6].hex() == hexbytes, (hex(va), original[at:at + 6].hex())
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())

    # A match player's kit 1: one batch of 16, the last an inventory of ours.
    r = Run(patched)
    count = r.cook(kit=1)
    source = r.read(PROFILE, PROFILE_SIZE)
    stopped, after = r.query()
    assert stopped == QUERIED, hex(stopped or 0)
    assert after[UC_X86_REG_EAX] == CALLBACK_WORD
    (q,) = r.queries
    batch = q["batch"]
    assert len(batch) == count + 1
    native = [(r.read(REQUESTS + 8 * n), r.read(REQUESTS + 8 * n + 4), r.read(USERS + 4 * n)) for n in range(count)]
    assert batch[:count] == native
    path, cls, user = batch[count]
    assert (path, cls) == (PATHS + 0x40 * INVENTORY_AT, 0x41) and user not in (0, EBP - 0xC0)
    assert q["args"] == [EBP - 0x50, ALLOCATOR, q["args"][2], PARENT, 1], [hex(a) for a in q["args"]]
    vt, _, recipe = struct.unpack_from("<III", q["variant"])
    this, type_id = struct.unpack_from("<II", q["variant"], 0x28)
    assert (vt, this, type_id) == (VARIANT_VTABLE, user, TYPE_ID)
    assert recipe in r.live and r.live[recipe] == 0x1C
    words = struct.unpack("<6I", r.read(recipe, 0x18))
    copy = words[0]
    assert copy in r.live and r.live[copy] == PROFILE_SIZE and copy != PROFILE
    # The kit and the flag are bytes (their padding is the allocator's, as the native recipe's).
    assert words[1] & 0xFF == 1 and words[2:] == (0xD1C7, 0xF1515, 0x0A0B0C0D, 0x5EED), [hex(w) for w in words]
    assert r.read(recipe + 0x18, 1)[0] == 0
    copied = r.read(copy, PROFILE_SIZE)
    assert r.read(PROFILE, PROFILE_SIZE) == source, "the player's profile changed"
    for kit in range(3):
        for slot in range(23):
            want = slot_word(source, kit, slot)
            if kit == 1 and slot not in KEEP:
                want = (0xFFFF, want[1])
            assert slot_word(copied, kit, slot) == want, (kit, slot)
    # A deep copy: the item records are the source's, their self-relative buffers rebuilt in
    # the copy (no word of it points into the player's profile).
    for item in range(69):
        at = ITEMS + 0xB4 * item + 4
        assert copied[at:at + 2] == source[at:at + 2], item
    words = struct.unpack("<%dI" % (PROFILE_SIZE // 4), copied[:PROFILE_SIZE // 4 * 4])
    assert not [hex(4 * n) for n, w in enumerate(words) if PROFILE <= w < PROFILE + PROFILE_SIZE]
    print("ok a match kit player's cook: one batch, the native requests and an inventory of a weapon-only copy "
          "of its profile; the native recipe's fields; the player's profile untouched; EAX, stack and registers")

    r.ready(count + 1, INVENTORY)
    assert r.read(INVENTORY + 0xD0) == 2 and recipe in r.frees and recipe not in r.live and copy in r.live
    assert any("made, inventory 0x%08x" % INVENTORY in l for l in r.log), r.log
    assert any("all cooks in: 1 spares made, 0 failed" in l for l in r.log), r.log
    assert all(l.startswith("srp: pid 4242 spares") for l in r.log), r.log
    assert any(": cook 250 ms, address space 903 -> 906 MiB, memory load 37 %" in l for l in r.log), r.log
    print("ok the batch in: the spare kept (one reference more), the recipe freed, logged with its time and memory")

    r.destroy(kit=0)
    assert r.read(INVENTORY + 0xD0) == 2 and copy in r.live
    r.destroy(kit=1)
    assert r.read(INVENTORY + 0xD0) == 1 and copy not in r.live and r.releases == []
    assert r.live == {}, {hex(k): v for k, v in r.live.items()}
    print("ok its kit player destroyed: the reference given back, the profile destroyed and freed; another kit's "
          "player leaves it; every allocation freed once")

    # The last reference: the native release.
    r = Run(patched)
    count = r.cook(kit=2, place=0)
    assert r.query()[0] == QUERIED
    r.ready(count + 1, INVENTORY, references=0)
    r.destroy(kit=2)
    assert r.releases == [(INVENTORY, INVENTORY + 0xD0)] and r.live == {}
    print("ok the spare's last reference: released through 0x0E0210")

    # A failed spare: an empty result, or another count.
    for got, inventory in ((16, 0), (15, INVENTORY)):
        r = Run(patched)
        count = r.cook()
        assert r.query()[0] == QUERIED
        r.ready(got, inventory)
        assert r.live == {}, "the recipe and profile given back"
        assert any("FAILED" in l for l in r.log) and any("1 failed" in l for l in r.log), r.log
        if inventory:
            assert r.read(INVENTORY + 0xD0) == 1
        r.destroy(kit=1)
        assert r.releases == []
    print("ok a spare not made (an empty result, another count): skipped and logged, nothing kept or leaked")

    # Nothing asked: the lobby, no inventory request, too many requests, switched off.
    for name, cook, off in (("lobby", dict(lobby=1), False), ("no inventory request", dict(inventory_at=99), False),
                            ("32 requests", dict(count=32), False), ("SRP_SPARE_WEAPONS=off", {}, True)):
        r = Run(patched, off=off)
        r.cook(**cook)
        stopped, after = r.query()
        assert stopped == QUERY_REPLAY_END, (name, hex(stopped or 0))
        assert after[UC_X86_REG_EDX] == PARENT and after[UC_X86_REG_EAX] == USERS, name
        assert r.queries == [] and r.live == {}, name
        r.ready(15, INVENTORY)
        assert r.read(INVENTORY + 0xD0) == 1, name
    print("ok the lobby, a batch with no inventory request, one too long, and switched off: the native bytes run "
          "on, nothing asked or kept, nothing leaked")


if __name__ == '__main__':
    main()
