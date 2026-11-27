#!/usr/bin/env python3
"""Run patch `reticle`'s drawing (rust/patches/src/reticle/draw.rs over vostok/reticle.rs, its hooks
of reticle.toml; ../docs/notes/reticle-choice.md, "The drawing") in the default build on made-up game
objects, the original beside it.

    python check_reticle_026g.py <original survarium.exe>

Requires Python `unicorn`. check_attachments_026g.py's harness: the modules table is the
weapon-modules crate's sample (the AK-74M with the sample's EOTech, a collimator, its eye in the
gun's eye plane), the weapon cooked and called back as there. The reticle table is fed by the
server's A9 at the lobby's dispatcher (0x274A39, as check_reticle_table_026g.py sends it). Stubbed
besides the attachments harness's: the scene renderer's register_material_parameter (0x24D30),
set_model_material_parameter_impl (0x24E60) and append_model_material (0x24FE0), get_all_surfaces
(0x159700: the pattern model's one surface instance) and query_resources (0xE5B60), each recorded.

- weapon::draw's end (0x245017, after patch 161's draw) of the fitted weapon, its optic's aimed
  model in the scene: the first draw wants the pattern and draws nothing; the patch's handler of
  game::tick's frame site (0x1EC8EC, through its shim) asks for its model alone (class 15); the patch's callback takes it (a reference); the
  next draw registers `solid_color_specular` (a float4) once, appends the model's own surface's
  material effects to it (index 0), sets the appended emission (the colour times the brightness's
  multiplier, 20 at 6, and 1) and adds the model to the scene, drawn first-person and movable; the
  draws after move it (an update) and set nothing again;
- its frame: the size's scale times the optic's `aim` place times the aimed model's frame (+660h):
  two draws at two frames give one place in the optic's frame, scaled, in the eye's axes (the
  table's eye of that fit, by the optic's anchor); for an optic whose eye is its own locator (the
  sample's PSO-1, given lists here), at that locator's origin (the eye is the locator after a half
  turn about y on the optic's frame, Placement::child);
- a new choice (A9 again: another colour, size and brightness): the emission set again, the scale
  the new one, nothing appended again;
- the optic's aimed model out of the scene: the reticle removed, once; back in: added again;
  weapon::on_hide (0x244017): removed;
- another weapon without an aimed optic drawn meanwhile: the reticle stays;
- a scope the table has no lists for: nothing drawn, nothing asked;
- the original: no call of the three, no query.
"""
import math
import os
import struct
import sys

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
import check_attachments_026g as A

REGISTER, SET_PARAMETER, APPEND, SURFACES, QUERY = 0x24D30, 0x24E60, 0x24FE0, 0x159700, 0xE5B60
DISPATCH, DISPATCH_DONE, DISPATCH_JUMP = 0x274A39, 0x274A30, 0x274A3F
OPTICS, AIM_MODEL = 0x1808, 0x10C
ASSETS_WORD = 3
AK, EOTECH, PSO1 = 3, 1771, 1755
SIZES = (1.0, 0.8, 0.6, 0.4)
RED, GREEN = ("red", (1.0, 0.0, 0.0)), ("green", (0.0, 1.0, 0.0))
MULTIPLIERS = {6: 20.0, 9: 56.568542, 0: 2.5}
SCOPE_ITEM = 90


def text(value):
    return bytes([len(value)]) + value.encode()


def message(choices, scopes=(1771,)):
    """A9's body: `scopes` (the sample's EOTech) with lists, the account's EOTech items' choices."""
    out = bytes([len(scopes)])
    for scope in scopes:
        out += struct.pack("<HB", scope, 2)
        for name in ("dot", "eotech_ring"):
            out += text(name) + text("weapons/reticles_069/" + name)
        out += bytes([len(SIZES)]) + b"".join(struct.pack("<f", s) for s in SIZES)
        out += bytes([2]) + b"".join(text(name) + struct.pack("<3f", *rgb) for name, rgb in (RED, GREEN))
    out += struct.pack("<H", len(choices))
    for item, numbers in choices:
        out += struct.pack("<IH", item, EOTECH) + bytes(numbers)
    return out


def inverse(m):
    """A rigid (rotation and translation, row vectors) matrix's inverse."""
    r = [[m[4 * i + j] for j in range(3)] for i in range(3)]
    t = m[12:15]
    rt = [[r[j][i] for j in range(3)] for i in range(3)]
    it = [-sum(t[k] * rt[k][j] for k in range(3)) for j in range(3)]
    return [rt[0][0], rt[0][1], rt[0][2], 0, rt[1][0], rt[1][1], rt[1][2], 0, rt[2][0], rt[2][1], rt[2][2], 0,
            it[0], it[1], it[2], 1]


class Run(A.Run):
    def __init__(self, data):
        super().__init__(data)
        self.calls, self.queries = [], []
        for at in (REGISTER, SET_PARAMETER, APPEND, SURFACES, QUERY):
            self.emu.hook_add(UC_HOOK_CODE, self.render_stub, begin=at, end=at)
        for at in (DISPATCH_DONE, DISPATCH_JUMP):
            self.emu.hook_add(UC_HOOK_CODE, self.stop, begin=at, end=at)
        self.surface_instance = 0

    def render_stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.u32(sp + 4 * n)
        if address == REGISTER:
            handle = arg(2)
            self.calls.append(("register", arg(1), handle, self.text(self.u32(handle + 4)).decode(), self.u32(handle + 8)))
            pops = 8
        elif address == APPEND:
            self.calls.append(("append", arg(1), arg(2), self.u32(arg(3)), arg(4)))
            pops = 0x10
        elif address == SET_PARAMETER:
            self.calls.append(("set", arg(1), arg(2), self.u32(arg(3)), arg(4), arg(5),
                               tuple(round(v, 4) for v in self.floats(arg(6), arg(7) // 4))))
            pops = 0x1C
        elif address == SURFACES:          # (instance, &buffer_vector): the model's one surface
            vector = arg(2)
            begin = self.u32(vector)
            self.u32(begin, self.surface_instance)
            self.u32(vector + 4, begin + 4)
            self.calls.append(("surfaces", arg(1)))
            pops = 8
        else:                              # query_resources: EAX the requests, ECX their count; the caller pops
            requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
            self.queries.append([(self.text(self.u32(requests + 8 * i)).decode(), self.u32(requests + 8 * i + 4))
                                 for i in range(count)])
            pops = 0
        emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def dispatch(self, body):
        """lobby_client::on_message_received past its messaging ids with A9 and a reader of `body`:
        where it went."""
        reader, data, frame, client = self.block(0x10), self.block(len(body) + 4), self.block(0x40), self.block(0x40)
        self.emu.mem_write(data, body)
        self.emu.mem_write(reader, struct.pack("<III", data, data, len(body)))
        self.u32(frame + 0xC, reader)
        sp = A.STACK - 0x8000
        return self.run(DISPATCH, (DISPATCH_DONE, DISPATCH_JUMP), sp, edx=0xA9, ebx=client, ebp=frame)

    def tick(self):
        """The patch's handler of game::tick's frame site (0x1EC8EC), through its generated shim
        (cdecl, the entry's saved machine: zeroed, EBX a zeroed game); the site's other handlers
        are other patches'."""
        frame = (self.block(0x1100) + 0xF) & ~0xF
        sp = A.STACK - 0x9000
        self.emu.mem_write(sp, struct.pack("<2I", A.STOP, frame))
        return self.run(base.arena().symbols["hook_reticle_on_tick"], (A.STOP,), sp)

    def answer(self):
        """The patch's query callback with the pattern's model: its holder, and its one surface's
        material effects."""
        holder, instance = self.part()
        surface, effects = self.block(0x100), self.block(0x100)
        self.surface_instance = self.block(0x80)
        self.u32(self.surface_instance + 0x38, surface)
        self.u32(surface + 0x1C, effects)
        results = self.block(0x12C + 0x2E0)
        self.u32(results + 0x38, 1)
        self.u32(results + 0x12C, holder)
        sp = A.STACK - 0xA000
        self.emu.mem_write(sp, struct.pack("<3I", A.STOP, results, ASSETS_WORD))
        self.run(base.arena().symbols["reticle_on_loaded"], (A.STOP,), sp)
        return holder, instance, effects

    def hidden(self, weapon):
        return self.run(A.HIDDEN, (A.HIDDEN_END,), A.STACK - 0x5000, esi=weapon)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None, late_join)
    built = bytes(built)
    table = (A.SAMPLE / "sample-v4.srpm").read_bytes()
    cases = A.loadouts()
    failed = 0

    def report(ok, what):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}")

    def named(gun, scope):
        return {"srp_gun": gun, "srp_scope": scope, "srp_muzzle": 0, "srp_grip": 0}

    natives = 5      # the native requests of check_attachments_026g's config (NATIVE)

    def fitted(data, key):
        """A weapon cooked with `key` fitted and called back, its optic's aimed model in the scene:
        the run, the weapon, the aimed model's instance."""
        run = Run(data)
        run.dictionary_configs(6, table)
        case = cases[key]
        weapon, _ = run.weapon(4, [(1, A.translate(0.0, 0.042, -0.636)), (1, A.translate(0.0, 0.07, 0.02)),
                                   (0, A.translate(0.0, 0.08, 0.1))])
        resource, _ = run.config(named(*key[:2]))
        if run.cook(resource, weapon) is None:
            return None
        parts = [run.part() for _ in case["parts"]] if isinstance(case, dict) else []
        scope = [run.block(0x120)] if key[1] in A.SCOPE_ITEMS else []
        results = scope + [run.block(0x120) for _ in range(natives)] + [h for h, _ in parts]
        if run.callback(weapon, resource, results) != A.LOADED_END:
            return None
        optics, (aimed, aimed_instance) = run.block(0x120), run.part()
        run.u32(weapon + OPTICS, optics)
        run.u32(optics + AIM_MODEL, aimed)
        run.emu.mem_write(aimed_instance + 0x10D, b"\1")
        return run, weapon, aimed_instance

    W1 = A.multiply(A.rotation_y(0.3), A.translate(1.0, 1.5, -2.0))
    W2 = A.multiply(A.rotation_y(-0.6), A.translate(-3.0, 0.5, 4.0))
    BONES = [A.IDENTITY] * 4

    def draw(run, weapon, frame):
        run.put_floats(weapon + A.FRAME, frame)
        run.calls = []
        stopped = run.draw(weapon, A.IDENTITY, BONES)
        return stopped, [c for c in run.calls if c[0] != "surfaces"], [r for r in run.rendered]

    print("the drawing (weapon::draw's end 0x245017, the frame site 0x1EC8EC, the query's callback)")
    made = fitted(built, (AK, EOTECH, 0, 0))
    if made is None:
        report(False, "the EOTech on the AK-74M: the cook or its callback did not reach its sites")
        sys.exit(1)
    run, weapon, aimed_instance = made
    report(run.dispatch(message([])) == DISPATCH_DONE, "A9 taken at the dispatcher (no choice: the lists' default)")
    stopped, calls, rendered = draw(run, weapon, W1)
    report(stopped == A.DRAW_END and calls == [] and run.queries == [],
           "the first draw: the pattern wanted, nothing drawn, nothing asked yet")
    run.tick()
    report(run.queries == [[("weapons/reticles_069/dot", 15)]], f"the frame site asks for the dot's model alone, class 15 ({run.queries})")
    run.tick()
    report(len(run.queries) == 1, "one query at a time: the next frame asks nothing more")
    holder, instance, effects = run.answer()
    report(run.u32(holder + 0xD0) == 2, "the callback keeps the model (a reference of its own)")

    stopped, calls, rendered = draw(run, weapon, W1)
    ref, renderer = holder + 0x108, run.u32(A.RENDERER + 0x200064)
    register = [c for c in calls if c[0] == "register"]
    handle = register[0][2] if register else None
    want = [("register", renderer, handle, "solid_color_specular", 1),
            ("append", renderer, ref, effects, 0),
            ("set", renderer, ref, effects, 0, handle, (20.0, 0.0, 0.0, 1.0))]
    report(calls == want, "the next draw registers solid_color_specular (a float4), appends the surface's material "
                          f"effects (index 0) and sets their emission, red at 20 ({calls})")
    ours = [(op, at) for op, inst, at in rendered if inst == instance]
    report(len(ours) == 1 and ours[0][0] == "add" and run.byte(instance + 0x114) == 1 and run.byte(instance + 0x115) == 1,
           "the model added to the scene, first-person and movable")
    first = ours[0][1] if ours else None

    stopped, calls, rendered = draw(run, weapon, W2)
    ours = [(op, at) for op, inst, at in rendered if inst == instance]
    report(calls == [] and len(ours) == 1 and ours[0][0] == "update", "the next draw: moved (an update), nothing set again")
    second = ours[0][1] if ours else None
    if first and second:
        a1, a2 = A.multiply(first, inverse(W1)), A.multiply(second, inverse(W2))
        scale = math.sqrt(sum(v * v for v in a1[0:3]))
        report(A.close(a1, a2, 1e-4) and abs(scale - 1.0) < 1e-4,
               f"one place in the optic's frame at both frames, scale {scale:.4f} (size 0, 1.0)")
        _, eye = cases[(AK, EOTECH, 0, 0)]["eye"]["optic"]
        report(A.close(A.multiply(a1, cases[(AK, EOTECH, 0, 0)]["optic"][1])[0:11], eye[0:11], 1e-4),
               "its axes the eye's (the EOTech's eye in the gun's plane)")

    report(run.dispatch(message([(SCOPE_ITEM, (0, 2, 1, 9))])) == DISPATCH_DONE, "A9 again: green, size 2, brightness 9")
    stopped, calls, rendered = draw(run, weapon, W1)
    ours = [(op, at) for op, inst, at in rendered if inst == instance]
    report(calls == [("set", renderer, ref, effects, 0, handle, (0.0, round(MULTIPLIERS[9], 4), 0.0, 1.0))],
           f"the emission set again, green at 56.57, nothing appended ({calls})")
    if ours and ours[0][1]:
        scale = math.sqrt(sum(v * v for v in A.multiply(ours[0][1], inverse(W1))[0:3]))
        report(abs(scale - SIZES[2]) < 1e-4, f"the scale the new size's ({scale:.4f})")

    other = run.weapon(4, [(1, A.translate(0, 0, 0))] * 3)[0]
    run.calls = []
    run.rendered = []
    run.draw(other, A.IDENTITY, BONES)
    report(not [r for r in run.rendered if r[1] == instance], "another weapon drawn without an aimed optic: the reticle stays")

    run.emu.mem_write(aimed_instance + 0x10D, b"\0")
    stopped, calls, rendered = draw(run, weapon, W1)
    report([(op, inst) for op, inst, _ in rendered if inst == instance] == [("remove", instance)],
           "the optic's aimed model out of the scene: the reticle removed")
    stopped, calls, rendered = draw(run, weapon, W1)
    report(not [r for r in rendered if r[1] == instance], "and not again")
    run.emu.mem_write(aimed_instance + 0x10D, b"\1")
    stopped, calls, rendered = draw(run, weapon, W1)
    report([(op, inst) for op, inst, _ in rendered if inst == instance] == [("add", instance)] and calls == [],
           "back in: added again, nothing set again")
    run.rendered = []
    report(run.hidden(weapon) == A.HIDDEN_END and [(op, inst) for op, inst, _ in run.rendered if inst == instance]
           == [("remove", instance)], "weapon::on_hide: removed")

    made = fitted(built, (AK, PSO1, 0, 0))
    if made is None:
        report(False, "the PSO-1 on the AK-74M: the cook or its callback did not reach its sites")
    else:
        # an optic whose eye is its own locator (no plane): the reticle at that locator, in its axes
        run, weapon, _ = made
        run.dispatch(message([], scopes=(PSO1,)))
        draw(run, weapon, W1)
        run.tick()
        holder, instance, _ = run.answer()
        stopped, calls, rendered = draw(run, weapon, W1)
        ours = [at for _, inst, at in rendered if inst == instance]
        case = cases[(AK, PSO1, 0, 0)]
        optic, (_, eye) = case["optic"][1], case["eye"]["optic"]
        # the eye on its bone is the table's locator L after a half turn about y on the optic's
        # frame (Placement::child): L = eye x optic^-1 x H; the reticle stands at L's origin
        local = A.multiply(eye, inverse(optic))
        half = [-1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1, 0, 0, 0, 0, 1]
        origin = A.multiply(local, half)[12:15]
        if ours:
            got = A.multiply(ours[0], inverse(W1))
            report(A.close(got[0:11], local[0:11], 1e-4) and A.close(got[12:15], origin, 1e-4),
                   "the PSO-1 (its own eye): the reticle at its locator's origin, in the eye's axes")
        else:
            report(False, "the PSO-1 (its own eye): no reticle drawn")

        made = fitted(built, (AK, EOTECH, 0, 0))
        run, weapon, _ = made
        run.dispatch(message([], scopes=(PSO1,)))
        stopped, calls, rendered = draw(run, weapon, W1)
        run.tick()
        report(calls == [] and run.queries == [], "a scope the table has no lists for: nothing drawn, nothing asked")

    made = fitted(original, (AK, EOTECH, 0, 0))
    if made is None:
        report(False, "the original: the cook or its callback did not run")
    else:
        run, weapon, _ = made
        stopped, calls, rendered = draw(run, weapon, W1)
        draw(run, weapon, W1)
        report(stopped == A.DRAW_END and run.calls == [] and run.queries == [], "the original: no call of the three, no query")

    print("FAILED" if failed else "all ok")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
