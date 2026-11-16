#!/usr/bin/env python3
"""Patch 153 (step_ground): a step whose body stands on something sounds
(docs/notes/footsteps.md, "Silent ground").

    python check_step_ground_026g.py <original survarium.exe>

step_manager::on_step (0x248AD0) run whole, in the original and in the
patched exe, on a made-up step manager, game world, match and ground: the
physics world's ray_test (0x75E20, its vtable +48h), the hit objects'
get_triangle_material (vtable +10h), the world's material manager (vtable
+1Ch), material_pair (0x306D50), the world user's emitter (vtable +8) and
emit_and_play_once (0x657C0) are stubs that record their calls, from
surfaces each with its height, collision group and mask, and material:

1. on_step's ray meets the ground: the patched exe's calls (each ray, each
   material read, the pair, the sound and its point and hud flag) are the
   original's, the memory and registers too; a pair without a sound stays
   silent in both;
2. its ray meets nothing over a walker-only floor (cgroup 2, cmask 4): the
   original is silent; the patched exe looks again with the walker
   collision (group 34h, mask 0Ah, the same 2 m), and plays the pair of the
   foot and the walker face's material, the sound at the foot; a face
   without a material (3FFFh) takes the hittable surface under it (group
   30h, mask 8, 5 m: 4 m under the foot), and with none there plays its
   own (which material_pair steps as default); third person: no hud flag;
3. nothing under the foot: silent in both, the patched exe's one look the
   walker one;
4. SKHID_STEP_LOG unset: GetEnvironmentVariableA once, at the first step,
   and no file is opened or written; set to `on`, each step writes its line
   (whose, movement id, view, foot material and place, each ray, the
   ground, what its pair plays).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

ON_STEP, RAY_TEST, MATERIAL_PAIR, EMIT, FREE = 0x248AD0, 0x75E20, 0x306D50, 0x657C0, 0xE0210
FOOT_STEP_RETURN = 0x2A6577                  # on_foot_step after its call of on_step (never run)
ENVIRONMENT, CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, THREAD_SLOT = 0x8701B4, 0x870048, 0x870108, 0x870040, 0x870118
ENGINE_TSV = "engine_026g.tsv"

DATA = 0x30000000
DATA_SIZE = 0x40000
(MANAGER, WORLD, WORLD_VTABLE, GAME, USER, USER_VTABLE, MATCH, PHYSICS, PHYSICS_VTABLE, MATERIALS, OBJECT_VTABLE,
 EFFECT, LOGIC, STATE, PLAYER, CLIENT, POSITION, DIRECTION, SOUNDS, PAIRS, OBJECTS) = (DATA + 0x1000 * i for i in range(21))
GAME_SIZE = 0x6000
STUBS = 0x31000000
TRIANGLE, MANAGER_OF, EMITTER, IMPORT, FORBIDDEN, THREAD = (STUBS + 0x10 * i for i in range(6))
STACK = 0x21000000
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_EDI: 0x0E0E0E0E, UC_X86_REG_EBP: 0x0019FF00}

ACTOR, ACTOR_LOCAL = 10, 42
DEFAULT, GROUND, METALL_THICK, CONCRETE, FOLIAGE_DRY, UNNAMED = 11, 9, 18, 19, 49, 0x3FFF
HITTABLE, WALKER = (8, 16), (2, 4)
FOOT = (10.0, 5.5, -3.0)


class Step:
    """on_step in one exe; each run one step over `surfaces`, a list of
    (height, (cgroup, cmask), material)."""

    def __init__(self, data, log=None):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, DATA_SIZE, UC_PROT_ALL)
        emu.mem_map(STUBS, 0x1000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.log, self.environment, self.lines, self.forbidden = log, 0, [], []
        # every import the arena calls fails the check but the log's and the thread's
        for line in open(ENGINE_TSV, encoding="utf-8"):
            cells = line.rstrip("\n").split("\t")
            if len(cells) > 4 and cells[4] == "import":
                self.word(int(cells[1], 16), FORBIDDEN)
        for slot, stub in ((ENVIRONMENT, IMPORT), (CREATE_FILE, IMPORT + 4), (WRITE_FILE, IMPORT + 8),
                           (CLOSE_HANDLE, IMPORT + 12), (THREAD_SLOT, THREAD)):
            self.word(slot, stub)
        emu.hook_add(UC_HOOK_CODE, self.code)
        self.world()

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", bytes(self.emu.mem_read(at, 4)))[0]

    def floats(self, at, n=3):
        return struct.unpack(f"<{n}f", bytes(self.emu.mem_read(at, 4 * n)))

    def world(self):
        emu = self.emu
        emu.mem_write(DATA, bytes(DATA_SIZE))
        self.word(MANAGER + 4, WORLD)
        self.word(WORLD, WORLD_VTABLE)
        self.word(WORLD_VTABLE + 0x1C, MANAGER_OF)
        self.word(WORLD + 0x9C, GAME)
        self.word(WORLD + 0x4D4C, MATCH)
        self.word(GAME + 0x5350, USER)
        self.word(GAME + 0x53AC, CLIENT)
        self.word(CLIENT + 4, PLAYER)                              # the local player
        self.word(USER, USER_VTABLE)
        self.word(USER_VTABLE + 8, EMITTER)
        self.word(MATCH + 0x12C, MATERIALS)
        self.word(MATCH + 0x130, PHYSICS)
        self.word(PHYSICS, PHYSICS_VTABLE)
        self.word(PHYSICS_VTABLE + 0x48, RAY_TEST)
        self.word(OBJECT_VTABLE + 0x10, TRIANGLE)
        self.word(EFFECT + 0x38, LOGIC)                            # on_foot_step's effect: its logic state's
        self.word(LOGIC + 0x20, STATE)                             # movement id 2 (stand_run), its player
        self.word(STATE + 0x24, 2)
        self.word(EFFECT + 0x3C, PLAYER)
        emu.mem_write(DIRECTION, struct.pack("<3f", 0.0, 0.0, 1.0))
        # the pairs: (foot, ground) -> a sound; (42, 49) none, as the live data has it
        self.pairs = {}
        for i, (foot, ground, sound) in enumerate([(ACTOR_LOCAL, GROUND, 1), (ACTOR_LOCAL, DEFAULT, 2),
                                                   (ACTOR_LOCAL, METALL_THICK, 3), (ACTOR_LOCAL, CONCRETE, 4),
                                                   (ACTOR_LOCAL, FOLIAGE_DRY, 0), (ACTOR, METALL_THICK, 5),
                                                   (ACTOR, DEFAULT, 6)]):
            pair = PAIRS + 0x100 * i
            self.pairs[(foot, ground)] = pair
            if sound:
                self.word(pair + 0x20, SOUNDS + 0x100 * sound)
                self.word(SOUNDS + 0x100 * sound + 0xD0, 1)        # its reference count
        self.objects = {}

    def code(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 9)]
        if address == RAY_TEST:
            self.ray_test(emu, args)
            self.ret(32)
        elif address == TRIANGLE:
            this = emu.reg_read(UC_X86_REG_ECX)
            self.calls.append(("material", this, args[0], args[1] & 0xFF))
            self.ret(8, self.objects[this])
        elif address == MANAGER_OF:
            self.calls.append(("materials", emu.reg_read(UC_X86_REG_ECX)))
            self.ret(0, MATERIALS)
        elif address == MATERIAL_PAIR:
            foot, ground = emu.reg_read(UC_X86_REG_EDX) & 0xFFFF, emu.reg_read(UC_X86_REG_EDI) & 0xFFFF
            assert emu.reg_read(UC_X86_REG_ECX) == MATERIALS, "material_pair's manager"
            self.calls.append(("pair", foot, ground))
            # an id of no material steps as default; a foot of no pair, none
            known = ground in (GROUND, DEFAULT, METALL_THICK, CONCRETE, FOLIAGE_DRY)
            self.ret(0, self.pairs.get((foot, ground if known else DEFAULT), 0))
        elif address == EMITTER:
            self.ret(0, 0x7777)
        elif address == EMIT:
            self.calls.append(("sound", args[0], self.floats(args[1]), args[2], emu.reg_read(UC_X86_REG_ECX)))
            self.ret(12)
        elif address == IMPORT:                                    # GetEnvironmentVariableA(name, buffer, size)
            name = bytes(emu.mem_read(args[0], 32)).split(b"\0")[0]
            self.environment += 1
            value = self.log if name == b"SKHID_STEP_LOG" else None
            if value is None:
                self.ret(12, 0)
            else:
                emu.mem_write(args[1], value + b"\0")
                self.ret(12, len(value))
        elif address == IMPORT + 4:                                # CreateFileA: 7 arguments, a handle
            self.ret(28, 0x1234)
        elif address == IMPORT + 8:                                # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])).decode())
            self.word(args[3], args[2])
            self.ret(20, 1)
        elif address == IMPORT + 12:                               # CloseHandle
            self.ret(4, 1)
        elif address in (FORBIDDEN, FREE):
            self.forbidden.append(hex(address))
            emu.emu_stop()
        elif address == FOOT_STEP_RETURN:
            self.returned = True
            emu.emu_stop()

    def ret(self, pops, eax=None):
        emu = self.emu
        sp = emu.reg_read(UC_X86_REG_ESP)
        if eax is not None:
            emu.reg_write(UC_X86_REG_EAX, eax)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def ray_test(self, emu, args):
        """The highest surface the ray meets, as ray_test writes its result."""
        result, origin, direction, length = args[0], self.floats(args[1]), self.floats(args[2]), \
            struct.unpack("<f", struct.pack("<I", args[3]))[0]
        group, mask, delegate, unflipped = args[4] & 0xFFFF, args[5] & 0xFFFF, args[6], args[7] & 0xFF
        assert emu.reg_read(UC_X86_REG_ECX) == PHYSICS, "ray_test's world"
        assert direction == (0.0, -1.0, 0.0) and delegate == 0 and unflipped == 1, (direction, delegate, unflipped)
        self.calls.append(("ray", origin, round(length, 4), group, mask))
        met = [(height, obj) for height, (cgroup, cmask), obj in self.surfaces
               if cgroup & mask and cmask & group and origin[1] - length <= height <= origin[1]]
        if not met:
            self.word(result + 0xC, 0)
            self.word(result + 0x1C, 0)
            return
        height, obj = max(met)
        emu.mem_write(result, struct.pack("<3f", origin[0], height, origin[2]))
        emu.mem_write(result + 0xC, struct.pack("<f", (origin[1] - height) / length))
        emu.mem_write(result + 0x10, struct.pack("<3f", 0.0, 1.0, 0.0))
        self.word(result + 0x1C, obj)
        self.word(result + 0x20, 7)
        emu.mem_write(result + 0x24, b"\0")

    def run(self, surfaces, foot=ACTOR_LOCAL, view=0):
        """One step of the foot material `foot` at FOOT over `surfaces`: the
        calls it made."""
        emu = self.emu
        self.calls, self.returned = [], False
        self.surfaces = []
        for i, (height, groups, material) in enumerate(surfaces):
            obj = OBJECTS + 0x40 * i
            self.word(obj, OBJECT_VTABLE)
            self.objects[obj] = material
            self.surfaces.append((height, groups, obj))
        emu.mem_write(POSITION, struct.pack("<3f", *FOOT))
        sp = STACK + 0x8000
        emu.mem_write(STACK, b"\xCC" * 0x8000)                     # on_step's frame: whatever was there
        arguments = [DIRECTION, foot, view]
        emu.mem_write(sp, struct.pack("<4I", FOOT_STEP_RETURN, *arguments))
        before = bytes(emu.mem_read(DATA, DATA_SIZE))
        for reg, value in KEPT.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESI, EFFECT)                       # on_foot_step's, which on_step pushes
        emu.reg_write(UC_X86_REG_ECX, MANAGER)
        emu.reg_write(UC_X86_REG_EAX, POSITION)
        emu.reg_write(UC_X86_REG_EDX, 0)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(ON_STEP, 0, count=2_000_000)
        assert not self.forbidden, f"an import or free called: {self.forbidden}"
        assert self.returned, hex(emu.reg_read(UC_X86_REG_EIP))
        kept = {reg: emu.reg_read(reg) for reg in KEPT}
        assert kept == KEPT and emu.reg_read(UC_X86_REG_ESI) == EFFECT, "a register the caller keeps"
        assert emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 12, "ret 0Ch"
        assert bytes(emu.mem_read(DATA, DATA_SIZE)) == before, "on_step wrote the objects"
        return self.calls


def sound(pair_sound, at, view=0):
    """The calls of a played sound: the manager, the pair, then the emit."""
    return [("sound", SOUNDS + 0x100 * pair_sound, at, int(view == 0), WORLD + 0x94)]


def game_ray(length=2.0, group=0x30, mask=8):
    return ("ray", (FOOT[0], FOOT[1] + 1.0, FOOT[2]), length, group, mask)


WALKER_RAY = game_ray(2.0, 0x34, 0x0A)
BELOW_RAY = game_ray(5.0)
AT_FOOT = FOOT


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())

    def material(i, triangle=7):
        return ("material", OBJECTS + 0x40 * i, triangle, 0)

    concrete = [(5.25, HITTABLE, CONCRETE)]
    walker_metal = [(5.25, WALKER, METALL_THICK), (3.0, HITTABLE, GROUND)]
    walker_bare = [(5.25, WALKER, UNNAMED), (4.0, HITTABLE, METALL_THICK)]
    walker_alone = [(5.25, WALKER, UNNAMED), (-1.0, HITTABLE, GROUND)]
    air = [(3.0, HITTABLE, GROUND), (2.0, WALKER, METALL_THICK)]
    dry = [(5.25, HITTABLE, FOLIAGE_DRY)]

    print("the original exe (step_manager::on_step 0x248AD0)")
    game = Step(original)
    hit_calls = game.run(concrete)
    want = [game_ray(), material(0), ("materials", WORLD), ("pair", ACTOR_LOCAL, CONCRETE)] + \
        sound(4, (FOOT[0], 5.25, FOOT[2]))
    assert hit_calls == want, hit_calls
    dry_calls = game.run(dry)
    assert dry_calls == [game_ray(), material(0), ("materials", WORLD), ("pair", ACTOR_LOCAL, FOLIAGE_DRY)], dry_calls
    third_calls = game.run(concrete, foot=ACTOR, view=1)
    for surfaces in (walker_metal, walker_bare, walker_alone, air):
        assert game.run(surfaces) == [game_ray()], surfaces
    print("  ok its ray meets the ground: the pair's sound at the hit; a pair without a sound: silent;")
    print("     a walker-only floor, an unnamed one, nothing under the foot: one ray, silent")

    print("the patched exe, SKHID_STEP_LOG unset")
    ours = Step(bytes(built))
    assert ours.run(concrete) == hit_calls, "a ray that met the ground: the game's own calls"
    assert ours.environment == 1, "the switch read at the first step"
    assert ours.run(dry) == dry_calls and ours.run(concrete, foot=ACTOR, view=1) == third_calls
    print("  ok its ray meets the ground: every call the original's, the hud flag by the view; a silent pair silent")
    calls = ours.run(walker_metal)
    assert calls == [game_ray(), WALKER_RAY, material(0), ("materials", WORLD), ("pair", ACTOR_LOCAL, METALL_THICK)] + \
        sound(3, AT_FOOT), calls
    print("  ok a walker-only floor of metall_thick: the walker look (34h/0Ah, 2 m), its face's metal at the foot")
    calls = ours.run(walker_metal, foot=ACTOR, view=1)
    assert calls == [game_ray(), WALKER_RAY, material(0), ("materials", WORLD), ("pair", ACTOR, METALL_THICK)] + \
        sound(5, AT_FOOT, view=1), calls
    print("  ok the same in third person: a point sound at the foot")
    calls = ours.run(walker_bare)
    assert calls == [game_ray(), WALKER_RAY, material(0), BELOW_RAY, material(1), ("materials", WORLD),
                     ("pair", ACTOR_LOCAL, METALL_THICK)] + sound(3, AT_FOOT), calls
    print("  ok a walker face without a material: the hittable floor 1.25 m under it (30h/8, 5 m), at the foot")
    calls = ours.run(walker_alone)
    assert calls == [game_ray(), WALKER_RAY, material(0), BELOW_RAY, ("materials", WORLD),
                     ("pair", ACTOR_LOCAL, UNNAMED)] + sound(2, AT_FOOT), calls
    print("  ok nothing under it in 4 m: its own (3FFFh), which material_pair steps as default")
    assert ours.run(air) == [game_ray(), WALKER_RAY], "nothing under the foot: one look, silent"
    print("  ok nothing under the foot within 1 m: the walker look alone, silent")
    assert ours.environment == 1 and not ours.lines, "the log: off, nothing written"
    print("  ok the switch read once; no file opened or written")

    print("the patched exe, SKHID_STEP_LOG=on")
    logged = Step(bytes(built), log=b"on")
    head = "srp: step: own foot, movement 2, first person, foot 42 at 1000 550 -300 cm; "
    for surfaces, foot, view, want in [
            (concrete, ACTOR_LOCAL, 0, "game met 19 at 25 cm; ground 19 (game): plays"),
            (dry, ACTOR_LOCAL, 0, "game met 49 at 25 cm; ground 49 (game): silent, the pair has no sound"),
            (walker_metal, ACTOR_LOCAL, 0, "game missed; walker met 18 at 25 cm; ground 18 (walker): plays"),
            (walker_bare, ACTOR_LOCAL, 0,
             "game missed; walker met 16383 at 25 cm; below met 18 at 150 cm; ground 18 (below): plays"),
            (walker_alone, ACTOR_LOCAL, 0,
             "game missed; walker met 16383 at 25 cm; below missed; ground 16383 (walker, no material): plays"),
            (air, ACTOR_LOCAL, 0, "game missed; walker missed; nothing under the foot: silent"),
            (walker_metal, ACTOR, 1, "game missed; walker met 18 at 25 cm; ground 18 (walker): plays")]:
        logged.lines = []
        logged.run(surfaces, foot=foot, view=view)
        line = head.replace("first person, foot 42", "third person, foot 10") if view else head
        assert logged.lines == [line + want + "\r\n"], logged.lines
        print("  ok", logged.lines[0].rstrip())


if __name__ == "__main__":
    main()
