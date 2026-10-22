#!/usr/bin/env python3
"""Run patch 55 path A (the Sponge, artefact type 12: sponge_a_026g.asm,
sponge_a_026g.c) in the default build on made-up cooks, inventories,
players, worlds and zones, and 0.28d0's own sponge code on the same inputs.

    python check_sponge_a_026g.py <original survarium.exe> [--v028d0 <0.28d0 survarium.exe>]
                                  [--options <artefact_sponge.options>]

Requires Python `unicorn`. The built exe's sections are loaded as they are
and its code runs as machine code; the game's own code runs where the patch
calls it: the artefact cook's construct and size switches (0x25E660,
0x25E770) and its query (0x25E800), Rattle's constructor (0x25FA10, with
artefact_base's 0x312FD0 and the config's look-ups 0xF6B10/0xF6C00), Rattle's
loader 0x2605F0, the damage type's index (0x304370), the variant's type id
(0x125790), the modifiers' getter (0x2D5AE0), find_pool (0x2E58B0),
Lifebone's regeneration callback and its invoker (0x312590, 0x312640), the
velocity setters and getters (0x3576C0.., 0x3577D0..), the matrix helpers
(0x573C0, 0x725E0, 0xEE700), the throw state's code around each hooked site,
remove_from_slot (0x2FC680) with the item's own slots, the field writers
(0x1F3DE0, 0x1F48A0, 0x301DE0, 0x81A70), the splash 0x339630 whole (its
per-bone helper 0x3393F0, its occlusion's ray query a made-up world's) and
the zone's hit call site. Intercepted (answered here): the allocator, the
physics' shape and body making, the physics world, the body's vtable, the
zone's activation, tick and switch-off, its serializable, the collision
geometry's load and vtable, the resource query (its callback then run as the
resource system would), the cook's finish, a buffer's resize and the scene's
model calls. With --v028d0 (the file as shipped: its x86 BCJ filter is undone
here) 0.28d0's action 0x514580, its passive's callback 0x514380, its throw
0x514620 with its launch 0x5146E0 and its splash's damage (0x5412AE..0x5412DC)
run on the same numbers, and must agree. --options checks the config this
script writes against the 0.28d0 file. What each case checks is printed.
"""
import argparse
import json
import lzma
import math
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_XMM0

import patch_026g as base

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "scripts", "research-mode"))
import bcw  # noqa: E402

SYMBOLS = {name: int(va, 16) for name, va in json.load(open(os.path.join(HERE, "sponge_a_026g.section.json")))
           ["symbols"].items()}
VA, STATE = base.SPONGE_A_VA, base.SPONGE_A_STATE_VA
HEAP, STACK, FAKE, CORE, CONFIG = 0x30000000, 0x21000000, 0x22000000, 0x40000000, 0x50000000
STOP = FAKE + 0x10
NEVER = 0xFFFFFFFF
# struct sponge (sponge_a_026g.c's _Static_asserts)
S_INFLICTOR, S_PASSIVE, S_DURATION, S_TIMEOUT, S_POWER, S_SPLASH, S_NODE, S_LINKED = \
    0x360, 0x370, 0x3A4, 0x3A8, 0x3AC, 0x3B0, 0x3F0, 0x418
S_EXPLODE, S_PIN, S_OWNER, S_BODY, S_PHYSICS, S_TRANSFORM, S_ZONE, S_SIZE = \
    0x41C, 0x420, 0x421, 0x424, 0x428, 0x430, 0x470, 0x688
S_SOUNDS, S_EFFECTS, S_SOUND_LOCAL, S_EFFECT_LOCAL = 0x474, 0x478, 0x47C, 0x57C
# the game's
STATE_AT, INVENTORY, SLOT, COUNT, WORLD = 0x280, 0x250, 0x254, 0x260, 0x248
TOXIC = 4                                              # the toxic damage type (0x304370: 0x9257E0 ranged, blunt, slash, explosive, toxic)


def sponge_options():
    """0.28d0's gameplay/items/artefacts/artefact_sponge.options, value for
    value (bcw: tuples are float vectors); --options compares it with the
    file."""
    curve = lambda k0, k1: {"key1": k1, "key0": k0}
    key = lambda lt, rt, pos: {"key_type": 3, "left_tangent": lt, "right_tangent": rt, "position": pos, "delta": 0}
    zero = (0.0, 0.0, 0.0)
    return {
        "parameters": {"weight": 0.10000000149011612},
        "ui_desc": {"combat_log_icon": 17, "text_descriptions": {"name": "st_artefact_sponge_name",
                                                                  "description": "st_artefact_spring_description"},
                    "icon": [22, 1], "props_list": {}},
        "object": {"particle_on_spawn": "fx/artifacts/pre_mochalka1", "model": "item/artefacts/artefact_sponge",
                   "sound_on_spawn": [257, "artefacts/artefact_appear_default"]},
        "data": {
            "pickup_hint": "st_sponge_pickup_hint",
            "active": {
                "damage_zone": {
                    "activated_sound_resource_id": 257, "damage_type": "toxic",
                    "lib_guid": "4587e8ff-b988-4320-a34a-118731e8e87b", "rotation": zero, "max_armor_piercing": 0,
                    "effect_zones": [{"rotation": zero, "position": zero}],
                    "position": (-53.3129997253418, 0.6710000038146973, 100.65799713134766),
                    "max_hit": 1.5, "interval_in_msec": 1000, "name": "damage_zone",
                    "idle_particle": "fx/test/idle_particle",
                    "speed_curve": curve(key((-1.0, 0.0), (3.3329999446868896, 1.0), (0.0, 1.0)),
                                         key((6.666999816894531, 1.0), (11.0, 0.0), (10.0, 1.0))),
                    "continuous_hit": True, "lib_name": "damage_zone", "tool_name": "misc", "type": "object",
                    "sound_zones": [{"position": zero}], "is_temporary": False, "hit_parts_filter": ["face"],
                    "pivot": zero,
                    "collision_geometries": [{
                        "collision_filter_group": 1, "filter_mask": 32, "rotation": zero, "filter_group": 1,
                        "position": zero, "mode": 0, "pivot": zero, "collision_filter_mask": 0,
                        "meshes": [{"rotation": zero, "position": (0.0, 1.0, 0.0), "type": 2,
                                    "scale": (3.5, 1.5, 3.5)}],
                        "full_name": "artefact_sponge_collision_geometry", "scale": (1.0, 1.0, 1.0)}],
                    "activated_sound": "weapons/f1",
                    "hit_curve": curve(key((-0.10000000149011612, 0.0), (0.9330000281333923, 0.699999988079071),
                                           (0.8999999761581421, 1.0)),
                                       key((0.9670000076293945, 0.4000000059604645),
                                           (126.802001953125, 0.10000000149011612), (1.0, 0.10000000149011612))),
                    "game_object_type": "damage_zone", "layer": "default", "scale": (1.0, 1.0, 1.0),
                    "activated_particle": "fx/artifacts/mochalka_explosion"},
                "stopping_power": 0, "throw_power": 15, "splash_max_damage": 1.5,
                "splash_min_damage": 0.05000000074505806, "duration_s": 10, "splash_damage_type": "toxic",
                "splash_affected_bodies": [6, 0, 1, 7, 11, 12], "stamina_damage": 0, "splash_radius": 10,
                "splash_pierce": 0},
            "cooldown_sec": 0, "activation_timeout_s": 3,
            "passive": {"regen_add": 0, "regen_mul": 1, "timeout_add": 0, "threshold_mul": 1, "timeout_mul": 1,
                        "threshold_add": 1.5},
            "type": 12, "amount": 1, "spawn_sec": 2, "effect": "artefact_spring",
            "activation_hint": "st_sponge_activation_hint"}}


def loaded(data, at):
    """The config as the game holds it: every offset of the file a pointer
    (check_research_026g.py's)."""
    out = bytearray(data)
    record = struct.Struct("<QQIHH")

    def fix(offset):
        payload, name, crc, kind, count = record.unpack_from(out, offset)
        if kind in (3, 4, 5, 6, 7, 8):
            payload = at + payload
        record.pack_into(out, offset, payload, at + name if name else 0, crc, kind, count)
        if kind in (3, 4):
            for i in range(count):
                fix(payload - at + 24 * i)
    fix(0)
    return bytes(out)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


class Run:
    """An exe in Unicorn: its sections mapped, the heap, a stack, a page of
    made-up code (each stub an address of its own, answered in Python)."""

    def __init__(self, sections):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for start, size, data in sections:
            emu.mem_map(start, (size + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, data)
        for at, size in ((HEAP, 0x1000000), (STACK, 0x20000), (FAKE, 0x10000), (CORE, 0x20000), (CONFIG, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.heap, self.next_stub, self.stubs, self.calls = HEAP, FAKE + 0x100, {}, []
        emu.mem_write(FAKE, b"\xCC" * 0x10000)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def float(self, at, value):
        self.emu.mem_write(at, struct.pack("<f", value))

    def floats(self, at, values):
        self.emu.mem_write(at, struct.pack(f"<{len(values)}f", *values))

    def read(self, at, size=4):
        return struct.unpack({1: "<B", 2: "<H", 4: "<I"}[size], self.emu.mem_read(at, size))[0]

    def readf(self, at, count=1):
        values = struct.unpack(f"<{count}f", self.emu.mem_read(at, 4 * count))
        return values[0] if count == 1 else list(values)

    def bytes(self, at, size):
        return bytes(self.emu.mem_read(at, size))

    def string(self, at):
        out = bytearray()
        while True:
            b = self.emu.mem_read(at + len(out), 1)[0]
            if not b:
                return out.decode("cp1251")
            out.append(b)

    def allocate(self, size, fill=0):
        at = self.heap
        self.heap += (size + 0xF) & ~0xF
        self.emu.mem_write(at, bytes([fill]) * size)
        return at

    def arg(self, i):
        return self.read(self.emu.reg_read(UC_X86_REG_ESP) + 4 + 4 * i)

    def ret(self, value, cleanup):
        emu = self.emu
        sp = emu.reg_read(UC_X86_REG_ESP)
        back = self.read(sp)
        emu.reg_write(UC_X86_REG_EAX, value & 0xFFFFFFFF)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + cleanup)
        emu.reg_write(UC_X86_REG_EIP, back)

    def stub(self, handler, cleanup=0, at=None, name=None):
        """`handler(run)` answers a call to `at` (a fresh address when none):
        it returns eax (None: 0) and the stub pops `cleanup`."""
        if at is None:
            at = self.next_stub
            self.next_stub += 0x10
        self.stubs[at] = (handler, cleanup, name)

        def hook(emu, address, size, user):
            value = handler(self)
            self.calls.append((name or hex(at), address))
            if value is not False:
                self.ret(value or 0, cleanup)
        self.emu.hook_add(UC_HOOK_CODE, hook, begin=at, end=at)
        return at

    def vtable(self, slots):
        """A made-up vtable: {slot index: (handler, cleanup)}."""
        table = self.allocate(4 * (1 + max(slots)))
        for index, (handler, cleanup) in slots.items():
            self.word(table + 4 * index, self.stub(handler, cleanup, name=f"slot {index}"))
        return table

    def call(self, function, args=(), regs=None, until=STOP):
        emu = self.emu
        sp = STACK + 0x10000 - 4 * len(args) - 4
        self.word(sp, STOP)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        for reg, value in (regs or {}).items():
            emu.reg_write(reg, value)
        self.start(function, until)
        return emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ESP) - sp - 4

    def start(self, function, until):
        if os.environ.get("SPONGE_TRACE") and not getattr(self, "traced", False):
            from unicorn import UC_HOOK_BLOCK
            self.trail = []
            self.emu.hook_add(UC_HOOK_BLOCK, lambda emu, address, size, user: (self.trail.append(address),
                                                                               len(self.trail) > int(os.environ.get("SPONGE_TRACE")) and self.trail.pop(0)))
            self.traced = True
        try:
            self.emu.emu_start(function, until, count=20_000_000)
        except Exception as error:
            regs = {name: self.emu.reg_read(reg) for name, reg in (
                ("eip", UC_X86_REG_EIP), ("eax", UC_X86_REG_EAX), ("ecx", UC_X86_REG_ECX), ("edx", UC_X86_REG_EDX),
                ("ebx", UC_X86_REG_EBX), ("esi", UC_X86_REG_ESI), ("edi", UC_X86_REG_EDI), ("esp", UC_X86_REG_ESP))}
            trail = " ".join(hex(a) for a in getattr(self, "trail", []))
            raise RuntimeError(f"{error} at " + " ".join(f"{k} {v:#x}" for k, v in regs.items())
                               + (f"; blocks {trail}" if trail else "")) from None

    def thiscall(self, function, this, *args):
        return self.call(function, args, {UC_X86_REG_ECX: this})

    def span(self, start, until, regs, stack=()):
        """The game's own code from `start` to `until` with esp at a frame
        of its own (`stack`: dwords at esp, esp+4, ...); (eip, esp moved by)."""
        emu = self.emu
        sp = STACK + 0x8000
        for i, value in enumerate(stack):
            self.word(sp + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        self.start(start, until)
        return emu.reg_read(UC_X86_REG_EIP), emu.reg_read(UC_X86_REG_ESP) - sp


def pe_sections(data):
    return [(base.IMAGE_BASE + rva, max(vsize, rsize), bytes(data[raw:raw + min(rsize, vsize or rsize)]))
            for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data)]


def raw_sections(data):
    """Any PE's sections (the 0.28d0 exe's, image base 0x10000)."""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    count, optional = struct.unpack_from("<H", data, pe + 6)[0], struct.unpack_from("<H", data, pe + 20)[0]
    image = struct.unpack_from("<I", data, pe + 24 + 28)[0]
    out = []
    for i in range(count):
        at = pe + 24 + optional + 40 * i
        vsize, rva, rsize, raw = struct.unpack_from("<IIII", data, at + 8)
        out.append((image + rva, max(vsize, rsize), bytes(data[raw:raw + rsize])))
    return out


def unbcj(data):
    """The x86 BCJ filter undone (what 0.28d0's installer's archives hold)."""
    packed = lzma.compress(data, format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2, "preset": 0}])
    return lzma.decompress(packed, format=lzma.FORMAT_RAW,
                           filters=[{"id": lzma.FILTER_X86}, {"id": lzma.FILTER_LZMA2, "preset": 0}])


IDENTITY = [1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0]


def translation(x, y, z):
    return IDENTITY[:12] + [x, y, z, 1.0]


class Game:
    """A made-up match around one sponge: the world (its physics world, the
    hit manager, the tick grid's origin 50, twenty places), a player in place
    3 (kit 0) with an inventory, a damage model of four pools, modifiers, the
    cook and the config resource."""
    PLACE = 3

    def __init__(self, run, options_bytes):
        self.run = r = run
        r.emu.mem_write(CORE, bytes(0x20000))
        self.log = []
        # the config resource ([res]+108h the root), the dictionary item (+22Ah is_stack)
        r.emu.mem_write(CONFIG, loaded(options_bytes, CONFIG))
        self.resource = r.allocate(0x200)
        r.word(self.resource + 0x108, CONFIG)
        r.word(self.resource + 0xD0, 1)
        self.resource_ptr = r.allocate(4)
        r.word(self.resource_ptr, self.resource)
        self.dict_item = r.allocate(0x240)
        r.emu.mem_write(self.dict_item + 0x22A, b"\x01")
        # GetCurrentThreadId (the resources' constructor 0xF7192)
        r.word(0x870118, r.stub(lambda r: 0x77, 0, name="GetCurrentThreadId"))
        # allocation: the cooks' allocator (0xE4F90)
        r.stub(lambda r: r.allocate(r.emu.reg_read(UC_X86_REG_EAX)), 4, at=0xE4F90, name="allocate")
        # physics: shapes and bodies
        self.shapes, self.bodies = [], []
        r.stub(self.make_shape, 0, at=0x7A390, name="make_shape")
        r.stub(self.make_body, 0, at=0x357100, name="make_body")
        r.stub(lambda r: self.log.append(("destroy_shape", r.emu.reg_read(UC_X86_REG_ESI))), 0, at=0x7A1B0)
        r.stub(lambda r: r.arg(0), 0, at=0x42A390, name="most_derived")
        allocator = r.allocate(0x20)
        r.word(allocator, r.vtable({6: (lambda r: self.log.append(("free_body", r.arg(0))), 0x10)}))
        r.word(0x1F1D384, allocator)
        self.body_vtable = r.vtable({
            1: (lambda r: r.emu.reg_read(UC_X86_REG_ECX) + 0x180, 0),                       # its object
            2: (lambda r: self.log.append(("body_dtor", r.emu.reg_read(UC_X86_REG_ECX))), 4),
            6: (self.body_set_transform, 4), 7: (self.body_get_transform, 4)})
        self.physics = r.allocate(0x40)
        r.word(self.physics, r.vtable({
            8: (lambda r: self.log.append(("pw_update", r.arg(0))), 4),
            11: (lambda r: self.log.append(("pw_add", r.arg(0), r.arg(1), r.arg(2))), 0xC),
            14: (lambda r: self.log.append(("pw_remove", r.arg(0), r.arg(1))), 8)}))
        r.word(CORE + 0x18FA4, self.physics)
        r.word(CORE + 0x18FBC, 50)
        # the hit manager: its collision world (the splash's queries)
        self.collision = r.allocate(0x40)
        self.targets = []
        r.word(self.collision, r.vtable({19: (self.ray_query, 0x18), 24: (self.sphere_query, 0x14)}))
        self.vector_allocator = r.allocate(0x10)
        r.word(self.vector_allocator, r.vtable({6: (lambda r: None, 0x10)}))
        self.hits = r.allocate(0x100)
        r.word(self.hits + 0x98, self.collision)
        r.word(CORE + 0x18F58, self.hits)
        # places, the player
        self.places = r.allocate(20 * 0x30)
        r.word(CORE + 0x18458, self.places)
        r.word(CORE + 0x1845C, self.places + 20 * 0x30)
        r.word(CORE + 0x18FD8, (1 << 20) - 1)                         # every place in the match
        self.player = self.make_player(self.PLACE)
        # the inventory: slots +10Ch, owner +174h, grenade slot +17Ch, dirty +184h
        self.inventory = r.allocate(0x200)
        r.word(self.player + 0x10C, self.inventory)
        r.word(self.inventory + 0x17C, 0x17)
        self.model = r.allocate(0x200)
        self.model_ptr = r.allocate(4)
        r.word(self.model_ptr, self.model)
        owner = r.allocate(0x10)
        r.word(owner, r.vtable({2: (lambda r: self.model_ptr, 0), 3: (lambda r: self.player, 0)}))
        r.word(self.inventory + 0x174, owner)
        # the damage model's pools (find_pool 0x2E58B0: +8 the first, +0 next, +80h the name)
        self.pools = {}
        previous = 0
        for name in ("head", "body", "pain", "infection"):
            pool = r.allocate(0x300)
            text = r.allocate(16)
            r.emu.mem_write(text, name.encode() + b"\0")
            r.word(pool + 0x80, text)
            if previous:
                r.word(previous, pool)
            else:
                r.word(self.model + 0x108 + 8, pool)
            previous = pool
            self.pools[name] = pool
        self.scene = self.make_scene()
        self.cook = r.allocate(0x40)
        r.word(self.cook, 0x9FC70C)

    def make_player(self, place):
        r = self.run
        player, profile = r.allocate(0x800), r.allocate(0x2200)
        r.word(player + 0x6B8, profile)
        r.emu.mem_write(player + 0x110 + 4, bytes([place, 0]))       # the game object's id; kit 0 (+115h)
        r.emu.mem_write(player + 0x33C, b"\x01")
        for index in range(0x40):                                        # the modifiers' lists sorted, empty
            r.emu.mem_write(profile + 0xF4 + 0x110 + 0x14 * index + 0x10, b"\x01")
        r.word(self.places + 0x30 * place, player)
        r.emu.mem_write(self.places + 0x30 * place + 0xC, b"\0")
        return player

    def modifier(self, index, value, player=None):
        """The kit's base value S of modifier `index` (no temporaries)."""
        profile = self.run.read((player or self.player) + 0x6B8)
        self.run.float(profile + 0xF4 + 4 * index, value)

    def make_scene(self):
        r = self.run
        scene, world, renderer = r.allocate(0x200), r.allocate(0x6000), r.allocate(0x10)
        r.word(scene + 0x9C, world)
        holder = r.allocate(0x200070)
        r.word(world + 0x5360, holder)
        r.word(holder + 0x200064, renderer)
        self.renderer = renderer
        for at, name in ((0x21CD0, "add_model"), (0x22280, "update_model")):
            r.stub(lambda r, name=name: self.log.append((name, r.emu.reg_read(UC_X86_REG_EAX), r.arg(0), r.arg(1),
                                                         r.readf(r.arg(2) + 0x30, 3))), 0x10, at=at)
        r.stub(lambda r: self.log.append(("remove_model", r.emu.reg_read(UC_X86_REG_EAX), r.arg(0), r.arg(1))), 8,
               at=0x21F40)
        return scene

    # physics stubs
    def make_shape(self, r):
        dims = r.readf(r.emu.reg_read(UC_X86_REG_ECX), 3)
        shape = r.allocate(0x200)
        self.shapes.append((shape, r.emu.reg_read(UC_X86_REG_EDX), dims, r.readf(r.arg(0), 3), r.arg(1)))
        return shape

    def make_body(self, r):
        body, bullet = r.allocate(0x200), r.allocate(0x200)
        r.word(body, self.body_vtable)
        r.word(body + 0x10, r.arg(1))
        r.word(body + 0x14, bullet)
        r.emu.mem_write(body + 0x100, struct.pack("<16f", *IDENTITY))
        self.bodies.append((body, r.bytes(r.arg(0), 0x50), r.arg(1)))
        return body

    def body_set_transform(self, r):
        body = r.emu.reg_read(UC_X86_REG_ECX)
        r.emu.mem_write(body + 0x100, r.bytes(r.arg(0), 0x40))
        self.log.append(("body_set", body, r.readf(r.arg(0) + 0x30, 3)))

    def body_get_transform(self, r):
        body, out = r.emu.reg_read(UC_X86_REG_ECX), r.arg(0)
        r.emu.mem_write(out, r.bytes(body + 0x100, 0x40))
        return out

    def ray_query(self, r):
        return 0                                                       # nothing between: the result empty

    def sphere_query(self, r):
        # (center, radius, 10h, 40h, &result {begin, end, ..}) cdecl? thiscall, ret 14h
        result = r.arg(4)
        objects = r.allocate(4 * max(1, len(self.targets)))
        for i, target in enumerate(self.targets):
            r.word(objects + 4 * i, target["object"])
        r.word(result, objects)
        r.word(result + 4, objects + 4 * len(self.targets))
        r.word(result + 8, self.vector_allocator)                     # its allocator frees it (0x3398CA)
        self.log.append(("sphere_query", r.readf(r.arg(0), 3), f32(struct.unpack("<f", struct.pack("<I", r.arg(1)))[0])))


def config_resource(run, at, value):
    """A config resource ([it]+108h the root) for `value` loaded at `at`;
    the pointer to it, as the cooks take it."""
    run.emu.mem_write(at, loaded(bcw.dump(value), at))
    resource, pointer = run.allocate(0x200), run.allocate(4)
    run.word(resource + 0x108, at)
    run.word(resource + 0xD0, 1)
    run.word(pointer, resource)
    return pointer


def child(run, table, *keys):
    """The game's own look-up (0xF6B10), key after key."""
    for key in keys:
        text = run.allocate(len(key) + 1)
        run.emu.mem_write(text, key.encode() + b"\0")
        table, _ = run.call(0xF6B10, regs={UC_X86_REG_EAX: table, UC_X86_REG_EDI: text})
    return table


def splash_of(run, at):
    """The splash parameters at `at` (struct splash)."""
    values = struct.unpack("<I8iI6f", run.bytes(at, 0x40))
    return {"inflictor": values[0], "bodies": list(values[1:9]), "type": values[9], "radius": values[10],
            "min": values[11], "max": values[12], "pierce": values[13], "stamina": values[14],
            "stopping": values[15]}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("original")
    parser.add_argument("--v028d0", help="0.28d0's binaries/x86/survarium.exe as shipped")
    parser.add_argument("--options", help="0.28d0's gameplay/items/artefacts/artefact_sponge.options")
    args = parser.parse_args()
    original = open(args.original, "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    options = bcw.dump(sponge_options())
    results = []

    def check(what, ok, detail=""):
        results.append(bool(ok))
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f": {detail}" if detail else ""))

    if args.options:
        shipped = bcw.load(args.options)
        check("the config written here is 0.28d0's artefact_sponge.options, value for value",
              shipped == sponge_options())

    run = Run(pe_sections(built))
    stock = Run(pe_sections(original))
    game = Game(run, options)
    stock_game = Game(stock, options)
    r = run

    # 1. The cook: size and construct, type 12 ours, every other type the switch's.
    size, cleanup = r.thiscall(0x25E770, game.cook, game.resource_ptr)
    check("the artefact cook's size for type 12 is the sponge's (688h), ret 4", size == S_SIZE and cleanup == 4,
          f"{size:#x}, cleanup {cleanup}")
    effect = r.allocate(4)
    sponge = r.allocate(S_SIZE, 0xAB)
    made, cleanup = r.thiscall(0x25E660, game.cook, sponge, game.resource_ptr, effect, game.scene)
    hit = splash_of(r, sponge + S_SPLASH)
    passive = r.readf(sponge + S_PASSIVE + 0x1C, 6)
    check("construct for type 12: the sponge's vtables, Rattle's object under them (artefact_base, the drawable's "
          "scene), ret 10h",
          made == sponge and cleanup == 0x10 and r.read(sponge) == SYMBOLS["sp_vtable"] + 4
          and r.read(sponge + 0x340) == SYMBOLS["sp_drawable_vtable"] + 4
          and r.read(sponge + S_INFLICTOR) == SYMBOLS["sp_inflictor_vtable"] + 4
          and r.read(sponge + 0x34C) == game.scene and r.read(sponge + STATE_AT) == 0
          and r.read(sponge + 0x27C, 2) == 1 and r.read(sponge + 0x274) == 0 and r.read(sponge + 0x278) == 2000,
          f"{made:#x}, vtable {r.read(sponge):#x}, amount {r.read(sponge + 0x27C, 2)}, spawn {r.read(sponge + 0x278)}")
    check("the config as 0.28's reader (0x5439E0) reads it: duration 10000, activation timeout 3000 (data's), "
          "throw power 15, passive {0, 1, 0, 1, 1.5, 1}",
          r.read(sponge + S_DURATION) == 10000 and r.read(sponge + S_TIMEOUT) == 3000
          and r.readf(sponge + S_POWER) == 15.0 and passive == [0.0, 1.0, 0.0, 1.0, 1.5, 1.0], str(passive))
    check("the splash's parameters in 0.26g's layout (the grenade set's +278h): radius 10, damage 0.05..1.5, "
          "pierce 0, toxic, the six bodies then -1, stamina and stopping 0",
          hit["radius"] == 10.0 and hit["min"] == f32(0.05) and hit["max"] == 1.5 and hit["pierce"] == 0.0
          and hit["type"] == TOXIC and hit["bodies"] == [6, 0, 1, 7, 11, 12, -1, -1]
          and hit["stamina"] == 0.0 and hit["stopping"] == 0.0, str(hit))
    check("not armed (m_explode_time_ms -1), the pin in, no body, identity m_transform, its inflictor "
          "{FFFFh, the sponge}, its regeneration node Lifebone's (0xA1566C|1, 0x312590, the sponge, the passive)",
          r.read(sponge + S_EXPLODE) == NEVER and r.read(sponge + S_PIN, 1) == 0 and r.read(sponge + S_BODY) == 0
          and r.readf(sponge + S_TRANSFORM, 16) == IDENTITY and r.read(sponge + S_INFLICTOR + 4, 2) == 0xFFFF
          and r.read(sponge + S_INFLICTOR + 0xC) == sponge and r.read(sponge + S_NODE) == 0xA1566C | 1
          and r.read(sponge + S_NODE + 8) == 0x312590 and r.read(sponge + S_NODE + 0x10) == sponge
          and r.read(sponge + S_NODE + 0x14) == sponge + S_PASSIVE)

    # 1b. Every other type is the switch's: Rattle (type 8) built alike by the built exe and the original.
    rattle = dict(sponge_options())
    rattle["data"] = dict(rattle["data"], type=8)
    same = []
    for emu in (r, stock):
        pointer = config_resource(emu, CONFIG + 0x8000, rattle)
        memory = emu.allocate(0x400, 0xAB)
        fixture = game if emu is r else stock_game
        made, cleanup = emu.thiscall(0x25E660, fixture.cook, memory, pointer, emu.allocate(4), fixture.scene)
        size, _ = emu.thiscall(0x25E770, fixture.cook, pointer)
        same.append((made == memory, cleanup, size, emu.read(memory), emu.read(memory + 0x340), emu.read(memory + 0x27C, 2)))
    check("type 8 (Rattle) through the hooked switches: as the original builds it (0x360 bytes, vtable 0xA06124)",
          same[0] == same[1] and same[0][2] == 0x360 and same[0][3] == 0xA06124, str(same))

    # 1c. The query: model, spawn particle and sound, and the damage zone from data/active/damage_zone.
    asked = []

    def query(r):
        functor = r.arg(0)
        asked.append({"requests": [(r.string(r.read(r.emu.reg_read(UC_X86_REG_EAX) + 8 * i)),
                                    r.read(r.emu.reg_read(UC_X86_REG_EAX) + 8 * i + 4))
                                   for i in range(r.emu.reg_read(UC_X86_REG_ECX))],
                      "functor": r.bytes(functor, 0x18), "allocator": r.arg(1),
                      "creation": [r.read(r.arg(2) + 4 * i) for i in range(r.emu.reg_read(UC_X86_REG_ECX))]
                      if r.arg(2) else [], "parent": r.arg(3), "flag": r.arg(4)})
        if asked[-1]["creation"] and asked[-1]["creation"][-1]:
            asked[-1]["variant"] = r.bytes(asked[-1]["creation"][-1], 0x30)
        return 0
    finished = []
    geometries = []
    for emu in (r, stock):
        emu.stub(query, 0, at=0xE5B60, name="query")
        emu.word(0x1F1D9C4, 0x5A110C)
    r.stub(lambda r: finished.append((r.emu.reg_read(UC_X86_REG_ECX), r.emu.reg_read(UC_X86_REG_EDX),
                                      r.emu.reg_read(UC_X86_REG_ESI), r.emu.reg_read(UC_X86_REG_EDI))), 0, at=0x2EE8D0)
    r.stub(lambda r: geometries.append((r.arg(0), r.arg(1))), 8, at=0x2F8C30)
    parent = r.allocate(0x40)
    _, cleanup = r.thiscall(0x25E800, game.cook, parent, sponge, game.resource_ptr)
    zone_value = child(r, CONFIG, "data", "active", "damage_zone")
    type_id, _ = r.call(0x125790)
    mine = asked[-1] if asked else {}
    functor = struct.unpack("<6I", mine.get("functor", bytes(0x18)))
    variant = struct.unpack("<II24s8xII", mine.get("variant", bytes(0x30)))
    check("a sponge's query: four requests (the model 0Fh, the particle 3Dh, the sound 101h, damage_zone 75h), "
          "the parent's, flag 1, the cooks' allocator; ret 0Ch",
          cleanup == 0xC and mine.get("requests") == [("item/artefacts/artefact_sponge", 0xF),
                                                      ("fx/artifacts/pre_mochalka1", 0x3D),
                                                      ("artefacts/artefact_appear_default", 257),
                                                      ("damage_zone", 0x75)]
          and mine.get("parent") == parent and mine.get("flag") == 1 and mine.get("allocator") == 0x5A110C,
          str(mine.get("requests")))
    check("the zone's creation data: a variant<32> (vtable 0x9ED1E4) of data/active/damage_zone's value, itself at "
          "+28h, the binary_config_value type id (0x125790) at +2Ch; none for the other three",
          mine.get("creation", [1])[:3] == [0, 0, 0] and variant[0] == 0x9ED1E4
          and variant[2] == r.bytes(zone_value, 24) and variant[3] == mine["creation"][3] and variant[4] == type_id,
          f"type id {type_id:#x}")
    check("the callback: our function (in place), {the sponge, the cook, the config resource}; the resource held",
          functor[0] == SYMBOLS["sp_function_vtable"] | 1 and functor[2:5] == (sponge, game.cook, game.resource)
          and r.read(game.resource + 0xD0) == 2, str([hex(v) for v in functor]))
    # the resources come (the resource system copies the function, then calls it)
    results_at = r.allocate(4 * 0x2E0 + 0x200)
    query_done = r.allocate(0x40)
    r.word(results_at + 0x20, query_done)
    resources = []
    for i in range(4):
        res = r.allocate(0x480)
        r.word(res + 0xD0, 1)
        r.word(results_at + 0x2E0 * i + 0x12C, res)
        resources.append(res)
    zone = resources[3]
    r.word(zone + 0x11C, r.allocate(4))
    r.word(zone + 0x120, 1)
    for at in (0x41C, 0x43C):
        r.word(zone + at, r.allocate(0x40))
        r.word(zone + at + 4, 1)
    copy = r.allocate(0x18)
    r.emu.mem_write(copy, mine["functor"])
    invoker = r.read((functor[0] & ~1) + 4)
    r.call(invoker, (copy + 8, results_at))
    shape = r.read(r.read(zone + 0x11C))
    check("loaded: Rattle's loader takes the model, particle and sound (+354h..+35Ch), the zone held (+470h), "
          "the cook's finish (cook, &resource, the sponge, the query), the resource let go",
          [r.read(sponge + 0x354 + 4 * i) for i in range(3)] == resources[:3] and r.read(sponge + S_ZONE) == zone
          and r.read(zone + 0xD0) == 2 and finished == [(game.cook, copy + 8 + 8, sponge, query_done)]
          and r.read(game.resource + 0xD0) == 1, f"finished {finished}")
    check("the zone's collision geometry made from the inline config (as 0x3046F0: 24h bytes, vtables "
          "0xA13428/0xA13454) and loaded (0x2F8C30) from collision_geometries[0]",
          shape and r.read(shape) == 0xA13428 and r.read(shape + 4) == 0xA13454
          and geometries == [(shape, child(r, CONFIG, "data", "active", "damage_zone", "collision_geometries")
                              and r.read(child(r, CONFIG, "data", "active", "damage_zone", "collision_geometries")))],
          str(geometries))
    check("one sound zone and one effect zone, their offsets kept (identity: both at the zone's origin)",
          r.read(sponge + S_SOUNDS) == 1 and r.read(sponge + S_EFFECTS) == 1
          and r.readf(sponge + S_SOUND_LOCAL, 16) == IDENTITY and r.readf(sponge + S_EFFECT_LOCAL, 16) == IDENTITY)
    rattle_queries = []
    for emu in (r, stock):
        before = len(asked)
        pointer = config_resource(emu, CONFIG + 0x8000, rattle)
        emu.thiscall(0x25E800, (game if emu is r else stock_game).cook, emu.allocate(0x40), emu.allocate(0x400), pointer)
        rattle_queries.append(asked[before:][0]["requests"] if len(asked) > before else None)
    check("any other type's query is the original's (Rattle: its three requests)",
          rattle_queries[0] == rattle_queries[1] and rattle_queries[0] is not None and len(rattle_queries[0]) == 3,
          str(rattle_queries[0]))

    # 2. set_world: Rattle's (the world, the drawable linked), the rigid body (0.28's numbers), the zone's drawable.
    vt = lambda obj, slot: r.read(r.read(obj) + 4 * slot)
    _, cleanup = r.thiscall(vt(sponge, 28), sponge, CORE)
    shape, kind, dims, scale, flag = game.shapes[-1] if game.shapes else (0, 0, 0, 0, 0)
    body, desc, body_shape = game.bodies[-1] if game.bodies else (0, b"", 0)
    bullet = r.read(body + 0x14) if body else 0
    expected = struct.pack("<4f4fB3x4f6fB3x", 0.3, 0, 0, 0, 0.5, 0, 0.8, 1.0, 0, 0.005, 0.01, 0.01, 0.01,
                           1, 1, 1, 1, 1, 1, 1)
    check("set_world: the world (+248h), a sphere 0.05 (type 3, dims 0.05 0.1 0.1, scale 1; its resource flag 2) "
          "and a body of 0.28's (and the grenade's) description, damping 0.03 and 0.01; ret 4",
          cleanup == 4 and r.read(sponge + WORLD) == CORE and kind == 3 and dims == [f32(0.05), f32(0.1), f32(0.1)]
          and scale == [1.0, 1.0, 1.0] and flag == 0 and r.read(shape + 0xFC) & 2 and desc == expected
          and body_shape == shape and r.read(sponge + S_BODY) == body and r.readf(bullet + 0x104) == f32(0.03)
          and r.readf(bullet + 0x100) == f32(0.01), f"desc {desc.hex()}")
    drawables = []
    at = r.read(game.scene + 0xDC + 4)
    while at and len(drawables) < 8:
        drawables.append(at)
        at = r.read(at + 8)
    check("the scene's drawables: the sponge's (+340h, by Rattle's 0x25FB30), then its zone's (+408h)",
          drawables == [sponge + 0x340, zone + 0x408] and r.read(game.scene + 0xDC + 8) == zone + 0x408,
          str([hex(d) for d in drawables]))

    # 3. Into the inventory (slot 13) as the game does it: set_inventory, added_to_inventory: the passive on.
    r.word(sponge + 0x25C, game.dict_item)
    r.emu.mem_write(sponge + COUNT, struct.pack("<H", 1))
    r.word(game.inventory + 0x10C + 4 * 13, sponge)
    r.word(sponge + 0xD0, r.read(sponge + 0xD0) + 1)
    r.thiscall(vt(sponge, 10), sponge, game.inventory, 13)
    r.thiscall(vt(sponge, 12), sponge)
    infection = game.pools["infection"]
    check("added_to_inventory: state 3, the node at the end of the infection pool's regeneration list (+2E8h)",
          r.read(sponge + STATE_AT) == 3 and r.read(infection + 0x2EC) == sponge + S_NODE
          and r.read(infection + 0x2F0) == sponge + S_NODE and r.read(sponge + S_LINKED) == infection + 0x2E8)
    info = r.allocate(0x10)
    regen_cases = [(0.0, 8000, 0.5), (0.03, 0, 1.0), (0.1, 2500, 0.0)]
    regen = []
    for speed, timeout, threshold in regen_cases:
        r.emu.mem_write(info, struct.pack("<fIf", speed, timeout, threshold))
        invoker = r.read((r.read(sponge + S_NODE) & ~1) + 4)
        r.call(invoker, (sponge + S_NODE + 8, 0, info))
        regen.append(struct.unpack("<fIf", r.bytes(info, 12)))
    check("the node through the game's invoker (0x312640) and Lifebone's callback: (s+0)x1, (t+0)x1, (th+1.5)x1",
          regen == [(0.0, 8000, 2.0), (f32(0.03), 0, 2.5), (f32(0.1), 2500, 1.5)], str(regen))

    # 4. The press: the grenade slot, the throw state's way in (0x2E4090), the pin, the release arming the fuse.
    _, cleanup = r.thiscall(vt(sponge, 11), sponge, 1, 100000)
    check("a press (not armed) puts it in the grenade slot (inventory +17Ch = its slot 13); ret 8",
          cleanup == 8 and r.read(game.inventory + 0x17C) == 13)

    def weapon_core(emu, player):
        core, mode, holder_if = emu.allocate(0x600), emu.allocate(0x40), emu.allocate(0x10)
        emu.word(core + 0x278, mode)
        emu.word(mode + 0x20, mode)                       # +20h -> +24h: not 4
        emu.word(core + 0x5EC, 6)
        emu.word(core + 8, player)
        flags = emu.allocate(0x400)
        flags_ptr = emu.allocate(4)
        emu.word(flags_ptr, flags)
        emu.word(holder_if, emu.vtable({2: (lambda r: flags_ptr, 0)}))
        emu.word(player + 0x108, emu.read(holder_if))
        return core
    core = weapon_core(r, game.player)
    entered, _ = r.thiscall(0x2E4090, core)
    stock_inventory = stock.allocate(0x200)
    stock_item = stock.allocate(S_SIZE)
    stock.word(stock_item, 0xA06124)                      # an artefact (Rattle's vtable): +64h returns 0
    stock.word(stock_inventory + 0x17C, 13)
    stock.word(stock_inventory + 0x10C + 4 * 13, stock_item)
    stock.word(stock_game.player + 0x10C, stock_inventory)
    stock_core = weapon_core(stock, stock_game.player)
    try:
        stock.thiscall(0x2E4090, stock_core)
        stock_fault = None
    except RuntimeError as error:
        stock_fault = str(error)
    check("the throw state's way in (0x2E4090) takes it: not in the air; the original, with an artefact there, "
          "reads its grenade set through null (0x30A090: [0+25Ch])",
          (entered & 0xFF) == 1 and stock_fault and "eip 0x30a090" in stock_fault, stock_fault)
    throw_state, substate = r.allocate(0x200), r.allocate(0x200)
    r.word(throw_state + 0x17C, sponge)
    r.emu.mem_write(throw_state + 0x180, b"\xFF")
    r.word(substate + 0x138, throw_state)
    eip, moved = r.span(0x3103C9, 0x3103E3, {UC_X86_REG_ESI: substate, UC_X86_REG_EAX: 100050, UC_X86_REG_EDI: 0})
    check("the pull substate (0x3103C9): the pin out, the throw state's grenade index 0 (0.28's "
          "grenade_set_base[1] 0x514610)", eip == 0x3103E3 and r.read(sponge + S_PIN, 1) == 1
          and r.read(throw_state + 0x180, 1) == 0, f"eip {eip:#x}")
    eip, _ = r.span(0x33A677, 0x33A692, {UC_X86_REG_ESI: throw_state, UC_X86_REG_EAX: 0})
    check("is_ready (0x33A677): no zone on, not done", eip == 0x33A692, f"eip {eip:#x}")
    _, cleanup = r.thiscall(vt(sponge, 11), sponge, 0, 100060)
    armed_at = r.read(sponge + S_EXPLODE)
    check("the release: out of the grenade slot (17h), armed: state 6, m_explode_time_ms = 100060 + 3000 rounded up "
          "to the tick grid from 50 (103100)",
          r.read(game.inventory + 0x17C) == 0x17 and r.read(sponge + STATE_AT) == 6 and armed_at == 103100,
          f"{armed_at}")
    fuse = []
    probe = r.allocate(S_SIZE)
    r.emu.mem_write(probe, r.bytes(sponge, S_SIZE))
    for now in (100000, 100001, 100049, 100050, 99999, 30, 0):
        r.word(probe + S_EXPLODE, NEVER)
        r.emu.mem_write(probe + S_PIN, b"\x01")
        r.thiscall(vt(probe, 11), probe, 0, now)
        fuse.append(r.read(probe + S_EXPLODE))
    check("the fuse's end on other times: the first tick of the grid (50 + 50k) at or past now + 3000, as 0.28's "
          "0x514580 and 0.26g's pull_pin 0x30A120 round",
          fuse == [103000, 103050, 103050, 103050, 103000, 3050, 3000], str(fuse))
    r.thiscall(vt(sponge, 11), sponge, 1, 100100)
    check("a press while armed changes nothing but the grenade slot (17h)",
          r.read(game.inventory + 0x17C) == 0x17 and r.read(sponge + S_EXPLODE) == 103100)

    # 5. The throw (0x33A7BB): from the hand's transform, along the aim's z, 15 x the holder's throw modifier (27).
    game.modifier(27, 0.2)
    aim = [1.0, 0, 0, 0, 0, 1.0, 0, 0, 0.6, 0, 0.8, 0, 5.0, 1.6, 7.0, 1.0]
    r.floats(game.player + 0x44C, aim)
    hand = translation(5.2, 1.5, 7.1)
    frame = [0, 0, 0, 0] + [struct.unpack("<I", struct.pack("<f", v))[0] for v in hand]
    game.log.clear()
    eip, moved = r.span(0x33A7BB, 0x33A7D9, {UC_X86_REG_ESI: throw_state, UC_X86_REG_EDI: game.player}, frame)
    linear = r.readf(bullet + 0x140, 3)
    angular = r.readf(bullet + 0x150, 3)
    check("thrown: the body at the hand's transform into the physics world (818h, 8), velocity (0.6, 0, 0.8) x 15 "
          "x 1.2 (0.26g's setter keeps it with z negated), no spin, the item count 0, the inventory dirty",
          eip == 0x33A7D9 and moved == 0 and ("body_set", body, [f32(5.2), 1.5, f32(7.1)]) in game.log
          and ("pw_add", body, 0x818, 8) in game.log and r.read(sponge + S_PHYSICS) == game.physics
          and [round(v, 4) for v in linear] == [10.8, 0.0, -14.4] and angular == [0.0, 0.0, 0.0]
          and r.read(sponge + COUNT, 2) == 0 and r.read(game.inventory + 0x184, 1) == 1,
          f"velocity {linear}, log {game.log}")
    entered, _ = r.thiscall(0x2E4090, core)
    check("in the air it is no longer a way into the throw state (0.28's grenade_set_base[6])", (entered & 0xFF) == 0)

    # 6. The fuse's ticks to the explosion (50 ms), in flight; the explosion; the zone's ten seconds.
    events = []
    zone_vtable = r.vtable({7: (lambda r: events.append(("zone_on_activate", r.emu.reg_read(UC_X86_REG_ECX), r.arg(0))), 4)})
    r.word(zone, zone_vtable)
    r.stub(lambda r: (events.append(("activate", r.arg(0), r.arg(1))), r.emu.mem_write(r.arg(0) + 0x1C, b"\x01"))
           and None, 8, at=0x3080B0)
    g = lambda: tuple(r.read(STATE + 4 * i) for i in range(4))
    r.stub(lambda r: events.append(("zone_tick", r.emu.reg_read(UC_X86_REG_ECX), r.arg(0), r.arg(1), r.arg(2), g())),
           0xC, at=0x319A70)
    r.stub(lambda r: (events.append(("zone_off", r.arg(0), r.arg(1))), r.emu.mem_write(r.arg(0) + 0x124, b"\x00"))
           and None, 8, at=0x319D30)
    placed = {}

    def geometry_set(r):
        placed[r.emu.reg_read(UC_X86_REG_ECX)] = r.bytes(r.arg(0), 0x40)
        events.append(("geometry_set", r.emu.reg_read(UC_X86_REG_ECX), r.readf(r.arg(0) + 0x30, 3)))

    def geometry_get(r):
        r.emu.mem_write(r.arg(0), placed.get(r.emu.reg_read(UC_X86_REG_ECX), bytes(0x40)))
        return r.arg(0)
    r.stub(geometry_set, 4, at=0x2F8FE0)
    r.stub(geometry_get, 4, at=0x2F9030)
    splashes = []
    r.stub(lambda r: splashes.append((splash_of(r, r.emu.reg_read(UC_X86_REG_ESI)), r.arg(0), r.readf(r.arg(1), 3),
                                      r.arg(2), r.read(r.read(r.emu.reg_read(UC_X86_REG_ESI)) + 8))), 0xC, at=0x339630)
    game.modifier(12, 0.5)
    game.modifier(11, 0.1)
    r.word(sponge + 0xD0, 2)                              # the anomaly's reference and the slot's
    time, flight = 100100, []
    while time < armed_at:
        time += 50
        r.floats(body + 0x100, translation(5.2 + (time - 100100) / 1000.0, 1.5, 7.1 + (time - 100100) / 500.0))
        game.log.clear()
        r.thiscall(vt(sponge, 29), sponge, 50, time)
        flight.append((time, r.readf(sponge + S_TRANSFORM + 0x30, 3), ("pw_update", body + 0x180) in game.log,
                       r.read(sponge + STATE_AT)))
    before_last = flight[-2]
    check("the fuse in flight: each tick m_transform the body's and the physics world updated; state 6 until "
          "m_explode_time_ms", all(f[2] for f in flight) and before_last[3] == 6
          and before_last[1] == [f32(5.2 + 2.95), 1.5, f32(7.1 + 5.9)], str(before_last))
    hit, manager, at, when, instigator = splashes[0] if splashes else ({}, 0, [], 0, 0)
    check("at 103100 the splash (0.26g's service 0x339630, as a grenade's): its parameters with the holder's "
          "radius (12: x1.5) and damage (11: x1.1) modifiers, the inflictor the sponge's (+360h) naming the "
          "thrower's game object, where it is, the time",
          len(splashes) == 1 and hit["inflictor"] == sponge + S_INFLICTOR and hit["radius"] == 15.0
          and hit["min"] == f32(f32(0.05) * f32(1.1)) and hit["max"] == f32(1.5 * f32(1.1)) and hit["pierce"] == 0
          and hit["type"] == TOXIC and hit["bodies"][:6] == [6, 0, 1, 7, 11, 12] and manager == game.hits
          and at == [f32(5.2 + 3.0), 1.5, f32(7.1 + 6.0)] and when == armed_at
          and instigator == game.player + 0x110, str(splashes))
    transform = r.readf(sponge + S_TRANSFORM, 16)
    zone_ticks = [e for e in events if e[0] == "zone_tick"]
    check("the zone on (0.28's 0x51E520 as 0.26g's anomaly does it: +1A8h the physics world, its geometries' "
          "ghosts in (0x3080B0), timers reset, vt+1Ch(1)), its geometry at the sponge's transform, its sound and "
          "effect zones there, the thrower's place (3) kept for its hits",
          ("activate", zone + 0x108, game.physics) in events and ("zone_on_activate", zone, 1) in events
          and r.read(zone + 0x1A8) == game.physics and r.read(zone + 0x1CC) == NEVER
          and r.read(zone + 0x1D1, 1) == 0 and ("geometry_set", r.read(r.read(zone + 0x11C)), transform[12:15]) in events
          and r.readf(r.read(zone + 0x41C), 16) == transform and r.readf(r.read(zone + 0x43C), 16) == transform
          and r.read(sponge + S_OWNER, 1) == 3,
          f"{r.read(zone + 0x1A8):#x} {r.read(zone + 0x1CC):#x} {r.read(zone + 0x1D1, 1)} sound "
          f"{r.readf(r.read(zone + 0x41C), 16)} effect {r.readf(r.read(zone + 0x43C), 16)} transform {transform} "
          f"owner {r.read(sponge + S_OWNER, 1)}")
    check("the body out of the physics world, the pin in, out of its slot (0x2FC680: slot 13 empty, the item's "
          "inventory none, slot 17h, the inventory dirty), the passive out, state 7",
          ("pw_remove", body, 1) in game.log and r.read(sponge + S_PHYSICS) == 0 and r.read(sponge + S_PIN, 1) == 0
          and r.read(game.inventory + 0x10C + 4 * 13) == 0 and r.read(sponge + INVENTORY) == 0
          and r.read(sponge + SLOT) == 0x17 and r.read(game.inventory + 0x184, 1) == 1
          and r.read(sponge + S_LINKED) == 0 and r.read(infection + 0x2EC) == 0 and r.read(sponge + STATE_AT) == 7
          and r.read(sponge + 0xD0) == 1)
    check("the same tick the zone ticks (0x319A70: 50, the time, on the tick grid) naming its thrower, the "
          "inflictor and the sponge for the hit hook; nothing named after",
          zone_ticks and zone_ticks[0][1:5] == (zone, 50, armed_at, 1)
          and zone_ticks[0][5] == (zone, game.player + 0x110, sponge + S_INFLICTOR, sponge) and g() == (0, 0, 0, 0),
          str(zone_ticks[:1]))
    events.clear()
    while time < armed_at + 10000 - 50:
        time += 50
        r.thiscall(vt(sponge, 29), sponge, 50, time)
    ticks = [e for e in events if e[0] == "zone_tick"]
    check("the zone ticks every tick for ten seconds, still state 7 at 9.95 s",
          len(ticks) == 199 and all(e[1:4] == (zone, 50, armed_at + 50 * (i + 1)) for i, e in enumerate(ticks))
          and r.read(sponge + STATE_AT) == 7 and not [e for e in events if e[0] == "zone_off"])
    time += 50
    r.thiscall(vt(sponge, 29), sponge, 50, time)
    check("at m_explode_time_ms + duration (113100): the zone off (0x319D30(zone, 0)), state 0, not armed",
          ("zone_off", zone, 0) in events and r.read(sponge + STATE_AT) == 0 and r.read(sponge + S_EXPLODE) == NEVER)

    # 7. The zone's hit call site (0x319C7C..0x319CAC): stock args, or the sponge's while it ticks its zone.
    def site(emu, zone_at, g_values):
        hits = []
        receiver = emu.allocate(0x20)
        emu.word(receiver, emu.vtable({8: (lambda e: hits.append(tuple(e.arg(i) for i in range(8))), 0x20)}))
        frame = [0] * 0x40
        frame[0x24 // 4] = struct.unpack("<I", struct.pack("<f", 0.25))[0]    # the power ([esp+24h])
        frame[0x10 // 4] = struct.unpack("<I", struct.pack("<f", 0.75))[0]    # the hit ([esp+20h] after 10h bytes)
        frame[0x9C // 4] = 123456                                              # the time ([esp+0B4h] after 18h)
        zone_obj = emu.allocate(0x200)
        emu.word(zone_obj + 0x1BC, 4)
        for i, value in enumerate(g_values):
            emu.word(STATE + 4 * i, value)
        emu.emu.reg_write(UC_X86_REG_ECX, receiver)
        eip, moved = emu.span(0x319C7C, 0x319CAC, {UC_X86_REG_EDI: zone_at or zone_obj, UC_X86_REG_EBX: 0,
                                                   UC_X86_REG_ECX: receiver}, frame)
        for i in range(len(g_values)):
            emu.word(STATE + 4 * i, 0)
        return eip, moved, hits, zone_obj
    _, _, stock_hits, _ = site(stock, 0, ())
    eip, moved, plain_hits, _ = site(r, 0, (0x77777777, 1, 2, 3))
    mine = r.allocate(0x200)
    r.word(mine + 0x1BC, 4)
    eip2, moved2, sponge_hits, _ = site(r, mine, (mine, game.player + 0x110, sponge + S_INFLICTOR, sponge))
    part = STACK + 0x8000 + 0x28                           # [esp+3Ch] after 14h bytes: the part's name
    check("the zone's hit (the receiver's vt+20h): the original's (time, 0, &part, type, hit, power, 0, 0) for any "
          "zone; for the zone a sponge ticks the thrower's game object, its inflictor and it in the three 0s",
          stock_hits == plain_hits and len(stock_hits) == 1 and stock_hits[0][0] == 123456
          and stock_hits[0][1] == 0 and stock_hits[0][2] == part and stock_hits[0][3] == 4
          and stock_hits[0][6:] == (0, 0)
          and struct.unpack("<2f", struct.pack("<2I", *stock_hits[0][4:6])) == (0.75, 0.25)
          and sponge_hits == [(123456, game.player + 0x110, stock_hits[0][2], 4, *stock_hits[0][4:6],
                               sponge + S_INFLICTOR, sponge)] and moved == moved2 and eip == eip2 == 0x319CAC,
          f"stock {stock_hits}, sponge {sponge_hits}")

    # 8. The splash (0x339630, the game's own) on made-up players at several distances, against 0.28's damage.
    srun = Run(pe_sections(built))
    sgame = Game(srun, options)
    srun.stub(lambda r: None, 4, at=0x2DA5B0)                     # the player's bones as they are
    recorded = []

    def target(distance, place):
        p = sgame.make_player(place)
        center = (10.0, 0.0, 20.0)
        position = (center[0] + distance, 0.0, center[2])
        obj, owner = srun.allocate(0x40), srun.allocate(0x10)
        matrix = srun.allocate(0x40)
        srun.floats(matrix, translation(*position))
        srun.word(obj, srun.vtable({7: (lambda r, m=matrix: (r.emu.mem_write(r.arg(0), r.bytes(m, 0x40)), r.arg(0))[1],
                                        4)}))
        srun.word(obj + 0xC, owner)
        srun.word(owner, srun.vtable({0: (lambda r, p=p: p + 0x118, 0), 2: (lambda r, p=p: p, 0)}))
        world_matrix = srun.allocate(0x40)
        srun.floats(world_matrix, translation(*position))
        srun.word(p + 0x118, srun.vtable({
            9: (lambda r, place=place: recorded.append((place, tuple(r.arg(i) for i in range(10)))), 0x28),
            10: (lambda r, m=world_matrix: m, 0)}))
        a, b_, c = srun.allocate(0x200), srun.allocate(0x40), srun.allocate(0x40)
        bones = srun.allocate(0x50 * 16)
        for i in range(16):
            srun.floats(bones + 0x50 * i, [1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0, 0, 0, 1.0, 0, 1.0])
        srun.word(p + 0x498, a)
        srun.word(a + 0x12C, b_)
        srun.word(b_ + 0x34, c)
        srun.word(c + 0x18, bones)
        return {"object": obj, "place": place, "distance": distance}
    distances = [0.5, 3.0, 7.5, 9.99, 10.5, 14.0]
    sgame.targets = [target(d, 4 + i) for i, d in enumerate(distances)]
    infl = srun.allocate(0x10)
    srun.word(infl + 8, sgame.player + 0x110)
    srun.word(infl + 0xC, 0x1234)

    def splash(pierce, radius=15.0):
        recorded.clear()
        params = srun.allocate(0x40)
        srun.emu.mem_write(params, struct.pack("<I8iI6f", infl, 6, 0, 1, 7, 11, 12, -1, -1, TOXIC, radius,
                                               f32(f32(0.05) * f32(1.1)), f32(1.5 * f32(1.1)), pierce, 0.0, 0.0))
        center = srun.allocate(12)
        srun.floats(center, [10.0, 0.0, 20.0])
        _, cleanup = srun.call(0x339630, (sgame.hits, center, 777), {UC_X86_REG_ESI: params})
        out = {}
        for place, hit in recorded:
            out.setdefault(place, []).append((hit[2], struct.unpack("<f", struct.pack("<I", hit[4]))[0]))
        return cleanup, out, list(recorded)
    cleanup, with_config, _ = splash(0.0)
    check("the sponge's own splash (splash_pierce 0): no hit at all, in 0.26g as in 0.28 (each bone's share is "
          "the pierce left after obstacles over the pierce: 0 here, 0/0 in 0.28, neither above 0); ret 0Ch",
          cleanup == 0xC and with_config == {}, str(with_config))
    cleanup, pierced, raw = splash(1.0)
    lo, hi = f32(f32(0.05) * f32(1.1)), f32(1.5 * f32(1.1))
    expected = {}
    for i, d in enumerate(distances):
        f = f32(f32(15.0 - d) / 15.0)
        if f >= 0:
            expected[4 + i] = f32(f32((hi - lo) * f) + lo)
    got = {place: values for place, values in pierced.items()}
    agree = all(got.get(place) and [b for b, _ in got[place]] == [6, 0, 1, 7, 11, 12]
                and all(abs(v - dmg) <= 1e-6 * max(1.0, dmg) for _, v in got[place]) for place, dmg in expected.items())
    check("with pierce 1 (no obstacle in between): each target within the radius 15 hit on the six bodies with "
          "0.28's damage min + (max - min) x (r - d) / r (0x5412AE..0x5412DC), none outside; the hit names the "
          "thrower (the inflictor's +8), the inflictor and its item (+0Ch), toxic, the time",
          agree and set(got) == set(expected) and all(h[1][0] == 777 and h[1][1] == sgame.player + 0x110
                                                      and h[1][3] == TOXIC and h[1][6] == infl and h[1][9] == 0x1234
                                                      for h in raw),
          str({p: v[0][1] for p, v in got.items()}) + f" expected {expected}")
    splash_inputs = [(d, got[4 + i][0][1] if got.get(4 + i) else None) for i, d in enumerate(distances)]

    # 9. Serialization (0.28's 0x513A70 / 0x513C80) at each stage, through the game's writers; loaded into
    #    a fresh sponge each time, which then writes the same bytes again.
    def resize(r):
        buffer, size = r.emu.reg_read(UC_X86_REG_EAX), r.arg(0)
        if size > r.read(buffer + 8):
            fresh = r.allocate(size + 0x400)
            r.emu.mem_write(fresh, r.bytes(r.read(buffer + 4), r.read(buffer + 0xC)))
            r.word(buffer + 4, fresh)
            r.word(buffer + 8, size + 0x400)
        r.word(buffer + 0xC, size)
    r.stub(resize, 4, at=0x7AB30)
    r.stub(lambda r: r.allocate(0x20), 0, at=0x80450)                # the dump's record
    type_name = r.allocate(8)
    r.stub(lambda r: type_name, 4, at=0x420724)
    zone_io = []

    def zone_serialize(r):
        buffer = r.read(r.arg(0) + 0x14)
        size = r.read(buffer + 0xC)
        if size + 5 > r.read(buffer + 8):
            r.emu.reg_write(UC_X86_REG_EAX, buffer)
        fresh = r.allocate(size + 5)
        r.emu.mem_write(fresh, r.bytes(r.read(buffer + 4), size) + b"ZONE!")
        r.word(buffer + 4, fresh)
        r.word(buffer + 8, size + 5)
        r.word(buffer + 0xC, size + 5)
        zone_io.append(("zone_serialize", r.emu.reg_read(UC_X86_REG_ECX)))

    def zone_deserialize(r):
        reader = r.arg(0)
        zone_io.append(("zone_deserialize", r.emu.reg_read(UC_X86_REG_ECX), r.bytes(r.read(reader + 4), 5)))
        r.word(reader + 4, r.read(reader + 4) + 5)
    serial_vtable = r.vtable({0: (zone_serialize, 8), 1: (zone_deserialize, 0xC),
                              2: (lambda r: zone_io.append(("zone_after", r.emu.reg_read(UC_X86_REG_ECX), r.arg(0))), 4)})

    def make_sponge(slot):
        memory = r.allocate(S_SIZE, 0xAB)
        r.thiscall(0x25E660, game.cook, memory, game.resource_ptr, r.allocate(4), game.scene)
        r.thiscall(0x25E800, game.cook, r.allocate(0x40), memory, game.resource_ptr)
        functor = asked[-1]["functor"]
        place = r.allocate(4 * 0x2E0 + 0x200)
        r.word(place + 0x20, r.allocate(0x40))
        for i in range(4):
            res = r.allocate(0x480)
            r.word(res + 0xD0, 1)
            r.word(place + 0x2E0 * i + 0x12C, res)
        zone_at = r.read(place + 0x2E0 * 3 + 0x12C)
        r.word(zone_at, zone_vtable)
        r.word(zone_at + 0x130, serial_vtable)
        r.word(zone_at + 0x11C, r.allocate(4))
        r.word(zone_at + 0x120, 1)
        for at in (0x41C, 0x43C):
            r.word(zone_at + at, r.allocate(0x40))
            r.word(zone_at + at + 4, 1)
        held = r.allocate(0x18)
        r.emu.mem_write(held, functor)
        r.call(r.read((struct.unpack_from("<I", functor)[0] & ~1) + 4), (held + 8, place))
        r.thiscall(vt(memory, 28), memory, CORE)
        r.word(memory + 0x25C, game.dict_item)
        r.emu.mem_write(memory + COUNT, struct.pack("<H", 1))
        r.word(memory + 0xD0, 2)
        r.word(game.inventory + 0x10C + 4 * slot, memory)
        r.thiscall(vt(memory, 10), memory, game.inventory, slot)
        r.thiscall(vt(memory, 12), memory)
        return memory, zone_at, r.read(memory + S_BODY)

    def write(obj, dump=False):
        writer, buffer = r.allocate(0x20), r.allocate(0x10)
        r.word(writer + 0x14, buffer)
        r.word(buffer + 4, r.allocate(0x10))
        r.word(buffer + 8, 0x10)
        r.emu.mem_write(0x1F1D32C, b"\x01\x01" if dump else b"\x00\x00")
        r.thiscall(vt(obj, 26), obj, writer, 0)
        r.emu.mem_write(0x1F1D32C, b"\x00\x00")
        names, at = [], r.read(writer + 8)
        while dump and at and len(names) < 64:
            names.append(r.string(r.read(at + 8)))
            at = r.read(at)
        return r.bytes(r.read(buffer + 4), r.read(buffer + 0xC)), names

    def load(obj, blob):
        reader, source = r.allocate(8), r.allocate(len(blob) + 16)
        r.emu.mem_write(source, blob)
        r.word(reader + 4, source)
        _, cleanup = r.thiscall(vt(obj, 27), obj, reader, 0, 0)
        return r.read(reader + 4) - source, cleanup

    def round_trip(obj, slot, label):
        blob, names = write(obj, dump=True)
        fresh, fresh_zone, fresh_body = make_sponge(slot)
        zone_io.clear()
        game.log.clear()
        events.clear()
        read, cleanup = load(fresh, blob)
        again, _ = write(fresh)
        return blob, names, fresh, fresh_zone, fresh_body, read == len(blob) and cleanup == 0xC, again == blob

    base_names = ["m_state", "m_time_left_to_cool", "m_last_activation_time_ms"]
    b_sponge, b_zone, b_body = make_sponge(14)
    blob, names, c, c_zone, c_body, read_ok, same = round_trip(b_sponge, 15, "held")
    check("held (state 3): the base's fields, then m_explode_time_ms, m_is_pin_pulled, thrown() (0.28's order and "
          "lines, named in the hash mismatch dump), then ours: the tap, the thrower's place; a fresh sponge reads them all (ret 0Ch), is held as it was "
          "(the passive in), and writes the same bytes",
          read_ok and same and names[-5:] == ["m_explode_time_ms", "m_is_pin_pulled", "thrown()",
                                              "m_released_before_pin", "m_zone_owner"]
          and blob[-8:] == b"\xff\xff\xff\xff\x00\x00\x00\xff" and r.read(c + STATE_AT) == 3 and r.read(c + S_LINKED),
          f"{names}")
    r.thiscall(vt(b_sponge, 11), b_sponge, 1, 200000)
    r.call(SYMBOLS["sp_pull_pin"], (b_sponge, 200000))
    r.thiscall(vt(b_sponge, 11), b_sponge, 0, 200050)
    b_armed = r.read(b_sponge + S_EXPLODE)
    blob, names, c, c_zone, c_body, read_ok, same = round_trip(b_sponge, 16, "armed")
    check("armed in the hand (state 6, the pin out, not thrown): read back the same, the same bytes again",
          read_ok and same and r.read(c + STATE_AT) == 6 and r.read(c + S_EXPLODE) == b_armed == 203050
          and r.read(c + S_PIN, 1) == 1 and r.read(c + S_PHYSICS) == 0, f"{b_armed}")
    r.word(throw_state + 0x17C, b_sponge)
    hand = translation(-3.0, 2.0, 4.0)
    frame = [0, 0, 0, 0] + [struct.unpack("<I", struct.pack("<f", v))[0] for v in hand]
    r.span(0x33A7BB, 0x33A7D9, {UC_X86_REG_ESI: throw_state, UC_X86_REG_EDI: game.player}, frame)
    b_bullet, spin = r.read(b_body + 0x14), r.allocate(12)
    r.floats(spin, [0.5, -1.0, 2.0])
    r.thiscall(0x357750, spin, b_body)                     # some spin, as a bounce would give it
    r.floats(b_body + 0x100, translation(-1.0, 2.5, 7.0))
    r.thiscall(vt(b_sponge, 29), b_sponge, 50, 200100)
    blob, names, c, c_zone, c_body, read_ok, same = round_trip(b_sponge, 17, "thrown")
    c_bullet = r.read(c_body + 0x14)
    check("thrown (state 6 in the air): m_transform, linear_velocity, angular_velocity written; read back the "
          "body goes into the physics world at m_transform with both velocities, the same bytes again",
          read_ok and same and names[-8:] == ["m_explode_time_ms", "m_is_pin_pulled", "thrown()", "m_transform",
                                              "linear_velocity", "angular_velocity", "m_released_before_pin",
                                              "m_zone_owner"]
          and r.read(c + S_PHYSICS) == game.physics and ("pw_add", c_body, 0x818, 8) in game.log
          and r.readf(c + S_TRANSFORM, 16) == translation(-1.0, 2.5, 7.0)
          and r.bytes(c_bullet + 0x140, 12) == r.bytes(b_bullet + 0x140, 12)
          and r.bytes(c_bullet + 0x150, 12) == r.bytes(b_bullet + 0x150, 12) and r.readf(c_bullet + 0x150) == 0.5,
          f"{names[-6:]}")
    again_blob = write(b_sponge)[0]
    game.log.clear()
    load(c, again_blob)
    check("loading a thrown state into one already thrown: the body moved there (set transform, the world's "
          "update), not added again", not [e for e in game.log if e[0] == "pw_add"]
          and ("pw_update", c_body + 0x180) in game.log and ("body_set", c_body, [-1.0, 2.5, 7.0]) in game.log
          and r.readf(c + S_TRANSFORM, 16) == translation(-1.0, 2.5, 7.0), str(game.log))
    time = 200100
    while time < b_armed:
        time += 50
        r.thiscall(vt(b_sponge, 29), b_sponge, 50, time)
    r.thiscall(vt(b_sponge, 29), b_sponge, 50, time + 50)
    blob, names, c, c_zone, c_body, read_ok, same = round_trip(b_sponge, 18, "zone")
    check("the zone (state 7): the thrower's place (ours, from the arming), then the zone's serializable (+130h) "
          "and the explosion point (0.28's geometry transform: m_transform, the zone hits' position); read back "
          "the zone is on, read, placed there, m_transform the explosion point, the thrower kept, the same bytes "
          "again",
          read_ok and same and names[-3:] == ["m_released_before_pin", "m_zone_owner",
                                              "m_damage_zone->get_collision_geometry(0)->transform()"]
          and r.readf(c + S_TRANSFORM + 0x30, 3) == [-1.0, 2.5, 7.0]
          and b"ZONE!" in blob and ("activate", c_zone + 0x108, game.physics) in events
          and ("zone_deserialize", c_zone + 0x130, b"ZONE!") in zone_io and ("zone_after", c_zone + 0x130, CORE) in zone_io
          and r.read(c + S_OWNER, 1) == 3 and r.read(c + STATE_AT) == 7
          and r.readf(r.read(c_zone + 0x43C) + 0x30, 3) == [-1.0, 2.5, 7.0], f"{names[-3:]} {zone_io}")
    r.word(c_zone + 0x124, 1)
    events.clear()
    load(c, round_trip(b_sponge, 13, "x")[0] if False else write(b_sponge)[0])
    t2 = time + 50
    while t2 < b_armed + 10000:
        t2 += 50
        r.thiscall(vt(b_sponge, 29), b_sponge, 50, t2)
    blob, names, c2, c2_zone, c2_body, read_ok, same = round_trip(b_sponge, 19, "after")
    events.clear()
    load(c, blob)
    check("after (state 0): read back the same, the same bytes again; read into one whose zone is on, the zone "
          "goes off (0x319D30(zone, 1))", read_ok and same and r.read(c2 + STATE_AT) == 0
          and ("zone_off", c_zone, 1) in events and r.read(c + STATE_AT) == 0, str(events))

    # 10. The draw (the drawable's slot 0): state 6 the model at the hand / the body, else the stock draw's.
    hand_matrix = translation(0.4, 1.7, 0.2)
    r.stub(lambda r: (r.floats(r.arg(0), hand_matrix), r.arg(0))[1], 4, at=0x309F00)
    d_sponge, d_zone, d_body = make_sponge(20)
    drawable = d_sponge + 0x340
    draw = r.read(r.read(drawable))
    model = r.read(drawable + 0x14) + 0x108
    r.word(drawable + 0x10, 3)
    game.log.clear()
    r.thiscall(draw, drawable)
    check("held (state 3): the stock draw (no model calls)", not [e for e in game.log if "model" in e[0]])
    r.thiscall(vt(d_sponge, 11), d_sponge, 1, 300000)
    r.call(SYMBOLS["sp_pull_pin"], (d_sponge, 300000))
    r.thiscall(vt(d_sponge, 11), d_sponge, 0, 300000)
    game.log.clear()
    r.thiscall(draw, drawable)
    r.thiscall(draw, drawable)
    models = [e for e in game.log if "model" in e[0]]
    check("armed in the hand (state 6): the model added at the hand's transform (0x309F00), then updated "
          "(scene_renderer add_model / update_model, the renderer and the scene's +4)",
          [m[0] for m in models] == ["add_model", "update_model"] and all(
              m[1] == model and m[2] == game.renderer and m[3] == game.scene + 4 and m[4] == [f32(0.4), f32(1.7), f32(0.2)]
              for m in models) and r.read(drawable + 0x10) == 6, str(models))
    r.word(throw_state + 0x17C, d_sponge)
    r.span(0x33A7BB, 0x33A7D9, {UC_X86_REG_ESI: throw_state, UC_X86_REG_EDI: game.player},
           [0, 0, 0, 0] + [struct.unpack("<I", struct.pack("<f", v))[0] for v in translation(1.0, 1.0, 1.0)])
    r.floats(d_body + 0x100, translation(2.0, 1.0, 3.0))
    r.thiscall(vt(d_sponge, 29), d_sponge, 50, 300050)
    game.log.clear()
    r.thiscall(draw, drawable)
    models = [e for e in game.log if "model" in e[0]]
    check("in the air: the model where the body is", [(m[0], m[4]) for m in models] == [("update_model", [2.0, 1.0, 3.0])],
          str(models))
    t3 = 300050
    while r.read(d_sponge + STATE_AT) == 6:
        t3 += 50
        r.thiscall(vt(d_sponge, 29), d_sponge, 50, t3)
    game.log.clear()
    r.thiscall(draw, drawable)
    models = [e for e in game.log if "model" in e[0]]
    check("exploded (state 7): the model taken away (remove_model), the stock draw then on (it last drew 7)",
          [m[0] for m in models] == ["remove_model"] and models[0][1:4] == (model, game.renderer, game.scene + 4)
          and r.read(drawable + 0x10) == 7, str(models))

    # 11. Death and back: the base tick (states 0..5) takes the passive off and puts state 5; +38h brings it back.
    alive = make_sponge(21)[0]
    r.emu.mem_write(game.player + 0x33C, b"\x00")
    r.thiscall(vt(alive, 29), alive, 50, 400000)
    dead_ok = r.read(alive + STATE_AT) == 5 and r.read(alive + S_LINKED) == 0
    r.emu.mem_write(game.player + 0x33C, b"\x01")
    r.thiscall(vt(alive, 14), alive)
    check("the holder dead: state 5, the passive out (the base tick 0x313200); back (+38h, 0.28's 0x515190): "
          "state 3, the passive in", dead_ok and r.read(alive + STATE_AT) == 3 and r.read(alive + S_LINKED))
    r.word(alive + STATE_AT, 3)
    r.thiscall(vt(alive, 14), alive)
    check("+38h from any state but 5 changes nothing (0.26g's base would put state 3 from anywhere)",
          r.read(alive + STATE_AT) == 3)

    # 12. Patch 54's one press for every ready artefact of a kind: never a sponge.
    lifebone_like = r.allocate(0x400)
    r.emu.mem_write(lifebone_like, r.bytes(alive, 0x400))
    r.word(lifebone_like, 0xA06124)                       # an artefact of another kind (Rattle's vtable), kind 1
    r.word(lifebone_like + 0x24C, 1)
    r.word(lifebone_like + STATE_AT, 3)
    r.word(lifebone_like + 0x28C, 0)
    r.word(game.inventory + 0x10C + 4 * 16, lifebone_like)
    other = make_sponge(17)[0]
    r.word(game.inventory + 0x10C + 4 * 18, other)
    r.word(game.inventory + 0x17C, 0x17)
    r.stub(lambda r: None, 8, at=0x313350)               # the activation's effect (the artefact, the time)
    r.thiscall(0x313300, lifebone_like, 1, 500000)
    untouched = r.read(other + S_EXPLODE) == NEVER and r.read(other + STATE_AT) == 3 \
        and r.read(game.inventory + 0x17C) == 0x17
    r.thiscall(vt(alive, 11), alive, 1, 500000)
    check("another kind's press (patch 54's fire_kind at 0x313300) fires none of the sponges; a sponge's press "
          "(its own +2Ch) takes that one sponge into the grenade slot, no other",
          untouched and r.read(game.inventory + 0x17C) == r.read(alive + SLOT) and r.read(other + S_EXPLODE) == NEVER
          and r.read(other + STATE_AT) == 3 and r.read(lifebone_like + STATE_AT) == 4)

    # 14. Lifetimes and lockstep (the review of 2026-10-22): each hole of 0.28's closed.
    infection = game.pools["infection"]

    def in_pool(node):
        found, at = 0, r.read(infection + 0x2EC)
        while at and found < 64:
            found += at == node
            at = r.read(at + 0x20)
        return found

    def in_scene(drawable):
        at, seen = r.read(game.scene + 0xDC + 4), []
        while at and len(seen) < 64:
            seen.append(at)
            at = r.read(at + 8)
        return seen.count(drawable)

    def remove_from_slot(slot):
        r.call(0x2FC680, regs={UC_X86_REG_EAX: slot, UC_X86_REG_ESI: game.inventory})

    def tap(obj, at):
        """press and release in one tick, the pull after them"""
        r.thiscall(vt(obj, 11), obj, 1, at)
        r.thiscall(vt(obj, 11), obj, 0, at)

    def throw(obj, x, y, z):
        r.word(throw_state + 0x17C, obj)
        r.span(0x33A7BB, 0x33A7D9, {UC_X86_REG_ESI: throw_state, UC_X86_REG_EDI: game.player},
               [0, 0, 0, 0] + [struct.unpack("<I", struct.pack("<f", v))[0] for v in translation(x, y, z)])

    def tick_until(obj, start, state):
        t = start
        while r.read(obj + STATE_AT) != state and t < start + 20000:
            t += 50
            r.thiscall(vt(obj, 29), obj, 50, t)
        return t

    # 14a. Out of the inventory while armed in the hand: the passive out of the pool (0.28 left it in);
    #      the sponge falls from the hand, its fuse and thrower kept, and explodes where it fell, his.
    a, a_zone, a_body = make_sponge(13)
    r.thiscall(vt(a, 11), a, 1, 600000)
    r.call(SYMBOLS["sp_pull_pin"], (a, 600000))
    r.thiscall(vt(a, 11), a, 0, 600000)
    a_fuse = r.read(a + S_EXPLODE)
    was_in = in_pool(a + S_NODE)
    game.log.clear()
    remove_from_slot(13)
    a_bullet = r.read(a_body + 0x14)
    check("taken out of its inventory armed in the hand (0x2FC680): the passive out of the holder's pool (0.28's "
          "+30h skipped state 6 and left it there); it falls from the hand (into the physics world there, no "
          "speed), still armed (the same fuse), the thrower kept",
          was_in == 1 and in_pool(a + S_NODE) == 0 and r.read(a + S_LINKED) == 0 and r.read(a + INVENTORY) == 0
          and r.read(a + STATE_AT) == 6 and r.read(a + S_EXPLODE) == a_fuse == 603000
          and ("body_set", a_body, [f32(0.4), f32(1.7), f32(0.2)]) in game.log and ("pw_add", a_body, 0x818, 8) in game.log
          and r.read(a + S_PHYSICS) == game.physics and r.readf(a_bullet + 0x140, 3) == [0.0, 0.0, 0.0]
          and r.read(a + S_OWNER, 1) == 3, f"fuse {a_fuse} log {game.log}")
    r.floats(a_body + 0x100, translation(0.4, 0.1, 0.2))
    splashes.clear()
    end = tick_until(a, 600000, 7)
    hit, _, at, when, instigator = splashes[0] if splashes else ({}, 0, [], 0, 0)
    check("it explodes at its fuse where it fell, the splash the thrower's (his game object, his modifiers: "
          "radius 15)", end == a_fuse and len(splashes) == 1 and at == [f32(0.4), f32(0.1), f32(0.2)]
          and instigator == game.player + 0x110 and hit["radius"] == 15.0 and r.read(a + S_INFLICTOR + 8) == 0,
          str(splashes))

    # 14b. Deserialize: the passive follows the state loaded, in and out (0.28 only ever linked it).
    free_one, _, _ = make_sponge(14)
    remove_from_slot(14)
    free_blob = write(free_one)[0]
    held_one, _, _ = make_sponge(15)
    held_blob = write(held_one)[0]
    target_one, _, _ = make_sponge(16)
    linked_first = in_pool(target_one + S_NODE)
    load(target_one, free_blob)
    out_after = (in_pool(target_one + S_NODE), r.read(target_one + S_LINKED), r.read(target_one + STATE_AT))
    load(target_one, held_blob)
    in_after = (in_pool(target_one + S_NODE), r.read(target_one + STATE_AT))
    load(target_one, held_blob)
    check("a rollback's load sets the passive by the state loaded: a held sponge given a state 0 is out of the "
          "pool; given state 3 again it is in once, and once after a second load",
          linked_first == 1 and out_after == (0, 0, 0) and in_after == (1, 3) and in_pool(target_one + S_NODE) == 1,
          f"{linked_first} {out_after} {in_after}")

    # 14c. The zone in a late joiner's world: the explosion point and the thrower from the state, the
    #      instigator found again at each tick from his place, none kept; none for a thrower who left.
    e, e_zone, e_body = make_sponge(17)
    r.thiscall(vt(e, 11), e, 1, 700000)
    r.call(SYMBOLS["sp_pull_pin"], (e, 700000))
    r.thiscall(vt(e, 11), e, 0, 700000)
    throw(e, 1.0, 1.0, 1.0)
    r.floats(e_body + 0x100, translation(6.0, 0.5, -2.0))
    e_end = tick_until(e, 700000, 7)
    blob, names, j, j_zone, j_body, read_ok, same = round_trip(e, 18, "late")
    events.clear()
    r.thiscall(vt(j, 29), j, 50, e_end + 50)
    position, _ = r.thiscall(r.read(r.read(j + S_INFLICTOR) + 8), j + S_INFLICTOR)
    ticked = [x for x in events if x[0] == "zone_tick"]
    check("a late joiner's sponge from a state-7 world: m_transform the explosion point (6, 0.5, -2), its zone "
          "ticked naming the thrower found from his place, the inflictor naming him for the tick and no one "
          "after, its position the explosion point",
          read_ok and same and r.readf(j + S_TRANSFORM + 0x30, 3) == [6.0, 0.5, -2.0]
          and ticked and ticked[0][5] == (j_zone, game.player + 0x110, j + S_INFLICTOR, j)
          and r.read(j + S_INFLICTOR + 8) == 0 and r.readf(position, 3) == [6.0, 0.5, -2.0], str(ticked))
    r.word(CORE + 0x18FD8, ((1 << 20) - 1) & ~(1 << 3))
    events.clear()
    r.thiscall(vt(j, 29), j, 50, e_end + 100)
    gone = [x for x in events if x[0] == "zone_tick"]
    r.word(CORE + 0x18FD8, (1 << 20) - 1)
    r.word(CORE + 0x1845C, game.places + 3 * 0x30)
    events.clear()
    r.thiscall(vt(j, 29), j, 50, e_end + 150)
    past = [x for x in events if x[0] == "zone_tick"]
    r.word(CORE + 0x1845C, game.places + 20 * 0x30)
    events.clear()
    r.thiscall(vt(j, 29), j, 50, e_end + 200)
    back = [x for x in events if x[0] == "zone_tick"]
    check("the thrower's place out of the match (its bit in world+18FD8h cleared, as 0x2D2040 does at a leave) "
          "or past the places' end: the zone's hits name no one; back in: him again",
          gone and gone[0][5][1] == 0 and past and past[0][5][1] == 0 and back and back[0][5][1] == game.player + 0x110,
          f"{gone} {past} {back}")

    # 14d. A tap: the release before the pull; the pull arms it (0.28 threw it unarmed, never to explode).
    t, _, t_body = make_sponge(19)
    tap(t, 800000)
    tapped = (r.read(t + S_EXPLODE), r.read(t + 0x422, 1), r.read(game.inventory + 0x17C))
    tap_blob, _, t2, _, _, read_ok, same = round_trip(t, 20, "tap")
    r.call(SYMBOLS["sp_pull_pin"], (t2, 800050))
    check("a tap (press and release before the pull substate): not armed yet, the release kept (and in the "
          "state: a game loading it has it too); the pull a tick later arms it once, from the pull: 803050, the "
          "thrower its holder",
          tapped == (NEVER, 1, 0x17) and read_ok and same and r.read(t2 + 0x422, 1) == 0
          and r.read(t2 + S_EXPLODE) == 803050 and r.read(t2 + STATE_AT) == 6 and r.read(t2 + S_OWNER, 1) == 3,
          f"{tapped} {r.read(t2 + S_EXPLODE)}")
    r.thiscall(vt(t2, 11), t2, 0, 800100)
    again = r.read(t2 + S_EXPLODE)
    throw(t2, 2.0, 1.0, 2.0)
    t_end = tick_until(t2, 800050, 7)
    r.call(SYMBOLS["sp_pull_pin"], (t, 800000))
    check("a release after that arms nothing again; thrown, it explodes at its fuse; a tap with the pull in the "
          "same tick arms from that tick (803000 from 800000)",
          again == 803050 and t_end == 803050 and r.read(t2 + STATE_AT) == 7 and r.read(t + S_EXPLODE) == 803000,
          f"{again} {t_end} {r.read(t2 + STATE_AT)} {r.read(t + S_EXPLODE)}")

    # 14e. The destructor takes each thing out of what it went into, then destroys it.
    r.stub(lambda r: game.log.append(("rattle_dtor", r.emu.reg_read(UC_X86_REG_ECX), r.arg(0))), 4, at=0x260860)
    r.stub(lambda r: events.append(("destroy_ghost", r.arg(0))), 0, at=0x357BA0)
    r.stub(lambda r: (events.append(("free", r.emu.reg_read(UC_X86_REG_EAX), r.emu.reg_read(UC_X86_REG_ECX))),
                      r.word(r.arg(0), 0)) and None, 0, at=0x1539D0)
    flying, _, f_body = make_sponge(21)
    r.thiscall(vt(flying, 11), flying, 1, 900000)
    r.call(SYMBOLS["sp_pull_pin"], (flying, 900000))
    r.thiscall(vt(flying, 11), flying, 0, 900000)
    throw(flying, 1.0, 2.0, 3.0)
    game.log.clear()
    r.thiscall(vt(flying, 0), flying, 1)
    kinds = [x[0] for x in game.log]
    check("a sponge destroyed in the air: its passive out, its body out of the physics world before the shape "
          "and the body are destroyed, its drawables out of the scene's list, then Rattle's destructor",
          in_pool(flying + S_NODE) == 0 and kinds.index("pw_remove") < kinds.index("destroy_shape")
          < kinds.index("free_body") < kinds.index("rattle_dtor") and ("pw_remove", f_body, 1) in game.log
          and in_scene(flying + 0x340) == 0 and in_scene(r.read(flying + S_ZONE) + 0x408) == 0, str(kinds))
    spent, s_zone, _ = make_sponge(22)
    made = r.read(r.read(s_zone + 0x11C))
    r.thiscall(vt(spent, 11), spent, 1, 900000)
    r.call(SYMBOLS["sp_pull_pin"], (spent, 900000))
    r.thiscall(vt(spent, 11), spent, 0, 900000)
    throw(spent, 1.0, 2.0, 3.0)
    tick_until(spent, 900000, 7)
    r.word(s_zone + 0xD0, 2)
    events.clear()
    game.log.clear()
    r.thiscall(vt(spent, 0), spent, 1)
    order = [x[0] for x in events]
    check("a sponge destroyed with its zone on: the zone off (0x319D30) before its geometry's ghost is destroyed "
          "(0x2F8BE0), the geometry out of the zone and freed to the cooks' allocator it came from, the zone's "
          "drawable out of the scene's list, the zone let go",
          order[:3] == ["zone_off", "destroy_ghost", "free"] and events[0][1:] == (s_zone, 1)
          and events[2][1:] == (made, 0x5A110C) and r.read(r.read(s_zone + 0x11C)) == 0
          and in_scene(s_zone + 0x408) == 0 and in_scene(spent + 0x340) == 0 and r.read(s_zone + 0xD0) == 1,
          str(events))

    # 14f. Nothing takes a spent sponge (state 7): a container's can_use (0x31C6B0) wants state 2; the zone
    #      runs on whatever the item's state, and its end touches only a sponge still in state 7.
    container, user, picker_inv = r.allocate(0x80), r.allocate(4), r.allocate(0x200)
    picker = r.allocate(0x200)
    r.word(user, picker)
    r.word(picker + 0x10C, picker_inv)
    r.emu.mem_write(picker_inv + 0x16C, b"\x02")
    z, z_zone, z_body = make_sponge(23)
    r.thiscall(vt(z, 11), z, 1, 1000000)
    r.call(SYMBOLS["sp_pull_pin"], (z, 1000000))
    r.thiscall(vt(z, 11), z, 0, 1000000)
    throw(z, 1.0, 2.0, 3.0)
    z_end = tick_until(z, 1000000, 7)
    r.word(container + 0x4C, z)
    spent_use, _ = r.thiscall(0x31C6B0, container, user)
    r.word(z + STATE_AT, 2)
    ready_use, _ = r.thiscall(0x31C6B0, container, user)
    r.word(z + STATE_AT, 3)                             # as if something had put it in a hand
    events.clear()
    t_z = z_end
    while t_z < z_end + 10000:
        t_z += 50
        r.thiscall(vt(z, 29), z, 50, t_z)
    z_ticks = [x for x in events if x[0] == "zone_tick"]
    check("a container's can_use: no for the spent sponge (state 7), yes in state 2 (so nothing picks one up "
          "while its zone runs); and were its state changed, the zone still ticks to its end, and the end leaves "
          "that state alone",
          (spent_use & 0xFF) == 0 and (ready_use & 0xFF) == 1 and len(z_ticks) == 200
          and ("zone_off", z_zone, 0) in events and r.read(z + STATE_AT) == 3 and r.read(z + S_EXPLODE) == NEVER,
          f"{spent_use:#x} {ready_use:#x} ticks {len(z_ticks)}")

    # 13. 0.28d0's own sponge code on the same numbers (its layout: artefact_sponge_core, client_patch.md).
    if args.v028d0:
        old = Run(raw_sections(unbcj(open(args.v028d0, "rb").read())))
        check("the 0.28d0 exe decoded (x86 BCJ undone): its sponge's vtable 0x98B850 starts with 0x5138E0, action "
              "0x514580", old.read(0x98B850) == 0x5138E0 and old.read(0x98B850 + 0x28) == 0x514580)
        world28 = old.allocate(0x1D100)
        old.word(world28 + 0x1D05C, 50)
        inventory28, s28 = old.allocate(0x400), old.allocate(0x500)
        old.word(s28 + 0x2C4, world28)
        old.word(s28 + 0x2CC, inventory28)
        old.word(s28 + 0x2D0, 13)
        old.word(s28 + 0x3F0, 3000)
        old.float(s28 + 0x3F4, 15.0)                          # throw_power
        old.word(s28 + 0x460, NEVER)
        old.thiscall(0x514580, s28, 1, 100000)
        pressed = old.read(inventory28 + 0x1F4)
        old28 = []
        for now in (100000, 100001, 100049, 100050, 99999, 30, 0, 100060):
            old.word(s28 + 0x460, NEVER)
            old.emu.mem_write(s28 + 0x464, b"\x01")
            old.thiscall(0x514580, s28, 0, now)
            old28.append(old.read(s28 + 0x460))
        armed28 = (old.read(s28 + 0x2EC), old.read(inventory28 + 0x1F4))
        old.thiscall(0x514580, s28, 1, 100100)
        check("0.28's action 0x514580: the press the same (the grenade slot its slot), the same fuse ends on the "
              "same times, state 6 at the release, a press while armed only the grenade slot 17h",
              pressed == 13 and old28 == fuse + [armed_at] and armed28 == (6, 0x17)
              and old.read(inventory28 + 0x1F4) == 0x17 and old.read(s28 + 0x460) == armed_at, str(old28))
        old.word(s28 + 0x460, NEVER)
        old.emu.mem_write(s28 + 0x464, b"\x00")
        old.thiscall(0x514580, s28, 1, 800000)
        old.thiscall(0x514580, s28, 0, 800000)
        _, cleanup28 = old.thiscall(0x514610, s28 + 0x3C0, 800050)
        check("0.28's hole: a tap (the release, 0x514580, before the pin's pull 0x514610) leaves its sponge "
              "unarmed with the pin out, and the key is already up: thrown, it never explodes; ours arms it at "
              "the pull (14d)",
              old.read(s28 + 0x460) == NEVER and old.read(s28 + 0x464, 1) == 1 and cleanup28 == 4,
              f"{old.read(s28 + 0x460):#x}")
        old.floats(s28 + 0x3D4, [0.0, 1.0, 0.0, 1.0, 1.5, 1.0])
        old_regen = []
        for speed, timeout, threshold in regen_cases:
            info28 = old.allocate(0x10)
            old.emu.mem_write(info28, struct.pack("<fIf", speed, timeout, threshold))
            old.thiscall(0x514380, s28, 0, info28)
            old_regen.append(struct.unpack("<fIf", old.bytes(info28, 12)))
        check("0.28's passive callback 0x514380 on the same regeneration infos: what Lifebone's 0x312590 gives",
              old_regen == regen, str(old_regen))
        # the throw: grenade_set_base[2] 0x514620 and its launch 0x5146E0
        log28 = []
        player28, profile28, owner28 = old.allocate(0x1000), old.allocate(0x2400), old.allocate(0x10)
        old.word(player28 + 0x900, profile28)
        for index in range(0x40):
            old.emu.mem_write(profile28 + 0xF4 + 0x110 + 0x14 * index + 0x10, b"\x01")
        old.float(profile28 + 0xF4 + 4 * 27, 0.2)
        old.word(owner28, old.vtable({3: (lambda r: player28, 0)}))
        old.word(inventory28 + 0x1EC, owner28)
        zone28, body28, bullet28, physics28 = old.allocate(0x300), old.allocate(0x40), old.allocate(0x200), old.allocate(0x10)
        old.word(s28 + 0x4B0, zone28)
        old.word(s28 + 0x3C0, 0x97B8D0)
        old.word(s28 + 0x468, body28)
        old.word(body28, old.vtable({6: (lambda r: log28.append(("body_set", r.readf(r.arg(0) + 0x30, 3))), 4)}))
        old.word(body28 + 0x14, bullet28)
        old.word(physics28, old.vtable({11: (lambda r: log28.append(("pw_add", r.arg(1), r.arg(2))), 0xC)}))
        old.word(world28 + 0x1D044, physics28)
        old.emu.mem_write(s28 + 0x2DC, b"\x01\x00")
        hand28, aim28 = old.allocate(0x40), old.allocate(0x40)
        old.floats(hand28, translation(5.2, 1.5, 7.1))
        old.floats(aim28, aim)
        _, cleanup = old.thiscall(0x514620, s28 + 0x3C0, hand28, aim28, 0)
        check("0.28's throw 0x514620 (and launch 0x5146E0) on the same hand, aim and throw modifier: the body at "
              "the hand, into its physics world (818h, 8), the same linear velocity in its Bullet body (+140h, z "
              "negated) and no spin, the count 0; ret 0Ch",
              cleanup == 0xC and log28 == [("body_set", [f32(5.2), 1.5, f32(7.1)]), ("pw_add", 0x818, 8)]
              and old.bytes(bullet28 + 0x140, 12) == r.bytes(bullet + 0x140, 12)
              and old.readf(bullet28 + 0x150, 3) == [0.0, 0.0, 0.0] and old.read(s28 + 0x2DC, 2) == 0,
              f"{log28} {old.readf(bullet28 + 0x140, 3)}")
        # the splash's damage by distance (0x5412A7..0x5412DC: f = (r - d) / r, min + (max - min) f)
        params28 = old.allocate(0x40)
        old.floats(params28 + 0x28, [15.0, f32(f32(0.05) * f32(1.1)), f32(1.5 * f32(1.1))])
        stop = []
        for end in (0x5412DC, 0x54152E):
            old.emu.hook_add(UC_HOOK_CODE, lambda emu, address, size, user: (stop.append(address), emu.emu_stop()),
                             begin=end, end=end)
        old_damage = []
        from unicorn.x86_const import UC_X86_REG_XMM0, UC_X86_REG_XMM1, UC_X86_REG_XMM3
        for d, _ in splash_inputs:
            stop.clear()
            # at 0x54128E: xmm3, xmm1, xmm0 the position's differences from the centre (x, y, z)
            old.emu.reg_write(UC_X86_REG_XMM3, struct.unpack("<I", struct.pack("<f", d))[0])
            old.emu.reg_write(UC_X86_REG_XMM1, 0)
            old.emu.reg_write(UC_X86_REG_XMM0, 0)
            old.emu.reg_write(UC_X86_REG_EBP, params28)
            old.emu.reg_write(UC_X86_REG_ESP, STACK + 0x8000)
            old.start(0x54128E, 0x5412DD)
            value = old.emu.reg_read(UC_X86_REG_XMM1) & 0xFFFFFFFF
            old_damage.append(struct.unpack("<f", struct.pack("<I", value))[0] if stop[:1] == [0x5412DC] else None)
        ours = [dmg for _, dmg in splash_inputs]
        check("0.28's splash damage by distance (its own code, 0x54128E..0x5412DC) at the six distances: what the "
              "game's 0x339630 gave each target here (none past the radius)",
              old_damage == ours, f"0.28 {old_damage} ours {ours}")
    else:
        print("  --  0.28d0's code not run (no --v028d0)")

    failed = results.count(False)
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
