#!/usr/bin/env python3
"""Run the original's and patch 32's lobby code for a weapon's upgrade
window: the movie's start_item_upgrade and exit_item_upgrade handlers
(0x29A870 -> 0x265DE0 and 0x29A890 -> 0x265F10, with the lobby camera's
own setter 0x2845C0), lobby_character::setup_profile's entry (0x284B10)
and the three blocks of the lobby camera's tick that clamp the distance
(0x284020, 0x284222, 0x28430E), on a made-up lobby: a character, its menu,
an account profile with three kits, a camera.

    python check_weapon_preview_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). Each image is loaded as is and again
1 MiB higher with its base relocations applied (a rebased image). The
decal markers' placement (0x265F80), the scene's and the input's virtual
calls and setup_profile's body (from 0x284B16 on: it copies the profile
and queries the character's resources) are stubs that write down how they
were called. What must hold:
- a window whose item is no kit's held weapon (armour, legs armour, a
  weapon only in the stash, a weapon 2 behind a weapon 1, an item index
  past the profile's table, a kit past its count) on this made-up lobby,
  whose profile has no inventory behind it (weapon_stash_026g.asm, which
  shows such a weapon from the inventory, has check_weapon_stash_026g.py):
  the build leaves the
  camera, the character, the globals and the stubs' calls exactly as the
  original does, at the start and at the close;
- a kit's held weapon (weapon 1, or weapon 2 without one; in the chosen kit
  or another): setup_profile is called with that kit, the camera gets the
  build's height and distance (patch 45's, the workbench's; the side angle
  stays), and the close calls
  setup_profile with the chosen kit and keeps the original's camera values
  but a distance under 1.5;
- setup_profile's hook gives the weapon's kit only while one is shown and
  only to a character with its upgrade window open, keeps every register
  and the stack, and with nothing shown changes nothing;
- the tick's three clamps are the original's (values and flags) while no
  weapon's window was open, and let the distance down to NEAR_MIN from one
  until the camera is back at 1.5;
- the character's update gives its player the window's mode (+F49Ch, which
  draws it for the decal editor: bind pose, the weapon put away) as the
  original does, and 0 while a weapon is shown, keeping every register.
Says nothing about the movie or what the game draws.
"""
import itertools
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_EFLAGS, \
    UC_X86_REG_XMM0, UC_X86_REG_XMM1, UC_X86_REG_XMM2, UC_X86_REG_XMM3, UC_X86_REG_XMM4

import patch_026g as base

START_HANDLER, EXIT_HANDLER = 0x29A870, 0x29A890
DECALS = 0x265F80                              # (character, item), `ret 8`
SETUP, SETUP_BODY = 0x284B10, 0x284B16
SAVED_DISTANCE = 0x1F1D9E4                     # where the start keeps the distance it replaces
GLOBALS = ((0x1F1D9E4, 4), (0x1F1DA26, 2), (0x1F1DA40, 4), (0xA7029C, 0x14))

UI, CHARACTER, SQUAD, MENU, PROFILE = 0x20000000, 0x20100000, 0x20200000, 0x20300000, 0x20400000
CAMERA, OWNER, FOCUS, SCENE, ARGS = 0x20500000, 0x20500400, 0x20500800, 0x20500C00, 0x20501000
STUBS, STACK, STOP = 0x20600000, 0x20708000, 0x20800000
PLAYER = 0x20900000                            # the lobby's player (+F49Ch: drawn for the decal editor)
SENTINELS = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
             UC_X86_REG_EDI: 0x44444444}

KIT0, KIT_SIZE, SLOTS, TABLE, ITEM_SIZE = 0x58, 0x704, 0x40, 0x1568, 0xB4
WEAPON_1, WEAPON_2, LEGS, VEST_SLOT = 7, 10, 4, 2

# Items: (id, index in the profile's table)
AK, SVD, PISTOL, PISTOL2, VEST, LEGS_ITEM, STASH_RIFLE, STALE = 501, 502, 503, 504, 505, 506, 507, 508


def f32(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def from_f32(word):
    return struct.unpack("<f", struct.pack("<I", word & 0xFFFFFFFF))[0]


EMULATORS, CURRENT = {}, {}


class Lobby:
    def __init__(self, data, delta, kits, chosen=0, count=3, items=None):
        """kits: [{slot: item id}] for the profile's kits; items: the
        profile's table (item ids), by default every item a kit names. One
        emulator per image and base, made again for each lobby: the made-up
        objects, the globals the handlers write and the build's state."""
        self.delta = delta
        key = (id(data), delta)
        if key not in EMULATORS:
            EMULATORS[key] = self.load(data, delta)
        self.emu, self.stubs, pristine = EMULATORS[key]
        CURRENT[key] = self
        emu = self.emu
        for at in (UI, CHARACTER, SQUAD, MENU, PROFILE, CAMERA, STUBS, STACK - 0x8000, STOP, PLAYER):
            emu.mem_write(at, bytes(0x10000))
        for at, value in pristine.items():
            emu.mem_write(at, value)
        self.calls = []
        self.fill_lobby(kits, chosen, count, items)

    def load(self, data, delta):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva + delta
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        if delta:
            reloc_rva, reloc_size = struct.unpack_from("<II", data, headers.reloc_at)
            table = next(raw + reloc_rva - rva for _, rva, vsize, raw, rsize, _ in headers.sections(data)
                         if rva <= reloc_rva < rva + vsize)
            at = table
            while at < table + reloc_size:
                page, block = struct.unpack_from("<II", data, at)
                if block < 8:
                    break
                for i in range((block - 8) // 2):
                    entry = struct.unpack_from("<H", data, at + 8 + 2 * i)[0]
                    if entry >> 12 == 3:
                        where = base.IMAGE_BASE + page + (entry & 0xFFF) + delta
                        self.word(where, self.read(where) + delta)
                at += block
        for at in (UI, CHARACTER, SQUAD, MENU, PROFILE, CAMERA, STUBS, STACK - 0x8000, STOP, PLAYER):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        names = ("owner_focus", "focus", "unfocus", "scene_begin", "scene_end")
        stubs = {STUBS + 0x10 * i: name for i, name in enumerate(names)}
        stubs[DECALS + delta] = "decals"
        stubs[SETUP_BODY + delta] = "setup"
        if self.read(SETUP + delta) & 0xFF == 0x55:     # the original's entry; the build's is a jump
            stubs[SETUP + delta] = "setup_original"
        key = (id(data), delta)
        for at in stubs:
            emu.hook_add(UC_HOOK_CODE, lambda e, a, z, u: CURRENT[key].stub(e, a, z, u), begin=at, end=at)
        pristine = {at + delta: bytes(emu.mem_read(at + delta, size)) for at, size in GLOBALS}
        state = base.late_join_state_va(open(base.late_join_bin(), "rb").read()) + base.WEAPON_PREVIEW_STATE_OFFSET
        if any(name == base.LATE_JOIN_STATE_NAME for name, *_ in base.Headers(data).sections(data)):
            pristine[state + delta] = bytes(base.WEAPON_PREVIEW_STATE_SIZE)
            table = state - base.WEAPON_PREVIEW_STATE_OFFSET + base.WEAPON_TABLE_STATE_OFFSET
            pristine[table + delta] = bytes(base.WEAPON_TABLE_STATE_SIZE)
        return emu, stubs, pristine

    def fill_lobby(self, kits, chosen, count, items):
        emu = self.emu
        for at, value in ((UI + 0x694, CHARACTER), (CHARACTER + 0xD2BC, MENU), (CHARACTER + 0xD2C0, SCENE),
                          (CHARACTER + 0xD504, 0x5EC0), (SQUAD + 0xD2BC, MENU),
                          (MENU + 0x6A0, PROFILE), (MENU + 0x10C, CAMERA), (MENU + 0x9C, OWNER),
                          (OWNER, OWNER + 0x100), (OWNER + 0x100 + 0x38, STUBS),
                          (FOCUS, FOCUS + 0x100), (FOCUS + 0x100 + 0x10, STUBS + 0x10),
                          (FOCUS + 0x100 + 0x14, STUBS + 0x20),
                          (SCENE, SCENE + 0x100), (SCENE + 0x100 + 0x2C, STUBS + 0x30),
                          (SCENE + 0x100 + 0x38, STUBS + 0x40)):
            self.word(at, value)
        # The camera as its constructor (0x283BC0) leaves it and a tick has
        # moved it: height 1.2, distance 1.96, side angle 0.
        for offset, value in ((0x98, 0.0), (0xAC, 0.0), (0x9C, 1.2), (0xA4, 1.2), (0xC0, 1.96), (0xC4, 1.96)):
            self.word(CAMERA + offset, f32(value))
        # The profile: its table and its kits; the character shows buffer 0
        # (a copy of it) with the chosen kit.
        if items is None:
            items = sorted({item for kit in kits for item in kit.values()})
        self.items = items
        emu.mem_write(PROFILE + 0x57, bytes([count]))
        emu.mem_write(PROFILE + 0x1564, bytes([len(items)]))
        emu.mem_write(PROFILE + 0x4632, bytes([chosen]))
        for k in range(3):
            for slot in range(23):
                emu.mem_write(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot, b"\xff\xff\x00\x00")
        for index, item in enumerate(items):
            self.word(PROFILE + TABLE + ITEM_SIZE * index, item)
        for k, kit in enumerate(kits):
            for slot, item in kit.items():
                index = items.index(item) if item in items else STALE_INDEX
                emu.mem_write(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot, struct.pack("<HH", index, 1))
        emu.mem_write(CHARACTER + 0x18, bytes(emu.mem_read(PROFILE, 0x4634)))
        emu.mem_write(CHARACTER + 0xD2B5, bytes([chosen]))

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def returns(self, pop):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        name = self.stubs.get(address)
        if name is None:
            return
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        ecx = emu.reg_read(UC_X86_REG_ECX)
        # What a call may change: make sure nothing counts on it.
        for register in (UC_X86_REG_ECX, UC_X86_REG_EDX):
            emu.reg_write(register, 0xBADC0DE0 + register)
        for register in (UC_X86_REG_XMM0, UC_X86_REG_XMM1, UC_X86_REG_XMM2, UC_X86_REG_XMM3, UC_X86_REG_XMM4):
            emu.reg_write(register, 0x7FC00BAD7FC00BAD7FC00BAD7FC00BAD)
        if name == "owner_focus":                                # thiscall, no argument
            assert ecx == OWNER
            emu.reg_write(UC_X86_REG_EAX, FOCUS)
            self.returns(0)
        elif name in ("focus", "unfocus"):
            assert ecx == FOCUS
            self.calls.append((name, arg(0)))
            self.returns(4)
        elif name == "scene_begin":
            assert ecx == SCENE
            self.calls.append((name, arg(0), arg(1), arg(2)))
            self.returns(12)
        elif name == "scene_end":
            assert ecx == SCENE
            self.calls.append((name, arg(0), arg(1)))
            self.returns(8)
        elif name == "decals":
            self.calls.append((name, arg(0), arg(1)))
            self.returns(8)
        elif name == "setup":
            # After the build's setup_hook and the three instructions it
            # replaced: ebp is the frame, [ebp+8] the character, [ebp+0Ch]
            # the kit; eax the profile.
            ebp = emu.reg_read(UC_X86_REG_EBP)
            self.calls.append(("setup", emu.reg_read(UC_X86_REG_EAX), self.read(ebp + 8),
                               self.read(ebp + 12) & 0xFF))
            self.registers_at_setup = {r: emu.reg_read(r) for r in (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ESI,
                                                                    UC_X86_REG_EDI)}
            self.registers_at_setup[UC_X86_REG_ECX] = ecx
            assert sp & 7 == 0 and sp <= ebp, "the frame is aligned as the original's prologue leaves it"
            emu.reg_write(UC_X86_REG_ESP, ebp + 4)
            emu.reg_write(UC_X86_REG_EBP, self.read(ebp))
            self.returns(8)
        elif name == "setup_original":
            self.calls.append(("setup", emu.reg_read(UC_X86_REG_EAX), arg(0), arg(1) & 0xFF))
            self.returns(8)

    def call(self, entry, args, ecx=0, eax=0):
        emu = self.emu
        self.word(STACK, STOP)
        for index, value in enumerate(args):
            self.word(STACK + 4 + 4 * index, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, ecx)
        emu.reg_write(UC_X86_REG_EAX, eax)
        for register, value in SENTINELS.items():
            emu.reg_write(register, value)
        emu.emu_start(entry + self.delta, STOP, count=200000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 4 + 4 * len(args), \
            "it returns, taking its arguments off"
        assert all(emu.reg_read(register) == value for register, value in SENTINELS.items()), \
            "callee-saved registers"

    def handler(self, entry, item):
        """The movie's start_item_upgrade(item) or exit_item_upgrade(): the
        delegate's arguments, the number at +8 of the first value."""
        self.word(ARGS + 8, ARGS + 0x40)
        self.word(ARGS + 0x48, item)
        self.call(entry, [ARGS], ecx=UI)
        assert self.emu.reg_read(UC_X86_REG_EAX) & 0xFF == 1, "it returns true"

    def camera(self):
        """height, side angle, distance (targets), what the start kept."""
        return (round(from_f32(self.read(CAMERA + 0x9C)), 6), round(from_f32(self.read(CAMERA + 0x98)), 6),
                round(from_f32(self.read(CAMERA + 0xC0)), 6), round(from_f32(self.read(SAVED_DISTANCE + self.delta)), 6))

    def snapshot(self):
        """Everything the handlers may write: the camera, the character's
        fields, the globals, the calls."""
        out = [bytes(self.emu.mem_read(CAMERA, 0x100)), bytes(self.emu.mem_read(CHARACTER + 0xD2B0, 0x300)),
               bytes(self.emu.mem_read(CHARACTER, 0xD2B0))]
        out += [bytes(self.emu.mem_read(at + self.delta, size)) for at, size in GLOBALS]
        return out, list(self.calls)

    def state(self, data):
        """MODE, ITEM, NEAR of the build's state (none in the original)."""
        state = base.late_join_state_va(open(base.late_join_bin(), "rb").read()) + base.WEAPON_PREVIEW_STATE_OFFSET
        return tuple(self.read(state + self.delta + 4 * i) for i in range(3))


STALE_INDEX = 60                               # an index past the table's count


def kits_of(*kits):
    return [dict(kit) for kit in kits]


# The account: kit 0 the AK (weapon 1), a pistol (weapon 2), the vest and
# legs armour; kit 1 only a pistol (weapon 2); kit 2 the SVD.
KITS = kits_of({WEAPON_1: AK, WEAPON_2: PISTOL, VEST_SLOT: VEST, LEGS: LEGS_ITEM},
               {WEAPON_2: PISTOL2, VEST_SLOT: VEST},
               {WEAPON_1: SVD, VEST_SLOT: VEST})
# A weapon shown: the build's camera is patch 45's, which looks at the weapon
# above the workbench (weapon_table_026g.asm's bench_y and frame_view;
# check_weapon_table_026g.py runs it); patch 32's own were 1.05 and 0.85.
# patch 45's frame_view for a 1 m weapon (16:9 without a viewport; check_weapon_table_026g.py's
# frame()): the camera's distance and its side angle (the weapon left of the view's middle)
def _bench_view():
    big = math.tan(math.radians(65 / 2)) * 16 / 9
    s, p, m = 365 / 1920, 882 / 1920, 0.02
    t = 2 * s * big
    left = math.sqrt(1 / big ** 2 + (1 - 2 * m) ** 2) / (4 * (0.5 - s - m))
    right = math.sqrt(1 / big ** 2 + (1 - 2 * m - 2 * p) ** 2) / (4 * (0.5 - p - m + s))
    distance = max(left, right, 0.962) * math.sqrt(1 + t * t)
    c, sn = math.cos(-0.25), math.sin(-0.25)
    side = math.atan(t * c) + math.asin(t * sn * sn / (c * math.sqrt(1 + (t * c) ** 2)))
    return round(distance, 6), side


DISTANCE, SIDE_ANGLE = _bench_view()
HEIGHT, NEAR_MIN, LEAST = 1.3, 0.3, 1.5


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0

    def say(text):
        nonlocal checks
        checks += 1
        print(f"  ok    {text}")

    for delta in (0, 0x100000):
        rebased = " (rebased)" if delta else ""
        # Windows the build leaves to the original, start and close.
        cases = [("a vest", KITS, 0, 3, None, VEST), ("legs armour", KITS, 0, 3, None, LEGS_ITEM),
                 ("a rifle only in the stash", KITS, 0, 3, None, STASH_RIFLE),
                 ("a weapon 2 behind a weapon 1", KITS, 0, 3, None, PISTOL),
                 ("a weapon whose index is past the table", kits_of({WEAPON_1: STALE}), 0, 3, [AK], STALE),
                 ("a weapon of a kit past the count", KITS, 0, 2, None, SVD),
                 ("item 0", KITS, 0, 3, None, 0)]
        for name, kits, chosen, count, items, item in cases:
            runs = []
            for data in (original, patched):
                lobby = Lobby(data, delta, kits, chosen, count, items)
                lobby.handler(START_HANDLER, item)
                started, camera = lobby.snapshot(), lobby.camera()[:3]
                lobby.handler(EXIT_HANDLER, 0)
                runs.append((started, lobby.snapshot(), lobby, camera))
            (o_start, o_exit, o_lobby, o_camera), (p_start, p_exit, p_lobby, _) = runs
            assert o_start == p_start and o_exit == p_exit, name
            assert p_lobby.state(patched) == (0, item, 0), p_lobby.state(patched)
            assert not [c for c in p_lobby.calls if c[0] == "setup"]
            say(f"{name}{rebased}: start and close as the original (the camera's height, side angle and "
                f"distance {o_camera} at the start, {o_lobby.camera()[:3]} after the close; the character, the "
                f"globals and the calls of the decals, scene and input the same; setup_profile not called)")

        # A held weapon: the build shows its kit and looks at it.
        for name, item, kit in (("the AK, weapon 1 of the chosen kit", AK, 0),
                                ("the SVD, weapon 1 of kit 2", SVD, 2),
                                ("a pistol, weapon 2 of kit 1, which has no weapon 1", PISTOL2, 1)):
            lobby = Lobby(original, delta, KITS)
            lobby.handler(START_HANDLER, item)
            o_camera, o_calls = lobby.camera(), list(lobby.calls)
            lobby.handler(EXIT_HANDLER, 0)
            o_exit = lobby.camera()
            assert o_camera == (1.3, 0.37, 1.5, 1.96) and o_exit[:3] == (1.2, 0.0, 1.96), (o_camera, o_exit)
            lobby = Lobby(patched, delta, KITS)
            lobby.handler(START_HANDLER, item)
            camera = lobby.camera()
            # patch 45 frames the weapon itself: its side angle (round 5e)
            assert camera[0] == HEIGHT and abs(camera[1] - SIDE_ANGLE) < 2e-5 and camera[2:] == (DISTANCE, 1.96), camera
            assert lobby.calls == [("setup", PROFILE, CHARACTER, kit)] + o_calls, lobby.calls
            assert lobby.state(patched) == (1, item, 1)
            lobby.calls.clear()
            lobby.handler(EXIT_HANDLER, 0)
            assert lobby.camera()[:3] == (1.2, 0.0, 1.96), lobby.camera()
            assert lobby.calls[0] == ("setup", PROFILE, CHARACTER, 0), lobby.calls
            assert lobby.state(patched) == (0, item, 1)
            say(f"{name}{rebased}: the original looks at the chest ({o_camera[:3]}); the build calls "
                f"setup_profile with kit {kit} before the decals, looks at the weapon ({camera[:3]}), and at the "
                f"close calls it with the chosen kit 0 and aims back as the original ({lobby.camera()[:3]})")

        # A window opened over another: from a weapon to a vest the chosen
        # kit comes back and the camera is the original's.
        lobby = Lobby(patched, delta, KITS)
        lobby.handler(START_HANDLER, SVD)
        lobby.calls.clear()
        lobby.handler(START_HANDLER, VEST)
        assert lobby.calls[0] == ("setup", PROFILE, CHARACTER, 0) and lobby.camera()[:3] == (1.3, 0.37, 1.5)
        assert lobby.state(patched) == (0, VEST, 1)
        lobby.calls.clear()
        lobby.handler(EXIT_HANDLER, 0)
        assert not [c for c in lobby.calls if c[0] == "setup"]
        # The distance the second start kept was the weapon's (DISTANCE): the
        # close does not aim at less than 1.5.
        assert lobby.camera()[:3] == (1.2, 0.0, 1.5), lobby.camera()
        say(f"a vest's window over the SVD's{rebased}: the chosen kit back, the original's camera; the close "
            "keeps no distance under 1.5")

        # setup_profile's hook alone.
        for name, mode, opened, character, kit_in, kit_out in (
                ("nothing shown", 0, 1, CHARACTER, 1, 1),
                ("the SVD shown, the lobby's character", 1, 1, CHARACTER, 0, 2),
                ("the SVD shown, a squad mate's character (no window)", 1, 0, SQUAD, 0, 0),
                ("the SVD shown, the window closed", 1, 0, CHARACTER, 1, 1)):
            lobby = Lobby(patched, delta, KITS)
            state = base.late_join_state_va(late_join) + base.WEAPON_PREVIEW_STATE_OFFSET + delta
            lobby.word(state, mode)
            lobby.word(state + 4, SVD)
            lobby.emu.mem_write(character + 0xD515, bytes([opened]))
            lobby.calls.clear()
            lobby.call(SETUP, [character, kit_in], ecx=0xC0FFEE01, eax=PROFILE)
            assert lobby.calls == [("setup", PROFILE, character, kit_out)], lobby.calls
            regs = lobby.registers_at_setup
            assert regs[UC_X86_REG_EAX] == PROFILE and regs[UC_X86_REG_ECX] == 0xC0FFEE01 \
                and all(regs[r] == SENTINELS[r] for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI)), regs
            say(f"setup_profile's hook{rebased}, {name}: kit {kit_in} -> {kit_out}; eax, ecx, ebx, esi, edi and "
                "the frame as the original's prologue leaves them")

        # The tick's clamps: the original's blocks against the build's.
        blocks = ((0x284020, 0x2840C5, "the wheel"), (0x284222, 0x2842C6, "the step"),
                  (0x28430E, 0x28433B, "the last clamp"))
        values = (0.3, 0.6, 0.85, 1.2, 1.49, 1.5, 1.6, 1.96, 4.6, 5.0)
        wheel = (0.0, 0.2, -0.2, -1.0, 1.0)

        def tick(data, start, stop, target, current, delta_wheel, near, mode):
            lobby = Lobby(data, delta, KITS)
            ebx = CAMERA + 4
            for offset, value in ((0xBC, target), (0xC0, current), (0xC4, delta_wheel), (0xE4, 3.0),
                                  (0xC8, 0.0), (0xCC, 0.0), (0xB4, 0.0), (0xB8, 0.0)):
                lobby.word(ebx + offset, f32(value))
            state = base.late_join_state_va(late_join) + base.WEAPON_PREVIEW_STATE_OFFSET + delta
            if data is patched:
                lobby.word(state, mode)
                lobby.word(state + 8, near)
            emu = lobby.emu
            emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
            lobby.word(STACK - 0x100 + 0x10, f32(0.016))       # the frame's time, the step's [esp+10h]
            emu.reg_write(UC_X86_REG_EBX, ebx)
            emu.reg_write(UC_X86_REG_XMM4, f32(0.001))
            emu.reg_write(UC_X86_REG_XMM3, 0)
            emu.emu_start(start + delta, stop + delta, count=1000)
            assert emu.reg_read(UC_X86_REG_EIP) == stop + delta
            out = (bytes(emu.mem_read(ebx + 0xB4, 0x40)), emu.reg_read(UC_X86_REG_EFLAGS) & 0x8D5,
                   emu.reg_read(UC_X86_REG_XMM0), emu.reg_read(UC_X86_REG_XMM1))
            return out, (from_f32(lobby.read(ebx + 0xBC)), from_f32(lobby.read(ebx + 0xC0))), \
                (lobby.read(state + 8) if data is patched else None)

        for start, stop, name in blocks:
            same = 0
            for target, current, delta_wheel in itertools.product(values, values, wheel):
                o, o_values, _ = tick(original, start, stop, target, current, delta_wheel, 0, 0)
                p, p_values, near = tick(patched, start, stop, target, current, delta_wheel, 0, 0)
                assert o == p and near == 0, (name, target, current, delta_wheel)
                same += 1
            say(f"{name}{rebased}: the build's block gives the original's distances, registers and flags in all "
                f"{same} cases with no weapon's window")
        # From a weapon's window: down to NEAR_MIN, never below.
        _, (target, _), near = tick(patched, 0x284020, 0x2840C5, 0.85, 0.85, -1.0, 1, 1)
        assert math.isclose(target, NEAR_MIN, rel_tol=1e-6) and near == 1, target
        _, (_, current), near = tick(patched, 0x284222, 0x2842C6, 0.85, 1.5, 0.0, 1, 1)
        assert NEAR_MIN <= current < 1.5 and near == 1, current
        _, (_, current), near = tick(patched, 0x28430E, 0x28433B, 0.85, 0.3, 0.0, 1, 1)
        assert math.isclose(current, NEAR_MIN, rel_tol=1e-6) and near == 1, current
        _, (_, current), near = tick(patched, 0x28430E, 0x28433B, 0.85, 0.85, 0.0, 1, 1)
        assert math.isclose(current, 0.85, rel_tol=1e-6) and near == 1, current
        # Closed: the least stays NEAR_MIN while the camera comes back, and
        # is 1.5 again from 1.49 on.
        _, (_, current), near = tick(patched, 0x28430E, 0x28433B, 1.96, 1.2, 0.0, 1, 0)
        assert math.isclose(current, 1.2, rel_tol=1e-6) and near == 1, current
        _, (_, current), near = tick(patched, 0x28430E, 0x28433B, 1.96, 1.49, 0.0, 1, 0)
        assert math.isclose(current, 1.5, rel_tol=1e-6) and near == 0, current
        say(f"from a weapon's window{rebased}: the wheel goes down to {NEAR_MIN}, the step and the last clamp keep "
            f"0.85 and hold at {NEAR_MIN}; after the close the least is {NEAR_MIN} until the camera reaches 1.49, "
            "then 1.5 again")

        # The character's update gives its player the window's mode
        # (0x284A33, `mov [ecx+0F49Ch],al`); run from there to the next
        # instruction.
        for mode in (0, 1):
            for upgrade in (0, 1):
                got = []
                for data in (original, patched):
                    lobby = Lobby(data, delta, KITS)
                    state = base.late_join_state_va(late_join) + base.WEAPON_PREVIEW_STATE_OFFSET + delta
                    if data is patched:
                        lobby.word(state, mode)
                    emu = lobby.emu
                    lobby.emu.mem_write(PLAYER + 0xF49C, b"\x77")
                    regs = {UC_X86_REG_EAX: 0x12345600 | upgrade, UC_X86_REG_ECX: PLAYER, UC_X86_REG_EBX: CHARACTER,
                            UC_X86_REG_EDX: 0xD0D0D0D0, UC_X86_REG_ESI: 0x5E5E5E5E, UC_X86_REG_EDI: 0xD1D1D1D1,
                            UC_X86_REG_EBP: 0xB0B0B0B0}
                    for register, value in regs.items():
                        emu.reg_write(register, value)
                    emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
                    emu.emu_start(0x284A33 + delta, 0x284A39 + delta, count=100)
                    assert emu.reg_read(UC_X86_REG_EIP) == 0x284A39 + delta and \
                        emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
                    assert all(emu.reg_read(r) == v for r, v in regs.items()), "every register kept"
                    got.append(bytes(emu.mem_read(PLAYER + 0xF49C, 1))[0])
                expected = 0 if mode else upgrade
                assert got == [upgrade, expected], (mode, upgrade, got)
        say(f"the player's decal pose{rebased}: the original gives it the window's mode; the build the same with no "
            "weapon shown and 0 with one, every register kept")

    print(f"patch 32: {checks} checks passed")


if __name__ == "__main__":
    main()
