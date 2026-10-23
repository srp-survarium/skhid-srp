#!/usr/bin/env python3
"""Run patch 47, decals on a weapon (weapon_decals_026g.asm), in the default
build: the bake patch 44 asks for, and the lobby's decal editor on the
weapon above the workbench.

    python check_weapon_decals_026g.py <original survarium.exe>

Requires Python `unicorn`. Builds the default exe with patch_026g.py and
loads both exes' sections as they are (the exe is never rebased).

The bake (check_weapon_camo_026g.py's harness, its stubs and made-up
configs; `data/decals` an indexed table of {texture, low, high} as the
merged config holds it): 0x24B0D2 to 0x24B0D7 of the original and of the
build, the game's own unpacking of a decal's u64 (0x263D00) and the
callbacks. Each case must give the registers, flags and stack the original
leaves; with decals (with or without a camouflage) one query a surface, its
P's first two decal layers (P + i*270h) the decal's texture, `_nmap`,
`_fmap` and `_smap` names and the editor's float4x4 of its u64 with the
point 1.0 lower and the two sides halved (a u64 0 or a value that is no
integer: the decal's default, the left or the right side of the receiver),
the camouflage's layer as patch 44's, the name `srpw1_<camouflage>_<model>
_<surface>` and `_<texture>_<high><low>` a decal ('/' and '\\' made '_'); a
third decal, an entry that is no table or has no texture: not painted;
`data/decals` empty and no camouflage: no query.

The editor (a made-up lobby character, lobby menu, inventory, held weapon
with its skeleton instance and bones, and patch 32's and 45's states, patch
45's own `matrix` run as mt_draw draws the weapon at the bench, which
publishes the transform; the game's own 0x263D00, encode 0x264A10 and
find_item_by_id 0x277AA0; stubs: the mouse's ray 0x26D130, the collision
pick 0x264960, the accept 0x263950, the movie call 0x511600, the start's
query 0x264CF0 and the inventory's owner): for armour, a weapon window with
the table off, another item's edit and every site's original state, each
hook does the original's (the pick and the query reached with the stack and
registers the original gives them, the matrix the original's, the bake on
at 0x2641A6 with the original's frame, the key handler on at 0x265C26 with
the original's frame). For the weapon above the bench (spin and tilt of
patch 45's state; the transform patch 45 published, and bone 1's skinning
read from the weapon's render instance, +229Ch, transposed): the pick gives
the point of the ray, in the weapon model's space (the inverse of the
skinning times the published transform), nearest the model's z axis (y +
1.0) and the reversed direction, al 1, `ret 14h` and the callee-saved
registers kept, a miss (behind the eye, 0.25 m off the axis, beyond the
ends, the weapon not yet drawn at the bench) al 0 and nothing written; a
held item that is no weapon: the published transform alone; the preview's
matrix is the weapon's decal times the skinning times the published
transform; the left button's release places (the original's handler, its
bake) after a press within 5 pixels, not after a drag or without a press
(al 0, `ret 0Ch`); the bake's click (1, 0) writes +D548h the game's
encode of the pick, accepts it (host, decal) and calls
root.exit_decal_edit, al 1, `ret 10h`; a miss: al 0, nothing accepted; the
start's and the cancel's calls: al 1, nothing called; the start's index
counts the records of slots 0 and 1 before the edited one, and a record of
another slot ends the edit with no query.
"""
import math
import random
import re
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_EFLAGS

import patch_026g as base
import check_weapon_camo_026g as camo

REGISTERS = camo.REGISTERS
DECODE = 0x263D00
T_INT, T_TABLE, T_LIST, T_STRING = 1, 3, 4, 5
DECAL_A = "character/human/decal/1_1"
DECAL_B = "character/human/decal/5_7"
Y_SHIFT, SIZE = 1.0, 0.5
DEFAULTS = ((0x20080000, 0x8A30CE00), (0x20080200, 0x8A30CE00))
LAYER = 0x270


class BakeRun(camo.Run):
    """Patch 44's harness with indexed tables and `%08x`."""

    def record(self, value):
        if isinstance(value, list):
            records = [self.record(item) for item in value]
            array = self.alloc(b"".join(bytes(self.emu.mem_read(r, 0x18)) for r in records) or b"\0" * 8)
            out = bytearray(0x18)
            struct.pack_into("<I", out, 0, array)
            struct.pack_into("<HH", out, 0x14, T_LIST, len(value))
            return self.alloc(bytes(out))
        if isinstance(value, int) and not isinstance(value, bool):
            out = bytearray(0x18)
            struct.pack_into("<I", out, 0, value & 0xFFFFFFFF)       # the u64's halves, as the merge puts them
            struct.pack_into("<HH", out, 0x14, T_INT, 4)
            return self.alloc(bytes(out))
        return super().record(value)

    def stub(self, emu, address, size, user):
        if camo.STUBS[address][0] != "appendf":
            return super().stub(emu, address, size, user)
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 + 4 * i)
        target, fmt = emu.reg_read(UC_X86_REG_EAX), self.text(arg(0))
        values, k = [], 1
        for spec in re.findall(r"%(?:s|d|08x)", fmt):
            values.append(self.text(arg(k)) if spec == "%s" else
                          struct.unpack("<i", struct.pack("<I", arg(k)))[0] if spec == "%d" else arg(k))
            k += 1
        text = re.sub(r"%(s|d)", "{}", fmt).replace("%08x", "{:08x}").format(*values).encode()
        begin, end, cap = self.string(target)
        text = text[:cap - begin - 1]
        self.emu.mem_write(end, text + b"\0")
        self.word(target + 4, end + len(text))
        self.calls.append(("appendf", fmt, tuple(values)))
        return self.ret(sp, 0, target)

    def layer(self, p, i):
        names = {}
        for field, at in (("diffuse", 0x40), ("normal", 0xCC), ("fresnel", 0x158), ("smoothness", 0x1E4)):
            begin, end, cap = self.string(p + LAYER * i + at)
            names[field] = bytes(self.emu.mem_read(begin, end - begin)).decode()
        return names, struct.unpack("<16f", self.emu.mem_read(p + LAYER * i, 64))

    def decode(self, low, high):
        """The game's own 0x263D00 (out, low, high)."""
        out = self.alloc(b"\0" * 64)
        sp = camo.STACK + 0x20000
        self.emu.mem_write(sp, struct.pack("<4I", camo.STOP_AT, out, low, high))
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(DECODE, camo.STOP_AT, count=1_000_000)
        return struct.unpack("<16f", self.emu.mem_read(out, 64))


def weapon_space(m):
    """The editor's float4x4 of a u64 on a weapon: the point Y_SHIFT lower, the two sides SIZE times."""
    m = list(m)
    m[13] -= Y_SHIFT
    for at in (0, 1, 2, 4, 5, 6):
        m[at] *= SIZE
    return tuple(m)


def close(a, b, eps=2e-5):
    return all(abs(x - y) <= eps * max(1.0, abs(y)) for x, y in zip(a, b))


def check_bake(original, build, say):
    safe = lambda text: text.replace("/", "_").replace("\\", "_")
    v1 = (0x98074123, 0x76961E1C)          # placed values (any u64 the editor gives)
    v2 = (0x20080000 ^ 0x5A5, 0x8A30CE00 ^ 0x1234)
    plain = (
        ("an empty decals table", camo.weapon({"decals": []}), 3),
        ("a decals table that is no table", camo.weapon({"decals": 5}), 3),
    )
    for title, root, surfaces in plain:
        outs = []
        for exe, cls in ((original, BakeRun), (build, BakeRun)):
            run = cls(exe)
            run.surfaces = surfaces
            model, _ = camo.make_render(run)
            outs.append((run.site(camo.config(run, root), model, dict(camo.REGS)), run))
        (o, _), (b, rb) = outs
        say(o == b and not [c for c in rb.calls if c[0] in ("query_resources", "allocate")],
            f"{title}: no query, the registers and the stack as the original's")
    cases = (
        ("one decal", {"decals": [[DECAL_A, v1[0], v1[1]]]}, 2, None, [(DECAL_A, v1)]),
        ("a camouflage and two decals", {"camouflage": camo.CAMO, "decals": [[DECAL_A, v1[0], v1[1]],
                                                                             [DECAL_B, v2[0], v2[1]]]},
         3, camo.CAMO, [(DECAL_A, v1), (DECAL_B, v2)]),
        ("three decals: two", {"decals": [[DECAL_A, v1[0], v1[1]], [DECAL_B, v2[0], v2[1]],
                                          [DECAL_A, v2[0], v2[1]]]}, 1, None, [(DECAL_A, v1), (DECAL_B, v2)]),
        ("never placed: the defaults", {"decals": [[DECAL_A, 0, 0], [DECAL_B, 0, 0]]}, 1, None,
         [(DECAL_A, DEFAULTS[0]), (DECAL_B, DEFAULTS[1])]),
        ("values that are no integers: the defaults", {"decals": [[DECAL_A, "<low_param>", "<high_param>"],
                                                                  [DECAL_B]]}, 1, None,
         [(DECAL_A, DEFAULTS[0]), (DECAL_B, DEFAULTS[1])]),
        ("an entry with no texture, one that is no table", {"decals": [["", 1, 2], 7]}, 1, None, [None, None]),
        ("the second of two only", {"camouflage": camo.CAMO, "decals": [[""], [DECAL_B, v2[0], v2[1]]]}, 1,
         camo.CAMO, [None, (DECAL_B, v2)]),
    )
    for title, data, surfaces, camouflage, decals in cases:
        root = camo.weapon(data)
        o_run = BakeRun(original)
        o_run.surfaces = surfaces
        o_model, _ = camo.make_render(o_run)
        o = o_run.site(camo.config(o_run, root), o_model, dict(camo.REGS))
        run = BakeRun(build)
        run.surfaces = surfaces
        model, render = camo.make_render(run, references=5)
        b = run.site(camo.config(run, root), model, dict(camo.REGS))
        say(o == b and not o_run.calls, f"{title}: the registers, flags and stack as the original's, which asks "
            "for nothing")
        queries = [c for c in run.calls if c[0] == "query_resources"]
        say(len(queries) == surfaces, f"{title}: {surfaces} queries (got {len(queries)})")
        for i, q in enumerate(queries):
            p, (names, _, p_model, surface, _, one, editor) = q[8], q[9]
            want_camo = {"diffuse": camouflage, "normal": camouflage + "_nmap", "fresnel": "",
                         "smoothness": camouflage + "_smap"} if camouflage else \
                {"diffuse": "", "normal": "", "fresnel": "", "smoothness": ""}
            want_name = f"srpw1_{safe(camouflage or '')}_{safe(camo.MODEL)}_{i}" + "".join(
                f"_{safe(tex)}_{high:08x}{low:08x}" for tex, (low, high) in (d for d in decals if d))
            got_camo = {k: v for k, v in names.items() if k != "name"}
            say(got_camo == want_camo and names["name"] == want_name and p_model == render and surface == i
                and one == 1 and editor == 0,
                f"{title} #{i}: the camouflage's layer {got_camo}, the name {names['name']}")
            for layer, decal in enumerate(decals):
                layer_names, matrix = run.layer(p, layer)
                if decal is None:
                    say(layer_names == {"diffuse": "", "normal": "", "fresnel": "", "smoothness": ""},
                        f"{title} #{i}: decal layer {layer} empty")
                    continue
                tex, (low, high) = decal
                want = weapon_space(run.decode(low, high))
                say(layer_names == {"diffuse": tex, "normal": tex + "_nmap", "fresnel": tex + "_fmap",
                                    "smoothness": tex + "_smap"} and close(matrix, want),
                    f"{title} #{i}: decal layer {layer}: {tex} and its weapon float4x4")
        for q in queries:
            run.callback(q[4])
        say(run.read(render + 0xD0) == 5, f"{title}: every reference given back")


# ------------------------------------------------------------------ the editor

STOP_AT, HEAP, STACK = 0x30000000, 0x31000000, 0x32000000
CHARACTER, MENU, MOVIE = 0x36000000, 0x36100000, 0x36200000
PICK, RAY, ACCEPT, MOVIE_CALL, START_QUERY, OWNER_CALL = 0x264960, 0x26D130, 0x263950, 0x511600, 0x264CF0, 0x38000000
BAKE, BAKE_BODY = 0x2641A0, 0x2641A6
PREVIEW_PICK, PREVIEW_DECODE, START_SITE = 0x265999, 0x2659B2, 0x263938
KEY_HANDLER, KEY_BODY = 0x265C20, 0x265C26
WEAPON_VTABLE, SKELETON_VTABLE = 0xA04174, 0x9F0F24
PLAYER = CHARACTER + 0x18000
EXIT_DECAL_EDIT = 0xA0BD7C
ITEM, OTHER = 0x1234567, 0x7654321
INVENTORY = 0x1EA978


class EditRun:
    def __init__(self, data, states):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for name, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            size = (max(vsize, rsize) + 0xFFF) & ~0xFFF
            emu.mem_map(start, size, UC_PROT_ALL)
            if rsize:
                emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((STOP_AT, 0x1000), (HEAP, 0x400000), (STACK, 0x100000), (CHARACTER, 0x40000),
                         (MENU, 0x100000), (MOVIE, 0x1000), (OWNER_CALL, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.preview, self.table = states
        for page in sorted({state & ~0xFFF for state in states}):
            try:                        # the original has no `.srpd`: its code reads no state
                emu.mem_map(page, 0x1000, UC_PROT_ALL)
            except Exception:
                pass
        self.calls, self.heap = [], HEAP
        self.ray = ((0.0, 1.0, -2.0), (0.0, 0.0, 1.0))
        self.pick_answer = 0
        self.lobby_client = self.alloc(b"\0" * (INVENTORY + 16))
        for at in (PICK, RAY, ACCEPT, MOVIE_CALL, START_QUERY, OWNER_CALL):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        for at in (STOP_AT, BAKE_BODY):
            emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: e.emu_stop(), begin=at, end=at)
        # the character's lobby menu, its owner of the inventory, the movie
        self.word(CHARACTER + 0xD2BC, MENU)
        self.word(MENU + 0x9C, MENU + 0x1000)
        self.word(MENU + 0x1000 + 0x53AC, MENU + 0x2000)
        self.word(MENU + 0x2000, MENU + 0x3000)                 # its vtable
        self.word(MENU + 0x3000 + 0x3C, OWNER_CALL)
        self.word(MENU + 0x690, MENU + 0x4000)
        self.word(MENU + 0x4000 + 0x108, MENU + 0x5000)
        self.word(MENU + 0x5000 + 4, MOVIE)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def floats(self, at, n):
        return struct.unpack(f"<{n}f", self.emu.mem_read(at, 4 * n))

    def alloc(self, raw):
        at = self.heap
        self.emu.mem_write(at, raw)
        self.heap += (len(raw) + 15) & ~15
        return at

    def ret(self, sp, taken, result=None):
        if result is not None:
            self.emu.reg_write(UC_X86_REG_EAX, result & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + taken)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 + 4 * i)
        reg = emu.reg_read
        if address == PICK:
            self.calls.append(("pick", tuple(arg(i) for i in range(5)), self.callee_saved()))
            return self.ret(sp, 0x14, self.pick_answer)
        if address == RAY:
            origin, direction = self.ray
            emu.mem_write(arg(0), struct.pack("<3f", *origin))
            emu.mem_write(reg(UC_X86_REG_EDI), struct.pack("<3f", *direction))
            self.calls.append(("ray", reg(UC_X86_REG_ESI)))
            return self.ret(sp, 4)
        if address == ACCEPT:
            self.calls.append(("accept", reg(UC_X86_REG_ESI), arg(0), arg(1), self.read(CHARACTER + 0xD548),
                               self.read(CHARACTER + 0xD54C)))
            return self.ret(sp, 8)
        if address == MOVIE_CALL:
            self.calls.append(("movie", reg(UC_X86_REG_ECX), arg(0), arg(1), arg(2), arg(3)))
            return self.ret(sp, 0x10)
        if address == START_QUERY:
            self.calls.append(("query", reg(UC_X86_REG_ESI), self.callee_saved(), sp))
            return self.ret(sp, 0)
        if address == OWNER_CALL:
            self.calls.append(("owner", reg(UC_X86_REG_ECX)))
            return self.ret(sp, 0, self.lobby_client)
        raise AssertionError(hex(address))

    def callee_saved(self):
        return tuple(self.emu.reg_read(r) for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))

    def set_states(self, table_on, mode, item, spin=0.0, tilt=0.0):
        self.word(self.table, table_on)
        self.emu.mem_write(self.table + 0x20, struct.pack("<2f", spin, tilt))
        self.word(self.preview, mode)
        self.word(self.preview + 4, item)

    def run(self, at, args, regs, until=STOP_AT):
        sp = STACK + 0x80000
        frame = struct.pack(f"<{1 + len(args)}I", STOP_AT, *args)
        self.emu.mem_write(sp - 0x200, b"\xEE" * 0x200)
        self.emu.mem_write(sp, frame + b"\x5A" * 0x40)
        for name, value in regs.items():
            self.emu.reg_write(REGISTERS[name], value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(at, until, count=2_000_000)
        out = {name: self.emu.reg_read(reg) for name, reg in REGISTERS.items()}
        out["eip"] = self.emu.reg_read(UC_X86_REG_EIP)
        out["stack"] = bytes(self.emu.mem_read(out["esp"], sp + len(frame) - out["esp"]))   # what is live
        return out


def mul(a, b):
    """Row-vector 4x4 product a * b."""
    return tuple(sum(a[r * 4 + k] * b[k * 4 + c] for k in range(4)) for r in range(4) for c in range(4))


def rigid(yaw, pitch, move):
    """rotY(yaw) * rotX(pitch), then the move (rows, v * M)."""
    cy, sy, cp, sp = math.cos(yaw), math.sin(yaw), math.cos(pitch), math.sin(pitch)
    ry = (cy, 0, -sy, 0, 0, 1, 0, 0, sy, 0, cy, 0, 0, 0, 0, 1)
    rx = (1, 0, 0, 0, 0, cp, sp, 0, 0, -sp, cp, 0, 0, 0, 0, 1)
    m = list(mul(ry, rx))
    m[12:15] = move
    return tuple(m)


def inverse(m):
    """The inverse of an affine row-vector 4x4 (its 3x3 by cofactors)."""
    r = [m[0:3], m[4:7], m[8:11]]
    cross = lambda a, b: (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
    c0, c1, c2 = cross(r[1], r[2]), cross(r[2], r[0]), cross(r[0], r[1])
    det = sum(r[0][i] * c0[i] for i in range(3))
    cols = [tuple(x / det for x in c) for c in (c0, c1, c2)]
    out = [0.0] * 16
    for i in range(3):
        for j in range(3):
            out[i * 4 + j] = cols[j][i]
    t = m[12:15]
    for j in range(3):
        out[12 + j] = -sum(t[i] * out[i * 4 + j] for i in range(3))
    out[15] = 1.0
    return tuple(out)


BIND_1 = rigid(0.3, 0.1, (0.01, 0.03, 0.05))      # bone 1's bind and its pose of the frame (the PM's
POSE_1 = rigid(0.25, 0.05, (0.0, 0.02, 0.25))     # idle moves its body 20 cm): rigid, as the game's


def draw_at_bench(run, weapon=True, ready=True):
    """Patch 45's `matrix` entry run as mt_draw draws the table's player (its held item a weapon with a
    skeleton instance, bones 0 and 1): it publishes the transform at its state's +50h. Returns (W, S1),
    S1 the skinning of bone 1 the render instance keeps (transposed at +229Ch + 40h)."""
    emu = run.emu
    held = run.alloc(b"\0" * 0x2000)
    model = run.alloc(b"\0" * 0x200)
    instance = run.alloc(b"\0" * 0x6400)
    render_model = run.alloc(b"\0" * 0x200)
    invbinds = run.alloc(struct.pack("<32f", *rigid(0, 0, (0, 0, 0)), *inverse(BIND_1)))
    bones = run.alloc(struct.pack("<32f", *rigid(0, 0, (0, 0, 0)), *POSE_1))
    s1 = mul(inverse(BIND_1), POSE_1)
    transposed = tuple(s1[c * 4 + r] for r in range(4) for c in range(4))
    skinning = run.alloc(struct.pack("<16f", *rigid(0, 0, (0, 0, 0))) + struct.pack("<16f", *transposed))
    run.word(held, WEAPON_VTABLE if weapon else 0x12345678)
    run.word(held + 0x1810, model)
    run.word(model + 0x108, instance)
    run.word(instance, SKELETON_VTABLE)
    run.word(instance + 0x62B4, render_model)
    run.word(instance + 0x229C, skinning)
    run.word(render_model + 0x140, invbinds)
    emu.mem_write(render_model + 0x108, struct.pack("<6f", -0.031, -0.092, -0.265, 0.022, 0.111, 0.673))
    run.word(PLAYER + 0x128, held)
    run.word(CHARACTER + 0xD2B8, PLAYER)
    emu.mem_write(CHARACTER + 0xD515, b"\1")             # the upgrade window open
    run.word(CHARACTER + 0xD2C4, 0xFFFFFFFF if ready else 0x55)
    run.word(CHARACTER + 0xD2C8, 0)
    run.word(run.table + 0x28, 0)                      # READY: set by the draw
    frame = STACK + 0x90000
    emu.mem_write(frame + 4, struct.pack("<I", 0x284A83))
    emu.mem_write(frame - 0xF0, struct.pack("<I", CHARACTER))
    emu.mem_write(frame - 0x0C, struct.pack("<I", bones))
    emu.mem_write(frame - 0x18, struct.pack("<I", 2))
    sp = frame - 0x200
    emu.mem_write(sp, struct.pack("<I", 0x247794))
    emu.reg_write(UC_X86_REG_ESP, sp)
    emu.reg_write(UC_X86_REG_EBP, frame)
    emu.reg_write(UC_X86_REG_EBX, PLAYER)
    camo_va = base.weapon_camo_va(open(base.late_join_bin(), "rb").read())
    hooks = [emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: e.emu_stop(), begin=b, end=e)
             for b, e in ((0x2477C2, 0x2477C8), (0x247794, 0x247794))]
    emu.emu_start(camo_va + base.WEAPON_TABLE_OFFSET + 0xF, 0x2477C8, count=200_000)
    for hook in hooks:
        emu.hook_del(hook)
    at = emu.reg_read(UC_X86_REG_EIP)
    # not yet there: the decal pose's way (10 m under; patch 45 round 5), not the hand
    assert at == (0x2477C8 if ready else 0x247794), hex(at)
    w = run.floats(run.table + base.WEAPON_TABLE_MATRIX_OFFSET, 16)
    return w, (s1 if weapon else rigid(0, 0, (0, 0, 0)))


def apply(m, v, point):
    """v * m (a point: w 1; a vector: w 0), three floats."""
    return tuple(sum(v[i] * m[i * 4 + c] for i in range(3)) + (m[12 + c] if point else 0) for c in range(3))


def expected_pick(m, origin, direction):
    """m: the weapon model's vertices to the world."""
    o, d = apply(inverse(m), origin, True), apply(inverse(m), direction, False)
    n = math.sqrt(sum(x * x for x in d))
    d = tuple(x / n for x in d)
    across = d[0] ** 2 + d[1] ** 2
    t = -(o[0] * d[0] + o[1] * d[1]) / across if across >= 1e-4 else -(o[0] * d[0] + o[1] * d[1] + o[2] * d[2])
    p = [o[i] + t * d[i] for i in range(3)]
    if t < 0 or p[0] ** 2 + p[1] ** 2 > 0.0625 or not -0.6 <= p[2] <= 1.4:
        return None
    return (p[0], p[1] + Y_SHIFT, p[2]), tuple(-x for x in d)


def check_editor(original, build, say):
    late_join = open(base.late_join_bin(), "rb").read()
    preview = base.late_join_state_va(late_join) + base.WEAPON_PREVIEW_STATE_OFFSET
    table = base.late_join_state_va(late_join) + base.WEAPON_TABLE_STATE_OFFSET
    regs = {"eax": 0xA0A0A0A0, "ecx": 0xC0C0C0C0, "edx": 0xD0D0D0D0, "ebx": 0xB0B0B0B0, "ebp": 0xBEBEBEBE,
            "esi": CHARACTER, "edi": 0x7A7A7A7A}
    point, normal = CHARACTER + 0xD530, CHARACTER + 0xD53C

    def prepare(run, upgrade=1, host=ITEM):
        run.emu.mem_write(CHARACTER + 0xD515, bytes([upgrade]))
        run.word(CHARACTER + 0xD520, host)
        run.word(CHARACTER + 0xD524, OTHER)
        run.emu.mem_write(CHARACTER + 0xD518, struct.pack("<2f", 0.15, 0.0))

    # Not the weapon above the bench: each site as the original.
    others = (("the table off", (0, 1, ITEM), 1, ITEM), ("no weapon shown", (1, 0, ITEM), 1, ITEM),
              ("no upgrade window", (1, 1, ITEM), 0, ITEM), ("another item's edit", (1, 1, ITEM), 1, OTHER))
    for title, states, upgrade, host in others:
        results = []
        for exe in (original, build):
            run = EditRun(exe, (preview, table))
            run.set_states(*states, spin=0.4)
            prepare(run, upgrade, host)
            run.emu.mem_write(CHARACTER + 0xD514, b"\1")
            # eax, ecx and edx at the bake's body and the start's query: the body and 0x264CF0 set them
            # before they read them; the bake's stack from its frame up (`and esp,-8` leaves a slot of
            # padding below it, which the body never reads)
            loose = lambda out: {k: (v[out["ebp"] - out["esp"]:] if k == "stack" else v) for k, v in out.items()
                                 if k not in ("eax", "ecx", "edx")}
            out = {"pick": call_site(run, PREVIEW_PICK, (CHARACTER, point, normal, 0x20, 0), regs),
                   "decode": call_site(run, PREVIEW_DECODE, (STACK + 0x60000, 0x98074123, 0x76961E1C), regs,
                                       read=(STACK + 0x60000, 64)),
                   "bake": loose(run.run(BAKE, (CHARACTER, 1, 0, 0), regs, until=BAKE_BODY)),
                   "start": loose(call_site(run, START_SITE, (), regs)),
                   "key": loose(run.run(KEY_HANDLER, (0x4444, 0x151, 1), dict(regs, ecx=CHARACTER),
                                        until=KEY_BODY))}
            results.append((out, run.calls))
        (o, oc), (b, bc) = results
        say(o == b and oc == bc, f"{title}: the pick, the preview's matrix, the bake, the start's query and the key "
            "handler as "
            "the original's" + ("" if o == b and oc == bc else "".join(f"\n          {k}: {o[k]} / {b[k]}" for k in o if o[k] != b[k])))

    rng = random.Random(46)
    for spin, tilt, weapon in ((0.0, 0.0, True), (0.7, 0.3, True), (-2.5, -1.1, True), (3.0, 1.2, True),
                               (0.4, -0.2, False)):
        probe = EditRun(build, (preview, table))
        probe.set_states(1, 1, ITEM, spin, tilt)
        w, s1 = draw_at_bench(probe, weapon)
        m = mul(s1, w)
        what = f"spin {spin}, tilt {tilt}" + ("" if weapon else ", no weapon held")
        # rays at points near the weapon's axis, from all sides, and misses
        for k in range(12):
            target = (rng.uniform(-0.04, 0.04), rng.uniform(-0.12, 0.1), rng.uniform(-0.3, 0.9))
            if k == 9:
                target = (0.3, 0.2, 0.1)                   # 0.36 m off the axis
            if k == 10:
                target = (0.0, 0.0, 1.6)                   # beyond the muzzle's end
            yaw, pitch = rng.uniform(-math.pi, math.pi), rng.uniform(-0.6, 0.6)
            d_local = (math.cos(pitch) * math.cos(yaw), math.sin(pitch), math.cos(pitch) * math.sin(yaw))
            world_point, world_dir = apply(m, target, True), apply(m, d_local, False)
            distance = -1.2 if k == 11 else 1.2           # the eye beyond the weapon: behind it
            origin = tuple(world_point[c] - distance * world_dir[c] for c in range(3))
            run = EditRun(build, (preview, table))
            run.set_states(1, 1, ITEM, spin, tilt)
            draw_at_bench(run, weapon)
            prepare(run)
            run.ray = (origin, world_dir)
            sentinel = struct.pack("<6f", *([123.0] * 6))
            run.emu.mem_write(point, sentinel)
            out = call_site(run, PREVIEW_PICK, (CHARACTER, point, normal, 0x20, 0), regs)
            want = expected_pick(m, origin, world_dir)
            got_point, got_normal = run.floats(point, 3), run.floats(normal, 3)
            if want is None:
                say(out["eax"] & 0xFF == 0 and bytes(run.emu.mem_read(point, 24)) == sentinel,
                    f"{what}, ray {k}: a miss, al 0, nothing written")
            else:
                say(out["eax"] & 0xFF == 1 and close(got_point, want[0], 1e-4) and close(got_normal, want[1], 1e-4),
                    f"{what}, ray {k}: the point {tuple(round(x, 4) for x in got_point)}, "
                    f"the normal the ray reversed")
            say(out["kept"], f"{what}, ray {k}: ret 14h, ebx, esi, edi, ebp kept")
        # the preview's matrix
        run = EditRun(build, (preview, table))
        run.set_states(1, 1, ITEM, spin, tilt)
        draw_at_bench(run, weapon)
        prepare(run)
        out_at = STACK + 0x60000
        for value in ((0x98074123, 0x76961E1C), DEFAULTS[1]):
            call_site(run, PREVIEW_DECODE, (out_at, *value), regs)
            got = run.floats(out_at, 16)
            ref = BakeRun(build)
            want = mul(mul(weapon_space(ref.decode(*value)), s1), w)
            say(close(got, want, 1e-4), f"{what}: the preview's box is the weapon's decal "
                f"{value[1]:08x}{value[0]:08x} at the weapon's transform at the bench")

    # Before patch 45 has drawn the weapon at the bench (READY 0): every ray misses.
    run = EditRun(build, (preview, table))
    run.set_states(1, 1, ITEM)
    draw_at_bench(run, ready=False)
    prepare(run)
    run.ray = (apply(m, (0.0, 0.0, 0.2), True), (0.0, 0.0, 1.0))
    out = call_site(run, PREVIEW_PICK, (CHARACTER, point, normal, 0x20, 0), regs)
    say(out["eax"] & 0xFF == 0 and out["kept"] and run.read(table + 0x28) == 0,
        "the weapon not yet drawn at the bench (READY 0): a miss")

    # The bake's calls for the weapon.
    probe = EditRun(build, (preview, table))
    probe.set_states(1, 1, ITEM)
    w, s1 = draw_at_bench(probe)
    m = mul(s1, w)
    target, d_local = (0.01, 0.05, 0.15), (1.0, 0.0, 0.0)
    world_point, world_dir = apply(m, target, True), apply(m, d_local, False)
    origin = tuple(world_point[c] - 1.0 * world_dir[c] for c in range(3))
    for title, args, hit in (("the click", (CHARACTER, 1, 0, 0), True), ("a click off the weapon", (CHARACTER, 1, 0, 0),
                                                                          False),
                             ("the start's bake", (CHARACTER, 0x98074123, 0x76961E1C, 1), None),
                             ("the cancel's bake", (CHARACTER, 0, 0, 0), None)):
        run = EditRun(build, (preview, table))
        run.set_states(1, 1, ITEM)
        draw_at_bench(run)
        prepare(run)
        run.ray = (origin, world_dir) if hit is not False else ((0.0, 5.0, 0.0), (0.0, 1.0, 0.0))
        out = run.run(BAKE, args, regs)
        accepted = [c for c in run.calls if c[0] in ("accept", "movie")]
        kept = all(out[r] == regs[r] for r in ("ebx", "esi", "edi", "ebp")) and \
            out["esp"] == STACK + 0x80000 + 4 + 0x10
        if hit:
            value = run.read(CHARACTER + 0xD548) | run.read(CHARACTER + 0xD54C) << 32
            decoded = BakeRun(build).decode(value & 0xFFFFFFFF, value >> 32)
            place = (decoded[12] + 0.75 * decoded[8] / 1.5, decoded[13] + 0.75 * decoded[9] / 1.5 - Y_SHIFT,
                     decoded[14] + 0.75 * decoded[10] / 1.5)
            want_place = expected_pick(m, origin, world_dir)[0]
            want_place = (want_place[0], want_place[1] - Y_SHIFT, want_place[2])
            say(out["eax"] & 0xFF == 1 and kept and close(place, want_place, 3e-3)
                and close(tuple(x / 1.5 for x in decoded[8:11]), d_local, 1e-2)
                and accepted == [("accept", CHARACTER, ITEM, OTHER, value & 0xFFFFFFFF, value >> 32),
                                 ("movie", MOVIE, EXIT_DECAL_EDIT, 0, 0, 0)],
                f"{title}: the u64 the game's encode makes of the pick ({tuple(round(x, 4) for x in place)}), "
                "accepted for the host and the decal, root.exit_decal_edit, al 1, ret 10h")
        else:
            say((out["eax"] & 0xFF) == (0 if hit is False else 1) and kept and not accepted
                and not [c for c in run.calls if c[0] == "pick"],
                f"{title}: al {0 if hit is False else 1}, ret 10h, nothing accepted")

    # The left button in a weapon's edit: a click places (the original's release, its bake), a drag not.
    for title, moves, press, editing, places in (
            ("a click", (3, -2), True, True, True),
            ("a drag", (30, 4), True, True, False),
            ("a release with no press in the edit (the movie's change)", (0, 0), False, True, False),
            ("not editing", (30, 4), True, False, False)):
        run = EditRun(build, (preview, table))
        run.set_states(1, 1, ITEM)
        draw_at_bench(run)
        prepare(run)
        run.emu.mem_write(CHARACTER + 0xD514, bytes([editing]))
        bakes = []
        run.emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: (bakes.append(a), e.emu_stop()), begin=BAKE, end=BAKE)
        run.word(MENU + 0x8C, 400)
        run.word(MENU + 0x90, 300)
        key_regs = dict(regs, ecx=CHARACTER)
        down = run.run(KEY_HANDLER, (0x4444, 0x151, 0), key_regs) if press else None
        run.word(MENU + 0x8C, 400 + moves[0])
        run.word(MENU + 0x90, 300 + moves[1])
        up = run.run(KEY_HANDLER, (0x4444, 0x151, 1), key_regs)
        down_ok = down is None or (down["eax"] & 0xFF == 0 and down["esp"] == STACK + 0x80000 + 4 + 0xC
                                   and down["eip"] == STOP_AT)
        if places:
            ok = down_ok and bakes == [BAKE]
        else:
            ok = down_ok and not bakes and up["eax"] & 0xFF == 0 and up["esp"] == STACK + 0x80000 + 4 + 0xC
        say(ok and all(up[r] == key_regs[r] for r in ("ebx", "esi", "edi", "ebp")) if not places else ok,
            f"{title}: " + ("the release goes on to the original's bake (1, 0, 0)" if places
                            else "the release places nothing, al 0, ret 0Ch"))
        say(run.read(base.late_join_state_va(late_join) + base.WEAPON_DECALS_STATE_OFFSET) == 0 or not editing,
            f"{title}: the press is over after the release")

    # The start's index.
    stock = ((3, 0x11), (2, 0x22), (0, 0x33), (1, 0x44))       # scope, camouflage, two decals
    moved = ((3, 0x11), (1, 0x44), (2, 0x22))                   # the owner's Mosin: a decal dragged to slot 1,
    #                                                             its record pushed last, the camouflage after
    for title, records, raw, weapon_window, want_index, query in (
            ("the first decal after a scope and a camouflage", stock, 2, True, 0, True),
            ("the second", stock, 3, True, 1, True),
            ("the camouflage's record", stock, 1, True, None, False),
            ("armour's window", stock, 3, False, 7, True),
            ("a decal moved to slot 1, alone", moved, 1, True, 0, True)):
        run = EditRun(build, (preview, table))
        run.set_states(1 if weapon_window else 0, 1, ITEM)
        prepare(run)
        item = run.alloc(struct.pack("<I", ITEM) + b"\0" * 0xB0)
        recs = run.alloc(b"".join(struct.pack("<IIIIB3x", rid, 0, 0, 0, slot) + b"\0" * 4 for slot, rid in records))
        run.word(item + 0x60, recs)
        run.word(item + 0x64, recs + 0x18 * len(records))
        other = run.alloc(struct.pack("<I", OTHER) + b"\0" * 0xB0)
        items = run.alloc(bytes(run.emu.mem_read(other, 0xB4)) + bytes(run.emu.mem_read(item, 0xB4)))
        run.word(run.lobby_client + INVENTORY, items)
        run.word(run.lobby_client + INVENTORY + 4, items + 2 * 0xB4)
        run.emu.mem_write(CHARACTER + 0xD52C, bytes([7, raw]))
        run.emu.mem_write(CHARACTER + 0xD514, b"\1")
        out = call_site(run, START_SITE, (), regs)
        queries = [c for c in run.calls if c[0] == "query"]
        index, editing = run.emu.mem_read(CHARACTER + 0xD52C, 1)[0], run.emu.mem_read(CHARACTER + 0xD514, 1)[0]
        if query:
            say(index == want_index and editing == 1 and len(queries) == 1 and queries[0][1] == CHARACTER
                and queries[0][2] == (regs["ebx"], CHARACTER, regs["edi"], regs["ebp"]) and out["kept"],
                f"{title}: the decal's index {index}, then the query with the original's registers")
        else:
            say(editing == 0 and not queries and out["kept"], f"{title}: the edit is off, no query")


def call_site(run, site, args, regs, read=None):
    """The instruction at `site` (a call of the original) run as from its function: the arguments pushed,
    stopped when it returns to the next instruction."""
    after = site + 5
    sp = STACK + 0x80000
    run.emu.mem_write(sp - 0x200, b"\xEE" * 0x200)
    run.emu.mem_write(sp, struct.pack(f"<{len(args)}I", *args) + b"\x5A" * 0x20 if args else b"\x5A" * 0x20)
    for name, value in regs.items():
        run.emu.reg_write(REGISTERS[name], value)
    run.emu.reg_write(UC_X86_REG_ESP, sp)
    hook = run.emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: e.emu_stop(), begin=after, end=after)
    run.emu.emu_start(site, after, count=2_000_000)
    run.emu.hook_del(hook)
    out = {name: run.emu.reg_read(reg) for name, reg in REGISTERS.items()}
    out["eip"] = run.emu.reg_read(UC_X86_REG_EIP)
    taken = {PREVIEW_PICK: 0x14, PREVIEW_DECODE: 0, START_SITE: 0}[site]
    out["kept"] = out["eip"] == after and out["esp"] == sp + taken and \
        all(out[r] == regs[r] for r in ("ebx", "esi", "edi", "ebp"))
    if read:
        out["read"] = bytes(run.emu.mem_read(*read))
    out.pop("eax"), out.pop("ecx"), out.pop("edx")
    out["eax"] = run.emu.reg_read(UC_X86_REG_EAX)
    return out


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    build, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), late_join=late_join)
    failures = 0

    def say(ok, text):
        nonlocal failures
        failures += not ok
        print(("ok    " if ok else "FAIL  ") + text)

    check_bake(original, build, say)
    check_editor(original, build, say)
    print("all checks passed" if not failures else f"{failures} FAILED")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
