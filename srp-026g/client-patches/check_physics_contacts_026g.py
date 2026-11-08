#!/usr/bin/env python3
"""Patch 121 (physics_contacts): a world carries the contact manifolds of the
bodies that move, and every load puts them back (docs/notes/physics_contacts.md).

python check_physics_contacts_026g.py ORIGINAL [PATCHED]

Runs the game's own Bullet on made-up bodies, in the original exe and in the
patched one: the collision configuration (0x8EEE0) and the dispatcher
(0x8F2C0) as bullet_physics_world::initialize makes them, the pair cache
(0xB3F40, addOverlappingPair), the near callback (0x8F860) with its
findAlgorithm (the convex-concave algorithm of a box and a triangle mesh, its
getNewManifold), and the patch's sites inside game_world_core::serialize
(0x2D26DB), the load's end (0x2D2D5D), internalSingleStepSimulation (0x9B3E3)
and solveGroup (0x9A8B0). Only the shapes (their type and contact breaking
threshold), the bodies and the worlds around the dispatcher are made up; the
allocator is a bump one.

- A rollback: a battery's manifold holds the points of tick T when the world
  of T is saved, other points when the game goes on; the world of T loaded,
  the near callback's processCollision gets the points of T (the original:
  the later ones, the divergence of match 0x54417103).
- A joiner: a fresh dispatcher with no manifold loads that world; the near
  callback makes the algorithm and its manifold, and processCollision gets
  the points of T (the original: none).
- Asleep: the same with the battery and the level asleep (the near callback
  takes no pair); after the step's collision detection the pair has its
  algorithm, in the world's order, its manifold the points of T, the bodies
  asleep again (the original: no algorithm).
- The world's bytes: the record as srp_validate::world::Contacts reads it
  (the battery's origin, the level's box, the order, the points).
- solveGroup gets its manifolds in their bodies' order whatever the order of
  the batch (the original: the batch's).
"""
import struct
import sys
from pathlib import Path
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_ESI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)
import patch_026g as base
import check_hide_ui_026g as hide

CONFIG_CTOR, DISPATCHER_CTOR, CACHE_CTOR, NEAR = 0x8EEE0, 0x8F2C0, 0xB3F40, 0x8F860
PROCESS_CALL = 0x8FA07                          # the near callback's discrete processCollision call
SAVED, SAVED_BACK = 0x2D26DB, 0x2D26E1          # mov ecx,[ebp+19030h] (6)
LOADED, LOADED_BACK = 0x2D2D5D, 0x2D2D64        # mov byte [ebp+18FE0h],0 (7)
DETECTED, DETECTED_BACK = 0x9B3E3, 0x9B3EB      # mov eax,[esi] / mov edx,[eax+8Ch] (8)
SOLVE, SOLVE_BACK = 0x9A8B0, 0x9A8B5            # mov edx,[esp+14h] / push ebx (5)
ALLOC_SLOT, FREE_SLOT = 0xA7AAF0, 0xA7AAF4      # btAlignedAlloc / btAlignedFree, cdecl

HEAP, HEAP_SIZE = 0x40000000, 0x8000000
FIX, FIX_SIZE = 0x30000000, 0x1000000
STUBS = FIX                                     # alloc, free, the shapes' breaking threshold
SHAPE_VT, BOX, MESH, INFO, CONST = FIX + 0x1000, FIX + 0x1100, FIX + 0x1200, FIX + 0x1300, FIX + 0x1400
WORLD_VT = FIX + 0x2000                         # a made-up dynamics world's vtable: zeros
STACK = FIX + 0x10000
STOP = FIX + 0x20000
POINT, MANIFOLD_POINTS, CACHED, BODY0, BODY1 = 0x120, 0x10, 0x498, 0x490, 0x494
FIELDS = ((0x00, 12), (0x10, 12), (0x20, 12), (0x30, 12), (0x40, 12), (0x50, 28), (0x70, 4), (0x74, 1),
          (0x78, 24), (0x90, 4), (0xA0, 12), (0xB0, 12))
POINT_BYTES = sum(n for _, n in FIELDS)


def fnv1a(words):
    h = 0xCBF29CE484222325
    for w in words:
        for b in struct.pack('<I', w):
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def bits(f):
    u = struct.unpack('<I', struct.pack('<f', f))[0]
    return 0 if u == 0x80000000 else u


class Game(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(HEAP, HEAP_SIZE, UC_PROT_ALL)
        emu.mem_map(FIX, FIX_SIZE, UC_PROT_ALL)
        self.next = HEAP
        emu.mem_write(STUBS, b'\xc3')                                  # alloc: the hook answers
        emu.mem_write(STUBS + 0x10, b'\xc3')                           # free: nothing
        emu.mem_write(STUBS + 0x20, b'\xd9\x05' + struct.pack('<I', CONST) + b'\xc2\x04\x00')  # fld [CONST]; ret 4
        emu.mem_write(CONST, struct.pack('<f', 0.02))
        self.word(ALLOC_SLOT, STUBS)
        self.word(FREE_SLOT, STUBS + 0x10)
        emu.hook_add(UC_HOOK_CODE, self.alloc, begin=STUBS, end=STUBS)
        self.word(SHAPE_VT + 0x10, STUBS + 0x20)                       # getContactBreakingThreshold
        for shape, kind in ((BOX, 0), (MESH, 21)):                     # a box, a triangle mesh
            self.word(shape, SHAPE_VT)
            self.word(shape + 4, kind)
        self.word(INFO + 8, 1)                                         # m_dispatchFunc: discrete
        emu.mem_write(STOP, b'\xcc')
        self.objects = FIX + 0x100000

    def run(self, start, stops, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.calls, self.invoked = stops, None, False, [], []
        self.emu.emu_start(start, 0, count=20000000)

    def alloc(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        want = self.read(sp + 4)
        self.next = (self.next + 15) & ~15
        emu.reg_write(UC_X86_REG_EAX, self.next)
        emu.mem_write(self.next, bytes(want))
        self.next += want

    def new(self, size):
        at = self.objects
        self.objects = (self.objects + size + 0xFFF) & ~0xFFF
        self.emu.mem_write(at, bytes(size))
        return at

    def float(self, at, *values):
        self.emu.mem_write(at, struct.pack('<%df' % len(values), *values))

    def call(self, va, args=(), regs=None, stops=(STOP,)):
        sp = STACK + 0x8000
        self.emu.mem_write(sp, struct.pack('<%dI' % (len(args) + 1), STOP, *args))
        regs = {**hide.GAME, **(regs or {})}
        regs.pop(UC_X86_REG_ESP, None)
        self.run(va, stops, sp, regs)
        return self.stopped

    def run_site(self, start, back, regs, sp=None):
        sp = sp or STACK + 0x8000
        self.run(start, (back,), sp, {**hide.GAME, **regs})
        assert self.stopped == back, (hex(start), self.stopped)

    # The physics world as bullet_physics_world::initialize makes it.
    def physics(self):
        config = self.new(0x6C)
        info = self.new(0x40)
        self.float(info, 0)
        struct_info = struct.pack('<8I', 0, 0, 0, 0x1000, 0x1000, 0, 0, 1)
        self.emu.mem_write(info, struct_info)
        assert self.call(CONFIG_CTOR, (info,), {UC_X86_REG_EDI: config}) == STOP
        dispatcher = self.new(0x1520)
        assert self.call(DISPATCHER_CTOR, (), {UC_X86_REG_ESI: dispatcher, UC_X86_REG_EAX: config}) == STOP
        cache = self.new(0x4C)
        assert self.call(CACHE_CTOR, (), {UC_X86_REG_EAX: cache}) == STOP
        broadphase = self.new(0x80)
        self.word(broadphase + 0x64, cache)
        world = self.new(0x100)
        self.word(world, WORLD_VT)
        objects = self.new(0x40)
        self.word(world + 0x10, objects)
        self.word(world + 0x18, dispatcher)
        self.word(world + 0x4C, broadphase)
        physics = self.new(0x100)
        self.word(physics + 0x3C, world)
        core = self.new(0x19100)
        self.word(core + 0x18FA4, physics)
        return dict(dispatcher=dispatcher, cache=cache, world=world, objects=objects, core=core)

    def body(self, w, origin, shape, group, mask, flags, unique, aabb):
        body, proxy = self.new(0x140), self.new(0x40)
        self.float(body + 0x10, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, *origin, 0)
        self.float(body + 0xC4, 1e18)                                  # contact processing threshold
        self.word(body + 0xC8, proxy)
        self.word(body + 0xCC, shape)
        self.word(body + 0xD8, flags)
        self.word(body + 0xE4, 1)
        self.word(proxy, body)
        self.emu.mem_write(proxy + 4, struct.pack('<HH', group, mask))
        self.word(proxy + 0xC, unique)
        self.float(proxy + 0x10, *aabb[0], 0, *aabb[1], 0)
        n = self.read(w['world'] + 8)
        self.word(w['objects'] + 4 * n, body)
        self.word(w['world'] + 8, n + 1)
        return body

    # A battery over the level, both in a pair of the cache.
    def scene(self, at=(2.5, 0.25, -1.5)):
        w = self.physics()
        w['level'] = self.body(w, (0, 0, 0), MESH, 8, 0x10, 1, 1, ((-100, -10, -100), (100, 10, 100)))
        w['battery'] = self.body(w, at, BOX, 0xA18, 0x418, 0, 2, ((at[0] - .5, at[1] - .25, at[2] - .5),
                                                                 (at[0] + .5, at[1] + .25, at[2] + .5)))
        level_proxy, battery_proxy = self.read(w['level'] + 0xC8), self.read(w['battery'] + 0xC8)
        cache_vt = self.read(w['cache'])
        add = self.read(cache_vt + 4)                                  # addOverlappingPair
        assert self.call(add, (level_proxy, battery_proxy), {UC_X86_REG_ECX: w['cache']}) == STOP
        w['pair'] = self.emu.reg_read(UC_X86_REG_EAX)
        assert w['pair'] and self.read(w['pair']) == level_proxy and self.read(w['pair'] + 4) == battery_proxy
        item = self.new(0xBC000)
        data = self.new(0x10000)
        self.word(item + 0xBBF08 + 4, data)
        self.word(item + 0xBBF08 + 8, 0x10000)
        self.word(item + 0xBBF18 + 0x14, item + 0xBBF08)
        w['item'] = item
        return w

    def near(self, w):
        """The near callback on the scene's pair: whether it reached processCollision."""
        return self.call(NEAR, (w['pair'], w['dispatcher'], INFO), stops=(STOP, PROCESS_CALL)) == PROCESS_CALL

    def manifolds(self, w):
        d = w['dispatcher']
        n, data = self.read(d + 0xC), self.read(d + 0x14)
        return [self.read(data + 4 * i) for i in range(n)]

    def points(self, manifold):
        n = self.read(manifold + CACHED)
        return [self.fields(manifold + MANIFOLD_POINTS + POINT * i) for i in range(n)]

    def fields(self, point):
        return b''.join(bytes(self.emu.mem_read(point + o, n)) for o, n in FIELDS)

    def put(self, manifold, seed, count):
        for i in range(count):
            at = manifold + MANIFOLD_POINTS + POINT * i
            self.emu.mem_write(at, bytes(POINT))
            self.float(at, *[seed + i + k / 8 for k in range(28)])
            self.word(at + 0x6C, 0)                                    # no user data
            self.word(at + 0x90, int(7 + seed + i))                    # its life time
        self.word(manifold + CACHED, count)

    def save(self, w):
        self.word(w['item'] + 0xBBF08 + 0xC, 0)
        self.run_site(SAVED, SAVED_BACK, {UC_X86_REG_ESI: w['item'], UC_X86_REG_EBP: w['core']})
        size = self.read(w['item'] + 0xBBF08 + 0xC)
        return bytes(self.emu.mem_read(self.read(w['item'] + 0xBBF08 + 4), size))

    def load(self, w, world):
        data = self.read(w['item'] + 0xBBF08 + 4)
        self.emu.mem_write(data, world)
        self.word(w['item'] + 0xBBF08 + 0xC, len(world))
        self.run_site(LOADED, LOADED_BACK, {UC_X86_REG_EBX: w['item'], UC_X86_REG_EBP: w['core']})

    def detected(self, w):
        self.run_site(DETECTED, DETECTED_BACK, {UC_X86_REG_ESI: w['world']})


def expected_record(game, w, points):
    battery, level = w['battery'], w['level']
    origin = struct.unpack('<3f', bytes(game.emu.mem_read(battery + 0x40, 12)))
    proxy = game.read(level + 0xC8)
    aabb = struct.unpack('<8I', bytes(game.emu.mem_read(proxy + 0x10, 32)))
    h = fnv1a([0 if aabb[i] == 0x80000000 else aabb[i] for i in (0, 1, 2, 4, 5, 6)])
    key0 = struct.pack('<B3I', 1, *[bits(v) for v in origin])
    key1 = struct.pack('<B3I', 2, h & 0xFFFFFFFF, h >> 32, 8 | 0x10 << 16)
    # the near callback passes the level first (nearer the origin; proxy 0)
    return bytes([1, 1]) + key0 + key1 + bytes([0, len(points)]) + b''.join(points)


def checks(original, built):
    for data, fixed in ((original, False), (built, True)):
        game = Game(data)
        # the step before tick T: the near callback makes the pair's algorithm and its manifold
        w = game.scene()
        assert game.near(w)
        (manifold,) = game.manifolds(w)
        assert game.read(manifold + BODY0) == w['battery'] and game.read(manifold + BODY1) == w['level']
        game.put(manifold, 1.0, 3)
        at_t = game.points(manifold)
        world = game.save(w)
        if fixed:
            assert world == expected_record(game, w, at_t), world.hex()
        else:
            assert world == b''                                        # a stock world: no contacts
        # the game goes on: the manifold's points are of a later tick
        game.put(manifold, 5.0, 4)
        later = game.points(manifold)
        assert later != at_t
        # a rollback to T: the world of T loaded, the next step
        game.load(w, world)
        assert game.near(w)
        assert game.manifolds(w) == [manifold]
        got = game.points(manifold)
        assert got == (at_t if fixed else later), 'rollback: processCollision gets other contacts than at T'

        # a joiner of tick T: a fresh dispatcher, no algorithm
        j = game.scene()
        assert game.manifolds(j) == []
        game.load(j, world)
        assert game.near(j)
        (made,) = game.manifolds(j)
        assert game.points(made) == (at_t if fixed else []), 'joiner: processCollision gets other contacts than at T'

        # asleep: the near callback takes no pair; the step's collision detection over, its algorithm
        s = game.scene()
        for body in (s['battery'], s['level']):
            game.word(body + 0xE4, 2)
        game.load(s, world)
        assert not game.near(s)
        assert game.read(s['pair'] + 8) == 0
        game.detected(s)
        if fixed:
            assert game.read(s['pair'] + 8) != 0 and game.read(s['pair'] + 0xC) == 0
            (made,) = game.manifolds(s)
            assert game.points(made) == at_t
            assert [game.read(b + 0xE4) for b in (s['battery'], s['level'])] == [2, 2]
        else:
            assert game.read(s['pair'] + 8) == 0 and game.manifolds(s) == []
        # and its next save holds the same world
        if fixed:
            assert game.save(s) == world

        # solveGroup: a batch's manifolds in their bodies' order, whatever the batch's
        other = game.scene((-4.0, 0.25, 3.0))
        assert game.near(other)
        (second,) = game.manifolds(other)
        game.put(second, 2.0, 1)
        orders = []
        for batch in ((manifold, second), (second, manifold)):
            frame = STACK + 0x7000
            array = STACK + 0x6000
            game.emu.mem_write(array, struct.pack('<2I', *batch))
            game.emu.mem_write(frame, struct.pack('<5I', STOP, 0, 0, array, 2))
            game.run(SOLVE, (SOLVE_BACK,), frame, {**hide.GAME, UC_X86_REG_ECX: 0x0C0C0C0C})
            assert game.stopped == SOLVE_BACK
            orders.append(struct.unpack('<2I', bytes(game.emu.mem_read(array, 8))))
        if fixed:
            assert orders[0] == orders[1]
        else:
            assert orders[0] != orders[1]
    print('ok: the original steps a rollback, a joiner and a sleeper from other contacts than the world\'s;'
          ' patched every load gives processCollision the contacts of the world, and solveGroup one order')


if __name__ == '__main__':
    original = Path(sys.argv[1]).read_bytes()
    if len(sys.argv) > 2:
        built = Path(sys.argv[2]).read_bytes()
    else:
        built, _ = base.build(original, 'section', Path(base.default_bin('section')).read_bytes(), None,
                              Path(base.late_join_bin()).read_bytes())
    checks(original, built)
