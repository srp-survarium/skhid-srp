#!/usr/bin/env python3
"""Patch 156 (the Rust patch collision_view, rust/patches/src/collision_view.rs):
the console command `collision` (client_patch.md section 3.82).

    python check_collision_view_026g.py <original survarium.exe>

Requires Python `unicorn`. check_console_where_026g.py's harness (the exe's
sections and the arena loaded as they are, the log's helper write stubbed),
the original beside the build; system_renderer::draw_aabb (0x11DD60) is
stubbed: its EAX, the six floats at EDI and the colour its stack argument
points to are kept, `ret 4`. The game's own code runs the rest:
on_configs_loaded's start, the console's execute (0xEB980), update_camera
(0x1F3480 to its call of 0x243440) over a made-up physics world laid out as
the game's (bullet_physics_world +3Ch its Bullet world: m_collisionObjects'
size +8, data +10h; an object's proxy +0C8h, flags +0D8h; a proxy's group
+4, aabbMin +10h, aabbMax +20h), and renderer::render's span at
`render.debug_lines` (0x13D9C1 to 0x13D9CB). Checked:

- `collision` registered at configs.start, its vtable cc_delegate's but
  execute; the original says "unknown command";
- `collision`: "collision: on"; the next update_camera prints the census, a
  line a group (static objects of groups 2, 8 and 0Ah in the whole world);
  the render site then draws, through draw_aabb with the system renderer
  ([2260480h]) in EAX, each static object of those groups within 50 m of
  the eye, in the game's frame (Bullet's z negated), moved in by 1 cm times
  the group's place, green, magenta or white; never a hit model (40h, even
  static), a controller (824h), a battery (0A18h), anything moving, an
  object outside the broadphase, or anything beyond 50 m; once a frame read;
  the span's registers as the original leaves them;
- `collision 8`: that group alone, the census again; `collision x`: the
  usage, nothing changed; `collision` again: off, nothing drawn; the
  original never draws.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_ESP

import patch_026g as base
import check_console_where_026g as where
import check_hide_ui_026g as hide
import check_language_026g as language

DRAW_AABB = 0x11DD60
SITE, SITE_END = 0x13D9C1, 0x13D9CB          # render.debug_lines: mov edi,[esp+1Ch] / mov edx,[edi+6334C4h]
SYSTEM_RENDERER_SLOT = 0x2260480
PHYSICS = 0x60000000                         # the physics world, its Bullet world, objects and proxies
WORLD, ARRAY, OBJECTS = PHYSICS + 0x100, PHYSICS + 0x1000, PHYSICS + 0x10000
SYSTEM_RENDERER = 0x61000000
LINES = 0x62000000                           # where the renderer's +6334C4h is read
RENDERER = LINES - 0x6334C4
EYE = (10.0, 1.7, 20.0)
GREEN, MAGENTA, WHITE = 0xFF00FF00, 0xFFFF00FF, 0xFFFFFFFF


class Run(where.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(PHYSICS, 0x100000, UC_PROT_ALL)
        self.emu.mem_map(SYSTEM_RENDERER, 0x1000, UC_PROT_ALL)
        self.emu.mem_map(LINES, 0x1000, UC_PROT_ALL)
        self.emu.mem_map(hide.STACK - 0x100000, 0x100000, UC_PROT_ALL)   # the render handler copies 29 KiB of boxes
        self.word(SYSTEM_RENDERER_SLOT, SYSTEM_RENDERER)
        self.drawn = []
        self.emu.hook_add(UC_HOOK_CODE, self.draw_aabb, begin=DRAW_AABB, end=DRAW_AABB)

    def draw_aabb(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        floats = struct.unpack("<6f", emu.mem_read(emu.reg_read(UC_X86_REG_EDI), 24))
        self.drawn.append((emu.reg_read(UC_X86_REG_EAX), floats, self.read(self.read(sp + 4))))
        self.ret(4)

    def world(self, objects):
        """The physics world: `objects` (group, flags, Bullet min, Bullet max; None a proxy of 0)."""
        self.emu.mem_write(PHYSICS, bytes(0x100000))
        self.word(PHYSICS + 0x3C, WORLD)
        self.word(WORLD + 0x8, len(objects))
        self.word(WORLD + 0x10, ARRAY)
        for i, item in enumerate(objects):
            obj = OBJECTS + 0x200 * i
            self.word(ARRAY + 4 * i, obj)
            if item is None:
                continue
            group, flags, low, high = item
            proxy = obj + 0x100
            self.word(obj + 0xC8, proxy)
            self.word(obj + 0xD8, flags)
            self.emu.mem_write(proxy + 4, struct.pack("<HH", group, 0xFFFF))
            self.emu.mem_write(proxy + 0x10, struct.pack("<4f", *low, 0.0))
            self.emu.mem_write(proxy + 0x20, struct.pack("<4f", *high, 0.0))

    def frame(self):
        """update_camera(player) in a spectator's first person at EYE, the player's physics world ours: the
        lines it wrote."""
        self.emu.mem_write(where.OBJECTS, bytes(0x40000))
        self.word(where.PLAYER + 0xF470, where.HANDLER)
        self.word(where.PLAYER + 0xF478, where.PRESENTERS)
        self.word(where.PLAYER + 0x350, PHYSICS)
        self.emu.mem_write(where.PLAYER + 0x33C, b"\x01")
        self.emu.mem_write(where.PLAYER + 0x40C, where.transform(0.0, 0.0, EYE))
        self.emu.mem_write(where.PLAYER + 0x44C, where.transform(0.5, -0.1, EYE))
        self.word(where.HANDLER + 0x358, 4)
        self.word(where.HANDLER + 0x334, where.WORLD)
        self.writes = []
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, where.PLAYER)
        self.run(where.UPDATE_CAMERA, (where.CAMERA_END,), sp, {**hide.GAME, UC_X86_REG_EBX: 0x0B0B0B0B})
        assert self.stopped == where.CAMERA_END, self.stopped
        return [w["argument"].decode() for w in self.writes]

    def debug_lines(self):
        """renderer::render's span at the site: the boxes drawn, and EDI and EDX after it."""
        self.drawn = []
        sp = hide.STACK + 0x8000
        self.word(sp + 0x1C, RENDERER)
        self.word(LINES, 0x51525354)
        after, end = self.run(SITE, (SITE_END,), sp, hide.GAME)
        assert self.stopped == SITE_END and end == sp, (self.stopped, end - sp)
        return list(self.drawn), (after[UC_X86_REG_EDI], after[UC_X86_REG_EDX])

    def said(self, line):
        return [w["argument"].decode() for w in self.execute(line)[0]]


def game_box(low, high, inset):
    """A Bullet box in the game's frame (z negated), moved in by `inset`."""
    return (low[0] + inset, low[1] + inset, -high[2] + inset, high[0] - inset, high[1] - inset, -low[2] - inset)


def same(drawn, wanted):
    return len(drawn) == len(wanted) and all(
        renderer == SYSTEM_RENDERER and color == want_color and all(abs(a - b) < 1e-4 for a, b in zip(box, want_box))
        for (renderer, box, color), (want_box, want_color) in zip(drawn, wanted))


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

    # in Bullet's frame (z the game's -z); the eye at game (10, 1.7, 20) is Bullet (10, 1.7, -20)
    walker = (0x2, 1, (9.0, 0.0, -22.0), (11.0, 3.0, -21.0))
    bullet = (0x8, 1, (9.0, 0.0, -22.0), (11.0, 3.0, -21.0))
    both = (0xA, 3, (-40.0, -5.0, -60.0), (60.0, 40.0, 30.0))       # static and kinematic: the eye inside
    objects = [walker, bullet, both,
               (0x40, 1, (9.0, 0.0, -21.0), (10.0, 2.0, -20.0)),     # a hit model, static
               (0x824, 0x10, (9.0, 0.0, -21.0), (10.0, 2.0, -20.0)),  # a character controller
               (0xA18, 0, (9.0, 0.0, -21.0), (10.0, 2.0, -20.0)),    # a battery
               (0x2, 0, (9.0, 0.0, -21.0), (10.0, 2.0, -20.0)),      # a walker group's, moving
               (0x2, 1, (70.0, 0.0, -21.0), (80.0, 2.0, -20.0)),     # 60 m away
               None]                                                # outside the broadphase
    for r in (old, new):
        r.world(objects)
    drawn_old, regs_old = old.debug_lines()
    drawn_new, regs_new = new.debug_lines()
    check(drawn_old == [] and drawn_new == [] and regs_old == regs_new == (RENDERER, 0x51525354),
          "off: nothing drawn; the span's EDI and EDX the original's")

    check(new.said("collision") == ["collision: on"], "collision through console_commands::execute: \"collision: on\"")
    lines = new.frame()
    old.frame()
    check(lines == ["collision: the level's static collision within 50 m, a colour a group:",
                    "collision: 0x2 green, 2 in the world (walkers only)",
                    "collision: 0x8 magenta, 1 in the world (bullets and the camera only)",
                    "collision: 0xa white, 1 in the world (walkers, bullets and the camera)"],
          "the first frame: the census, a line a group: " + " | ".join(lines))
    wanted = [(game_box(walker[2], walker[3], 0.0), GREEN), (game_box(bullet[2], bullet[3], 0.01), MAGENTA),
              (game_box(both[2], both[3], 0.02), WHITE)]
    drawn, regs = new.debug_lines()
    check(same(drawn, wanted) and regs == regs_old,
          "the render site: the three level boxes near the eye in the game's frame, each group moved in by 1 cm a "
          "place, green, magenta, white, the system renderer in EAX; no player, controller, battery, moving or far "
          "object; the span's registers the original's")
    check(new.debug_lines()[0] == [] and old.debug_lines()[0] == [], "drawn once a frame read; the original never")
    check(new.frame() == [] and same(new.debug_lines()[0], wanted), "the next frame: no census, the boxes again")

    check(new.said("collision 8") == ["collision: on, 0x8 only"], "collision 8: \"collision: on, 0x8 only\"")
    check(len(new.frame()) == 4 and same(new.debug_lines()[0], wanted[1:2]), "collision 8: the census, the 0x8 box "
          "alone")
    check(new.said("collision x") == ["collision: collision [2 | 8 | a] (walkers, bullets, both)"]
          and new.frame() == [] and same(new.debug_lines()[0], wanted[1:2]), "collision x: the usage, nothing changed")
    check(new.said("collision") == ["collision: off"] and new.frame() == [] and new.debug_lines()[0] == [],
          "collision again: off, nothing drawn")


if __name__ == "__main__":
    main()
