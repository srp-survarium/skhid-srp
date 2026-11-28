#!/usr/bin/env python3
"""Patch 156 (the Rust patch collision_view, rust/patches/src/collision_view.rs):
the console command `collision` (client_patch.md section 3.112).

    python check_collision_view_026g.py <original survarium.exe>

Requires Python `unicorn`. check_console_where_026g.py's harness (the exe's
sections and the arena loaded as they are, the log's helper write stubbed),
the original beside the build. Stubbed: kernel32's VirtualAlloc (its
import slot 0x87015C: fresh zeroed room of ours, `ret 10h`), system_renderer::draw_lines
(0x11CD10: its arguments and the vertices kept, `ret 14h`) and
btBvhTriangleMeshShape::processAllTriangles (0xA8660: its shape, callback
and box kept, then x86 of ours that does what its node callback does with
each triangle of a made-up mesh, `call [vtable+4]` thiscall (triangle,
part, index) with the triangle's three btVector3, `ret 0Ch`). The game's
own code runs the rest: on_configs_loaded's start, the console's execute
(0xEB980), update_camera (0x1F3480 to its call of 0x243440) over a made-up
physics world laid out as the game's (bullet_physics_world +3Ch its Bullet
world; an object's m_worldTransform +10h, proxy +0C8h, shape +0CCh, flags
+0D8h; a proxy's group +4 and box; a convex shape's type +4, scaling +10h,
dimensions +20h, margin +30h, up axis +40h; a compound's children +10h/+18h,
50h each), and renderer::render's span at `render.debug_lines` (0x13D9C1 to
0x13D9CB). Checked:

- `collision` registered at configs.start, its vtable cc_delegate's but
  execute; the original says "unknown command";
- `collision`: "collision: on"; the next update_camera prints the census, a
  line a group; the render site then draws, through draw_lines (technique
  0, the system renderer, 32768 vertices a call at most, indices 0..n, the
  lines' room VirtualAlloc'd once), the
  level's static objects within 20 m of the eye in the game's frame (z
  negated), a colour a group: a turned box's twelve edges, a compound's
  sphere, cylinder and capsule where its pose and theirs put them, and the
  edges of a mesh's triangles near the eye (asked of processAllTriangles
  with the eye's box in the mesh's space, through a copy of
  btConvexTriangleCallback's vtable, processTriangle ours); never a hit
  model (40h, even static), a controller, a battery, anything moving, far
  or of another shape type; the span's registers
  as the original's;
- within 20 m: a strip's two triangles 15 m away, their shared edge once,
  one 26 m away not (the cap's step-down and the 32768-vertex draws are the
  crate's tests: in Unicorn a frame of more than about 7 000 lines loses a
  carry flag when it re-executes a store, a false bounds panic);
- the lines drawn every render frame until the next read, none while no
  match's world is loaded; no read while the eye stays put;
- each end moved toward the eye (undone by the check before it compares);
- `collision shot`: what stops bullets and the camera (shot and solid), the
  census of those; `collision 8`: the usage and the options; `collision
  walk`: switched (walk and solid); again: off; `collision solid`: solid
  alone; `collision` off, on and off; the original never draws;
- the whole view only where the match shown allows it, the options reads
  run whole (process_static_match_info from its pushes through the game's
  own match_options::deserialize past `static_info.options_read`, a replay's
  chunk 1 to its `ret 8`, as check_blackout_026g.py runs them, at addresses
  of this check's own), each as the game's code leaves its registers and
  stack: `collision`, `walk`, `anomaly`, `fill`, `shot fill` refused before
  any options and after a public match's (srp's block without tag 8),
  nothing turned on, no room made, the usage still the usage; allowed after
  a private room's (tag 8, one byte 1, after an unknown tag): every check
  above; a filled view given a public match's options: told once, no read,
  nothing drawn; another filter there refused; another public match's:
  nothing told; a replay's: back, filled; a private room's after it: still
  on, no read; the entry's value 0: a public match's; `collision fill`
  there: off, not refused.
"""
import math
import struct
import sys
from collections import Counter

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base
import check_blackout_026g as blackout
import check_console_where_026g as where
import check_hide_ui_026g as hide
import check_language_026g as language

DRAW_LINES, PROCESS_ALL_TRIANGLES = 0x11CD10, 0xA8660
TRIANGLE_METHODS, TRIANGLE_DTOR, TRIANGLE_PROCESS, TRIANGLE_RTTI = 0x9E4F90, 0xC55B0, 0xC5600, 0xA3D4DC
CYLINDER_VTABLE = 0x9E4B9C
SITE, SITE_END = 0x13D9C1, 0x13D9CB          # render.debug_lines: mov edi,[esp+1Ch] / mov edx,[edi+6334C4h]
SYSTEM_RENDERER_SLOT = 0x2260480
PHYSICS = 0x60000000                         # the physics world, its Bullet world, objects, shapes
WORLD, ARRAY, OBJECTS, SHAPES = PHYSICS + 0x100, PHYSICS + 0x1000, PHYSICS + 0x10000, PHYSICS + 0x80000
SYSTEM_RENDERER = 0x61000000
LINES = 0x62000000                           # where the renderer's +6334C4h is read
RENDERER = LINES - 0x6334C4
STUB, TRIANGLES = 0x63000000, 0x63001000     # processAllTriangles' stand-in and its mesh
STUB_CODE = bytes.fromhex("5356578b742410bb0010006331ff3b3b731a89f8c1e0048d04408d440310576a005089f18b16ff520447"
                          "ebe25f5e5bc20c00")
EYE = (10.0, 1.7, 20.0)                      # the game's frame; Bullet's (10, 1.7, -20)
GREEN, MAGENTA, WHITE = 0xFF00FF00, 0xFFFF00FF, 0xFFFFFFFF
TURNED = ((0.0, 0.0, 1.0), (0.0, 1.0, 0.0), (-1.0, 0.0, 0.0))     # a quarter turn about y
IDENTITY = ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))
MOST = 131072                                # lines: a room's 256Ki vertices
DRAW_TRIANGLES = 0x11E370
HELP = ["  collision walk    - what stops walking (green + white)",
        "  collision shot    - what stops bullets and the camera (magenta + white)",
        "  collision solid   - what stops both (white)",
        "  collision anomaly - anomaly damage zones (red, dim when off)",
        "  ... fill          - after any of these (or alone): surfaces filled, to find holes",
        "  collision         - again to turn off"]
ON = ["collision: on, all (walk green, shot magenta, solid white, anomaly red)"] + HELP
RED, DIM_RED = 0xFF0000FF, 0xFF000060
ZONES = PHYSICS + 0xC0000                    # the project, its damage zones, their geometries and ghosts
VIRTUAL_ALLOC_SLOT, ROOM = 0x87015C, 0x70000000
OPTIONS, WORK = 0x64000000, 0x64100000         # a match's options and the message they are read from


class Run(where.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(PHYSICS, 0x100000, UC_PROT_ALL)
        emu.mem_map(SYSTEM_RENDERER, 0x1000, UC_PROT_ALL)
        emu.mem_map(LINES, 0x1000, UC_PROT_ALL)
        emu.mem_map(STUB, 0x200000, UC_PROT_ALL)
        emu.mem_map(hide.STACK - 0x100000, 0x100000, UC_PROT_ALL)
        emu.mem_write(STUB, STUB_CODE)
        emu.mem_map(ROOM, 0x1000000, UC_PROT_ALL)
        for at, size in ((OPTIONS, 0x60000), (WORK, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.room, self.allocations = ROOM, []
        self.word(VIRTUAL_ALLOC_SLOT, STUB + 0x100)
        emu.mem_write(STUB + 0x100, b"\xC2\x10\x00")
        emu.hook_add(UC_HOOK_CODE, self.virtual_alloc, begin=STUB + 0x100, end=STUB + 0x100)
        self.word(SYSTEM_RENDERER_SLOT, SYSTEM_RENDERER)
        self.draws, self.queries, self.shapes = [], [], 0
        emu.hook_add(UC_HOOK_CODE, self.draw_lines, begin=DRAW_LINES, end=DRAW_LINES)
        self.fills = []
        emu.hook_add(UC_HOOK_CODE, self.draw_triangles, begin=DRAW_TRIANGLES, end=DRAW_TRIANGLES)
        emu.hook_add(UC_HOOK_CODE, self.process_all_triangles, begin=PROCESS_ALL_TRIANGLES, end=PROCESS_ALL_TRIANGLES)

    def options_read(self, block=None):
        """check_blackout_026g's runs of the two options reads, the whole chain of their hooks (its addresses are
        check_language_026g's data here: OPTIONS and WORK ours): a match's options with srp's `block` after them
        (None: a replay's chunk 1), the game's own match_options::deserialize first; the lines told."""
        payload = blackout.empty_options(block or b"")
        self.emu.mem_write(OPTIONS, bytes(0x578A0))
        self.emu.mem_write(WORK, bytes(0x10000))
        self.emu.mem_write(WORK, payload)
        world = hide.WORLD
        self.emu.mem_write(world, bytes(0x10000))
        self.writes = []
        if block is not None:
            self.word(world, world + 0x8000)                  # [edi]: a vtable
            self.word(world + 0x14, world + 0x1000)           # [edi+14h]+53A4h: the dictionary (none)
            ebp = hide.STACK + 0x9000
            for i, value in enumerate((WORK, WORK, len(payload))):
                self.word(ebp - 0x2C + 4 * i, value)
            sp = ebp - 0x100
            regs, esp = self.run(blackout.STATIC_START, (blackout.STATIC_PAST,), sp,
                                 {**hide.GAME, UC_X86_REG_EDI: world, UC_X86_REG_ESI: OPTIONS - 0xC,
                                  UC_X86_REG_EBP: ebp})
            ok = self.stopped == blackout.STATIC_PAST and regs[UC_X86_REG_ESI] == OPTIONS and esp == sp \
                and regs[UC_X86_REG_EBP] == ebp and regs[UC_X86_REG_EDI] == world
        else:
            sp = hide.STACK + 0x8000
            self.emu.mem_write(sp, bytes(0x100))
            self.word(sp + 0x40, OPTIONS)
            self.word(sp + 0x44, 0)
            self.word(sp, 0x55555555)                         # the epilogue's saved esi and ebx
            self.word(sp + 4, 0x66666666)
            regs, esp = self.run(blackout.REPLAY_START, (blackout.REPLAY_RET,), sp,
                                 {**hide.GAME, UC_X86_REG_EAX: WORK, UC_X86_REG_ECX: len(payload),
                                  UC_X86_REG_EDI: world})
            ok = self.stopped == blackout.REPLAY_RET and esp == sp + 0x3C and regs[UC_X86_REG_ESI] == 0x55555555
        assert ok, "the options read run to past the site, the registers and the stack as the game's code leaves them"
        return [w["argument"].decode() for w in self.writes]
    def run(self, start, stops, sp, regs):
        """check_hide_ui_026g's run, with room for a frame of thousands of triangles."""
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.calls, self.invoked = stops, None, False, [], []
        self.emu.emu_start(start, 0)
        return {reg: self.emu.reg_read(reg) for reg in hide.REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def virtual_alloc(self, emu, *_):
        size = self.read(emu.reg_read(UC_X86_REG_ESP) + 8)
        self.allocations.append(size)
        emu.reg_write(UC_X86_REG_EAX, self.room)
        self.room += (size + 0xFFFF) & ~0xFFFF

    def draw_lines(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        end = emu.reg_read(UC_X86_REG_EAX)
        renderer, begin, first, last, technique = (self.read(sp + 4 * i) for i in range(1, 6))
        vertices = [struct.unpack("<3fI", emu.mem_read(at, 16)) for at in range(begin, end, 16)]
        indices = list(struct.unpack(f"<{(last - first) // 2}H", emu.mem_read(first, last - first)))
        self.draws.append(dict(renderer=renderer, technique=technique, vertices=vertices, indices=indices, first=first))
        self.ret(0x14)

    def draw_triangles(self, emu, *_):
        """draw_triangles(renderer, vertices, their end, indices, their end, override byte), `ret 18h`."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        renderer, begin, end, first, last, override = (self.read(sp + 4 * i) for i in range(1, 7))
        vertices = [struct.unpack("<3fI", emu.mem_read(at, 16)) for at in range(begin, end, 16)]
        indices = list(struct.unpack(f"<{(last - first) // 2}H", emu.mem_read(first, last - first)))
        self.fills.append(dict(renderer=renderer, override=override & 0xFF, vertices=vertices, indices=indices))
        self.ret(0x18)

    def process_all_triangles(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        callback = self.read(sp + 4)
        vtable = self.read(callback)
        self.queries.append(dict(
            shape=emu.reg_read(UC_X86_REG_ECX), low=struct.unpack("<3f", emu.mem_read(self.read(sp + 8), 12)),
            high=struct.unpack("<3f", emu.mem_read(self.read(sp + 12), 12)),
            slots=(self.read(vtable - 4), self.read(vtable), self.read(vtable + 4)), vtable=vtable))
        emu.reg_write(UC_X86_REG_EIP, STUB)

    def mesh(self, triangles):
        """The made-up mesh every processAllTriangles reports: `triangles`, three points each."""
        self.word(TRIANGLES, len(triangles))
        blob = b"".join(struct.pack("<4f", *p, 0.0) for t in triangles for p in t)
        self.emu.mem_write(TRIANGLES + 16, blob)

    def shape(self, kind, dims=(0.0, 0.0, 0.0), margin=0.0, scaling=(1.0, 1.0, 1.0), up=1, vtable=0x9E4AFC,
              children=()):
        at = SHAPES + 0x400 * self.shapes
        self.shapes += 1
        self.word(at, vtable)
        self.word(at + 4, kind)
        self.emu.mem_write(at + 0x10, struct.pack("<4f", *scaling, 0.0))
        self.emu.mem_write(at + 0x20, struct.pack("<4f", *dims, 0.0))
        self.emu.mem_write(at + 0x30, struct.pack("<f", margin))
        self.word(at + 0x40, up)
        if children:
            table = at + 0x100
            self.word(at + 0x10, len(children))
            self.word(at + 0x18, table)
            for i, (rows, origin, child) in enumerate(children):
                self.emu.mem_write(table + 0x50 * i, transform(rows, origin))
                self.word(table + 0x50 * i + 0x40, child)
        return at

    def world(self, objects):
        """The physics world: `objects` (group, flags, rows, origin, shape; None: no proxy)."""
        self.emu.mem_write(PHYSICS, bytes(0x80000))
        self.word(PHYSICS + 0x3C, WORLD)
        self.word(WORLD + 0x8, len(objects))
        self.word(WORLD + 0x10, ARRAY)
        for i, item in enumerate(objects):
            obj = OBJECTS + 0x200 * i
            self.word(ARRAY + 4 * i, obj)
            if item is None:
                continue
            group, flags, rows, origin, shape = item
            proxy = obj + 0x100
            self.emu.mem_write(obj + 0x10, transform(rows, origin))
            self.word(obj + 0xC8, proxy)
            self.word(obj + 0xCC, shape)
            self.word(obj + 0xD8, flags)
            self.emu.mem_write(proxy + 4, struct.pack("<HH", group, 0xFFFF))
            self.emu.mem_write(proxy + 0x10, struct.pack("<4f", *(c - 40.0 for c in origin), 0.0))
            self.emu.mem_write(proxy + 0x20, struct.pack("<4f", *(c + 40.0 for c in origin), 0.0))

    def in_match(self):
        """A match's world shown: the engine world's game in a match, its game world active (+0D4h)."""
        self.map("level_01/day_2", map_id=1)
        self.emu.mem_write(where.GAME + 0x18 + 0xD4, b"\x01")

    def zones(self, zones):
        """The loaded project's damage zones: (active, Bullet origin, shape) each, one geometry a zone (its
        sensor +108h: geometries +14h, count +18h, active +1Ch; a geometry's ghost +18h, the ghost's object +18h)."""
        self.emu.mem_write(ZONES, bytes(0x20000))
        project, table = ZONES, ZONES + 0x400
        self.word(where.GAME + 0x18 + 0x1D88, project)
        self.word(project + 0x19C, table)
        self.word(project + 0x1A0, table + 4 * len(zones))
        for i, (active, origin, shape) in enumerate(zones):
            zone = ZONES + 0x1000 + 0x1000 * i
            geometries, geometry, ghost, obj = zone + 0x200, zone + 0x300, zone + 0x400, zone + 0x500
            self.word(table + 4 * i, zone)
            self.word(zone + 0x11C, geometries)
            self.word(zone + 0x120, 1)
            self.emu.mem_write(zone + 0x124, bytes([int(active)]))
            self.word(geometries, geometry)
            self.word(geometry + 0x18, ghost)
            self.word(ghost + 0x18, obj)
            self.emu.mem_write(obj + 0x10, transform(IDENTITY, origin))
            self.word(obj + 0xCC, shape)

    def frame(self, eye=EYE):
        """update_camera(player) in a spectator's first person at `eye`: the lines it wrote to the log."""
        self.emu.mem_write(where.OBJECTS, bytes(0x40000))
        self.word(where.PLAYER + 0xF470, where.HANDLER)
        self.word(where.PLAYER + 0xF478, where.PRESENTERS)
        self.word(where.PLAYER + 0x350, PHYSICS)
        self.emu.mem_write(where.PLAYER + 0x33C, b"\x01")
        self.emu.mem_write(where.PLAYER + 0x40C, where.transform(0.0, 0.0, eye))
        self.emu.mem_write(where.PLAYER + 0x44C, where.transform(0.5, -0.1, eye))
        self.word(where.HANDLER + 0x358, 4)
        self.word(where.HANDLER + 0x334, where.WORLD)
        self.writes, self.queries = [], []
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, where.PLAYER)
        self.run(where.UPDATE_CAMERA, (where.CAMERA_END,), sp, {**hide.GAME, UC_X86_REG_EBX: 0x0B0B0B0B})
        assert self.stopped == where.CAMERA_END, self.stopped
        return [w["argument"].decode() for w in self.writes]

    def debug_lines(self):
        """renderer::render's span at the site: the draws, and EDI and EDX after it."""
        self.draws = []
        sp = hide.STACK + 0x8000
        self.word(sp + 0x1C, RENDERER)
        self.word(LINES, 0x51525354)
        after, end = self.run(SITE, (SITE_END,), sp, hide.GAME)
        assert self.stopped == SITE_END and end == sp, (self.stopped, end - sp)
        return list(self.draws), (after[UC_X86_REG_EDI], after[UC_X86_REG_EDX])

    def said(self, line):
        return [w["argument"].decode() for w in self.execute(line)[0]]


def transform(rows, origin):
    return b"".join(struct.pack("<4f", *r, 0.0) for r in rows) + struct.pack("<4f", *origin, 0.0)


def apply(rows, origin, p):
    return tuple(sum(rows[k][j] * p[j] for j in range(3)) + origin[k] for k in range(3))


def game(p):
    return (p[0], p[1], -p[2])


def rounded(p):
    return tuple(round(c, 4) for c in p)


def unbias(p):
    """A drawn end where its line is: the patch moves each end toward the eye by 1 cm or 0.5 % of its
    distance, the more (so a line on a surface is drawn over it); this undoes it."""
    d = math.dist(p, EYE)
    if d == 0:
        return p
    was = d + 0.01 if d <= 1.99 else d / 0.995
    return tuple(EYE[k] + (p[k] - EYE[k]) * was / d for k in range(3))


def segments(draws):
    """Every line drawn: (colour, end, end) in the game's frame, unbiased, rounded."""
    out = []
    for d in draws:
        v = d["vertices"]
        for i in range(0, len(v), 2):
            out.append((v[i][3], rounded(unbias(v[i][:3])), rounded(unbias(v[i + 1][:3]))))
    return out


check = where.check


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))

    old.start()
    new.start()
    ours = new.commands().get("collision")
    check("collision" not in old.commands() and ours is not None, "configs.start: `collision` registered; the "
          "original has none")
    vtable = new.read(ours)
    slots = [new.read(vtable - 4 + 4 * i) for i in range(where.VTABLE_SLOTS + 1)]
    stock = [new.read(where.CC_DELEGATE_VTABLE - 4 + 4 * i) for i in range(where.VTABLE_SLOTS + 1)]
    check(slots[:2] + slots[3:] == stock[:2] + stock[3:] and slots[2] != stock[2] and vtable != where.CC_DELEGATE_VTABLE
          and new.read(ours + 0x14) == 2, "its vtable: cc_delegate's RTTI and slots but execute (+4), kind 2")
    head = new.read(language.LIST_HEAD)
    new.start()
    check(new.read(language.LIST_HEAD) == head, "configs.start again: registered once")
    check(old.execute("collision")[0][0]["format"] == b"unknown command [%s]", "the original: unknown command")

    # Bullet's frame (z the game's -z); the eye at Bullet (10, 1.7, -20)
    near = [((9.0, 0.0, -21.0), (11.0, 0.0, -21.0), (10.0, 0.0, -19.0)),
            ((10.0, 3.0, -20.0), (12.0, 3.0, -20.0), (10.0, 4.0, -18.0))]
    far = ((100.0, 0.0, 100.0), (101.0, 0.0, 100.0), (100.0, 0.0, 101.0))
    box_origin = (12.0, 1.0, -22.0)
    compound_origin = (8.0, 0.0, -18.0)
    sphere_at, cylinder_at, capsule_at = (0.0, 1.0, 0.0), (2.0, 0.0, 0.0), (0.0, 0.0, 2.0)
    for r in (old, new):
        r.shapes = 0
        r.mesh(near + [far])
        box = r.shape(0, dims=(0.96, 1.96, 2.96), margin=0.04)
        sphere = r.shape(8, dims=(0.5, 0.0, 0.0), scaling=(2.0, 2.0, 2.0))
        cylinder = r.shape(13, dims=(0.46, 0.96, 0.46), margin=0.04, vtable=CYLINDER_VTABLE)
        capsule = r.shape(10, dims=(0.3, 0.6, 0.3), up=1)
        mesh = r.shape(21)
        compound = r.shape(31, children=((IDENTITY, sphere_at, sphere), (IDENTITY, cylinder_at, cylinder),
                                         (IDENTITY, capsule_at, capsule), (IDENTITY, (-8.0, 0.0, 18.0), mesh)))
        other = r.shape(24)
        r.in_match()
        zone_ball = r.shape(8, dims=(1.5, 0.0, 0.0), scaling=(2.0, 2.0, 2.0))   # radius 3
        zone_box = r.shape(0, dims=(1.0, 1.0, 1.0))
        r.zones([(True, (14.0, 0.0, -20.0), zone_ball), (False, (6.0, 0.0, -20.0), zone_box)])
        r.world([(0x2, 1, TURNED, box_origin, box),
                 (0x8, 1, IDENTITY, compound_origin, compound),
                 (0xA, 1, IDENTITY, (0.0, 0.0, 0.0), mesh),
                 (0x40, 1, IDENTITY, (10.0, 1.0, -20.0), box),       # a hit model, static
                 (0x824, 0x10, IDENTITY, (10.0, 1.0, -20.0), box),   # a character controller
                 (0xA18, 0, IDENTITY, (10.0, 1.0, -20.0), box),      # a battery
                 (0x2, 0, IDENTITY, (10.0, 1.0, -20.0), box),        # a walker group's, moving
                 (0x2, 1, IDENTITY, (80.0, 1.0, -20.0), box),        # its box 30 m away
                 (0x2, 1, IDENTITY, (10.0, 1.0, -20.0), other),      # another shape type
                 None])
    drawn_old, regs_old = old.debug_lines()
    drawn_new, regs_new = new.debug_lines()
    check(drawn_old == [] and drawn_new == [] and regs_old == regs_new == (RENDERER, 0x51525354),
          "off: nothing drawn; the span's EDI and EDX the original's")

    # the view only where the match shown allows it: none read yet, a public match's options, then a private room's
    refused = ["collision: only in private rooms and replays"]
    asks = ("collision", "collision walk", "collision anomaly", "collision fill", "collision shot fill")
    said = [new.said(line) for line in asks]
    check(all(s == refused for s in said) and new.frame() == [] and new.allocations == []
          and new.debug_lines()[0] == [],
          f"before any options read: `collision`, `walk`, `anomaly`, `fill`, `shot fill` each \"collision: only in "
          f"private rooms and replays\", nothing turned on, no room made, no read, nothing drawn ({said[0]})")
    check(new.said("collision 8")[0] == "collision: collision [walk | shot | solid | anomaly] [fill]",
          "a wrong argument there: the usage, as anywhere")
    check(new.options_read(blackout.srpx((4, b"\x2c\x01"))) == [] and new.said("collision") == refused
          and new.said("collision solid fill") == refused and new.frame() == [] and new.allocations == [],
          "a public match's options (srp's block without the private room entry): refused")
    check(new.options_read(blackout.srpx((9, b"xyz"), (8, b"\x01"))) == [],
          "a private room's options (tag 8, one byte 1, after an unknown tag): nothing told")

    check(new.said("collision") == ON, "collision through console_commands::execute: \"collision: on, all\" and the "
          "four options' lines")
    lines = new.frame()
    old.frame()
    check(new.allocations == [2 * MOST * 16, 2 * MOST * 16, 32768 * 2, 32766 * 2 * 2] and old.allocations == [],
          "turned on: the two rooms (shown and spare, 4 MiB of vertices each) and their shared indices (64 KiB for the "
          "lines, 128 KiB for the fill), VirtualAlloc'd once, outside the lock")
    check(lines == ["collision: the level's static collision within 20 m (less where that is over 131072 lines):",
                    "collision: walk (green) 3 in the world: stops walkers only",
                    "collision: shot (magenta) 1 in the world: stops bullets and the camera only",
                    "collision: solid (white) 1 in the world: stops walkers, bullets and the camera",
                    "collision: anomaly (red) 2 in the world: damage zones, dim red while off"],
          "the first frame: the census, a line a group: " + " | ".join(lines))
    queries = new.queries
    eye = (10.0, 1.7, -20.0)
    want_box = tuple(e - 20.0 for e in eye) + tuple(e + 20.0 for e in eye)
    check(len(queries) == 2 and {q["shape"] for q in queries} == {mesh}
          and all(all(abs(a - b) < 1e-4 for a, b in zip(q["low"] + q["high"], want_box)) for q in queries)
          and all(q["slots"][0] == TRIANGLE_RTTI and q["slots"][1] == TRIANGLE_DTOR and q["slots"][2] != TRIANGLE_PROCESS
                  and q["vtable"] != TRIANGLE_METHODS for q in queries),
          "a mesh's triangles through processAllTriangles' callback: asked with the eye's box +-20 m in the mesh's "
          "space (the compound's mesh child and the solid mesh), the callback's vtable a copy of "
          "btConvexTriangleCallback's (its RTTI and destructor, processTriangle ours)")

    draws, regs = new.debug_lines()
    got = segments(draws)
    by_colour = Counter(c for c, _, _ in got)
    check(len(draws) == 1 and regs == regs_old and all(
              d["renderer"] == SYSTEM_RENDERER and d["technique"] == 0 and d["indices"] == list(range(len(d["vertices"])))
              for d in draws)
          and by_colour == {GREEN: 12, MAGENTA: 36 + 28 + 52 + 6, WHITE: 6, RED: 36, DIM_RED: 12},
          f"the render site: one draw_lines (technique 0, the system renderer, indices 0..n) of {len(got)} lines: "
          f"12 green, 122 magenta, 6 white, the active zone's 36 red, the zone off's 12 dim red ({dict(by_colour)}); "
          "the span's registers the original's")
    ball = [p for c, a, b in got if c == RED for p in (a, b)]
    zone_box_points = {rounded(game((6.0 + x, y, -20.0 + z))) for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)}
    check(all(abs(math.dist(p, game((14.0, 0.0, -20.0))) - 3.0) < 1e-3 for p in ball)
          and all(p in zone_box_points for c, a, b in got if c == DIM_RED for p in (a, b)),
          "the anomalies' damage zones where their ghosts stand: the active one's sphere (radius 3) red, the box of the "
          "one off dim red")
    corners = [(x * 1.0, y * 2.0, z * 3.0) for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)]
    box_points = {rounded(game(apply(TURNED, box_origin, p))) for p in corners}
    green = [s for s in got if s[0] == GREEN]
    check(len({(a, b) for _, a, b in green}) == 12 and all(a in box_points and b in box_points for _, a, b in green),
          "the walkers' box: its twelve edges, turned a quarter about y where it stands, in the game's frame")
    tri_edges = Counter((colour, rounded(game(a)), rounded(game(b))) for colour in (MAGENTA, WHITE) for t in near
                        for a, b in ((t[0], t[1]), (t[1], t[2]), (t[2], t[0])))
    check(Counter(s for s in got if s in tri_edges) == tri_edges,
          "the two meshes' near triangles, each edge once a mesh, magenta and white; the far triangle not")

    def around(centre, test):
        c = game(apply(IDENTITY, compound_origin, centre))
        return lambda p: test(tuple(p[k] - c[k] for k in range(3)))

    sphere_ok = around(sphere_at, lambda d: abs(math.dist(d, (0, 0, 0)) - 1.0) < 1e-3)
    cylinder_ok = around(cylinder_at, lambda d: abs(math.hypot(d[0], d[2]) - 0.5) < 1e-3 and abs(d[1]) <= 1.0 + 1e-4)
    capsule_ok = around(capsule_at, lambda d: abs(math.dist(d, (0, max(-0.6, min(0.6, d[1])), 0)) - 0.3) < 1e-3)
    ends = [p for c, a, b in got if c == MAGENTA and (c, a, b) not in tri_edges for p in (a, b)]
    kinds = Counter("sphere" if sphere_ok(p) else "cylinder" if cylinder_ok(p) else "capsule" if capsule_ok(p)
                    else "none" for p in ends)
    check(kinds == {"sphere": 72, "cylinder": 56, "capsule": 104},
          f"the compound's children where their poses put them: the sphere (36 lines on its radius 1), the cylinder "
          f"(28 on radius 0.5 within its half height 1), the capsule (52 at 0.3 from its segment): {dict(kinds)}")
    check(segments(new.debug_lines()[0]) == got and old.debug_lines()[0] == [],
          "drawn again the next render frame, the same lines; the original never")
    check(new.frame() == [] and new.queries == [] and segments(new.debug_lines()[0]) == got,
          "the next camera at the same eye: no read, no census, the same lines")
    raw = draws[0]["vertices"][0][:3]
    put = unbias(raw)
    check(abs(math.dist(put, EYE) - math.dist(raw, EYE) - max(0.01, 0.005 * math.dist(put, EYE))) < 1e-4,
          "each end drawn nearer the eye by 1 cm or 0.5 % of its distance, the more")
    moved = [(dx, bool(new.frame((EYE[0] + dx, EYE[1], EYE[2])) or True) and len(new.queries)) for dx in (0.4, 0.6)]
    check(moved == [(0.4, 0), (0.6, 2)], f"the eye 0.4 m on: no read; 0.6 m on: read again (both meshes) {moved}")
    new.frame()                             # back at EYE: 0.6 m from that read, read again
    new.map(None)
    lobby = new.frame((EYE[0] + 3.0, EYE[1], EYE[2]))
    check(new.debug_lines()[0] == [] and lobby == [] and new.queries == [],
          "no match's world shown (the lobby): nothing drawn, no read even 3 m on")
    new.in_match()

    # 20 m: a triangle 15 m away drawn, one 25 m away not; a strip's shared edge drawn once
    a, b, c, d = (25.0, 1.0, -20.0), (25.0, 2.0, -20.0), (25.5, 1.0, -20.0), (25.5, 2.0, -20.0)
    beyond = ((36.0, 1.0, -20.0), (36.0, 2.0, -20.0), (36.5, 1.0, -20.0))
    new.mesh([(a, b, c), (b, c, d), beyond])
    new.world([(0x2, 1, IDENTITY, (0.0, 0.0, 0.0), mesh)])
    new.said("collision")
    new.said("collision")                   # off and on: read again
    new.frame()
    draws, _ = new.debug_lines()
    edges = {frozenset((p, q)) for _, p, q in segments(draws)}
    want = {frozenset((rounded(game(p)), rounded(game(q)))) for p, q in ((a, b), (b, c), (c, a), (c, d), (d, b))}
    check([round(q["high"][0] - eye[0], 3) for q in new.queries] == [20.0] and len(segments(draws)) == 5
          and edges == want,
          "within 20 m: a strip of two triangles 15 m away, its shared edge drawn once (5 lines); 26 m away not")
    new.mesh(near + [far])
    new.world([(0x2, 1, TURNED, box_origin, box), (0x8, 1, IDENTITY, compound_origin, compound),
               (0xA, 1, IDENTITY, (0.0, 0.0, 0.0), mesh)])
    new.zones([(True, (14.0, 0.0, -20.0), zone_ball), (False, (6.0, 0.0, -20.0), zone_box)])

    def colours():
        return Counter(c for c, _, _ in segments(new.debug_lines()[0]))

    check(new.said("collision shot") == ["collision: on, shot (magenta + white)"],
          "collision shot: \"collision: on, shot (magenta + white)\"")
    census = new.frame()
    check(census[1:] == ["collision: shot (magenta) 1 in the world: stops bullets and the camera only",
                         "collision: solid (white) 1 in the world: stops walkers, bullets and the camera"]
          and colours() == {MAGENTA: 122, WHITE: 6},
          "collision shot: what stops bullets and the camera, the shot compound and the solid mesh; the census of those")
    check(new.said("collision 8") == ["collision: collision [walk | shot | solid | anomaly] [fill]"] + HELP
          and new.frame() == [] and colours() == {MAGENTA: 122, WHITE: 6},
          "collision 8: the usage with the options, nothing changed")
    check(new.said("collision walk") == ["collision: on, walk (green + white)"] and len(new.frame()) == 3
          and colours() == {GREEN: 12, WHITE: 6},
          "collision walk: switched, what stops walkers, the walk box and the solid mesh")
    check(new.said("collision walk") == ["collision: off"] and new.frame() == [] and new.debug_lines()[0] == [],
          "collision walk again: off, nothing drawn")
    check(new.said("collision solid") == ["collision: on, solid (white)"] and len(new.frame()) == 2
          and colours() == {WHITE: 6}, "collision solid: the solid mesh alone")
    check(new.said("collision anomaly") == ["collision: on, anomaly (red, dim when off)"]
          and new.frame()[1:] == ["collision: anomaly (red) 2 in the world: damage zones, dim red while off"]
          and colours() == {RED: 36, DIM_RED: 12}, "collision anomaly: the damage zones alone")
    new.emu.mem_write(ZONES + 0x1000 + 0x124, b"\x00")    # the active zone goes off
    new.emu.mem_write(ZONES + 0x2000 + 0x124, b"\x01")    # the other comes on
    check(new.frame() == [] and colours() == {DIM_RED: 36, RED: 12},
          "a zone going on and another off: read again at the same eye, their colours swapped")
    check(new.said("collision") == ["collision: off"] and new.frame() == [] and new.debug_lines()[0] == []
          and new.said("collision") == ON and len(new.frame()) == 5
          and new.said("collision") == ["collision: off"] and new.frame() == [] and new.debug_lines()[0] == [],
          "collision from anomaly: off; again: on (all, its census five lines); again: off")

    # fill: the walk box's twelve faces' triangles and the solid mesh's near two, translucent, under the lines
    check(new.said("collision walk fill") == ["collision: on, walk (green + white), filled"],
          "collision walk fill: \"collision: on, walk (green + white), filled\"")
    census = new.frame()
    new.fills = []
    draws, _ = new.debug_lines()
    fills = list(new.fills)
    tris = [v for f in fills for v in f["vertices"]]
    alphas = {v[3] >> 24 for v in tris}
    green_shades = {(v[3] >> 8) & 0xFF for v in tris if v[3] & 0xFF == 0}
    check(len(census) == 3 and Counter(c for c, _, _ in segments(draws)) == {GREEN: 12, WHITE: 6} and len(fills) == 1
          and fills[0]["renderer"] == SYSTEM_RENDERER and fills[0]["override"] == 0 and len(tris) == 3 * (12 + 2)
          and fills[0]["indices"] == [3 * t + k for t in range(14) for k in (0, 1, 2, 0, 2, 1)]
          and alphas == {77} and green_shades == {102, 255},
          f"the fill: one draw_triangles (the system renderer, override 0: the blended technique) of 14 triangles "
          f"(the walk box's 12, the solid mesh's 2), each both ways round, alpha 77, the box's top and bottom full "
          f"green, its sides 0.4 ({sorted(green_shades)}, {len(tris) // 3} triangles); the lines over them as before")
    new.fills = []
    said, census = new.said("collision fill"), new.frame()
    framed, (draws, _) = len(new.fills), new.debug_lines()
    check(said == [ON[0] + ", filled"] + HELP and len(census) == 5 and framed == 0 and draws and len(new.fills) == 1,
          f"collision fill: every group filled, switched from walk ({said}, {len(census)} census lines, "
          f"{framed} fills read, {len(draws)} line draws, {len(new.fills)} fill draws)")
    check(new.said("collision fill") == ["collision: off"] and new.frame() == [] and new.debug_lines()[0] == [],
          "collision fill again: off")

    # a view on carried into a match that does not allow it: nothing drawn; back where it is allowed
    def drawn_now():
        """The render site's line draws and fill draws."""
        new.fills = []
        draws, _ = new.debug_lines()
        return len(draws), len(new.fills)

    hidden = ["collision: only in private rooms and replays: nothing drawn in this match"]
    check(new.said("collision fill") == [ON[0] + ", filled"] + HELP and len(new.frame()) == 5
          and drawn_now() == (1, 1), "collision fill in the private room's match: filled")
    told = new.options_read(b"")
    census = new.frame()
    check(told == hidden and census == [] and new.queries == [] and drawn_now() == (0, 0),
          f"a public match's options (no srp block) while on: told once ({told}), no read, nothing drawn")
    check(new.said("collision shot") == refused and new.frame() == [] and drawn_now() == (0, 0),
          "collision shot there: refused, nothing turned on")
    check(new.options_read(b"") == [] and new.frame() == [] and drawn_now() == (0, 0),
          "another public match's options: nothing told, nothing drawn")
    told = new.options_read(None)
    check(told == [] and len(new.frame()) == 5 and drawn_now() == (1, 1),
          f"a replay's options (whatever they say): back, filled, its census, nothing told ({told})")
    check(new.options_read(blackout.srpx((8, b"\x01"))) == [] and new.frame() == [] and drawn_now() == (1, 1),
          "a private room's match after the replay: still on, no read again")
    check(new.options_read(blackout.srpx((8, b"\x00"))) == hidden and new.frame() == [] and drawn_now() == (0, 0),
          "the entry of another value: a public match's, nothing drawn")
    check(new.said("collision fill") == ["collision: off"] and new.frame() == [] and new.debug_lines()[0] == [],
          "collision fill there (what is shown): off, not refused")


if __name__ == "__main__":
    main()
