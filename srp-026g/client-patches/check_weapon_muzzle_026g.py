#!/usr/bin/env python3
"""Run patch 68's muzzle device (weapon_muzzle_026g.asm and .c; client_patch.md
section 3.56, ../docs/notes/weapon-kit.md) in the default build and the
original, from the game's own sites, on made-up weapons and configs.

    python check_weapon_muzzle_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). The exe's sections are loaded as they are,
the arena with them. Stubs that write down how they were called: the
configs' has and child (0x0F6C00, 0x0F6B10) over made-up configs, the
allocator (0x0E4F90), the weapon's constructors (0x243640, 0x24A400), the
scene's add_model, remove_model and update_model (0x21CD0, 0x21F40,
0x22280), the resource release (0x0E0210). What must hold:

- the cook's last request (0x24ABF9 to 0x24AC00, the game's own push
  0x5FCB0): a config with data/srp_muzzle_model gets one more request {that
  path, class 0Fh}; one without none; ecx the original's;
- the requests' alloca holds one more (0x24AA04: +2), and weapon_cook
  0x24A780 (the game's own, from its entry to its `ret`) asks the allocator
  for 4710h + 4 a pointer + 50h bytes, and the weapon's part after the last
  array is zeroes (the memory was not), the arrays as the original makes them;
- the query's callback entry (0x24ACF0 to 0x24ACF5): with the model named,
  the last result (after the scope's when the config names one, the model,
  the user animations, the shells and the shot effects) is kept in the
  weapon's part with its count up, the offset from data/srp_muzzle_offset
  and the fire point from data/srp_muzzle_fire_mm (millimetres, an int
  or a float, 0 without); without the model nothing; the registers and stack as the
  original's;
- the weapon model's add (on_show 0x243F37, remove_aimed 0x244CF5), remove
  (on_hide 0x244012, add_aimed 0x244C08) and update (draw 0x244F3A): the
  original's call with its arguments, then the device's (its render model
  +108h, the same channel and scene, its own matrix: barrel_point's with
  the point moved the offset forward, against the third row), each once
  (an add while added, a remove while removed: nothing more); without a
  device only the original's; back after the call with the original's pops;
  after the update (only), barrel_point's point (+0x620) moved offset + fire
  point forward (the shot's flash), the rest of its matrix kept; after the
  others barrel_point as it was;
- delete_resource (0x24B23A to 0x24B240): the device's model released (its
  count down; at zero the game's release), the part's pointer zero, ebx the
  user animations' count as the original's.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

HAS, CHILD, ALLOCATE, WEAPON_CTOR, PART_CTOR = 0x0F6C00, 0x0F6B10, 0x0E4F90, 0x243640, 0x24A400
ADD, REMOVE, UPDATE, DESTROY = 0x21CD0, 0x21F40, 0x22280, 0x0E0210
STACK, STOP = 0x21000000, 0x21100000
CONFIG, WEAPON, RESULTS, HEAP, STRINGS, MODEL = 0x30000000, 0x30100000, 0x30200000, 0x30300000, 0x30400000, \
    0x30500000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
EXTENSION = 0x50
T_INT, T_FLOAT, T_STRING = 1, 2, 5


def floats(data):
    return list(struct.unpack(f"<{len(data) // 4}f", data))


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (CONFIG, WEAPON, RESULTS, HEAP, STRINGS, MODEL, STOP):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        emu.mem_map(STACK - 0x40000, 0x80000, UC_PROT_ALL)
        for at in (HAS, CHILD, ALLOCATE, WEAPON_CTOR, PART_CTOR, ADD, REMOVE, UPDATE, DESTROY):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, self.stop)
        self.stops, self.calls, self.tables, self.strings = (), [], {}, {}

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0]

    def ret(self, value, pops):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.u32(sp + 4 * i) for i in range(1, 5)]
        eax = emu.reg_read(UC_X86_REG_EAX)
        if address == HAS:                  # eax the table, esi the key: al
            self.ret(int((eax, self.text(emu.reg_read(UC_X86_REG_ESI))) in self.tables), 0)
        elif address == CHILD:              # eax the table, edi the key: the child
            self.ret(self.tables[(eax, self.text(emu.reg_read(UC_X86_REG_EDI)))], 0)
        elif address == ALLOCATE:           # esi the allocator, eax the size, [esp+4] where; ret 4
            self.calls.append(("allocate", eax))
            emu.mem_write(HEAP, b"\xAA" * 0x8000)
            self.ret(HEAP, 4)
        elif address == WEAPON_CTOR:        # (the memory, ...): this; ret 0Ch
            self.ret(args[0], 12)
        elif address == PART_CTOR:          # thiscall: ecx; ret 4
            self.ret(emu.reg_read(UC_X86_REG_ECX), 4)
        elif address in (ADD, UPDATE):      # eax the render model; (channel, &scene, &m, &m); ret 10h
            m = floats(bytes(emu.mem_read(args[2], 64)))
            self.calls.append(("add" if address == ADD else "update", eax, args[0], args[1], args[2], args[3], m))
            self.ret(0, 16)
        elif address == REMOVE:             # (channel, &scene); ret 8
            self.calls.append(("remove", eax, args[0], args[1]))
            self.ret(0, 8)
        else:                               # the release: eax the count's place
            self.calls.append(("destroy", eax))
            self.ret(0, 0)

    def stop(self, emu, address, size, _):
        if address in self.stops and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def run(self, start, stops, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.calls = stops, None, False, []
        self.emu.emu_start(start, 0, count=300000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def node(self, at, value, kind, count=0):
        self.emu.mem_write(at, struct.pack("<I16xHH", value, kind, count))

    def number(self, table, key, at, value):
        if value is None:
            return
        self.tables[(table, key)] = at
        if isinstance(value, float):
            self.node(at, struct.unpack("<I", struct.pack("<f", value))[0], T_FLOAT)
        else:
            self.node(at, value, T_INT)

    def config(self, model=None, offset=None, scope=False, previews=2, fire=None):
        """A weapon config's root at CONFIG: data (srp_muzzle_model,
        srp_muzzle_offset, srp_muzzle_fire_mm), addons (rifle_scope_dict_id), user_animations_in_place/
        preview of `previews`."""
        self.emu.mem_write(CONFIG, bytes(0x4000))
        self.emu.mem_write(STRINGS, bytes(0x1000))
        root, data, addons, anims, preview = CONFIG, CONFIG + 0x100, CONFIG + 0x200, CONFIG + 0x300, CONFIG + 0x400
        self.tables = {(root, b"data"): data, (root, b"user_animations_in_place"): anims,
                       (anims, b"preview"): preview}
        self.node(preview, 0, 4, previews)
        if scope:
            self.tables[(root, b"addons")] = addons
            self.tables[(addons, b"rifle_scope_dict_id")] = addons + 0x40
        if model is not None:
            self.emu.mem_write(STRINGS, model + b"\0")
            self.tables[(data, b"srp_muzzle_model")] = data + 0x40
            self.node(data + 0x40, STRINGS, T_STRING)
        self.number(data, b"srp_muzzle_offset", data + 0x80, offset)
        self.number(data, b"srp_muzzle_fire_mm", data + 0xC0, fire)
        return root

    def weapon(self, shells=2, shots=1, anims=3):
        """A made-up weapon at WEAPON with its arrays after +4710h, its part after them."""
        self.emu.mem_write(WEAPON, b"\x5A" * 0x6000)
        shots_at = WEAPON + 0x4710 + 4 * (anims + shells)
        self.u32(WEAPON + 0x820, WEAPON + 0x4710 + 4 * anims)
        self.u32(WEAPON + 0x81C, shots_at)
        self.emu.mem_write(WEAPON + 0x824, bytes([shots, shells]))
        self.u32(WEAPON + 0x1818, anims)
        part = shots_at + 4 * shots
        self.emu.mem_write(part, bytes(EXTENSION))
        return part

    # the cook's last request: ebx the root, [ebp-40h] the requests' vector, [ebp-10h] a node
    def request(self, root):
        ebp = STACK
        sp = ebp - 0x80
        self.emu.mem_write(STACK - 0x2000, bytes(0x3000))
        storage = STACK - 0x1000
        for at, v in ((ebp - 0x40, storage), (ebp - 0x3C, storage + 8), (ebp - 0x38, storage + 32)):
            self.u32(at, v)
        self.u32(storage, 0x1234), self.u32(storage + 4, 0x10)
        self.u32(ebp - 0x10, CONFIG + 0x800)
        self.node(CONFIG + 0x800, 0, 4, 5)
        regs, esp = self.run(0x24ABF9, (0x24AC00,), sp, {UC_X86_REG_EBX: root, UC_X86_REG_EBP: ebp,
                                                         UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EDI: 0x44444444})
        end = self.u32(ebp - 0x3C)
        requests = [struct.unpack("<II", bytes(self.emu.mem_read(at, 8))) for at in range(storage, end, 8)]
        return self.stopped, esp - sp, regs, [(self.text(p) if p == STRINGS else p, c) for p, c in requests]

    # weapon_cook 0x24A780 from its entry: eax the user animations' count; (config value, ?, shells, shots, ?)
    def make(self, anims, shells, shots):
        sp = STACK - 0x100
        self.emu.mem_write(sp - 0x400, bytes(0x800))
        for i, v in enumerate((STOP, CONFIG, 0, shells, shots, 0)):
            self.u32(sp + 4 * i, v)
        self.tables = {(CONFIG, b"parameters"): CONFIG + 0x100}
        regs, esp = self.run(0x24A780, (STOP,), sp, {UC_X86_REG_EAX: anims, UC_X86_REG_EBX: 0x11111111,
                                                     UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EDI: 0x44444444,
                                                     UC_X86_REG_EBP: 0x22222222})
        size = next((c[1] for c in self.calls if c[0] == "allocate"), None)
        weapon = regs[UC_X86_REG_EAX]
        arrays = 4 * (anims + shells + shots)
        tail = bytes(self.emu.mem_read(weapon + 0x4710, arrays + EXTENSION))
        return self.stopped, size, tail

    # the callback's entry: (results, config, weapon) on the stack
    def loaded(self, root, weapon_part, index_model):
        sp = STACK - 0x200
        self.emu.mem_write(RESULTS, bytes(0x8000))
        self.emu.mem_write(MODEL, bytes(0x400))
        self.u32(MODEL + 0xD0, 1)
        if index_model is not None:
            self.u32(RESULTS + 0x50 + 0x2E0 * index_model + 0xDC, MODEL)
        resource = CONFIG + 0x3000
        self.u32(resource + 0x108, root)
        for i, v in enumerate((STOP, RESULTS, resource, WEAPON)):
            self.u32(sp + 4 * i, v)
        regs, esp = self.run(0x24ACF0, (0x24ACF5,), sp, {UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EBX: 0x11111111,
                                                         UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
                                                         UC_X86_REG_EDI: 0x44444444})
        part = bytes(self.emu.mem_read(weapon_part, EXTENSION))
        return self.stopped, esp - sp, regs, struct.unpack("<IfIf", part[:16]), self.u32(MODEL + 0xD0)

    # a weapon model's call at `site` (args pushed): `who` the register of the weapon
    def model_call(self, site, pops, who, barrel):
        sp = STACK - 0x300
        args = (0xC4A11E10, STACK - 0x280, WEAPON + 0x9BC, WEAPON + 0x9BC)[:pops // 4]
        for i, v in enumerate(args):
            self.u32(sp + 4 * i, v)
        self.emu.mem_write(WEAPON + 0x620, struct.pack("<16f", *barrel))
        self.u32(sp - 4, 0)
        regs, esp = self.run(site, (site + 5,), sp, {UC_X86_REG_EAX: 0x5E5E0108, who: WEAPON,
                                                     UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
                                                     UC_X86_REG_EDI if who != UC_X86_REG_EDI else UC_X86_REG_ESI:
                                                         0x44444444,
                                                     UC_X86_REG_EBP: 0x22222222})
        return self.stopped, esp - sp, list(self.calls), floats(bytes(self.emu.mem_read(WEAPON + 0x620, 64)))

    def release(self, count):
        self.u32(MODEL + 0xD0, count)
        sp = STACK - 0x300
        regs, esp = self.run(0x24B23A, (0x24B240,), sp, {UC_X86_REG_EDI: WEAPON, UC_X86_REG_EBX: 0x11111111,
                                                         UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EBP: 0x22222222})
        return self.stopped, esp - sp, regs, list(self.calls), self.u32(MODEL + 0xD0)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None, late_join)
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    MODEL_PATH = b"weapons/attach/fire_arrestor/fire_arrestor_04"
    print("the cook's last request (0x24ABF9 to 0x24AC00)")
    for model in (MODEL_PATH, None):
        a = stock.request(stock.config(model))
        b = patched.request(patched.config(model))
        want = [(0x1234, 0x10)] + ([(MODEL_PATH, 0x0F)] if model else [])
        same = a[0] == b[0] == 0x24AC00 and a[1] == b[1] == 0 and a[2][UC_X86_REG_ECX] == b[2][UC_X86_REG_ECX] == 5 \
            and all(a[2][r] == b[2][r] for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))
        report(same and a[3] == [(0x1234, 0x10)] and b[3] == want,
               f"{'a config naming a model' if model else 'a config naming none'}: the requests "
               f"{['%s/%x' % r for r in b[3]]}, ecx 5 and the registers the original's")
    report(stock.emu.mem_read(0x24AA04, 4).hex() == "8d440101" and patched.emu.mem_read(0x24AA04, 4).hex() == "8d440102",
           "the requests' alloca: `lea eax,[ecx+eax+2]` (the original +1): one more place")

    print("weapon_cook 0x24A780 (the weapon made)")
    for anims, shells, shots in ((3, 2, 1), (0, 0, 0), (5, 1, 4)):
        a, b = stock.make(anims, shells, shots), patched.make(anims, shells, shots)
        n = 4 * (anims + shells + shots)
        report(a[0] == b[0] == STOP and a[1] == 0x4710 + n and b[1] == 0x4710 + n + EXTENSION
               and b[2][:n] == a[2][:n] == bytes(n) and b[2][n:] == bytes(EXTENSION) and a[2][n:] != bytes(EXTENSION),
               f"{anims} animations, {shells} shells, {shots} shots: {b[1]:#x} bytes asked (the original "
               f"{a[1]:#x}), the arrays as the original's, the part after them zeroes")

    print("the query's callback entry (0x24ACF0 to 0x24ACF5)")
    cases = [("the model named, a scope, two animations, shells 2, shots 1", dict(model=MODEL_PATH, scope=True), 1 + 1 + 2 + 2 + 1, 0.0, 0.0),
             ("the model named, no scope, the offset 0.04 (a float)", dict(model=MODEL_PATH, offset=0.04), 1 + 2 + 2 + 1, 0.04, 0.0),
             ("the offset an int (0), the fire point 42 mm (an int)", dict(model=MODEL_PATH, offset=0, fire=42), 6, 0.0, 0.042),
             ("the fire point 48.5 mm (a float)", dict(model=MODEL_PATH, fire=48.5), 6, 0.0, 0.0485),
             ("no model named", dict(fire=42), None, 0.0, 0.0)]
    for name, kw, index, offset, fire in cases:
        stock.weapon(), patched.weapon()
        part = patched.weapon()
        stock.weapon()
        a = stock.loaded(stock.config(**kw), part, index if index is not None else 6)
        b = patched.loaded(patched.config(**kw), part, index if index is not None else 6)
        want = (MODEL if index is not None else 0, offset, 0)
        same = a[0] == b[0] == 0x24ACF5 and a[1] == b[1] and all(a[2][r] == b[2][r] for r in REGS)
        ok = same and b[3][0] == want[0] and math.isclose(b[3][1], want[1], abs_tol=1e-6) and b[3][2] == 0 \
            and math.isclose(b[3][3], fire, abs_tol=1e-6) and b[4] == (2 if index is not None else 1) and a[4] == 1
        report(ok, f"{name}: {'result ' + str(index) + ' kept, its count 2' if index is not None else 'nothing kept'}"
                   f", the offset {b[3][1]:.2f}, the fire point {b[3][3]:.3f}; the registers and stack the original's")

    print("the weapon model's calls (add, remove, update)")
    barrel = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0.1, 0.2, 0.3, 1]
    turned = [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 1.0, 2.0, 3.0, 1]
    sites = ((0x243F37, "add", 16, UC_X86_REG_ESI), (0x244CF5, "add", 16, UC_X86_REG_ESI),
             (0x244F3A, "update", 16, UC_X86_REG_EBX), (0x244012, "remove", 8, UC_X86_REG_ESI),
             (0x244C08, "remove", 8, UC_X86_REG_ESI))
    for device in (True, False):
        tag = "with a device (offset 0.05, fire point 0.04)" if device else "without a device"
        for matrix in (barrel, turned):
            expected, flash = list(matrix), list(matrix)
            for i in range(3):
                expected[12 + i] -= 0.05 * matrix[8 + i]
                flash[12 + i] -= (0.05 + 0.04) * matrix[8 + i]
            for site, kind, pops, who in sites:
                for added in (0, 1):
                    parts = []
                    for run in (stock, patched):
                        part = run.weapon()
                        run.emu.mem_write(part, struct.pack("<IfIf", MODEL if device else 0, 0.05, added, 0.04))
                        parts.append(part)
                    a = stock.model_call(site, pops, who, matrix)
                    b = patched.model_call(site, pops, who, matrix)
                    part = parts[1]
                    first = b[2][:1] == a[2] and len(a[2]) == 1 and a[0] == b[0] == site + 5 and a[1] == b[1] == pops
                    acts = device and (added == 0 if kind == "add" else added == 1)
                    state = patched.u32(part + 8)
                    kept = all(math.isclose(x, y, abs_tol=1e-6) for x, y in zip(b[3], matrix)) and a[3] == b[3] \
                        if not acts or kind != "update" else \
                        all(math.isclose(x, y, abs_tol=1e-6) for x, y in zip(b[3], flash))
                    if not acts:
                        report(first and b[2] == a[2] and state == added and kept,
                               f"{site:#x} ({kind}) {tag}, the device {'in' if added else 'out of'} the scene: the "
                               f"original's call alone")
                        continue
                    more = b[2][1:]
                    part_m = struct.unpack("<16f", bytes(patched.emu.mem_read(part + 0x10, 64)))
                    if kind == "remove":
                        good = more == [("remove", MODEL + 0x108, 0xC4A11E10, STACK - 0x280)] and state == 0
                    else:
                        good = len(more) == 1 and more[0][:4] == (kind, MODEL + 0x108, 0xC4A11E10, STACK - 0x280) \
                            and more[0][4] == more[0][5] == part + 0x10 and state == 1 \
                            and all(math.isclose(x, y, abs_tol=1e-6) for x, y in zip(part_m, expected))
                    report(first and good and kept,
                           f"{site:#x} ({kind}) {tag}, the barrel {'turned' if matrix is turned else 'plain'}: the "
                           f"original's call, then the device's"
                           + ("" if kind == "remove" else " (barrel_point's matrix, its point 0.05 forward)")
                           + ("; barrel_point then 0.09 forward (the flash)" if kind == "update" else
                              "; barrel_point as it was"))

    print("delete_resource (0x24B23A to 0x24B240)")
    for count, gone in ((2, False), (1, True)):
        for run in (stock, patched):
            part = run.weapon()
            run.emu.mem_write(part, struct.pack("<IfI", MODEL, 0.0, 0))
        a, b = stock.release(count), patched.release(count)
        part_ptr = patched.u32(part)
        destroyed = [c for c in b[3] if c[0] == "destroy"]
        report(a[0] == b[0] == 0x24B240 and a[1] == b[1] and a[2][UC_X86_REG_EBX] == b[2][UC_X86_REG_EBX] == 3
               and b[4] == count - 1 and part_ptr == 0 and a[4] == count and bool(destroyed) == gone,
               f"the model's count {count}: released (now {b[4]}{', the game destroys it' if gone else ''}), the "
               f"part's pointer zero, ebx the animations' count")

    print(f"muzzle device: {'all passed' if not failed else f'{failed} FAILED'}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
