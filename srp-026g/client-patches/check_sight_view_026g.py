#!/usr/bin/env python3
"""Run the first-person view of patch 206's second sights (patch 161's
rust/patches/src/attachments/sights.rs over vostok::sight_view; the rules:
../docs/notes/second-sights.md, "The view (0.26g)") in the default build on
made-up game objects, beside the original.

    python check_sight_view_026g.py <original survarium.exe>

Requires Python `unicorn`. check_attachments_026g.py's harness: the modules
table is the weapon-modules crate's sample (../crates/weapon-modules/tests/
sample-v4.srpm), what stands where its pinned loadouts (sample-v4.loadouts.
tsv). An HK416 with a DeltaPoint Pro on its right offset rail is cooked
through patch 161's sites (the cook 0x24A920, its callback 0x24ACF0 and
0x24B0E1), made a weapon of two sights through patch 206's (weapon_core's
entry 0x2DF0D0), held in its player's slot 7 and switched by a press
(apply_new_input 0x2D877C at 104520 ms: the blend from 104550, the end at
105020) and the tick that ends it (weapon_core::tick's join 0x2E0A45). The
items dictionary's callback (0x2C8AA0) gives the DeltaPoint's srp_sight_views
row (0.8, 0.5) and the gun's srp_weapon_views row (1, 0.5). Then, each site
run in the game's own code:

- the eye (process_hands_ik before calculate_aim_alignment, 0x295AA5 to
  0x295AAD, patch 172 after it): the aim record (+7A8h) the main eye on the
  main sight, the two eyes blended by the switch (the crate's rule, worked
  out here again: the place on the line, the turn along the shorter arc),
  by the frame's time between two ticks (the game's +5854h), the second eye
  settled on the second sight, the main one again after a switch back; the
  original leaves the record;
- the world's zoom and near plane (update_camera 0x1F3480 to its join
  0x1F3643): the camera's +94h and +8Ch the original's on the main sight,
  else 1 + (f - 1) x the effective aim, f the two sights' factors blended by
  the switch (the main's its optic's +114h/+118h, or the gun's own
  +5B4h/+5B8h), the near plane times the image's 0.05;
- the weapon's own projection (update_camera's weapon_view.camera 0x1F3505,
  read back by the foreground pass): its FOV factor at the effective aim;
- the scope view (the client weapon's tick after weapon_core::tick,
  0x244ABD to 0x244B2D / 0x244B5B, at the tick's time +5A4h): +181Eh the
  original's on the main sight, 0 while the second sight is shown, back on
  the main;
- the second sight's reticle (weapon::draw's end 0x245017): its aimed model,
  asked for at the cook after the parts, in the scene at the part's frame in
  place of its own while the second sight is shown in first person; its own
  otherwise and out of first person.
"""
import math
import os
import struct
import sys

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])

from unicorn import UC_HOOK_CODE

import check_attachments_026g as att
import patch_026g as base
from check_attachments_026g import STACK, STOP, RENDERER, T_INT, T_TABLE, close, translate, rotation_y, multiply

MADE, MADE_END = 0x2DF0D0, 0x2DF0D5
PRESS, PRESS_END = 0x2D877C, 0x2D8782
TICK, TICK_END = 0x2E0A45, 0x2E0A4B
EYE, EYE_END = 0x295AA5, 0x295AAD
CAMERA, CAMERA_END = 0x1F3480, 0x1F3643
VIEW_TICK, VIEW_SET, VIEW_DONE = 0x244ABD, 0x244B2D, 0x244B5B
NEAR_UNIT = 0x9CC878
ENDS = (MADE_END, PRESS_END, TICK_END, EYE_END, CAMERA_END, VIEW_SET, VIEW_DONE)
BIT = 1 << 30
HK416, EOTECH, MIKI, DPPRO = 763, 1771, 1438, 2112
T = 104_520
WORLD = multiply(rotation_y(0.4), translate(10.0, 1.5, -4.0))
BONES = [att.IDENTITY, multiply(rotation_y(-0.2), translate(0.0, 0.01, -0.03)), translate(0.3, 0.2, 0.1),
         translate(0.0, 0.0, 0.0)]
GAME_RECORDS = [(1, translate(0.0, 0.042, -0.636)), (1, translate(0.0, 0.07, 0.02)), (0, translate(0.0, 0.08, 0.1))]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


# ------------------------------------------------- the crate's blend, again

def quaternion(m):
    """Placement::toward's quaternion of a row-vector matrix (pose.rs)."""
    rows = [m[0:3], m[4:7], m[8:11]]
    r = lambda i, j: rows[j][i]
    trace = r(0, 0) + r(1, 1) + r(2, 2)
    if trace > 0:
        s = math.sqrt(trace + 1) * 2
        q = [0.25 * s, (r(2, 1) - r(1, 2)) / s, (r(0, 2) - r(2, 0)) / s, (r(1, 0) - r(0, 1)) / s]
    elif r(0, 0) > r(1, 1) and r(0, 0) > r(2, 2):
        s = math.sqrt(1 + r(0, 0) - r(1, 1) - r(2, 2)) * 2
        q = [(r(2, 1) - r(1, 2)) / s, 0.25 * s, (r(0, 1) + r(1, 0)) / s, (r(0, 2) + r(2, 0)) / s]
    elif r(1, 1) > r(2, 2):
        s = math.sqrt(1 + r(1, 1) - r(0, 0) - r(2, 2)) * 2
        q = [(r(0, 2) - r(2, 0)) / s, (r(0, 1) + r(1, 0)) / s, 0.25 * s, (r(1, 2) + r(2, 1)) / s]
    else:
        s = math.sqrt(1 + r(2, 2) - r(0, 0) - r(1, 1)) * 2
        q = [(r(1, 0) - r(0, 1)) / s, (r(0, 2) + r(2, 0)) / s, (r(1, 2) + r(2, 1)) / s, 0.25 * s]
    n = math.sqrt(sum(v * v for v in q))
    return [v / n for v in q]


def toward(a, b, t):
    """Placement::toward: the place on the line, the turn the normalised
    blend of the two quaternions along the shorter arc."""
    qa, qb = quaternion(a), quaternion(b)
    if sum(x * y for x, y in zip(qa, qb)) < 0:
        qb = [-v for v in qb]
    q = [(1 - t) * x + t * y for x, y in zip(qa, qb)]
    n = math.sqrt(sum(v * v for v in q))
    w, x, y, z = (v / n for v in q)
    m = [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
         [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
         [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]
    out = [m[j][i] for i in range(3) for j in range(3)]
    place = [a[12 + k] + (b[12 + k] - a[12 + k]) * t for k in range(3)]
    return out[0:3] + [0.0] + out[3:6] + [0.0] + out[6:9] + [0.0] + place + [1.0]


# ------------------------------------------------- the harness

class Run(att.Run):
    def __init__(self, data, table):
        super().__init__(data)
        for at in ENDS:
            self.emu.hook_add(UC_HOOK_CODE, self.stop, begin=at, end=at)
        self.table_bytes = table
        self.player = self.block(0x10000)
        inventory, core, presenters, client = (self.block(n) for n in (0x200, 0x40, 0x5400, 0x40))
        self.emu.mem_write(self.player + 0x114, bytes([3, 1]))
        self.u32(self.player + 0x120, core)
        self.u32(self.player + 0x10C, inventory)
        self.u32(self.player + 0x328, 90_000)
        self.emu.mem_write(self.player + 0x33C, b"\1")
        self.inventory = inventory
        # the network client's local player: this one (the view tick's test)
        self.u32(self.player + 0xF478, presenters)
        self.u32(presenters + 0x53AC, client)
        self.u32(client + 4, self.player)
        # the camera handler, first person
        self.handler = self.block(0x400)
        self.u32(self.player + 0xF470, self.handler)
        # a vtable whose +70h (the camera's cast to a client weapon) gives the weapon
        self.vtable = self.block(0x100)
        cast = self.block(0x10)
        self.emu.mem_write(cast, bytes.fromhex("89c8c3"))           # mov eax,ecx / ret
        self.u32(self.vtable + 0x70, cast)

    def dictionary(self, sights, guns):
        """The items dictionary's callback with its first config's srp_sight_views
        and srp_weapon_views rows, and the modules table."""
        def pair(a, b):
            at = self.block(8)
            self.put_floats(at, (a, b))
            return self.node(6, at, 8)

        def rows(key, items):
            array = self.block(0x18 * len(items))
            for n, (dict_id, factors) in enumerate(items):
                at = array + 0x18 * n
                self.u32(at, self.block(0x30))
                self.emu.mem_write(at + 0x14, struct.pack("<HH", T_TABLE, 2))
                self.children[(at, key.encode())] = self.node(T_INT, dict_id)
                self.children[(at, b"fov")] = pair(*factors)
            return self.node(T_TABLE, array, len(items))

        config = self.block(0x10C)
        self.u32(config + 0x108, self.table({"srp_sight_views": rows("sight", sights),
                                             "srp_weapon_views": rows("gun", guns)}))
        self.dictionary_configs(6, self.table_bytes, config)

    def fitted(self, case, key):
        """The weapon cooked with `key`, through the callback: the weapon, its
        model's instance, the parts' (holder, instance), the aimed model's."""
        weapon, instance = self.weapon(4, GAME_RECORDS)
        # a game of the size the frame time's field needs, the weapon's world's
        world, game = self.u32(weapon + 0x180C), self.block(0x6000)
        self.u32(game + 0x5360, RENDERER)
        self.u32(world + 0x9C, game)
        self.game = game
        # the player's game world and its scene view (the weapon's own projection's)
        self.view = self.block(0x100)
        self.u32(world + 8, self.view)
        self.u32(self.player + 0xF474, world)
        config = {"srp_gun": key[0], "srp_scope": key[1], "srp_muzzle": key[2], "srp_grip": key[3],
                  "srp_second_sight": key[4], "srp_second_side": key[5]}
        resource, root = self.config(config)
        requests = self.cook(resource, weapon)
        parts = [self.part() for _ in case["parts"]]
        aimed = self.part()
        scope = [self.block(0x120)] if key[1] in att.SCOPE_ITEMS else []
        native = [self.block(0x120) for _ in range(5)]
        stopped = self.callback(weapon, resource, scope + native + [h for h, _ in parts] + [aimed[0]])
        assert stopped == att.LOADED_END, hex(stopped)
        # the player's, in slot 7, its dictionary entry the gun's
        self.u32(weapon, self.vtable)
        self.u32(weapon + 8, self.player)
        entry = self.block(0x120)
        self.u32(weapon + 0x26C, entry)
        self.emu.mem_write(entry, struct.pack("<H", key[0]))
        self.u32(self.inventory + 0x10C + 4 * 7, weapon + 0x10)
        self.u32(self.player + 0x128, weapon)
        # patch 206: a weapon of two sights by its config
        sp = STACK - 0x8000
        for n, value in enumerate((STOP, weapon, root, 0x77)):
            self.u32(sp + 4 * n, value)
        assert self.run(MADE, (MADE_END,), sp) == MADE_END
        return weapon, instance, requests, parts, aimed

    def time(self, simulation, frame):
        self.u32(self.player + 0x324, simulation)
        self.u32(self.game + 0x5854, frame)

    def press(self, now):
        self.u32(self.player + 0x330, BIT)
        sp, ebp = STACK - 0x8000, STACK - 0x7000
        self.emu.mem_write(sp, bytes(0x40))
        self.u32(ebp + 8, now)
        assert self.run(PRESS, (PRESS_END,), sp, edi=self.player, ebp=ebp) == PRESS_END

    def tick(self, weapon, now):
        self.u32(weapon + 0x5A4, now)
        assert self.run(TICK, (TICK_END,), STACK - 0x8000, ebp=weapon, ebx=now) == TICK_END

    def eye(self, weapon):
        """process_hands_ik before calculate_aim_alignment: the aim record after it."""
        sp = STACK - 0x8000
        self.emu.mem_write(sp, bytes(0x40))
        self.u32(sp + 0x1C, weapon)
        bones = self.block(64 * 4)
        assert self.run(EYE, (EYE_END,), sp, ecx=bones) == EYE_END
        return self.read_record(weapon + att.AIM)

    def camera(self, weapon, progress, optic=0):
        """update_camera to its join: the camera's zoom and near plane."""
        self.put_floats(weapon + 0x5FC, [progress])
        self.u32(weapon + 0x1808, optic)
        self.put_floats(weapon + 0x5B4, [0.9, 1.0])
        self.put_floats(self.handler + 0x8C, [7.0])
        self.put_floats(self.handler + 0x94, [7.0])
        sp = STACK - 0x8000
        self.u32(sp, STOP)
        self.u32(sp + 4, self.player)
        assert self.run(CAMERA, (CAMERA_END,), sp) == CAMERA_END
        return self.floats(self.handler + 0x94, 1)[0], self.floats(self.handler + 0x8C, 1)[0]

    def lens(self):
        """The weapon's own FOV the last camera sampled, through its draw command (+14h)."""
        command = self.block(0xB8)
        self.emu.mem_write(command + 0x14, b"\xCD" * 4)
        self.u32(command + 0x9C, self.view)
        self.run(att.VIEW_SUBMIT, att.VIEW_ENDS, STACK, eax=command, ebx=RENDERER)
        focal = self.floats(command + 0x14, 1)[0]
        return math.degrees(2 * math.atan(1 / focal)) if focal > 0 else None

    def scope_view(self, weapon, optic, before):
        """The client weapon's tick after weapon_core::tick, aimed: where it
        stopped and +181Eh."""
        self.u32(weapon + 0x1808, optic)
        self.emu.mem_write(weapon + 0x5E8, b"\1")
        self.put_floats(weapon + 0x5FC, [1.0])
        self.emu.mem_write(weapon + 0x181E, bytes([before]))
        self.u32(weapon + 0x5A4, self.u32(self.player + 0x324))     # weapon_core::tick's time, set before
        sp = STACK - 0x8000
        stopped = self.run(VIEW_TICK, (VIEW_SET, VIEW_DONE), sp, esi=weapon)
        return stopped, self.byte(weapon + 0x181E)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None, late_join)
    built = bytes(built)
    table = (att.SAMPLE / "sample-v4.srpm").read_bytes()
    cases = att.loadouts()
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    key = (HK416, 0, 0, 0, DPPRO, 1)
    case = cases[key]
    main_eye, second_eye = case["eye"]["gun_idle"], case["eye2"]["gun_idle"]

    def setup(exe, at=key):
        run = Run(exe, table)
        run.dictionary([(DPPRO, (0.8, 0.5))], [(HK416, (1.0, 0.5))])
        made = run.fitted(cases[at], at)
        run.time(T - 20, 50_000)
        return (run, *made)

    print("the cook: the second sight's aimed model asked for after the parts (0x24AC30)")
    run, weapon, instance, requests, parts, aimed = setup(built)
    got = [path for path, kind in requests[0] if kind == 15]
    report(got == [model for model, _, _ in case["parts"]] + [case["aimed"]] and requests[1],
           f"the parts' models, then {case['aimed'].rsplit('/', 1)[-1]}, inside the alloca")
    report(run.u32(aimed[0] + 0xD0) == 2, "its model retained once with the parts'")

    print("the eye (process_hands_ik 0x295AA5, before calculate_aim_alignment; patch 172 after)")
    stock = setup(original)
    bone, record = run.eye(weapon)
    report(bone == main_eye[0] and close(record, main_eye[1]), "on the main sight: the main eye (as loaded)")
    run.press(T)
    for simulation, frame, s in ((T + 30, 50_000, 0.0), (104_800, 50_100, 0.5), (104_800, 50_120, 0.54),
                                 (104_800, 50_400, 0.598), (105_000, 50_420, 0.9)):
        run.time(simulation, frame)
        bone, record = run.eye(weapon)
        want = toward(main_eye[1], second_eye[1], s)
        report(bone == main_eye[0] == second_eye[0] and close(record, want, 1e-4),
               f"switching, simulation {simulation} ms, frame {frame}: the eyes blended at {s}")
    run.tick(weapon, 105_050)
    run.time(105_050, 50_500)
    bone, record = run.eye(weapon)
    report(bone == second_eye[0] and close(record, second_eye[1], 1e-5), "settled on the second sight: the second eye")
    stock[0].press(T)
    stock[0].time(104_800, 50_100)
    bone, record = stock[0].eye(stock[1])
    report(bone == GAME_RECORDS[2][0] and close(record, GAME_RECORDS[2][1]), "the original, switching: the record as loaded")

    print("the scope view (the client weapon's tick 0x244ABD..0x244B5C)")
    optic = run.block(0x120)
    run.put_floats(optic + 0x110, [0.82, 0.5, 0.5])
    s_optic = stock[0].block(0x120)
    stock[0].put_floats(s_optic + 0x110, [0.82, 0.5, 0.5])
    stopped, byte = run.scope_view(weapon, optic, 1)
    report(stopped == VIEW_DONE and byte == 0, "the second sight shown, aimed past change_scope_factor: +181Eh 0")
    stopped, byte = run.scope_view(weapon, optic, 0)
    report(stopped == VIEW_DONE and byte == 0, "and it stays 0")

    print("the world's zoom and near plane, the weapon's own FOV (update_camera 0x1F3480 to 0x1F3643; "
          "weapon_view.camera 0x1F3505, its draw command 0x1F8F7)")
    unit = f32(struct.unpack("<f", run.emu.mem_read(NEAR_UNIT, 4))[0])
    zoom, near = run.camera(weapon, 1.0)
    report(abs(zoom - 0.8) < 1e-6 and abs(near - 0.5 * unit) < 1e-7,
           f"settled on the second sight, aimed: the DeltaPoint's row, zoom {zoom:.4f}, near {near:.4f}")
    # the switch back: from the second (0.8, 0.5) to the gun's own (0.9, 1.0)
    run.press(200_000)
    for simulation, frame, s in ((200_300, 60_000, 0.5), (200_300, 60_020, 0.54)):
        run.time(simulation, frame)
        zoom, near = run.camera(weapon, 1.0)
        f = 0.8 + (0.9 - 0.8) * s
        n = 0.5 + (1.0 - 0.5) * s
        aim = 1.0 - min(s, 1 - s)
        report(abs(zoom - (1 + (f - 1) * aim)) < 1e-5 and abs(near - (1 + (n - 1) * aim) * unit) < 1e-6,
               f"switching back at {s}: the factors blended, then from 1 by the effective aim {aim:.2f}")
        angle = run.lens()
        want = 65 * (1 + (0.5 - 1) * aim)
        report(abs(angle - want) < 1e-3, f"the weapon's own FOV at the effective aim: {angle:.3f} degrees ({want:.3f})")
    stopped, byte = run.scope_view(weapon, optic, 1)
    report(stopped == VIEW_DONE and byte == 0, "the scope view at the tick's time, s 0.5 (the second still shown): 0")
    run.time(200_350, 60_100)
    stopped, byte = run.scope_view(weapon, optic, 0)
    report(stopped == VIEW_SET and byte == 1, "s 0.6, the main shown: the original's rule, set")
    run.tick(weapon, 200_500)
    run.time(200_500, 60_500)
    for progress in (0.0, 0.4, 1.0):
        for optic_at in (0, 1):
            o1 = optic if optic_at else 0
            o2 = s_optic if optic_at else 0
            got, want = run.camera(weapon, progress, o1), stock[0].camera(stock[1], progress, o2)
            report(got == want, f"settled on the main sight, aim {progress}, {'an optic' if optic_at else 'the own factors'}: "
                                f"the original's {want}")
    bone, record = run.eye(weapon)
    report(bone == main_eye[0] and close(record, main_eye[1], 1e-5), "the eye after the switch back: the main eye")
    for before in (0, 1):
        a, b = run.scope_view(weapon, optic, before), stock[0].scope_view(stock[1], s_optic, before)
        report(a == b, f"the scope view on the main sight, +181Eh {before} before: the original's {b}")

    print("the second sight's reticle (weapon::draw 0x245017)")
    idle = parts[-1][1]
    run.press(300_000)
    run.time(300_200, 70_000)                       # s 0.3: the main shown
    run.draw(weapon, WORLD, BONES)
    report(run.byte(idle + 0x10D) == 1 and run.byte(aimed[1] + 0x10D) == 0, "switching to it, s 0.3: its own model")
    run.time(300_350, 70_000)                       # s 0.6
    run.draw(weapon, WORLD, BONES)
    frame = att.frame_of(case["second"][1], BONES[case["second"][0]], WORLD)
    at = [a for op, inst, a in run.rendered if inst == aimed[1] and op == "add"]
    report(run.byte(idle + 0x10D) == 0 and run.byte(aimed[1] + 0x10D) == 1 and at and close(at[0], frame),
           "past half way: the aimed model, at the part's frame, in its place")
    run.emu.mem_write(instance + 0x114, b"\0")
    run.draw(weapon, WORLD, BONES)
    report(run.byte(idle + 0x10D) == 1 and run.byte(aimed[1] + 0x10D) == 0, "out of first person: its own model")
    run.emu.mem_write(instance + 0x114, b"\1")
    run.tick(weapon, 300_500)
    run.press(400_000)
    run.tick(weapon, 400_500)
    run.time(400_500, 80_000)
    run.draw(weapon, WORLD, BONES)
    report(run.byte(idle + 0x10D) == 1 and run.byte(aimed[1] + 0x10D) == 0, "back on the main sight: its own model")

    print("FAILED" if failed else "all passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
