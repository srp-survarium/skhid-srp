#!/usr/bin/env python3
"""Run the original and patch 31's respawn rule tick (0x316820), the
client's view of a kit change (0x2678E0), the hud's update of the local
player's state (0x23B7B3), the kits' pick (0x233F28), the input world's
mouse moves through the game world's input handler (0x3B9F0A, 0x25D0E0),
the mouse's buttons through it (0x3B9D90, 0x25CF60) and the warmup mode's
step (0x242200) on made-up objects.

    python check_kit_change_026g.py <original survarium.exe>

Requires Python `unicorn`. The places' tick (0x321790, which takes a
place's kit choice from its list), the respawn rule tick's start (up to
0x316826), the game's own spawn (0x316DA0, with patches 18 and 20 in the
build) and the view of a kit change run as machine code; the player's calls
(out of the world, initialize, into the world, its damage model), the
network client's, the cameras, the statistics' kit event and the hud's
calls are stubs. The rules: ../docs/notes/kit-change-before-start.md.

The build must respawn a place whose next kit is another, where it stands
and facing as it faces, with what was left of its invulnerability, while the
match waits or counts down, and only then: in the last tick before play, in
the tick whose time rule begins play (the places and the respawn rule tick
before the time rule, check 32 of selftest_026g.py), never after; whatever
tick the choice came to a game in, by the time the choice is for; never a
dead player, one out of the world, one with any statistics, a late joiner,
nor a place past the end of the core's places (the buffer holds 20, a match
of fewer places leaves the rest as memory had it). The new kit's damage
model is given its protection at every switch, -1 when none is left, never
left with what an earlier life gave it.
The view keeps the warmup's camera and input before play and gives the
player's in play, by the state the network client set the camera for. The
hud shows the kits to a player who becomes alive before play, keeps them
shown (and its cursor) after a pick before play, and hides them from a live
one in play. While the kits are shown before play (the player's input
handler in the warmup mode), mouse moves through the input world's loop in
order: a move without a button is the cursor's; a left press that moves
little is the cursor's (a click); past that, or with the lobby's buttons
at once, a drag takes the hud's pointer off the screen and gives the moves
(and the wheel) to the player's input handler, which turns the warmup
camera; the first move after it puts the cursor back where the drag began;
the wheel alone goes on to the player's handler without the frame's turn.
With the kits shown in any other mode (the death screen), the cursor shown
without them, or no player's handler, the cursor takes every move, as in
the original (these runs bind no local player: a dead one's death screen
in the first person mode is patch 46's, check_death_camera_026g.py). The warmup mode keeps the pitch within +-60 degrees and the
distance within 1.5 and 4.6, the lobby camera's. The original does none of
it: it never changes a kit of a live player, its view always gives the
player camera, its hud only hides the kits at a respawn and shows them at a
death, its cursor takes every move while it is shown, and its warmup camera
has no limits. Says nothing about what the real initialize, physics,
cameras and hud movie do.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_FPSW

import patch_026g as base

TICK, TICK_ON = base.KIT_CHANGE_TICK_VA, base.KIT_CHANGE_TICK_VA + 6
SPAWN, PLACE_TICK, KIT_VIEW = 0x316DA0, 0x321790, 0x2678E0
PLAYER_CAMERA, WARMUP_CAMERA = 0x1F8200, 0x1F8190
HEAP = 0x20000000
RULE, RVT, CORE, PLACES, SVT, PVT, DMI = (HEAP + i * 0x1000 for i in range(7))
STATS = HEAP + 0x30000                             # 20 records of 1F8h
CHOICES = HEAP + 0x38000                           # kit-choice list nodes, 40h each
LISTS = HEAP + 0x7C000                             # the places' kit-choice lists, 480h each
CORE_SIZE = 0x1A000                                # the core's fields up to +18FE4h
PLAYERS = HEAP + 0x40000                           # 20 places x 3 kits, 0x1000 each
STACK, STOP, STUB = 0x2F008000, 0x2F100000, 0x2F100100
CLIENTS = 0x30000000
INIT, PUT, REMOVE, VIEW, STAT_EVENT, DAMAGE = (STUB + 0x10 * i for i in range(6))
NOW = 21_750
START = 20_400                                     # the tick whose time rule begins play
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI)


def player_at(place, kit):
    return PLAYERS + (place * 3 + kit) * 0x1000


class Emu:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x40000 + 60 * 0x1000 + CORE_SIZE, UC_PROT_ALL)
        emu.mem_map(0x2F000000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x10000, UC_PROT_ALL)
        emu.mem_map(CLIENTS, 0x400000, UC_PROT_ALL)    # network clients: +1F1C60h
        self.calls = []
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def byte(self, at, value=None):
        if value is None:
            return self.emu.mem_read(at, 1)[0]
        self.emu.mem_write(at, bytes([value]))

    def returns(self, value, pop):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        pass


class Tick(Emu):
    """A rule, a core with 20 places of three kits, statistics."""

    def __init__(self, data):
        super().__init__(data)
        core = CORE + 0x10000                      # leaves room below for the core's fields
        self.core = core
        self.nodes = 0
        for at, value in ((RULE, RVT), (RULE + 0x134, core), (RVT + 0x5C, SPAWN), (RVT + 0x60, VIEW),
                          (core + 0x18458, PLACES), (core + 0x1845C, PLACES + 20 * 0x30),
                          (core + 0x18FA8, STATS), (STATS, SVT),
                          (SVT + 0x2C, STAT_EVENT), (PVT + 0x1C, INIT), (PVT + 0x20, PUT),
                          (PVT + 0x24, REMOVE), (DMI + 8, DAMAGE), (core + 0x89D8, LISTS)):
            self.word(at, value)
        self.state(2)
        for place in range(20):
            self.word(RULE + 0x138 + place * 4, -1)
            entry = PLACES + place * 0x30
            for kit in range(3):
                p = player_at(place, kit)
                self.word(entry + kit * 4, p)
                self.word(p, PVT)
                self.word(p + 0x108, DMI)
                self.word(p + 0x10C, p + 0x800)    # an inventory of 23 empty slots
                self.byte(p + 0x114, place)
                self.byte(p + 0x115, kit)
                self.word(p + 0x700, p + 0x900)    # its damage model, [player+108h] -> +8 -> here
                self.word(p + 0x900 + 0x32C, -1)
            self.byte(entry + 0xC, 0)              # the first kit, as the slot's bind (0x321820)
            self.byte(entry + 0xD, 0)
            self.word(entry + 0x28, core)
            self.byte(entry + 0x2C, place)

    def state(self, value):
        self.word(self.core + 0x18FE4, value)

    def places(self, count):
        """A match of `count` places: the core's places end there; the
        entries after it are as an earlier match left them."""
        self.word(self.core + 0x1845C, PLACES + count * 0x30)

    def alive(self, place, kit=0, feet=(1.0, 2.0, 3.0), yaw=0.5, protected_until=None):
        p = player_at(place, kit)
        self.byte(p + 0x33C, 1)
        self.byte(p + 0x33F, 1)
        self.emu.mem_write(p + 0x43C, struct.pack("<3f", *feet))
        self.emu.mem_write(p + 0x490, struct.pack("<f", yaw))
        if protected_until is not None:
            self.word(p + 0x900 + 0x32C, protected_until)
        self.byte(PLACES + place * 0x30 + 0xC, kit)
        self.byte(PLACES + place * 0x30 + 0xD, kit)

    def choose(self, place, kit):
        self.byte(PLACES + place * 0x30 + 0xD, kit)

    def relay(self, place, time, kit):
        """A kit choice in the place's list ([core+89D8h], 480h a place: +8 the
        newest node; a node: +4 the next older, +8 the kit, +20h its time),
        kept in time order as the network's insert keeps it."""
        node = CHOICES + self.nodes * 0x40
        self.nodes += 1
        self.word(node + 0x20, time)
        self.byte(node + 8, kit)
        head = LISTS + place * 0x480 + 8
        newer, older = 0, self.read(head)
        while older and self.read(older + 0x20) > time:
            newer, older = older, self.read(older + 4)
        self.word(node + 4, older)
        if newer:
            self.word(newer + 4, node)
        else:
            self.word(head, node)

    def record(self, place):
        return STATS + 8 + place * 0x1F8

    def stub(self, emu, address, size, _):
        sp, this = emu.reg_read(UC_X86_REG_ESP), emu.reg_read(UC_X86_REG_ECX)
        if address == REMOVE:
            assert self.read(sp + 4) == 1
            self.byte(this + 0x33F, 0)
            self.calls.append(("remove", this))
            self.returns(0, 4)
        elif address == INIT:                      # initialize(time, &position, yaw, pitch)
            assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "x87 stack not empty"
            time, position = self.read(sp + 4), self.read(sp + 8)
            feet = struct.unpack("<3f", emu.mem_read(position, 12))
            yaw, pitch = struct.unpack("<2f", emu.mem_read(sp + 12, 8))
            self.calls.append(("initialize", this, time, feet, yaw, pitch))
            self.byte(this + 0x33C, 1)
            self.returns(0, 16)
        elif address == PUT:
            assert self.read(sp + 4) == 1
            self.byte(this + 0x33F, 1)
            self.calls.append(("put", this))
            self.returns(0, 4)
        elif address == VIEW:
            self.calls.append(("view", self.read(sp + 4) & 0xFF))
            self.returns(0, 4)
        elif address == STAT_EVENT:                # (time, place, kit before, kit after)
            self.calls.append(("kit event", self.read(sp + 4), self.read(sp + 8) & 0xFF,
                               self.read(sp + 12) & 0xFF, self.read(sp + 16) & 0xFF))
            self.returns(0, 16)
        elif address == DAMAGE:                    # the damage model's holder, as 0x1F1790
            self.returns(this - 0x108 + 0x700, 0)

    def call(self, address, this, time):
        """address(this, 0, time, 0) as thiscall, `ret 0Ch`, to its return."""
        emu = self.emu
        for at, value in ((STACK, STOP), (STACK + 4, 0), (STACK + 8, time), (STACK + 12, 0)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, this)
        emu.emu_start(address, STOP, count=10000)
        assert emu.reg_read(UC_X86_REG_ESP) == STACK + 16

    def run(self, time=NOW):
        emu = self.emu
        for at, value in ((STACK, STOP), (STACK + 4, 0), (STACK + 8, time), (STACK + 12, 0)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, RULE)
        emu.reg_write(UC_X86_REG_EBP, 0x0BADF00D)
        for register in KEPT:
            emu.reg_write(register, 0x12345678)
        emu.emu_start(TICK, TICK_ON, count=200000)
        # Where the original is after `push ebp` / `mov ebp,esp` / `sub esp,34h`.
        assert emu.reg_read(UC_X86_REG_EIP) == TICK_ON
        assert emu.reg_read(UC_X86_REG_ESP) == STACK - 4 - 0x34
        assert emu.reg_read(UC_X86_REG_EBP) == STACK - 4 and self.read(STACK - 4) == 0x0BADF00D
        assert emu.reg_read(UC_X86_REG_ECX) == RULE
        assert all(emu.reg_read(register) == 0x12345678 for register in KEPT)
        assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "x87 stack left non-empty"
        return self.calls

    def world_tick(self, time, start=START):
        """One tick in the order of the builds with the late join: the
        respawn rule (tick list 0, the core's counter for its key: this
        patch's pass, then the spawn of each place due now), the places
        (list 0, keys (place+1) << 16 from the late join's binds), then the
        time rule (list 1), which begins play at `start`."""
        self.run(time)
        for place in range(20):
            if self.read(RULE + 0x138 + place * 4) == time:
                self.spawn(place, time)
        for place in range(20):
            self.call(PLACE_TICK, PLACES + place * 0x30, time)
        if time >= start:
            self.state(3)

    def spawn(self, place, time):
        """The tick's own spawn of a place due now (0x3169B9: the game's
        spawn 0x316DA0 with (place, point, time)), at a point of the map."""
        point = STACK - 0x400
        self.emu.mem_write(point, bytes(0x24))
        self.emu.mem_write(point + 8, struct.pack("<4f", 50.0, 0.0, -20.0, 1.5))
        emu = self.emu
        for at, value in ((STACK, STOP), (STACK + 4, place), (STACK + 8, point), (STACK + 12, time)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, RULE)
        emu.emu_start(SPAWN, STOP, count=20000)
        assert emu.reg_read(UC_X86_REG_ESP) == STACK + 16

    def die(self, place, respawn_at):
        """The place's player dead, its respawn time set (0x316AA0)."""
        kit = self.kits(place)[0]
        self.byte(player_at(place, kit) + 0x33C, 0)
        self.word(RULE + 0x138 + place * 4, respawn_at)

    def kits(self, place):
        entry = PLACES + place * 0x30
        return self.byte(entry + 0xC), self.byte(entry + 0xD)


def switched(place, before, after, feet=(1.0, 2.0, 3.0), yaw=0.5, protection=0, time=NOW):
    """The calls of one respawn in place."""
    old, new = player_at(place, before), player_at(place, after)
    feet = struct.unpack("<3f", struct.pack("<3f", *feet))
    yaw = struct.unpack("<f", struct.pack("<f", yaw))[0]
    calls = [("remove", old), ("view", place), ("kit event", time, place, before, after),
             ("initialize", new, time, feet, yaw, 0.0)]
    return calls + [("put", new)], protection


def tick_cases():
    """(name, set-up, the build's calls)."""
    def plain(t):
        t.alive(3)
        t.choose(3, 1)
    yield "waits for the countdown", lambda t: (plain(t), t.state(1)), [switched(3, 0, 1)]
    yield "counts down", plain, [switched(3, 0, 1)]
    yield "in process", lambda t: (plain(t), t.state(3)), []
    yield "ended", lambda t: (plain(t), t.state(4)), []
    yield "no state yet", lambda t: (plain(t), t.state(0)), []
    yield "the same kit", lambda t: t.alive(3), []
    yield "kit 2 to kit 0", lambda t: (t.alive(7, kit=2, feet=(-5.5, 0.25, 100.0), yaw=-3.0), t.choose(7, 0)), \
        [switched(7, 2, 0, feet=(-5.5, 0.25, 100.0), yaw=-3.0)]
    yield "a kit with no player", lambda t: (plain(t), t.word(PLACES + 3 * 0x30 + 4, 0)), []
    yield "kit 3", lambda t: (t.alive(3), t.choose(3, 3)), []
    yield "dead", lambda t: (plain(t), t.byte(player_at(3, 0) + 0x33C, 0)), []
    yield "not in the world", lambda t: (plain(t), t.byte(player_at(3, 0) + 0x33F, 0)), []
    yield "a shot fired", lambda t: (plain(t), t.emu.mem_write(t.record(3) + (22 * 3 + 0) * 2, b"\1\0")), []
    yield "a shot in kit 2", lambda t: (plain(t), t.emu.mem_write(t.record(3) + (22 * 3 + 2) * 2, b"\1\0")), []
    yield "a kill", lambda t: (plain(t), t.emu.mem_write(t.record(3), b"\1\0")), []
    yield "damage taken", lambda t: (plain(t), t.emu.mem_write(t.record(3) + 0xA8 + 19 * 12 + 8,
                                                               struct.pack("<f", 0.25))), []
    yield "another place's statistics", lambda t: (plain(t), t.emu.mem_write(t.record(4), b"\1\0")), \
        [switched(3, 0, 1)]
    yield "no statistics", lambda t: (plain(t), t.word(t.core + 0x18FA8, 0)), []
    yield "invulnerable 2.5 s more", lambda t: (t.alive(3, protected_until=NOW + 2500), t.choose(3, 1)), \
        [switched(3, 0, 1, protection=2500)]
    yield "invulnerable until now", lambda t: (t.alive(3, protected_until=NOW), t.choose(3, 1)), [switched(3, 0, 1)]
    yield "invulnerable until before", lambda t: (t.alive(3, protected_until=NOW - 1), t.choose(3, 1)), \
        [switched(3, 0, 1)]
    yield "places 0 and 19", lambda t: (t.alive(0), t.choose(0, 2), t.alive(19, kit=1), t.choose(19, 0)), \
        [switched(0, 0, 2), switched(19, 1, 0)]
    # A match of six places: the entries past them are not this match's,
    # whatever an earlier match left in them.
    yield "a match of six places", lambda t: (t.places(6), t.alive(5), t.choose(5, 1), t.alive(6), t.choose(6, 1),
                                              t.alive(12, kit=2), t.choose(12, 0)), [switched(5, 0, 1)]
    yield "no places", lambda t: (t.places(0), plain(t)), []
    # The new kit played before (an earlier switch gave it a protection that
    # is over, and a fade): the spawn is given -1 - now, so it writes -1 and
    # no fade, as in a game that loaded a world since and has that kit's
    # player as it was made.
    yield "the new kit protected before", lambda t: (plain(t), t.word(player_at(3, 1) + 0x900 + 0x32C, NOW - 9000),
                                                     t.word(player_at(3, 1) + 0x900 + 0x330, 1500)), \
        [switched(3, 0, 1)]
    yield "the new kit protected before, 2.5 s more", \
        lambda t: (t.alive(3, protected_until=NOW + 2500), t.choose(3, 1),
                   t.word(player_at(3, 1) + 0x900 + 0x32C, NOW - 9000), t.word(player_at(3, 1) + 0x900 + 0x330, 1500)), \
        [switched(3, 0, 1, protection=2500)]
    # A late joiner comes into a match in play: its slot is bound with its
    # first kit current and next (0x321820), it spawns in it, and a choice
    # of its is for its next life.
    yield "a late joiner", lambda t: (t.state(3), t.alive(12)), []
    yield "a late joiner's choice", lambda t: (t.state(3), t.alive(12), t.choose(12, 1)), []


def check_tick(data, patched):
    checks = 0
    for name, setup, expected in tick_cases():
        t = Tick(data)
        setup(t)
        before = [t.kits(place) for place in range(20)]
        calls = t.run()
        wanted = expected if patched else []
        assert calls == [call for calls_, _ in wanted for call in calls_], (patched, name, calls)
        for place in range(20):
            kits = t.kits(place)
            moved = [w for w in wanted if w[0][0][1] == player_at(place, before[place][0])]
            if moved:
                assert kits == (before[place][1],) * 2, (name, place, kits)
                assert t.read(RULE + 0x138 + place * 4) == 0xFFFFFFFF, name
                new = player_at(place, before[place][1])
                protection = moved[0][1]
                until = t.read(new + 0x900 + 0x32C)
                assert until == (NOW + protection if protection else 0xFFFFFFFF), (name, until)
                assert t.read(new + 0x900 + 0x330) == 0, name
            else:
                assert kits == before[place], (patched, name, place)
        checks += 1
    return checks


def boundary_cases():
    """(name, the relayed choices as (time, kit), the ticks played, the
    tick of the build's switch or None, the kits after)."""
    ticks = [START - 150, START - 100, START - 50, START, START + 50, START + 100]
    # A choice for a tick is the place's next kit from that tick's places on,
    # so the respawn rule plays it at the next tick.
    yield "a choice for two ticks before play", [(START - 100, 1)], ticks, START - 50, (1, 1)
    yield "a choice for the last tick before play", [(START - 50, 1)], ticks, START, (1, 1)
    yield "a choice for the tick play begins in", [(START, 1)], ticks, None, (0, 1)
    yield "a choice for the first tick of play", [(START + 50, 1)], ticks, None, (0, 1)
    yield "two choices before play, the last wins", [(START - 150, 2), (START - 50, 1)], ticks, \
        [START - 100, START], (1, 1)
    yield "a choice before, and one in play", [(START - 50, 2), (START + 50, 1)], ticks, START, (2, 1)


def check_boundary(data, patched):
    """The world's ticks around the start in the game's order: a choice
    switches at the tick after the one it is for while that is before play
    or is the one play begins in (before the time rule's change), never
    later. A game that gets the choice late plays those ticks again from a
    world before it (a rollback): a fresh game with the choice in its list
    is that game, and switches at the same tick."""
    checks = 0
    for name, choices, ticks, at, after in boundary_cases():
        t = Tick(data)
        t.alive(5)
        for time, kit in choices:
            t.relay(5, time, kit)
        switches = []
        for time in ticks:
            done = len(t.calls)
            t.world_tick(time)
            switches += [time for call in t.calls[done:] if call[0] == "initialize"]
        wanted = ([] if at is None else at if isinstance(at, list) else [at]) if patched else []
        assert switches == wanted, (patched, name, switches)
        assert t.kits(5) == (after if patched else (0, after[1])), (patched, name, t.kits(5))
        assert t.read(t.core + 0x18FE4) == 3
        checks += 1
    return checks


# The client's view of a spawn into another kit (0x2678E0): ebx = the
# client's respawn rule, [esp+4] the place.
GAME, CVT = HEAP + 0x10000, HEAP + 0x11000
CLIENT = CLIENTS


class View(Emu):
    def __init__(self, data, shown_state, local=True):
        super().__init__(data)
        self.local = local
        self.word(RULE + 0x1A8, GAME)
        self.word(GAME + 0x53AC, CLIENT)
        self.word(CLIENT, CVT)
        for slot, stub in ((0x54, STUB), (0x4C, STUB + 0x10), (0x5C, STUB + 0x20)):
            self.word(CVT + slot, stub)
        self.word(CLIENT + base.KIT_CHANGE_SHOWN_STATE, shown_state)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == STUB:                        # is the place the local one
            self.returns(1 if self.local else 0, 4)
        elif address == STUB + 0x10:               # the place's player, into [arg]
            self.word(self.read(sp + 4), 0)
            self.calls.append(("bind",))
            self.returns(self.read(sp + 4), 8)
        elif address == STUB + 0x20:               # made the local player
            self.returns(0, 4)
        elif address == PLAYER_CAMERA:             # eax = the game world, [esp+4] first person
            assert emu.reg_read(UC_X86_REG_EAX) == GAME + 0x18
            self.calls.append(("player camera", self.read(sp + 4)))
            self.returns(0, 4)
        elif address == WARMUP_CAMERA:             # edx = the game world
            assert emu.reg_read(UC_X86_REG_EDX) == GAME + 0x18
            self.calls.append(("warmup camera",))
            self.returns(0, 0)

    def run(self):
        emu = self.emu
        for at, value in ((STACK, STOP), (STACK + 4, 6)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, RULE)
        for register in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP):
            emu.reg_write(register, 0x12345678)
        emu.emu_start(KIT_VIEW, STOP, count=2000)
        assert emu.reg_read(UC_X86_REG_ESP) == STACK + 8
        assert all(emu.reg_read(register) == 0x12345678
                   for register in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))
        return self.calls


def view_cases():
    """(name, the state the camera is set for, local, the original's calls, the build's)."""
    player, warmup = [("bind",), ("player camera", 1)], [("bind",), ("warmup camera",)]
    yield "a switch while waiting", 1, True, player, warmup
    yield "a switch counting down", 2, True, player, warmup
    yield "a switch before any state", 0, True, player, warmup
    # A rollback that plays a switch again after play began: the camera is
    # play's, set by the time rule's change of the tick first played.
    yield "a switch played again in play", 3, True, player, player
    yield "a respawn into another kit in play", 3, True, player, player
    yield "another place's switch", 2, False, [], []


def check_view(data, patched):
    checks = 0
    for name, state, local, original, build in view_cases():
        assert View(data, state, local).run() == (build if patched else original), (patched, name)
        checks += 1
    return checks


# The hud: game_world_ui (esi) and the local player (ebp), from 0x23B7B3
# (the alive flag compared with the one the hud saw) to the player's
# release (0x23B814).
UI_FROM, UI_TO = 0x23B7B3, 0x23B814
UI, WORLD, HGAME, LOCAL = (HEAP + 0x20000 + i * 0x1000 for i in range(4))
NET = CLIENTS + 0x200000
SHOWN = 0x2DF


class Hud(Emu):
    def __init__(self, data, alive, seen_alive, shown, state):
        super().__init__(data)
        self.word(UI + 0x18, WORLD)
        self.word(WORLD + 0x9C, HGAME)
        self.word(HGAME + 0x53AC, NET)
        self.word(NET + base.KIT_CHANGE_SHOWN_STATE, state)
        self.byte(UI + 0x2DE, seen_alive)
        self.byte(UI + SHOWN, shown)
        self.byte(LOCAL + 0x33C, alive)
        self.word(LOCAL + 0xD0, 5)                 # a reference besides the hud's

    def stub(self, emu, address, size, _):
        if address in (0x239C60, 0x23A1A0):        # the hud's other elements; al, edx
            assert emu.reg_read(UC_X86_REG_EAX) & 0xFF in (0, 1)
            self.returns(emu.reg_read(UC_X86_REG_EAX), 0)
        elif address == 0x2383B0:                  # show or hide the kits: eax = the hud, [esp+4]
            assert emu.reg_read(UC_X86_REG_EAX) == UI
            show = self.read(emu.reg_read(UC_X86_REG_ESP) + 4) & 0xFF
            if self.byte(UI + SHOWN) != show:      # the real one returns at once when it is so
                self.calls.append(("kits", show))
                self.byte(UI + SHOWN, show)
            self.returns(0, 4)

    def run(self):
        emu = self.emu
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ESI, UI)
        emu.reg_write(UC_X86_REG_EBP, LOCAL)
        emu.reg_write(UC_X86_REG_EBX, 0x12345678)
        emu.emu_start(UI_FROM, UI_TO, count=1000)
        assert emu.reg_read(UC_X86_REG_EIP) == UI_TO and emu.reg_read(UC_X86_REG_ESP) == STACK
        assert (emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EBP), emu.reg_read(UC_X86_REG_EBX)) \
            == (UI, LOCAL, 0x12345678)
        assert self.read(LOCAL + 0xD0) == 4
        return self.calls


def hud_cases():
    """(name, alive, alive as the hud saw it, kits shown, the state the
    camera is set for, the original's, the build's)."""
    for state in (0, 1, 2):
        yield f"alive in state {state}", 1, 0, 1, state, [("kits", 0)], []
        yield f"alive in state {state}, kits hidden", 1, 0, 0, state, [], [("kits", 1)]
        yield f"kits shown, alive in state {state}", 1, 1, 1, state, [], []
        yield f"kits hidden, alive in state {state}", 1, 1, 0, state, [], [("kits", 1)]
    yield "alive in play", 1, 0, 1, 3, [("kits", 0)], [("kits", 0)]
    yield "a late joiner alive", 1, 0, 0, 3, [], []
    yield "dead before the start", 0, 1, 0, 2, [("kits", 1)], [("kits", 1)]
    yield "dead in play", 0, 1, 0, 3, [("kits", 1)], [("kits", 1)]
    yield "kits shown, play begins", 1, 1, 1, 3, [], [("kits", 0)]
    yield "kits shown, dead in play", 0, 0, 1, 3, [], []
    yield "kits hidden, alive in play", 1, 1, 0, 3, [], []


def check_hud(data, patched):
    checks = 0
    for name, alive, seen, shown, state, original, build in hud_cases():
        hud = Hud(data, alive, seen, shown, state)
        assert hud.run() == (build if patched else original), (patched, name, hud.calls)
        checks += 1
    return checks


# The profile_changed callback after it queued the pick (0x1F8070): from
# 0x233F28 (ebx = the hud) to its `pop edi` (0x233F31).
PICK_FROM, PICK_TO = 0x233F28, 0x233F31


class Pick(Hud):
    def run(self):
        emu = self.emu
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_EBX, UI)
        emu.reg_write(UC_X86_REG_ESI, 0x12345678)
        emu.reg_write(UC_X86_REG_EBP, 0x0BADF00D)
        emu.emu_start(PICK_FROM, PICK_TO, count=1000)
        assert emu.reg_read(UC_X86_REG_EIP) == PICK_TO and emu.reg_read(UC_X86_REG_ESP) == STACK
        assert (emu.reg_read(UC_X86_REG_EBX), emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EBP)) \
            == (UI, 0x12345678, 0x0BADF00D)
        return self.calls


def pick_cases():
    """(name, alive, the state the camera is set for, the original's, the
    build's): the kits are shown; the build leaves them so, and the hud's
    cursor where it is, for another pick."""
    for state in (0, 1, 2):
        yield f"a pick alive in state {state}", 1, state, [("kits", 0)], []
    yield "a pick dead before the start", 0, 2, [("kits", 0)], []
    yield "a pick on the death screen in play", 0, 3, [("kits", 0)], []


def check_pick(data, patched):
    checks = 0
    for name, alive, state, original, build in pick_cases():
        pick = Pick(data, alive, alive, 1, state)
        assert pick.run() == (build if patched else original), (patched, name, pick.calls)
        checks += 1
    return checks


# A mouse move: the input world's loop over its handlers (0x3B9F0A, esi =
# the mouse: +4, +8, +0Ch the move, +10h the buttons held, +2Ch the input
# world, whose +28h is the mouse; ecx = the handlers' vector, its first dword
# their first, ebp their end) to its end (0x3B9F34), with the game world's
# input handler (its vtable 0x9FC588, on_mouse_move 0x25D0E0) and then the
# player's (priority 7 before 10). A run is a series of moves in one game,
# the patch's state kept between them.
MOVE_FROM, MOVE_TO = 0x3B9F0A, 0x3B9F34
LOOK_WORLD = HEAP + 0x30000                        # the game world; its input handler at +1F8h
LOOK_HANDLER = LOOK_WORLD + 0x1F8
MOUSE, LIST, PLAYER_INPUT, INPUT_WORLD, HANDLERS = (HEAP + 0x38000 + i * 0x400 for i in range(5))
GVT, CURSOR_POS, PIVT = HEAP + 0x39800, HEAP + 0x39900, HEAP + 0x39A00
SET_POINTER, GET_CURSOR, PLAYER_MOVE, PUT_CURSOR = STUB + 0x80, STUB + 0x90, STUB + 0xA0, STUB + 0xB0
LEFT, RIGHT = 1, 2


class Look(Emu):
    def __init__(self, data, kits=1, mode=3, cursor=1):
        super().__init__(data)
        self.word(LOOK_WORLD, GVT)
        self.word(GVT + 0x30, SET_POINTER)
        self.word(GVT + 0x3C, PUT_CURSOR)
        self.word(LOOK_HANDLER, 0x9FC588)
        self.word(LOOK_HANDLER + 0x4B70, CURSOR_POS)
        self.word(CURSOR_POS, CURSOR_POS + 0x10)
        self.word(CURSOR_POS + 0x10, GET_CURSOR)
        self.byte(LOOK_WORLD + 0x1FC + 0x2DC, cursor)
        self.byte(LOOK_WORLD + 0x1FC + 0x2DF, kits)
        if mode is not None:
            self.word(LOOK_WORLD + 0x4D58, PLAYER_INPUT)
            self.word(PLAYER_INPUT + 0x358, mode)
        self.word(PLAYER_INPUT, PIVT)
        self.word(PIVT + 0xC, PLAYER_MOVE)
        self.word(LIST, LOOK_HANDLER)
        self.word(LIST + 4, PLAYER_INPUT)
        self.word(HANDLERS, LIST)
        self.word(MOUSE + 0x2C, INPUT_WORLD)
        self.word(INPUT_WORLD + 0x28, MOUSE)
        self.system = [300, 200]                   # the system's cursor

    def stub(self, emu, address, size, _):
        sp, this = emu.reg_read(UC_X86_REG_ESP), emu.reg_read(UC_X86_REG_ECX)
        signed = lambda at: struct.unpack("<i", emu.mem_read(at, 4))[0]
        if address == GET_CURSOR:                  # the system's cursor into (&x, &y)
            assert this == CURSOR_POS
            self.word(self.read(sp + 4), self.system[0])
            self.word(self.read(sp + 8), self.system[1])
            self.returns(0, 8)
        elif address == SET_POINTER:               # the hud's pointer to (x, y, wheel), as 0x25D160
            assert this == LOOK_WORLD
            x, y, z = signed(sp + 4), signed(sp + 8), signed(sp + 12)
            self.word(LOOK_WORLD + 0x8C, x)
            self.word(LOOK_WORLD + 0x90, y)
            self.calls.append(("pointer", x, y, z))
            self.returns(0, 12)
        elif address == PUT_CURSOR:                # the system's cursor and the pointer to (x, y), as 0x25D130
            assert this == LOOK_WORLD
            x, y = signed(sp + 4), signed(sp + 8)
            self.system = [x, y]
            self.word(LOOK_WORLD + 0x8C, x)
            self.word(LOOK_WORLD + 0x90, y)
            self.calls.append(("cursor back", x, y))
            self.returns(0, 8)
        elif address == PLAYER_MOVE:               # the player's on_mouse_move(world, x, y, z)
            assert this == PLAYER_INPUT and self.read(sp + 4) == INPUT_WORLD
            self.calls.append(("player", signed(sp + 8), signed(sp + 12), signed(sp + 16)))
            self.returns(1, 16)

    def move(self, buttons, x, y, z=0):
        """One frame's move with these buttons held; the system's cursor
        follows the mouse unless a drag holds the pointer."""
        self.system = [self.system[0] + x, self.system[1] + y]
        for at, value in ((MOUSE + 4, x), (MOUSE + 8, y), (MOUSE + 0xC, z)):
            self.word(at, value)
        self.byte(MOUSE + 0x10, buttons)
        self.calls = []
        emu = self.emu
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ESI, MOUSE)
        emu.reg_write(UC_X86_REG_ECX, HANDLERS)
        emu.reg_write(UC_X86_REG_EBP, LIST + 8)
        emu.reg_write(UC_X86_REG_EBX, 0x12345678)
        emu.emu_start(MOVE_FROM, MOVE_TO, count=2000)
        assert emu.reg_read(UC_X86_REG_EIP) == MOVE_TO and emu.reg_read(UC_X86_REG_ESP) == STACK
        assert (emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EBP), emu.reg_read(UC_X86_REG_EBX)) \
            == (MOUSE, LIST + 8, 0x12345678)
        return self.calls


def look_cases():
    """(name, the kits shown, the player's input handler's mode or None for
    none, the cursor shown, [(buttons, x, y, wheel, the original's calls, the
    build's)]): moves in one game, in order."""
    def cursor(x, y):                              # the original gives the pointer no wheel (`push 0`)
        return [("pointer", x, y, 0)]

    def player(x, y, z=0):
        return [("player", x, y, z)]
    away = [("pointer", -128, -128, 0)]
    yield "moves", 1, 3, 1, [
        (0, 5, 0, 0, cursor(305, 200), cursor(305, 200)),
        (0, 3, 4, 0, cursor(308, 204), cursor(308, 204))]
    yield "a click on a kit, the hand moving a little", 1, 3, 1, [
        (LEFT, 2, 1, 0, cursor(302, 201), cursor(302, 201)),
        (LEFT, 1, -2, 0, cursor(303, 199), cursor(303, 199)),
        (0, 0, 1, 0, cursor(303, 200), cursor(303, 200)),
        (LEFT, 4, 4, 0, cursor(307, 204), cursor(307, 204))]   # a new press counts from 0
    yield "a left drag", 1, 3, 1, [
        (0, 10, 0, 0, cursor(310, 200), cursor(310, 200)),
        (LEFT, 5, 0, 0, cursor(315, 200), cursor(315, 200)),
        (LEFT, 6, 0, 0, cursor(321, 200), away + player(6, 0)),
        (LEFT, 10, -4, 0, cursor(331, 196), player(10, -4)),
        (LEFT, -3, 2, 120, cursor(328, 198), player(-3, 2, 120)),
        (0, 1, 0, 0, cursor(329, 198), [("cursor back", 315, 200)] + cursor(315, 200)),
        (0, 2, 2, 0, cursor(331, 200), cursor(317, 202))]
    yield "a right drag (the lobby's button)", 1, 3, 1, [
        (RIGHT, 3, 0, 0, cursor(303, 200), away + player(3, 0)),
        (RIGHT, 1, 1, 0, cursor(304, 201), player(1, 1)),
        (0, 1, 0, 0, cursor(305, 201), [("cursor back", 300, 200)] + cursor(300, 200))]
    yield "the wheel", 1, 3, 1, [
        (0, 0, 0, 120, cursor(300, 200), player(0, 0, 120)),
        (0, 4, 2, -120, cursor(304, 202), player(0, 0, -120)),
        (0, 1, 0, 0, cursor(305, 202), cursor(305, 202))]
    for mode, what in ((0, "first person"), (2, "third person"), (4, "the input off")):
        yield f"the death screen ({what}): a left drag", 1, mode, 1, [
            (LEFT, 10, 0, 0, cursor(310, 200), cursor(310, 200)),
            (LEFT, 10, 0, 120, cursor(320, 200), cursor(320, 200)),
            (0, 1, 0, 0, cursor(321, 200), cursor(321, 200))]
    yield "a cursor without the kits before play (a replay's): a drag", 0, 3, 1, [
        (LEFT, 10, 0, 0, cursor(310, 200), cursor(310, 200)),
        (RIGHT, 10, 0, 0, cursor(320, 200), cursor(320, 200)),
        (0, 0, 0, 120, cursor(320, 200), cursor(320, 200))]
    yield "no player input handler", 1, None, 1, [
        (LEFT, 10, 0, 0, cursor(310, 200), cursor(310, 200)),
        (0, 0, 0, 120, cursor(310, 200), cursor(310, 200))]
    yield "no cursor, no kits, before play", 0, 3, 0, [
        (0, 5, 0, 0, cursor(305, 200) + player(5, 0), cursor(305, 200) + player(5, 0)),
        (LEFT, 5, 0, 120, cursor(310, 200) + player(5, 0, 120), cursor(310, 200) + player(5, 0, 120))]


def check_look(data, patched):
    checks = 0
    for name, kits, mode, shown, moves in look_cases():
        look = Look(data, kits, mode, shown)
        if shown:                                  # the system's cursor starts where the pointer is
            look.word(LOOK_WORLD + 0x8C, 300)
            look.word(LOOK_WORLD + 0x90, 200)
        for i, (buttons, x, y, z, original, build) in enumerate(moves):
            calls = look.move(buttons, x, y, z)
            assert calls == (build if patched else original), (patched, name, i, calls)
            checks += 1
    return checks


# The mouse's buttons (vostok::input::platform::mouse::process 0x3B9D90, ecx
# = the mouse, [esp+4] the handlers' vector, to 0x3B9EF8 where its moves
# begin): the buttons held now (+10h) against the frame before (+20h), each
# button gone down (kinds 0 and 2), let go (1) and still held (2) to the
# handlers (vtable+8, the world, the code 151h + the bit, the kind) until one
# answers (al) not 0: the game world's input handler (on_mouse_key_action 0x25CF60,
# which takes them while the cursor is shown) and then the player's, here a
# stub that keeps what it is told. A run is a series of frames in one game:
# the cursor shown (the kits before the start, the death screen) or not.
KEYS_FROM, KEYS_TO = 0x3B9D90, 0x3B9EF8
KEYBOARD, KBVT, HUD_OBJECT = HEAP + 0x39B00, HEAP + 0x39C00, HEAP + 0x39D00
IWVT = HEAP + 0x39E00
GET_KEYBOARD, KEY_DOWN, UI_CLICK, PLAYER_KEY = STUB + 0xC0, STUB + 0xD0, STUB + 0xE0, STUB + 0xF0
UI_CLICK_VA = 0x2AF610                             # the hud's click (shown cursor), a stub


class Keys(Look):
    def __init__(self, data):
        super().__init__(data, kits=1, mode=3, cursor=1)
        self.word(PIVT + 0x8, PLAYER_KEY)
        self.word(INPUT_WORLD, IWVT)
        self.word(IWVT + 0x24, GET_KEYBOARD)
        self.word(KEYBOARD, KBVT)
        self.word(KBVT, KEY_DOWN)
        self.word(LOOK_HANDLER + 0xC, HUD_OBJECT)
        self.word(HANDLERS + 4, LIST + 8)          # the vector's end
        self.held = 0
        self.player = []

    def shown(self, value):
        self.byte(LOOK_WORLD + 0x1FC + 0x2DC, value)

    def stub(self, emu, address, size, _):
        sp, this = emu.reg_read(UC_X86_REG_ESP), emu.reg_read(UC_X86_REG_ECX)
        if address == GET_KEYBOARD:                # the input world's keyboard
            assert this == INPUT_WORLD
            self.returns(KEYBOARD, 0)
        elif address == KEY_DOWN:                  # no Alt held
            assert this == KEYBOARD
            self.returns(0, 4)
        elif address == UI_CLICK_VA:               # the hud takes the click
            self.calls.append(("hud", self.read(sp + 4) & 0xFF))
            self.returns(0, 12)
        elif address == PLAYER_KEY:                # the player's on_mouse_key_action(world, code, kind)
            assert this == PLAYER_INPUT and self.read(sp + 4) == INPUT_WORLD
            self.player.append((self.read(sp + 8), self.read(sp + 12)))
            self.returns(0, 12)
        else:
            super().stub(emu, address, size, _)

    def frame(self, buttons):
        """One frame with these buttons held: what the player's handler is told."""
        self.byte(MOUSE + 0x20, self.held)
        self.byte(MOUSE + 0x10, buttons)
        self.held = buttons
        self.player = []
        emu = self.emu
        emu.mem_write(STACK - 8, struct.pack("<II", STOP, HANDLERS))
        emu.reg_write(UC_X86_REG_ESP, STACK - 8)
        emu.reg_write(UC_X86_REG_ECX, MOUSE)
        emu.emu_start(KEYS_FROM, KEYS_TO, count=4000)
        assert emu.reg_read(UC_X86_REG_EIP) == KEYS_TO
        return self.player


def keys_cases():
    """(name, [(the cursor shown, the buttons held, what the original tells
    the player's handler, what the build tells it)]): frames in one game."""
    left, right = 0x151, 0x152
    held = lambda code: [(code, 2)]
    press = lambda code: [(code, 0), (code, 2)]
    up = lambda code: [(code, 1)]
    yield "a left drag held into the start", [
        (1, LEFT, [], []), (1, LEFT, [], []),
        (0, LEFT, held(left), []), (0, LEFT, held(left), []),        # the original fires
        (0, 0, up(left), []),                                       # and at the release too
        (0, LEFT, press(left), press(left)), (0, LEFT, held(left), held(left)), (0, 0, up(left), up(left))]
    yield "a right drag held into the start", [
        (1, RIGHT, [], []), (0, RIGHT, held(right), []), (0, 0, up(right), []),
        (0, RIGHT, press(right), press(right))]
    yield "a pick on the death screen held through the respawn", [
        (1, LEFT, [], []), (1, LEFT | RIGHT, [], []),
        (0, LEFT | RIGHT, held(left) + held(right), []), (0, RIGHT, up(left) + held(right), []),
        (0, 0, up(right), [])]
    yield "a click before the start, let go before it", [
        (1, LEFT, [], []), (1, 0, [], []), (0, 0, [], []), (0, LEFT, press(left), press(left))]
    yield "a press in play", [
        (0, LEFT, press(left), press(left)), (0, LEFT, held(left), held(left)), (0, 0, up(left), up(left))]


def check_keys(data, patched):
    checks = 0
    for name, frames in keys_cases():
        keys = Keys(data)
        for i, (shown, buttons, original, build) in enumerate(frames):
            keys.shown(shown)
            told = keys.frame(buttons)
            assert told == (build if patched else original), (patched, name, i, told)
            checks += 1
    return checks


# The warmup mode's step (process_warmup_mode 0x242200, esi = the player's
# input handler, to its `ret`): the wheel of the frame (+348h) into the
# distance (+354h), the turn (+338h, two halves; 0: none) into the yaw and
# pitch (+34Ch, +350h).
WARMUP_MODE = 0x242200


def warmup_step(data, pitch, distance, wheel):
    emu = Emu(data)
    for at, value in ((PLAYER_INPUT + 0x348, wheel), (PLAYER_INPUT + 0x34C, 0.25), (PLAYER_INPUT + 0x350, pitch),
                      (PLAYER_INPUT + 0x354, distance)):
        emu.emu.mem_write(at, struct.pack("<f", value))
    emu.word(PLAYER_INPUT + 0x338, 0)
    emu.word(STACK, STOP)
    emu.emu.reg_write(UC_X86_REG_ESP, STACK)
    emu.emu.reg_write(UC_X86_REG_ESI, PLAYER_INPUT)
    emu.emu.reg_write(UC_X86_REG_EDI, 0x12345678)
    emu.emu.emu_start(WARMUP_MODE, STOP, count=2000)
    assert emu.emu.reg_read(UC_X86_REG_ESP) == STACK + 4
    assert (emu.emu.reg_read(UC_X86_REG_ESI), emu.emu.reg_read(UC_X86_REG_EDI)) == (PLAYER_INPUT, 0x12345678)
    yaw, pitch, distance = struct.unpack("<3f", emu.emu.mem_read(PLAYER_INPUT + 0x34C, 12))
    return round(yaw, 4), round(pitch, 4), round(distance, 4)


def limits_cases():
    """(name, pitch, distance, the wheel of the frame, the original's (yaw,
    pitch, distance), the build's)."""
    yield "the spawn's distance", 0.0, 1.0, 0.0, (0.25, 0.0, 1.0), (0.25, 0.0, 1.5)
    yield "within the limits", 0.5, 2.0, 0.12, (0.25, 0.5, 2.12), (0.25, 0.5, 2.12)
    yield "nearer than the nearest", -0.5, 1.6, -0.36, (0.25, -0.5, 1.24), (0.25, -0.5, 1.5)
    yield "further than the farthest", 0.0, 4.5, 0.36, (0.25, 0.0, 4.86), (0.25, 0.0, 4.6)
    yield "a negative distance", 0.0, 0.1, -0.36, (0.25, 0.0, -0.26), (0.25, 0.0, 1.5)
    yield "looked up past 60 degrees", 1.5, 2.0, 0.0, (0.25, 1.5, 2.0), (0.25, 1.0472, 2.0)
    yield "looked down past 60 degrees", -3.0, 2.0, 0.0, (0.25, -3.0, 2.0), (0.25, -1.0472, 2.0)


def check_limits(data, patched):
    checks = 0
    for name, pitch, distance, wheel, original, build in limits_cases():
        got = warmup_step(data, pitch, distance, wheel)
        assert got == (build if patched else original), (patched, name, got)
        checks += 1
    return checks


def death_cases():
    """(name, the relayed choices as (time, kit), the kit of the respawn,
    the next kit after it): a dead player in play, due to respawn at R."""
    R = 30_000
    yield "a pick", [(R - 2000, 1)], 1, 1
    yield "a pick and a re-pick: the last wins", [(R - 2000, 1), (R - 1000, 2)], 2, 2
    yield "a pick for the tick before the respawn", [(R - 2000, 1), (R - 50, 2)], 2, 2
    yield "a pick for the tick of the respawn: the next life's", [(R - 2000, 1), (R, 2)], 1, 2
    yield "no pick", [], 0, 0


def check_death(data, patched):
    """The death screen in play: the original's and the build's respawn
    alike, by the last kit choice for a tick before the respawn's; a choice
    for the respawn's tick is the next life's (the respawn rule ticks before
    the places). No switch while alive in play."""
    R = 30_000
    checks = 0
    for name, choices, kit, after in death_cases():
        t = Tick(data)
        t.state(3)
        t.alive(9)
        t.die(9, R)
        for time, choice in choices:
            t.relay(9, time, choice)
        for time in range(R - 2100, R + 150, 50):
            t.world_tick(time, start=0)
        spawned = [call for call in t.calls if call[0] == "initialize"]
        assert spawned == [("initialize", player_at(9, kit), R,
                            struct.unpack("<3f", struct.pack("<3f", 50.0, 0.0, -20.0)), 1.5, 0.0)], \
            (patched, name, spawned)
        assert t.kits(9) == (kit, after), (patched, name, t.kits(9))
        assert not [call for call in t.calls if call[0] == "remove"], (patched, name)
        checks += 1
    return checks


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0
    for data, is_patched in ((original, False), (patched, True)):
        checks += check_tick(data, is_patched) + check_boundary(data, is_patched)
        checks += check_view(data, is_patched) + check_hud(data, is_patched) + check_pick(data, is_patched)
        checks += check_death(data, is_patched) + check_look(data, is_patched) + check_limits(data, is_patched)
        checks += check_keys(data, is_patched)
    # A build that does not look at the statistics, or at the state, or at
    # the end of the places, or that gives the spawn no protection when none
    # is left (which leaves the new kit's damage model as it was), fails; so
    # does one whose view always gives the warmup, one whose mouse moves
    # drag in any mode, one that drags at the first move of a click, one
    # with no nearest distance, one that never marks a button, and one that
    # gives a marked button's release on (the player's handler fires at it).
    for old, new, check in ((bytes.fromhex("837c8afc00"), bytes.fromhex("39c0909090"), check_tick),
                            (bytes.fromhex("48d1e8750d"), bytes.fromhex("48d1e8eb00"), check_boundary),
                            (bytes.fromhex("3b865c84010073ee"), bytes.fromhex("3b865c8401009090"), check_tick),
                            (bytes.fromhex("29e8770489e8f7d0"), bytes.fromhex("29e8770431c09090"), check_tick),
                            (bytes.fromhex("83be601c1f000375"), bytes.fromhex("83be601c1f0003eb"), check_view),
                            (bytes.fromhex("83b858030000037416"), bytes.fromhex("83b85803000003eb16"), check_look),
                            (bytes.fromhex("890b83f90876b4"), bytes.fromhex("890b83f908eb00"), check_look),
                            (bytes.fromhex("39d00f4cc2"), bytes.fromhex("39d0909090"), check_limits),
                            (bytes.fromhex("0fab02"), bytes.fromhex("909090"), check_keys),
                            (bytes.fromhex("0fb302731c"), bytes.fromhex("0fb302eb1c"), check_keys)):
        broken = bytearray(patched)
        _, _, _, raw, size, _ = next(section for section in base.Headers(broken).sections(broken)
                                     if section[0] == base.SECTION_NAME)
        at = broken.rfind(old, raw, raw + size)    # in `.srp`
        assert at > raw
        broken[at:at + len(old)] = new
        try:
            check(bytes(broken), True)
        except AssertionError:
            checks += 1
        else:
            sys.exit(f"a broken build ({new.hex()}) passed")
    print(f"patch 31: {checks} runs as expected (the original, the build and ten broken builds)")


if __name__ == "__main__":
    main()
