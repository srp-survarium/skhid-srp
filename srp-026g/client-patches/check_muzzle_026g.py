#!/usr/bin/env python3
"""Run patch 90 (the Rust patch muzzle, rust/patches/src/muzzle.rs and
muzzle.toml over vostok::muzzle: a weapon's muzzle device, a static model at
the weapon model's muzzle_point; ../docs/notes/weapon-kit.md, "The muzzle
device") in the default build and the original.

    python check_muzzle_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4) and `capstone` (the alignment probe,
align_probe_026g.py, built in). The exe's sections are loaded as they are,
the arena with them; the game's own code runs between the sites, on
made-up objects:

- the cook (on_weapon_config_loaded 0x24A920, from its entry to its call of
  weapon_cook 0x24A780 at 0x24AC2B), the configs' lookups (0xF6B10,
  0xF6C00) stubs over made-up nodes, the game's alloca (0x424CA0) and push
  (0x5FCB0): a config naming a device (the kit's, or the gun's own) gets
  its model requested last, class 15 (one request more, inside the
  alloca); one naming none, or half a pair, the original's requests;
- the callback after load_weapon (muzzle.loaded, 0x24B0E1 to its span's
  end), the game's render_model::get_locator 0x1585B0 over a made-up
  skeleton model: the device's result retained once (+0D0h) when the
  weapon model has muzzle_point; nothing without it, nor without the
  request;
- weapon::draw's muzzle.locate (0x244E99, the game's calculate_locator
  0x243BC0 and its matrix code): +620h the device's frame moved to its fire
  point, the frame and +620h as this file's model of calculate_locator
  (R_y(pi), the locator's matrix, the bone's, the world's) gives, which the
  original's own calculate_locator gives too; a weapon without a device:
  +620h untouched;
- muzzle.draw (0x245017): the device added at the frame, then updated,
  while the weapon's model is in the scene; removed when it is not;
  muzzle.culled (0x245353), muzzle.hidden (0x244017): removed; nothing for
  a weapon without one, nor in the original; the renderer's add, update and
  remove (0x21CD0, 0x22280, 0x21F40) stubs that keep the in-scene byte;
- muzzle.release (0x24B190): the device out of the scene and its
  reference given back (0xE0210 at 0), once;
- artefact carry (check_artefact_carry_026g.py's fixture): a carrier's gun
  with a device: carry takes the device out with the gun (its fourth
  model) and puts it back with it.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import align_probe_026g as align
import patch_026g as base

COOK, COOK_WEAPON = 0x24A920, 0x24AC2B             # on_weapon_config_loaded; its call of weapon_cook
LOADED, LOADED_END = 0x24B0E1, 0x24B0E9
LOCATE, LOCATE_END = 0x244E99, 0x244E9F
DRAW, DRAW_END = 0x245017, 0x24501C
CULLED, CULLED_END = 0x245353, 0x245359
HIDDEN, HIDDEN_END = 0x244017, 0x24401D
RELEASE, RELEASE_END = 0x24B190, 0x24B197
CALCULATE = 0x243BC0
CHILD, HAS, RELEASE_PTR, DESTROY = 0x0F6B10, 0x0F6C00, 0x011540, 0x0E0210
ADD, REMOVE, UPDATE = 0x21CD0, 0x21F40, 0x22280
SKELETON_INSTANCE = 0x9F0F24
T_INT, T_TABLE, T_STRING = 1, 3, 5

STACK, STOP, HEAP = 0x21000000, 0x21100000, 0x30000000
RENDERER = 0x31000000
REGS = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "ebx": UC_X86_REG_EBX,
        "esi": UC_X86_REG_ESI, "edi": UC_X86_REG_EDI, "ebp": UC_X86_REG_EBP}


# ------------------------------------------------------------ the model

def multiply(a, b):
    """multiply_affine 0xEE700: a then b, row vectors, the w column 0, 0, 0, 1."""
    out = [0.0] * 16
    for r in range(4):
        for c in range(3):
            out[4 * r + c] = sum(a[4 * r + k] * b[4 * k + c] for k in range(3)) + (b[12 + c] if r == 3 else 0.0)
    out[15] = 1.0
    return out


def rotation_y(angle):
    c, s = math.cos(angle), math.sin(angle)
    return [c, 0, s, 0, 0, 1, 0, 0, -s, 0, c, 0, 0, 0, 0, 1]


def translate(x, y, z):
    return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]


IDENTITY = translate(0, 0, 0)
PI = struct.unpack("<f", struct.pack("<f", math.pi))[0]


def frame_of(locator, bone, world):
    """calculate_locator: the extra (identity), R_y(pi), the locator's matrix,
    the bone's, the world's."""
    return multiply(multiply(multiply(multiply(IDENTITY, rotation_y(PI)), locator), bone), world)


def flash_of(frame, fire_mm):
    out = list(frame)
    for k in range(3):
        out[12 + k] += fire_mm / 1000.0 * frame[8 + k]
    return out


def close(a, b, eps=1e-5):
    return a is not None and b is not None and len(a) == len(b) and all(abs(x - y) <= eps for x, y in zip(a, b))


# ------------------------------------------------------------ the runs

class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(STACK - 0x40000, 0x80000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x1000, UC_PROT_ALL)
        emu.mem_map(HEAP, 0x400000, UC_PROT_ALL)
        emu.mem_map(RENDERER, 0x201000, UC_PROT_ALL)
        self.next = HEAP
        self.children = {}
        self.rendered, self.destroyed = [], []
        for at in (CHILD, HAS, RELEASE_PTR, DESTROY, ADD, REMOVE, UPDATE):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, self.stop)
        self.ends = ()
        # kernel32's GetCurrentThreadId (the Lock's owner): 1; the log's
        # CreateFileA, WriteFile (its lines kept) and CloseHandle
        stub = STOP + 0x800
        self.u32(0x870118, stub)
        emu.mem_write(stub, bytes.fromhex("b801000000c3"))
        self.logged = []
        for slot, at, code in ((0x870048, STOP + 0x820, "b855000000c21c00"), (0x870108, STOP + 0x840, None),
                               (0x870040, STOP + 0x860, "b801000000c20400")):
            self.u32(slot, at)
            if code:
                emu.mem_write(at, bytes.fromhex(code))
        self.write_file = STOP + 0x840
        emu.hook_add(UC_HOOK_CODE, self.log, begin=self.write_file, end=self.write_file)

    def block(self, size):
        at = self.next
        self.next += (size + 0xF) & ~0xF
        self.emu.mem_write(at, bytes(size))
        return at

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def floats(self, at, n=16):
        return list(struct.unpack(f"<{n}f", self.emu.mem_read(at, 4 * n)))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 128)).split(b"\0")[0]

    def string(self, text):
        at = self.block(len(text) + 1)
        self.emu.mem_write(at, text.encode() + b"\0")
        return at

    def node(self, kind=T_INT, data=0, count=0):
        """A config node (18h bytes: the value, the type at +14h, the count +16h)."""
        at = self.block(0x18)
        self.u32(at, data)
        self.emu.mem_write(at + 0x14, struct.pack("<HH", kind, count))
        return at

    def table(self, entries):
        """A table node whose children the lookup stubs give by name."""
        at = self.node(T_TABLE, self.block(0x18 * max(1, len(entries))), len(entries))
        for name, child in entries.items():
            self.children[(at, name.encode())] = child
        return at

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        pops, value = 0, 0
        if address in (CHILD, HAS):
            table = emu.reg_read(UC_X86_REG_EAX)
            key = self.text(emu.reg_read(UC_X86_REG_EDI if address == CHILD else UC_X86_REG_ESI))
            child = self.children.get((table, key))
            if address == CHILD:
                if child is None:
                    raise KeyError(f"child {table:#x} {key!r}")
                value = child
            else:
                value = 1 if child is not None else 0
        elif address == RELEASE_PTR:
            pass
        elif address == DESTROY:
            self.destroyed.append(emu.reg_read(UC_X86_REG_ECX))
        else:
            reference = emu.reg_read(UC_X86_REG_EAX)
            instance = self.u32(reference)
            op = {ADD: "add", REMOVE: "remove", UPDATE: "update"}[address]
            before = emu.mem_read(instance + 0x10D, 1)[0]
            assert (op != "add" or before == 0) and (op != "remove" or before == 1), (op, before)
            emu.mem_write(instance + 0x10D, bytes([0 if op == "remove" else 1]))
            matrix = None if op == "remove" else self.floats(self.u32(sp + 12))
            self.rendered.append((op, instance, matrix))
            pops = 8 if op == "remove" else 16
        emu.reg_write(UC_X86_REG_EAX, value)
        emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def log(self, emu, address, size, _):
        """WriteFile(handle, bytes, size, &written, 0): stdcall, ret 14h."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        data, size_ = self.u32(sp + 8), self.u32(sp + 12)
        self.logged.append(bytes(emu.mem_read(data, size_)).decode(errors="replace"))
        if self.u32(sp + 16):
            self.u32(self.u32(sp + 16), size_)
        emu.reg_write(UC_X86_REG_EAX, 1)
        emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 20)

    def stop(self, emu, address, size, _):
        if address in self.ends:
            emu.emu_stop()

    def run(self, start, ends, sp, **regs):
        for name, value in regs.items():
            self.emu.reg_write(REGS[name], value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.ends = tuple(ends)
        self.emu.emu_start(start, STOP, count=2_000_000)
        self.ends = ()
        return self.emu.reg_read(UC_X86_REG_EIP)

    # -------------------------------------------------- objects

    def config(self, data=None):
        """A weapon config's resource: one shell, two shot effects, one
        preview animation, no scope; `data` its data table's ints and
        strings (None: no data table)."""
        resource = self.block(0x140)
        shell, shoot = self.string("shells/5_45x040"), self.string("fx/weapons/ak_74")
        particles = self.table({"bullet_shells_count": self.node(T_INT, 1), "shoot_pfx_count": self.node(T_INT, 2),
                                "bullet_shells": self.node(T_STRING, shell), "shoot": self.node(T_STRING, shoot)})
        animation = self.node(T_STRING, self.string("weapons/ak_74/preview"))
        preview = self.node(4, animation, 1)
        entries = {"particles": particles, "object": self.table({"model": self.node(T_STRING,
                                                                                   self.string("weapons/ak74m/ak74m"))}),
                   "user_animations_in_place": self.table({"preview": preview})}
        if data is not None:
            entries["data"] = self.table({k: self.node(T_STRING, self.string(v)) if isinstance(v, str)
                                          else self.node(T_INT, v) for k, v in data.items()})
        root = self.table(entries)
        self.u32(resource + 0x108, root)
        self.u32(resource + 0xD0, 1)
        return resource, root

    def cook(self, resource):
        """on_weapon_config_loaded to its weapon_cook call: the requests
        [(path, class)] and whether they fit their alloca."""
        cook = self.block(0x40)
        # its argument (the config's query) holds the config's resource at
        # +12Ch (0x24A92F; retained into [ebp+8]), its parent query at +20h
        query = self.block(0x140)
        self.u32(query + 0x12C, resource)
        sp = STACK - 0x2000
        for i, value in enumerate((STOP, query, 0, 0, 0)):
            self.u32(sp + 4 * i, value)
        stopped = self.run(COOK, (COOK_WEAPON,), sp, ecx=cook, ebx=0x11111111, esi=0x33333333, edi=0x44444444,
                           ebp=0x22222222)
        if stopped != COOK_WEAPON:
            return None
        ebp = self.emu.reg_read(UC_X86_REG_EBP)
        begin, end, capacity = (self.u32(ebp - 0x40 + 4 * i) for i in range(3))
        requests = [(self.text(self.u32(at)).decode(), self.u32(at + 4)) for at in range(begin, end, 8)]
        return requests, end <= capacity, capacity - begin

    def weapon(self, locators):
        """A client weapon whose model is a skeleton model instance with
        `locators` [(name, matrix, bone)]; in the scene (its model's byte),
        its world and renderer."""
        weapon, holder, instance, model = self.block(0x4800), self.block(0x120), self.block(0x6300), self.block(0x140)
        self.u32(weapon + 0x1810, holder)
        self.u32(holder + 0x108, instance)
        self.u32(instance, SKELETON_INSTANCE)
        self.u32(instance + 0x62B4, model)
        array = self.block(0x64 * max(1, len(locators)))
        for i, (name, matrix, bone) in enumerate(locators):
            at = array + 0x64 * i
            self.emu.mem_write(at, name.encode() + b"\0")
            self.emu.mem_write(at + 0x20, struct.pack("<16f", *matrix))
            self.emu.mem_write(at + 0x60, struct.pack("<H", bone))
        self.u32(model + 0x120, array)
        self.emu.mem_write(model + 0x124, struct.pack("<H", len(locators)))
        world, game, scene = self.block(0x100), self.block(0x5400), self.block(0x100)
        self.u32(world + 0x9C, game)
        self.u32(game + 0x5360, RENDERER)
        self.u32(RENDERER + 0x200064, scene)
        self.u32(world + 4, scene)
        self.u32(weapon + 0x180C, world)
        self.emu.mem_write(instance + 0x10D, b"\1")
        self.emu.mem_write(weapon + 0x620, struct.pack("<16f", *translate(7, 7, 7)))
        return weapon, instance

    def device(self):
        """A device model holder (a static model's): its instance, count 1."""
        holder, instance = self.block(0x120), self.block(0x300)
        self.u32(holder + 0x108, instance)
        self.u32(holder + 0xD0, 1)
        return holder, instance

    def loaded(self, weapon, resource, results):
        """muzzle.loaded, the callback's frame made: [esp+38h] the results
        (one a resource), [esp+3Ch] the config, [esp+40h] the weapon."""
        queries = self.block(0x50 + 0x2E0 * max(1, len(results)))
        self.u32(queries + 0x38, len(results))
        for i, resource_ in enumerate(results):
            self.u32(queries + 0x50 + 0x2E0 * i + 0xDC, resource_)
        sp = STACK - 0x3000
        self.emu.mem_write(sp, bytes(0x60))
        self.u32(sp + 0x38, queries)
        self.u32(sp + 0x3C, resource)
        self.u32(sp + 0x40, weapon)
        return self.run(LOADED, (LOADED_END,), sp, edi=weapon)

    def locate(self, weapon, world, bones):
        frame = STACK - 0x4000
        w, b = self.block(64), self.block(64 * len(bones))
        self.emu.mem_write(w, struct.pack("<16f", *world))
        for i, bone in enumerate(bones):
            self.emu.mem_write(b + 64 * i, struct.pack("<16f", *bone))
        self.u32(frame + 8, w)
        self.u32(frame + 0xC, b)
        return self.run(LOCATE, (LOCATE_END,), STACK - 0x5000, ebx=weapon, ebp=frame)

    def calculate(self, locator, world, bones):
        """The game's own calculate_locator 0x243BC0 (EDI the out, ret 10h)."""
        loc, w, b, extra, out = self.block(0x64), self.block(64), self.block(64 * len(bones)), self.block(64), \
            self.block(64)
        name, matrix, bone = locator
        self.emu.mem_write(loc + 0x20, struct.pack("<16f", *matrix))
        self.emu.mem_write(loc + 0x60, struct.pack("<H", bone))
        self.emu.mem_write(w, struct.pack("<16f", *world))
        for i, m in enumerate(bones):
            self.emu.mem_write(b + 64 * i, struct.pack("<16f", *m))
        self.emu.mem_write(extra, struct.pack("<16f", *IDENTITY))
        sp = STACK - 0x6000
        for i, value in enumerate((STOP, loc, w, b, extra)):
            self.u32(sp + 4 * i, value)
        self.run(CALCULATE, (STOP,), sp, edi=out)
        return self.floats(out)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    align.install()                 # every Uc from here: aligned-SSE operands checked
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None, late_join)
    built = bytes(built)
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    KIT = {"srp_muzzle_model": "weapons/v069d/muzzle/zenit_dtk3", "srp_muzzle_fire_mm": 67}
    STOCK = {"srp_muzzle_stock_model": "weapons/ak74m/ak74m_muzzle", "srp_muzzle_stock_fire_mm": 81}
    NATIVE = [("shells/5_45x040", 0x3D), ("fx/weapons/ak_74", 0x3D), ("fx/weapons/ak_74", 0x3D)]

    print("the cook's requests (on_weapon_config_loaded 0x24A920 to its weapon_cook call, 0x24AC2B)")
    cases = [
        ("no data table", None, None),
        ("a data table naming no device", {"camo_scale": 4}, None),
        ("the kit's device", KIT, KIT["srp_muzzle_model"]),
        ("the gun's own device", STOCK, STOCK["srp_muzzle_stock_model"]),
        ("the kit's beside the gun's own: the kit's", {**STOCK, **KIT}, KIT["srp_muzzle_model"]),
        ("half a pair (no fire point): none", {"srp_muzzle_model": "weapons/x"}, None),
        ("an empty model path: none", {"srp_muzzle_model": "", "srp_muzzle_fire_mm": 5}, None),
    ]
    for name, data, want in cases:
        a, b = Run(original), Run(built)
        got_a, got_b = a.cook(a.config(data)[0]), b.cook(b.config(data)[0])
        ok = got_a is not None and got_b is not None and got_a[1] and got_b[1]
        if ok:
            stock_requests = got_a[0]
            ok = [r for r in stock_requests if r[1] == 0x3D] == NATIVE and len(stock_requests) == 5
            if want is None:
                ok = ok and got_b[0] == stock_requests and got_b[2] == got_a[2]
            else:
                ok = ok and got_b[0] == stock_requests + [(want, 15)] and got_b[2] == got_a[2] + 8
        report(ok, f"{name}: {'the original requests' if want is None else f'one more, last: {want!r}, class 15'}"
                   f" (the original's: {[r[1] for r in got_a[0]] if got_a else '?'}), inside the alloca")

    print("the device kept with the weapon (muzzle.loaded, 0x24B0E1)")
    muzzle_at = translate(0.0, 0.042, -0.586)
    LOCATORS = [("barrel_point", translate(0.0, 0.042, -0.636), 1), ("muzzle_point", muzzle_at, 1)]
    bones = [translate(0.1, 0.2, 0.3), multiply(rotation_y(0.3), translate(0.0, 1.3, 0.05))]
    world = multiply(rotation_y(1.1), translate(10.0, 2.0, -4.0))

    def fitted(run, data=KIT, locators=LOCATORS, request=True, extra=0):
        resource, root = run.config(data)
        if request:
            run.cook(resource)
        weapon, instance = run.weapon(locators)
        holder, device = run.device()
        others = [run.block(0x120) for _ in range(5)]
        stopped = run.loaded(weapon, resource, others + [holder] + [run.block(0x120)] * extra)
        return stopped, weapon, instance, holder, device

    a, b = Run(original), Run(built)
    sa, *_ , ha, _ = fitted(a)
    sb, wb, ib, hb, db = fitted(b)
    report(sa == sb == LOADED_END and a.u32(ha + 0xD0) == 1 and b.u32(hb + 0xD0) == 2,
           "the kit's device: its result (the sixth, after the original's five) retained once (+0D0h 1 -> 2); "
           "the original: untouched")
    for name, kwargs in (("a weapon model without muzzle_point", {"locators": LOCATORS[:1]}),
                         ("no request made (a config the cook did not ask for)", {"request": False}),
                         ("a result count not the request's", {"extra": 1}),
                         ("a config naming no device", {"data": {"camo_scale": 4}})):
        r = Run(built)
        s, w, _, h, _ = fitted(r, **kwargs)
        said = "muzzle_point" in "".join(r.logged)
        report(s == LOADED_END and r.u32(h + 0xD0) == 1 and said == ("locators" in kwargs),
               f"{name}: nothing retained" + (" (logged once)" if "locators" in kwargs else ""))

    print("the frame and the flash (weapon::draw's muzzle.locate, 0x244E99)")
    want_frame = frame_of(muzzle_at, bones[1], world)
    native = Run(original).calculate(LOCATORS[1], world, bones)
    report(close(native, want_frame),
           "the original's calculate_locator gives this file's model: R_y(pi), the locator's, the bone's, the world's")
    stopped = b.locate(wb, world, bones)
    barrel = b.floats(wb + 0x620)
    report(stopped == LOCATE_END and close(barrel, flash_of(want_frame, KIT["srp_muzzle_fire_mm"])),
           "+620h: the device's frame moved 67 mm along its third row (the bore), its orientation the frame's")
    a2 = Run(original)
    wa, _ = a2.weapon(LOCATORS)
    r = Run(built)
    wn, _ = r.weapon(LOCATORS)
    a2.locate(wa, world, bones), r.locate(wn, world, bones)
    report(a2.floats(wa + 0x620) == r.floats(wn + 0x620) == translate(7, 7, 7),
           "a weapon without a device (and the original): +620h untouched")

    print("the scene (muzzle.draw 0x245017, muzzle.culled 0x245353, muzzle.hidden 0x244017)")
    b.rendered.clear()
    b.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=wb)
    first = list(b.rendered)
    b.rendered.clear()
    b.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=wb)
    second = list(b.rendered)
    report(len(first) == 1 and first[0][0] == "add" and first[0][1] == db and close(first[0][2], want_frame)
           and len(second) == 1 and second[0][0] == "update" and second[0][1] == db,
           "the weapon's model in the scene: the device added at its frame, then updated")
    b.emu.mem_write(ib + 0x10D, b"\0")
    b.rendered.clear()
    b.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=wb)
    report(b.rendered == [("remove", db, None)], "the weapon's model out (the aimed scope's): the device removed")
    b.rendered.clear()
    b.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=wb)
    report(b.rendered == [], "out again: nothing (removed once)")
    for name, start, end, reg in (("culled", CULLED, CULLED_END, "ebx"), ("hidden", HIDDEN, HIDDEN_END, "esi")):
        b.emu.mem_write(ib + 0x10D, b"\1")
        b.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=wb)
        b.rendered.clear()
        stopped = b.run(start, (end,), STACK - 0x5000, **{reg: wb})
        report(stopped == end and b.rendered == [("remove", db, None)], f"{name}: the device removed")
    for name, start, end, reg in (("draw", DRAW, DRAW_END, "ebx"), ("culled", CULLED, CULLED_END, "ebx"),
                                  ("hidden", HIDDEN, HIDDEN_END, "esi")):
        a2.rendered.clear(), r.rendered.clear()
        a2.run(start, (end,), STACK - 0x5000, **{reg: wa}), r.run(start, (end,), STACK - 0x5000, **{reg: wn})
        report(a2.rendered == r.rendered == [], f"{name}: a weapon without a device, and the original: nothing")

    print("the weapon goes (muzzle.release, weapon_cook::delete_resource 0x24B190)")
    b.emu.mem_write(ib + 0x10D, b"\1")
    b.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=wb)
    b.rendered.clear(), b.destroyed.clear()
    sp = STACK - 0x5000
    b.u32(sp + 4, wb + 0x10)
    stopped = b.run(RELEASE, (RELEASE_END,), sp)
    report(stopped == RELEASE_END and b.rendered == [("remove", db, None)] and b.u32(hb + 0xD0) == 1
           and b.destroyed == [], "the device out of the scene, its reference given back (2 -> 1)")
    b.rendered.clear()
    b.run(RELEASE, (RELEASE_END,), sp)
    b.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=wb)
    report(b.rendered == [] and b.u32(hb + 0xD0) == 1, "again, and a draw after: nothing (given back once)")
    r = Run(built)
    _, w, _, h, _ = fitted(r)
    r.u32(h + 0xD0, 1)                  # ours the last reference: the instance destroyed at 0
    r.u32(sp + 4, w + 0x10)
    r.run(RELEASE, (RELEASE_END,), sp)
    report(r.destroyed == [h] and r.rendered == [], "the last reference ours: the model destroyed (0xE0210), "
                                                    "never drawn: no scene call")
    r.u32(sp + 4, 0)
    report(r.run(RELEASE, (RELEASE_END,), sp) == RELEASE_END, "delete_resource of nothing (0): nothing")

    print("artefact carry: the device is the gun's fourth model")
    import check_artefact_carry_026g as carry
    g = carry.Run(built)
    p = g.players[0]
    _, gun_instance = g.renderer()
    gun = g.read(p + 0x128)
    resource = g.block(0x140)
    root = g.block(0x18)
    g.word(resource + 0x108, root)
    data = g.block(0x18)
    model_key, fire_key = g.block(0x18), g.block(0x18)
    names = {}

    def node(at, kind, value, count=0):
        g.word(at, value)
        g.emu.mem_write(at + 0x14, struct.pack("<HH", kind, count))

    path = g.block(32)
    g.emu.mem_write(path, b"weapons/x\0")
    node(root, T_TABLE, g.block(0x18), 1)
    node(data, T_TABLE, g.block(0x18), 2)
    node(model_key, T_STRING, path)
    node(fire_key, T_INT, 50)
    names.update({(root, b"data"): data, (data, b"srp_muzzle_model"): model_key,
                  (data, b"srp_muzzle_fire_mm"): fire_key})

    def lookup(emu, address, *_):
        sp_ = emu.reg_read(UC_X86_REG_ESP)
        table = emu.reg_read(UC_X86_REG_EAX)
        key = bytes(emu.mem_read(emu.reg_read(UC_X86_REG_EDI if address == CHILD else UC_X86_REG_ESI), 64)).split(b"\0")[0]
        hit = names.get((table, key))
        emu.reg_write(UC_X86_REG_EAX, (hit or 0) if address == CHILD else int(hit is not None))
        emu.reg_write(UC_X86_REG_EIP, g.read(sp_))
        emu.reg_write(UC_X86_REG_ESP, sp_ + 4)
    for at in (CHILD, HAS):
        g.emu.hook_add(UC_HOOK_CODE, lookup, begin=at, end=at)
    # the gun's model a skeleton model with muzzle_point, the request pending
    gun_holder = g.read(gun + 0x1810)
    skeleton = g.block(0x6300)        # a skeleton model's instance in the gun's holder, in the scene
    g.emu.mem_write(skeleton + 0x10D, b"\1")
    g.word(gun_holder + 0x108, skeleton)
    gun_instance = skeleton
    g.word(gun_instance, SKELETON_INSTANCE)
    gun_model = g.block(0x140)
    g.word(gun_instance + 0x62B4, gun_model)
    locators = g.block(0x64)
    g.emu.mem_write(locators, b"muzzle_point\0")
    g.emu.mem_write(locators + 0x20, struct.pack("<16f", *translate(0, 0, 0.5)))
    g.emu.mem_write(locators + 0x60, struct.pack("<H", 0xFFFF))
    g.word(gun_model + 0x120, locators)
    g.emu.mem_write(gun_model + 0x124, struct.pack("<H", 1))
    # the cook's request: muzzle.request on a made-up frame whose vector has room
    frame = g.block(0x80) + 0x40
    requests = g.block(8 * 3)
    g.word(frame - 0x40, requests)
    g.word(frame - 0x3C, requests + 16)
    g.word(frame - 0x38, requests + 24)
    g.word(frame - 0x10, g.block(0x18))   # the span's own read: the preview animations' node
    g.site(0x24ABF9, [0x24AC00], ebx=root, ebp=frame)
    device_holder, device_instance = g.block(0x120), g.block(0x300)
    g.word(device_holder + 0x108, device_instance)
    g.word(device_holder + 0xD0, 1)
    queries = g.block(0x50 + 0x2E0 * 3)
    g.word(queries + 0x38, 3)
    g.word(queries + 0x50 + 0x2E0 * 2 + 0xDC, device_holder)
    sp = carry.STACK - 0x800
    g.word(sp + 0x38, queries)
    g.word(sp + 0x3C, resource)
    g.emu.reg_write(UC_X86_REG_EDI, gun)
    g.emu.reg_write(UC_X86_REG_ESP, sp)
    g.ends = (LOADED_END,)
    g.emu.emu_start(LOADED, carry.STOP, count=200_000)
    g.ends = ()
    report(g.read(device_holder + 0xD0) == 2, "the carrier's gun has a device (retained)")
    g.emu.mem_write(device_instance + 0x10D, b"\1")         # in the scene with the gun
    g.input(p, 0, carry.COMMAND)
    g.tick(p, 800)
    g.rendered.clear()
    g.draw()
    out = g.emu.mem_read(device_instance + 0x10D, 1) == b"\0" and g.emu.mem_read(gun_instance + 0x10D, 1) == b"\0"
    report(out and ("remove", device_instance) in [(op, m) for op, m, _ in g.rendered],
           "carrying: the device out of the scene with the gun")
    g.rendered.clear()
    g.site(0x243FA0, [carry.STOP], (1,), ecx=gun)
    report(("add", device_instance) in [(op, m) for op, m, _ in g.rendered],
           "on_hide: carry puts the device back with the gun before the native removal")

    print("the alignment probe (align_probe_026g.py)")
    align.report(report)
    print(f"muzzle: {'all passed' if not failed else f'{failed} FAILED'}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
