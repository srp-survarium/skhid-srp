#!/usr/bin/env python3
"""Patch 143 (the Rust patch battery_hit, rust/patches/src/battery_hit.rs):
a battery's carrier hits with it (docs/notes/battery-hit.md).

    python check_battery_hit_026g.py <original survarium.exe>

Requires Python `unicorn`. check_hide_ui_026g.py's harness (the exe's
sections and the arena loaded as they are), the original beside the build.
The game's own code runs: initialize_logic (0x301920), the cook callback's
site after it (`battery.logic`, 0x2FA7E9), the take state's constructor
(0x338940), fsm::append_transition, fsm::tick (0x38CA20) with the states'
initialize, execute and finalize, the functors' invokers, the carry -> put
predicate (0x301B30), the animation player's clip-end handler (0x344830),
emit_bullet (0x322830) with the bullet's constructor (0x32BA60),
player_stance (0x2E43A0), the melee damage functions (0x2E1CF0, 0x2E1C80),
bullet::tick (0x32C180) with check_collision, update_bullet_position and
try_pierce, victory_item_core::deactivate (0x300F40, to past its state's
finalize) and is_ready_to_be_deactivated (0x301270), fsm::serialize and
deserialize (0x38CC90, 0x38CCF0) with the take's own. Stubbed: the
allocator (0xE4F90, a bump of ours), the allocation name (0x420789), a
clip's time scale (0x30D750: its time argument kept), the animation
player's event channels (subscribe 0x391C40, unsubscribe 0x3921A0), the
battery's get_transform / set_transform (victory_item's vtable +24h,
+0A4h), the bullet pool's room (0x323200) and live-list add (0x322A30),
the physics world (its ray test, +48h and 0x75E20 called directly by the
strike's aim: nothing, a body, or a scene of a body and a wall on the
battery's front; its bounds +64h), the struck body (a hit model's body:
the game's vtable, material actor; its cast, its hit +24h kept), the world
writers (their bytes kept). Checked:

- the cook's site: the original leaves the three states; the patched exe
  appends five: each strike's swing and recovery (the ram's, the shove's:
  0ECh bytes of the game's allocator, the take's allocation name; over
  the cook's clips of config states 3 to 6; the strikes' times; kind 3;
  their vtable the take's with exactly finalize +0Ch and may-be-put-away
  +20h ours) and, before the shove's, a carry of its own (0E0h bytes, the
  carry's name, constructor and vtable, over the carry's clips); a carry's transitions put then its swing (a
  function-pointer functor whose function is ours, the battery its
  buffer's second word), a swing's put then its recovery, a recovery's put
  then the next strike's carry, the shove's the first carry (byte copies of
  the native functors);
- fsm::tick from the carry: the swing only for the melee key (bit 27) and
  a living carrier; a switch away: the put first; the original stays;
- in a swing: nothing until its own clip's end event (another clip's
  changes nothing); then its recovery (or, switching, the put) and one strike
  through fsm::tick: the game's emit_bullet and constructor make the
  bullet (its source the slot-7 gun's item, else slot 10's; the melee
  data; damage 1.5; its stance the original's for a held gun in that
  posture), the deactivated gun's bytes never written;
- the bullet's flight (bullet::tick, a tick meeting nothing, every step
  reading the gun's melee damage) and a hit (a body met: try_pierce hits it
  once, the carrier the shooter, the gun's item the source, the damage read
  again at 0x32DA5A); the gun never written;
- the strikes in turn: the ram, then from its own carry the shove, then
  the ram again, each one strike at its swing's end;
- the strike's aim: 15 ray tests from the eye as check_collision's (the
  melee distance, group 10h, mask 48h, the carrier's own body skipped);
  a body off the look within the battery's width, or below the look at
  its height: the bullet along the first ray to it, one hit; a body just
  past its width or under its lower edge: along the look, no hit; a wall
  in front of the body: no hit; a body several rays meet: one bullet, one
  hit; the shove's grid a column further right;
- the kill feed: a strike's kill event (its gun's melee data, its gun's
  player holding the battery) gets the battery's icon 200; a gun-stock
  hit's, a round's and a bulletless one keep theirs;
- put away mid-swing (deactivate): the strike only with its clip ended and
  its carrier alive; a dead carrier's swing may be put away, a living
  one's not; left before its clip ended: no strike;
- a world saved mid-swing (the ram's, the shove's) or in the shove's carry
  and loaded by another game: its own state of that index current, every
  field the saver's;
- no gun in slot 7 or 10 (never in a match): the patch's own crash;
- the controls, the original exe: the deactivated gun's bullet constructor
  faults at 0x2E43A3 (the first in-game crash), its flight at 0x2E1D00
  (the second).
Says nothing about what the clip looks like, the pierce or ricochet at
0x32E35C, or a struck body's own hit code.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL, UcError
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI,
                               UC_X86_REG_ESP, UC_X86_REG_XMM0)

import patch_026g as base
import check_hide_ui_026g as hide

INITIALIZE_LOGIC = 0x301920      # EAX the battery; take clips, take time, carry clips, put clips, put time; ret 14h
SITE, SITE_AFTER = 0x2FA7E9, 0x2FA7EF   # battery.logic: mov ecx,[esp+20h] / mov eax,[ecx] replayed
TICK = 0x38CA20                  # fsm::tick, stdcall (the fsm)
MALLOC, NAME, SCALE = 0xE4F90, 0x420789, 0x30D750
SUBSCRIBE, UNSUBSCRIBE, EMIT = 0x391C40, 0x3921A0, 0x322830
POOL, LISTED = 0x323200, 0x322A30   # emit_bullet's room from its pool (ESI the pool; ret), the live list's add (ret 4)
TAKE_VTABLE, FUNCTION_PREDICATE, TAKE_NAME = 0xA1BF84, 0xA13CAC, 0xAE1E18
CARRY_VTABLE, CARRY_NAME = 0xA1BFB8, 0xAE1E50
WEAPON_VTABLE = 0xA04174
MELEE = 1 << 27

DATA = 0x30000000
BATTERY, IDENTITY = DATA, DATA + 0x900
BATTERY_VTABLE = 0xA042AC         # victory_item's (the client's battery); its +24h and +0A4h stubbed below
PLAYER, INVENTORY = DATA + 0x10000, DATA + 0x20000
GUN7, GUN10, MANAGER = DATA + 0x30000, DATA + 0x34000, DATA + 0x38000
CLIPS = DATA + 0x40000           # the three states' six clips each, then the clips
HEAP = DATA + 0x50000
QUERIES = DATA + 0xC0000         # the cook's queries_result: count +38h, results' managed resources +128h, 2E0h apart
# the strikes in their turn (battery_hit.rs TIMES, vostok/battery.rs Strike): its name, its swing's and its
# recovery's time; its config states are 3 + 2i and the next, its carry, swing and recovery the machine's
# states 1, 3, 4 (the ram, its carry the game's), then 5, 6, 7
STRIKES = (("ram", 6.0 / 30.0, 10.5 / 30.0), ("shove", 6.0 / 30.0, 10.5 / 30.0))
PLACES = ((1, 3, 4), (5, 6, 7))
CONFIG_STATES = 3 + 2 * len(STRIKES)
OWNER_WORLD, PROFILE, BULLET = DATA + 0x60000, DATA + 0x80000, DATA + 0x90000
GUN_ANIMS, BATTERY_ANIMS, POSTURE = DATA + 0x92000, DATA + 0x93000, DATA + 0x94000
# the bullet manager's physics world (+98h: ray test +48h, the level's bounds +64h), a body the ray
# meets (its collision object, game object and the hit target the object casts to)
PHYS, RESULT, BOUNDS = DATA + 0x95000, DATA + 0x95200, DATA + 0x95300
COLLISION, GAME_OBJECT, TARGET, MATERIAL = DATA + 0x96000, DATA + 0x96100, DATA + 0x96200, DATA + 0x97000
WALL = DATA + 0x96400            # a level object between (not a hit model's body, no game object)
RAY_TEST = 0x75E20               # bullet_physics_world::ray_test, called directly by the strike's aim (ret 20h)
ANIMATED_BODY_VTABLE = 0x9E2AF4  # a hit model's body (bt_animated_rigid_body; its +10h 0x70C00 reads +38h)
ACTOR = 10                       # its material (+38h)
STRIKE_FACE = 0.7                # battery.rs STRIKE_FACE
RIGHT, UP, FORWARD = (1.0, 0.0, 0.0), (0.0, 0.8, -0.6), (0.0, 0.6, 0.8)   # the carrier's eye
BULLET_TICK, AFTER_PIERCE = 0x32C180, 0x32D886   # bullet::tick (EAX now; the bullet; ret 4); check_collision after try_pierce
PIERCE_DAMAGE, STEP_DAMAGE = 0x32DA5A, 0x32F09A     # try_pierce's and update_bullet_position's damage calls
MELEE_DAMAGE_READ = 0x2E1D00
CLIP_END = 0x344830          # victory_item_core_animation_end_aware_state::on_animation_end (ECX the state; the event; ret 4)
DEACTIVATE, DEACTIVATED = 0x300F40, 0x300FBE   # victory_item_core::deactivate (ECX, a bool; ret 4), past its state's finalize
PUT_AWAY = 0x301270          # victory_item_core::is_ready_to_be_deactivated (ECX the battery)
SERIALIZE, DESERIALIZE = 0x38CC90, 0x38CCF0    # fsm::serialize (ESI the fsm, EDI the writer; ret 4) / deserialize (EDI the reader; ret 8)
WRITERS = {0x1F3DE0: 1, 0x80FA0: 1, 0x81A70: 12, 0x301DE0: 64, 0x1F48A0: 4}  # w<u8>, w<bool>, w<float3>, w<float4x4>, w<u32>                      # 0x2E1CF0's read of its gun's user's kit (the second crash)
STUBS = DATA + 0xF0000           # get_transform / set_transform of the battery: ret 4
SIZE = 0x100000
ALLOCATOR, AI_ALLOCATOR = DATA + 0xE8000, DATA + 0xE9000   # the slots 0x1F1D9C4, 0x1F1DA44 (run-time in the game)
FSM = BATTERY + 0x168


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, SIZE, UC_PROT_ALL)
        for at, handler in ((MALLOC, self.malloc), (NAME, self.name), (SCALE, self.scale),
                            (SUBSCRIBE, self.subscribe), (UNSUBSCRIBE, self.unsubscribe), (EMIT, self.emit),
                            (POOL, self.pool), (LISTED, self.listed), (STUBS + 0x40, self.ray), (STUBS + 0x80, self.hit),
                            (RAY_TEST, self.ray_direct),
                            (PIERCE_DAMAGE, self.pierce_damage), (STEP_DAMAGE, self.step_damage)):
            self.emu.hook_add(UC_HOOK_CODE, handler, begin=at, end=at)
        self.written = bytearray()
        for at in WRITERS:
            self.emu.hook_add(UC_HOOK_CODE, self.write, begin=at, end=at)

    def write(self, emu, address, *_):
        """A world writer (EAX the value, the writer and four names; ret 14h): its bytes kept."""
        self.written.extend(emu.mem_read(emu.reg_read(UC_X86_REG_EAX), WRITERS[address]))
        self.ret(0x14)

    def clip_end(self, state, clip):
        """The animation player's end event of `clip` (+4 of the event), through the state's own handler."""
        event = DATA + 0xE1000
        self.emu.mem_write(event, bytes(0x40))
        self.word(event + 4, clip)
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, event)
        self.run(CLIP_END, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: state})
        assert self.stopped == hide.SENTINEL

    def call(self, at, ecx, args=(), stop=hide.SENTINEL, regs=None):
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        self.emitted, self.channels = [], []
        out, _ = self.run(at, (stop,), sp, {**hide.GAME, UC_X86_REG_ECX: ecx, **(regs or {})})
        assert self.stopped == stop, hex(self.stopped or 0)
        return out

    def ret(self, pops, eax=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def arg(self, i):
        return self.read(self.emu.reg_read(UC_X86_REG_ESP) + 4 * i)

    def malloc(self, emu, *_):
        size = emu.reg_read(UC_X86_REG_EAX)
        at = self.next
        self.next = (at + size + 15) & ~15
        self.allocated.append((size, self.arg(1), emu.reg_read(UC_X86_REG_ESI), at))
        self.ret(4, at)

    def name(self, emu, *_):
        self.ret(0, emu.reg_read(UC_X86_REG_ECX))

    def scale(self, emu, *_):
        self.times.append(struct.unpack("<f", struct.pack("<I", self.arg(1)))[0])
        emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", 2.5))[0])
        self.ret(0)

    def subscribe(self, emu, *_):
        self.channels.append(("subscribe", tuple(self.arg(i) for i in range(1, 6))))
        self.ret(0x14)

    def unsubscribe(self, emu, *_):
        self.channels.append(("unsubscribe", self.arg(2)))
        self.ret(8)

    def emit(self, emu, *_):
        """emit_bullet's entry: its EDI and arguments kept, and what the gun
        holds then (its user, its clip set's posture); the game's own code
        goes on (the bullet's constructor, 0x32BA60)."""
        args = [self.arg(i) for i in range(1, 9)]
        point = struct.unpack("<3f", self.read(args[1], 12))
        velocity = struct.unpack("<3f", self.read(args[2], 12))
        gun = emu.reg_read(UC_X86_REG_EDI)
        self.emitted.append((gun, args[0], args[1], point, velocity, *args[3:]))

    def pool(self, emu, *_):
        self.emu.mem_write(BULLET, bytes(0x100))
        self.ret(0, BULLET)

    def ray(self, emu, *_):
        """The physics world's ray test (+48h, thiscall, 8 arguments): what
        the case says the ray meets, in the result it was given (28h bytes:
        the point, the normal, +1Ch the collision object), EAX the result.
        With a scene: the nearest object whose box on the battery's front
        (right and up spans, STRIKE_FACE ahead of the eye) the ray crosses;
        else `meets`: the body."""
        result = self.arg(1)
        origin = struct.unpack("<3f", self.read(self.arg(2), 12))
        direction = struct.unpack("<3f", self.read(self.arg(3), 12))
        self.rays.append(origin)
        self.emu.mem_write(result, bytes(0x28))
        met = None
        if self.scene is None:
            met = (COLLISION, 0.01) if self.meets else None
        else:
            dot = lambda axis: sum(a * b for a, b in zip(axis, direction))
            right, up = (dot(RIGHT) / dot(FORWARD) * STRIKE_FACE, dot(UP) / dot(FORWARD) * STRIKE_FACE)
            for thing, (left, right_edge), (low, high), distance in self.scene:
                if left <= right <= right_edge and low <= up <= high and (met is None or distance < met[1]):
                    met = (thing, distance)
        if met is not None:
            self.floats(result, [origin[i] + direction[i] * met[1] for i in range(3)])
            self.floats(result + 0x10, [0.0, 0.0, -1.0])
            self.word(result + 0x1C, met[0])
        emu.reg_write(UC_X86_REG_EAX, result)

    def ray_direct(self, emu, *_):
        """ray_test 0x75E20 called directly (the strike's aim): as the stub,
        then its ret 20h; what it was asked: (origin, length, group, mask,
        the requester's +8, unflipped)."""
        requester = self.arg(7)
        self.aims.append((struct.unpack("<3f", self.read(self.arg(2), 12)),
                          struct.unpack("<f", struct.pack("<I", self.arg(4)))[0], self.arg(5), self.arg(6),
                          requester and self.read(requester + 8), self.arg(8) & 0xFF))
        self.ray(emu)
        self.ret(0x20, self.arg(1))

    def pierce_damage(self, emu, *_):
        self.reads.append(("pierce", emu.reg_read(UC_X86_REG_ECX)))

    def step_damage(self, emu, *_):
        self.reads.append(("step", emu.reg_read(UC_X86_REG_ECX)))

    def hit(self, emu, *_):
        """The struck target's hit (+24h, thiscall, 10 arguments), as
        try_pierce calls it: its arguments kept."""
        self.hits.append(tuple(self.arg(i) for i in range(1, 11)))

    def listed(self, emu, *_):
        bullet = self.read(self.arg(1))
        self.bullets.append({o: self.read(bullet + o) for o in (0xC, 0x40, 0x44, 0x48, 0x58, 0x70)})
        self.ret(4)

    def floats(self, at, values):
        self.emu.mem_write(at, struct.pack(f"<{len(values)}f", *values))

    def made(self):
        """A battery as initialize_logic makes it, carried by PLAYER with two
        guns; the machine's current state the carry."""
        self.emu.mem_write(DATA, bytes(SIZE))
        self.next, self.allocated, self.times, self.channels, self.emitted = HEAP, [], [], [], []
        self.bullets, self.rays, self.hits, self.meets, self.reads = [], [], [], False, []
        self.scene, self.aims = None, []
        # the physics world: its ray test (a hook of ours at STUBS+40h, then
        # ret 20h), the level's bounds (a box of 10 km); a body: its
        # collision object (a hit model's: the game's vtable, its material
        # +38h actor, which the vtable's +10h 0x70C00 reads), its game
        # object (+0Ch; its +0 casts to the target), the target (its hit
        # +24h a hook of ours at STUBS+80h, then ret 28h; +8h: pierceable)
        stubs = {0x40: b"\xC2\x20\x00", 0x60: b"\xB8" + struct.pack("<I", BOUNDS) + b"\xC2\x04\x00",
                 0x80: b"\xC2\x28\x00", 0xA0: b"\x66\xB8\xFF\xFF\xC2\x08\x00",
                 0xC0: b"\xB8" + struct.pack("<I", TARGET) + b"\xC3", 0xE0: b"\xB8\x01\x00\x00\x00\xC3"}
        for at, code in stubs.items():
            self.emu.mem_write(STUBS + at, code)
        self.word(MANAGER + 0x98, PHYS)
        self.word(PHYS, PHYS + 0x100)
        self.word(PHYS + 0x100 + 0x48, STUBS + 0x40)
        self.word(PHYS + 0x100 + 0x64, STUBS + 0x60)
        self.floats(BOUNDS, [-1e4, -1e4, -1e4, 1e4, 1e4, 1e4])
        self.word(COLLISION, ANIMATED_BODY_VTABLE)
        self.emu.mem_write(COLLISION + 0x38, struct.pack("<H", ACTOR))
        self.word(WALL, WALL + 0x80)
        self.word(WALL + 0x80 + 0x10, STUBS + 0xA0)
        self.word(COLLISION + 0xC, GAME_OBJECT)
        self.word(GAME_OBJECT, GAME_OBJECT + 0x80)
        self.word(GAME_OBJECT + 0x80, STUBS + 0xC0)
        self.word(TARGET, TARGET + 0x80)
        self.word(TARGET + 0x80 + 0x24, STUBS + 0x80)
        self.word(TARGET + 0x80 + 0x8, STUBS + 0xE0)
        self.word(0x1F1D9C4, ALLOCATOR)
        self.word(0x1F1DA44, AI_ALLOCATOR)
        self.word(BATTERY, BATTERY_VTABLE)
        self.word(BATTERY + 8, PLAYER)
        self.emu.mem_write(STUBS, b"\xB8" + struct.pack("<I", IDENTITY) + b"\xC2\x04\x00")
        self.word(BATTERY_VTABLE + 0x24, STUBS)
        self.word(BATTERY_VTABLE + 0xA4, STUBS)
        self.floats(IDENTITY, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])
        for state in range(CONFIG_STATES):
            for i in range(6):
                clip = CLIPS + 0x1000 + 0x100 * (6 * state + i)
                self.word(clip + 0xDC, 100)
                self.word(CLIPS + 0x18 * state + 4 * i, clip)
                self.word(QUERIES + 0x128 + 0x2E0 * (2 + 6 * state + i), clip)
        self.word(QUERIES + 0x38, 2 + 6 * CONFIG_STATES)    # the config, the skeleton, every state's clips
        # the carrier: alive, holding the battery, at 1000 ms; its eye at
        # (1, 2, 3) looking along FORWARD; its guns in slots 7 and 10
        self.emu.mem_write(PLAYER + 0x33C, b"\x01")
        self.word(PLAYER + 0x128, BATTERY)
        self.word(PLAYER + 0x12C, BATTERY)
        self.word(PLAYER + 0x324, 1000)
        self.floats(PLAYER + 0x44C, [*RIGHT, 0, *UP, 0, *FORWARD, 0, 1, 2, 3, 1])
        self.word(PLAYER + 0x10C, INVENTORY)
        self.word(INVENTORY + 0x174, PLAYER + 0x108)  # the inventory's holder: the player's +108h interface
        self.word(PLAYER + 0x128, BATTERY)
        # its battery's clip set's posture now (kind 1, +24h 1); the guns
        # deactivated as a carrier's are (weapon_core's deactivate 0x2E2EE0:
        # no user, their clip sets no posture), their melee data's damage 1.5;
        # emit_bullet's pool with room (+30h below +34h), the manager's world
        # (+94h, the bullet's +48h from its +108h), the carrier's profile
        self.word(BATTERY + 0x1AC, BATTERY_ANIMS)
        self.word(BATTERY_ANIMS + 0x20, POSTURE)
        self.word(POSTURE + 0x20, 1)
        self.word(POSTURE + 0x24, 1)
        self.word(MANAGER + 0x80, MANAGER + 0x400)
        self.word(MANAGER + 0x400 + 0x34, 1)
        self.word(MANAGER + 0x94, OWNER_WORLD)
        self.word(OWNER_WORLD + 0x108, MATERIAL)     # the materials: every one the same made-up one
        self.word(OWNER_WORLD + 0x10308, MATERIAL)
        self.word(PLAYER + 0x6B8, PROFILE)
        for gun, slot in ((GUN7, 7), (GUN10, 10)):
            self.word(gun, WEAPON_VTABLE)
            self.word(gun + 0x5A0, MANAGER)
            self.word(gun + 0x278, GUN_ANIMS + (0x100 if gun == GUN10 else 0))
            self.floats(gun + 0x53C, [1.7])          # melee/data: its distance (+0), its damage (+8)
            self.floats(gun + 0x53C + 8, [1.5])
            self.floats(gun + 0x578, [1.7 if gun == GUN7 else 1.5])
            self.word(INVENTORY + 0x10C + 4 * slot, gun + 0x10)
            self.word(gun + 0x10 + 0x250, INVENTORY)  # its item's inventory, as the kit's build sets it
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        for i, value in enumerate((CLIPS, struct.unpack("<I", struct.pack("<f", 0.5))[0], CLIPS + 0x18, CLIPS + 0x30,
                                   struct.unpack("<I", struct.pack("<f", 0.6))[0])):
            self.word(sp + 4 + 4 * i, value)
        self.run(INITIALIZE_LOGIC, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_EAX: BATTERY})
        assert self.stopped == hide.SENTINEL, hex(self.stopped or 0)
        self.take, self.carry, self.put = self.states()[:3]
        for state in (self.take, self.carry, self.put):
            self.word(state + 0xC4, 1000)    # executed at this tick already: no carried transform
        self.word(BATTERY + 0x178, self.carry)

    def states(self):
        out, at = [], self.read(BATTERY + 0x170)
        while at:
            out.append(at)
            at = self.read(at + 4)
        return out

    def transitions(self, state):
        out, at = [], self.read(state + 0x10)
        while at:
            out.append((self.read(at + 0x20), self.read(at, 0x20), at))
            at = self.read(at + 0x24)
        return out

    def site(self, states=CONFIG_STATES):
        """The cook callback's site: ESI the battery, ESP+28h the put's clips,
        [EBP+8] the queries_result (with the config's first `states` states: the
        content's 3 to 6 too, or not)."""
        sp = hide.STACK + 0x9000
        self.word(sp + 0x108, QUERIES)
        self.word(QUERIES + 0x38, 2 + 6 * states)
        dummy = DATA + 0xE0000
        self.word(dummy, 0x7E57)
        self.word(sp + 0x20, dummy)
        self.emu.mem_write(sp + 0x28, self.read(CLIPS + 0x30, 0x18))
        self.times, self.allocated = [], []
        regs, after = self.run(SITE, (SITE_AFTER,), sp, {**hide.GAME, UC_X86_REG_ESI: BATTERY, UC_X86_REG_EBP: sp + 0x100})
        assert self.stopped == SITE_AFTER and after == sp, hex(self.stopped or 0)
        assert regs[UC_X86_REG_ECX] == dummy and regs[UC_X86_REG_EAX] == 0x7E57, "the replayed moves"
        assert regs[UC_X86_REG_ESI] == BATTERY

    def tick(self):
        """fsm::tick on the battery's machine: the state it is in after."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, FSM)
        self.emitted, self.channels = [], []
        _, after = self.run(TICK, (hide.SENTINEL,), sp, hide.GAME)
        assert self.stopped == hide.SENTINEL and after == sp + 8, hex(self.stopped or 0)
        return self.read(BATTERY + 0x178)

    def finalize(self, state):
        """The state's finalize through its vtable (+0Ch), as a drop calls it."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.emitted, self.channels = [], []
        self.run(self.read(self.read(state) + 0xC), (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: state})
        assert self.stopped == hide.SENTINEL

    def actions(self, bits):
        self.word(PLAYER + 0x330, bits)


def check(ok, what):
    if not ok:
        sys.exit(f"FAIL {what}")
    print(f"  ok {what}")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))

    print("the cook's site (battery.logic 0x2FA7E9)")
    for r in (old, new):
        r.made()
        r.site()
    check(len(old.states()) == 3 and old.read(FSM) == 3, "the original: take, carry, put")
    states = new.states()
    check(len(states) == 8 and new.read(FSM) == 8 and states[:3] == [new.take, new.carry, new.put]
          and new.read(BATTERY + 0x174) == states[7] and new.read(states[7] + 4) == 0,
          "the patched: take, carry, put, then five more, the last the list's tail")
    swing, recover = states[3], states[4]
    new.swing = swing
    made = [(size, name, allocator) for size, name, allocator, _ in new.allocated]
    at = [state for _, _, _, state in new.allocated[:5]]
    take_state, carry_state = (0xEC, TAKE_NAME, ALLOCATOR), (0xE0, CARRY_NAME, ALLOCATOR)
    check(made[:5] == [take_state, take_state, carry_state, take_state, take_state]
          and at == [states[i] for i in (3, 4, 5, 6, 7)] and len(made) == 5 + 11
          and all(size == 0x28 and allocator == AI_ALLOCATOR for size, _, allocator in made[5:]),
          "of the game's allocator: each strike's swing and recovery (0ECh bytes, the take's allocation name) and, "
          "before the shove's, a carry (0E0h, the carry's name); 11 transitions of the ai arena's")
    times = []
    for (name, swing_s, recover_s), (carry_at, swing_at, recover_at), first in zip(STRIKES, PLACES, (3, 5)):
        for state, index in ((states[swing_at], first), (states[recover_at], first + 1)):
            clips = [new.read(CLIPS + 0x18 * index + 4 * i) for i in range(6)]
            stored = [new.read(state + o) for o in (0xE0, 0xE4, 0xD0, 0xD8, 0xD4, 0xDC)]
            check(stored == clips and new.read(state + 0x20) == 3 and new.read(state + 0x1C) == BATTERY
                  and new.read(state + 0x28, 1) == b"\0" and new.read(state) == new.read(swing),
                  f"the {name}'s config state {index}: a take state over the cook's six clips of it (results "
                  f"{2 + 6 * index}..{7 + 6 * index}), of the carry's kind (3), its battery, not ended, the patch's vtable copy")
        times += [f32(swing_s), f32(recover_s)]
        if carry_at != 1:
            state = states[carry_at]
            clips = [new.read(CLIPS + 0x18 + 4 * i) for i in range(6)]
            stored = [new.read(state + o) for o in (0xD8, 0xDC, 0xC8, 0xD0, 0xCC, 0xD4)]
            check(new.read(state) == CARRY_VTABLE and stored == clips and new.read(state + 0x20) == 3
                  and new.read(state + 0x1C) == BATTERY and stored == [new.read(new.carry + o) for o in (0xD8, 0xDC, 0xC8, 0xD0, 0xCC, 0xD4)],
                  f"the {name}'s carry: a carry state of the game's (its constructor's vtable 0xA1BFB8, kind 3) over the "
                  "carry's six clips, as the machine's own")
    check(new.times == times and new.read(swing + 0xE8) == 0x40200000,
          "scaled to their times: 0.2 s to the strike and 0.35 s back, the ram's and the shove's")
    vtable = new.read(swing)
    ours = [new.read(vtable - 4 + 4 * i) for i in range(13)]
    stock = [new.read(TAKE_VTABLE - 4 + 4 * i) for i in range(13)]
    mine = [i for i in range(13) if ours[i] != stock[i]]
    check(vtable != TAKE_VTABLE and mine == [4, 9] and all(ours[i] >= 0xA42D000 for i in mine),
          "its vtable: the take's RTTI and slots, finalize (+0Ch) and may-be-put-away (+20h) ours in the arena")
    carry = new.transitions(new.carry)
    take = new.transitions(new.take)
    check([d for d, _, _ in carry] == [new.put, swing] and [d for d, _, _ in take] == [new.carry],
          "the carry's transitions: put, then the ram's swing")
    wants = carry[1][1]
    manager, _, function, battery = struct.unpack_from("<4I", wants)
    check(manager == FUNCTION_PREDICATE | 1 and function >= 0xA42D000 and battery == BATTERY,
          "carry -> swing: a function-pointer functor (0xA13CAC | 1), its function ours, the battery its buffer's")
    for i, ((name, _, _), (carry_at, swing_at, recover_at)) in enumerate(zip(STRIKES, PLACES)):
        following = states[PLACES[(i + 1) % len(PLACES)][0]]
        out = new.transitions(states[carry_at])
        check([d for d, _, _ in out] == [new.put, states[swing_at]] and out[0][1] == carry[0][1] and out[1][1] == wants,
              f"the {name}'s carry: put (carry -> put's functor), then its swing (ours)")
        out = new.transitions(states[swing_at])
        check([d for d, _, _ in out] == [new.put, states[recover_at]] and out[0][1] == carry[0][1] and out[1][1] == take[0][1],
              f"the {name}'s swing: put, then its recovery (take -> carry's functor)")
        out = new.transitions(states[recover_at])
        check([d for d, _, _ in out] == [new.put, following] and out[0][1] == carry[0][1] and out[1][1] == take[0][1],
              f"the {name}'s recovery: put, then the next strike's carry" + (" (the first again)" if following == new.carry else ""))
    for count, what in ((3, "without the content's states"), (5, "with the ram's alone (an older content)")):
        probe = Run(bytes(built))
        probe.made()
        try:
            probe.site(states=count)
            crashed = None
        except UcError:
            crashed = probe.emu.reg_read(UC_X86_REG_EIP)
        check(crashed is not None and crashed >= 0xA42D000 and len(probe.states()) == 3,
              f"results {what}: the patch's own crash, the machine untouched (a game without them would play otherwise)")

    print("fsm::tick from the carry (0x38CA20)")
    for r in (old, new):
        r.actions(MELEE)
    check(old.tick() == old.carry, "the original: the key down, the carry stays")
    for bits, alive, want in ((0, 1, "carry"), (1 << 26, 1, "carry"), (MELEE, 0, "carry"), (MELEE, 1, "swing"),
                              (MELEE | 0x41, 1, "swing")):
        new.made()
        new.site()
        new.actions(bits)
        new.emu.mem_write(PLAYER + 0x33C, bytes([alive]))
        state = new.tick()
        check(state == (new.states()[3] if want == "swing" else new.carry) and new.emitted == [],
              f"actions {bits:#x}, alive {alive}: the {want}, no strike")
    for motion, want in ((4, "carry"), (3, "swing"), (2, "swing")):
        new.made()
        new.site()
        new.actions(MELEE)
        new.word(POSTURE + 0x24, motion)
        state = new.tick()
        check(state == (new.states()[3] if want == "swing" else new.carry),
              f"the key down, the carrier's posture of motion {motion} ({ {4: 'a jump', 3: 'sprint', 2: 'run'}[motion] }): "
              f"the {want} (the gun-stock hit's rule, melee_pred 0x2E4100)")
    new.made()
    new.site()
    new.actions(MELEE)
    new.tick()
    check(any(kind == "subscribe" and new.states()[3] in args for kind, args in new.channels)
          and new.read(new.states()[3] + 0x28, 1) == b"\0",
          "the swing waits for its clip's end (subscribed, not ended)")
    new.made()
    new.site()
    new.actions(MELEE)
    new.word(PLAYER + 0x12C, GUN7 + 0x10)
    check(new.tick() == new.put, "a switch away (next item another), the key down: the put first")

    print("fsm::tick in the swing")
    eye = (1.0, 2.0, 3.0)

    def swinging(strike=0):
        """In a strike's swing (the ram's, or the strike of that turn from its own carry), the key up again."""
        new.made()
        new.site()
        new.actions(MELEE)
        carry_at, swing_at, _ = PLACES[strike]
        new.word(BATTERY + 0x178, new.states()[carry_at])
        new.word(new.states()[carry_at] + 0xC4, 1000)
        swing = new.states()[swing_at]
        assert new.tick() == swing
        new.actions(0)
        return swing

    # what 0x2E43A0 gives a held gun (its user the carrier, its posture
    # the battery's), run in the original exe: the stance to expect
    old.made()
    old.word(GUN7 + 8, PLAYER)
    old.word(GUN_ANIMS + 0x20, POSTURE)
    sp = hide.STACK + 0x9000
    old.word(sp, hide.SENTINEL)
    regs, _ = old.run(0x2E43A0, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: GUN7})
    stance = regs[UC_X86_REG_EAX]
    swing = swinging()
    gun7_before = new.read(GUN7, 0x600)
    check(new.tick() == swing and new.emitted == [], "before its clip ended: nothing")
    # the clip it waits for: the one its expression plays (+0C8h's, index
    # +0CCh: 0 the first person's), as 0x338B6B sets it
    new.emu.mem_write(swing + 0xCC, b"\x00")
    clip = new.read(new.read(swing + 0xC8))
    new.clip_end(swing, clip + 0x100)
    check(new.read(swing + 0x28, 1) == b"\0" and new.tick() == swing,
          "another clip's end (the event 0x344830): still the swing")
    new.clip_end(swing, clip)
    check(new.read(swing + 0x28, 1) == b"\x01", "its own clip's end (the event 0x344830): ended")
    recover = new.states()[4]
    check(new.tick() == recover, "then fsm::tick: the recovery, the strike through fsm::tick")
    gun = (GUN7, MANAGER, PLAYER + 0x47C, eye, (0.0, f32(f32(0.6) * f32(1.7)), f32(f32(0.8) * f32(1.7))), GUN7 + 0x53C, 1000,
           PLAYER + 0x110, PLAYER + 0x118, 0)
    check(new.emitted == [gun], "one strike: the slot-7 gun's melee bullet from the eye along its forward, now")
    check(len(new.bullets) == 1 and new.bullets[0][0xC] == GUN7 + 0x10 and new.bullets[0][0x40] == GUN7 + 0x53C
          and new.bullets[0][0x48] == MATERIAL
          and new.bullets[0][0x70] == struct.unpack("<I", struct.pack("<f", 1.5))[0],
          "the game's own emit_bullet and bullet constructor (0x32BA60) make it: its gun the source, the melee data, "
          "the gun-stock damage (1.5 through the kit's melee modifier)")
    check(new.read(GUN7 + 8) == 0 and new.read(GUN_ANIMS + 0x20) == 0 and new.read(GUN7, 0x600) == gun7_before,
          "the gun is never written: still no user, no posture")
    check(new.bullets[0][0x58] == stance,
          "its stance (bullet +58h) the one the game gives a held gun in the carrier's posture")
    check(("unsubscribe", swing) in new.channels, "the swing's own finalize ran (its channel left)")
    new.emu.mem_write(recover + 0xCC, b"\x00")
    new.clip_end(recover, new.read(new.read(recover + 0xC8)))
    check(new.tick() == new.states()[5] and new.emitted == [],
          "the recovery's clip ended: the carry before the shove, no second strike")

    print("the strikes in turn")
    new.made()
    new.site()
    states = new.states()
    new.actions(MELEE)
    for name, (carry_at, swing_at, recover_at), following in (("ram", PLACES[0], 5), ("shove", PLACES[1], 1), ("ram again", PLACES[0], 5)):
        went = [new.read(BATTERY + 0x178) == states[carry_at], new.tick() == states[swing_at], new.emitted == []]
        went.append(new.tick() == states[swing_at] and new.emitted == [])
        new.emu.mem_write(states[swing_at] + 0xCC, b"\x00")
        new.clip_end(states[swing_at], new.read(new.read(states[swing_at] + 0xC8)))
        went += [new.tick() == states[recover_at], len(new.emitted) == 1]
        new.emu.mem_write(states[recover_at] + 0xCC, b"\x00")
        new.clip_end(states[recover_at], new.read(new.read(states[recover_at] + 0xC8)))
        went += [new.tick() == states[following], new.emitted == []]
        check(all(went), f"the key held, the {name}: from its carry (state {carry_at}) its swing ({swing_at}), one strike at "
                         f"its clip's end, its recovery ({recover_at}), then the carry of the next ({following})")

    print("the bullet's flight and hit (bullet::tick 0x32C180, the game's own)")
    struck = new.read(BULLET, 0x100)

    def flight(r, meets, stops=(hide.SENTINEL,)):
        r.emu.mem_write(BULLET, struck)
        r.meets, r.rays, r.hits, r.reads = meets, [], [], []
        sp = hide.STACK + 0x9000
        r.word(sp, hide.SENTINEL)
        r.word(sp + 4, BULLET)
        try:
            r.run(BULLET_TICK, stops, sp, {**hide.GAME, UC_X86_REG_EAX: 1050})
            return r.stopped
        except UcError:
            return ("fault", r.emu.reg_read(UC_X86_REG_EIP))

    end = flight(new, False)
    check(end == hide.SENTINEL and new.rays and ("step", GUN7) in new.reads and new.read(GUN7, 0x600) == gun7_before,
          "a tick in which it meets nothing: its steps (update_bullet_position 0x32EE90, its melee damage "
          "0x2E1CF0 and 0x2E1C80 through the gun) run to the tick's end; the gun never written")
    end = flight(new, True, (hide.SENTINEL, AFTER_PIERCE))
    check(end == AFTER_PIERCE and len(new.hits) == 1 and new.hits[0][1] == PLAYER + 0x110
          and new.hits[0][9] == GUN7 + 0x10 and ("pierce", GUN7) in new.reads and ("step", GUN7) in new.reads
          and new.read(GUN7, 0x600) == gun7_before,
          "meeting a body: check_collision 0x32D4E0, its step to the point and try_pierce 0x32D960 hit the target "
          "once (the carrier its shooter, the gun's item the source) and read the gun's melee damage again "
          "(0x32DA5A); the gun never written")

    print("the strike's aim over where its battery lands (battery.rs Strike::grid: the game's ray test from the eye)")

    def aimed(scene, strike=0):
        """A strike (the ram, or the strike of that turn) in `scene` and its
        bullet's first tick there: (bullets made, its velocity, its start
        velocity, the targets hit)."""
        swing = swinging(strike)
        new.scene = scene
        new.emu.mem_write(swing + 0x28, b"\x01")
        assert new.tick() == new.states()[PLACES[strike][2]]
        made = len(new.emitted)
        velocity = struct.unpack("<3f", new.read(BULLET + 0x1C, 12))
        start = struct.unpack("<3f", new.read(BULLET + 0x34, 12))
        new.hits = []
        sp = hide.STACK + 0x9000
        new.word(sp, hide.SENTINEL)
        new.word(sp + 4, BULLET)
        new.run(BULLET_TICK, (hide.SENTINEL, AFTER_PIERCE), sp, {**hide.GAME, UC_X86_REG_EAX: 1050})
        return made, velocity, start, len(new.hits)

    def toward(x, y):
        """The bullet's velocity along the ray through (x right, y up) on the battery's front."""
        ray = [FORWARD[i] * STRIKE_FACE + RIGHT[i] * x + UP[i] * y for i in range(3)]
        length = sum(v * v for v in ray) ** 0.5
        return [v / length * 1.7 for v in ray]

    def near(a, b):
        return all(abs(p - q) < 1e-4 for p, q in zip(a, b))

    TALL = (-1.0, 1.0)
    made, velocity, start, hits = aimed([(COLLISION, (0.36, 0.6), TALL, 0.9)])
    check(len(new.aims) == 15 and all(aim == ((1.0, 2.0, 3.0), f32(1.7), 0x10, 0x48, PLAYER + 0x110, 1) for aim in new.aims),
          "nothing on the battery's face: its 15 rays, each the game's ray test as check_collision's (from the eye, the "
          "melee distance 1.7 m, group 10h, mask 48h, unflipped), its requester's shooter the carrier's (+110h: his "
          "own body skipped, 0x2DA5E3)")
    centre = toward(0.0, 0.0)
    made, velocity, start, hits = aimed([(COLLISION, (0.22, 0.4), TALL, 0.9)])
    check(made == 1 and near(velocity, toward(0.3, 0.0)) and near(start, velocity) and hits == 1,
          "a body off the look, within the battery's width (0.22..0.4 m right on its front): the one bullet along the "
          "grid's first ray to it (0.3 m right on the look's row), and it hits once")
    made, velocity, start, hits = aimed([(COLLISION, (-0.1, 0.1), (-0.4, -0.1), 0.9)])
    check(made == 1 and near(velocity, toward(0.0, -0.175)) and hits == 1,
          "a body below the look at the battery's height (0.1..0.4 m down): along the ray 0.175 m down, one hit")
    made, velocity, start, hits = aimed([(COLLISION, (0.36, 0.6), TALL, 0.9)])
    check(made == 1 and near(velocity, centre) and near(start, centre) and hits == 0,
          "a body just past the battery's width (0.36..0.6 m right): along the look, as the gun-stock hit, no hit")
    made, velocity, start, hits = aimed([(COLLISION, (-0.1, 0.1), (-0.6, -0.4), 0.9)])
    check(made == 1 and near(velocity, centre) and hits == 0,
          "a body just under the battery's lower edge (0.4..0.6 m down): along the look, no hit")
    made, velocity, start, hits = aimed([(COLLISION, (0.22, 0.4), TALL, 0.9), (WALL, (0.1, 0.5), TALL, 0.5)])
    check(made == 1 and near(velocity, centre) and hits == 0,
          "the same body behind a wall (0.1..0.5 m right, 0.5 m ahead): the wall is each such ray's first hit, along "
          "the look, no hit")
    made, velocity, start, hits = aimed([(COLLISION, (-0.2, 0.2), TALL, 0.9)])
    check(made == 1 and near(velocity, centre) and hits == 1,
          "a body in front that 9 of the rays meet (0.2 m each side): one bullet, along the look (the first ray), "
          "one hit")
    made, velocity, start, hits = aimed([(COLLISION, (0.36, 0.6), TALL, 0.9)], strike=1)
    check(made == 1 and near(velocity, toward(0.45, 0.0)) and hits == 1,
          "the shove, the body just past the ram's width (0.36..0.6 m right): along its grid's own column 0.45 m right, "
          "one hit (its right end lands there)")
    made, velocity, start, hits = aimed([(COLLISION, (-0.4, -0.2), TALL, 0.9)], strike=1)
    check(made == 1 and near(velocity, centre) and hits == 0,
          "the shove, a body 0.2..0.4 m left (which the ram's column 0.3 m left meets): along the look, no hit")
    swing = swinging()
    new.word(INVENTORY + 0x10C + 4 * 7, 0)
    new.emu.mem_write(swing + 0x28, b"\x01")
    check(new.tick() == new.states()[4] and len(new.emitted) == 1 and new.emitted[0][0] == GUN10
          and new.emitted[0][5] == GUN10 + 0x53C and new.emitted[0][4][2] == f32(f32(0.8) * f32(1.5)),
          "slot 7 empty: slot 10's gun")
    swing = swinging()
    new.word(PLAYER + 0x12C, GUN7 + 0x10)
    new.emu.mem_write(swing + 0x28, b"\x01")
    check(new.tick() == new.put and new.emitted == [gun], "a switch away at its end: the put, the strike made")

    print("the swing left early")
    swing = swinging()
    new.finalize(swing)
    check(new.emitted == [] and ("unsubscribe", swing) in new.channels, "before its clip ended (a drop): no strike")
    swing = swinging()
    new.word(INVENTORY + 0x10C + 4 * 7, 0)
    new.word(INVENTORY + 0x10C + 4 * 10, 0)
    new.emu.mem_write(swing + 0x28, b"\x01")
    try:
        new.finalize(swing)
        crashed = None
    except UcError:
        crashed = new.emu.reg_read(UC_X86_REG_EIP)
    check(new.emitted == [] and crashed is not None and crashed >= 0xA42D000,
          "no gun in slot 7 or 10 (never in a match): the patch's own crash in the arena, not a skipped strike")

    print("a strike whose gun has no player (no inventory holder): the patch's own crash, named")
    swing = swinging()
    new.word(INVENTORY + 0x174, 0)
    new.emu.mem_write(swing + 0x28, b"\x01")
    try:
        new.tick()
        crashed = None
    except UcError:
        crashed = new.emu.reg_read(UC_X86_REG_EIP)
    check(crashed is not None and crashed >= 0xA42D000, "the substitution's panic in the arena, not a fault in the game's code")

    print("put away mid-swing (victory_item_core::deactivate 0x300F40, to past its state's finalize)")
    for alive, ended, want in ((1, 0, False), (1, 1, True), (0, 1, False), (0, 0, False)):
        swing = swinging()
        new.emu.mem_write(swing + 0x28, bytes([ended]))
        new.emu.mem_write(PLAYER + 0x33C, bytes([alive]))
        new.call(DEACTIVATE, BATTERY, (0,), DEACTIVATED)
        check(new.read(BATTERY + 0x178) == 0 and (len(new.emitted) == 1) == want
              and ("unsubscribe", swing) in new.channels,
              f"carrier {'alive' if alive else 'dead'}, clip {'ended' if ended else 'playing'}: its finalize runs, "
              f"{'the strike' if want else 'no strike'}")
    swing = swinging()
    new.emu.mem_write(POSTURE + 0x2B, b"\x01")      # the clip set's own readiness (0x30128D)
    ready = lambda: new.call(PUT_AWAY, BATTERY)[UC_X86_REG_EAX] & 0xFF
    alive_ready = ready()
    new.emu.mem_write(PLAYER + 0x33C, b"\0")
    check(alive_ready == 0 and ready() == 1,
          "is_ready_to_be_deactivated 0x301270 in the swing: no for a living carrier (he leaves through the put), "
          "yes for a dead one, as from the carry: a dead carrier's swing put away through victory_item_core::deactivate")

    print("a world saved and loaded mid-strike (fsm::serialize 0x38CC90 / deserialize 0x38CCF0, the states' own)")
    for what, index, strike in (("the ram's swing", 3, 0), ("the shove's swing", 6, 1), ("the carry before the shove", 5, None)):
        if strike is None:
            new.made()
            new.site()
            state = new.states()[index]
            new.word(BATTERY + 0x178, state)
            layout = lambda at: new.read(at + 0x28, 1) + new.read(at + 0x2C, 0xC8 - 0x2C)
        else:
            state = swinging(strike)
            new.emu.mem_write(state + 0xCC, b"\x02")
            layout = lambda at: new.read(at + 0x28, 1) + new.read(at + 0x2C, 0xC8 - 0x2C) + new.read(at + 0xCC, 1)
        new.floats(state + 0xAC, [0.5, -1.5, 2.5])      # something of its own to carry over: its last speeds
        fields = layout(state)
        new.written = bytearray()
        new.call(SERIALIZE, 0, (0,), regs={UC_X86_REG_ESI: FSM, UC_X86_REG_EDI: DATA + 0xE2000})
        stream = bytes(new.written)
        check(stream[0] == index, f"{what}: the world holds the machine's index {index} (then its state's fields)")
        new.made()
        new.site()
        reader = DATA + 0xE3000
        new.emu.mem_write(DATA + 0xE4000, stream)
        new.word(reader + 4, DATA + 0xE4000)
        new.call(DESERIALIZE, 0, (0, 0), regs={UC_X86_REG_ESI: FSM, UC_X86_REG_EDI: reader})
        loaded = new.read(BATTERY + 0x178)
        check(loaded == new.states()[index] and new.read(reader + 4) == DATA + 0xE4000 + len(stream) and layout(loaded) == fields,
              "another game loading it: its own state of that index current, every field the saver's, the stream read "
              "to its end")

    print("the kill feed's weapon icon (kill_feed.weapon_icon 0x2DA3CB: generate_killed_event writing the event)")
    node, round_data = DATA + 0xE6000, DATA + 0xE7000

    def kill_icon(r, data, held):
        """generate_killed_event's write of the event's killer (5) and weapon
        icon (48, what it took from the bullet's data) for a bullet of
        GUN7's item with `data` (None: no bullet), its player holding `held`:
        (killer, icon) of the event."""
        r.made()
        r.word(PLAYER + 0x128, held)
        r.emu.mem_write(node, bytes(0x28))
        r.emu.mem_write(BULLET, bytes(0x100))
        r.word(BULLET + 0xC, GUN7 + 0x10)
        r.word(BULLET + 0x40, data or 0)
        r.run(0x2DA3CB, (0x2DA3D1,), hide.STACK + 0x9000, {**hide.GAME, UC_X86_REG_EAX: node, UC_X86_REG_ECX: 5,
                                                          UC_X86_REG_EDX: 48, UC_X86_REG_EBP: BULLET if data else 0})
        return tuple(r.read(node + 1, 2))

    check(kill_icon(new, GUN7 + 0x53C, BATTERY) == (5, 200) and kill_icon(old, GUN7 + 0x53C, BATTERY) == (5, 48),
          "a strike's kill (its gun's melee data, its player holding the battery): the battery's icon 200 (the "
          "original: the gun stock's 48)")
    check(kill_icon(new, GUN7 + 0x53C, GUN7) == (5, 48), "a gun-stock hit's (the gun held): its icon")
    check(kill_icon(new, round_data, BATTERY) == (5, 48), "a round's, its shooter holding a battery now: its icon")
    check(kill_icon(new, None, BATTERY) == (5, 48), "no bullet: the icon the function took")

    print("the controls: the game's own code without the patch (the original exe)")
    # emit_bullet with the deactivated gun: the first build's strike
    old.made()
    sp = hide.STACK + 0x9000
    old.word(sp, hide.SENTINEL)
    for i, value in enumerate((MANAGER, PLAYER + 0x47C, PLAYER + 0x47C, GUN7 + 0x53C, 1000, PLAYER + 0x110,
                               PLAYER + 0x118, 0)):
        old.word(sp + 4 + 4 * i, value)
    try:
        old.run(EMIT, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_EDI: GUN7})
        fault = None
    except UcError:
        fault = old.emu.reg_read(UC_X86_REG_EIP)
    check(fault == 0x2E43A3, "the bullet's constructor reads the deactivated gun's missing user's +33Ch at 0x2E43A3 "
                             "(the owner's first crash, 2026-11-14)")
    # the same bullet's flight: the second build's (it lent the gun only while the bullet was made)
    check(flight(old, False) == ("fault", MELEE_DAMAGE_READ),
          "its flight's melee damage reads the deactivated gun's missing user's +115h at 0x2E1D00 "
          "(the owner's second crash, 2026-11-15)")
    print("battery_hit: all checks passed")


if __name__ == "__main__":
    main()
