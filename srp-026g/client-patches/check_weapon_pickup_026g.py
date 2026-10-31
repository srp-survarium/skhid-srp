#!/usr/bin/env python3
"""Patch 83 (weapon_pickup, stage 1: the instance; docs/notes/weapon_pickup.md):
at a death that drops a gun, that gun alone is cooked and kept by donor life.

Runs the installed Rust hooks of the patched exe with Unicorn, the game's own
profile code (player_profile::player_profile 0x2F73F0, operator= 0x284D10,
~player_profile 0x26F3B0), match_options 0x219990 and the boost member
callback 0x27D280 (with its store 0x2FFB20) and invoker 0x202300 unstubbed,
on a made-up player, game world, game, network client, match and profile:

- the client death (carry.client_death 0x1F28E0, artefact_carry's handler
  composing weapon_pickup::died) of a player holding a client weapon in slot
  10: one query of one request {"inventory" 0x9FD55C, 41h}, its user data an
  inventory variant (vtable 0x9FD808, its own address at +28h, the type id
  0x205C60 gives) whose recipe has the match's fields (dictionary game+53A4h,
  spatial tree match+130h, the game world, the seed options+577B0h, kit,
  flag 0) and a deep copy of the player's profile whose kit keeps slot 10
  and the ammunition slots 8, 9, 11, 12, 19..22 (slot 7 and the rest FFFFh);
  the callback a boost function (0xA09374 | 1) whose invoker calls our member;
  registers and stack kept, the native bytes run on;
- the same death again (a rollback replaying it): no second query;
- nothing held, a battery's vtable, a weapon in no weapon slot: no query;
- the completion (the game's invoker 0x202300 with the queries_result): the
  inventory kept (one reference more), the recipe freed; the frame's step
  (game_world.after_move 0x1F7C3C, ragdoll's handler composing
  weapon_pickup::after_move) logs it made; past the window and its margin
  the gun is given back (0x0E0210 at its last reference), the profile copy
  destroyed and freed;
- an empty result: FAILED, the recipe and the profile given back;
- the donor destroyed (player.destroy 0x1F2100) while the cook runs: the cook
  is abandoned and its completion gives everything back; destroyed after it
  is made: the gun given back;
- every allocation is freed exactly once.

Usage: python check_weapon_pickup_026g.py <original survarium.exe>
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide

DEATH_SITE, DEATH_AFTER = 0x1F28E0, 0x1F28E5
STEP_SITE, STEP_AFTER = 0x1F7C3C, 0x1F7C41
DESTROY_SITE, DESTROY_AFTER = 0x1F2100, 0x1F2106
MALLOC, FREE, QUERY, RELEASE, TYPE_ID_FN = 0xE4F90, 0x1539D0, 0xE5B60, 0xE0210, 0x205C60
INVOKER, MANAGER_VTABLE = 0x202300, 0xA09374
IAT = {"memory": 0x870164, "pid": 0x8701B8, "qpc": 0x870158, "qpf": 0x870150, "thread": 0x870118,
       "create": 0x870048, "write": 0x870108, "close": 0x870040}
ALLOCATOR_SLOT = 0x1F1D9C4
ORIGINAL = {DEATH_SITE: "578bf96a01", STEP_SITE: "807c240f00", DESTROY_SITE: "558bec83e4f8"}
WEAPON_VTABLE, BATTERY_VTABLE = 0xA04174, 0xA042AC

DATA = 0x30000000
HEAP = 0x31000000
STUBS = 0x32000000
CLIENT = 0x33000000                         # the network client (+1F1C48h its options holder)
OPTIONS_HOLDER = 0x34000000                 # +0Ch the options (+577B0h the seed)
BIG_STACK = 0x36000000                      # the ragdoll's step keeps big frames
MATCH = 0x35000000                          # pvp match: +130h spatial tree, +1F8h the core (+18FC0h time)
PROFILE, RESULT, PLAYER = DATA + 0x20000, DATA + 0x40000, DATA + 0x50000
INVENTORY, WEAPON, WORLD, GAME = DATA + 0x70000, DATA + 0x72000, DATA + 0x74000, DATA + 0x7A000
COOKED, FUNCTOR, ALLOCATOR = DATA + 0x90000, DATA + 0x91000, DATA + 0x92000
PROFILE_SIZE, KIT_STRIDE, SLOTS, ITEMS = 0x4634, 0x704, 0x98, 0x1568
AMMO = {8, 9, 11, 12, 19, 20, 21, 22}
VARIANT_VTABLE, TYPE_ID, PATH = 0x9FD808, 0x1234, 0x9FD55C
DICTIONARY, SPATIAL, SEED = 0xD1C7, 0x5BA7, 0x5EED
PLACE, KIT, SPAWNED, DIED = 5, 1, 4000, 12_350


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(DATA, 0x100000, UC_PROT_ALL)
        emu.mem_map(HEAP, 0x400000, UC_PROT_ALL)
        emu.mem_map(STUBS, 0x1000, UC_PROT_ALL)
        emu.mem_map(CLIENT, 0x200000, UC_PROT_ALL)
        emu.mem_map(OPTIONS_HOLDER, 0x60000, UC_PROT_ALL)
        emu.mem_map(MATCH, 0x20000, UC_PROT_ALL)
        emu.mem_map(BIG_STACK, 0x100000, UC_PROT_ALL)
        self.heap, self.live, self.frees, self.queries, self.releases, self.log = HEAP, {}, [], [], [], []
        self.used, self.ticks = 900 << 20, 0
        self.stub_at = {}
        for n, name in enumerate(IAT):
            at = STUBS + 0x10 * n
            emu.mem_write(at, b"\xC3")
            self.word(IAT[name], at)
            self.stub_at[at] = name
        self.word(ALLOCATOR_SLOT, ALLOCATOR)
        for stub in [MALLOC, FREE, QUERY, RELEASE, TYPE_ID_FN, *self.stub_at]:
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
            variant = self.read(batch[0][2], 0x30) if count == 1 and batch[0][2] else None
            # the query copies the callback (the caller's room goes with its frame)
            self.queries.append({"batch": batch, "args": [arg(n) for n in range(1, 6)], "variant": variant,
                                 "callback": self.read(arg(1), 0x20), "variant_at": batch[0][2] if count else 0})
            self.ret(0)
        elif address == RELEASE:                    # ECX the object, EAX &its count
            self.releases.append((reg(UC_X86_REG_ECX), reg(UC_X86_REG_EAX)))
            self.ret(0)
        elif address == TYPE_ID_FN:                 # type_to_int<inventory_cooker_data *>: EAX
            self.ret(0, TYPE_ID)
        else:
            name = self.stub_at[address]
            if name == "memory":                    # (status): MEMORYSTATUSEX
                self.used += 3 << 20                # each look 3 MiB more in use
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

    def call(self, va, ecx=0, args=()):
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        for n, value in enumerate(args):
            self.word(sp + 4 + 4 * n, value)
        self.emu.reg_write(UC_X86_REG_ECX, ecx)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.started = (hide.SENTINEL,), False
        self.emu.emu_start(va, hide.SENTINEL, count=2_000_000)

    # ---- the made-up match -----------------------------------------------------------------
    def match(self):
        """A constructed profile (the game's constructor) with three kits' slot words, a game
        world, its game (dictionary, client, options) and match (spatial tree, core)."""
        self.emu.mem_write(PROFILE, bytes(PROFILE_SIZE + 0x100))
        self.call(0x2F73F0, ecx=PROFILE)
        self.emu.mem_write(PROFILE + 0x57, b"\x03")
        for kit in range(3):
            for slot in range(23):
                self.emu.mem_write(PROFILE + SLOTS + kit * KIT_STRIDE + 4 * slot,
                                   struct.pack("<HH", kit * 23 + slot, 100 + slot))
        for item in range(69):
            self.emu.mem_write(PROFILE + ITEMS + 0xB4 * item + 4, struct.pack("<H", item))
        self.word(WORLD + 0x9C, GAME)
        self.word(WORLD + 0x4D4C, MATCH)
        self.word(GAME + 0x53A4, DICTIONARY)
        self.word(GAME + 0x53AC, CLIENT)
        self.word(CLIENT + 0x1F1C48, OPTIONS_HOLDER)
        self.word(OPTIONS_HOLDER + 0xC + 0x577B0, SEED)
        self.word(MATCH + 0x130, SPATIAL)
        self.time(0)

    def time(self, now):
        self.word(MATCH + 0x1F8 + 0x18FC0, now)

    def player(self, held=WEAPON, vtable=WEAPON_VTABLE, slot=10, spawned=SPAWNED):
        """The dying player: place 5, kit 1, its life `spawned`, holding `held` (its
        inventory item at +10h in inventory slot `slot`)."""
        self.emu.mem_write(PLAYER, bytes(0x10000))
        self.emu.mem_write(PLAYER + 0x114, bytes([PLACE, KIT]))
        self.word(PLAYER + 0x128, held)
        self.word(PLAYER + 0x10C, INVENTORY)
        self.word(PLAYER + 0x324, DIED)
        self.word(PLAYER + 0x328, spawned)
        self.word(PLAYER + 0x6B8, PROFILE)
        self.word(PLAYER + 0xF474, WORLD)
        self.emu.mem_write(INVENTORY, bytes(0x200))
        self.emu.mem_write(WEAPON, bytes(0x100))
        self.word(WEAPON, vtable)
        if slot is not None:
            self.word(INVENTORY + 0x10C + 4 * slot, WEAPON + 0x10)

    def death(self):
        self.queries = []
        regs = {**hide.GAME, UC_X86_REG_ECX: PLAYER}
        after, sp = self.run(DEATH_SITE, (DEATH_AFTER,), hide.STACK + 0x9000, regs)
        assert self.stopped == DEATH_AFTER, hex(self.stopped or 0)
        assert sp == hide.STACK + 0x9000 - 8, hex(sp)          # the replayed push edi / push 1
        assert after[UC_X86_REG_EDI] == PLAYER                  # the replayed mov edi,ecx
        for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EBP, UC_X86_REG_ECX):
            assert after[r] == regs[r], (r, hex(after[r]))

    def complete(self, query, inventory, references=1):
        """The query's completion as dispatch_callbacks runs it: the boost function's
        invoker with its functor and a queries_result of one result."""
        callback = query["callback"]
        manager, = struct.unpack_from("<I", callback)
        assert manager == MANAGER_VTABLE | 1, hex(manager)
        assert self.read(MANAGER_VTABLE + 4) == INVOKER
        self.emu.mem_write(FUNCTOR, callback)
        self.word(RESULT + 0x38, 1 if inventory is not None else 0)
        self.word(RESULT + 0x12C, inventory or 0)
        if inventory:
            self.word(inventory + 0xD0, references)
        self.call(INVOKER, args=(FUNCTOR + 8, RESULT))

    def step(self, now):
        self.time(now)
        self.word(GAME + 0x53AC, 0)              # no client: the ragdoll's step has no places
        top = BIG_STACK + 0xF0000
        _, sp = self.run(STEP_SITE, (STEP_AFTER,), top, {**hide.GAME, UC_X86_REG_EBX: WORLD})
        assert self.stopped == STEP_AFTER and sp == top, (hex(self.stopped or 0), hex(sp))
        self.word(GAME + 0x53AC, CLIENT)

    def destroy(self):
        _, sp = self.run(DESTROY_SITE, (DESTROY_AFTER,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_ECX: PLAYER})
        assert self.stopped == DESTROY_AFTER


def slot_word(snapshot, kit, slot):
    at = SLOTS + kit * KIT_STRIDE + 4 * slot
    return struct.unpack_from("<HH", snapshot, at)


def main():
    original = open(sys.argv[1], 'rb').read()
    for va, hexbytes in ORIGINAL.items():
        at = hide.file_at(original, va)
        assert original[at:at + len(hexbytes) // 2].hex() == hexbytes, (hex(va), original[at:at + 6].hex())
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())

    # A death holding slot 10's gun: one query, the gun alone.
    r = Run(patched)
    r.match()
    r.player()
    source = r.read(PROFILE, PROFILE_SIZE)
    r.death()
    (q,) = r.queries
    (path, cls, user), = q["batch"]
    assert (path, cls) == (PATH, 0x41) and user == q["variant_at"]
    assert q["args"][1] == ALLOCATOR and q["args"][3:] == [0, 1], [hex(a) for a in q["args"]]
    vt, _, recipe = struct.unpack_from("<III", q["variant"])
    this, type_id = struct.unpack_from("<II", q["variant"], 0x28)
    assert (vt, this, type_id) == (VARIANT_VTABLE, user, TYPE_ID)
    assert recipe in r.live and r.live[recipe] == 0x1C
    words = struct.unpack("<6I", r.read(recipe, 0x18))
    copy = words[0]
    assert copy in r.live and r.live[copy] == PROFILE_SIZE and copy != PROFILE
    assert words[1] & 0xFF == KIT and words[2:] == (DICTIONARY, SPATIAL, WORLD, SEED), [hex(w) for w in words]
    assert r.read(recipe + 0x18, 1)[0] == 0
    copied = r.read(copy, PROFILE_SIZE)
    assert r.read(PROFILE, PROFILE_SIZE) == source, "the player's profile changed"
    for kit in range(3):
        for slot in range(23):
            want = slot_word(source, kit, slot)
            if kit == KIT and slot != 10 and slot not in AMMO:
                want = (0xFFFF, want[1])
            assert slot_word(copied, kit, slot) == want, (kit, slot)
    words = struct.unpack("<%dI" % (PROFILE_SIZE // 4), copied[:PROFILE_SIZE // 4 * 4])
    assert not [hex(4 * n) for n, w in enumerate(words) if PROFILE <= w < PROFILE + PROFILE_SIZE]
    assert any("pid 4242 pickup: asked for the gun of place 5 kit 1 slot 10 (death at 12350 ms)" in l
               for l in r.log), r.log
    print("ok a death holding slot 10's gun: one query of one inventory, the match's recipe, a deep copy of the "
          "profile keeping slot 10 and the ammunition; the boost callback; registers and stack kept")

    r.death()
    assert r.queries == [], "a replayed death asks again"
    print("ok the same life's death again (a rollback replaying it): no second cook")

    r.complete(q, COOKED)
    assert r.read(COOKED + 0xD0) == 2 and recipe in r.frees and copy in r.live
    r.step(DIED + 100)
    assert any("cook of the gun of place 5 kit 1 slot 10 (death at 12350 ms): made, inventory 0x%08x" % COOKED in l
               for l in r.log), r.log
    assert any("ms after it was asked for, address space 9" in l for l in r.log), r.log
    r.step(DIED + 7_900)
    assert r.read(COOKED + 0xD0) == 2, "freed inside the margin"
    r.step(DIED + 8_000)
    assert r.read(COOKED + 0xD0) == 1 and copy not in r.live and r.releases == []
    assert r.live == {}, {hex(k): v for k, v in r.live.items()}
    print("ok the completion through the game's boost invoker 0x202300: the gun kept (one reference), the recipe freed, logged made with its ms and memory; "
          "past the window and margin given back, every allocation freed once")

    # The last reference: the native release; a new life dies again: a new cook.
    r.player(spawned=SPAWNED + 9000)
    r.death()
    (q,) = r.queries
    r.complete(q, COOKED, references=0)
    r.destroy()
    assert r.releases == [(COOKED, COOKED + 0xD0)] and r.live == {}
    print("ok a new life's death cooks again; its donor destroyed: the gun's last reference released (0x0E0210)")

    # No gun to drop: nothing held, a battery, a weapon in no weapon slot.
    for name, player in (("nothing held", dict(held=0)), ("a battery", dict(vtable=BATTERY_VTABLE)),
                         ("a weapon in slot 8", dict(slot=8)), ("a weapon in no slot", dict(slot=None))):
        r = Run(patched)
        r.match()
        r.player(**player)
        r.death()
        assert r.queries == [] and r.live == {}, name
    print("ok nothing held, a battery, a weapon outside slots 7 and 10: no cook")

    # A failed cook: FAILED, all given back.
    r = Run(patched)
    r.match()
    r.player(slot=7)
    r.death()
    (q,) = r.queries
    r.complete(q, None)
    r.step(DIED + 50)
    assert r.live == {}, "the recipe and profile given back"
    assert any("slot 7 (death at 12350 ms): FAILED" in l for l in r.log), r.log
    print("ok an empty result: logged FAILED, nothing kept or leaked")

    # The donor destroyed while its gun cooks: abandoned; the completion gives all back.
    r = Run(patched)
    r.match()
    r.player()
    r.death()
    (q,) = r.queries
    r.destroy()
    assert r.live, "freed under the running cook"
    r.complete(q, COOKED)
    assert r.read(COOKED + 0xD0) == 1 and r.live == {} and r.releases == []
    print("ok the donor destroyed while its gun cooks: the completion gives the gun, recipe and profile back")


if __name__ == '__main__':
    main()
