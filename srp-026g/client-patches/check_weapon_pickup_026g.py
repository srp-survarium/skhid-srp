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
CORE = MATCH + 0x1F8
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

    stage2(patched, original)


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
NEW_SPANS = {SAVE_SITE: 6, LOAD_SITE: 6, REMOVE_SITE: 6, CAN_USE: 6, HINT: 6, RESUPPLY: 6, CLIENT_RESUPPLY: 6}
WRITE_U8, DEACTIVATE, FORGET, SET_USER, INITIALIZE, ACTIVATE, ACTIVE_PLAYER = (
    0x1F3DE0, 0x244860, 0x3919E0, 0x2445E0, 0x2449E0, 0x244610, 0x2D0AB0)
SET_OWNER = 0x243590                        # inventory_item::set_owner: the item's +250h/+254h (ret 8)
CHORD = 1 << 29
PATTERNS = [v for v in range(64) if bin(v).count("1") >= 2][:20]
GUN_ID, BULLETS, CAPACITY = 4, 0xB0B0, 30


def command(target):
    return CHORD | PATTERNS[target] << 14


def record(drop=None, borrow=None):
    """weapon_pickup::Record::encode: drop ('open', kit, slot, died) / 'taken'; borrow (donor, kit, slot, own)."""
    out = bytearray(12)
    out[0] = 1
    if drop == "taken":
        out[1] = 2
    elif drop:
        _, kit, slot, died = drop
        out[1], out[2], out[3] = 1, kit, slot
        out[4:8] = struct.pack("<I", died)
    if borrow:
        donor, kit, slot, own = borrow
        out[8:12] = bytes([1, donor, kit | slot << 4, own])
    return bytes(out)


class Pick(Run):
    """The match of stage 1 plus a picker (place 3, kit 0) holding its own gun in slot 10, a
    cooked inventory with the donor's gun in slot 10 and its ammunition in slot 11, the core's
    registry; the game's inventory set_item 0x2FC620, remove_item 0x2FC680 and set_owner
    0x243590 run as they are; the weapon's switch calls are recorded."""

    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(PICK, 0x100000, UC_PROT_ALL)
        self.next, self.written, self.native = PICK, bytearray(), []
        for at in (WRITE_U8, DEACTIVATE, FORGET, SET_USER, INITIALIZE, ACTIVATE, ACTIVE_PLAYER):
            emu.hook_add(UC_HOOK_CODE, self.natives, begin=at, end=at)
        self.bare_ret = self.blk(16)
        emu.mem_write(self.bare_ret, b"\xC3")
        self.item_vtable = self.blk(0x40)
        self.word(self.item_vtable + 0x28, SET_OWNER)
        self.word(self.item_vtable + 0x34, self.bare_ret)

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
            self.ret(4)

    def gun(self, double_handed=1, ammo_slot=11, loaded=0, chamber=0):
        """A client weapon (active object, its inventory item +10h): one bound ammunition slot."""
        gun = self.blk(0x700)
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

    def setup(self, double_handed=1):
        """The donor of stage 1 (place 5, kit 1, its gun in slot 10: 17 rounds and a chambered one)
        dies; its cook completes with the cooked inventory; the picker at place 3."""
        self.match()
        self.player()
        self.registry = self.blk(0x40)
        self.word(CORE + 0x18B4C, self.registry)
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
        self.picker = p = self.blk(0x10000)
        self.roster[3] = p
        self.inventory = self.blk(0x200)
        self.own_gun = self.gun(ammo_slot=8, loaded=12)
        self.own_ammo = self.stack(90)
        self.emu.mem_write(self.own_gun + 0xC, bytes([GUN_ID]))
        self.word(self.own_gun + 0x4, CORE)
        self.word(self.own_gun + 0x5A0, BULLETS)
        self.word(self.registry + 4 * GUN_ID, self.own_gun)
        for item, slot in ((self.own_gun + 0x10, 10), (self.own_ammo, 8)):
            self.word(self.inventory + 0x10C + 4 * slot, item)
            self.word(item + 0x250, self.inventory)
            self.word(item + 0x254, slot)
        self.word(self.own_gun + 0x538, self.own_ammo)
        self.word(p + 0xD0, 3)
        self.emu.mem_write(p + 0x114, bytes([3, 0]))
        self.word(p + 0x10C, self.inventory)
        self.word(p + 0x120, CORE)
        self.word(p + 0x128, self.own_gun)
        self.word(p + 0x12C, self.own_gun)
        self.word(p + 0x324, DIED + 1_000)
        self.word(p + 0x328, 1000)
        self.word(p + 0x6B8, PROFILE)
        self.word(p + 0xF474, WORLD)
        self.emu.mem_write(p + 0x33C, bytes([1, 0, 0, 1]))

    def slot(self, inventory, n):
        return self.read(inventory + 0x10C + 4 * n)

    def lohook(self, site, after, regs, stops=None, sp=None):
        top = sp or hide.STACK + 0x9000
        out, esp = self.run(site, stops or (after,), top, {**hide.GAME, **regs})
        assert self.stopped in (stops or (after,)), (hex(site), hex(self.stopped or 0))
        return out, esp

    def pick(self, target=PLACE, now=None):
        if now is not None:
            self.word(self.picker + 0x324, now)
        self.word(self.picker + 0x330, command(target))
        self.native = []
        self.lohook(FILTER_SITE, FILTER_AFTER, {UC_X86_REG_EDI: self.picker, UC_X86_REG_EAX: self.picker + 0x32C})

    def save(self, player):
        self.written = bytearray()
        self.lohook(SAVE_SITE, SAVE_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_ESI: 0x5A5A0000})
        return bytes(self.written)

    def load(self, player, data, size=None):
        bytes_at, reader = self.blk(0x40), self.blk(12)
        self.emu.mem_write(bytes_at, data)
        for off, value in ((0, bytes_at), (4, bytes_at), (8, len(data) if size is None else size)):
            self.word(reader + off, value)
        self.native = []
        self.lohook(LOAD_SITE, LOAD_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_ESI: reader})
        return self.read(reader + 4) - bytes_at

    def installed(self):
        """The borrowed gun is in the picker's slot 10 and its ammunition in slot 9, the own gun in
        the cooked slot 10, each item's owner and slot set by the game's set_owner; the gun bound."""
        inv, cooked, gun, own = self.inventory, self.cooked, self.new_gun, self.own_gun
        assert self.slot(inv, 10) == gun + 0x10 and self.slot(inv, 9) == self.new_ammo
        assert self.slot(inv, 8) == self.own_ammo and self.slot(cooked, 10) == own + 0x10
        assert self.slot(cooked, 11) == 0
        assert (self.read(gun + 0x10 + 0x250), self.read(gun + 0x10 + 0x254)) == (inv, 10)
        assert (self.read(self.new_ammo + 0x250), self.read(self.new_ammo + 0x254)) == (inv, 9)
        assert (self.read(own + 0x10 + 0x250), self.read(own + 0x10 + 0x254)) == (cooked, 10)
        assert [self.read(x + 0xD0) for x in (gun + 0x10, own + 0x10, self.new_ammo)] == [1, 1, 1]
        assert self.read(gun + 4) == CORE and self.read(gun + 0xC, 1)[0] == GUN_ID
        assert self.read(gun + 0x10 + 0x248) == CORE and self.read(self.new_ammo + 0x248) == CORE
        assert self.read(gun + 0x5A0) == BULLETS and self.read(gun + 0x5F4, 1)[0] == 0
        assert self.read(self.read(gun + 0x5AC)) == 9
        assert self.read(self.registry + 4 * GUN_ID) == gun

    def restored(self):
        """Everything as before the pickup: the cooked inventory as cooked, the own gun back."""
        inv, cooked, gun, own = self.inventory, self.cooked, self.new_gun, self.own_gun
        assert self.slot(inv, 10) == own + 0x10 and self.slot(inv, 9) == 0 and self.slot(inv, 8) == self.own_ammo
        assert self.slot(cooked, 10) == gun + 0x10 and self.slot(cooked, 11) == self.new_ammo
        assert (self.read(own + 0x10 + 0x250), self.read(own + 0x10 + 0x254)) == (inv, 10)
        assert (self.read(gun + 0x10 + 0x250), self.read(gun + 0x10 + 0x254)) == (cooked, 10)
        assert (self.read(self.new_ammo + 0x250), self.read(self.new_ammo + 0x254)) == (cooked, 11)
        assert [self.read(x + 0xD0) for x in (gun + 0x10, own + 0x10, self.new_ammo)] == [1, 1, 1]
        assert self.read(gun + 4) == 0 and self.read(gun + 0xC, 1)[0] == 0 and self.read(gun + 0x5A0) == 0
        assert self.read(gun + 0x10 + 0x248) == 0 and self.read(self.new_ammo + 0x248) == 0
        assert self.read(self.read(gun + 0x5AC)) == 11
        assert self.read(self.registry + 4 * GUN_ID) == own


def no_branch_into(original):
    """Nothing of the original exe branches into bytes 1.. of a new span (selftest's scan)."""
    import selftest_026g as test
    stock = test.Image(original)
    for va, size in NEW_SPANS.items():
        spelled = [va + i for i in range(1, size) if original.find(struct.pack("<I", va + i)) != -1]
        hits = [x for x in test.aimed_inside(original, va, size, 0x82)
                if x not in spelled and test.starts_instruction(stock, x)]
        assert not hits, (hex(va), [hex(h) for h in hits])


def stage2(patched, original):
    no_branch_into(original)
    print("ok nothing in the original branches into the new spans (0x2DAF70, 0x2DB4CF, 0x2DA790, 0x2F809C, "
          "0x2F80E4, 0x2F81B8, 0x2954EF)")

    r = Pick(patched)
    r.setup()
    assert r.save(PLAYER) == record(("open", KIT, 1, DIED)), r.written.hex()
    assert r.save(r.picker) == record()
    print("ok the donor's death opens its drop (kit 1, slot 10, its time) in its record; the picker's is empty; "
          "each written by the native byte writer at base_player::serialize's start")

    # A pickup of another kind: refused, nothing changed.
    other = Pick(patched)
    other.setup(double_handed=0)
    other.pick()
    assert other.slot(other.inventory, 10) == other.own_gun + 0x10 and other.native == []
    assert any("refused, the gun is not of the held gun's kind" in l for l in other.log), other.log
    other.pick(target=3)
    assert any("refused, no drop to take" in l for l in other.log), other.log
    print("ok a gun of another kind, or the picker's own place: refused and logged, nothing changed")

    # The pickup.
    r.pick()
    r.installed()
    assert r.native == [("deactivate", r.own_gun, 1), ("forget", r.picker + 0x750, r.own_gun),
                        ("set_user", r.new_gun, r.picker), ("initialize", r.new_gun),
                        ("activate", r.new_gun, 1)], r.native
    assert r.read(r.picker + 0x128) == r.read(r.picker + 0x12C) == r.new_gun
    assert r.read(r.new_gun + 0x5E5, 1)[0] == 17 and r.read(r.new_gun + 0x5F1, 1)[0] == 1
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", CAPACITY)
    assert any("place 3 at 13350 ms took: the gun of place 5 kit 1 slot 10 in place of its slot 10" in l
               for l in r.log), r.log
    assert r.save(r.picker) == record(borrow=(PLACE, KIT, 1, 1))
    assert r.save(PLAYER) == record("taken")
    print("ok the pickup at set_input's apply point (carry.filter): the slots swapped by the game's set_item, the "
          "ammunition moved to a free slot (9) and rebound, the gun bound (core, registry id and entry, bullet "
          "manager, autoload off), the switch replayed (deactivate, forget, held = pending, set_user, initialize, "
          "activate), the donor's magazine and chamber with one magazine in reserve; picker borrows, drop taken")

    r.pick()
    assert any("refused, the picker already holds a borrowed gun" in l for l in r.log)
    r.installed()
    print("ok a second command: refused (it borrows already)")

    # The crate.
    _, _ = r.lohook(CAN_USE, CAN_USE_AFTER, {UC_X86_REG_EAX: r.picker})
    view = r.emu.reg_read(UC_X86_REG_ECX)
    assert view not in (r.inventory, hide.GAME[UC_X86_REG_ECX])
    assert r.slot(view, 10) == r.own_gun + 0x10 and r.slot(view, 9) == 0 and r.slot(view, 8) == r.own_ammo
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", 4))
    r.lohook(CAN_USE, CAN_USE_AFTER, {UC_X86_REG_EAX: r.picker}, stops=(CAN_USE_AFTER, CAN_USE_AVAILABLE))
    assert r.stopped == CAN_USE_AVAILABLE
    r.lohook(HINT, HINT_AFTER, {UC_X86_REG_EAX: r.picker}, stops=(HINT_AFTER, HINT_AVAILABLE))
    assert r.stopped == HINT_AVAILABLE
    r.emu.mem_write(r.inventory + 0x184, b"\0")
    r.lohook(RESUPPLY, RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.emu.reg_read(UC_X86_REG_EAX) == view and r.slot(view, 10) == r.own_gun + 0x10
    assert r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", CAPACITY) and r.read(r.inventory + 0x184, 1) == b"\1"
    r.emu.mem_write(r.new_ammo + 0x260, struct.pack("<H", 4))
    r.lohook(CLIENT_RESUPPLY, CLIENT_RESUPPLY_AFTER, {UC_X86_REG_EAX: r.picker})
    assert r.emu.reg_read(UC_X86_REG_EAX) == view and r.read(r.new_ammo + 0x260, 2) == struct.pack("<H", CAPACITY)
    r.lohook(CAN_USE, CAN_USE_AFTER, {UC_X86_REG_EAX: PLAYER})
    assert r.emu.reg_read(UC_X86_REG_ECX) == INVENTORY, "a player without a borrow: the native inventory"
    print("ok the crate: check_resupply gets the view (the own gun in slot 10, the moved slot empty); a reserve "
          "below a magazine is available (can_use, use_hint); its use refills it to a magazine for free and the "
          "native resupply gets the view (both finish_use sites)")

    # A loaded record without the borrow: restored before the native readers.
    assert r.load(r.picker, record()) == 12
    r.restored()
    assert r.read(r.picker + 0x128) == r.new_gun, "held left to the native reader"
    assert r.native == []
    # ... and with it again: installed without the switch (the reader switches).
    assert r.load(r.picker, record(borrow=(PLACE, KIT, 1, 1))) == 12
    r.installed()
    assert r.native == [("set_user", r.new_gun, r.picker), ("initialize", r.new_gun)], r.native
    print("ok a loaded record without the borrow: the install restored (held left to the reader); with it: "
          "installed again from the kept gun, without the switch's calls")

    # A lent gun outlives its window; the frame's step keeps it.
    r.step(DIED + 9_000)
    assert r.read(r.cooked + 0xD0) == 2 and r.releases == []
    r.installed()
    print("ok a borrowed gun is kept past its death's window and margin while it is held")

    # The removal: flag 0 (a snapshot's) leaves it, flag 1 (death, respawn) restores.
    r.lohook(REMOVE_SITE, REMOVE_AFTER, {UC_X86_REG_ECX: r.picker}, sp=r.frame(0))
    r.installed()
    r.lohook(REMOVE_SITE, REMOVE_AFTER, {UC_X86_REG_ECX: r.picker}, sp=r.frame(1))
    r.restored()
    assert r.read(r.picker + 0x128) == r.new_gun, "held left for remove to deactivate"
    assert r.save(r.picker) == record(), "the borrow ends with the life"
    print("ok base_player::remove: flag 0 leaves the borrow; flag 1 puts the own gun back first (its unload "
          "charges the own profile), held left for remove to deactivate; the record borrows nothing")

    # A record that does not decode: logged, read as empty, the reader past it.
    r.load(r.picker, record(borrow=(PLACE, KIT, 1, 1)))
    r.installed()
    assert r.load(r.picker, b"\x07" + bytes(11)) == 12
    r.restored()
    assert any("record of place 3 does not decode (read as empty): bad bytes" in l for l in r.log), r.log
    assert r.load(r.picker, bytes(12), size=8) == 0
    assert any("the state is too short" in l for l in r.log)
    print("ok a record that does not decode (bad bytes, a short state): logged loudly, read as empty (the borrow "
          "restored), never a crash")
    r.step(DIED + 9_100)
    assert r.read(r.cooked + 0xD0) == 1, "given back once nobody holds it, past the window"
    print("ok given back, the gun is freed at the next frame's step past its window")

    # The window: past 5 s nothing to take.
    late = Pick(patched)
    late.setup()
    late.pick(now=DIED + 5_000)
    assert any("refused, no drop to take" in l for l in late.log) and late.native == []
    print("ok a command 5 s after the death: refused")


def frame_for(run, flag):
    sp = hide.STACK + 0x9000
    run.word(sp + 4, flag)
    return sp


Pick.frame = frame_for


if __name__ == '__main__':
    main()
