#!/usr/bin/env python3
"""Exercise breakable lamps through build 2777's native project/history loops.

python check_lamps_026g.py <original survarium.exe>
python check_lamps_026g.py <original survarium.exe> --patched <local test exe>
Renderer/effect queue endpoints, the writer's byte sink and GetCurrentThreadId are stubbed. Config
lookup, project iteration, vtable dispatch, visual/light methods, empty-bullet
serialization/deserialization and all installed lamp hooks execute x86 code.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import struct

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
    UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP,
    UC_X86_REG_EIP, UC_X86_REG_EFLAGS, UC_X86_REG_XMM0)
import patch_026g as base

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('content_edit', ROOT / 'scripts/content-edit/content_edit.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
HEAP, STACK, RET = 0x40000000, 0x408ff000, 0x40900000
PROJECT, CONFIG, SCENE, GAME, RENDERER = (HEAP + i * 0x1000 for i in range(5))
VISUALS, OBJECTS, LIGHTS, MODELS = HEAP + 0x300000, HEAP + 0x400000, HEAP + 0x410000, HEAP + 0x500000
CFG, MANAGER, BULLET, RESULT, WRITER, READER, BUFFER = (HEAP + i * 0x10000 for i in range(4, 11))
PHYSICS = HEAP + 0xb0000
CAMERA, CORE = HEAP + 0xd0000, HEAP + 0xe0000
MATERIALS, BULLET_MAT, GLASS_MAT, PAIR, PARTICLES = (HEAP + i for i in (0x200000, 0x211000, 0x212000, 0x213000, 0x214000))
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
        UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


def config_tree():
    rows = {}
    for i, x in enumerate((0.0, 3.0)):
        views = {'intact': {'index': i * 2, 'state': 0}, 'broken': {'index': i * 2 + 1, 'state': 1}}
        if i == 0:
            views['cone'] = {'index': 4, 'state': 0}
        rows[str(i)] = dict(min_x=x - .1, max_x=x + .1, min_y=-.1, max_y=.1, min_z=-.1, max_z=.1,
                           visuals=views, lights={'light': dict(x=x, y=0.0, z=0.0)})
    return {'srp_lamps': {'version': 2, 'break_material': 29, 'lamps': rows}}


def relocated_config(value):
    b = bytearray(24)
    b[:24] = c.value_record(b, 0, '', value)
    offsets = []

    def visit(off):
        d, name, crc, t, count = c.rec(b, off)
        offsets.append((off, d, name, crc, t, count))
        if t in (3, 4):
            for kid in c.children(b, off):
                visit(kid)
    visit(0)
    for off, d, name, crc, t, count in offsets:
        c.REC.pack_into(b, off, d + CFG if t >= 3 and d else d,
                       name + CFG if name else 0, crc, t, count)
    return b


class Game:
    def __init__(self, built):
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(built).sections(built):
            addr = base.IMAGE_BASE + rva
            u.mem_map(addr, (max(vsize, rsize) + 0xfff) & ~0xfff)
            u.mem_write(addr, bytes(built[raw:raw + min(rsize, vsize or rsize)]))
        u.mem_map(HEAP, 0xa00000)
        self.active, self.events, self.output = set(), [], bytearray()
        self.effects = []
        u.hook_add(UC_HOOK_CODE, self.endpoint, begin=0x21cd0, end=0x21cd0)
        u.hook_add(UC_HOOK_CODE, self.endpoint, begin=0x21f40, end=0x21f40)
        u.hook_add(UC_HOOK_CODE, self.endpoint, begin=0x24750, end=0x24750)
        u.hook_add(UC_HOOK_CODE, self.endpoint, begin=0x24890, end=0x24890)
        u.hook_add(UC_HOOK_CODE, self.write, begin=0x1f48a0, end=0x1f48a0)
        u.hook_add(UC_HOOK_CODE, self.effect, begin=0x3223e0, end=0x3223e0)
        u.hook_add(UC_HOOK_CODE, self.effect, begin=0x3222f0, end=0x3222f0)
        self.w(PROJECT + 0x17c, CONFIG); self.w(CONFIG + 0x108, CFG)
        self.w(PROJECT + 0x180, SCENE); self.w(SCENE + 0x98, CAMERA)
        self.w(SCENE + 0x9c, GAME); self.w(GAME + 0x5360, RENDERER)
        self.w(RENDERER + 0x200064, HEAP + 0x300000)
        self.w(PROJECT + 0x1e8, VISUALS); self.w(PROJECT + 0x1ec, 5)
        self.w(PROJECT + 0x184, OBJECTS); self.w(PROJECT + 0x188, OBJECTS + 8)
        for i in range(5):
            self.w(VISUALS + i * 0x48, 0xa029b4)
            self.w(VISUALS + i * 0x48 + 0x44, MODELS + i * 0x200)
        for i in range(2):
            light = LIGHTS + i * 0x300
            self.w(OBJECTS + i * 4, light); self.w(light, 0xa0adec)
            self.w(light + 0x108, SCENE); self.w(light + 0x250, 100 + i)
            self.f(light + 0x140, (float(i * 3), 0., 0.))
        u.mem_write(CFG, bytes(relocated_config(config_tree())))
        self.w(MANAGER + 0x98, PHYSICS); self.w(BULLET + 0x44, MANAGER)
        self.w(MANAGER + 0x90, RENDERER); self.w(MANAGER + 0x94, MATERIALS)
        self.w(BULLET + 0x48, BULLET_MAT)
        for address, ident in ((BULLET_MAT, 7), (GLASS_MAT, 29)):
            u.mem_write(address + 0x64, struct.pack('<HH', ident, 0xffff))
            self.w(MATERIALS + 0x108 + ident * 4, address)
        self.w(MATERIALS + 0x308 + 4 * (7 * 128 + 29), PAIR)
        self.w(PAIR + 0x18, HEAP + 0x215000)
        self.w(PAIR + 0x24, PARTICLES); self.w(PAIR + 0x28, PARTICLES + 4)
        self.w(PAIR + 0x74, 1)
        # Ragdoll's neighboring hook uses this import for its own lock.
        self.w(0x870118, RET + 0x100)
        u.mem_write(RET + 0x100, b'\xb8\x01\x00\x00\x00\xc3')

    def map_metadata(self, metadata):
        assert not self.active
        self.u.mem_write(CFG, bytes(relocated_config({'srp_lamps': metadata})))
        rows = metadata['lamps'].values()
        indices = [v['index'] for row in rows for v in row['visuals'].values()]
        count = max(indices) + 1
        assert count * 0x48 < OBJECTS - VISUALS
        self.w(PROJECT + 0x1ec, count)
        for i in range(count):
            self.w(VISUALS + i * 0x48, 0xa029b4)
            self.w(VISUALS + i * 0x48 + 0x44, 0)
        for i in indices:
            self.w(VISUALS + i * 0x48 + 0x44, MODELS + i * 0x200)
        lights = [light for row in rows for light in row['lights'].values()]
        self.w(PROJECT + 0x188, OBJECTS + len(lights) * 4)
        for i, position in enumerate(lights):
            light = LIGHTS + i * 0x300
            self.w(OBJECTS + i * 4, light); self.w(light, 0xa0adec)
            self.w(light + 0x108, SCENE); self.w(light + 0x250, 100 + i)
            self.f(light + 0x140, tuple(position[k] for k in ('x', 'y', 'z')))
        return {('model', v['index']) for row in rows for v in row['visuals'].values() if v['state'] == 0} | {
            ('light', i) for i in range(len(lights))}

    def w(self, at, value):
        self.u.mem_write(at, struct.pack('<I', value))

    def r(self, at):
        return struct.unpack('<I', self.u.mem_read(at, 4))[0]

    def f(self, at, values):
        self.u.mem_write(at, struct.pack('<%df' % len(values), *values))

    def finish(self, cleanup):
        esp = self.u.reg_read(UC_X86_REG_ESP)
        self.u.reg_write(UC_X86_REG_EIP, self.r(esp))
        self.u.reg_write(UC_X86_REG_ESP, esp + 4 + cleanup)

    def endpoint(self, u, va, _size, _data):
        esp = u.reg_read(UC_X86_REG_ESP)
        if va in (0x21cd0, 0x21f40):
            key = ('model', (u.reg_read(UC_X86_REG_EAX) - MODELS - 0x108) // 0x200)
            add, cleanup = va == 0x21cd0, 16 if va == 0x21cd0 else 8
        else:
            key = ('light', self.r(esp + 12) - 100)
            add, cleanup = va == 0x24750, 16 if va == 0x24750 else 12
        assert (key not in self.active) if add else (key in self.active), (va, key, self.active)
        (self.active.add if add else self.active.remove)(key)
        self.events.append((add, key)); self.finish(cleanup)

    def write(self, u, *_):
        self.output += u.mem_read(u.reg_read(UC_X86_REG_EAX), 4)
        self.finish(20)

    def effect(self, u, va, _size, _data):
        esp = u.reg_read(UC_X86_REG_ESP)
        assert self.r(esp + 4) == MANAGER
        if va == 0x3223e0:
            assert self.r(esp + 8) == PARTICLES and self.r(esp + 20) == 1
            point, kind, cleanup = u.reg_read(UC_X86_REG_EAX), 'shards', 20
            direction = struct.unpack('<3f', u.mem_read(self.r(esp + 12), 12))
            normal = struct.unpack('<3f', u.mem_read(self.r(esp + 16), 12))
            assert normal == tuple(-v for v in direction)
        else:
            assert self.r(esp + 8) == PAIR + 0x18
            point, kind, cleanup = self.r(esp + 12), 'glass sound', 12
        self.effects.append((kind, struct.unpack('<3f', u.mem_read(point, 12))))
        self.finish(cleanup)

    def run(self, site, stop=RET, args=(), regs=None, flags=2):
        u = self.u
        u.mem_write(STACK, struct.pack('<%dI' % (1 + len(args)), RET, *args))
        u.reg_write(UC_X86_REG_ESP, STACK)
        for i, reg in enumerate(REGS):
            u.reg_write(reg, HEAP + 0x100000 + i * 0x100)
        for reg, val in (regs or {}).items():
            u.reg_write(reg, val)
        u.reg_write(UC_X86_REG_EFLAGS, flags)
        u.emu_start(site, stop, count=2_000_000)
        assert u.reg_read(UC_X86_REG_EIP) == stop, hex(u.reg_read(UC_X86_REG_EIP))
        return u.reg_read(UC_X86_REG_ESP)

    def insert(self):
        # Enter through the real match insertion ABI, with the scene's
        # camera deliberately distinct from the physics argument. Stop
        # after its native insert_static call, before unrelated NPC setup.
        self.run(0x285750, 0x285793, args=(PHYSICS, CORE), regs={UC_X86_REG_ECX: PROJECT})

    def remove(self):
        assert self.run(0x2855b0, regs={UC_X86_REG_ESI: PROJECT}) == STACK + 4

    def present(self):
        self.run(0x1f7c41, 0x1f7c6c, flags=0x42)

    def shoot(self, start=(-1., 0., 0.), length=2., hit=None, direction=(1., 0., 0.)):
        self.f(STACK + 0x90, start); self.f(STACK + 0x20, direction); self.f(STACK + 0x14, (length,))
        self.w(RESULT + 0x1c, HEAP if hit is not None else 0)
        if hit is not None:
            self.f(RESULT, hit)
        before = bytes(self.u.mem_read(BULLET, 0x100))
        self.run(0x32d627, 0x32d63c, regs={UC_X86_REG_EAX: RESULT, UC_X86_REG_EDI: BULLET})
        assert before == bytes(self.u.mem_read(BULLET, 0x100))
        assert bytes(self.u.mem_read(STACK + 0x40, 8)) == bytes(self.u.mem_read(RESULT + 8, 8))

    def save(self):
        self.output.clear()
        assert self.run(0x321b90, args=(WRITER,), regs={UC_X86_REG_EDI: MANAGER}) == STACK + 8
        return bytes(self.output)

    def load(self, data):
        self.u.mem_write(BUFFER, data)
        self.w(READER, BUFFER); self.w(READER + 4, BUFFER); self.w(READER + 8, len(data))
        assert self.run(0x321c50, args=(READER,), regs={UC_X86_REG_EAX: MANAGER}) == STACK + 8
        assert self.r(READER + 4) == BUFFER + len(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('original', type=Path)
    parser.add_argument('--patched', type=Path, help='Exercise an already-built client, without relinking')
    args = parser.parse_args()
    original = args.original.read_bytes()
    if args.patched:
        built = args.patched.read_bytes()
    else:
        built, _ = base.build(original, 'section', Path(base.default_bin('section')).read_bytes(), None,
                              Path(base.late_join_bin()).read_bytes())
    for va, old, _, _ in base.LAMPS_HOOKS:
        assert original[base.file_offset(va):base.file_offset(va) + len(old)] == old
    g = Game(built)
    g.insert()
    intact = {('model', i) for i in (0, 2, 4)} | {('light', 0), ('light', 1)}
    assert g.active == intact
    print('ok: visual insert through project loop; light insert through project loop')
    # Exercise both displaced branches; the fallthrough must still execute
    # its native fld, and neither path may disturb the frame's registers.
    for flags, stop in ((0x42, 0x1f7c6c), (2, 0x1f7c4a)):
        assert g.run(0x1f7c41, stop, flags=flags) == STACK
        assert g.u.reg_read(UC_X86_REG_EFLAGS) == flags
        assert all(g.u.reg_read(reg) == HEAP + 0x100000 + i * 0x100 for i, reg in enumerate(REGS))
    print('ok: both frame continuations preserve registers and flags')
    fresh = g.save()
    assert len(fresh) == 28 and struct.unpack_from('<II', fresh, 16) == (0x31504d4c, 2)
    # A separate unconfigured scenery project must neither reject nor clear
    # the active match's lamp owner. Its native arrays are empty.
    scenery, config, root = HEAP + 0xc0000, HEAP + 0xc1000, HEAP + 0xc2000
    g.w(scenery + 0x17c, config); g.w(config + 0x108, root)
    g.w(scenery + 0x180, SCENE)
    g.u.mem_write(root, bytes(relocated_config({})))
    g.run(0x285750, 0x285793, args=(PHYSICS, CORE), regs={UC_X86_REG_ECX: scenery})
    g.run(0x2855b0, regs={UC_X86_REG_ESI: scenery})
    assert g.active == intact and g.save() == fresh
    print('ok: unrelated scenery does not steal or clear the active lamp project')
    for kw in ({'hit': (-.5, 0., 0.)}, {'start': (-1., .2, 0.)}, {'length': .5}):
        g.shoot(**kw); g.present(); assert g.active == intact
    assert not g.effects
    print('ok: misses, short segments and an intervening wall do not break lamps')
    g.shoot(); g.present()
    broken = (intact - {('model', 0), ('model', 4), ('light', 0)}) | {('model', 1)}
    assert g.active == broken
    assert g.effects == [('shards', (0., 0., 0.)), ('glass sound', (0., 0., 0.))]
    events = len(g.events)
    g.shoot(); g.present(); assert len(g.events) == events
    assert len(g.effects) == 2
    damaged = g.save(); assert damaged != fresh
    print('ok: hit switches model and removes light/cone once, preserving the bullet')
    g.load(fresh); g.present(); assert g.active == intact
    g.shoot(); assert g.save() == damaged
    g.present(); assert g.active == broken
    g.load(damaged); g.present(); assert g.active == broken
    print('ok: rollback restores the intact lamp; resimulation and snapshot load reproduce the break')
    late = Game(built); late.insert(); late.load(damaged); late.present(); assert late.active == broken
    assert not late.effects  # snapshot loading must not replay a historical break
    quiet = Game(built); quiet.insert(); quiet.u.mem_write(BULLET + 0x80, b'\x01')
    quiet.shoot(); quiet.present(); assert quiet.active == broken and not quiet.effects
    print('ok: each new break queues glass sound and shards once; quiet bullets and snapshot loads stay silent')
    for invalid in (damaged[:-1], damaged[:16], damaged[:16] + bytes(12),
                    damaged[:20] + struct.pack('<I', 256) + damaged[24:]):
        try:
            late.load(invalid)
        except UcError:
            pass
        else:
            raise AssertionError('invalid lamp history accepted')
        late.present(); assert late.active == broken
    print('ok: truncated, unknown and mismatched histories rejected before lamp state changes')
    g.shoot(length=5.); g.present(); assert g.active == {('model', 1), ('model', 3)}
    print('ok: late join/replay snapshot; two lamps on one unobstructed segment')
    g.remove(); assert not g.active
    assert all(g.r(VISUALS + i * 0x48) == 0xa029b4 for i in range(5))
    assert all(g.r(LIGHTS + i * 0x300) == 0xa0adec for i in range(2))
    print('ok: visual remove through project loop; light remove through project loop; original vtables restored')
    g.insert(); assert g.active == intact and g.save() == fresh
    # Destruction without remove still drops exactly the inserted objects
    # before the native destructor starts releasing members.
    g.run(0x285330, 0x28533a, regs={UC_X86_REG_ECX: PROJECT})
    assert not g.active
    assert all(g.r(VISUALS + i * 0x48) == 0xa029b4 for i in range(5))
    # A map without metadata retains the original bullet wire format.
    g.u.mem_write(CFG, bytes(relocated_config({})))
    g.insert(); assert len(g.save()) == 16
    g.remove()
    print('ok: new match resets lamps; maps without lamp metadata retain native history')
    # Every shipped metadata row is accepted by the actual client parser and
    # dispatches its intended model/light instances. Unrelated visuals are
    # empty native models; the renderer endpoints remain the only stubs.
    source = json.loads((ROOT / 'scripts/content-edit/breakable-lamps-1.json').read_text())
    for entry in source['add_value']:
        expected = g.map_metadata(entry['value'])
        g.insert(); assert g.active == expected, entry['files']
        g.remove(); assert not g.active, entry['files']
    print(f"ok: all {len(source['add_value'])} map variants bind, insert and remove their authored fixtures through native code")


if __name__ == '__main__':
    main()
