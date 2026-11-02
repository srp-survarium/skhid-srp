#!/usr/bin/env python3
"""Patch 85 (weapon_pickup, stage 1: the instance; docs/notes/weapon_pickup.md):
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
  weapon_pickup::after_move) logs it made; kept while the body is in the world,
  never registered nor in hands, given back at the frame after base_player::remove(1) takes it out (0x0E0210
  at its last reference), the profile copy destroyed and freed;
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
# base_player::remove's callers' return addresses (fields_026g.tsv's image rows).
REMOVE_ON_RESPAWN = 0x3169A2                     # player::remove's return address in the respawn tick
PLAYER_REMOVE = 0x1F2870                        # player::remove, vtable 0x9FB35C +24h: calls base_player::remove
STEP_SITE, STEP_AFTER = 0x1F7C3C, 0x1F7C41
DESTROY_SITE, DESTROY_AFTER = 0x1F2100, 0x1F2106
MALLOC, FREE, QUERY, RELEASE, TYPE_ID_FN = 0xE4F90, 0x1539D0, 0xE5B60, 0xE0210, 0x205C60
INVOKER, MANAGER_VTABLE, DISPATCH = 0x202300, 0xA09374, 0xE5AB0
# tick_active_object 0x2D7650 (the game's weapon switch) and the weapon vtable's entries it asks:
# +0 can it be held (thiscall, ret 4), +4 (ret 4), +2Ch the update, +30h ready to switch.
TICK_ACTIVE, CAN_HOLD, VT4, VT_UPDATE, VT_READY = 0x2D7650, 0x301CA0, 0x2E3800, 0x2E0BA0, 0x2E2F00
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
CORE = MATCH + 0x1F8
PLACE, KIT, SPAWNED, DIED = 5, 1, 4000, 12_350
PICKED = (DIED + 1_000, PLACE, KIT, 1)          # the picker's pickup of the donor's slot-10 gun


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
        self.word(PLAYER + 0x120, CORE)
        self.word(PLAYER + 0xD0, 3)
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

    def destroy_player(self, player):
        """player::~player of `player` (player.destroy 0x1F2100)."""
        self.run(DESTROY_SITE, (DESTROY_AFTER,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_ECX: player})
        assert self.stopped == DESTROY_AFTER

    def destroy(self):
        _, sp = self.run(DESTROY_SITE, (DESTROY_AFTER,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_ECX: PLAYER})
        assert self.stopped == DESTROY_AFTER

    def remove(self, player, at=None, flag=1):
        """player::remove(flag) 0x1F2870 of `player` (at its tick time `at`), as the game's virtual call
        (vtable +24h) makes it, up to base_player::remove's hook (its call at 0x1F2879), from the respawn
        tick (0x3169A0): its body leaves the world, its drop stays."""
        if at is not None:
            self.word(player + 0x324, at)
        sp = hide.STACK + 0x9000
        self.word(sp, REMOVE_ON_RESPAWN)
        self.word(sp + 4, flag)
        self.run(PLAYER_REMOVE, (0x2DA796,), sp, {**hide.GAME, UC_X86_REG_ECX: player})
        assert self.stopped == 0x2DA796, hex(self.stopped or 0)


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
    assert any("pid 4242 pickup: asked for the gun of place 5 kit 1 slot 10: address space" in l
               for l in r.log), r.log
    print("ok a death holding slot 10's gun: one query of one inventory, the match's recipe, a deep copy of the "
          "profile keeping slot 10 and the ammunition; the boost callback; registers and stack kept")

    r.death()
    assert r.queries == [], "a replayed death asks again"
    print("ok the same life's death again (a rollback replaying it): no second cook")

    r.complete(q, COOKED)
    assert r.read(COOKED + 0xD0) == 2 and recipe in r.frees and copy in r.live
    r.step(DIED + 100)
    assert any("cook of the gun of place 5 kit 1 slot 10: made, inventory 0x%08x" % COOKED in l
               for l in r.log), r.log
    assert any("ms after it was asked for, address space 9" in l for l in r.log), r.log
    r.step(DIED + 60_000)
    assert r.read(COOKED + 0xD0) == 2, "freed while its body is in the world"
    respawn = DIED + 61_000
    r.remove(PLAYER, respawn)
    assert any("its body left the world: the drop stays, alone, the gun of place 5 kit 1 slot 10 of place 5 at %d ms"
               % respawn in l for l in r.log), r.log
    r.step(respawn + 600_000)
    assert r.read(COOKED + 0xD0) == 2, "freed after the respawn (the drop stays until the player dies again)"
    gone = respawn + 600_000                            # its next death, holding nothing: no drop now
    r.word(PLAYER + 0x128, 0)
    r.word(PLAYER + 0x328, respawn)
    r.word(PLAYER + 0x324, gone)
    r.death()
    assert r.queries == []
    r.step(gone + 1)
    assert r.read(COOKED + 0xD0) == 2, "freed inside a rollback's reach of its drop's end"
    r.step(gone + 10_000)
    assert r.read(COOKED + 0xD0) == 1 and copy not in r.live and r.releases == []
    assert r.live == {}, {hex(k): v for k, v in r.live.items()}
    print("ok the completion through the game's boost invoker 0x202300: the gun kept (one reference), the recipe freed, logged made with its ms and memory; "
          "kept while its body is in the world (60 s), past its respawn (10 min on: the drop stays until the player "
          "dies again), and 10 s past its next death, which ends it (a rollback can bring it back); never "
          "registered nor in hands, given back after; every allocation freed once")

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
    assert any("slot 7: FAILED" in l for l in r.log), r.log
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

    stage2(patched, original)
    bullet_record(patched, original)
    holster(patched)
    keep_and_reuse(patched)
    second_pickup(patched)
    two_slots(patched)
    ordinary_kit(patched)
    abc(patched)
    late_join(patched)
    late_join_after_player_made(patched)
    a_loaded_record_saves_the_same(patched)
    a_new_life(patched)
    the_end_gives_back_the_kit(patched)
    the_own_wear_comes_back(patched)
    no_cook_in_a_tick(patched)
    donor_left_then_a_late_join(patched)
    a_departed_donors_gun_is_cooked(patched)
    a_cancel_reopens_the_drop_everywhere(patched)
    an_open_drops_gun_is_not_idle(patched)
    crate_per_gun(patched)
    stage2b(patched)
    death_drop(patched)
    linger(patched)
    one_tick(patched)


# ---- stage 2a: the record, the pickup, the restore, the crate --------------------------------------

PICK = 0x37000000                           # the picker's objects
SAVE_SITE, SAVE_AFTER = 0x2DAF70, 0x2DAF76
LOAD_SITE, LOAD_AFTER = 0x2DB4CF, 0x2DB4D5
FILTER_SITE, FILTER_AFTER = 0x2D876F, 0x2D8774
REMOVE_SITE, REMOVE_AFTER = 0x2DA790, 0x2DA796
CAN_USE, CAN_USE_AFTER, CAN_USE_AVAILABLE = 0x2F809C, 0x2F80A2, 0x2F80B3
HINT, HINT_AFTER, HINT_AVAILABLE = 0x2F80E4, 0x2F80EA, 0x2F8116
RESUPPLY, RESUPPLY_AFTER = 0x2F81B8, 0x2F81BE
CLIENT_RESUPPLY, CLIENT_RESUPPLY_AFTER = 0x2954EF, 0x2954F5
NEW_SPANS = {SAVE_SITE: 6, LOAD_SITE: 6, REMOVE_SITE: 6, CAN_USE: 6, HINT: 6, RESUPPLY: 6, CLIENT_RESUPPLY: 6,
             0x2D7650: 5, 0x2D76D0: 6, 0x32C148: 7, 0x32BE12: 7, 0x32BDBA: 5}
WRITE_U8, DEACTIVATE, FORGET, SET_USER, INITIALIZE, ACTIVATE, ACTIVE_PLAYER = (
    0x1F3DE0, 0x244860, 0x3919E0, 0x2445E0, 0x2449E0, 0x244610, 0x2D0AB0)
SET_OWNER = 0x243590                        # inventory_item::set_owner: the item's +250h/+254h (ret 8)
CHORD = 1 << 29
PATTERNS = [v for v in range(64) if bin(v).count("1") >= 2][:20]
GUN_ID, BULLETS, CAPACITY = 4, 0xB0B0, 30
# The registry's reserved top (vostok::pickup::RegistryId::reserved): three ids a place from 195, slot 7's
# pick, slot 10's pick, the pending gun. The picker is place 3.
RESERVED = 0xFF - 3 * 20
def reserved(place, role):
    return RESERVED + 3 * place + {7: 0, 10: 1, "pending": 2}[role]
PICK_ID, PENDING_ID = reserved(3, 10), reserved(3, "pending")
CFG_CHILD, CFG_HAS = 0xF6B10, 0xF6C00


def command(target):
    return CHORD | PATTERNS[target] << 14


A_GUN = (PLACE, KIT, 1)                         # the donor's own gun: place 5 kit 1 slot 10


def record(drop=None, slot7=None, slot10=None, pending=None, wear7=0.0, wear10=0.0):
    """weapon_pickup::Record::encode: drop ('open' or 'taken', the slot bit it was held in, its recipe (place,
    kit, slot bit)[, alone: its body left the world]); each weapon slot's pick
    (taken ms, the recipe's place, kit, slot bit); the pickup waiting for its switch (its weapon slot 7
    or 10, its pick, the donor's (magazine, chamber) or None, and the place it was taken from, by default
    its recipe's)."""
    out = bytearray(RECORD)
    out[0] = 8
    if drop:
        state, held, (place, kit, slot), *alone = drop
        out[1] = {"open": 1, "taken": 2}[state] | held << 2 | (1 << 3 if alone and alone[0] else 0)
        out[28] = (place + 1) | kit << 5 | slot << 7
    for at, pick in ((2, slot7), (7, slot10)):
        if pick:
            taken, place, kit, slot = pick
            out[at] = (place + 1) | kit << 5 | slot << 7
            out[at + 1:at + 5] = struct.pack("<I", taken)
    if pending:
        slot, (taken, place, kit, slot_bit), magazine, *taken_from = pending
        out[12] = (1 if slot == 7 else 2) | (taken_from or [place])[0] << 2
        out[13] = (place + 1) | kit << 5 | slot_bit << 7
        out[14:18] = struct.pack("<I", taken)
        if magazine:
            out[18], out[19] = magazine[0], 1 | magazine[1] << 1
    # Each pick's own wear (the own gun's condition at the swap, the f32's bits).
    for at, pick, wear in ((20, slot7, wear7), (24, slot10, wear10)):
        if pick:
            out[at:at + 4] = struct.pack("<f", wear)
    return bytes(out)


RECORD = 29
# The record's synced part: all of it (version 8: the picks' own wear at 20..28, the drop's recipe at 28).
SYNCED = 29


class Pick(Run):
    """The match of stage 1 plus a picker (place 3, kit 0) holding its own gun in slot 10, a
    cooked inventory with the donor's gun in slot 10 and its ammunition in slot 11, the core's
    registry; the game's inventory set_item 0x2FC620, remove_item 0x2FC680 and set_owner
    0x243590 run as they are; the weapon's switch calls are recorded; the binary config's
    child and has (0xF6B10, 0xF6C00) answer from `cfg`, (table, key) -> node."""

    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(PICK, 0x100000, UC_PROT_ALL)
        self.next, self.written, self.native, self.put_away = PICK, bytearray(), [], []
        for at in (WRITE_U8, DEACTIVATE, FORGET, SET_USER, INITIALIZE, ACTIVATE, ACTIVE_PLAYER):
            emu.hook_add(UC_HOOK_CODE, self.natives, begin=at, end=at)
        self.cfg = {}
        for at in (CFG_CHILD, CFG_HAS):
            emu.hook_add(UC_HOOK_CODE, self.configs, begin=at, end=at)
        self.bare_ret = self.blk(16)
        emu.mem_write(self.bare_ret, b"\xC3")
        self.item_vtable = self.blk(0x40)
        self.word(self.item_vtable + 0x28, SET_OWNER)
        self.word(self.item_vtable + 0x34, self.bare_ret)
        # The game's dispatch_callbacks in a wait: completes the last query with `on_dispatch` (a
        # cooked inventory) through the boost invoker, as the game would.
        self.on_dispatch = None
        self.dispatch_stub = self.blk(32)
        emu.hook_add(UC_HOOK_CODE, self.dispatch, begin=DISPATCH, end=DISPATCH)
        for at in (CAN_HOLD, VT4, VT_UPDATE, VT_READY):
            emu.hook_add(UC_HOOK_CODE, self.weapon_asks, begin=at, end=at)
        self.ready = True

    def weapon_asks(self, emu, address, *_):
        """The weapon's answers to the switch: it can be held, it is ready to switch (unless `ready` is
        off: still holstering)."""
        self.ret(4 if address in (CAN_HOLD, VT4) else 0, 0 if address == VT_READY and not self.ready else 1)

    def switch(self, to):
        """The player's switch to the weapon `to` (its pending object, as a weapon key sets it) by the
        game's own tick_active_object: what is held after it."""
        self.ready = True
        self.word(self.picker + 0x12C, to)
        self.native = []
        self.emu.reg_write(UC_X86_REG_EAX, self.picker)
        self.call(TICK_ACTIVE, args=(0,))
        return self.read(self.picker + 0x128)

    def dispatch(self, emu, address, *_):
        if self.on_dispatch is None or not self.queries:
            return self.ret(0)
        callback, inventory = self.queries[-1]["callback"], self.on_dispatch
        self.on_dispatch = None
        self.emu.mem_write(FUNCTOR, callback)
        self.word(RESULT + 0x38, 1)
        self.word(RESULT + 0x12C, inventory)
        self.word(inventory + 0xD0, 1)
        at = self.dispatch_stub                     # push result; push functor; call invoker; add esp,8; ret
        code = b"\x68" + struct.pack("<I", RESULT) + b"\x68" + struct.pack("<I", FUNCTOR + 8)
        code += b"\xE8" + struct.pack("<i", INVOKER - (at + len(code) + 5)) + b"\x83\xC4\x08\xC3"
        emu.mem_write(at, code)
        emu.reg_write(UC_X86_REG_EIP, at)

    def blk(self, size):
        at, self.next = self.next, self.next + ((size + 0xF) & ~0xF)
        self.emu.mem_write(at, bytes(size))
        return at

    def natives(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 * n)
        ecx = emu.reg_read(UC_X86_REG_ECX)
        if address == WRITE_U8:                         # EAX &value, writer; ret 14h
            self.written.extend(self.read(emu.reg_read(UC_X86_REG_EAX), 1))
            self.ret(20)
        elif address == ACTIVE_PLAYER:                  # ESI world, CL place, EDI out: a reference added
            place, out = emu.reg_read(UC_X86_REG_ECX) & 0xFF, emu.reg_read(UC_X86_REG_EDI)
            player = self.roster.get(place, 0)
            self.word(out, player)
            if player:
                self.word(player + 0xD0, self.read(player + 0xD0) + 1)
            self.ret(0, out)
        elif address == FORGET:                         # EAX the animation player, EDX the object
            self.native.append(("forget", emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_EDX)))
            self.ret(0)
        elif address == INITIALIZE:                     # binds the retained stack as 2E2A99 does
            self.native.append(("initialize", ecx))
            slots, selected = self.read(ecx + 0x5AC), self.read(ecx + 0x5B1, 1)[0]
            inventory = self.read(ecx + 0x10 + 0x250)
            self.word(ecx + 0x538, self.read(inventory + 0x10C + 4 * self.read(slots + 4 * selected)))
            self.ret(0)
        else:
            name = {DEACTIVATE: "deactivate", SET_USER: "set_user", ACTIVATE: "activate"}[address]
            self.native.append((name, ecx, arg(1)))
            if address == DEACTIVATE:
                # Where the gun is when it is put away: its owner inventory and slot (item +250h, +254h).
                self.put_away.append((ecx, self.read(ecx + 0x10 + 0x250), self.read(ecx + 0x10 + 0x254)))
            self.ret(4)

    def configs(self, emu, address, *_):
        """EAX the table, EDI (child) or ESI (has) the key."""
        key = self.text(emu.reg_read(UC_X86_REG_EDI if address == CFG_CHILD else UC_X86_REG_ESI))
        child = self.cfg.get((emu.reg_read(UC_X86_REG_EAX), key))
        self.ret(0, child if address == CFG_CHILD else (1 if child else 0))

    def node(self, kind, data=0):
        """A binary config node: +0 its data, +14h its type (1 integer, 3 table, 5 text)."""
        at = self.blk(0x18)
        self.word(at, data)
        self.emu.mem_write(at + 0x14, struct.pack("<H", kind))
        return at

    def entry(self, item):
        """`item`'s dictionary entry and its config's root table (entry +114h -> +108h)."""
        entry, holder, root = self.blk(0x3A0), self.blk(0x110), self.node(3)
        self.word(item + 0x25C, entry)
        self.word(entry + 0x114, holder)
        self.word(holder + 0x108, root)
        return entry, root

    def ammo_config(self, item, dict_id, clip_size, required_bags):
        """An ammunition's entry (its dict id) and config: parameters/clip_size and
        parameters/required_bags, integers, as the item configs have them."""
        entry, root = self.entry(item)
        self.emu.mem_write(entry, struct.pack("<H", dict_id))
        parameters = self.node(3)
        self.cfg.update({(root, b"parameters"): parameters,
                         (parameters, b"clip_size"): self.node(1, clip_size),
                         (parameters, b"required_bags"): self.node(1, required_bags)})

    def gun(self, double_handed=1, ammo_slot=11, loaded=0, chamber=0):
        """A client weapon (active object, its inventory item +10h): one bound ammunition slot."""
        gun = self.blk(0x1820)
        self.word(gun, WEAPON_VTABLE)
        self.word(gun + 0x10, self.item_vtable)
        self.word(gun + 0x10 + 0xD0, 1)
        slots = self.blk(16)
        self.word(slots, ammo_slot)
        self.word(gun + 0x5AC, slots)
        self.emu.mem_write(gun + 0x5B0, bytes([1, 0]))
        self.emu.mem_write(gun + 0x5E4, bytes([CAPACITY, loaded]))
        self.emu.mem_write(gun + 0x5E9, bytes([double_handed]))
        self.emu.mem_write(gun + 0x5F0, bytes([1, chamber, 0, 0, 1]))
        return gun

    def stack(self, count):
        item = self.blk(0x280)
        self.word(item, self.item_vtable)
        self.word(item + 0xD0, 1)
        self.emu.mem_write(item + 0x260, struct.pack("<H", count))
        return item

    def setup(self, double_handed=1, own_double_handed=1):
        """The donor of stage 1 (place 5, kit 1, its gun in slot 10: 17 rounds and a chambered one)
        dies; its cook completes with the cooked inventory; the picker at place 3 (kit 0) holds its
        own gun in slot 10, bound to its ammunition in slot 8, slot 7 empty."""
        self.match()
        self.player()
        self.registry = self.blk(0x400)
        self.word(CORE + 0x18B4C, self.registry)
        self.word(CORE + 0x18B50, self.registry + 4 * 12)      # the game's own ids: 0..11
        self.emu.mem_write(WEAPON + 0x5E5, bytes([17]))
        self.emu.mem_write(WEAPON + 0x5F0, bytes([1, 1]))
        self.roster = {PLACE: PLAYER}
        self.death()
        (q,) = self.queries
        self.cooked = self.blk(0x200)
        self.new_gun = self.gun(double_handed=double_handed)
        self.new_ammo = self.stack(240)
        self.word(self.cooked + 0x10C + 4 * 10, self.new_gun + 0x10)
        self.word(self.cooked + 0x10C + 4 * 11, self.new_ammo)
        for item, slot in ((self.new_gun + 0x10, 10), (self.new_ammo, 11)):
            self.word(item + 0x250, self.cooked)
            self.word(item + 0x254, slot)
        self.complete(q, self.cooked)
        # Real configs: the own 5.45x39 7N10 (217: 30 a clip, a bag a clip), the borrowed PKP's
        # 7.62x54 7N13 (2204: 100 a clip, 2 bags a clip).
        self.ammo_config(self.new_ammo, 2204, 100, 2)
        c = self.add_picker(3, GUN_ID, own_double_handed)
        self.picker, self.inventory, self.own_gun, self.own_ammo = c.p, c.inventory, c.own_gun, c.own_ammo

    def add_picker(self, place, gun_id, own_double_handed=1):
        """A picker at `place` (kit 0) holding its own gun in slot 10 (registry id `gun_id`), bound to
        its ammunition in slot 8, slot 7 empty."""
        from types import SimpleNamespace
        p = self.blk(0x10000)
        self.roster[place] = p
        inventory = self.blk(0x200)
        own_gun = self.gun(double_handed=own_double_handed, ammo_slot=8, loaded=12)
        own_ammo = self.stack(90)
        self.ammo_config(own_ammo, 217, 30, 1)
        self.emu.mem_write(own_gun + 0xC, bytes([gun_id]))
        self.word(own_gun + 0x4, CORE)
        self.word(own_gun + 0x5A0, BULLETS)
        self.word(self.registry + 4 * gun_id, own_gun)
        for item, slot in ((own_gun + 0x10, 10), (own_ammo, 8)):
            self.word(inventory + 0x10C + 4 * slot, item)
            self.word(item + 0x250, inventory)
            self.word(item + 0x254, slot)
        self.word(own_gun + 0x538, own_ammo)
        self.word(p + 0xD0, 3)
        self.emu.mem_write(p + 0x114, bytes([place, 0]))
        self.word(p + 0x10C, inventory)
        self.word(p + 0x120, CORE)
        self.word(p + 0x128, own_gun)
        self.word(p + 0x12C, own_gun)
        self.word(p + 0x324, DIED + 1_000)
        self.word(p + 0x328, 1000)
        self.word(p + 0x6B8, PROFILE)
        self.word(p + 0xF474, WORLD)
        self.emu.mem_write(p + 0x33C, bytes([1, 0, 0, 1]))
        return SimpleNamespace(p=p, inventory=inventory, own_gun=own_gun, own_ammo=own_ammo)

    def new_life(self, place, spawned, died, double_handed=1):
        """The donor object at `place` in a new life `spawned` dies at `died` holding its gun in slot 10;
        its cook completes with a new cooked inventory: (cooked, gun, ammo)."""
        self.emu.mem_write(PLAYER + 0x114, bytes([place, KIT]))
        self.word(PLAYER + 0x328, spawned)
        self.word(PLAYER + 0x324, died)
        self.roster = {**{k: v for k, v in self.roster.items() if v != PLAYER}, place: PLAYER}
        self.death()
        (q,) = self.queries
        cooked, gun, ammo = self.cooked_gun(double_handed)
        self.complete(q, cooked)
        return cooked, gun, ammo

    def cooked_gun(self, double_handed=1):
        """A cooked inventory: a PKP-ammunition gun in slot 10, its ammunition in slot 11."""
        cooked, gun, ammo = self.blk(0x200), self.gun(double_handed=double_handed), self.stack(240)
        self.ammo_config(ammo, 2204, 100, 2)
        for item, slot in ((gun + 0x10, 10), (ammo, 11)):
            self.word(cooked + 0x10C + 4 * slot, item)
            self.word(item + 0x250, cooked)
            self.word(item + 0x254, slot)
        return cooked, gun, ammo

    def slot(self, inventory, n):
        return self.read(inventory + 0x10C + 4 * n)

    def second(self, double_handed=1, ammo_slot=12, stack=None):
        """A gun of the picker's in slot 7 (its ammunition in `ammo_slot`, `stack` there if given),
        bound to the world (registry id GUN_ID + 1)."""
        gun = self.gun(double_handed=double_handed, ammo_slot=ammo_slot)
        self.word(self.inventory + 0x10C + 4 * 7, gun + 0x10)
        self.word(gun + 0x10 + 0x250, self.inventory)
        self.word(gun + 0x10 + 0x254, 7)
        self.emu.mem_write(gun + 0xC, bytes([GUN_ID + 1]))
        self.word(gun + 0x4, CORE)
        self.word(self.registry + 4 * (GUN_ID + 1), gun)
        if stack:
            self.word(self.inventory + 0x10C + 4 * ammo_slot, stack)
            self.word(stack + 0x250, self.inventory)
            self.word(stack + 0x254, ammo_slot)
            self.word(gun + 0x538, stack)
        return gun

    def modifier(self, index, value):
        """The picker's kit 0's modifier `index` (profile +0F4h, a float each), as a skill sets it."""
        self.emu.mem_write(PROFILE + 0xF4 + 4 * index, struct.pack("<f", value))

    def lohook(self, site, after, regs, stops=None, sp=None):
        top = sp or hide.STACK + 0x9000
        out, esp = self.run(site, stops or (after,), top, {**hide.GAME, **regs})
        assert self.stopped in (stops or (after,)), (hex(site), hex(self.stopped or 0))
        return out, esp

    def pick(self, target=PLACE, now=None, picker=None, switch=True, actions=0):
        """The pickup command (with `actions` besides) played at the input's apply point, then
        (`switch`) the game's switch it waits for, the held gun ready (holstered) at the next
        tick_active_object. `applied`: the actions the game's tick then reads (+330h)."""
        p = picker or self.picker
        if now is not None:
            self.word(p + 0x324, now)
        self.native = []
        self.applied = self.set_input(p, self.read(p + 0x324), command(target) | actions) & ~(CHORD | 63 << 14)
        self.word(p + 0x330, 0)
        if switch:
            self.tick(player=p)

    def set_input(self, p, now, actions):
        """base_player::set_input as the game runs it: its entry (patch 78's carry.input reads the
        packet), its copy of the actions to +330h, the apply point (carry.filter: patch 85's command,
        patch 78's filter, patch 85's held fire). The actions the tick then reads."""
        packet = self.blk(0x40)
        self.word(packet + 4, actions)
        sp = hide.STACK + 0x9000
        self.word(sp + 4, now)
        self.word(sp + 8, packet)
        self.lohook(0x2D86E0, 0x2D86E6, {UC_X86_REG_ECX: p}, sp=sp)
        self.word(p + 0x330, actions)
        self.lohook(FILTER_SITE, FILTER_AFTER, {UC_X86_REG_EDI: p, UC_X86_REG_EAX: p + 0x32C})
        return self.read(p + 0x330)

    def save(self, player):
        """The record written: its synced part (the parked own kit after it zeroed, kept in `parked`)."""
        self.written = bytearray()
        self.lohook(SAVE_SITE, SAVE_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_ESI: 0x5A5A0000})
        assert len(self.written) == RECORD, len(self.written)
        self.parked = bytes(self.written[SYNCED:])
        return bytes(self.written[:SYNCED]) + bytes(RECORD - SYNCED)

    def load(self, player, data, size=None):
        bytes_at, reader = self.blk(0x40), self.blk(12)
        self.emu.mem_write(bytes_at, data)
        for off, value in ((0, bytes_at), (4, bytes_at), (8, len(data) if size is None else size)):
            self.word(reader + off, value)
        self.native = []
        self.lohook(LOAD_SITE, LOAD_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_ESI: reader})
        return self.read(reader + 4) - bytes_at

    def installed(self):
        """The borrowed gun is in the picker's slot 10 and its ammunition in slot 8, where the own
        gun's was ("your ammo is replaced"); the own gun in the cooked slot 10 and the own stack
        parked in the cooked slot 11, each item's owner and slot set by the game's set_owner; the
        gun bound to slot 8."""
        inv, cooked, gun, own = self.inventory, self.cooked, self.new_gun, self.own_gun
        assert self.slot(inv, 10) == gun + 0x10 and self.slot(inv, 8) == self.new_ammo
        assert self.slot(inv, 9) == 0 and self.slot(inv, 11) == 0
        assert self.slot(cooked, 10) == own + 0x10 and self.slot(cooked, 11) == self.own_ammo
        assert (self.read(gun + 0x10 + 0x250), self.read(gun + 0x10 + 0x254)) == (inv, 10)
        assert (self.read(self.new_ammo + 0x250), self.read(self.new_ammo + 0x254)) == (inv, 8)
        assert (self.read(own + 0x10 + 0x250), self.read(own + 0x10 + 0x254)) == (cooked, 10)
        assert (self.read(self.own_ammo + 0x250), self.read(self.own_ammo + 0x254)) == (cooked, 11)
        assert [self.read(x + 0xD0) for x in (gun + 0x10, own + 0x10, self.new_ammo, self.own_ammo)] == [1] * 4
        assert self.read(gun + 4) == CORE and self.read(gun + 0xC, 1)[0] == PICK_ID
        assert self.read(gun + 0x10 + 0x248) == CORE and self.read(self.new_ammo + 0x248) == CORE
        assert self.read(gun + 0x5A0) == BULLETS and self.read(gun + 0x5F4, 1)[0] == 0
        assert self.read(self.read(gun + 0x5AC)) == 8 and self.read(gun + 0x5B0, 2) == b"\1\0"
        assert self.read(self.registry + 4 * PICK_ID) == gun and self.read(self.registry + 4 * PENDING_ID) == 0
        assert self.read(self.registry + 4 * GUN_ID) == self.own_gun, "the own gun keeps its id and entry"
        self.in_hands()

    def in_hands(self, player=None):
        """The owner's invariant: a living picker always has a gun in its hands, held and pending set,
        pending the held gun or a registered picked gun."""
        p = player or self.picker
        held, pending = self.read(p + 0x128), self.read(p + 0x12C)
        assert held and self.read(held) == WEAPON_VTABLE, ("nothing in hands", hex(held))
        assert pending and self.read(pending) == WEAPON_VTABLE, ("no pending object", hex(pending))

    def reader(self, held_id, player=None):
        """What base_player::deserialize does after the record: held and pending from the state's ids
        through the registry (0x2DB626, 0x2DB796)."""
        p = player or self.picker
        held = self.read(self.registry + 4 * held_id)
        self.word(p + 0x128, held)
        self.word(p + 0x12C, held)

    def put_away_in_slot(self, gun, inventory=None, slot=10):
        """The gun put away as the native switch does it (deactivate(1), then forget_active_object), while
        it is still in the picker's slot: nothing of it left in the animation player when it is moved."""
        p = self.picker
        i = self.native.index(("deactivate", gun, 1))
        assert self.native[i + 1] == ("forget", p + 0x750, gun), self.native
        assert (gun, inventory or self.inventory, slot) in self.put_away, [tuple(map(hex, x)) for x in self.put_away]

    def tick(self, ready=True, now=None, player=None):
        """The game's tick_active_object of the picker as it is (its pending object not changed): the
        held gun `ready` to be deactivated or still holstering."""
        p = player or self.picker
        if now is not None:
            self.word(p + 0x324, now)
        self.ready = ready
        self.emu.reg_write(UC_X86_REG_EAX, p)
        self.call(TICK_ACTIVE, args=(0,))
        return self.read(p + 0x128)

    def restored(self):
        """Everything as before the pickup: the cooked inventory as cooked, the own gun and its
        ammunition back."""
        inv, cooked, gun, own = self.inventory, self.cooked, self.new_gun, self.own_gun
        assert self.slot(inv, 10) == own + 0x10 and self.slot(inv, 9) == 0 and self.slot(inv, 8) == self.own_ammo
        assert self.slot(cooked, 10) == gun + 0x10 and self.slot(cooked, 11) == self.new_ammo
        assert (self.read(own + 0x10 + 0x250), self.read(own + 0x10 + 0x254)) == (inv, 10)
        assert (self.read(self.own_ammo + 0x250), self.read(self.own_ammo + 0x254)) == (inv, 8)
        assert (self.read(gun + 0x10 + 0x250), self.read(gun + 0x10 + 0x254)) == (cooked, 10)
        assert (self.read(self.new_ammo + 0x250), self.read(self.new_ammo + 0x254)) == (cooked, 11)
        assert [self.read(x + 0xD0) for x in (gun + 0x10, own + 0x10, self.new_ammo, self.own_ammo)] == [1] * 4
        assert self.read(gun + 4) == 0 and self.read(gun + 0xC, 1)[0] == 0 and self.read(gun + 0x5A0) == 0
        assert self.read(gun + 0x10 + 0x248) == 0 and self.read(self.new_ammo + 0x248) == 0
        assert self.read(self.read(gun + 0x5AC)) == 11 and self.read(gun + 0x5B0, 2) == b"\1\0"
        assert self.read(self.registry + 4 * GUN_ID) == own
        assert self.read(self.registry + 4 * PICK_ID) == 0 and self.read(self.registry + 4 * PENDING_ID) == 0


def second_pickup(patched):
    """A pistol for a pistol: slot 10 holds a picked gun; the picker takes another place's drop into
    slot 10 again. The picked gun is let go (back in its cooked inventory with its ammunition), the
    new one comes in, the own gun and ammunition stay hidden (now in the second cooked inventory)
    and come back at the removal, which lets the second go."""
    r = Pick(patched)
    r.setup()
    r.pick()
    r.installed()
    first_gun, first_ammo, first = r.new_gun, r.new_ammo, r.cooked
    second, gun, ammo = r.new_life(6, SPAWNED + 20_000, DIED + 2_000)
    r.log.clear()
    r.pick(target=6, now=DIED + 2_500)
    inv, own = r.inventory, r.own_gun
    assert r.slot(inv, 10) == gun + 0x10 and r.slot(inv, 8) == ammo, [hex(r.slot(inv, n)) for n in (8, 10)]
    assert r.slot(second, 10) == own + 0x10 and r.slot(second, 11) == r.own_ammo, "the own gun and ammo hidden"
    assert r.slot(first, 10) == first_gun + 0x10 and r.slot(first, 11) == first_ammo, "the first picked gun let go"
    assert r.read(r.registry + 4 * PICK_ID) == gun and r.read(r.picker + 0x128) == gun
    assert r.read(r.registry + 4 * GUN_ID) == own and r.read(first_gun + 0xC, 1)[0] == 0
    assert r.native[0] == ("deactivate", first_gun, 1) and r.native[1][::2] == ("forget", first_gun), r.native
    assert r.save(r.picker) == record(slot10=(DIED + 2_500, 6, KIT, 1))
    assert any("let go, replaced: slot 10's picked gun (taken at 13350 ms" in l for l in r.log), r.log
    r.in_hands()
    # Its death: the respawn's removal puts the own gun and ammunition back.
    r.remove(r.picker)
    assert r.slot(inv, 10) == own + 0x10 and r.slot(inv, 8) == r.own_ammo
    assert r.slot(second, 10) == gun + 0x10 and r.slot(second, 11) == ammo
    assert r.read(r.registry + 4 * GUN_ID) == own and r.save(r.picker) == record()
    # Let go, both kept (never freed mid-match: bullets and the game's objects may point at them) ...
    r.step(DIED + 2_500 + 3_000)
    r.step(DIED + 2_500 + 300_000)
    assert r.read(second + 0xD0) == 2 and r.read(first + 0xD0) == 2
    # ... until their holder is destroyed (its departure or the match's end).
    r.destroy_player(r.picker)
    assert r.read(second + 0xD0) == 1 and r.read(first + 0xD0) == 1
    print("ok a pistol for a pistol (slot 10 picked twice): the picked gun let go for the new one, the own gun "
          "and ammunition hidden; the removal puts the own ones back; guns let go are kept (5 min on) until their "
          "holder is destroyed")


def two_slots(patched):
    """The terminator skill (modifier 25): a rifle picked into slot 10, a switch to slot 7, another
    rifle picked into slot 7, the game's switch between the two picked guns both ways. Both own
    guns share one ammunition stack (slot 8, the same calibre): slot 10's own gun leaves it to slot
    7's (the other gun binds it), so its pick's stack goes to a free slot; slot 7's own gun hides it
    with itself. The removal gives each slot its own gun and ammunition back."""
    r = Pick(patched)
    r.setup()
    r.modifier(25, 1.0)
    inv, own10, shared = r.inventory, r.own_gun, r.own_ammo
    own7 = r.second(ammo_slot=8)                       # slot 7's own rifle: the same stack in slot 8
    r.pick()
    gun10, ammo10, cooked10 = r.new_gun, r.new_ammo, r.cooked
    assert r.slot(inv, 10) == gun10 + 0x10 and r.slot(inv, 8) == shared and r.slot(inv, 9) == ammo10
    assert r.slot(cooked10, 10) == own10 + 0x10 and r.slot(cooked10, 11) == 0, "the shared stack left to slot 7"
    assert r.switch(own7) == own7, "the game's switch to slot 7"
    cooked7, gun7, ammo7 = r.new_life(6, SPAWNED + 20_000, DIED + 2_000)
    r.pick(target=6, now=DIED + 2_500)
    assert r.slot(inv, 7) == gun7 + 0x10 and r.slot(inv, 8) == ammo7 and r.slot(inv, 10) == gun10 + 0x10
    assert r.slot(cooked7, 10) == own7 + 0x10 and r.slot(cooked7, 11) == shared, "slot 7's own gun hides the stack"
    assert r.save(r.picker) == record(slot7=(DIED + 2_500, 6, KIT, 1), slot10=PICKED)
    # The game's own switch between the two picked guns, both ways.
    assert r.switch(gun10) == gun10 and ("deactivate", gun7, 1) in r.native and ("set_user", gun10, r.picker) in r.native
    assert r.switch(gun7) == gun7 and ("deactivate", gun10, 1) in r.native and ("activate", gun7, 1) in r.native
    # Death: each slot its own gun and ammunition.
    r.remove(r.picker)
    assert [r.slot(inv, n) for n in (7, 8, 9, 10)] == [own7 + 0x10, shared, 0, own10 + 0x10], \
        [hex(r.slot(inv, n)) for n in (7, 8, 9, 10)]
    assert r.slot(cooked10, 10) == gun10 + 0x10 and r.slot(cooked7, 10) == gun7 + 0x10
    assert r.read(r.registry + 4 * GUN_ID) == own10 and r.read(r.registry + 4 * (GUN_ID + 1)) == own7
    assert r.save(r.picker) == record()
    print("ok two picked rifles (modifier 25: slot 10, then slot 7), the game's switch between them both ways; "
          "each slot's own gun hidden with its ammunition, a shared stack left with the gun that still binds it, "
          "then hidden with the second; the removal gives both own guns and their ammunition back")


def ordinary_kit(patched):
    """A rifle in slot 7 and a pistol in slot 10, no skill: a rifle for the rifle, the game's switch to
    slot 10, a pistol for the pistol, the switch between the two picked guns; the removal gives both
    own guns and their ammunition back. Then the carry rule (two pistols yes, two rifles only with
    modifier 25): a pistol for the rifle in hands gives two pistols, taken."""
    r = Pick(patched)
    r.setup(own_double_handed=0)                        # slot 10: the own pistol, its stack in slot 8
    inv, pistol, pistol_ammo = r.inventory, r.own_gun, r.own_ammo
    rifle_ammo = r.stack(120)
    rifle = r.second(double_handed=1, ammo_slot=12, stack=rifle_ammo)
    assert r.switch(rifle) == rifle
    r.pick()                                            # the donor's rifle for the rifle
    gun7, cooked7 = r.new_gun, r.cooked
    assert r.slot(inv, 7) == gun7 + 0x10 and r.slot(cooked7, 10) == rifle + 0x10
    assert r.slot(cooked7, 11) == rifle_ammo and r.slot(inv, 12) == r.new_ammo
    assert r.switch(pistol) == pistol
    cooked10, gun10, ammo10 = r.new_life(6, SPAWNED + 20_000, DIED + 2_000, double_handed=0)
    r.pick(target=6, now=DIED + 2_500)                 # a pistol for the pistol
    assert r.slot(inv, 10) == gun10 + 0x10 and r.slot(cooked10, 10) == pistol + 0x10
    assert r.slot(cooked10, 11) == pistol_ammo and r.slot(inv, 8) == ammo10
    assert r.save(r.picker) == record(slot7=(DIED + 1_000, PLACE, KIT, 1), slot10=(DIED + 2_500, 6, KIT, 1))
    assert r.switch(gun7) == gun7 and r.switch(gun10) == gun10, "the game's switch between the picked guns"
    r.remove(r.picker)
    assert [r.slot(inv, n) for n in (7, 8, 10, 12)] == [rifle + 0x10, pistol_ammo, pistol + 0x10, rifle_ammo]
    assert r.save(r.picker) == record()
    print("ok the ordinary kit: a rifle for the rifle, a pistol for the pistol, the game's switch between the "
          "picked guns; the removal gives both own guns and their ammunition back")
    # Two pistols are fine: a pistol for the rifle in hands, the own pistol in slot 10.
    two = Pick(patched)
    two.setup(double_handed=0, own_double_handed=0)
    rifle = two.second(double_handed=1)
    assert two.switch(rifle) == rifle
    two.pick()
    assert two.slot(two.inventory, 7) == two.new_gun + 0x10, two.log
    print("ok a pistol for the rifle in hands beside the own pistol (two pistols): taken; a rifle for the pistol "
          "beside a rifle is refused without modifier 25 (the case above)")


def abc(patched):
    """The owner's A/B/C: A (place 5) dies, B (place 3) takes its gun; A respawns and dies again, C
    (place 4) takes the new one; a rollback undoing C's pickup (its record without it) gives the gun
    back to A's drop, and C's pickup played again takes it from there (no new cook)."""
    r = Pick(patched)
    r.setup()
    r.pick()
    r.installed()
    c = r.add_picker(4, GUN_ID + 2)
    r.remove(PLAYER, DIED + 8_000)
    cooked, gun, ammo = r.new_life(PLACE, SPAWNED + 20_000, DIED + 20_000)
    r.pick(picker=c.p, now=DIED + 21_000)
    assert r.slot(c.inventory, 10) == gun + 0x10 and r.slot(cooked, 10) == c.own_gun + 0x10
    assert r.slot(r.inventory, 10) == r.new_gun + 0x10, "B's gun is B's"
    assert r.save(c.p) == record(slot10=(DIED + 21_000, PLACE, KIT, 1)) and r.save(PLAYER) == record(("taken", 1, A_GUN))
    # A rollback to before C's pickup: C's record without it, A's drop open again.
    r.load(PLAYER, record(("open", 1, A_GUN)))
    r.load(c.p, record())
    assert r.slot(c.inventory, 10) == c.own_gun + 0x10 and r.slot(cooked, 10) == gun + 0x10
    r.queries = []
    r.word(c.p + 0x128, c.own_gun)
    r.word(c.p + 0x12C, c.own_gun)
    r.pick(picker=c.p, now=DIED + 21_000)
    assert r.queries == [] and r.slot(c.inventory, 10) == gun + 0x10, "taken again from A's drop"
    assert r.slot(r.inventory, 10) == r.new_gun + 0x10
    print("ok A/B/C: B keeps A's first gun, C takes A's next life's; a rollback undoing C's pickup puts the gun "
          "back in A's drop, and the pickup played again takes it from there")


def late_join(patched):
    """A loaded record naming a gun this client has none of, with no world load before it (a replay's
    key frame): cooked anew from its recipe (the profile of place 6's kit 1, slot 10) in the wait, then
    installed."""
    r = Pick(patched)
    r.setup()
    r.roster[6] = PLAYER
    cooked, gun, ammo = r.cooked_gun()
    r.on_dispatch = cooked
    r.queries = []
    r.load(r.picker, record(slot10=(DIED + 900, 6, KIT, 1)))
    assert len(r.queries) == 1 and r.slot(r.inventory, 10) == gun + 0x10, r.log
    assert r.slot(cooked, 10) == r.own_gun + 0x10
    assert r.native == [("set_user", gun, r.picker), ("initialize", gun)], ("a fresh cook: set_user + initialize", r.native)
    assert any("a loaded record: installed: slot 10's picked gun (taken at 13250 ms, the gun of place 6 kit 1 "
               "slot 10)" in l for l in r.log), r.log
    print("ok a loaded record naming a gun not on this client, no world load before it: cooked from its recipe "
          "in the wait (the game's dispatch), installed without the switch")


PLAYER_MADE_SITE, PLAYER_MADE_AFTER = 0x202DD0, 0x202DD6
WORLD_LOAD_SITE, WORLD_LOAD_AFTER = 0x27C0D0, 0x27C0D9      # world_load.unpacked, before the load call
GET_ACTIVE_PLAYER = 0x27A9C0


def world_with(states):
    """A world as the 0xE8 handler loads it: its time, the mask, each place's state (not animated),
    six empty sections."""
    out = bytearray(struct.pack("<II", DIED + 2_000, sum(1 << place for place in states)))
    for place in sorted(states):
        out += struct.pack("<H", len(states[place])) + states[place] + b"\0"
    return bytes(out + bytes(2 * 6))


def load_world(r, world):
    """process_dynamic_match_info at world_load.unpacked (patch 87's site, patch 85's preparation composed):
    the world in the frame's buffer {allocator, begin, capacity, size} at EBP-24h, EDI the client."""
    frame, at = r.blk(0x100) + 0x80, r.blk(len(world))
    r.emu.mem_write(at, world)
    for off, value in ((-0x24, 0), (-0x20, at), (-0x1C, 0x8000), (-0x18, len(world))):
        r.word(frame + off, value)
    r.lohook(WORLD_LOAD_SITE, WORLD_LOAD_AFTER, {UC_X86_REG_EDI: CLIENT, UC_X86_REG_EBP: frame, UC_X86_REG_ESI: at,
                                                 UC_X86_REG_EDX: len(world), UC_X86_REG_ECX: CORE})


def late_join_after_player_made(patched):
    """The late join's first world (0xE8) with its players made this frame (player_cook.ready, the flag the
    wait refuses on: a late join's install must not run inside a load): the gun a record names that this
    client never cooked is cooked at the 0xE8 handler before the load (world_load.unpacked, outside any tick
    and load: the wait there is not refused) and the record's reader installs it without a wait; the state
    then matches the record. The load alone (no world load before it: a replay's key frame) still refuses."""
    pick = (DIED + 900, 6, KIT, 1)
    loaded = record(slot10=pick)

    def joiner():
        r = Pick(patched)
        r.setup()
        r.roster[6] = PLAYER

        def active_player(emu, address, *_):          # thiscall: ECX the client, out, place; ret 8
            sp = emu.reg_read(UC_X86_REG_ESP)
            out, place = r.read(sp + 4), r.read(sp + 8)
            player = r.roster.get(place, 0)
            r.word(out, player)
            if player:
                r.word(player + 0xD0, r.read(player + 0xD0) + 1)
            r.ret(8, out)
        r.emu.hook_add(UC_HOOK_CODE, active_player, begin=GET_ACTIVE_PLAYER, end=GET_ACTIVE_PLAYER)
        r.lohook(PLAYER_MADE_SITE, PLAYER_MADE_AFTER, {})
        cooked, gun, ammo = r.cooked_gun()
        r.on_dispatch = cooked
        r.queries = []
        return r, cooked, gun

    # Before: the record read with the flag up refuses the wait; the slot keeps the own gun.
    r, cooked, gun = joiner()
    r.load(r.picker, loaded)
    assert r.slot(r.inventory, 10) == r.own_gun + 0x10, "installed with a player made this frame"
    assert any("waited for the gun of place 6 kit 1 slot 10" in l and "a player was made since the last move" in l
               for l in r.log), r.log
    assert any("a loaded record's pick NOT installed (the state will not match)" in l for l in r.log), r.log

    # The 0xE8: cooked before the load, then read with no cook and no wait.
    r, cooked, gun = joiner()
    load_world(r, world_with({3: b"\0\0" + loaded}))
    assert len(r.queries) == 1, ("one cook before the load", r.log)
    assert any("a world to load names the gun of place 6 kit 1 slot 10, which this client has no idle copy of" in l
               for l in r.log), r.log
    assert any("waited for the gun of place 6 kit 1 slot 10" in l and "made" in l for l in r.log), r.log
    r.queries = []
    r.load(r.picker, loaded)
    assert r.queries == [] and r.slot(r.inventory, 10) == gun + 0x10, r.log
    assert r.slot(cooked, 10) == r.own_gun + 0x10
    assert r.native == [("set_user", gun, r.picker), ("initialize", gun)], ("a fresh cook: set_user + initialize", r.native)
    assert not any("NOT installed" in l for l in r.log), r.log
    assert r.save(r.picker) == loaded, "the state matches the record"
    # A world whose records name only guns this client has: nothing cooked.
    r.queries = []
    load_world(r, world_with({3: b"\0\0" + loaded}))
    assert r.queries == [], r.log
    # Several guns named, none made in time: one deadline for them all (2 s from the first wait, each look of
    # the clock a quarter of a second), each logged loudly; the reader then waits for its own (refused here).
    r, cooked, gun = joiner()
    r.on_dispatch = None
    r.roster[7] = PLAYER
    several = record(slot7=(DIED + 800, 7, KIT, 1), slot10=pick)
    load_world(r, world_with({3: b"\0\0" + several}))
    assert len(r.queries) == 2, r.log
    import re
    waits = [int(m.group(1)) for l in r.log for m in [re.search(r"waited for the gun of place \d+ kit 1 slot 10: "
                                                                 r"(\d+) ms, not made in time", l)] if m]
    held_for = sum(waits) / 1000
    # 2 s for the first; the second at the deadline at once (its own clock looks only: a cap each would be 4 s).
    assert len(waits) == 2 and waits[0] >= 2000 and held_for <= 2.5, ("one deadline for all", waits, r.log)
    assert sum("NOT made before the world's load: the gun of place" in l and "not made in time" in l
               for l in r.log) == 2, r.log
    print("ok several guns named and none made: one deadline for the 0xE8 handler (%.2f s waited in all, not 2 s "
          "a gun), each logged NOT made before the load" % held_for)
    print("ok a late join's first world with its players made this frame (player_cook.ready): the gun a record "
          "names that this client never cooked is cooked at the 0xE8 handler before the load, the reader installs "
          "it with no wait (fresh: set_user + initialize), the state saved is the record; a world naming only kept "
          "guns cooks nothing; the record read alone with the flag up still refuses (as before)")


def a_loaded_record_saves_the_same(patched):
    """No byte of a picker's state depends on the game that writes it: the record (29 bytes, version 8) has
    of the own kit a pick sets aside only the own gun's wear at the swap, which every game read alike, and nothing
    of its counts (the live 0x1c0ebd31: joiners wrote zeros in the old 28-byte block
    and were dropped). A picker's state saved, loaded by a fresh client (a late join) whose own kit differs (a full
    magazine and stack of its own), saved again: byte-identical; a rollback's load then save: identical. The kit
    set aside is the same hidden in both games (no magazine, each stack as the kit gives it)."""
    r = Pick(patched)
    r.setup()
    r.emu.mem_write(r.own_gun + 0x5E5, bytes([9]))
    r.emu.mem_write(r.own_ammo + 0x260, struct.pack("<H", 40))
    r.pick()
    r.installed()
    r.save(r.picker)
    full = bytes(r.written)
    assert len(full) == RECORD == 29 and full[0] == 8, full.hex()

    j = Pick(patched)
    j.setup()
    j.emu.mem_write(j.own_gun + 0x5E5, bytes([30]))
    j.emu.mem_write(j.own_ammo + 0x260, struct.pack("<H", 120))
    j.load(j.picker, full)
    assert j.slot(j.inventory, 10) == j.new_gun + 0x10, j.log
    j.save(j.picker)
    assert bytes(j.written) == full, ("a loading client saves what it loaded", bytes(j.written).hex(), full.hex())
    hidden = lambda g: (g.read(g.own_gun + 0x5E5, 1), g.read(g.own_gun + 0x5F1, 1), g.read(g.own_ammo + 0x260, 2))
    assert hidden(r) == hidden(j) == (b"\0", b"\0", struct.pack("<H", 108)), (hidden(r), hidden(j))
    assert not any("NOT" in l for l in j.log), j.log

    # A rollback's load (the pick stays installed) over a hidden own kit changed since: the loaded bytes again.
    r.emu.mem_write(r.own_ammo + 0x260, struct.pack("<H", 75))
    r.load(r.picker, full)
    r.save(r.picker)
    assert bytes(r.written) == full, ("a rollback saves what it loaded", bytes(r.written).hex(), full.hex())
    print("ok a picker's state is no game's own: saved (29-byte record, version 8), loaded by a late joiner with "
          "its own full kit, saved again: byte-identical; a rollback's load and save too; the kit set aside hidden "
          "alike in both games (no magazine, each stack as the kit gives it, 108)")


def a_new_life(patched):
    """A picked gun starts its pick as a spawn starts a kit's weapons, whatever the copy did before: a kept copy
    re-used (its seed, last tick, recoil, aim and fire mode from its last holder) and a fresh cook in another game
    give the same bytes +5A4h..+60Fh at the switch; the seed is the spawn hook's of the picker's place, kit, the
    slot and the switch's time, not the constructor's 1 (the live 0x1c0ebd31: 1 against 0x442aa381)."""
    def picked(dirty):
        r = Pick(patched)
        r.setup()
        if dirty:
            g = r.new_gun
            r.word(g + 0x5BC, 0x442AA381)
            r.word(g + 0x5A4, 123_450)
            r.word(g + 0x5A8, 77)
            r.emu.mem_write(g + 0x5E6, bytes([2, 0, 1]))
            r.emu.mem_write(g + 0x5FC, struct.pack("<4f", 0.5, -0.25, 1.0, 2.0))
            r.word(g + 0x60C, 5)
            r.emu.mem_write(g + 0x5F5, b"\1")
        r.pick()
        r.installed()
        return r.read(r.new_gun + 0x5A4, 0x6C), r
    fresh, r = picked(False)
    reused, _ = picked(True)
    assert fresh == reused, ("the same pick from a fresh and a re-used copy", fresh.hex(), reused.hex())
    seed = struct.unpack_from("<I", fresh, 0x5BC - 0x5A4)[0]
    key = ((3 << 16 | 0 << 8 | 10) * 0x85EBCA6B) & 0xFFFFFFFF
    h = key ^ ((r.read(r.picker + 0x324) * 0x9E3779B1) & 0xFFFFFFFF)
    assert seed == (h ^ h >> 15) % 0x7FFFFFFE + 1 and seed != 1, hex(seed)
    assert struct.unpack_from("<I", fresh, 0) == (0xFFFFFFFF,), "the last tick of a spawn"
    print("ok a picked gun starts a new life at its switch: a re-used copy (seed 0x442aa381, last tick, recoil, aim, "
          "fire mode left) and a fresh cook give the same +5A4h..+60Fh; the seed the spawn hook's (%#x)" % seed)


UNLOAD = 0x2FD2D0                           # inventory::unload_to_profile: AL kit, inventory, profile; ret 8
STORE_8 = PROFILE + ITEMS + 0xB4 * 8 + 8    # amount_in_storage of the kit 0 slot 8's item (its word: item 8)


STORE_10 = PROFILE + ITEMS + 0xB4 * 10 + 8  # the kit 0 slot 10 gun's profile item: its condition x 1e6


def end_of_pick(patched, hidden=None, refused=False, own_wear=0.93, joins=None, pick=True):
    """A picker (place 3, kit 0, 4000 rounds of its slot-8 ammunition in store) takes the donor's gun over its
    own (its own slot-8 stack set aside), the hidden own stack then `hidden` rounds in this game (a game's own
    count: a late joiner's made-up kit, or whatever made it differ), the restore `refused` in this game or not;
    then base_player::remove(1) (the patch's hook) and the native unload (0x2FD2D0) as remove runs it. The store
    after, the picker's slot 8 and its slot-10 gun's magazine and chamber."""
    r = Pick(patched)
    r.setup()
    r.word(STORE_8, 4_000)
    # Kit 0 as a real kit is: no item in the slots its inventory leaves empty (the unload reads every named one).
    for slot in set(range(23)) - {8, 10}:
        r.emu.mem_write(PROFILE + SLOTS + 4 * slot, struct.pack("<H", 0xFFFF))
    for stack in (r.own_ammo, r.new_ammo):         # stack items (entry +22Ah): the unload refunds their count
        r.emu.mem_write(r.read(stack + 0x25C) + 0x22A, b"\1")
    r.emu.mem_write(r.own_gun + 0x5E5, bytes([9]))
    r.emu.mem_write(r.own_ammo + 0x260, struct.pack("<H", 90))
    r.emu.mem_write(r.own_gun + 0x270, struct.pack("<f", own_wear))
    r.word(STORE_10, 0)
    if not pick:                                # the same life without a pickup, its kit as the spawn gave it
        r.emu.mem_write(r.own_gun + 0x5E5, bytes([30]))
        r.emu.mem_write(r.own_ammo + 0x260, struct.pack("<H", 108 - 30))
    elif joins is None:
        r.pick()
        r.installed()
    else:                                       # a game that loads the picker's record (a late join, a resync)
        r.load(r.picker, joins)
        r.reader(PICK_ID)
        r.installed()
    saved = r.save(r.picker)
    if hidden is not None:
        r.emu.mem_write(r.own_ammo + 0x260, struct.pack("<H", hidden))
    if refused:
        r.word(r.cooked + 0x10C + 4 * 10, r.stack(1))
    r.remove(r.picker)
    gun = r.slot(r.inventory, 10) - 0x10
    in_slots = (r.read(r.slot(r.inventory, 8) + 0x260, 2), r.read(gun + 0x5E5, 1), r.read(gun + 0x5F1, 1))
    r.emu.reg_write(UC_X86_REG_EAX, 0)
    try:
        r.call(UNLOAD, args=(r.inventory, PROFILE))
    except Exception:
        raise AssertionError("the native unload stopped at %#x" % r.emu.reg_read(UC_X86_REG_EIP))
    r.saved = saved
    r.store_10 = r.read(STORE_10)
    return r.read(STORE_8), in_slots, r


def the_own_wear_comes_back(patched):
    """The own gun's wear is no game's own at a pick's end: the record keeps it from the swap (the world carried
    it until then) and the end gives it back, whatever gun is in the slot. A game that loaded the record over its
    own made-up gun (wear 0.99, a late join's static info) and a game whose restore was refused (the borrowed gun
    left there) unload the same condition into the profile item as the picker's game (0.93): the release's
    probe had 930000 against 990000."""
    _, _, picker = end_of_pick(patched)
    rec = picker.saved
    assert rec[24:28] == struct.pack("<f", 0.93), rec.hex()
    _, _, joiner = end_of_pick(patched, own_wear=0.99, joins=rec)
    _, _, refused = end_of_pick(patched, refused=True)
    assert joiner.saved == rec, (joiner.saved.hex(), rec.hex())
    stores = (picker.store_10, joiner.store_10, refused.store_10)
    assert stores[0] == stores[1] == stores[2] and abs(stores[0] - 930_000) <= 1, stores
    print("ok the own gun's wear comes back from the record at a pick's end: the picker's game, a joiner over its own "
          "0.99 gun and a refused restore all unload %d into the profile item" % stores[0])


def the_end_gives_back_the_kit(patched):
    """At a pick's end (base_player::remove(1)) the own kit set aside comes back as the kit gives it (each own
    stack's slot at its allowance, the gun empty): neither charged nor refunded, the same store as the same life
    without a pickup (its kit as the spawn gave it: 30 in the gun, 78 in slot 8, the kit's 108), in every game
    alike, whatever objects are there. Restored or refused, with any hidden count (the live 0x1c0ebd31: the donor's
    game refunded 14, and in another pick 120 rounds less), the store is the baseline's."""
    baseline = end_of_pick(patched, pick=False)[0]
    assert baseline == 4_108, baseline
    (store, slots, _), (refused_store, refused_slots, f) = end_of_pick(patched), end_of_pick(patched, refused=True)
    assert any("FAILED to restore" in l for l in f.log), f.log
    assert slots == refused_slots == (struct.pack("<H", 108), b"\0", b"\0"), (slots, refused_slots)
    assert store == refused_store == baseline, (store, refused_store, baseline)
    # The two live cases: a hidden stack of 90 against 76 (14 rounds) and of 120 against 0 (120).
    for a, b in ((90, 76), (120, 0)):
        assert end_of_pick(patched, a)[0] == end_of_pick(patched, b)[0] == baseline, (a, b)
    print("ok a pick's end gives the kit back, neither charged nor refunded, in every game alike: restored or "
          "refused, the unload finds slot 8 at the kit's 108 and the slot-10 gun empty; the store %d is the same "
          "life's without a pickup; the live cases (hidden 90 vs 76, 120 vs 0) end with it too" % baseline)


def no_cook_in_a_tick(patched):
    """No client cooks or waits for a drop's gun in a tick, where a refusal (a player made this frame, the 2 s cap)
    would be its own: a rollback across the donor's next death (it ended the drop, the late pickup replayed
    before it) finds the copy its drop's again, and a late joiner's first world with an open drop
    has its gun cooked before the load (outside the tick); each pickup then takes it with no query."""
    # The rollback: the removal, the frame's step, the load of the donor's state before it, the pickup replayed.
    r = Pick(patched)
    r.setup()
    gone = DIED + 8_000
    r.remove(PLAYER, gone)
    unarmed_death(r, gone)
    r.step(gone + 100)
    r.load(PLAYER, record(drop=("open", 1, A_GUN)))
    r.lohook(PLAYER_MADE_SITE, PLAYER_MADE_AFTER, {})
    r.queries = []
    r.pick(now=gone - 200)
    assert r.queries == [], ("cooked in a tick", r.log)
    r.installed()
    print("ok a rollback across the donor's next death: its drop's copy kept and its drop's again, the replayed pickup "
          "takes it with no cook nor wait (a player made this frame)")

    # The late joiner: the donor's open drop in its first world, cooked before the load.
    j = Pick(patched)
    j.setup()
    j.remove(PLAYER, gone)
    unarmed_death(j, gone)
    j.step(gone + 20_000)                       # this client's copy of that death gone: a death it never saw
    j.emu.mem_write(PLAYER + 0x328, struct.pack("<I", SPAWNED + 30_000))

    def active_player(emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        out, place = j.read(sp + 4), j.read(sp + 8)
        player = j.roster.get(place, 0)
        j.word(out, player)
        if player:
            j.word(player + 0xD0, j.read(player + 0xD0) + 1)
        j.ret(8, out)
    j.emu.hook_add(UC_HOOK_CODE, active_player, begin=GET_ACTIVE_PLAYER, end=GET_ACTIVE_PLAYER)
    j.lohook(PLAYER_MADE_SITE, PLAYER_MADE_AFTER, {})
    cooked, gun, _ = j.cooked_gun()
    j.on_dispatch, j.queries = cooked, []
    opened = record(drop=("open", 1, A_GUN))
    load_world(j, world_with({PLACE: b"\0\0" + opened}))
    assert len(j.queries) == 1 and any("a world to load has the open drop" in l for l in j.log), j.log
    j.load(PLAYER, opened)
    j.queries = []
    j.cooked, j.new_gun = cooked, gun
    j.new_ammo = j.slot(cooked, 11)
    j.pick(now=gone + 20_100)
    assert j.queries == [] and j.slot(j.inventory, 10) == gun + 0x10, j.log
    print("ok a late joiner's first world with an open drop it never saw: its gun cooked before the load, the pickup "
          "takes it with no cook nor wait in the tick")


def donor_left_then_a_late_join(patched):
    """The donor leaves while B holds its gun, then a game joins (late join). The departure keeps the place and its
    profile; the joiner has that place's players from the static info, so it cooks the same item, but its wear is
    the static info's (the world carries the profile's changing part only for the places in it): 1.0 where the
    games that saw the death have the donor's (0.7 here). A held gun's wear comes with the world; a pickup still
    waiting for its switch at the join is not in the world, so at its swap the picked gun takes a constant
    condition (unworn, 1.0) in every game. Without that, the joiner's gun and the others' differ there."""
    cond = lambda g: bytes(g.emu.mem_read(g.new_gun + 0x10 + 0x260, 4))
    # Held: the donor's players destroyed in the game that stays; B keeps the gun; the joiner installs it.
    r = Pick(patched)
    r.setup()
    r.emu.mem_write(r.new_gun + 0x10 + 0x260, struct.pack("<f", 0.7))
    r.pick()
    r.installed()
    r.destroy_player(PLAYER)
    r.installed()
    held = r.save(r.picker)
    j = Pick(patched)
    j.setup()
    j.load(j.picker, held)
    assert j.slot(j.inventory, 10) == j.new_gun + 0x10 and j.save(j.picker) == held, j.log
    # Pending at the join: the swap gives both games the same condition.
    r = Pick(patched)
    r.setup()
    r.emu.mem_write(r.new_gun + 0x10 + 0x260, struct.pack("<f", 0.7))
    r.pick(switch=False)
    waiting = r.save(r.picker)
    j = Pick(patched)
    j.setup()
    j.emu.mem_write(j.new_gun + 0x10 + 0x260, struct.pack("<f", 1.0))
    j.load(j.picker, waiting)
    j.word(j.picker + 0x12C, j.read(j.registry + 4 * PENDING_ID))     # the reader resolves the pending id
    r.tick()
    j.tick()
    r.installed()
    j.installed()
    assert cond(r) == cond(j) == struct.pack("<f", 1.0), (cond(r).hex(), cond(j).hex())
    assert r.save(r.picker) == j.save(j.picker)
    print("ok the donor leaves while B holds its gun, then a late join: the joiner installs the held gun from the "
          "static info's profile and saves the same; a pickup waiting for its switch at the join gives both games "
          "an unworn gun at the swap (1.0, the donor's 0.7 and the static 1.0 alike)")


def a_departed_donors_gun_is_cooked(patched):
    """A record names the gun of a place whose player left (its bit of core+18FD8h clear: the lookup by place,
    core_active_player 0x2D0AB0, refuses it in every game). The place keeps its players and its profile (a
    returning player gets them again: the static info is fixed per match), so a game that has no copy cooks the
    recipe from the place's kit player in the core's place table (core+18458h, 30h a place): a late joiner's world
    load (before the load) and a resync's record (at its reader) both install it."""
    pick = (DIED + 900, 6, KIT, 1)
    loaded = record(slot10=pick)

    def departed(g):
        """Place 6 departed: not in the world (the lookup gives none), its kit players in the place table."""
        places = g.blk(0x30 * 8)
        g.word(CORE + 0x18458, places)
        g.word(CORE + 0x1845C, places + 0x30 * 8)
        g.word(places + 0x30 * 6 + 4 * KIT, PLAYER)
        g.roster.pop(6, None)
        g.word(CLIENT + 4, g.picker)            # the local player, whose world the table is read in

    # A late joiner's first world.
    j = Pick(patched)
    j.setup()
    departed(j)

    def active_player(emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        out, place = j.read(sp + 4), j.read(sp + 8)
        player = j.roster.get(place, 0)
        j.word(out, player)
        if player:
            j.word(player + 0xD0, j.read(player + 0xD0) + 1)
        j.ret(8, out)
    j.emu.hook_add(UC_HOOK_CODE, active_player, begin=GET_ACTIVE_PLAYER, end=GET_ACTIVE_PLAYER)
    cooked, gun, _ = j.cooked_gun()
    j.on_dispatch, j.queries = cooked, []
    load_world(j, world_with({3: b"\0\0" + loaded}))
    assert len(j.queries) == 1, ("the departed place's gun cooked before the load", j.log)
    j.load(j.picker, loaded)
    assert j.slot(j.inventory, 10) == gun + 0x10 and not any("NOT installed" in l for l in j.log), j.log
    assert j.save(j.picker) == loaded
    # A resync's record read with no world load before it (its reader cooks it, no player made this frame).
    r = Pick(patched)
    r.setup()
    departed(r)
    cooked, gun, _ = r.cooked_gun()
    r.on_dispatch, r.queries = cooked, []
    r.load(r.picker, loaded)
    assert len(r.queries) == 1 and r.slot(r.inventory, 10) == gun + 0x10, r.log
    print("ok a departed donor's gun (its place out of the world, the lookup by place refusing it): cooked from the "
          "place's kit player in the core's place table, by a late joiner before its load and by a resync's reader; "
          "installed, the state saved the record's")


def a_cancel_reopens_the_drop_everywhere(patched):
    """A pickup waiting for its switch is cancelled (the picker's death): its drop is open again in every game,
    from the synced state alone (the donor's record taken, its life older than the pickup), not from where this
    game's copy is marked to lie: a game whose copy came from a loaded record (a late joiner, a resync) had it
    marked gone, and there the drop stayed taken."""
    def died(g):
        g.lohook(DEATH_SITE, DEATH_AFTER, {UC_X86_REG_ECX: g.picker})
    r = Pick(patched)
    r.setup()
    r.pick(switch=False)
    waiting = r.save(r.picker)
    died(r)
    # The other game: its copy marked gone (as a loaded record's), the donor's record taken, the pending loaded.
    j = Pick(patched)
    j.setup()
    j.remove(PLAYER, DIED + 500)
    j.load(PLAYER, record(("taken", 1, A_GUN)))
    j.load(j.picker, waiting)
    j.word(j.picker + 0x12C, j.read(j.registry + 4 * PENDING_ID))
    died(j)
    opened = record(drop=("open", 1, A_GUN))
    assert r.save(PLAYER) == opened and j.save(PLAYER) == opened, (r.save(PLAYER).hex(), j.save(PLAYER).hex())
    print("ok a cancelled pickup (the picker's death) opens its drop again in every game, a game whose copy came "
          "from a loaded record too (the donor's record and life decide, not the copy's local mark)")


def an_open_drops_gun_is_not_idle(patched):
    """A holds a pick of D's gun and D, dead again with the same kit and slot, has its drop open (the same recipe).
    A late joiner whose only copy of that recipe lies in D's open drop must not count it as idle for A's pick: it
    cooks another before the load, A's reader takes that one, and D's drop keeps its gun (its next pickup then
    takes it with no cook in the tick)."""
    j = Pick(patched)
    j.setup()                                   # the joiner's copy of the recipe: D's open drop's (place 5)

    def active_player(emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        out, place = j.read(sp + 4), j.read(sp + 8)
        player = j.roster.get(place, 0)
        j.word(out, player)
        if player:
            j.word(player + 0xD0, j.read(player + 0xD0) + 1)
        j.ret(8, out)
    j.emu.hook_add(UC_HOOK_CODE, active_player, begin=GET_ACTIVE_PLAYER, end=GET_ACTIVE_PLAYER)
    a_holds = record(slot10=(DIED - 5_000, PLACE, KIT, 1))
    d_open = record(drop=("open", 1, A_GUN))
    cooked, gun, _ = j.cooked_gun()
    j.on_dispatch, j.queries = cooked, []
    load_world(j, world_with({3: b"\0\0" + a_holds, PLACE: b"\0\0" + d_open}))
    assert len(j.queries) == 1, ("another copy cooked for A's pick before the load", j.log)
    j.load(j.picker, a_holds)
    j.reader(PICK_ID)
    j.load(PLAYER, d_open)
    assert j.slot(j.inventory, 10) == gun + 0x10, "A's reader took the cooked copy"
    assert j.slot(j.cooked, 10) == j.new_gun + 0x10, "D's drop kept its gun"
    print("ok A holds a pick of D's gun while D's drop of the same recipe is open: a late joiner cooks another copy "
          "for A before the load and A's reader takes it; D's drop keeps its gun")


def crate_per_gun(patched):
    """The owner's 83f41 case: the picker's slot-7 gun's ammunition 70 (50 a clip, a bag) has the kit's
    100 rounds in slot 8 (2 bags); the borrowed gun binds three stacks of ammunition 7 (30 a clip, a
    bag), placed in slots 8, 9 and 11, with 13 rounds in it. The game's rule: 2 bags of ammunition 7,
    60 rounds, 13 in the gun: slot 8 gets 47, slots 9 and 11 nothing."""
    r = Pick(patched)
    r.setup()
    extra = [r.stack(0), r.stack(0)]
    for item, slot in zip(extra, (12, 19)):
        r.word(r.cooked + 0x10C + 4 * slot, item)
        r.word(item + 0x250, r.cooked)
        r.word(item + 0x254, slot)
    slots = r.read(r.new_gun + 0x5AC)
    r.word(slots + 4, 12)
    r.word(slots + 8, 19)
    r.emu.mem_write(r.new_gun + 0x5B0, bytes([3, 0]))
    for stack in (r.new_ammo, *extra):
        r.ammo_config(stack, 7, 30, 1)
    r.ammo_config(r.own_ammo, 70, 50, 1)
    r.emu.mem_write(PROFILE + SLOTS + 4 * 8, struct.pack("<HH", 8, 100))
    r.pick()
    inv = r.inventory
    assert [r.slot(inv, n) for n in (8, 9, 11)] == [r.new_ammo, *extra], [hex(r.slot(inv, n)) for n in (8, 9, 11)]
    r.emu.mem_write(r.new_gun + 0x5E5, bytes([13]))
    r.emu.mem_write(r.new_gun + 0x5F1, bytes([0]))
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", 0))
    r.log.clear()
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    counts = [struct.unpack("<H", r.read(x + 0x260, 2))[0] for x in (r.new_ammo, *extra)]
    assert counts == [47, 0, 0], counts
    crate = [l.split("place 3 crate: ", 1)[1] for l in r.log if "place 3 crate: " in l]
    assert crate == ["the weapon slot's own ammo: slot 8, the kit's 100 of ammo 70 (clip 50/1) = 2 bags",
                     "slot 8 ammo 7 0 -> 47; the weapon slot's 2 bags = 60 (clip 30/1) less 13",
                     "slot 9 ammo 7 0 -> 0 (not the gun's loaded stack: left)",
                     "slot 11 ammo 7 0 -> 0 (not the gun's loaded stack: left)"], crate
    print("ok the owner's case: the weapon slot's 2 bags (100 rounds of ammunition 70, 50 a clip) are 60 rounds "
          "of the borrowed ammunition 7 (30 a clip), 13 in the gun: slot 8 47, slots 9 and 11 nothing")


def no_branch_into(original):
    """Nothing of the original exe branches into bytes 1.. of a new span (selftest's scan)."""
    import selftest_026g as test
    stock = test.Image(original)
    for va, size in NEW_SPANS.items():
        spelled = [va + i for i in range(1, size) if original.find(struct.pack("<I", va + i)) != -1]
        hits = [x for x in test.aimed_inside(original, va, size, 0x82)
                if x not in spelled and test.starts_instruction(stock, x)]
        assert not hits, (hex(va), [hex(h) for h in hits])


def holster(patched):
    """Option (b): the pickup committed at the command (the drop taken, every check of the install made,
    the gun the picker's pending object under its own reserved registry id), the holster only its
    animation: the game's switch holsters the gun in hands and then the gun takes the slot. Weapon keys
    while holstering are ignored; only the picker's death cancels; the swap is always the game's own (a gun firing is waited for; past
    3 s the game is asked each tick to look at the switch, never forced); a rollback inside the holster leaves the registry as the record says; a defensive
    refusal at the switch draws the old gun back. The picker always has a gun in its hands."""
    taken = DIED + 1_000
    waiting = (10, PICKED, (17, 1))

    def started():
        r = Pick(patched)
        r.setup()
        r.ready = False
        r.pick(switch=False)
        r.tick(ready=False)
        return r

    def window(r):
        """The command played, the own gun holstering: nothing of the slots moved yet."""
        gun, own = r.new_gun, r.own_gun
        assert r.read(r.picker + 0x128) == own and r.read(r.picker + 0x12C) == gun
        assert r.slot(r.inventory, 10) == own + 0x10 and r.slot(r.cooked, 10) == gun + 0x10
        assert r.read(gun + 0xC, 1)[0] == PENDING_ID and r.read(r.registry + 4 * PENDING_ID) == gun
        assert r.read(r.registry + 4 * PICK_ID) == 0 and r.read(r.registry + 4 * GUN_ID) == own
        assert r.native == [], r.native
        assert r.save(r.picker) == record(pending=waiting), r.save(r.picker).hex()
        assert r.save(PLAYER) == record(("taken", 1, A_GUN)), "claimed at the command"
        r.in_hands()

    # The switch: the command, the holster (ticks not ready), the game's switch.
    r = started()
    window(r)
    r.tick(ready=False, now=taken + 400)
    window(r)
    assert any(f"took (the gun in hands holstering): slot 10's pending gun (taken at {taken} ms, the gun of place 5 "
               f"kit 1 slot 10), registry id {PENDING_ID}" in l for l in r.log), r.log
    assert r.tick(now=taken + 600) == r.new_gun
    r.installed()
    assert r.native == [("deactivate", r.own_gun, 1), ("forget", r.picker + 0x750, r.own_gun),
                        ("set_user", r.new_gun, r.picker), ("initialize", r.new_gun),
                        ("activate", r.new_gun, 1)], r.native
    r.put_away_in_slot(r.own_gun)
    assert r.read(r.new_gun + 0x5E5, 1)[0] == 17 and r.read(r.new_gun + 0x5F1, 1)[0] == 1
    assert r.save(r.picker) == record(slot10=PICKED) and r.save(PLAYER) == record(("taken", 1, A_GUN))
    assert any("switched (the gun in hands holstered): slot 10's picked gun" in l for l in r.log), r.log
    print("ok (b) the command claims the drop and registers the gun as the pending object under its own id "
          f"({PENDING_ID}: the own gun keeps id {GUN_ID} and stays in its slot while it holsters); the game's "
          f"switch then installs it (id {PICK_ID}, the switch's calls, the donor's magazine); logged at the command "
          "and the switch; a gun in hands throughout")

    # The hide starts on the command's tick, as a weapon key's: set_input sets m_need_to_select_animations
    # (+33Eh) when the actions change (0x2D899D), and tick_animations then runs select_animations
    # (0x2D8F42) and with it tick_active_object. The pickup's command is filtered out of the actions, so
    # the command sets it itself. The game's own tick_animations head, run after the command, reaches
    # select_animations; without a command it does not.
    def tick_animations_selects(g):
        anim_state = g.blk(0x400)
        g.emu.mem_write(anim_state + 0x388, bytes([2]))       # no other reason to select
        g.word(g.picker + 0x2E0, anim_state)
        sp = hide.STACK + 0x9000
        g.word(sp + 4, g.read(g.picker + 0x324))
        g.word(sp + 8, g.read(g.picker + 0x324) + 50)
        g.run(0x2D8EE0, (0x2D8F42, 0x2D8F4E), sp, {**hide.GAME, UC_X86_REG_EAX: g.picker})
        return g.stopped == 0x2D8F42
    k = Pick(patched)
    k.setup()
    k.emu.mem_write(k.picker + 0x33E, b"\0")
    assert not tick_animations_selects(k), "no command: nothing to select"
    k.emu.mem_write(k.picker + 0x33E, b"\0")
    k.ready = False
    k.pick(switch=False)
    assert k.read(k.picker + 0x33E, 1) == b"\1", "the command asks for the animations to be selected"
    assert tick_animations_selects(k), "this tick's tick_animations runs select_animations (the switch looked at)"
    print("ok the hide starts on the command's tick, as a weapon key's: the command sets m_need_to_select_animations "
          "(+33Eh, set_input's 0x2D899D for changed actions), and the game's tick_animations then reaches "
          "select_animations (0x2D8F42) that same tick; without a command it does not")

    # A weapon key while holstering: ignored (pending and the selected slot put back).
    r = started()
    other = r.second(double_handed=0)
    r.word(r.picker + 0x12C, other)
    r.word(r.inventory + 0x170, 7)
    r.tick(ready=False, now=taken + 200)
    window(r)
    assert r.read(r.inventory + 0x170) == 10
    assert any("ignored another weapon switch while holstering for: slot 10's picked gun" in l for l in r.log), r.log
    r.tick(now=taken + 500)
    r.installed()
    print("ok a weapon key while holstering is ignored until the swap (the pending object and the inventory's "
          "selected slot +170h put back), then the switch comes")

    # The donor's body leaves the world (its respawn), or the donor leaves the match, while the picker
    # holsters: the pickup stands (committed at Use).
    for departure in (False, True):
        r = started()
        if departure:
            left_match(r, PLACE, PLAYER, KIT, taken + 300)
        else:
            r.remove(PLAYER, taken + 300)
        window_record = r.save(r.picker)
        assert window_record == record(pending=waiting) and r.save(PLAYER) == record(("taken", 1, A_GUN, True)), departure
        r.tick(now=taken + 700)
        r.installed()
    print("ok the donor's respawn or departure during the holster does not cancel it: the gun is the picker's "
          "(the drop taken, alone)")

    # A pickup while firing (the 83Lb41 crash: the gun in hands never ready while the trigger is held,
    # then force-swapped at the 3 s deadline mid-fire, its intervals firing on a gun with no user). The
    # game's fire -> hide waits for the trigger and its animation, as for a 1/2 key: the pickup waits
    # too, however long, and nothing of the gun in hands is put away while it is not ready (its
    # intervals stay the game's). Past 3 s the game is asked, each tick at the input's apply point, to
    # look at the switch (m_need_to_select_animations), logged once; never a forced swap. A rollback
    # during the wait keeps it waiting. Once the trigger is let go the game's switch puts the gun away
    # (deactivate, forget, in its slot) and the picked gun is drawn.
    def apply_input(g, now, actions=0x20):
        g.word(g.picker + 0x324, now)
        g.emu.mem_write(g.picker + 0x33E, b"\0")
        g.applied = g.set_input(g.picker, now, actions)     # fire held
        return g.read(g.picker + 0x33E, 1) == b"\1"
    r = started()
    r.native, r.put_away = [], []
    r.log.clear()
    for t in range(100, 2_999, 450):
        assert not apply_input(r, taken + t) and r.applied == 0, "no nudge before the deadline; fire held"
        r.tick(ready=False, now=taken + t)                # firing: not ready to be deactivated
    window(r)
    nudged = [apply_input(r, taken + t) for t in (3_000, 3_050, 4_000, 6_000)]
    assert nudged == [True] * 4, nudged
    for t in (3_000, 4_000, 6_000, 9_000):
        r.tick(ready=False, now=taken + t)
    window(r)                                               # still waiting: nothing put away, no switch
    assert r.native == [] and r.put_away == [], r.native
    waits = [l for l in r.log if "still waiting for the gun in hands to be put away" in l]
    assert len(waits) == 1, r.log
    assert not any("AT ONCE" in l for l in r.log), r.log
    # A rollback during the wait (to a state of the window): it keeps waiting, the registry as before.
    assert r.load(r.picker, record(pending=waiting)) == RECORD
    window(r)
    r.tick(ready=False, now=taken + 9_100)
    window(r)
    # The trigger let go: the game's switch.
    assert r.tick(now=taken + 9_400) == r.new_gun
    r.installed()
    r.put_away_in_slot(r.own_gun)
    assert r.native.index(("deactivate", r.own_gun, 1)) == 0
    print("ok a gun not ready to be put away is waited for (no forced swap): no switch and nothing of it put "
          "away while it is not ready (its intervals stay the game's), past 3 s the game is asked each tick to look at the "
          "switch (logged once), never forced; a rollback during the wait keeps it waiting; the trigger let go, the "
          "game's switch puts the gun away in its slot and draws the picked one")

    # E interrupts the firing (the owner, 2026-11-01): from the command to the swap the fire (0x20) and
    # reload (0x40) bits are cleared from the actions as they are applied, keyed on the synced record's
    # pending pickup, so the held gun ends its current shot and starts no other (the game's fire -> hide
    # waits only for the shot's animation), starts no reload (reload -> hide does not wait), and the
    # game's switch holsters it. Other bits pass. After the swap the actions are the player's again.
    # Sustained fire: the command's own input (fire held) already reaches the gun without fire.
    f = Pick(patched)
    f.setup()
    f.ready = False
    f.pick(switch=False, actions=0x20)
    assert f.applied & 0x60 == 0, hex(f.applied)
    plain = Pick(patched)                                   # the same inputs without a pickup: what passes
    plain.setup()
    for t, actions in ((50, 0x20), (100, 0x20 | 0x40), (150, 0x40 | 0x80), (200, 0x20)):
        apply_input(plain, taken + t, actions)
        apply_input(f, taken + t, actions)
        assert plain.applied & 0x60 == actions & 0x60, (t, hex(plain.applied))
        assert f.applied == plain.applied & ~0x60, (t, hex(f.applied), hex(plain.applied))
        f.tick(ready=False, now=taken + t)
    window(f)
    # A rollback during the wait: the replayed inputs are held alike; one to before the command lets them go.
    assert f.load(f.picker, record(pending=waiting)) == RECORD
    apply_input(f, taken + 250)
    assert f.applied == 0, "a replay inside the wait holds the fire alike"
    # The shot ended (the gun ready): the game's switch, then the new gun fires as the player asks.
    assert f.tick(now=taken + 300) == f.new_gun
    f.installed()
    apply_input(f, taken + 350)
    assert f.applied == 0x20, "after the swap the fire is the player's again (the new gun fires)"
    apply_input(f, taken + 400, 0x40)
    assert f.applied == 0x40
    g = Pick(patched)
    g.setup()
    g.ready = False
    g.pick(switch=False)
    assert g.load(g.picker, record()) == RECORD
    apply_input(g, taken + 50, 0x20 | 0x40)
    assert g.applied == 0x60, "rolled back to before the command: nothing held"
    # A single shot: the command's input with the trigger pressed once, the next ones without.
    h = Pick(patched)
    h.setup()
    h.ready = False
    h.pick(switch=False, actions=0x20)
    assert h.applied == 0, "the shot pressed with E is not started"
    apply_input(h, taken + 50, 0)
    h.tick(ready=False, now=taken + 50)
    assert h.tick(now=taken + 100) == h.new_gun
    h.installed()
    print("ok E interrupts the firing: from the command to the swap fire and reload are cleared from the applied "
          "actions (keyed on the record's pending pickup), so sustained fire ends with the current shot and a "
          "single shot pressed with E is not fired, no reload starts; a rollback during the wait holds the "
          "replayed inputs alike, one to before the command does not; after the swap the new gun fires and "
          "reloads as the player asks")

    # The picker's death while holstering: the only cancel. The gun back in the drop (open again).
    r = started()
    r.log.clear()
    r.run(DEATH_SITE, (DEATH_AFTER,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_ECX: r.picker})
    assert r.read(r.picker + 0x128) == r.own_gun and r.read(r.picker + 0x12C) == r.own_gun
    assert r.read(r.registry + 4 * PENDING_ID) == 0 and r.read(r.new_gun + 0xC, 1)[0] == 0
    assert r.slot(r.inventory, 10) == r.own_gun + 0x10 and r.slot(r.cooked, 10) == r.new_gun + 0x10
    assert r.save(r.picker)[12:20] == bytes(8) and r.save(PLAYER) == record(("open", 1, A_GUN))
    assert any("CANCELLED the switch to: slot 10's picked gun" in l and "(its death)" in l for l in r.log), r.log
    r.in_hands()
    print("ok the picker's death while holstering cancels: the own gun stays in hands, the reserved id emptied, "
          "the gun back in its body's drop (open again)")

    # A rollback inside the holster window. Back to before the command: the registry as before it, the
    # gun back in its drop; the command played again takes it from there.
    r = started()
    r.load(PLAYER, record(("open", 1, A_GUN)))
    r.load(r.picker, record())
    assert r.read(r.registry + 4 * PENDING_ID) == 0 and r.read(r.new_gun + 0xC, 1)[0] == 0
    assert r.read(r.picker + 0x12C) == r.own_gun, "pending back to the held gun (the reader then sets the state's)"
    r.queries = []
    r.pick(switch=False)
    window(r)
    assert r.queries == [], "taken again from the drop, no cook"
    # After the switch, back into the window: the pick undone, the gun registered pending again.
    r.tick(now=taken + 600)
    r.installed()
    r.native, r.put_away = [], []
    assert r.load(r.picker, record(pending=waiting)) == RECORD
    assert r.native == [("deactivate", r.new_gun, 1), ("forget", r.picker + 0x750, r.new_gun)], r.native
    r.put_away_in_slot(r.new_gun)
    assert r.read(r.picker + 0x128) == 0, "put away: the reader has nothing to deactivate"
    assert any("gave back to its drop: slot 10's picked gun" in l and "from its holster: pending again" in l
               for l in r.log), r.log
    assert r.slot(r.inventory, 10) == r.own_gun + 0x10 and r.slot(r.cooked, 10) == r.new_gun + 0x10
    assert r.read(r.registry + 4 * PICK_ID) == 0 and r.read(r.registry + 4 * PENDING_ID) == r.new_gun
    assert r.read(r.new_gun + 0xC, 1)[0] == PENDING_ID
    assert any("a loaded record: registered: slot 10's pending gun" in l for l in r.log), r.log
    # The reader sets held and pending from the state's ids (the own gun's and PENDING_ID's entries).
    r.word(r.picker + 0x128, r.read(r.registry + 4 * GUN_ID))
    r.word(r.picker + 0x12C, r.read(r.registry + 4 * PENDING_ID))
    r.native = []
    window(r)
    r.tick(now=taken + 700)
    r.installed()
    print("ok a rollback inside the holster: to before the command the reserved id is emptied and the gun "
          "back in its drop (taken again, no cook); from after the switch back into the window the pick is "
          "undone and the gun registered pending again, the registry as the record says; the switch then "
          "comes again")

    # A defensive refusal at the switch (the slots changed under the holster): cancelled, the old gun
    # drawn back by the native switch, never nothing in hands.
    r = started()
    r.word(r.cooked + 0x10C + 4 * 11, 0)            # the gun's bound stack gone from its cooked slot
    r.log.clear()
    assert r.tick(now=taken + 600) == r.own_gun
    assert ("activate", r.own_gun, 1) in r.native and r.read(r.picker + 0x12C) == r.own_gun
    assert r.read(r.registry + 4 * PENDING_ID) == 0 and r.slot(r.inventory, 10) == r.own_gun + 0x10
    assert any("CANCELLED the switch to" in l and "bound ammunition slot" in l for l in r.log), r.log
    assert r.save(PLAYER) == record(("open", 1, A_GUN))
    r.in_hands()
    # ... and a command whose install would be refused is refused at once, nothing changed.
    q = Pick(patched)
    q.setup()
    q.word(q.cooked + 0x10C + 4 * 11, 0)
    q.pick(switch=False)
    assert q.read(q.picker + 0x12C) == q.own_gun and q.read(q.registry + 4 * PENDING_ID) == 0
    assert q.save(PLAYER) == record(("open", 1, A_GUN)) and q.save(q.picker) == record()
    q.in_hands()
    print("ok a defensive refusal at the switch: cancelled and logged, the native switch draws the own gun "
          "back; a command whose install would be refused is refused at the command, nothing changed")

    # A pickup over a picked gun waits for the switch too (both ids reserved: slot 10's and the pending).
    r = Pick(patched)
    r.setup()
    r.pick()
    first = r.new_gun
    second, gun, ammo = r.new_life(6, SPAWNED + 20_000, DIED + 2_000)
    r.pick(target=6, now=DIED + 2_500, switch=False)
    r.tick(ready=False)
    assert r.read(r.picker + 0x128) == first and r.read(r.picker + 0x12C) == gun
    assert r.read(r.registry + 4 * PICK_ID) == first and r.read(r.registry + 4 * PENDING_ID) == gun
    r.tick()
    assert r.read(r.registry + 4 * PICK_ID) == gun and r.read(r.registry + 4 * PENDING_ID) == 0
    assert r.read(r.picker + 0x128) == gun and r.slot(second, 10) == r.own_gun + 0x10
    r.in_hands()
    print("ok a pickup over a picked gun: both guns registered while it holsters (slot 10's id and the "
          "pending one), then the new one takes slot 10's id")

    # The 83Lb41 crash's case: a pickup over a picked gun switched, then a rollback into its holster.
    # The record names the first picked gun in slot 10 and the second pending. The second gun, in hands,
    # is put away as the native switch does (deactivate, forget, in its slot) before it goes back to the
    # pending role; the first is installed again; at the switch the first is put away in its slot before
    # any slot moves. Every gun deactivated is forgotten by the animation player, and the picker always
    # has a gun in hands.
    r = Pick(patched)
    r.setup()
    r.pick()
    first, first_cooked = r.new_gun, r.cooked
    first_pick = PICKED
    second_cooked, second, _ = r.new_life(6, SPAWNED + 20_000, DIED + 2_000)
    later = DIED + 2_500
    r.pick(target=6, now=later, switch=False)
    r.tick(ready=False)
    window_record = r.save(r.picker)
    assert window_record == record(slot10=first_pick, pending=(10, (later, 6, KIT, 1), (17, 1))), window_record.hex()
    r.native, r.put_away = [], []
    r.tick(now=later + 1_600)
    assert r.read(r.picker + 0x128) == second and r.read(r.registry + 4 * PICK_ID) == second
    r.put_away_in_slot(first)
    assert r.native.index(("deactivate", first, 1)) == 0, "the gun in hands put away before anything moves"
    # The rollback to 40 ms before the switch.
    r.native, r.put_away = [], []
    r.log.clear()
    assert r.load(r.picker, window_record) == RECORD
    assert r.native[:2] == [("deactivate", second, 1), ("forget", r.picker + 0x750, second)], r.native
    r.put_away_in_slot(second)
    assert [n for n in r.native if n[1] == first] == [], ("the re-installed first gun: no weapon call of ours", r.native)
    assert r.native == [("deactivate", second, 1), ("forget", r.picker + 0x750, second)], r.native
    assert r.slot(r.inventory, 10) == first + 0x10 and r.slot(second_cooked, 10) == second + 0x10
    assert r.slot(first_cooked, 10) == r.own_gun + 0x10, "the own gun hidden with the first picked gun again"
    assert r.read(r.registry + 4 * PICK_ID) == first and r.read(first + 0xC, 1)[0] == PICK_ID
    assert r.read(r.registry + 4 * PENDING_ID) == second and r.read(second + 0xC, 1)[0] == PENDING_ID
    assert r.read(r.registry + 4 * GUN_ID) == r.own_gun
    assert any("ms gun %#x: put away, registry id %d, slot 10 (deactivate(1) + forget_active_object)"
               % (second, PICK_ID) in l for l in r.log), r.log
    assert any("ms gun %#x: a loaded record re-installed, registry id %d, slot 10 (no weapon call" % (first, PICK_ID) in l
               for l in r.log), r.log
    assert [l.split(" ms ", 1)[1].split(" (taken", 1)[0] for l in r.log if " ms " in l and " ms gun " not in l] == [
        "gave back to its drop: slot 10's picked gun", "a loaded record: installed: slot 10's picked gun",
        "a loaded record: registered: slot 10's pending gun"], r.log
    assert r.save(r.picker) == window_record
    r.reader(PICK_ID)
    r.word(r.picker + 0x12C, r.read(r.registry + 4 * PENDING_ID))
    r.in_hands()
    # The switch again: the first gun put away in its slot first, then the second in its place.
    r.native, r.put_away = [], []
    r.tick(now=later + 1_650)
    r.put_away_in_slot(first)
    assert r.native.index(("deactivate", first, 1)) == 0
    assert r.read(r.picker + 0x128) == second and r.read(r.registry + 4 * PICK_ID) == second
    assert r.read(r.registry + 4 * PENDING_ID) == 0 and r.slot(first_cooked, 10) == first + 0x10
    assert r.slot(second_cooked, 10) == r.own_gun + 0x10
    assert r.save(r.picker) == record(slot10=(later, 6, KIT, 1))
    r.in_hands()
    print("ok the 83Lb41 crash's case, a rollback into the holster of a pickup over a picked gun: the gun in "
          "hands put away as the native switch does (deactivate, forget, still in its slot) before it is "
          "moved back to pending, the first picked gun installed again, the registry as the record says; "
          "the switch again puts the first gun away in its slot before any move; a gun in hands throughout")


def keep_and_reuse(patched):
    """The owner's lifetime model (2026-11-01, after 83Lb41's crash on a bullet of a freed gun): a copy that
    was ever registered or in hands is never freed mid-match, only with its last holder (its departure, the
    match's end); an idle one is re-used for its donor's next drop; a loaded record naming a kept copy
    re-installs that object with no weapon call of ours."""
    # A replacement, a rollback across its switch, then much later: the replaced gun (which bullets of its
    # may still name) is alive.
    r = Pick(patched)
    r.setup()
    r.pick()
    first, first_cooked = r.new_gun, r.cooked
    second_cooked, second, _ = r.new_life(6, SPAWNED + 20_000, DIED + 2_000)
    r.pick(target=6, now=DIED + 2_500, switch=False)
    r.tick(ready=False)
    window = r.save(r.picker)
    r.tick(now=DIED + 3_400)
    assert r.read(r.picker + 0x128) == second
    r.load(r.picker, window)                            # back into the holster: the first gun in hands again
    r.reader(PICK_ID)
    r.word(r.picker + 0x12C, r.read(r.registry + 4 * PENDING_ID))
    r.tick(now=DIED + 3_450)
    assert r.read(r.picker + 0x128) == second
    for t in (10_000, 60_000, 600_000):
        r.step(DIED + t)
    assert r.read(first_cooked + 0xD0) == 2 and r.read(second_cooked + 0xD0) == 2, "freed mid-match"
    assert r.slot(first_cooked, 10) == first + 0x10, "the replaced gun alive in its cooked inventory"
    print("ok a replacement, a rollback across its switch, the switch again: 10 min on the replaced gun (which a "
          "bullet of its may name) and the new one are both alive")

    # Re-use: the picker's removal lets the gun go; the donor (the same player object) dies again with the
    # same kit and slot: the kept copy is its drop's gun, no new cook; a pickup of it is the usual one.
    r = Pick(patched)
    r.setup()
    r.pick()
    kept, kept_cooked = r.new_gun, r.cooked
    r.remove(r.picker)
    r.restored()
    r.remove(PLAYER, DIED + 8_000)
    r.word(PLAYER + 0x328, SPAWNED + 20_000)
    r.word(PLAYER + 0x324, DIED + 30_000)
    r.emu.mem_write(WEAPON + 0x5E5, bytes([9]))
    r.queries, r.log = [], []
    r.death()
    assert r.queries == [], "an idle kept copy is re-used: no new cook"
    assert any("re-using a kept copy of the gun of place 5 kit 1 slot 10 for the drop of place 5: gun %#x" % kept in l for l in r.log), r.log
    assert r.save(PLAYER) == record(("open", 1, A_GUN))
    r.word(r.picker + 0x128, r.own_gun)
    r.word(r.picker + 0x12C, r.own_gun)
    r.pick(now=DIED + 31_000)
    assert r.slot(r.inventory, 10) == kept + 0x10 and r.read(r.registry + 4 * PICK_ID) == kept
    assert r.read(kept + 0x5E5, 1)[0] == 9, "the donor's magazine of this death, granted at the switch"
    r.in_hands()
    print("ok re-use: the picker's removal lets the gun go (kept); the donor's next death with that kit and slot "
          "takes the kept copy as its drop's gun (no cook, logged); its pickup installs it with this death's magazine")

    # A resync or late join's record naming a kept copy: that object re-installed, no cook, no weapon call.
    r.remove(r.picker)
    r.queries, r.native = [], []
    r.load(r.picker, record(slot10=(DIED + 31_000, PLACE, KIT, 1)))
    assert r.queries == [] and r.slot(r.inventory, 10) == kept + 0x10, r.log
    assert [n for n in r.native if n[1] == kept] == [], r.native
    print("ok a loaded record naming a kept copy: the same object re-installed, no cook, no weapon call of ours")

    # No free until teardown: the holder's destruction frees what it last held.
    r.step(DIED + 900_000)
    assert r.read(kept_cooked + 0xD0) == 2
    r.destroy_player(r.picker)
    assert r.read(kept_cooked + 0xD0) == 1
    print("ok no free of a copy ever held until its holder's destruction (15 min on, still kept)")


def bullet_record(patched, original):
    """A bullet's record names its weapon by its shooter's slot and the ammo's ordinal; after a pickup's swap
    the slot holds another gun than the one that fired a bullet in flight (the 85a crash: a rifle's records
    read with a shotgun in the slot, its quiet byte read where none was written, every record after out of
    step). The game's own branch sites, run: the original writes the quiet byte (+80h) only for an ammo of
    more than one pellet and reads it only for the loading gun's such ammo; the patched build always writes
    and reads it, and an ordinal past the loading gun's ammo is none."""
    SAVE, SAVE_WRITE, SAVE_SKIP = 0x32C148, 0x32C151, 0x32C171
    LOAD, LOAD_READ, LOAD_SKIP = 0x32BE12, 0x32BE1B, 0x32BE29
    ORDINAL, ORDINAL_CALL = 0x32BDBA, 0x32BDC8

    def quiet_bytes(data, pellets_at_save, pellets_at_load):
        """(written, read) quiet bytes of one record: saved with the firing gun's ammo, loaded with the gun
        in the slot then."""
        r = Pick(data)
        bullet, saved_ammo, loaded_ammo, material = r.blk(0x90), r.blk(0x50), r.blk(0x50), r.blk(0x10)
        r.emu.mem_write(saved_ammo + 0x44, bytes([pellets_at_save]))
        r.emu.mem_write(loaded_ammo + 0x44, bytes([pellets_at_load]))
        r.word(bullet + 0x40, saved_ammo)
        r.lohook(SAVE, SAVE_WRITE, {UC_X86_REG_ESI: bullet}, stops=(SAVE_WRITE, SAVE_SKIP))
        written = r.stopped == SAVE_WRITE
        r.lohook(LOAD, LOAD_READ, {UC_X86_REG_EDI: bullet, UC_X86_REG_EAX: material, UC_X86_REG_ECX: loaded_ammo},
                 stops=(LOAD_READ, LOAD_SKIP))
        assert r.read(bullet + 0x4C) == material, "the struck material stored as the replaced mov does"
        return int(written), int(r.stopped == LOAD_READ)

    # The original: a rifle's record (1 pellet) loaded with a shotgun (10) in the slot reads a byte it lacks.
    assert quiet_bytes(original, 1, 10) == (0, 1), "the original's misstep"
    assert quiet_bytes(original, 1, 1) == (0, 0) and quiet_bytes(original, 10, 10) == (1, 1)
    # Patched: always one, written and read: every swap, and the native swap-free cases, in step.
    for save, load in ((1, 10), (10, 1), (1, 1), (10, 10)):
        assert quiet_bytes(patched, save, load) == (1, 1), (save, load)

    # The ordinal: past the loading gun's ammo count it is that count (0x2E2750's none, +53Ch); within, kept.
    r = Pick(patched)
    shotgun = r.gun()
    r.emu.mem_write(shotgun + 0x5B0, bytes([3]))
    bullet = r.blk(0x90)
    r.word(bullet + 0xC, shotgun + 0x10)
    for ordinal, want in ((16, 3), (22, 3), (4, 3), (3, 3), (2, 2), (0, 0)):
        out, _ = r.lohook(ORDINAL, ORDINAL_CALL, {UC_X86_REG_EDI: bullet, UC_X86_REG_ECX: 0x1234500 | ordinal})
        assert out[UC_X86_REG_ECX] == 0x1234500 | want, (ordinal, hex(out[UC_X86_REG_ECX]))
        assert out[UC_X86_REG_EAX] == shotgun, "the weapon passed on as the replaced code computes it"
    o = Pick(original)
    o_gun = o.gun()
    o.emu.mem_write(o_gun + 0x5B0, bytes([3]))
    o_bullet = o.blk(0x90)
    o.word(o_bullet + 0xC, o_gun + 0x10)
    out, _ = o.lohook(ORDINAL, ORDINAL_CALL, {UC_X86_REG_EDI: o_bullet, UC_X86_REG_ECX: 16})
    assert out[UC_X86_REG_ECX] & 0xFF == 16, "the original passes it on unchecked"
    print("ok a bullet's record after a swap: the original writes the quiet byte only for multi-pellet ammo and "
          "reads it by the loading gun's (a rifle's record with a shotgun in the slot: written 0, read 1, out of "
          "step); patched it is always written and read (rifle to shotgun, shotgun to rifle, and the swap-free "
          "rifle and shotgun alike: in step), the struck material stored as before; an ordinal past the loading "
          "gun's ammo is its count (none), one within is kept, where the original passes 16 on")


def stage2(patched, original):
    no_branch_into(original)
    print("ok nothing in the original branches into the new spans (0x2DAF70, 0x2DB4CF, 0x2DA790, 0x2F809C, "
          "0x2F80E4, 0x2F81B8, 0x2954EF, tick_active_object 0x2D7650 and its switch 0x2D76D0, the bullet record's 0x32C148, 0x32BE12, 0x32BDBA)")

    r = Pick(patched)
    r.setup()
    assert r.save(PLAYER) == record(("open", 1, A_GUN)), r.written.hex()
    assert r.save(r.picker) == record()
    print("ok the donor's death opens its drop (kit 1, slot 10) in its record; the picker's is empty; "
          "each written by the native byte writer at base_player::serialize's start")

    # The carry rules: any gun for the held one, as the skill modifiers allow.
    pistol = Pick(patched)
    pistol.setup(own_double_handed=0)
    pistol.pick()
    pistol.installed()
    print("ok a rifle replaces the held pistol (slot 7 empty): any gun for any gun, the held one replaced")

    beside = Pick(patched)
    beside.setup(own_double_handed=0)
    beside.second(double_handed=1)
    beside.pick()
    assert beside.slot(beside.inventory, 10) == beside.own_gun + 0x10 and beside.native == []
    assert any("refused, a second double-handed gun without modifier 25" in l for l in beside.log), beside.log
    beside.modifier(25, 1.0)
    beside.pick()
    beside.installed()
    print("ok a rifle for the held pistol beside a rifle in slot 7: refused and logged, nothing changed; with "
          "use_two_double_handed_weapons (modifier 25, the game's apply_modifier 0x2D5AE0): taken")

    one = Pick(patched)
    one.setup()
    one.modifier(64, 1.0)
    one.pick()
    assert one.slot(one.inventory, 10) == one.own_gun + 0x10 and one.native == []
    assert any("refused, its kit carries one gun only, in slot 7 (modifier 64)" in l for l in one.log), one.log
    one.pick(target=3)
    assert any("refused, no drop to take" in l for l in one.log), one.log
    print("ok disable_second_weapon (modifier 64): a gun held in slot 10 is not replaced; the picker's own "
          "place: refused; each logged, nothing changed")

    # The pickup.
    r.pick()
    r.installed()
    assert r.native == [("deactivate", r.own_gun, 1), ("forget", r.picker + 0x750, r.own_gun),
                        ("set_user", r.new_gun, r.picker), ("initialize", r.new_gun),
                        ("activate", r.new_gun, 1)], r.native
    assert r.read(r.picker + 0x128) == r.read(r.picker + 0x12C) == r.new_gun
    assert r.read(r.new_gun + 0x5E5, 1)[0] == 17 and r.read(r.new_gun + 0x5F1, 1)[0] == 1
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", CAPACITY)
    assert any("place 3 at 13350 ms took (the gun in hands holstering): slot 10's pending gun (taken at 13350 ms, the gun "
               f"of place 5 kit 1 slot 10), registry id {PENDING_ID}" in l for l in r.log), r.log
    assert any("place 3 at 13350 ms switched (the gun in hands holstered): slot 10's picked gun (taken at 13350 ms" in l
               for l in r.log), r.log
    took = [l.split("pickup: ", 1)[1] for l in r.log if "pickup: place 3 pickup: " in l]
    assert took == ["place 3 pickup: the donor's magazine 17, chamber 1",
                    f"place 3 pickup: slot 8 ammo 2204 x {CAPACITY} (cooked slot 11); own ammo 217 x 108 parked"], took  # set aside as the kit gives it
    assert r.save(r.picker) == record(slot10=PICKED) and r.parked == b"", "the record: nothing of the own kit"
    # The own kit set aside, the same in every game: the own gun empty, the own 5.45 as the kit gives it (108).
    assert r.read(r.own_gun + 0x5E5, 1) == b"\0" and r.read(r.own_ammo + 0x260, 2) == struct.pack("<H", 108)
    assert r.save(PLAYER) == record(("taken", 1, A_GUN))
    print("ok the pickup at set_input's apply point (carry.filter): the slots swapped by the game's set_item, the "
          "ammunition into the own gun's slot (8, the own stack parked in the cooked inventory) and rebound, the gun bound (core, registry id and entry, bullet "
          "manager, autoload off), the switch replayed (deactivate, forget, held = pending, set_user, initialize, "
          "activate), the donor's magazine and chamber with one magazine in reserve; picker borrows, drop taken; "
          "logged: the donor's magazine and chamber, the stack placed (slot, ammunition, count) and the own one parked")

    r.pick()
    assert any("refused, no drop to take" in l for l in r.log)
    r.installed()
    print("ok a second command for the same drop: refused (taken)")

    # The crate, the native refill's rule in bags with real configs: the picker's own 5.45x39 7N10
    # (217: 30 a clip, a bag a clip) has the kit's allowance of 90 rounds in slot 8, 3 bags; the
    # borrowed PKP's 7.62x54 7N13 (2204: 100 a clip, 2 bags a clip) gets what 3 bags carry of it,
    # one belt of 100, the gun's magazine and chamber (17 + 1) inside: 82, for free.
    allowance = lambda rounds: r.emu.mem_write(PROFILE + SLOTS + 4 * 8, struct.pack("<HH", 8, rounds))
    allowance(90)
    storage = r.read(PROFILE + ITEMS, 0xB4 * 69)
    r.lohook(CAN_USE, CAN_USE_AFTER, {UC_X86_REG_EAX: r.picker}, stops=(CAN_USE_AFTER, CAN_USE_AVAILABLE))
    assert r.stopped == CAN_USE_AVAILABLE, "30 of 82: available"
    r.lohook(HINT, HINT_AFTER, {UC_X86_REG_EAX: r.picker}, stops=(HINT_AFTER, HINT_AVAILABLE))
    assert r.stopped == HINT_AVAILABLE
    r.emu.mem_write(r.inventory + 0x184, b"\0")
    r.log.clear()
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    view = r.emu.reg_read(UC_X86_REG_EAX)
    assert view not in (r.inventory, hide.GAME[UC_X86_REG_EAX])
    assert r.slot(view, 10) == r.own_gun + 0x10 and r.slot(view, 8) == r.own_ammo and r.slot(view, 9) == 0
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", 100 - 18) and r.read(r.inventory + 0x184, 1) == b"\1"
    assert r.read(PROFILE + ITEMS, 0xB4 * 69) == storage, "a storage charged"
    assert any(l.endswith("pickup: place 3 crate: the weapon slot's own ammo: slot 8, the kit's 90 of ammo 217 "
                          "(clip 30/1) = 3 bags") for l in r.log), r.log
    assert any(l.endswith("pickup: place 3 crate: slot 8 ammo 2204 30 -> 82; the weapon slot's 3 bags = 100 "
                          "(clip 100/2) less 18") for l in r.log), r.log
    r.lohook(CAN_USE, CAN_USE_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.emu.reg_read(UC_X86_REG_ECX) == view, "full: check_resupply gets the view"
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", 4))
    r.lohook(CLIENT_RESUPPLY, CLIENT_RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.emu.reg_read(UC_X86_REG_EAX) == view and r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", 82)
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", 95))
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", 95), "never below what it has"
    # Four bags (120 rounds of 7N10): two belts, 200 less 18.
    allowance(120)
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", CAPACITY))
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", 200 - 18)
    # One bag (30 rounds) carries no belt: nothing to refill (no minimum), the native check sees the own loadout.
    allowance(30)
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", CAPACITY))
    r.lohook(CAN_USE, CAN_USE_AFTER, {UC_X86_REG_EAX: r.picker}, stops=(CAN_USE_AFTER, CAN_USE_AVAILABLE))
    assert r.stopped == CAN_USE_AFTER and r.emu.reg_read(UC_X86_REG_ECX) == view
    r.log.clear()
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", CAPACITY)
    assert any(l.endswith("pickup: place 3 crate: slot 8 ammo 2204 30 -> 30; the weapon slot's 1 bags = 0 "
                          "(clip 100/2) less 18") for l in r.log), r.log
    # The same ammunition as the own (the borrowed stack's config 7N10 too): the own gun's own refill, 90 - 18.
    r.ammo_config(r.new_ammo, 217, 30, 1)
    allowance(90)
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", CAPACITY))
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", 90 - 18)
    # A weapon slot the kit gives no bags: nothing.
    allowance(0)
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", 0))
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", 0)
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", CAPACITY))
    allowance(108)
    r.lohook(CAN_USE, CAN_USE_AFTER, {UC_X86_REG_EAX: PLAYER})
    assert r.emu.reg_read(UC_X86_REG_ECX) == INVENTORY, "a player without a borrow: the native inventory"
    print("ok the crate, the native refill's rule in bags (resupply_from_profile: the kit's allowance for the slot, "
          "the magazine and chamber inside) with the borrowed ammunition's config: the own 7N10's 90 rounds in "
          "slot 8 are 3 bags, a PKP's 7N13 (100 a clip, 2 bags) gets 100 less 18 = 82 (available at can_use and "
          "use_hint while below; both finish_use sites; logged with its inputs), 4 bags 182, 1 bag none (no "
          "minimum); the same ammunition 72, no bags nothing; no storage charged, never lowered; the native resupply and check_resupply get the view")

    # A loaded record without the pick (a rollback undoing it): restored before the native readers,
    # the gun back in its body's drop.
    r.put_away = []
    assert r.load(r.picker, record()) == RECORD
    r.restored()
    assert r.native == [("deactivate", r.new_gun, 1), ("forget", r.picker + 0x750, r.new_gun)], r.native
    r.put_away_in_slot(r.new_gun)
    assert r.read(r.picker + 0x128) == 0 and r.read(r.picker + 0x12C) == 0, "the reader then has nothing to deactivate"
    r.reader(GUN_ID)
    # ... and with it again: installed without the switch (the reader switches).
    assert r.load(r.picker, record(slot10=PICKED)) == RECORD
    assert r.native == [], ("a gun already in hands here: no weapon call of ours (the reader's)", r.native)
    r.reader(PICK_ID)
    r.installed()
    assert any("a loaded record re-installed, registry id %d, slot 10 (no weapon call" % PICK_ID in l
               for l in r.log), r.log
    print("ok a loaded record without the pick: the held gun put away as the native switch does (deactivate, "
          "forget, in its slot), then restored, the gun back in its drop; with it: installed again from it with no "
          "weapon call of ours (it was in hands here: the game's reader set_users and activates it)")

    # The donor's body leaves the world (its respawn: base_player::remove(1)) while its gun is lent:
    # the drop stays (taken, alone), the lent gun stays.
    gone = DIED + 8_000
    r.remove(PLAYER, gone)
    assert r.save(PLAYER) == record(("taken", 1, A_GUN, True)), "the drop stays past the respawn, alone"
    r.step(gone + 3_500)
    assert r.read(r.cooked + 0xD0) == 2 and r.releases == []
    r.installed()
    print("ok the donor's body leaves the world (its respawn's base_player::remove(1)): its record's drop stays, "
          "taken, alone; a borrowed gun is kept past it while it is held")

    # The removal: flag 0 (a snapshot's) leaves it, flag 1 (respawn, departure) restores.
    r.remove(r.picker, flag=0)
    r.installed()
    r.native, r.put_away = [], []
    r.remove(r.picker)
    r.restored()
    r.put_away_in_slot(r.new_gun)
    assert r.read(r.picker + 0x128) == 0, "put away: remove has nothing to deactivate"
    assert r.save(r.picker) == record(), "the borrow ends with the life"
    print("ok base_player::remove: flag 0 leaves the borrow; flag 1 puts the gun away (deactivate, forget, in its "
          "slot), then the own gun and its ammunition back (its unload charges the own profile); the record "
          "borrows nothing")

    # A record that does not decode: logged, read as empty, the reader past it. The pick taken back
    # first from what the removal let go (10 s after the donor's body left the world).
    r.word(r.picker + 0x324, gone + 10_000)
    r.load(r.picker, record(slot10=PICKED))
    r.reader(PICK_ID)
    r.installed()
    assert r.load(r.picker, b"\x63" + bytes(RECORD - 1)) == RECORD
    r.restored()
    r.reader(GUN_ID)
    assert any("record of place 3 does not decode (read as empty): bad bytes" in l for l in r.log), r.log
    assert r.load(r.picker, bytes(RECORD), size=8) == 0
    assert any("the state is too short" in l for l in r.log)
    print("ok a record that does not decode (bad bytes, a short state): logged loudly, read as empty (the pick "
          "restored), never a crash")
    r.step(gone + 10_100)
    r.step(gone + 600_000)
    assert r.read(r.cooked + 0xD0) == 2, "a copy that was in hands is never freed mid-match"
    r.destroy_player(r.picker)
    assert r.read(r.cooked + 0xD0) == 2, "back in a drop that lasts (its donor respawned): that drop's"
    r.destroy_player(PLAYER)
    assert r.read(r.cooked + 0xD0) == 1, "freed with its drop's player"
    print("ok keep and reuse: a copy that was in hands, back in the drop of a donor that respawned long ago, is "
          "never freed mid-match (10 min on); its last holder's destruction leaves it to that drop, whose "
          "player's destruction frees it")

    # Availability lasts until the player dies again, or leaves: a command after the donor's respawn is
    # taken (a full magazine: the body's gun is gone), one after its departure refused.
    late = Pick(patched)
    late.setup()
    late.pick(now=DIED + 60_000)
    late.installed()
    after = Pick(patched)
    after.setup()
    after.remove(PLAYER, DIED + 8_000)
    assert after.save(PLAYER) == record(("open", 1, A_GUN, True))
    after.pick(now=DIED + 600_000)
    assert after.slot(after.inventory, 10) == after.new_gun + 0x10, after.log
    assert after.read(after.new_gun + 0x5E5, 1)[0] == CAPACITY, "a full magazine"
    assert after.save(PLAYER) == record(("taken", 1, A_GUN, True)) and after.save(after.picker) == record(
        slot10=(DIED + 600_000, PLACE, KIT, 1))
    gone = Pick(patched)
    gone.setup()
    left_match(gone, PLACE, PLAYER, KIT, DIED + 8_000)
    assert gone.save(PLAYER) == record(("open", 1, A_GUN, True)), "a departed player's drop stays"
    gone.pick(now=DIED + 8_000)
    assert any("refused, the place to cook it from has no player" in l for l in gone.log) and gone.native == []
    assert gone.slot(gone.inventory, 10) == gone.own_gun + 0x10
    print("ok a command 60 s after the death with the body still there: taken; 10 min after the donor's respawn "
          "(the drop alone): taken, a full magazine; after the donor's departure its drop stays, refused (its place "
          "out of the world, the same in every game)")


# ---- stage 2b: the ragdoll's gun, the prompt, the command -----------------------------------------

WEAPON_DRAW, WEAPON_DRAWN, WEAPON_SKIPPED = 0x2477C8, 0x2477CE, 0x2477DF
ICONS_SITE, ICONS_AFTER = 0x23AA10, 0x23AA16
BUILDER_END, BUILDER_AFTER = 0x2421B0, 0x2421B5
RAY_TEST, REMOVE_MODEL, TRANSLATE = 0x75E20, 0x21F40, 0x1FF0F0
USE = 1 << 21
IDENTITY = (1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.)
NAME_KEY, NAME = b"st_wpn_svd_name", "SVD"
PLACES_AT = OPTIONS_HOLDER + 0xC + 0x577B5


class Prompt(Pick):
    """Stage 2a's match (the donor at place 5 dead, its gun cooked; the picker at place 3) with the
    donor's gun drawn on its ragdoll (a model box, a renderer), the hud (its movie's Invoke
    recorded), the picker's eye, a ray test, the gun's config (ui_desc/text_descriptions/name) and
    the dictionary's translate_text; the input builder's end."""

    def __init__(self, data):
        super().__init__(data)
        self.wall, self.removed = False, []
        for at in (RAY_TEST, REMOVE_MODEL, TRANSLATE):
            self.emu.hook_add(UC_HOOK_CODE, self.fixtures, begin=at, end=at)

    def fixtures(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 * n)
        if address == RAY_TEST:                     # ECX world, result, origin, direction, length, group, mask; ret 20h
            assert (arg(5), arg(6)) == (0x24, 2), "patch 78's level filter"
            self.word(arg(1) + 0x1C, 1 if self.wall else 0)
            self.ret(32)
        elif address == REMOVE_MODEL:               # EAX the model's reference, renderer, scene; ret 8
            instance = self.read(emu.reg_read(UC_X86_REG_EAX))
            emu.mem_write(instance + 0x10D, b"\0")
            self.removed.append(instance)
            self.ret(8)
        else:                                       # ECX translator, key, out; ret 8
            assert emu.reg_read(UC_X86_REG_ECX) == GAME + 0x52A8
            if self.text(arg(1)) == NAME_KEY:
                emu.mem_write(arg(2), NAME.encode() + b"\0")
            self.ret(8)

    def setup(self, double_handed=1):
        super().setup(double_handed)
        p = self.picker
        self.emu.mem_write(PLACES_AT, b"\x08")
        self.word(p + 0x350, 0x5050)                    # its physics world
        self.eye([0., 0., 1.], [0., 1.0, 0.])
        # The donor's gun on its ragdoll at (0, 1, 1): its model's instance in the scene (what
        # patch 78's hiding takes out), without its resource (replaced as the body went to ragdoll):
        # the box is the cooked gun's, 1 m along z, read once when its cook is logged made.
        renderer, scene = 0x38000000, self.blk(0x100)
        self.emu.mem_map(renderer, 0x201000, UC_PROT_ALL)
        self.word(GAME + 0x5360, renderer)
        self.word(renderer + 0x200064, scene)
        model, self.instance = self.blk(0x120), self.blk(0x6300)
        self.word(WEAPON + 0x1810, model)
        self.word(WEAPON + 0x180C, WORLD)
        self.word(model + 0x108, self.instance)
        self.emu.mem_write(self.instance + 0x10D, b"\1")
        cooked_model, cooked_instance, resource = self.blk(0x120), self.blk(0x6300), self.blk(0x140)
        self.word(self.new_gun + 0x1810, cooked_model)
        self.word(cooked_model + 0x108, cooked_instance)
        self.word(cooked_instance + 0x62B4, resource)
        self.emu.mem_write(resource + 0x108, struct.pack("<6f", -0.05, -0.05, -0.5, 0.05, 0.05, 0.5))
        self.matrix = self.blk(64)
        at = list(IDENTITY)
        at[13], at[14] = 1.0, 1.0
        self.emu.mem_write(self.matrix, struct.pack("<16f", *at))
        # The cooked gun's config: ui_desc/text_descriptions/name, a string.
        entry, holder = self.blk(0x3A0), self.blk(0x110)
        self.word(self.new_gun + 0x10 + 0x25C, entry)
        self.word(entry + 0x114, holder)
        nodes = [self.blk(0x18) for _ in range(4)]
        self.word(holder + 0x108, nodes[0])
        for node in nodes[:3]:
            self.emu.mem_write(node + 0x14, struct.pack("<H", 3))
        text = self.blk(32)
        self.emu.mem_write(text, NAME_KEY + b"\0")
        self.word(nodes[3], text)
        self.emu.mem_write(nodes[3] + 0x14, struct.pack("<H", 5))
        self.cfg.update({(nodes[0], b"ui_desc"): nodes[1], (nodes[1], b"text_descriptions"): nodes[2],
                         (nodes[2], b"name"): nodes[3]})
        # The hud: [esp+4] the ui; its world, its movie.
        self.ui = self.blk(0x20)
        self.word(self.ui + 0x18, WORLD)
        self.word(CLIENT + 4, p)
        self.handler = self.blk(0x400)
        self.step(DIED + 100)

    def eye(self, forward, position):
        m = list(IDENTITY)
        m[8:11], m[12:15] = forward, position
        self.emu.mem_write(self.picker + 0x44C, struct.pack("<16f", *m))

    def draw(self):
        """mt_draw's carry.weapon of the dead donor: whether its native weapon draw goes on."""
        self.lohook(WEAPON_DRAW, WEAPON_DRAWN, {UC_X86_REG_EBX: PLAYER, UC_X86_REG_EAX: self.matrix},
                    stops=(WEAPON_DRAWN, WEAPON_SKIPPED))
        return self.stopped == WEAPON_DRAWN

    def hud(self):
        """update_player_icons' start: the hud's prompts (show_artefact_offer's arguments)."""
        sp = hide.STACK + 0x9000
        self.movies(self.ui, True, False)
        self.word(sp + 4, self.ui)
        self.lohook(ICONS_SITE, ICONS_AFTER, {}, sp=sp)
        return [values for _, method, values in self.invokes() if method == "root.show_artefact_offer"]

    def build(self, actions):
        """The input builder's end: the actions the input carries."""
        self.word(self.handler + 0x33C, actions)
        self.lohook(BUILDER_END, BUILDER_AFTER, {UC_X86_REG_EBX: self.handler})
        return self.read(self.handler + 0x33C)


def stage2b(patched):
    r = Prompt(patched)
    r.setup()
    assert r.draw(), "an open drop's gun is drawn"
    assert r.hud() == [("st_weapon_pick_up", NAME)], r.invokes()
    assert r.hud() == [("st_weapon_pick_up", NAME)]
    assert sights(r) == ["offered"], "once per drop and reason"
    boxes = [l for l in r.log if "the gun box of place 5" in l]
    assert len(boxes) == 1 and boxes[0].endswith(
        "place 3: the gun box of place 5 at (0, 1000, 1000), half sizes (50, 50, 500) mm"), boxes
    print("ok the donor's gun drawn on its ragdoll (carry.weapon: the site's transform; the box its cooked gun's, "
          "nothing of the body's model read): the "
          "picker looking at it within 2.2 m, in sight, is offered it: show_artefact_offer(\"st_weapon_pick_up\", "
          "the gun's ui_desc/text_descriptions/name translated)")

    sent = r.build(USE | 0x21)
    assert sent == command(PLACE) | 0x21, hex(sent)
    assert r.build(USE | 0x01) == 0x01, "Use held after the pickup reached the game"
    assert r.build(0x01) == 0x01 and r.build(USE) == command(PLACE)
    r.build(0)
    assert any("place 3 sends a pickup of the gun of place 5" in l for l in r.log), r.log
    print("ok Use's press on it: the input builder's end sends the pickup command (Use, the quick bits out; fire "
          "kept); held, the game sees no Use until it is let go")

    # The command played: taken; the gun is hidden on the ragdoll, the prompt gone.
    r.word(r.picker + 0x330, sent)
    r.lohook(FILTER_SITE, FILTER_AFTER, {UC_X86_REG_EDI: r.picker, UC_X86_REG_EAX: r.picker + 0x32C})
    r.word(r.picker + 0x330, 0)
    r.tick()
    r.installed()
    # "Already taken" only while the gun is still drawn there (before the ragdoll's next draw hides it).
    assert r.hud() == [("st_weapon_already_taken",)], "the gun still seen at its spot"
    assert sights(r)[-1] == "not offered, already taken", sights(r)
    assert not r.draw() and r.removed == [r.instance], "the taken gun is still drawn"
    assert r.hud() == [("",)] and r.hud() == [], "hidden: nothing left to aim at"
    assert r.build(USE) == USE
    print("ok the command played (carry.filter): taken; its gun out of the scene on the ragdoll (native draw "
          "skipped, as patch 78 hides a carrier's); \"Already taken\" only while it is still seen there, then "
          "nothing at the empty spot; Use the game's again")

    rule = "; held 10 two-handed, other %s, gun two-handed, rule %s"
    def battery(g):
        held = g.blk(0x40)
        g.word(held, BATTERY_VTABLE)
        g.word(g.picker + 0x128, held)
        g.word(g.picker + 0x12C, held)
    switching = lambda g: g.word(g.picker + 0x12C, g.blk(0x40))
    for case, change, sight, shown in (
            # Not aimed at: no prompt, as for the crate and the artefacts.
            ("looking away", lambda g: g.eye([1., 0., 0.], [0., 1.0, 0.]), "in reach, not aimed at its box", []),
            ("25 cm beside the gun", lambda g: g.eye([0., 0., 1.], [0.25, 1.0, 0.]), "in reach, not aimed at its box", []),
            ("at the body 40 cm below it", lambda g: g.eye([0., 0., 1.], [0., 0.6, 0.]),
             "in reach, not aimed at its box", []),
            ("beyond 2.2 m", lambda g: g.eye([0., 0., 1.], [0., 1.0, -2.1]), "out of reach (2.2 m)", []),
            ("a wall between", lambda g: setattr(g, "wall", True), "not offered, no line of sight (the level's ray)", []),
            # Aimed at but refused: the prompt always says why.
            ("a second double-handed gun", lambda g: g.second(double_handed=1),
             "not offered, a second double-handed gun without modifier 25" + rule % ("two-handed", "1 two-handed"),
             [("st_weapon_cannot_carry",)]),
            ("modifier 64 with the gun in slot 10", lambda g: g.modifier(64, 1.0),
             "not offered, its kit carries one gun only, in slot 7 (modifier 64)" + rule % ("none", "1 gun (mod 64)"),
             [("st_weapon_one_gun_only",)]),
            ("a battery in hands", battery, "not offered, no gun of its two weapon slots in its hands",
             [("st_weapon_no_gun_in_hands",)]),
            ("a weapon switch under way", switching, "not offered, a weapon switch under way",
             [("st_weapon_switching",)]),
            ("the match has fewer places", lambda g: g.emu.mem_write(PLACES_AT, b"\x05"), "offered", [])):
        g = Prompt(patched)
        g.setup()
        if change:
            change(g)
        assert g.draw()
        assert g.hud() == shown, (case, g.invokes())
        assert g.build(USE) == USE, case
        assert len(sights(g)) == 1 and sights(g)[0].startswith(sight), (case, sights(g))
    print("ok aimed at but refused, the prompt says why: a second rifle, one gun only (modifier 64), no gun in "
          "hands (a battery), a weapon switch under way, already taken; Use does nothing")
    g = Prompt(patched)
    g.setup()
    g.word(g.picker + 0x324, DIED + 60_000)
    assert g.draw() and g.hud() == [("st_weapon_pick_up", NAME)], "60 s after the death, the body still there"
    g.remove(PLAYER, DIED + 61_000)
    g.word(PLAYER + 0x33C, 1)                           # alive again: no ragdoll draws the gun now
    assert g.hud() == [("st_weapon_pick_up", NAME)], "its respawn leaves the gun where it lay, at its last pose"
    left_match(g, PLACE, PLAYER, KIT, DIED + 600_000)
    assert g.hud() == [("st_weapon_cannot_pick_up",)] and g.build(USE) == USE, "its player left the match"
    assert sights(g) == ["offered", "not offered, its player left the match"], sights(g)
    g = Prompt(patched)
    g.setup()
    g.word(PLAYER + 0x324, DIED + 50)
    g.death()
    assert g.queries == [], "the same life's death again: no second cook"
    assert g.draw() and g.hud() == [("st_weapon_pick_up", NAME)], sights(g)
    print("ok a rollback replays the death at another time: its gun (cooked at the first) is still offered")
    g = Prompt(patched)
    g.setup()
    g.eye([0., -1., 0.], [0.19, 2.0, 1.2])
    g.draw()
    assert g.hud() == [("st_weapon_pick_up", NAME)], "15 cm of padding"
    print("ok the box is the gun's own (10 cm thick, 1 m long, at its transform; logged once a drop): no prompt 25 cm "
          "beside it or at the body below it; no prompt (and Use the game's) looking away, beyond 2.2 m, behind a wall, a second "
          "double-handed gun, modifier 64 in slot 10, or a place the match does not have; the box padded by 15 cm; offered 60 s after the death while the body is there "
          "and after the donor's respawn at the gun's last pose, \"Can't pick this up\" once the donor left; each drop's reason logged once a change (not aimed at, "
          "out of reach, no line of sight, the carry rule with both slots' handedness and the gun's, taken)")


# ---- a dead holder's picked gun drops (the owner, 2026-11-02) ---------------------------------

class Death(Prompt):
    """Stage 2b's match where anyone can die: A (PLAYER, place 5, kit 1) dead with its gun cooked, B the
    picker (place 3, kit 0), C (place 4, kit 0) beside them. The game's dispatch completes the cooks
    in the order they were asked with the inventories queued in `inventories`."""

    def __init__(self, data):
        super().__init__(data)
        self.inventories, self.completed = [], 0

    def dispatch(self, emu, address, *_):
        if not self.inventories or self.completed >= len(self.queries):
            return super().dispatch(emu, address, *_)
        callback, inventory = self.queries[self.completed]["callback"], self.inventories.pop(0)
        self.completed += 1
        self.emu.mem_write(FUNCTOR, callback)
        self.word(RESULT + 0x38, 1)
        self.word(RESULT + 0x12C, inventory)
        self.word(inventory + 0xD0, 1)
        at = self.dispatch_stub
        code = b"\x68" + struct.pack("<I", RESULT) + b"\x68" + struct.pack("<I", FUNCTOR + 8)
        code += b"\xE8" + struct.pack("<i", INVOKER - (at + len(code) + 5)) + b"\x83\xC4\x08\xC3"
        emu.mem_write(at, code)
        emu.reg_write(UC_X86_REG_EIP, at)

    def queue(self, *inventories):
        """Cooks from now on are completed by the game's dispatch with these, in order."""
        self.queries, self.completed, self.inventories = [], 0, list(inventories)

    def die(self, p, at):
        """carry.client_death of `p` at `at` (its death hook, the player intact), then dead (+33Ch):
        the cooks it asked for."""
        self.word(p + 0x324, at)
        self.queries = []
        self.run(DEATH_SITE, (DEATH_AFTER,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_ECX: p})
        assert self.stopped == DEATH_AFTER, hex(self.stopped or 0)
        self.emu.mem_write(p + 0x33C, b"\0")
        return self.queries

    def arm(self, p, inventory, gun_id, at):
        """`p` respawned at `at` holding an own gun in slot 10 (registry id `gun_id`) bound to its
        ammunition in slot 8, as add_picker's: (gun, ammo)."""
        gun, ammo = self.gun(double_handed=1, ammo_slot=8, loaded=12), self.stack(90)
        self.ammo_config(ammo, 217, 30, 1)
        self.emu.mem_write(gun + 0xC, bytes([gun_id]))
        self.word(gun + 0x4, CORE)
        self.word(gun + 0x5A0, BULLETS)
        self.word(self.registry + 4 * gun_id, gun)
        for item, slot in ((gun + 0x10, 10), (ammo, 8)):
            self.word(inventory + 0x10C + 4 * slot, item)
            self.word(item + 0x250, inventory)
            self.word(item + 0x254, slot)
        self.word(gun + 0x538, ammo)
        for off, value in ((0x128, gun), (0x12C, gun), (0x324, at), (0x328, at)):
            self.word(p + off, value)
        self.emu.mem_write(p + 0x33C, bytes([1, 0, 0, 1]))
        return gun, ammo

    def dressed_gun(self, double_handed=1):
        """A cooked inventory (cooked_gun's) whose gun has the drawn model's box (1 m along z) and the
        name config, as the prompt reads them once its cook is logged made: (cooked, gun, ammo)."""
        cooked, gun, ammo = self.cooked_gun(double_handed)
        model, instance, resource = self.blk(0x120), self.blk(0x6300), self.blk(0x140)
        self.word(gun + 0x1810, model)
        self.word(gun + 0x180C, WORLD)
        self.word(model + 0x108, instance)
        self.emu.mem_write(instance + 0x10D, b"\1")
        self.word(instance + 0x62B4, resource)
        self.emu.mem_write(resource + 0x108, struct.pack("<6f", -0.05, -0.05, -0.5, 0.05, 0.05, 0.5))
        entry, holder = self.blk(0x3A0), self.blk(0x110)
        self.word(gun + 0x10 + 0x25C, entry)
        self.word(entry + 0x114, holder)
        nodes = [self.blk(0x18) for _ in range(4)]
        self.word(holder + 0x108, nodes[0])
        for node in nodes[:3]:
            self.emu.mem_write(node + 0x14, struct.pack("<H", 3))
        text = self.blk(32)
        self.emu.mem_write(text, NAME_KEY + b"\0")
        self.word(nodes[3], text)
        self.emu.mem_write(nodes[3] + 0x14, struct.pack("<H", 5))
        self.cfg.update({(nodes[0], b"ui_desc"): nodes[1], (nodes[1], b"text_descriptions"): nodes[2],
                         (nodes[2], b"name"): nodes[3]})
        return cooked, gun, ammo

    def draw_body(self, p):
        """mt_draw's carry.weapon of the dead `p`: whether its native weapon draw goes on."""
        self.lohook(WEAPON_DRAW, WEAPON_DRAWN, {UC_X86_REG_EBX: p, UC_X86_REG_EAX: self.matrix},
                    stops=(WEAPON_DRAWN, WEAPON_SKIPPED))
        return self.stopped == WEAPON_DRAWN

    def viewer(self, p):
        """`p` the local player, looking from (0, 1, 0) along z at the gun drawn at (0, 1, 1)."""
        self.word(CLIENT + 4, p)
        self.word(p + 0x350, 0x5050)
        m = list(IDENTITY)
        m[8:11], m[12:15] = [0., 0., 1.], [0., 1.0, 0.]
        self.emu.mem_write(p + 0x44C, struct.pack("<16f", *m))

    def asked(self, recipe):
        place, kit, slot = recipe
        return [l for l in self.log if "asked for the gun of place %d kit %d slot %d" % (place, kit, slot) in l]


def fresh_client(patched, at):
    """A client that saw none of the match (a late joiner): A (PLAYER, place 5, kit 1), B (place 3) and C
    (place 4) each holding an own gun in slot 10, no death seen, nothing cooked; the network client's
    players as the roster: (run, B, C, A's own gun)."""
    r = Death(patched)
    r.match()
    r.player()
    r.registry = r.blk(0x400)
    r.word(CORE + 0x18B4C, r.registry)
    r.word(CORE + 0x18B50, r.registry + 4 * 12)
    r.roster = {PLACE: PLAYER}

    def active_player(emu, address, *_):          # thiscall: ECX the client, out, place; ret 8
        sp = emu.reg_read(UC_X86_REG_ESP)
        out, place = r.read(sp + 4), r.read(sp + 8)
        player = r.roster.get(place, 0)
        r.word(out, player)
        if player:
            r.word(player + 0xD0, r.read(player + 0xD0) + 1)
        r.ret(8, out)
    r.emu.hook_add(UC_HOOK_CODE, active_player, begin=GET_ACTIVE_PLAYER, end=GET_ACTIVE_PLAYER)
    b, c = r.add_picker(3, GUN_ID), r.add_picker(4, GUN_ID + 2)
    r.picker, r.inventory, r.own_gun, r.own_ammo = b.p, b.inventory, b.own_gun, b.own_ammo
    own_a, _ = r.arm(PLAYER, INVENTORY, GUN_ID + 3, at)
    return r, b, c, own_a


def join(patched, at, states, order=(3, 4, 5)):
    """A late joiner's first world (0xE8) with `states` (place -> its saved record), then each record
    read by its player's load in `order`: (run, players by place, the cooked guns in the order asked,
    A's own gun). Every record it saves is the one it loaded."""
    r, b, c, own_a = fresh_client(patched, at)
    players = {3: b.p, 4: c.p, 5: PLAYER}
    guns = [r.dressed_gun() for _ in range(4)]
    r.queue(*[cooked for cooked, _, _ in guns])
    load_world(r, world_with({place: b"\0\0" + rec for place, rec in states.items()}))
    asked = len(r.queries)
    r.queries = []
    for place in order:
        r.load(players[place], states[place])
    assert r.queries == [], ("the reader cooks nothing", r.log)
    assert not any("NOT installed" in l or "NOT made" in l for l in r.log), r.log
    for place in order:
        assert r.save(players[place]) == states[place], (place, r.log)
    return r, players, guns[:asked], own_a


def death_drop(patched):
    picked_gun_drops(patched)
    picked_into_slot_7(patched)
    own_gun_drops(patched)
    late_join_of_a_picked_drop(patched)
    there_and_back(patched)


def picked_gun_drops(patched):
    """A dies; B takes A's gun over B's own (slot 10); B dies holding it: B's drop is that gun, its copy
    cooked from A's recipe (place 5 kit 1 slot 10, the record's pick) out of A's profile; B's ragdoll
    draws the gun in its hands, the prompt's box is the copy's; C takes it: a gun of A's recipe with
    B's gun's magazine; B's ragdoll then hides it. A rollback across B's death replays it with no cook."""
    r = Death(patched)
    r.setup()
    r.pick()
    r.installed()
    picked = r.new_gun
    c = r.add_picker(4, GUN_ID + 2)
    r.emu.mem_write(picked + 0x5E5, bytes([7]))       # B fired: 7 left
    # B's own gun drawn on a ragdoll is hidden by patch 78's way: its model in the scene.
    instance = r.read(r.read(picked + 0x1810) + 0x108)
    r.word(picked + 0x180C, WORLD)
    r.emu.mem_write(instance + 0x10D, b"\1")
    b, died = r.picker, DIED + 5_000
    before = r.save(b)
    r.log.clear()
    (q,) = r.die(b, died)
    words = struct.unpack("<6I", r.read(struct.unpack_from("<III", q["variant"])[2], 0x18))
    assert words[1] & 0xFF == KIT, ("cooked with A's kit, not B's", words)
    assert r.asked((PLACE, KIT, 10)), r.log
    assert r.save(b) == record(("open", 1, A_GUN), slot10=PICKED), "B's drop: its slot 10, the pick's recipe"
    cooked, gun, ammo = r.dressed_gun()
    r.complete(q, cooked)
    r.step(died + 100)
    assert r.draw_body(b), "B's ragdoll draws the gun in its hands (the picked one)"
    r.viewer(c.p)
    assert r.hud() == [("st_weapon_pick_up", NAME)], r.invokes()
    assert any("place 4: the gun box of place 3 at (0, 1000, 1000), half sizes (50, 50, 500) mm" in l
               for l in r.log), r.log
    print("ok B dies holding A's gun it picked: its drop is that gun (B's record: slot 10 open, the pick of A's "
          "recipe), its copy cooked from A's profile kit 1 slot 10; B's ragdoll draws it; C sees the copy's box "
          "and is offered it")

    # A rollback across B's death: the death replayed finds its copy (no cook), the same record.
    r.load(b, before)
    r.queries = []
    assert r.die(b, died + 30) == [] and r.save(b) == record(("open", 1, A_GUN), slot10=PICKED)
    print("ok a rollback across B's death replays it: its copy found, no cook, the same record")

    r.queries = []
    r.pick(target=3, picker=c.p, now=died + 2_000)
    assert r.queries == [] and r.slot(c.inventory, 10) == gun + 0x10, r.log
    assert r.slot(cooked, 10) == c.own_gun + 0x10, "C's own gun hidden in the copy's cooked inventory"
    assert r.read(gun + 0x5E5, 1)[0] == 7, "B's gun's magazine"
    assert r.read(r.registry + 4 * reserved(4, 10)) == gun
    assert r.slot(r.inventory, 10) == picked + 0x10, "B's body keeps the gun it held"
    assert r.save(c.p) == record(slot10=(died + 2_000, PLACE, KIT, 1)), "C's pick: A's recipe"
    assert r.save(b) == record(("taken", 1, A_GUN), slot10=PICKED)
    r.in_hands(c.p)
    removed = len(r.removed)
    assert not r.draw_body(b) and len(r.removed) == removed + 1, "taken: hidden on B's ragdoll"
    print("ok C takes it: a gun of A's recipe (place 5 kit 1 slot 10) with B's magazine (7), C's own gun hidden; "
          "B's drop taken (its kit and slot kept), its ragdoll's gun hidden")

    # A rollback undoing C's pickup: the copy back in B's drop, open as it was; the pickup again takes it.
    r.load(b, record(("open", 1, A_GUN), slot10=PICKED))
    r.load(c.p, record())
    assert r.slot(c.inventory, 10) == c.own_gun + 0x10 and r.slot(cooked, 10) == gun + 0x10
    r.word(c.p + 0x128, c.own_gun)
    r.word(c.p + 0x12C, c.own_gun)
    r.queries = []
    r.pick(target=3, picker=c.p, now=died + 2_000)
    assert r.queries == [] and r.slot(c.inventory, 10) == gun + 0x10
    assert r.save(c.p) == record(slot10=(died + 2_000, PLACE, KIT, 1))
    print("ok a rollback undoing C's pickup gives the copy back to B's drop; played again, C takes it from there")

    # The picker's death while holstering: cancelled, the drop it came from (B's) open again as it was.
    g = Death(patched)
    g.setup()
    g.pick()
    gc = g.add_picker(4, GUN_ID + 2)
    (q,) = g.die(g.picker, died)
    gcooked, _, _ = g.cooked_gun()
    g.complete(q, gcooked)
    g.pick(target=3, picker=gc.p, now=died + 2_000, switch=False)
    assert g.save(g.picker) == record(("taken", 1, A_GUN), slot10=PICKED)
    g.die(gc.p, died + 2_100)
    assert g.save(g.picker) == record(("open", 1, A_GUN), slot10=PICKED), "B's drop open again (kit 0, slot 10)"
    assert g.save(PLAYER) == record(("taken", 1, A_GUN)), "A's drop untouched"
    print("ok C dies while holstering it: cancelled, B's drop open again as it was (not A's)")


def picked_into_slot_7(patched):
    """The pistol-slot variant: A's pistol (its slot 10) picked into B's slot 7 (B holding its rifle there);
    B dies holding it: B's drop is slot 7, its copy cooked from A's slot 10 (the cooked gun in slot 10);
    C, holding its own rifle in slot 10, takes the pistol into slot 10 with B's pistol's magazine."""
    r = Death(patched)
    r.setup(double_handed=0)
    rifle = r.second(double_handed=1)
    assert r.switch(rifle) == rifle
    r.pick()
    pistol = r.new_gun
    assert r.slot(r.inventory, 7) == pistol + 0x10
    r.emu.mem_write(pistol + 0x5E5, bytes([5]))
    c = r.add_picker(4, GUN_ID + 2)
    died = DIED + 5_000
    (q,) = r.die(r.picker, died)
    assert r.asked((PLACE, KIT, 10)), r.log
    assert r.save(r.picker) == record(("open", 0, A_GUN), slot7=PICKED)
    cooked, gun, _ = r.dressed_gun(double_handed=0)
    r.complete(q, cooked)
    r.step(died + 100)
    assert r.draw_body(r.picker)
    r.viewer(c.p)
    assert r.hud() == [("st_weapon_pick_up", NAME)], r.invokes()
    r.pick(target=3, picker=c.p, now=died + 2_000)
    assert r.slot(c.inventory, 10) == gun + 0x10 and r.read(gun + 0x5E5, 1)[0] == 5, r.log
    assert r.save(c.p) == record(slot10=(died + 2_000, PLACE, KIT, 1))
    assert r.save(r.picker) == record(("taken", 0, A_GUN), slot7=PICKED)
    print("ok the pistol-slot variant: A's slot-10 pistol picked into B's slot 7; B's drop is slot 7, its copy "
          "cooked from A's slot 10; C takes it into its slot 10 with B's pistol's magazine (5)")


def own_gun_drops(patched):
    """B dies holding its own gun: unchanged, its own stock gun (place 3 kit 0 slot 10) from its profile."""
    r = Death(patched)
    r.setup()
    (q,) = r.die(r.picker, DIED + 5_000)
    words = struct.unpack("<6I", r.read(struct.unpack_from("<III", q["variant"])[2], 0x18))
    assert words[1] & 0xFF == 0 and r.asked((3, 0, 10)), r.log
    assert r.save(r.picker) == record(("open", 1, (3, 0, 1)))
    # B picked into slot 10 but dies holding its own slot-7 gun: that one.
    r = Death(patched)
    r.setup()
    r.pick()
    own7 = r.second(double_handed=0)
    assert r.switch(own7) == own7
    (q,) = r.die(r.picker, DIED + 5_000)
    assert r.asked((3, 0, 7)) and r.save(r.picker) == record(("open", 0, (3, 0, 0)), slot10=PICKED), r.log
    print("ok B dies holding its own gun: its own stock gun drops (place 3 kit 0, from its profile), also beside "
          "a gun picked into its other slot")


def late_join_of_a_picked_drop(patched):
    """A fresh client loading B's open drop of A's gun (B dead holding the pick): the drop's copy and the
    pick B holds each cooked before the load (the open drop's copy is not idle for the pick), the pick
    installed from the other; the records saved as loaded; C's pickup there takes the drop's copy with no
    cook nor wait."""
    died = DIED + 5_000
    states = {3: record(("open", 1, A_GUN), slot10=PICKED), 4: record(), 5: record(("taken", 1, A_GUN))}
    r, players, guns, _ = join(patched, died, states)
    assert len(guns) == 2, r.log
    assert any("a world to load has the open drop the gun of place 5 kit 1 slot 10 (of place 3)" in l for l in r.log), r.log
    (drop_inv, drop_gun, _), (pick_inv, pick_gun, _) = guns
    assert r.slot(r.inventory, 10) == pick_gun + 0x10 and r.slot(pick_inv, 10) == r.own_gun + 0x10
    assert r.slot(drop_inv, 10) == drop_gun + 0x10, "the drop's copy stays its drop's"
    c = players[4]
    r.word(players[3] + 0x33C, 0)
    r.pick(target=3, picker=c, now=died + 2_000)
    assert r.queries == [] and r.slot(r.read(c + 0x10C), 10) == drop_gun + 0x10, r.log
    assert r.save(c) == record(slot10=(died + 2_000, PLACE, KIT, 1))
    assert r.save(players[3]) == record(("taken", 1, A_GUN), slot10=PICKED)
    print("ok a late joiner loading B's open drop of A's gun: the drop's copy and B's pick each cooked before "
          "the load from A's recipe, B's pick installed from the other; the records as loaded; C's pickup takes "
          "the drop's copy (no cook, no wait), the same records as on the clients that saw it")


def there_and_back(patched):
    """The owner's chain (2026-11-02): A dies, B takes A's gun; A respawns; B dies holding it (its drop a
    copy of A's recipe); A, holding its own gun of that recipe, takes it back from B's body as any pickup
    (its own gun hidden in the copy's cooked inventory); B respawns (A's first copy let go); A dies holding
    the copy: A's drop re-uses the first copy (no cook); B leaves (its last copy, now A's drop's, kept); C
    takes it from A's body; A respawns: its own gun back once, the copy let go; C dies holding it and A
    takes it back again from C's body: still the two copies. Rollbacks across B's death and C's pickup and
    late joiners at two points give the same records."""
    r = Death(patched)
    r.setup()
    first, first_inv = r.new_gun, r.cooked
    b, c = r.picker, r.add_picker(4, GUN_ID + 2)
    cooks = 1
    r.pick()                                            # B takes A's gun (first)
    r.remove(PLAYER, DIED + 8_000)                     # A respawns: its first drop stays (taken, alone)
    assert r.save(PLAYER) == record(("taken", 1, A_GUN, True))
    own_a, own_a_ammo = r.arm(PLAYER, INVENTORY, GUN_ID + 3, DIED + 8_000)

    t3 = DIED + 10_000
    before_b, before_a = r.save(b), r.save(PLAYER)
    (q,) = r.die(b, t3)                                 # B dies holding it: a copy of A's recipe (second)
    cooks += 1
    assert r.asked((PLACE, KIT, 10)), r.log
    second_inv, second, second_ammo = r.cooked_gun()
    r.complete(q, second_inv)
    r.step(t3 + 100)

    t4 = t3 + 1_000
    a_pick = (t4, PLACE, KIT, 1)
    r.queries = []
    r.pick(target=3, picker=PLAYER, now=t4)            # A takes it back: a normal pickup
    assert r.queries == [], "no cook"
    assert r.slot(INVENTORY, 10) == second + 0x10 and r.slot(second_inv, 10) == own_a + 0x10, \
        "A's own gun hidden in the copy's cooked inventory (its twin)"
    assert r.slot(INVENTORY, 8) == second_ammo and r.slot(second_inv, 11) == own_a_ammo, "its ammunition replaced"
    assert r.read(r.registry + 4 * (GUN_ID + 3)) == own_a, "A's own gun keeps its id and entry"
    assert r.read(r.registry + 4 * reserved(PLACE, 10)) == second
    assert r.slot(r.inventory, 10) == first + 0x10, "B's body keeps the first copy: never handed out"
    p1 = {3: record(("taken", 1, A_GUN), slot10=PICKED), 4: record(), 5: record(("taken", 1, A_GUN, True), slot10=a_pick)}
    assert {place: r.save(p) for place, p in ((3, b), (4, c.p), (5, PLAYER))} == p1
    r.in_hands(PLAYER)
    print("ok A, holding its own gun of the same recipe, takes its gun back from B's body as any pickup: its own "
          "gun and ammunition hidden in the copy's cooked inventory, the copy in its slot 10 under A's reserved id; "
          "B's body keeps the first copy")

    # A rollback across B's death: A's pickup undone (the copy back in B's drop, A's own gun back), B alive.
    r.load(b, before_b)
    r.load(PLAYER, before_a)
    r.reader(GUN_ID + 3, player=PLAYER)
    assert r.slot(INVENTORY, 10) == own_a + 0x10 and r.slot(second_inv, 10) == second + 0x10
    r.emu.mem_write(b + 0x33C, bytes([1]))
    assert r.die(b, t3) == [], "the death replayed finds its copy"
    r.pick(target=3, picker=PLAYER, now=t4)
    assert r.queries == [] and r.slot(INVENTORY, 10) == second + 0x10
    assert {place: r.save(p) for place, p in ((3, b), (4, c.p), (5, PLAYER))} == p1
    print("ok a rollback across B's death and A's pickup: replayed with no cook, the same records")

    j, players, guns, _ = join(patched, t4, p1)
    assert len(guns) == 2, ("A's and B's picks each a copy", j.log)
    held_a, held_b = j.slot(INVENTORY, 10), j.slot(j.inventory, 10)
    assert {held_a, held_b} == {g + 0x10 for _, g, _ in guns}, "two copies, one each"
    print("ok a late joiner while A holds it: both picks of A's recipe (B's body's, A's) cooked before the load, "
          "one each; the records saved as loaded")

    r.remove(b, t4 + 7_000)                            # B respawns: the first copy let go
    assert r.slot(r.inventory, 10) == r.own_gun + 0x10 and r.slot(first_inv, 10) == first + 0x10
    t6 = t4 + 9_000
    r.log.clear()
    assert r.die(PLAYER, t6) == [], "A's drop re-uses the first copy"
    assert any("re-using a kept copy of the gun of place 5 kit 1 slot 10 for the drop of place 5: gun %#x" % first
               in l for l in r.log), r.log
    p2 = {3: record(("taken", 1, A_GUN, True)), 4: record(), 5: record(("open", 1, A_GUN), slot10=a_pick)}
    assert {place: r.save(p) for place, p in ((3, b), (4, c.p), (5, PLAYER))} == p2
    # B leaves: the first copy, which B held last, is A's drop's now: kept.
    r.destroy_player(b)
    r.step(t6 + 60_000)
    assert r.read(first_inv + 0xD0) == 2, "freed with its last holder while it lies in another body's drop"

    t7 = t6 + 1_000
    r.pick(target=5, picker=c.p, now=t7)
    assert r.queries == [] and r.slot(c.inventory, 10) == first + 0x10 and r.slot(first_inv, 10) == c.own_gun + 0x10
    p3 = {4: record(slot10=(t7, PLACE, KIT, 1)), 5: record(("taken", 1, A_GUN), slot10=a_pick)}
    assert {place: r.save(p) for place, p in ((4, c.p), (5, PLAYER))} == p3
    print("ok A dies holding the copy: its drop is it (A's recipe) and re-uses the first copy, no cook; B leaves "
          "and the first copy, now A's drop's, is kept; C takes it from A's body")

    # A rollback undoing C's pickup: the first copy back in A's drop; played again.
    r.load(PLAYER, p2[5])
    r.load(c.p, record())
    r.reader(GUN_ID + 2, player=c.p)
    assert r.slot(c.inventory, 10) == c.own_gun + 0x10 and r.slot(first_inv, 10) == first + 0x10
    r.pick(target=5, picker=c.p, now=t7)
    assert r.queries == [] and r.slot(c.inventory, 10) == first + 0x10
    assert {place: r.save(p) for place, p in ((4, c.p), (5, PLAYER))} == p3

    # A late joiner while A lies dead holding it, its drop open: the drop's copy and A's pick each cooked
    # (both of A's recipe: the reader may install either, the other is the drop's); C's pickup there takes
    # the one left in the drop.
    j, players, guns, _ = join(patched, t6, {**p2, 3: record()})
    copies = [g + 0x10 for _, g, _ in guns]
    assert len(guns) == 2 and j.slot(INVENTORY, 10) in copies, j.log
    j.emu.mem_write(PLAYER + 0x33C, b"\0")
    j.pick(target=5, picker=players[4], now=t7)
    assert j.queries == [] and {j.slot(j.read(players[4] + 0x10C), 10), j.slot(INVENTORY, 10)} == set(copies), j.log
    assert {place: j.save(players[place]) for place in (4, 5)} == p3
    print("ok a rollback undoing C's pickup and a late joiner while A lies dead holding it: the same records")

    # A respawns: its own gun back once, the copy let go; a second removal changes nothing.
    r.remove(PLAYER, t7 + 7_000)
    restored = [r.slot(INVENTORY, 10), r.slot(INVENTORY, 8), r.slot(second_inv, 10), r.slot(second_inv, 11)]
    assert restored[0] == own_a + 0x10 and restored[1] == own_a_ammo and restored[2] == second + 0x10, \
        [hex(x) for x in restored]
    assert r.read(r.registry + 4 * reserved(PLACE, 10)) == 0 and r.read(r.registry + 4 * (GUN_ID + 3)) == own_a
    r.remove(PLAYER, t7 + 7_001)
    assert [r.slot(INVENTORY, 10), r.slot(INVENTORY, 8), r.slot(second_inv, 10), r.slot(second_inv, 11)] == restored
    r.word(PLAYER + 0x128, own_a)
    r.word(PLAYER + 0x12C, own_a)
    r.emu.mem_write(PLAYER + 0x33C, bytes([1, 0, 0, 1]))

    # Round two: C dies holding the first copy (its drop re-uses the second, let go), A takes it back again.
    t9 = t7 + 9_000
    assert r.die(c.p, t9) == [], "C's drop re-uses the second copy"
    t10 = t9 + 1_000
    r.pick(target=4, picker=PLAYER, now=t10)
    assert r.queries == [] and r.slot(INVENTORY, 10) == second + 0x10 and r.slot(second_inv, 10) == own_a + 0x10
    assert r.save(PLAYER) == record(("taken", 1, A_GUN, True), slot10=(t10, PLACE, KIT, 1)), "its drop C took stays, alone"
    assert r.save(c.p) == record(("taken", 1, A_GUN), slot10=(t7, PLACE, KIT, 1))
    assert cooks == 2 and r.read(first_inv + 0xD0) == 2 and r.read(second_inv + 0xD0) == 2
    print("ok A respawns: its own gun and ammunition back once (a second removal changes nothing), the copy let "
          "go; round two (C dies holding the first copy, A takes it back from C's body): the drop re-uses the "
          "second copy; A -> B -> A -> C -> A with two copies in all, none freed while held or in a drop")


# ---- a drop lasts until its player dies again (the owner, 2026-11-02) ------------------------------

def linger(patched):
    picked_after_the_respawn(patched)
    replaced_by_the_next_death(patched)
    late_join_after_the_respawn(patched)


def picked_after_the_respawn(patched):
    """A respawns 8 s after its death: its drop stays, alone; B takes it 10 min on, with a full magazine. A
    rollback to before the respawn gives the body's magazine; one into the holster and one undoing the pickup
    play out alike."""
    r = Death(patched)
    r.setup()
    respawn = DIED + 8_000
    on_body = r.save(PLAYER)
    r.remove(PLAYER, respawn)
    alone = record(("open", 1, A_GUN, True))
    assert r.save(PLAYER) == alone
    taken = DIED + 600_000
    r.pick(now=taken, switch=False)
    holstering = r.save(r.picker)
    r.tick(now=taken + 300)
    r.installed()
    assert r.read(r.new_gun + 0x5E5, 1)[0] == CAPACITY, "alone: a full magazine"
    assert r.save(PLAYER) == record(("taken", 1, A_GUN, True))
    # Into the holster: pending again, then the same swap.
    r.load(PLAYER, record(("taken", 1, A_GUN, True)))
    r.load(r.picker, holstering)
    r.reader(GUN_ID)
    r.word(r.picker + 0x12C, r.read(r.registry + 4 * PENDING_ID))
    r.tick(now=taken + 300)
    r.installed()
    # Undoing the pickup: the copy back in the drop (alone), taken again with no cook.
    r.load(PLAYER, alone)
    r.load(r.picker, record())
    r.reader(GUN_ID)
    r.queries = []
    r.pick(now=taken)
    assert r.queries == [] and r.slot(r.inventory, 10) == r.new_gun + 0x10
    print("ok a drop after its donor's respawn: open, alone; taken 10 min on with a full magazine; a rollback into "
          "the holster and one undoing the pickup play out alike, no cook")

    # A rollback to before the respawn: on its body again, its magazine (17 and a chambered round) the body's.
    g = Death(patched)
    g.setup()
    g.remove(PLAYER, respawn)
    g.load(PLAYER, on_body)
    g.queries = []
    g.pick(now=respawn - 100)
    assert g.queries == [] and g.slot(g.inventory, 10) == g.new_gun + 0x10, g.log
    assert g.read(g.new_gun + 0x5E5, 1)[0] == 17 and g.read(g.new_gun + 0x5F1, 1)[0] == 1
    print("ok a rollback across the respawn: the drop on its body again, the pickup gets the body's magazine")


def replaced_by_the_next_death(patched):
    """A's drop (its slot-10 gun) stays past its respawn; A dies again holding its slot-7 gun: that is its drop
    now (at most one a place), the old copy, never held, freed 10 s after it ended. A rollback across the second
    death brings the old drop back (its copy, no cook) and the death replayed brings the new one back (no cook)."""
    second = DIED + 30_000

    def died_twice():
        r = Death(patched)
        r.setup()
        old = (r.cooked, r.new_gun)
        r.remove(PLAYER, DIED + 8_000)
        before = r.save(PLAYER)
        r.player(slot=7, spawned=DIED + 8_000)
        r.word(PLAYER + 0x324, second)
        r.queries = []
        r.death()
        (q,) = r.queries
        assert r.asked((PLACE, KIT, 7)), r.log
        new_inv, new_gun, new_ammo = r.cooked_gun()
        for item, slot in ((new_gun + 0x10, 7), (new_ammo, 11)):
            r.word(new_inv + 0x10C + 4 * slot, item)
            r.word(item + 0x254, slot)
        r.word(new_inv + 0x10C + 4 * 10, 0)
        r.complete(q, new_inv)
        return r, old, new_gun, before

    replaced = record(("open", 0, (PLACE, KIT, 0)))
    r, (old_inv, old_gun), new_gun, before = died_twice()
    assert r.save(PLAYER) == replaced, "the second death's drop replaces the first"
    r.step(second + 100)
    assert r.read(old_inv + 0xD0) == 2, "freed inside a rollback's reach of its end"
    r.step(second + 10_100)
    assert r.read(old_inv + 0xD0) == 1, "the replaced drop's copy, never held, freed past the rollback margin"
    r.queries = []
    r.pick(now=second + 11_000, switch=False)
    assert r.queries == [] and r.read(r.registry + 4 * PENDING_ID) == new_gun, r.log
    print("ok the next death replaces the drop (slot 7's gun now): the pickup takes its copy; the old copy, never "
          "held, freed 10 s after its end")

    # A rollback across the second death: the old drop's copy its drop again; replayed to the death (the
    # pickup undone), the death again finds its own copy.
    r, (old_inv, old_gun), new_gun, before = died_twice()
    r.step(second + 100)
    r.load(PLAYER, before)
    r.queries = []
    r.pick(now=second - 50, switch=False)
    assert r.queries == [] and r.read(r.registry + 4 * PENDING_ID) == old_gun, r.log
    r.load(r.picker, record())
    r.reader(GUN_ID)
    r.load(PLAYER, before)
    r.word(PLAYER + 0x324, second)
    r.queries = []
    r.death()
    assert r.queries == [] and r.save(PLAYER) == replaced, r.log
    r.pick(now=second + 1_000, switch=False)
    assert r.queries == [] and r.read(r.registry + 4 * PENDING_ID) == new_gun, r.log
    r.step(second + 60_000)
    assert r.read(old_inv + 0xD0) == 2, "registered once: kept until its holder leaves"
    print("ok a rollback across the second death brings the old drop and its copy back (a pickup takes it), the "
          "death replayed its own copy back; no cook")


def late_join_after_the_respawn(patched):
    """A game joins after A respawned: its first world has A's drop open, alone; its copy is cooked before the
    load (this game has no pose for it: it draws and offers nothing), and B's pickup there takes it with no cook,
    a full magazine, the same records as the games that saw the death."""
    at = DIED + 60_000
    states = {3: record(), 4: record(), 5: record(("open", 1, A_GUN, True))}
    j, players, guns, _ = join(patched, at, states)
    assert len(guns) == 1, j.log
    j.pick(target=5, picker=players[3], now=at + 1_000)
    gun = guns[0][1]
    assert j.queries == [] and j.slot(j.inventory, 10) == gun + 0x10, j.log
    assert j.read(gun + 0x5E5, 1)[0] == CAPACITY
    assert j.save(players[3]) == record(slot10=(at + 1_000, PLACE, KIT, 1))
    assert j.save(PLAYER) == record(("taken", 1, A_GUN, True))
    print("ok a late join after the respawn: the open drop, alone, cooked before the load; B's pickup takes it, a "
          "full magazine, the same records")


# ---- one tick: a leave, a death and a pickup (the review, 2026-11-02) ------------------------------

LEFT_MATCH, PLAYER_VTABLE = 0x316BE0, 0x9FB35C


def left_match(r, place, player, kit, at):
    """A 0xEA leave of `place` as tick_events' process_events (0x2D22C5) plays it: the rule's
    on_player_left_match 0x316BE0 (its place's slot emptied, the place's current kit player's remove(1)
    through its vtable +24h, player::remove 0x1F2870), up to base_player::remove's hook."""
    assert r.read(PLAYER_VTABLE + 0x24) == PLAYER_REMOVE, "the player's vtable +24h is player::remove"
    vtable, rule, holder, entries = r.blk(0x40), r.blk(0x200), r.blk(0x18460), r.blk(0x30 * 20)
    r.word(vtable + 0x24, PLAYER_REMOVE)
    r.word(player, vtable)
    r.word(rule + 0x134, holder)
    r.word(holder + 0x18458, entries)
    r.emu.mem_write(entries + 0x30 * place + 0xC, bytes([kit]))
    r.word(entries + 0x30 * place + 4 * kit, player)
    r.word(player + 0x324, at)
    sp = hide.STACK + 0x9000
    r.word(sp, hide.SENTINEL)
    r.word(sp + 4, place)
    r.run(LEFT_MATCH, (0x2DA796,), sp, {**hide.GAME, UC_X86_REG_ECX: rule})
    assert r.stopped == 0x2DA796, hex(r.stopped or 0)
    assert r.read(rule + 0x138 + 4 * place) == 0xFFFFFFFF
    # Then game_world_core::on_player_left_match 0x2D2040 clears the place's bit of the active places
    # (core+18FD8h, 0x2D214E), before the tick's players: core_active_player 0x2D0AB0 refuses it from then on.
    r.roster.pop(place, None)


def unarmed_death(r, at):
    """A (PLAYER) dies again at `at` holding nothing: its drop is none now."""
    r.word(PLAYER + 0x128, 0)
    r.word(PLAYER + 0x324, at)
    r.queries = []
    r.death()
    assert r.queries == [] and r.save(PLAYER) == record()


def one_tick(patched):
    """tick_events 0x2D21E0 plays a tick's events (process_events 0x2D22C5) before its players' tick
    (atomic_tick 0x2D22E2, where inputs and a pickup are played). D (place 5) leaves the match with its
    drop open and P (place 3) sends its pickup in the same tick: the leave comes first (its remove through
    player::remove, then its place out of the world), so P is refused (no player for the place) and D's
    drop stays, alone, in every game. A death and a pickup in one tick follow the players'
    order there: the death first opens the drop the pickup then takes; the pickup first finds none."""
    r = Death(patched)
    r.setup()
    t = DIED + 3_000
    left_match(r, PLACE, PLAYER, KIT, t)
    assert r.save(PLAYER) == record(("open", 1, A_GUN, True)), "a departed player's drop stays"
    r.pick(now=t)
    assert any("refused, the place to cook it from has no player" in l for l in r.log) and r.native == [], r.log
    assert r.slot(r.inventory, 10) == r.own_gun + 0x10 and r.save(r.picker) == record()
    assert r.save(PLAYER) == record(("open", 1, A_GUN, True))
    print("ok one tick, D leaves (on_player_left_match 0x316BE0 -> player::remove 0x1F2870 -> base_player::remove, "
          "then its place out of the world) and P picks D's gun up: the leave first, P refused, D's drop stays")

    # A death and a pickup in one tick, C (place 4) dying.
    for death_first in (True, False):
        g = Death(patched)
        g.setup()
        c = g.add_picker(4, GUN_ID + 2)
        t = DIED + 5_000
        if not death_first:
            g.pick(target=4, now=t)
            assert any("refused, no drop to take" in l for l in g.log), g.log
        (q,) = g.die(c.p, t)
        cooked, gun, _ = g.cooked_gun()
        g.complete(q, cooked)
        if death_first:
            g.pick(target=4, now=t)
            assert g.slot(g.inventory, 10) == gun + 0x10, g.log
            assert g.save(c.p) == record(("taken", 1, (4, 0, 1)))
        else:
            assert g.slot(g.inventory, 10) == g.own_gun + 0x10 and g.save(c.p) == record(("open", 1, (4, 0, 1)))
    print("ok one tick, a death and a pickup of its gun: the death first, taken; the pickup first, refused (the "
          "drop open after it)")


def sights(run):
    """The sight lines of the gun of place 5 the picker logged."""
    head = "pickup: place 3 sees the gun of place 5: "
    return [l.split(head, 1)[1] for l in run.log if head in l]





if __name__ == '__main__':
    main()
