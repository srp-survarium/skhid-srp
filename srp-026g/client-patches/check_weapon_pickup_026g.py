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
  3 s after base_player::remove(1) takes it out the gun is given back (0x0E0210 at its last reference), the profile copy
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

    def destroy(self):
        _, sp = self.run(DESTROY_SITE, (DESTROY_AFTER,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_ECX: PLAYER})
        assert self.stopped == DESTROY_AFTER

    def remove(self, player, at, flag=1):
        """base_player::remove(flag) of `player` at its tick time `at` (the respawn tick's 0x3169A0
        takes a dead player out of the world with 1: its body and gun leave it)."""
        self.word(player + 0x324, at)
        sp = hide.STACK + 0x9000
        self.word(sp + 4, flag)
        self.run(0x2DA790, (0x2DA796,), sp, {**hide.GAME, UC_X86_REG_ECX: player})
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
    gone = DIED + 61_000
    r.remove(PLAYER, gone)
    assert any("place 5 at %d ms: its body left the world, the drop of its kit 1 slot 10 ends" % gone in l
               for l in r.log), r.log
    r.step(gone + 2_999)
    assert r.read(COOKED + 0xD0) == 2, "freed inside the margin"
    r.step(gone + 3_000)
    assert r.read(COOKED + 0xD0) == 1 and copy not in r.live and r.releases == []
    assert r.live == {}, {hex(k): v for k, v in r.live.items()}
    print("ok the completion through the game's boost invoker 0x202300: the gun kept (one reference), the recipe freed, logged made with its ms and memory; "
          "kept while its body is in the world (60 s), given back 3 s after base_player::remove(1) takes the body out, "
          "every allocation freed once")

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
    second_pickup(patched)
    two_slots(patched)
    ordinary_kit(patched)
    abc(patched)
    late_join(patched)
    crate_per_gun(patched)
    stage2b(patched)


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
CFG_CHILD, CFG_HAS = 0xF6B10, 0xF6C00


def command(target):
    return CHORD | PATTERNS[target] << 14


def record(drop=None, slot7=None, slot10=None):
    """weapon_pickup::Record::encode: drop ('open', kit, slot bit) / 'taken'; each weapon slot's pick
    (taken ms, the recipe's place, kit, slot bit)."""
    out = bytearray(12)
    out[0] = 2
    if drop == "taken":
        out[1] = 2
    elif drop:
        _, kit, slot = drop
        out[1] = 1 | kit << 2 | slot << 4
    for at, pick in ((2, slot7), (7, slot10)):
        if pick:
            taken, place, kit, slot = pick
            out[at] = (place + 1) | kit << 5 | slot << 7
            out[at + 1:at + 5] = struct.pack("<I", taken)
    return bytes(out)


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
        self.next, self.written, self.native = PICK, bytearray(), []
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

    def weapon_asks(self, emu, address, *_):
        """The weapon's answers to the switch: it can be held, it is ready to switch."""
        self.ret(4 if address in (CAN_HOLD, VT4) else 0, 1)

    def switch(self, to):
        """The player's switch to the weapon `to` (its pending object, as a weapon key sets it) by the
        game's own tick_active_object: what is held after it."""
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

    def pick(self, target=PLACE, now=None, picker=None):
        p = picker or self.picker
        if now is not None:
            self.word(p + 0x324, now)
        self.word(p + 0x330, command(target))
        self.native = []
        self.lohook(FILTER_SITE, FILTER_AFTER, {UC_X86_REG_EDI: p, UC_X86_REG_EAX: p + 0x32C})

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
        assert self.read(gun + 4) == CORE and self.read(gun + 0xC, 1)[0] == GUN_ID
        assert self.read(gun + 0x10 + 0x248) == CORE and self.read(self.new_ammo + 0x248) == CORE
        assert self.read(gun + 0x5A0) == BULLETS and self.read(gun + 0x5F4, 1)[0] == 0
        assert self.read(self.read(gun + 0x5AC)) == 8 and self.read(gun + 0x5B0, 2) == b"\1\0"
        assert self.read(self.registry + 4 * GUN_ID) == gun

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
    assert r.read(r.registry + 4 * GUN_ID) == gun and r.read(r.picker + 0x128) == gun
    assert r.native[0] == ("deactivate", first_gun, 1) and r.native[1][::2] == ("forget", first_gun), r.native
    assert r.save(r.picker) == record(slot10=(DIED + 2_500, 6, KIT, 1))
    assert any("let go, replaced: slot 10's picked gun (taken at 13350 ms" in l for l in r.log), r.log
    # Its death: the respawn's removal puts the own gun and ammunition back.
    r.lohook(REMOVE_SITE, REMOVE_AFTER, {UC_X86_REG_ECX: r.picker}, sp=r.frame(1))
    assert r.slot(inv, 10) == own + 0x10 and r.slot(inv, 8) == r.own_ammo
    assert r.slot(second, 10) == gun + 0x10 and r.slot(second, 11) == ammo
    assert r.read(r.registry + 4 * GUN_ID) == own and r.save(r.picker) == record()
    # Let go, each is freed 3 s after (a rollback can take it back meanwhile).
    r.step(DIED + 2_500 + 2_900)
    assert r.read(second + 0xD0) == 2 and r.read(first + 0xD0) == 2
    r.step(DIED + 2_500 + 3_000)
    assert r.read(second + 0xD0) == 1 and r.read(first + 0xD0) == 1
    print("ok a pistol for a pistol (slot 10 picked twice): the picked gun let go for the new one, the own gun "
          "and ammunition hidden; the removal puts the own ones back; guns let go are freed 3 s after")


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
    r.lohook(REMOVE_SITE, REMOVE_AFTER, {UC_X86_REG_ECX: r.picker}, sp=r.frame(1))
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
    r.lohook(REMOVE_SITE, REMOVE_AFTER, {UC_X86_REG_ECX: r.picker}, sp=r.frame(1))
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
    assert r.save(c.p) == record(slot10=(DIED + 21_000, PLACE, KIT, 1)) and r.save(PLAYER) == record("taken")
    # A rollback to before C's pickup: C's record without it, A's drop open again.
    r.load(PLAYER, record(("open", KIT, 1)))
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
    """A loaded record naming a gun this client has none of (a late join, a resync): cooked anew
    from its recipe (the profile of place 6's kit 1, slot 10) in the wait, then installed."""
    r = Pick(patched)
    r.setup()
    r.roster[6] = PLAYER
    cooked, gun, ammo = r.cooked_gun()
    r.on_dispatch = cooked
    r.queries = []
    r.load(r.picker, record(slot10=(DIED + 900, 6, KIT, 1)))
    assert len(r.queries) == 1 and r.slot(r.inventory, 10) == gun + 0x10, r.log
    assert r.slot(cooked, 10) == r.own_gun + 0x10
    assert any("a loaded record: installed: slot 10's picked gun (taken at 13250 ms, the gun of place 6 kit 1 "
               "slot 10)" in l for l in r.log), r.log
    print("ok a late join's record naming a gun not on this client: cooked from its recipe in the wait (the "
          "game's dispatch), installed without the switch")


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


def stage2(patched, original):
    no_branch_into(original)
    print("ok nothing in the original branches into the new spans (0x2DAF70, 0x2DB4CF, 0x2DA790, 0x2F809C, "
          "0x2F80E4, 0x2F81B8, 0x2954EF)")

    r = Pick(patched)
    r.setup()
    assert r.save(PLAYER) == record(("open", KIT, 1)), r.written.hex()
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
    assert any("place 3 at 13350 ms took: slot 10's picked gun (taken at 13350 ms, the gun of place 5 kit 1 slot 10)" in l
               for l in r.log), r.log
    took = [l.split("pickup: ", 1)[1] for l in r.log if "pickup: place 3 pickup: " in l]
    assert took == ["place 3 pickup: the donor's magazine 17, chamber 1",
                    f"place 3 pickup: slot 8 ammo 2204 x {CAPACITY} (cooked slot 11); own ammo 217 x 90 parked"], took
    assert r.save(r.picker) == record(slot10=PICKED)
    assert r.save(PLAYER) == record("taken")
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
    assert r.load(r.picker, record()) == 12
    r.restored()
    assert r.read(r.picker + 0x128) == r.new_gun, "held left to the native reader"
    assert r.native == []
    # ... and with it again: installed without the switch (the reader switches).
    assert r.load(r.picker, record(slot10=PICKED)) == 12
    r.installed()
    assert r.native == [("set_user", r.new_gun, r.picker), ("initialize", r.new_gun)], r.native
    print("ok a loaded record without the pick: the install restored (held left to the reader), the gun back "
          "in its drop; with it: installed again from it, without the switch's calls")

    # The donor's body leaves the world (its respawn: base_player::remove(1)) while its gun is lent:
    # the drop ends, the lent gun stays.
    gone = DIED + 8_000
    r.remove(PLAYER, gone)
    assert r.save(PLAYER) == record(), "the drop ends with the body"
    r.step(gone + 3_500)
    assert r.read(r.cooked + 0xD0) == 2 and r.releases == []
    r.installed()
    print("ok the donor's body leaves the world (base_player::remove(1)): its record's drop ends; a borrowed gun "
          "is kept past it while it is held")

    # The removal: flag 0 (a snapshot's) leaves it, flag 1 (respawn, departure) restores.
    r.lohook(REMOVE_SITE, REMOVE_AFTER, {UC_X86_REG_ECX: r.picker}, sp=r.frame(0))
    r.installed()
    r.lohook(REMOVE_SITE, REMOVE_AFTER, {UC_X86_REG_ECX: r.picker}, sp=r.frame(1))
    r.restored()
    assert r.read(r.picker + 0x128) == r.new_gun, "held left for remove to deactivate"
    assert r.save(r.picker) == record(), "the borrow ends with the life"
    print("ok base_player::remove: flag 0 leaves the borrow; flag 1 puts the own gun and its ammunition back first "
          "(its unload charges the own profile), held left for remove to deactivate; the record borrows nothing")

    # A record that does not decode: logged, read as empty, the reader past it. The pick taken back
    # first from what the removal let go (10 s after the donor's body left the world).
    r.word(r.picker + 0x324, gone + 10_000)
    r.load(r.picker, record(slot10=PICKED))
    r.installed()
    assert r.load(r.picker, b"\x07" + bytes(11)) == 12
    r.restored()
    assert any("record of place 3 does not decode (read as empty): bad bytes" in l for l in r.log), r.log
    assert r.load(r.picker, bytes(12), size=8) == 0
    assert any("the state is too short" in l for l in r.log)
    print("ok a record that does not decode (bad bytes, a short state): logged loudly, read as empty (the pick "
          "restored), never a crash")
    r.step(gone + 10_100)
    assert r.read(r.cooked + 0xD0) == 1, "back in the drop of a body gone 10 s ago: freed at the next frame"
    print("ok a gun back in the drop of a body that left the world is freed at the next frame's step after the margin")

    # Availability ends when the body leaves the world: a command after the donor's removal.
    late = Pick(patched)
    late.setup()
    late.pick(now=DIED + 60_000)
    late.installed()
    early = Pick(patched)
    early.setup()
    early.remove(PLAYER, DIED + 8_000)
    early.pick(now=DIED + 8_000)
    assert any("refused, no drop to take" in l for l in early.log) and early.native == []
    assert early.slot(early.inventory, 10) == early.own_gun + 0x10
    print("ok a command 60 s after the death with the body still there: taken; after base_player::remove(1) of "
          "the body (the same tick on every client): refused")


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
            ("15 cm beside the gun", lambda g: g.eye([0., 0., 1.], [0.15, 1.0, 0.]), "in reach, not aimed at its box", []),
            ("at the body 40 cm below it", lambda g: g.eye([0., -0.371, 0.928], [0., 1.0, 0.]),
             "in reach, not aimed at its box", []),
            ("beyond 2.2 m", lambda g: g.eye([0., 0., 1.], [0., 1.0, -1.9]), "out of reach (2.2 m)", []),
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
    assert g.hud() == [("",)] and g.hud() == [] and g.build(USE) == USE, "its body out of the world"
    assert sights(g) == ["offered"], ("its pose forgotten with the body: nothing aimed at", sights(g))
    g = Prompt(patched)
    g.setup()
    g.word(PLAYER + 0x324, DIED + 50)
    g.death()
    assert g.queries == [], "the same life's death again: no second cook"
    assert g.draw() and g.hud() == [("st_weapon_pick_up", NAME)], sights(g)
    print("ok a rollback replays the death at another time: its gun (cooked at the first) is still offered")
    g = Prompt(patched)
    g.setup()
    g.eye([0., -1., 0.], [0.09, 2.0, 1.2])
    g.draw()
    assert g.hud() == [("st_weapon_pick_up", NAME)], "5 cm of padding"
    print("ok the box is the gun's own (10 cm thick, 1 m long, at its transform; logged once a drop): no prompt 15 cm "
          "beside it or at the body below it; no prompt (and Use the game's) looking away, beyond 2.2 m, behind a wall, a second "
          "double-handed gun, modifier 64 in slot 10, or a place the match does not have; the box padded by 5 cm; offered 60 s after the death while the body is there, "
          "gone with base_player::remove(1) of the body (its pose forgotten); each drop's reason logged once a change (not aimed at, "
          "out of reach, no line of sight, the carry rule with both slots' handedness and the gun's, taken)")


def sights(run):
    """The sight lines of the gun of place 5 the picker logged."""
    head = "pickup: place 3 sees the gun of place 5: "
    return [l.split(head, 1)[1] for l in run.log if head in l]


def frame_for(run, flag):
    sp = hide.STACK + 0x9000
    run.word(sp + 4, flag)
    return sp


Pick.frame = frame_for


if __name__ == '__main__':
    main()
