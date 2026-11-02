#!/usr/bin/env python3
"""A resync always converges: a game that loads a world and saves it at once
writes the bytes it loaded, whatever it held before (../docs/notes/client_patch.md,
"World round trip").

    python check_world_roundtrip_026g.py <original survarium.exe> [WORLD or FOLDER ...]
        [--exe PATCHED] [--show N]

Requires Python `unicorn`. A late join and a resync load the majority's world
(0xE8, process_dynamic_match_info 0x27BFF0); the game then saves its own world
for the next check. A patch that writes saved bytes from this game's own
objects instead of from what it loaded makes this game's next world differ
from the others': it resyncs again until the server drops it (rev41's weapon
pickup: its record's 28 bytes of the own kit a pick set aside, written from
each game's own objects; ../docs/notes/weapon_pickup.md).

What runs, the patched exe's own code under Unicorn: every patch code inside
the world's load and save, at its own site, in the order the game reaches it,
over the bytes it owns in a recorded world:

- the 0xE8 handler's preparation over the whole world (its time, then the
  body): `world_load.unpacked` (0x27C0D0: patch 87 parses it, patch 85 cooks
  the guns its records name that this game has no copy of);
- in each place's state, as base_player::deserialize reaches them:
  `pickup.load` (0x2DB4CF) at offset 2, patch 85's record, and `carry.load`
  (0x2DBAB5) at its last 32 bytes, patch 78's carry; then, as
  base_player::serialize does, each place's `pickup.save` (0x2DAF70) and
  `carry.save` (0x2DB47E), with the game's byte writer (0x1F3DE0).

Each hook must read as many bytes as it writes and write the bytes it read;
and no patch may log that what it loaded is not what this game has (a pick
NOT installed or registered, a gun NOT made before the load, a record that
does not decode, FAILED): the game's own fields after it would differ.

The native codec around them (game_world_core::deserialize 0x2D0B70 and
0x2D2770, serialize 0x2D2570; base_player 0x2DB490, 0x2DAF60) is not run: it
needs a live game's objects (the animation player, physics, the inventory's
items cooked from their configs), which no made-up world gives. Its own fields
are the game's (resyncs converged before the patches); a patch's effect on
them shows as one of the log lines above.

Three games load each world: two fresh ones (a late joiner: nothing seen;
every place's three kit players, each with its own guns in slots 7 and 10,
the core's place table and registry), which differ in their own kits (30
rounds in each own gun, 90 in its stack, condition 1.0; 7, 13 and 0.5), and one that loaded
every world before it (a resync over its own state, the worlds in the order of
their times). A byte one of them writes from its own objects differs from the
loaded one in at least one of them.

The coverage: every byte the patched build changes in a function of the
world's codec (a re-kit name with ::serialize or ::deserialize, the bullet
loader 0x32BBC0, the 0xE8 handler) is in a site this check runs (RUN), or in
OWN_CHECKS with the check that runs its round trip and why it is not run here.
A new site there fails this check until it is one or the other.

The worlds: worlds_026g/ (default: a few of rev41's match 0x1c0ebd31, each
kind of record and carry in it), or WORLD files and folders of the match
server's dumps (SRP_SNAPSHOT_DIR: match026g_<id>_<tick>_..bin, the body; the
tick is the world's time); the whole match's 501:
private/live-worlds-1c0ebd31/ in the owner's checkout. A world recorded with
another version of patch 85's record than the exe writes stops the check, but
for rev41's version 4 (the corpus): `from_v4` reads it as version 8 writes it.
The record is otherwise opaque: its version and size are what the exe saves.
--exe runs a built exe instead of this tree's build (rev41's: the check then
finds what it wrote from its own objects).
"""
import argparse
import bisect
import itertools
import os
import re
import struct
import sys
import time

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBP, UC_X86_REG_ESP, \
    UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_weapon_pickup_026g as wp

HERE = os.path.dirname(os.path.abspath(__file__))
WORLDS = os.path.join(HERE, "worlds_026g")
REKIT = os.path.join(HERE, "..", "..", "tools", "re-kit", "v026g")

CARRY_SAVE, CARRY_SAVE_AFTER = 0x2DB47E, 0x2DB483
CARRY_LOAD, CARRY_LOAD_AFTER = 0x2DBAB5, 0x2DBABC
CARRY = 32                                  # artefact_carry::Carry::SNAPSHOT_SIZE, the state's last bytes
PLACES = 20
EXTRA = wp.PICK + 0x100000                  # room for every place's kit players after the Pick's own
# Each own gun's magazine, its stack's rounds, and its condition (weapon+270h, the wear a pick hides).
OWN_KITS = {"fresh A": (30, 90, 1.0), "fresh B": (7, 13, 0.5)}

# The sites this check runs (VA, span): the 0xE8 handler's preparation and the player state's hooks.
RUN = {
    (0x27C0D0, 9): "world_load.unpacked (patch 87's parse, patch 85's guns cooked before the load)",
    (0x2DB4CF, 6): "pickup.load (patch 85's record, state offset 2)",
    (0x2DBAB5, 7): "carry.load (patch 78's carry, the state's last 32 bytes)",
    (0x2DAF70, 6): "pickup.save",
    (0x2DB47E, 5): "carry.save",
}
# The world codec's other patch sites: (VA, span) -> why it is not run here, and the check that runs its round
# trip. Not run here = the bytes it owns are in a section no decoder of this repository locates (bullets,
# objects, the statistics' batteries), or it owns no bytes.
OWN_CHECKS = {
    (0x27C0FF, 6): "world_load.raw: the same preparation as world_load.unpacked (world_check.rs `check`), for a "
                   "world sent raw; check_world_check_026g.py",
    (0x2D1050, 10): "first_load input and kit (late_join_026g.asm): the place's input and kit choice into the core's "
                    "lists at a first load, no bytes of the world; check_late_join_026g.py",
    (0x2F5ACF, 6): "shared_statistics::serialize: twenty records, a constant, as its deserialize; selftest_026g.py",
    (0x2F5AF4, 6): "shared_statistics::serialize: twenty records (its second loop)",
    (0x2F5BE6, 6): "shared_statistics::deserialize: twenty records, as its serialize",
    (0x2F5C03, 6): "shared_statistics::deserialize: twenty records (its second loop)",
    (0x313735, 6): "carry.ground_save: a dropped artefact's 20 bytes in the objects section (no decoder locates "
                   "them); check_artefact_carry_026g.py (state 6's 20 bytes saved, then loaded)",
    (0x3137EC, 5): "carry.ground_load: as carry.ground_save",
    (0x321C3B, 5): "bullet_history.saved: patch 75's lamp words after the bullets' history (the bullets section, "
                   "no decoder); check_lamps_026g.py (a rollback's and a late joiner's load: the lamps the words say)",
    (0x321D01, 7): "bullet_history.loaded: as bullet_history.saved",
    (0x32C148, 7): "bullet.save_quiet: the quiet byte always written (the bullets section, no decoder); "
                   "check_weapon_pickup_026g.py `bullet_record` (written and read in step)",
    (0x32BE12, 7): "bullet.load_quiet: the quiet byte always read, as bullet.save_quiet",
    (0x32BDBA, 5): "bullet.load_ordinal: an ordinal past the loading gun's ammo is none (the same in a world whose "
                   "guns match); check_weapon_pickup_026g.py `bullet_record`",
    (0x334F66, 10): "battery.save: patch 92's life after each battery's record (the statistics, modes 2 and 3; "
                    "none in the recorded match); check_battery_progress_026g.py (a load gives every life back)",
    (0x3350D0, 6): "battery.load: as battery.save",
}
CODEC = re.compile(r"::(de)?serialize")
CODEC_ALSO = {0x32BBC0: "survarium::bullet::bullet (the bullet loader)",
              0x27BFF0: "survarium::network_client::process_dynamic_match_info (0xE8)"}

# Patch 85's record by version: its size; and how a recorded state's record becomes the next version's.
# The record is opaque bytes here: its version and size are what the exe's own save writes (`record_version`), and
# each hook must read as many bytes as it writes. The one layout this check knows is the recorded corpus's: rev41's
# version 4 (48 bytes: the drop, the picks and the pending pickup in bytes 1..20, then the 28 of the own kit set
# aside), read as version 8 (29 bytes) writes it: those 20 bytes (the drop's byte and the pending pickup's slot byte
# as `from_v4` says), the own gun's wear each pick hides (slot 7's, slot 10's), the drop's recipe. Rev41's record had none, and after the swap the world no longer carries the own
# gun (it is hidden in the cooked inventory, which the world does not save), so a recorded world cannot give it:
# V4_WEAR, a condition no own gun of the check's games has (OWN_KITS), so a save taking it from the game's own gun
# differs. A record of another version, or an exe writing another than 4 or 8, stops the check here.
V4, V4_BYTES = 4, 48
V4_WEAR = struct.pack("<f", 0.75)


def from_v4(record, place, version, size):
    """A recorded version 4 record of `place` as the exe's (version, size) has it. Version 8 (29 bytes) moves the
    drop's recipe to its own byte (28): rev41's open drop was always its body's own gun (place, kit, slot, held in
    that slot, on its body); a taken drop kept nothing of it there, so it reads as none (a cancel then reopens
    nothing, in every game alike). A pending pickup names the place it was taken from: rev41's was its recipe's."""
    if (version, size) != (8, 29):
        sys.exit(f"the exe writes record version {version} ({size} bytes): from_v4 knows version 8 (29 bytes) only")
    drop, recipe = 0, 0
    if record[1] & 3 == 1:
        kit, slot = record[1] >> 2 & 3, record[1] >> 4 & 1
        drop, recipe = 1 | slot << 2, (place + 1) | kit << 5 | slot << 7
    pending = record[12] | ((record[13] & 0x1F) - 1) << 2 if record[12] else 0
    wears = b"".join(V4_WEAR if record[at] else bytes(4) for at in (2, 7))
    return bytes([8, drop]) + record[2:12] + bytes([pending]) + record[13:20] + wears + bytes([recipe])


NOT_MATCHING = re.compile(r"NOT (installed|registered|made)|does not decode|FAILED")


# ---- the worlds ---------------------------------------------------------------------------------------------

def animation_len(data):
    """players::animation_len (crates/survarium): the animation state's bytes at the start of `data`."""
    at = 0

    def read(bits):
        nonlocal at
        if at + bits > 8 * len(data):
            raise ValueError("the animation state ends early")
        value = 0
        for _ in range(bits):
            value = value << 1 | (data[at // 8] >> (7 - at % 8) & 1)
            at += 1
        return value

    def skip(bits):
        nonlocal at
        at += bits
        if at > 8 * len(data):
            raise ValueError("the animation state ends early")

    width = lambda count: max(count.bit_length(), 1)
    skip(read(16))
    count = read(10)
    if count:
        indexed = read(1)
        skip(32)
        if count > 1:
            each = read(5)
            if indexed:
                unique = read(width(count))
                skip(max(unique - 1, 0) * each + count * width(unique))
            else:
                skip(count * each)
    indexed, count = read(1), read(10)
    if indexed:
        unique = read(width(count))
        skip(unique * 32 + count * width(unique))
    else:
        skip(count * 32)
    return (at + 7) // 8


def walk(body):
    """players::blocks: [(place, state start, state end)] of each player block; six length-prefixed sections
    must follow them to the body's end."""
    mask, = struct.unpack_from("<I", body)
    at, states = 4, []
    for place in range(32):
        if mask >> place & 1:
            size, = struct.unpack_from("<H", body, at)
            states.append((place, at + 2, at + 2 + size))
            at += 2 + size
            animated = body[at]
            at += 1
            if animated:
                at += animation_len(body[at:])
    for _ in range(6):
        size, = struct.unpack_from("<H", body, at)
        at += 2 + size
    if at != len(body):
        raise ValueError(f"the body has {len(body) - at} bytes after its sections")
    return states


def migrate(body, version, size):
    """`body` with each state's record the exe's (version, size): one recorded at version 4 converted
    (`from_v4`), each state's length with it."""
    out, at = bytearray(body[:4]), 4
    for place, start, end in walk(body):
        state = bytes(body[start:end])
        if state[2] == V4 != version:
            state = state[:2] + from_v4(state[2:2 + V4_BYTES], place, version, size) + state[2 + V4_BYTES:]
        elif state[2] != version:
            sys.exit(f"a world's record of version {state[2]}: the exe writes {version}, from_v4 converts 4 only")
        out += body[at:start - 2] + struct.pack("<H", len(state)) + state
        at = end
    return bytes(out + body[at:])


def worlds(paths):
    """[(name, time, body)] of the dumps in `paths` (files and folders), in the order of their times."""
    files = []
    for path in paths:
        if os.path.isdir(path):
            files += [os.path.join(path, name) for name in os.listdir(path) if name.endswith(".bin")]
        else:
            files.append(path)
    found = []
    for path in files:
        name = os.path.basename(path)
        tick = name.split("_")
        if len(tick) < 3 or not tick[2].isdigit():
            sys.exit(f"{path}: not a dump's name (match026g_<id>_<tick>_...)")
        found.append((int(tick[2]), name, open(path, "rb").read()))
    return [(name, tick, body) for tick, name, body in sorted(found)]


# ---- a game -------------------------------------------------------------------------------------------------

class Game(wp.Pick):
    """A game of the recorded match: check_weapon_pickup's made-up match (profile, game world, game, client,
    core, the byte writer and the weapon calls recorded) with every place's three kit players (place table
    core+18458h), each holding its own guns in slots 7 and 10 (registered by id, their stacks in slots 12 and 8),
    and a cook that makes any recipe it is asked for (guns in slots 7 and 10)."""

    def __init__(self, data, own_kit):
        super().__init__(data)
        self.emu.mem_map(EXTRA, 0x4000000, UC_PROT_ALL)
        self.emu.hook_add(UC_HOOK_CODE, self.active_player, begin=wp.GET_ACTIVE_PLAYER, end=wp.GET_ACTIVE_PLAYER)
        self.own_kit, self.completed, self.now = own_kit, set(), 0
        self.match()
        self.registry = self.blk(0x400)
        self.word(wp.CORE + 0x18B4C, self.registry)
        table = self.blk(0x30 * PLACES)
        self.word(wp.CORE + 0x18458, table)
        self.word(wp.CORE + 0x1845C, table + 0x30 * PLACES)
        ids = itertools.count()
        self.kits = {(place, kit): self.kit_player(place, kit, ids) for place in range(PLACES) for kit in range(3)}
        for (place, kit), player in self.kits.items():
            self.word(table + 0x30 * place + 4 * kit, player)
        self.word(wp.CORE + 0x18B50, self.registry + 4 * next(ids))
        self.word(wp.CLIENT + 4, self.kits[(0, 0)])
        self.roster = {}

    def active_player(self, emu, address, *_):
        """network_client::get_active_player (thiscall: out, place; ret 8): the place's player in the world."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        out, place = self.read(sp + 4), self.read(sp + 8)
        player = self.roster.get(place, 0)
        self.word(out, player)
        if player:
            self.word(player + 0xD0, self.read(player + 0xD0) + 1)
        self.ret(8, out)

    def dispatch(self, emu, address, *_):
        """The game's dispatch_callbacks in a wait: every cook asked so far completes, each through the boost
        invoker with its own functor and queries_result, as the game's dispatch runs them."""
        waiting = [n for n in range(len(self.queries)) if n not in self.completed]
        if not waiting:
            return self.ret(0)
        at = code_at = self.blk(24 * len(waiting) + 1)
        code = b""
        for n in waiting:
            self.completed.add(n)
            cooked, functor, result = self.cook(), self.blk(0x40), self.blk(0x140)
            self.emu.mem_write(functor, self.queries[n]["callback"])
            self.word(result + 0x38, 1)
            self.word(result + 0x12C, cooked)
            self.word(cooked + 0xD0, 1)
            # push result; push functor; call the invoker; add esp,8
            call = b"\x68" + struct.pack("<I", result) + b"\x68" + struct.pack("<I", functor + 8)
            code += call + b"\xE8" + struct.pack("<i", wp.INVOKER - (at + len(call) + 5)) + b"\x83\xC4\x08"
            at = code_at + len(code)
        emu.mem_write(code_at, code + b"\xC3")
        emu.reg_write(UC_X86_REG_EIP, code_at)

    def cook(self):
        """A cooked inventory: a gun in slot 7 (its ammunition in 12) and one in slot 10 (in 11)."""
        cooked = self.blk(0x200)
        for slot, ammo_slot in ((7, 12), (10, 11)):
            gun, ammo = self.gun(ammo_slot=ammo_slot), self.stack(240)
            self.ammo_config(ammo, 2204, 100, 2)
            for item, at in ((gun + 0x10, slot), (ammo, ammo_slot)):
                self.word(cooked + 0x10C + 4 * at, item)
                self.word(item + 0x250, cooked)
                self.word(item + 0x254, at)
        return cooked

    def kit_player(self, place, kit, ids):
        """A place's player of `kit`: its own guns in slots 7 (stack in 12) and 10 (stack in 8), slot 10's held;
        their magazines, stacks and conditions the game's own kit's."""
        magazine, rounds, condition = self.own_kit
        player, inventory = self.blk(0x10000), self.blk(0x200)
        for slot, ammo_slot in ((7, 12), (10, 8)):
            gun, ammo = self.gun(ammo_slot=ammo_slot, loaded=magazine), self.stack(rounds)
            self.ammo_config(ammo, 217, 30, 1)
            gun_id = next(ids)
            self.emu.mem_write(gun + 0xC, bytes([gun_id]))
            self.word(gun + 0x4, wp.CORE)
            self.word(gun + 0x5A0, wp.BULLETS)
            self.word(self.registry + 4 * gun_id, gun)
            for item, at in ((gun + 0x10, slot), (ammo, ammo_slot)):
                self.word(inventory + 0x10C + 4 * at, item)
                self.word(item + 0x250, inventory)
                self.word(item + 0x254, at)
            self.word(gun + 0x538, ammo)
            self.emu.mem_write(gun + 0x270, struct.pack("<f", condition))
        self.word(player + 0xD0, 3)
        self.emu.mem_write(player + 0x114, bytes([place, kit]))
        self.word(player + 0x10C, inventory)
        self.word(player + 0x120, wp.CORE)
        self.word(player + 0x128, gun)
        self.word(player + 0x12C, gun)
        self.word(player + 0x328, 1000)
        self.word(player + 0x6B8, wp.PROFILE)
        self.word(player + 0xF474, wp.WORLD)
        self.emu.mem_write(player + 0x33C, bytes([1, 0, 0, 1]))
        return player

    def reader(self, begin, cursor, size):
        reader = self.blk(12)
        for at, value in ((0, begin), (4, cursor), (8, size)):
            self.word(reader + at, value)
        return reader

    def load(self, now, body):
        """The 0xE8 handler's load of the world `body` at `now`: its preparation, then each place's state's
        hooks. {place: (player, the record's bytes read, the carry's bytes read)}."""
        self.now = now
        self.time(now)
        states = walk(body)
        self.roster = {place: self.kits[(place, body[start])] for place, start, _ in states}
        for player in self.kits.values():
            self.word(player + 0x324, now)
        wp.load_world(self, struct.pack("<I", now) + body)
        at = self.blk(len(body))
        self.emu.mem_write(at, body)
        read = {}
        for place, start, end in states:
            player = self.roster[place]
            reader = self.reader(at + start, at + start + 2, end - start)
            self.lohook(wp.LOAD_SITE, wp.LOAD_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_ESI: reader})
            record = body[start + 2:start + 2 + self.read(reader + 4) - (at + start + 2)]
            reader = self.reader(at + start, at + end - CARRY, end - start)
            self.lohook(CARRY_LOAD, CARRY_LOAD_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_ESI: reader})
            carry = body[end - CARRY:self.read(reader + 4) - at]
            read[place] = (player, record, carry)
        return read

    def save(self, player):
        """base_player::serialize's hooks: (the record's bytes, the carry's)."""
        self.written = bytearray()
        self.lohook(wp.SAVE_SITE, wp.SAVE_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_ESI: 0x5A5A0000})
        record, self.written = bytes(self.written), bytearray()
        self.lohook(CARRY_SAVE, CARRY_SAVE_AFTER, {UC_X86_REG_EBX: player, UC_X86_REG_EDI: 0x5A5A0000,
                                                   UC_X86_REG_EBP: hide.STACK})
        return record, bytes(self.written)


def record_version(data):
    """The version and size of the record the exe writes: a fresh game's save of a player it never loaded."""
    game = Game(data, OWN_KITS["fresh A"])
    record, _ = game.save(game.kits[(0, 0)])
    return record[0], len(record)


def round_trip(game, name, now, body):
    """Failures of `game` loading `body` and saving it at once: [(what, the bytes that differ, the world, the
    line)]."""
    game.log = []
    read = game.load(now, body)
    failures = []
    for place, (player, record, carry) in read.items():
        saved = game.save(player)
        for what, loaded, written in (("record", record, saved[0]), ("carry", carry, saved[1])):
            where = f"{name} place {place} {what}"
            if len(loaded) != len(written):
                failures.append((f"{what} size", (), name, f"{where}: read {len(loaded)} bytes, wrote {len(written)}"))
            elif loaded != written:
                at = tuple(n for n in range(len(loaded)) if loaded[n] != written[n])
                failures.append((what, at, name, f"{where}: bytes {at[0]}..{at[-1]} differ: loaded {loaded.hex()}, "
                                                 f"saved {written.hex()}"))
    failures += [("log", (), name, f"{name}: {line}") for line in game.log if NOT_MATCHING.search(line)]
    return failures


def summary(failures, worlds):
    """One line: how many worlds failed, and for each kind of failure its count and the bytes that differ."""
    kinds = {}
    for what, at, _, _ in failures:
        count, offsets = kinds.get(what, (0, set()))
        kinds[what] = (count + 1, offsets | set(at))
    parts = [f"{what} {count}" + (f" (bytes {min(offsets)}..{max(offsets)})" if offsets else "")
             for what, (count, offsets) in sorted(kinds.items())]
    failed = len({name for _, _, name, _ in failures})
    return f"{failed} of {worlds} worlds differ: " + ", ".join(parts)


# ---- the coverage -------------------------------------------------------------------------------------------

def codec_sites(original, patched):
    """[(VA, size, function)] of the bytes the patched build changes in a function of the world's codec."""
    starts = []
    for table in ("functions.tsv", "inferred.tsv"):
        for line in open(os.path.join(REKIT, table)):
            cols = line.rstrip("\n").split("\t")
            if cols[0].startswith("0x") and len(cols) > 3:
                starts.append((int(cols[0], 16), cols[3]))
    starts.sort()
    at = [va for va, _ in starts]
    out = []
    for _, rva, vsize, raw, rsize, characteristics in base.Headers(original).sections(original):
        if not characteristics & 0x20000000:          # code
            continue
        n, end = 0, min(vsize, rsize)
        while n < end:
            if original[raw + n] == patched[raw + n]:
                n += 1
                continue
            first = n
            while n < end and original[raw + n] != patched[raw + n]:
                n += 1
            va = base.IMAGE_BASE + rva + first
            start, function = starts[bisect.bisect_right(at, va) - 1]
            if CODEC.search(function) or start in CODEC_ALSO:
                out.append((va, n - first, function))
    return out


def coverage(original, patched):
    """Failures: a changed byte of the codec in no site of RUN or OWN_CHECKS."""
    known = {**RUN, **OWN_CHECKS}
    failures = []
    for va, size, function in codec_sites(original, patched):
        if not any(at <= va and va + size <= at + span for at, span in known):
            failures.append(f"{va:#x} ({size} bytes, {function}): a patch site in the world's codec this check "
                            "neither runs (RUN) nor names with its own check (OWN_CHECKS)")
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("exe", help="the original survarium.exe")
    parser.add_argument("worlds", nargs="*", default=[WORLDS], help="dumps, or folders of them")
    parser.add_argument("--exe", dest="built", help="a built exe to run instead of this tree's build")
    parser.add_argument("--show", type=int, default=10, help="failures shown per game (default 10)")
    args = parser.parse_args()
    started = time.time()
    original = open(args.exe, "rb").read()
    if args.built:
        patched = open(args.built, "rb").read()
    else:
        patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                                open(base.late_join_bin(), "rb").read())
        patched = bytes(patched)
    failed = 0

    missing = coverage(original, patched)
    for line in missing:
        print(f"  FAIL  coverage: {line}")
    print(f"  {'FAIL' if missing else 'ok'}  coverage: every patch site in the world's codec is run here "
          f"({len(RUN)}) or named with its own check ({len(OWN_CHECKS)})")
    failed += len(missing)

    version, size = record_version(patched)
    recorded = worlds(args.worlds)
    loaded = [(name, now, migrate(body, version, size)) for name, now, body in recorded]
    games = {what: (lambda kit=kit: Game(patched, kit)) for what, kit in OWN_KITS.items()}
    resync = Game(patched, OWN_KITS["fresh A"])
    for what in [*games, "resync"]:
        failures = []
        for name, now, body in loaded:
            game = resync if what == "resync" else games[what]()
            failures += round_trip(game, name, now, body)
        for _, _, _, line in failures[:args.show]:
            print(f"  FAIL  {what}: {line}")
        print(f"  {'FAIL' if failures else 'ok'}  {what}: {len(loaded)} worlds loaded and saved at once "
              f"(record version {version}, {size} bytes; carry {CARRY}): "
              f"{summary(failures, len(loaded)) if failures else 'every hook wrote what it read'}")
        failed += len(failures)
    print(f"{'all cases passed' if not failed else f'{failed} FAILED'} ({time.time() - started:.0f} s)")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
