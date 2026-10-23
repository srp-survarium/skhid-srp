#!/usr/bin/env python3
"""Run patch 56 (fast_weapon_026g.asm, an experiment: a weapon's upgrade
window shows the weapon above the workbench without making the lobby
character again; client_patch.md section 3.53) in the build, with the
game's own code: setup_profile's entry (0x284B10) through the stash's and
patch 32's hooks to the site at 0x284B16, lobby_character::update's call of
player::mt_draw (from 0x284A7E to the update's `ret 0Ch`), the lobby menu's
unload call (0x21D093), the game's member callback 0x27D280 and its invoker
0x202300 for the weapon query's callback, the record's constructor and copy
(0x2F7C10, 0x270390), the query data's type (0x2C8090) and
weapon::calculate_locator (0x243BC0) for the scope, on a made-up lobby.

    python check_fast_weapon_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). The image is loaded as is (the exe is
never rebased). Stubs that write down how they were called: query_resources
(0xE5B60), the scene renderer's add_model, remove_model, update_model
(0x21CD0, 0x21F40, 0x22280: each checks the instance's in-scene flag as the
game's asserts do and sets it), a resource's release (0xE0210), mt_draw
(0x2474A0), the unload's next call (0x2855B0), the game's log (0x111ED0),
setup_profile's body after the site (0x284B1D: the character made again),
the network client's vtable+34h. A query is answered by calling the
callback through the function query_resources was given, as the resource
system does (its manager's invoker), with a made-up queries_result holding a
made-up weapon (its skeleton model's render instance, its render model's
box and inverse binds, a scope with its locator), whose one reference the
result lets go after the callback. What must hold:
- creation: a weapon's window (patch 32's mode 1) asks for the weapon of the
  record its kit holds, a query of class 46h "weapon" with the inventory's
  data (the record's two vectors, kept in the state; the dictionary, the
  lobby menu, the dict id, the client's word, 1), and setup_profile returns
  without its body: the character's bytes are as they were;
- placement: once the weapon comes its model and its scope's go into the
  lobby's scene once and are moved every frame to patch 45's transform (the
  middle at the bench, turned by the spin and the tilt; the scope at its
  locator through it, the locator's bone in the bind pose); patch 45's READY,
  transform and stamp; one log line with the time; mt_draw is called from
  the patch (patch 45 then leaves the held item in the hand);
- close: the models leave the scene, the weapon is kept; the character is
  not made again; mt_draw is the update's call again (return 0x284A83);
- repeated opens: the same record shows the weapon kept at once, with no
  query; another weapon or a changed record (an attach) is asked for, the
  one shown stays until it comes; a record asked while another is in flight
  waits for it and that one is let go;
- the unload lets the weapon go and the query in flight is let go when it
  comes; a squad mate's character, armour and a closed window are the
  original's way;
- no leak: 100 opens and closes over three weapons and an attach: at most two
  weapons alive, every one made let go but the one kept, the scene empty
  after each close, the state's size fixed.
Says nothing about what the game draws.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base

# The game's
SETUP, SETUP_BODY, SITE = 0x284B10, 0x284B1D, 0x284B16
UPDATE_CALL, UPDATE_AFTER = 0x284A7E, 0x284A83
UNLOAD_CALL, UNLOAD_AFTER = 0x21D093, 0x21D098
QUERY, ADD, REMOVE, UPDATE, RELEASE = 0xE5B60, 0x21CD0, 0x21F40, 0x22280, 0xE0210
MT_DRAW, UNLOAD_NEXT, LOG = 0x2474A0, 0x2855B0, 0x111ED0
INVOKER, MEMBER_MANAGER = 0x202300, 0xA09374
WEAPON_TYPE, WEAPON_HELPER, WEAPON_PATH = 0x2C8090, 0xA0E7D0, 0xA04840
WEAPON_VTABLE, SKELETON_VTABLE = 0xA04174, 0x9F0F24

# Made-up memory
CHAR, SQUAD, MENU, GAME = 0x20000000, 0x20010000, 0x20020000, 0x20030000
CLIENT, PROFILE, STUBS, STACK = 0x20040000, 0x20050000, 0x20060000, 0x20078000
RESULT, STOP, RENDER_HOLD, CLIENT_DATA = 0x20080000, 0x20090000, 0x20100000, 0x20400000
SCENE, RENDERER, DICTIONARY, PLAYER = 0x20500000, 0x20500100, 0x20500200, 0x20500300
WEAPONS = 0x21000000                           # one 64 KiB region a weapon
CLIENT_WORD = 0x0C11E07
KIT0, KIT_SIZE, SLOTS, TABLE, ITEM_SIZE = 0x58, 0x704, 0x40, 0x1568, 0xB4

FAST = base.FAST_WEAPON_VA
STATE = base.FAST_WEAPON_STATE_VA
LATE = base.late_join_state_va(open(base.late_join_bin(), "rb").read())
PREVIEW = LATE + base.WEAPON_PREVIEW_STATE_OFFSET
TABLE_STATE = LATE + base.WEAPON_TABLE_STATE_OFFSET
# the state's fields (fast_weapon_026g.asm)
S_FAST, S_ITEM, S_HAVE, S_SHOWN, S_PENDING, S_WANT, S_DROP, S_ASKED, S_TIMING, S_KEPT, S_WORLD, S_QUERIES, \
    S_READY_SEEN, S_MADE = range(0, 0x38, 4)
S_MATRIX, S_SCOPE, S_REC, S_WANT_REC, S_HAVE_REC = 0x40, 0x80, 0x100, 0x1C0, 0x280
T_SPIN, T_TILT, T_READY, T_STAMP, T_MATRIX = 0x20, 0x24, 0x28, 0x48, 0x50
BENCH = (4.25, 1.30, 3.05)

# Weapons: item id, dict id, box (min, max) or None (the default), the scope's locator bone (or None)
AK, SVD, VEPR, PISTOL, VEST = 701, 702, 703, 704, 705
DICT = {AK: 999, SVD: 998, VEPR: 216, PISTOL: 997, VEST: 996}
BOXES = {999: ((-0.0224, -0.1585, -0.6726), (0.0315, 0.1114, 0.2654)),
         998: ((-0.03, -0.12, -0.9), (0.03, 0.15, 0.3)), 216: None, 997: ((-0.08, -0.07, -0.12), (0.02, 0.05, 0.03))}
MIDDLES_216 = (-0.005, -0.031, 0.021)          # patch 45's MIDDLES row of the Vepr (bind space)


def f32(v):
    return struct.unpack("<I", struct.pack("<f", v))[0]


def ff(word):
    return struct.unpack("<f", struct.pack("<I", word & 0xFFFFFFFF))[0]


def matmul(a, b):
    """Rows (v * M), 4x4 as 16 floats."""
    return [sum(a[r * 4 + k] * b[k * 4 + c] for k in range(4)) for r in range(4) for c in range(4)]


def affine(a, b):
    """0xEE700: a * b with the w column taken as 0, 0, 0, 1."""
    a = list(a)
    b = list(b)
    for m in (a, b):
        m[3], m[7], m[11], m[15] = 0.0, 0.0, 0.0, 1.0
    return matmul(a, b)


def rot_y(angle):
    c, s = math.cos(angle), math.sin(angle)
    return [c, 0, s, 0, 0, 1, 0, 0, -s, 0, c, 0, 0, 0, 0, 1]


def rigid(yaw, pitch, t):
    """A rigid float4x4 (rows): rotY(yaw) * rotX(pitch), then t."""
    c, s = math.cos(pitch), math.sin(pitch)
    rx = [1, 0, 0, 0, 0, c, -s, 0, 0, s, c, 0, 0, 0, 0, 1]
    m = matmul(rot_y(yaw), rx)
    m[12:15] = t
    return m


def inverse_rigid(m):
    r = [m[0:3], m[4:7], m[8:11]]
    t = m[12:15]
    out = [r[0][0], r[1][0], r[2][0], 0, r[0][1], r[1][1], r[2][1], 0, r[0][2], r[1][2], r[2][2], 0]
    out += [-sum(t[k] * r[j][k] for k in range(3)) for j in range(3)] + [1.0]
    return out


class Lobby:
    def __init__(self, data, body=False, measure=False):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((CHAR, 0x10000), (SQUAD, 0x10000), (MENU, 0x10000), (GAME, 0x10000), (CLIENT, 0x10000),
                         (PROFILE, 0x10000), (STUBS, 0x10000), (STACK - 0x8000, 0x10000), (RESULT, 0x10000),
                         (STOP, 0x1000), (RENDER_HOLD, 0x201000), (CLIENT_DATA, 0x60000), (SCENE, 0x1000),
                         (WEAPONS, 0x400000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.stubs = {QUERY: self.query, ADD: self.add, REMOVE: self.remove, UPDATE: self.update,
                      RELEASE: self.release, MT_DRAW: self.mt_draw, UNLOAD_NEXT: self.unload_next, LOG: self.log,
                      SETUP_BODY: self.setup_body, STUBS: self.client_data}
        if body:                                  # setup_profile's own body runs (its copies made by
            del self.stubs[SETUP_BODY]            # player_profile's constructor)
        for at in self.stubs:
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        self.counting = None
        if measure:
            emu.hook_add(UC_HOOK_CODE, self.count)
        else:
            emu.hook_add(UC_HOOK_CODE, self.count, begin=FAST, end=FAST + 0x1000)
        self.queries, self.scene, self.updates, self.draws, self.lines = [], {}, [], [], []
        self.bodies, self.unloads, self.released, self.made, self.live = [], 0, [], [], set()
        self.clock = 1000
        if body:
            for at in [CHAR + 0x18 + 0x4634 * i for i in range(3)] + [PROFILE]:
                self.run(0x2F73F0, STOP, [STOP], {UC_X86_REG_ECX: at})
            self.word(CHAR + 0xD2C4, 0xFFFFFFFF)  # no player being made
        self.fill()

    # memory
    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def floats(self, at, n=16):
        return [ff(self.read(at + 4 * i)) for i in range(n)]

    def set_floats(self, at, values):
        for i, v in enumerate(values):
            self.word(at + 4 * i, f32(v))

    def fill(self):
        w = self.word
        w(MENU + 0x694, CHAR)
        w(MENU + 0x4, SCENE)
        w(MENU + 0x9C, GAME)
        w(MENU + 0x6A0, PROFILE)
        w(CHAR + 0xD2BC, MENU)
        w(CHAR + 0xD2B8, PLAYER)
        w(SQUAD + 0xD2BC, MENU)
        w(GAME + 0x53A4, DICTIONARY)
        w(GAME + 0x53AC, CLIENT)
        w(GAME + 0x5360, RENDER_HOLD)
        w(RENDER_HOLD + 0x200064, RENDERER)
        w(GAME + 0x584C, self.clock)
        w(CLIENT, CLIENT + 0x100)
        w(CLIENT + 0x100 + 0x34, STUBS)
        w(CLIENT_DATA + 0x577B0, CLIENT_WORD)
        self.emu.mem_write(STUBS, b"\xC3")      # never run: the stub returns
        # The account: kit 0 the AK (weapon 1) and the vest, kit 1 the SVD,
        # kit 2 the Vepr and a pistol (weapon 2); the character shows kit 0.
        self.items = [AK, SVD, VEPR, PISTOL, VEST]
        self.emu.mem_write(PROFILE + 0x57, bytes([3]))
        self.emu.mem_write(PROFILE + 0x1564, bytes([len(self.items)]))
        for k in range(3):
            self.word(PROFILE + 0x754 + k * KIT_SIZE, 11 + k)     # the kits' ids
            for slot in range(23):
                self.emu.mem_write(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot, b"\xff\xff\x00\x00")
        for k, kit in enumerate(({7: AK, 2: VEST}, {7: SVD}, {7: VEPR, 10: PISTOL})):
            for slot, item in kit.items():
                self.emu.mem_write(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot,
                                   struct.pack("<HH", self.items.index(item), 1))
        for index, item in enumerate(self.items):
            self.record(PROFILE + TABLE + ITEM_SIZE * index, item, [])

    def record(self, at, item, upgrades):
        """An item record as its constructor (0x2F7C10) leaves it, with
        upgrades {id, dict id, value, slot} in its second vector."""
        self.emu.mem_write(at, bytes(ITEM_SIZE))
        w = self.word
        w(at, item)
        self.emu.mem_write(at + 4, struct.pack("<H", DICT[item]))
        w(at + 0x14, at + 0x20)
        w(at + 0x18, at + 0x20)
        w(at + 0x1C, at + 0x60)
        w(at + 0x60, at + 0x6C)
        w(at + 0x64, at + 0x6C + 0x18 * len(upgrades))
        w(at + 0x68, at + 0xB4)
        for i, (uid, udict, value, slot) in enumerate(upgrades):
            self.emu.mem_write(at + 0x6C + 0x18 * i, struct.pack("<IIQB", uid, udict, value, slot))

    def item_record(self, item):
        return PROFILE + TABLE + ITEM_SIZE * self.items.index(item)

    def weapon(self, item, scope_bone=None, number=None):
        """A made-up weapon as weapon_cook makes it: one reference (the
        queries_result's), the lobby menu its world, its skeleton model's
        render instance, its render model's box and inverse binds, and a
        scope at a locator on bone scope_bone (None: no scope)."""
        number = len(self.made) if number is None else number
        at = WEAPONS + 0x10000 * (number % 64)
        assert at not in self.live, "a made-up weapon's memory in use"
        self.live.add(at)
        self.emu.mem_write(at, bytes(0x10000))
        w = self.word
        w(at, WEAPON_VTABLE)
        w(at + 0x10 + 0xD0, 1)                     # its resource (+10h, the inventory_item)'s count
        w(at + 0x180C, MENU)
        model, inst, render, binds = at + 0x2000, at + 0x3000, at + 0xA000, at + 0xB000
        w(at + 0x1810, model)
        w(model + 0x108, inst)
        w(inst, SKELETON_VTABLE)
        w(inst + 0x62B4, render)
        box = BOXES[DICT[item]]
        lo, hi = box if box else ((-1.0, -1.0, -1.0), (1.0, 1.0, 1.0))
        self.set_floats(render + 0x108, list(lo) + list(hi))
        self.binds = [rigid(0.3 * i, 0.1 * i, (0.01 * i, 0.02 * i, -0.05 * i)) for i in range(4)]
        for i, m in enumerate(self.binds):
            self.set_floats(binds + 64 * i, m)
        w(render + 0x140, binds)
        w(render + 0x144, binds + 64 * len(self.binds))
        if scope_bone is not None:
            scope, smodel, sinst = at + 0xC000, at + 0xD000, at + 0xE000
            w(at + 0x1808, scope)
            w(scope + 0x108, smodel)
            w(smodel + 0x108, sinst)
            self.locator = rigid(0.2, -0.1, (0.0, 0.07, 0.15))
            self.set_floats(at + 0x744 + 0x20, self.locator)
            self.emu.mem_write(at + 0x744 + 0x60, struct.pack("<H", 0xFFFF if scope_bone < 0 else scope_bone))
        self.made.append(at)
        return at

    # stubs
    def returns(self, pop, eax=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)
        for register in (UC_X86_REG_ECX, UC_X86_REG_EDX):
            self.emu.reg_write(register, 0xBADC0DE0)

    def stub(self, emu, address, size, _):
        handler = self.stubs.get(address)
        if handler:
            handler()

    def count(self, emu, address, size, _):
        if self.counting is not None:
            self.counting[0] += 1

    def arg(self, n):
        return self.read(self.emu.reg_read(UC_X86_REG_ESP) + 4 + 4 * n)

    def query(self):
        emu = self.emu
        requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
        function, allocator, users, parent, one = (self.arg(i) for i in range(5))
        variant = self.read(users)
        self.queries.append({
            "path": self.read(requests), "class": self.read(requests + 4), "count": count,
            "function": bytes(emu.mem_read(function, 0x20)), "allocator": allocator, "parent": parent, "one": one,
            "variant": [self.read(variant + 4 * i) for i in range(12)], "variant_at": variant,
            "ready": self.read(STATE + S_PENDING)})
        self.returns(0, eax=len(self.queries))

    def instance(self):
        return self.read(self.emu.reg_read(UC_X86_REG_EAX))

    def add(self):
        inst = self.instance()
        renderer, scene, t1, t2 = (self.arg(i) for i in range(4))
        assert renderer == RENDERER and self.read(scene) == SCENE and t1 == t2
        assert self.emu.mem_read(inst + 0x10D, 1)[0] == 0, "add_model: not in the scene yet"
        self.emu.mem_write(inst + 0x10D, b"\x01")
        self.scene[inst] = self.floats(t1)
        self.returns(16)

    def remove(self):
        inst = self.instance()
        renderer, scene = self.arg(0), self.arg(1)
        assert renderer == RENDERER and self.read(scene) == SCENE
        assert self.emu.mem_read(inst + 0x10D, 1)[0] == 1, "remove_model: in the scene"
        self.emu.mem_write(inst + 0x10D, b"\x00")
        del self.scene[inst]
        self.returns(8)

    def update(self):
        inst = self.instance()
        renderer, scene, t1, t2 = (self.arg(i) for i in range(4))
        assert renderer == RENDERER and self.read(scene) == SCENE and t1 == t2
        assert self.emu.mem_read(inst + 0x10D, 1)[0] == 1, "update_model: in the scene"
        self.scene[inst] = self.floats(t1)
        self.updates.append(inst)
        self.returns(16)

    def release(self):
        resource = self.emu.reg_read(UC_X86_REG_ECX)
        assert self.emu.reg_read(UC_X86_REG_EAX) == resource + 0xD0 and self.read(resource + 0xD0) == 0
        self.released.append(resource - 0x10)
        self.live.discard(resource - 0x10)
        self.returns(0)

    def mt_draw(self):
        self.draws.append((self.emu.reg_read(UC_X86_REG_ECX), self.arg(-1)))
        self.returns(0)

    def unload_next(self):
        self.unloads += 1
        self.returns(0)

    def log(self):
        text = bytes(self.emu.mem_read(self.arg(8), 120)).split(b"\0")[0].decode()
        args = [self.arg(9), self.arg(10), self.arg(11)]
        fields = iter(args)
        parts = text.split("%")
        line = parts[0]
        for part in parts[1:]:
            value = next(fields)
            line += (str(value) if part[0] == "u" else
                     bytes(self.emu.mem_read(value, 40)).split(b"\0")[0].decode()) + part[1:]
        self.lines.append(line)
        self.returns(0)

    def setup_body(self):
        """setup_profile's body: the character made again. ebx = the
        character (the site's `mov ebx,[ebp+8]`), [esp] the ebx pushed."""
        emu = self.emu
        ebp, sp = emu.reg_read(UC_X86_REG_EBP), emu.reg_read(UC_X86_REG_ESP)
        assert emu.reg_read(UC_X86_REG_EBX) == self.read(ebp + 8)
        assert sp == (ebp & ~7) - 0x70, "the three instructions' stack effect"
        self.bodies.append((emu.reg_read(UC_X86_REG_EAX), self.read(ebp + 8), self.read(ebp + 12) & 0xFF))
        emu.reg_write(UC_X86_REG_EBX, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, ebp + 4)
        emu.reg_write(UC_X86_REG_EBP, self.read(ebp))
        self.returns(8)

    def client_data(self):
        assert self.emu.reg_read(UC_X86_REG_ECX) == CLIENT
        self.returns(0, eax=CLIENT_DATA)

    # runs
    def run(self, start, until, stack, registers, count=400000):
        emu = self.emu
        for i, value in enumerate(stack):
            self.word(STACK + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        for register, value in registers.items():
            emu.reg_write(register, value)
        emu.emu_start(start, until, count=count)
        assert emu.reg_read(UC_X86_REG_EIP) == until, hex(emu.reg_read(UC_X86_REG_EIP))

    def setup(self, character=CHAR, kit=0, profile=PROFILE):
        """setup_profile(profile, character, kit) through the build's entry."""
        sentinels = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EDI: 0x44444444,
                     UC_X86_REG_EBP: 0x22222222}
        self.run(SETUP, STOP, [STOP, character, kit], {**sentinels, UC_X86_REG_EAX: profile})
        assert self.emu.reg_read(UC_X86_REG_ESP) == STACK + 12, "setup_profile pops its two arguments"
        assert all(self.emu.reg_read(r) == v for r, v in sentinels.items()), "callee-saved registers"

    def frame(self, character=CHAR):
        """lobby_character::update from its call of mt_draw to its `ret 0Ch`."""
        frame = [0x44444444, 0x33333333, 0x11111111] + [0] * (0x44 // 4) + [STOP, 0, 0, 0]
        self.emu.mem_write(character + 0xD514, b"\x00")
        self.run(UPDATE_CALL, STOP, frame, {UC_X86_REG_EBX: character, UC_X86_REG_ECX: PLAYER})

    def unload(self):
        self.run(UNLOAD_CALL, UNLOAD_AFTER, [0], {UC_X86_REG_EBP: MENU, UC_X86_REG_ESI: 0x55555555})
        assert self.emu.reg_read(UC_X86_REG_EBP) == MENU and self.emu.reg_read(UC_X86_REG_ESI) == 0x55555555

    def answer(self, weapon):
        """The resource system calls the last query's function with a
        queries_result holding `weapon` (0: none), then lets its reference go."""
        q = self.queries[-1]
        manager = struct.unpack_from("<I", q["function"], 0)[0]
        assert manager & 1 and manager & ~1 == MEMBER_MANAGER
        invoker = self.read((manager & ~1) + 4)
        assert invoker == INVOKER
        function = STACK - 0x100
        self.emu.mem_write(function, q["function"])
        self.emu.mem_write(RESULT, bytes(0x400))
        self.word(RESULT + 0x12C, weapon + 0x10 if weapon else 0)   # the resource: the weapon's +10h
        self.run(invoker, STOP, [STOP, function + 8, RESULT], {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: 0x33,
                                                               UC_X86_REG_EDI: 0x44, UC_X86_REG_EBP: 0x22})
        assert self.emu.reg_read(UC_X86_REG_ESP) == STACK + 4, "the invoker is cdecl, the callback `ret 4`"
        assert (self.emu.reg_read(UC_X86_REG_EBX), self.emu.reg_read(UC_X86_REG_ESI),
                self.emu.reg_read(UC_X86_REG_EDI), self.emu.reg_read(UC_X86_REG_EBP)) == (0x11111111, 0x33, 0x44, 0x22)
        if weapon:
            refs = self.read(weapon + 0x10 + 0xD0) - 1
            self.word(weapon + 0x10 + 0xD0, refs)
            if refs == 0:
                self.released.append(weapon)
                self.live.discard(weapon)

    def window(self, item, opened=1):
        """Patch 32's state as its start leaves it for a weapon a kit holds
        (mode 1, the item) and patch 45's table on; the character's window."""
        self.emu.mem_write(PREVIEW, bytes([1 if opened else 0]))
        self.word(PREVIEW + 4, item)
        self.word(TABLE_STATE, 1 if opened else 0)
        self.emu.mem_write(CHAR + 0xD515, bytes([opened]))

    def open(self, item, kit):
        """A weapon's window: patch 32's show_kit, setup_profile(account,
        character, chosen kit 0), which setup_hook gives the weapon's kit."""
        self.window(item)
        self.setup(kit=0)

    def close(self):
        self.window(self.read(PREVIEW + 4), opened=0)
        self.setup(kit=0)

    def state(self, field):
        return self.read(STATE + field)

    def tick(self, ms=16):
        self.clock += ms
        self.word(GAME + 0x584C, self.clock)


def expected_matrix(lobby, weapon, dict_id, spin=0.0, tilt=0.0):
    ca, sa, cb, sb = math.cos(spin), math.sin(spin), math.cos(tilt), math.sin(tilt)
    rows = [ca * cb, ca * sb, sa, 0, -sb, cb, 0, 0, -sa * cb, -sa * sb, ca, 0]
    if dict_id == 216:
        mid = MIDDLES_216
    elif BOXES[dict_id] is None:
        mid = (0.0, 0.0, 0.2)
    else:
        lo, hi = BOXES[dict_id]
        mid = (0.0, 0.5 * (lo[1] + hi[1]), -0.5 * (lo[2] + hi[2]))
    moved = [sum(mid[k] * rows[k * 4 + c] for k in range(3)) for c in range(3)]
    return rows + [BENCH[c] - moved[c] for c in range(3)] + [1.0], mid


def close_to(a, b, eps=2e-5):
    return all(abs(x - y) <= eps for x, y in zip(a, b))


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    plain, _ = base.build(original, "section", code, None, late_join, fast_weapon=False)
    checks = 0

    def check(ok, text):
        nonlocal checks
        if not ok:
            print(f"  FAIL  {text}")
            raise SystemExit(1)
        checks += 1
        print(f"  ok    {text}")

    def inside(address):
        return FAST <= address < FAST + 0x1000

    # 1. Creation.
    lobby = Lobby(patched)
    lobby.window(SVD)
    before = bytes(lobby.emu.mem_read(CHAR, 0xD558))
    lobby.counting = [0]
    lobby.setup(kit=0)
    asked = lobby.counting[0]
    lobby.counting = None
    q = lobby.queries[-1] if lobby.queries else None
    record = lobby.item_record(SVD)
    v = q["variant"] if q else []
    type_id = lobby.read(0x2260B2C)
    check(q is not None and not lobby.bodies and bytes(lobby.emu.mem_read(CHAR, 0xD558)) == before,
          "creation: the SVD's window (kit 1, the character shows kit 0) asks for one query and setup_profile "
          "returns without its body: the character's bytes are as they were")
    check(q["path"] == WEAPON_PATH and q["class"] == 0x46 and q["count"] == 1 and q["parent"] == 0
          and q["one"] == 1 and q["allocator"] == lobby.read(0x1F1D9C4) and q["ready"] == 1,
          "the query: one request of class 46h, \"weapon\" (0xA04840), no parent, the game's allocator, the "
          "state's PENDING set before query_resources is called")
    check(v[0] == WEAPON_HELPER and v[2] == STATE + S_REC + 0x14 and v[3] == STATE + S_REC + 0x60
          and v[4] == DICTIONARY and v[5] == MENU and v[6] == DICT[SVD] and v[7] == CLIENT_WORD and v[8] == 1
          and v[10] == q["variant_at"] and v[11] == type_id and type_id != 0,
          "its data as the inventory's (0x2C7340..0x2C73D5): the helper 0xA0E7D0, the state's record's two "
          "vectors, the dictionary, the lobby menu (the weapon's world), the dict id, the network client's word "
          "(vtable+34h, +577B0h), 1; the variant's self pointer and type (0x2C8090's)")
    rec = bytes(lobby.emu.mem_read(STATE + S_REC, ITEM_SIZE))
    check(struct.unpack_from("<IH", rec, 0) == (SVD, DICT[SVD])
          and struct.unpack_from("<III", rec, 0x14) == (STATE + S_REC + 0x20,) * 2 + (STATE + S_REC + 0x60,)
          and struct.unpack_from("<III", rec, 0x60) == (STATE + S_REC + 0x6C,) * 2 + (STATE + S_REC + 0xB4,),
          "the record is copied into the state by the game's copy-assign (0x270390), its vectors pointing into "
          "the copy")
    # Before it comes: the character as it is.
    lobby.frame()
    check(lobby.draws[-1][0] == PLAYER and inside(lobby.draws[-1][1]) and not lobby.scene
          and lobby.read(TABLE_STATE + T_READY) == 0,
          "before the weapon comes: mt_draw called from the patch (patch 45's is_table false: the held item in "
          "the hand), nothing in the scene, patch 45's READY 0")
    # It comes.
    svd = lobby.weapon(SVD, scope_bone=2)
    lobby.counting = [0]
    lobby.answer(svd)
    came = lobby.counting[0]
    lobby.counting = None
    check(lobby.state(S_HAVE) == svd and lobby.read(svd + 0xE0) == 1 and not lobby.released
          and lobby.state(S_PENDING) == 0,
          "driven: ready through the game's member callback 0x27D280 and invoker 0x202300: the weapon kept, one "
          "reference ours once the result lets its own go")
    lobby.tick(30)
    lobby.counting = [0]
    lobby.frame()
    drawn = lobby.counting[0]
    lobby.counting = None
    inst, sinst = svd + 0x3000, svd + 0xE000
    want, mid = expected_matrix(lobby, svd, DICT[SVD])
    got = lobby.scene.get(inst)
    check(got is not None and close_to(got, want) and close_to(lobby.floats(STATE + S_MATRIX), want)
          and lobby.updates.count(inst) == 1,
          f"placement: the SVD's model in the scene once, at the bench: the box's middle {tuple(round(x, 4) for x in mid)} "
          f"(x 0) at {BENCH}, rows rotY(0) * rotZ(0)")
    bind = inverse_rigid(lobby.binds[2])
    scope_want = affine(affine(affine(affine([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], rot_y(math.pi)),
                                             lobby.locator), bind), want)
    check(close_to(lobby.scene.get(sinst, []), scope_want, 2e-4),
          "the scope's model in the scene at its locator through the weapon's transform, the locator's bone 2 in "
          "the bind pose (the inverse of its inverse bind): weapon::calculate_locator's (0x243BC0) result")
    check(lobby.read(TABLE_STATE + T_READY) == 1 and close_to(lobby.floats(TABLE_STATE + T_MATRIX), want)
          and lobby.read(TABLE_STATE + T_STAMP) == 1,
          "patch 45's READY 1, its interface's transform the weapon's, its stamp 1")
    check(lobby.lines == [f"srp: fast weapon: item {SVD} at the bench 30 ms after the window asked (made)"]
          and inside(lobby.draws[-1][1]),
          f"one log line: \"{lobby.lines[0] if lobby.lines else ''}\"; mt_draw from the patch")
    # Turned.
    lobby.set_floats(TABLE_STATE + T_SPIN, [0.7, -0.4])
    lobby.frame()
    want, _ = expected_matrix(lobby, svd, DICT[SVD], 0.7, -0.4)
    m = lobby.scene[inst]
    middle = [sum(mid[k] * m[k * 4 + c] for k in range(3)) + m[12 + c] for c in range(3)]
    check(close_to(m, want) and close_to(middle, BENCH) and lobby.updates.count(inst) == 2 and len(lobby.lines) == 1,
          "a drag's spin 0.7 and tilt -0.4: moved (update_model each frame), the middle still at the bench")
    lobby.set_floats(TABLE_STATE + T_SPIN, [0.0, 0.0])
    # Close.
    lobby.close()
    lobby.frame()
    check(not lobby.scene and lobby.state(S_HAVE) == svd and lobby.read(svd + 0xE0) == 1 and not lobby.released
          and lobby.bodies == [(PROFILE, CHAR, 0)] and lobby.draws[-1][1] == UPDATE_AFTER
          and lobby.state(S_FAST) == 0,
          "close: both models out of the scene, the weapon kept; setup_profile's body runs (the chosen kit, its "
          "early return as before); mt_draw the update's own call again (return 0x284A83)")
    # Reopen: kept, at once.
    queries = len(lobby.queries)
    lobby.tick(500)
    lobby.counting = [0]
    lobby.open(SVD, 1)
    lobby.frame()
    kept = lobby.counting[0]
    lobby.counting = None
    check(len(lobby.queries) == queries and inst in lobby.scene and sinst in lobby.scene
          and lobby.lines[-1] == f"srp: fast weapon: item {SVD} at the bench 0 ms after the window asked (kept)",
          f"reopen of the same record: no query, at the bench at the next frame: \"{lobby.lines[-1]}\"")
    # Another weapon (a window over a window).
    lobby.open(VEPR, 2)
    lobby.frame()
    check(len(lobby.queries) == queries + 1 and not lobby.scene and lobby.state(S_HAVE) == svd,
          "the Vepr's window over it: a query; the SVD out of the scene (another item) and still kept")
    vepr = lobby.weapon(VEPR)
    lobby.tick(40)
    lobby.answer(vepr)
    lobby.frame()
    want, mid = expected_matrix(lobby, vepr, 216)
    check(lobby.released == [svd] and lobby.state(S_HAVE) == vepr and close_to(lobby.scene.get(vepr + 0x3000, []), want)
          and len(lobby.scene) == 1,
          f"the Vepr comes: the SVD let go (its last reference, 0xE0210), the Vepr at the bench by patch 45's "
          f"MIDDLES row (216: {mid}), no scope")
    # An attach: a changed record; the one shown stays until the new one comes.
    lobby.record(lobby.item_record(VEPR), VEPR, [(9001, 5001, 0, 2)])
    lobby.setup(kit=0)
    lobby.frame()
    check(len(lobby.queries) == queries + 2 and vepr + 0x3000 in lobby.scene and lobby.state(S_PENDING) == 1,
          "an attach (the Vepr's record with a camouflage in slot 2): a query; the Vepr shown stays at the bench")
    # A second change while the first is in flight.
    lobby.record(lobby.item_record(VEPR), VEPR, [(9001, 5001, 0, 2), (9002, 5002, 7, 3)])
    lobby.setup(kit=0)
    first = lobby.weapon(VEPR)
    lobby.answer(first)
    check(len(lobby.queries) == queries + 3 and first in lobby.released and lobby.state(S_HAVE) == vepr
          and lobby.state(S_PENDING) == 1 and lobby.state(S_WANT) == 0,
          "a second change while the first is in flight: it waits; the first comes and is let go (by the result), "
          "the second is asked then; the Vepr still shown")
    second = lobby.weapon(VEPR)
    lobby.answer(second)
    lobby.frame()
    check(vepr in lobby.released and lobby.state(S_HAVE) == second and second + 0x3000 in lobby.scene
          and len(lobby.scene) == 1 and lobby.read(second + 0xE0) == 1,
          "the second comes: the old Vepr out of the scene and let go, the new one at the bench")
    # The same record asked again while shown: nothing.
    lobby.setup(kit=0)
    lobby.frame()
    check(len(lobby.queries) == queries + 3 and second + 0x3000 in lobby.scene,
          "the lobby's refresh with the same record (the account's answer): no query, the same weapon shown")
    # Squad mate and armour.
    bodies = len(lobby.bodies)
    lobby.setup(character=SQUAD, kit=2)
    lobby.frame(character=SQUAD)
    check(lobby.bodies[-1] == (PROFILE, SQUAD, 2) and lobby.draws[-1] == (PLAYER, UPDATE_AFTER),
          "a squad mate's character: setup_profile's body as before; its update's mt_draw the original's call")
    lobby.window(VEST)
    lobby.emu.mem_write(PREVIEW, b"\x00")          # armour: patch 32's mode 0
    lobby.setup(kit=0)
    lobby.frame()
    check(len(lobby.bodies) == bodies + 2 and not lobby.scene and lobby.draws[-1][1] == UPDATE_AFTER,
          "an armour window over the weapon's (mode 0): setup_profile's body (the stock way), the weapon out of "
          "the scene, mt_draw the update's call")
    # Unload while shown, and while a query is in flight.
    lobby.open(VEPR, 2)
    lobby.frame()
    lobby.unload()
    check(lobby.unloads == 1 and second in lobby.released and lobby.state(S_HAVE) == 0 and not lobby.scene,
          "the lobby menu's unload: the weapon out of the scene and let go, then the unload's own call")
    lobby.open(AK, 0)
    pending = lobby.weapon(AK)
    lobby.unload()
    lobby.answer(pending)
    check(pending in lobby.released and lobby.state(S_HAVE) == 0,
          "the unload with a query in flight: the weapon that comes after is let go (by the result)")
    # Without the patch: the original's way.
    other = Lobby(plain)
    other.open(SVD, 1)
    other.frame()
    check(other.bodies == [(PROFILE, CHAR, 1)] and not other.queries and other.draws[-1][1] == UPDATE_AFTER,
          "the build without it (--no-fast-weapon): setup_profile's body with the SVD's kit (patch 32's way), "
          "mt_draw the update's call")

    # 2. No leak: 100 opens and closes.
    lobby = Lobby(patched)
    alive_max, weapons = 0, {}
    plan = [SVD, SVD, VEPR, AK, SVD, AK, AK, VEPR]
    for n in range(100):
        item = plan[n % len(plan)]
        kit = {AK: 0, SVD: 1, VEPR: 2}[item]
        if n % 9 == 4:                             # an attach now and then
            lobby.record(lobby.item_record(item), item, [(9100 + n, 5100, n, 2)])
        lobby.open(item, kit)
        lobby.frame()
        if lobby.state(S_PENDING):
            made = lobby.weapon(item, scope_bone=1 if item == SVD else None, number=n)
            lobby.answer(made)
        lobby.frame()
        lobby.frame()
        assert lobby.state(S_HAVE) and lobby.read(lobby.state(S_HAVE) + 0x3000 + 0x10D), n
        alive_max = max(alive_max, len(lobby.live))
        lobby.close()
        lobby.frame()
        assert not lobby.scene, n
    check(alive_max <= 2 and lobby.live == {lobby.state(S_HAVE)} and len(lobby.released) == len(lobby.made) - 1
          and lobby.read(lobby.state(S_HAVE) + 0xE0) == 1,
          f"no leak: 100 opens and closes over three weapons with attaches: {len(lobby.made)} weapons made, "
          f"{len(lobby.released)} let go (each once, at its last reference), at most {alive_max} alive at a time, "
          "1 kept after the last close (its one reference ours)")
    check(lobby.state(S_QUERIES) == len(lobby.made) and not lobby.scene and lobby.state(S_PENDING) == 0
          and base.ARENA_BLOCKS["fastweapon.state"].size >= 0x340,
          f"every close leaves the scene empty; {lobby.state(S_QUERIES)} queries for {len(lobby.made)} weapons (a "
          "record seen before and kept needs none); the state is its fixed 340h bytes, nothing allocated")

    # 3. The time: the main thread's instructions (the loading itself runs
    # on the resource threads and is not emulated: the game's log line says).
    old = Lobby(plain, body=True, measure=True)
    old.window(SVD)
    old.counting = [0]
    old.setup(kit=0)
    old_open = old.counting[0]
    new = Lobby(patched, measure=True)
    new.window(SVD)
    new.counting = [0]
    new.setup(kit=0)
    new_open = new.counting[0]
    new.answer(new.weapon(SVD, scope_bone=2))
    new.counting = [0]
    new.frame()
    new_frame = new.counting[0]
    new.close()
    new.window(SVD)
    new.counting = [0]
    new.setup(kit=0)
    new.frame()
    new_kept = new.counting[0]
    check(old.queries and old.queries[-1]["class"] == 0x54 and new.queries[-1]["class"] == 0x46,
          f"the window's ask on the main thread, every instruction counted: the build without it "
          f"{old_open} (setup_profile's body: the profile copied, a player query of class 54h), with it "
          f"{new_open} (the record copied, a weapon query of class 46h); the first frame at the bench "
          f"{new_frame} (mt_draw a stub); a reopen of the kept weapon, ask and frame, {new_kept}")
    print(f"  info  instructions of the patch's own code: the window's ask {asked}, the callback {came}, the first "
          f"frame at the bench {drawn}, a reopen of the kept weapon {kept}")
    print(f"{checks} checks passed")


if __name__ == "__main__":
    main()
