#!/usr/bin/env python3
"""Run upgrade_preview_026g.asm (patch 48: client_patch.md section 3.45) in
the build: its six hook sites as the game reaches them (the movie's
relocate_upgrade_item and drop_upgrade_item handlers before they send, the
0x87 reader's end, setup_profile past its early return, on_player_ready)
and patch 32's finds through setup_profile's entry, with the game's own
setup_profile (to the query), profile ctor and copy, inventory find,
binary-config lookups (0x0F6C00, 0x0F6B10) and the stash's and patch 32's
code, on a made-up account: a profile with three kits, its item table and
the inventory vector after it, a lobby character with its three copies,
the items dictionary (for a scope: variants whose config has
`srp_scope_fit`).
    python check_upgrade_preview_026g.py <original survarium.exe>
Requires Python `unicorn` (2.1.4) and `capstone` (the alignment probe,
align_probe_026g.py: every aligned-SSE operand our code runs, patch 68's
preview apply in Rust among it, is 16-byte aligned). Stubs that write down how they were
called: the dictionary's item_by_id (0x1F0090), the game's allocator
(0x420789, 0xE4F90), setup_profile's query (from 0x284BDF, after it copied
the profile into the character's next copy: the query is "made", nothing
waits (+D2C8h 0, as 0x284BEE) and setup_profile returns), the 0x87 reader's last call (0x270320), the
game's log (0x111ED0, 0x111860) and, for the armour shown worn,
can_move_item_to_slot (0x2CA910: a vest in slot 2). What must hold:
- a camouflage dropped on the window's weapon (a kit's, or one only in the
  stash): the character is made again at once from the scratch profile,
  whose weapon records have the camouflage as the weapon kit (patch 68), the
  other upgrades kept, a kit revision no server sends; the account's
  profile and inventory are not written; the handler's registers are the
  original's (edx / ecx loaded as before), the stack kept;
- the server's account (0x87) confirming it, while that player is still
  being made: no second player (setup_profile returns at `again`), and once
  made, a refresh returns at setup_profile's own early return;
- a refusal (the account without it): the character goes back to the
  account's (a revision of its own again, a waiting setup_profile);
- a camouflage in the slot of another: it replaces it; a scope beside two
  decals and a camouflage: all four in three records, the weapon kit
  (patch 68's weapon_kit_preview_apply through apply's slot); a take-off
  (drop_upgrade_item): the upgrade goes;
- a scope: its record holds the variant made for the weapon, found by
  `srp_scope_fit` (names, or dict ids); no variant: nothing previewed;
- not the window's item, no weapon shown, a character without its window:
  nothing previewed, nothing made;
- `again` for a squad mate's character and the 0x87 site with nothing
  queued: the original's; `made` writes one log line for the lobby's own
  character (the time since its query), the original's comparison after;
- a window closed at once (the owner's quick Esc, 2026-10-23): the movie's
  start_item_upgrade and exit_item_upgrade handlers (0x29A870, 0x29A890,
  the lobby camera's own setter and patch 45's camera; the input focus,
  the scene's two calls and the decal markers stubs), then the game's own
  on_player_ready (0x284F20; the players' virtual calls and
  set_effect_presenter 0x1F3740 stubs) for each player the queries make:
  for a rifle only in the stash and for armour the chosen kit does not
  wear, the close while the scratch's player is being made waits for it
  and on_player_ready makes the account's after it, so the character
  shows the chosen kit's own weapon (the build without `early` shows the
  window's rifle); `early` (0x284B7C) itself: the early return's flag as
  the original's but for the lobby's own character while its player is
  being made, every register kept.
Says nothing about what the game draws.
"""
import itertools
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_EFLAGS

import align_probe_026g as align
import patch_026g as base

ITEM_BY_ID, ALLOCATION_NAME, ALLOCATE = 0x1F0090, 0x420789, 0x0E4F90
QUERY_STUB, SETUP_DONE, SETUP = 0x284BDF, 0x284CF8, 0x284B10
ACCOUNT_ITEMS, LOG, LOG_FILTER = 0x270320, 0x111ED0, 0x111860
RELOCATE_SITE, RELOCATE_NEXT = 0x29A9B2, 0x29A9B8
DROP_SITE, DROP_NEXT = 0x29AACF, 0x29AAD5
ACCOUNT_SITE, ACCOUNT_NEXT = 0x270313, 0x270318
MADE_SITE, MADE_NEXT = 0x284F7F, 0x284F86
EARLY_SITE, EARLY_JZ = 0x284B7C, 0x284B83      # setup_profile's last early-return test, then `jz 284CF8h`
START_HANDLER, EXIT_HANDLER = 0x29A870, 0x29A890   # the movie's start_item_upgrade / exit_item_upgrade
PLAYER_READY = 0x284F20                        # lobby_character::on_player_ready (ecx, [esp+4] the query's result)
DECALS, EFFECT_PRESENTER = 0x265F80, 0x1F3740  # the decal markers (`ret 8`); player::set_effect_presenter (`ret`)
CAN_MOVE, VEST_SLOT = 0x2CA910, 2              # items_dictionary::can_move_item_to_slot (stdcall, `ret 0Ch`)
ITEM_CTOR = 0x2F7C10
GAME_ALLOCATOR, ALLOCATION_WHERE = 0x1F1D9C4, 0xAC1220
LOG_FILTERED = 0x1F1D60C
LOG_SINK, LOG_SINK_VALUE, LOG_MANAGER = 0x1F1D614, 0x5100C000, 0x9DAF4C

CLIENT = 0x20000000                            # the lobby client: the profile at +1E6344h
PROFILE = CLIENT + 0x1E6344
PLAYERS = 0x20400000                           # [lc+1E56C0h]: +53A4h the items dictionary
DICTIONARY = 0x20500000
ENTRIES = 0x20600000                           # dictionary entries, 3A0h each, by dict id
CONFIGS = 0x20680000                           # config nodes and names
INVENTORY = 0x20700000
SCRATCH = 0x20800000
CHARACTER, SQUAD, MENU, CLOCK = 0x20A00000, 0x20A10000, 0x20B00000, 0x20B10000
# The handlers' objects: the lobby camera (menu+10Ch), the input focus (the
# owner's, [menu+9Ch] = CLOCK, vtable+38h), the character's scene (+D2C0h),
# the movie's arguments, players made (+D0h their count), and the stubs the
# objects' vtables name
CAMERA, FOCUS, SCENE, ARGS = 0x20C00000, 0x20C01000, 0x20C02000, 0x20C03000
PLAYERS_MADE, VTABLES, STUB_CODE = 0x20C04000, 0x20C0A000, 0x20C0C000
STUB_NAMES = ("owner_focus", "focus", "unfocus", "scene_begin", "scene_end", "player_insert", "player_update",
              "player_remove", "player_detach")
STUB_AT = {name: STUB_CODE + 0x10 * i for i, name in enumerate(STUB_NAMES)}
STACK, STOP = 0x21000000, 0x21100000
ALLOCATOR = 0x5A110C00

KIT0, KIT_SIZE, SLOTS, ITEMS, TABLE, ITEM_SIZE = 0x58, 0x704, 0x40, 0x1564, 0x1568, 0xB4
REVISIONS, CHOSEN, KITS = 0x45FC, 0x4632, 0x57
WEAPON_1, WEAPON_2 = 7, 10
RIFLE, CAMO_CATEGORY, SCOPE_CATEGORY = 15, 25, 22
COPY_SIZE = 0x4634
KIT_WORD = 0xABCD0001                          # the profile's +4620h

# A weapon kit record's value (patch 68, check_weapon_kit_026g.py): its own
# logical slot, the dict ids it names or'ed in by the caller.
KIT = lambda own: 0xA7 << 56 | own << 48

SENTINELS = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
             UC_X86_REG_EDI: 0x44444444}


class Emu:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((CLIENT, 0x200000), (PLAYERS, 0x10000), (DICTIONARY, 0x10000), (ENTRIES, 0x80000),
                         (CONFIGS, 0x10000), (INVENTORY, 0x100000), (SCRATCH, 0x10000),
                         (STACK - 0x10000, 0x10000), (STOP, 0x1000), (CHARACTER, 0x10000), (SQUAD, 0x10000),
                         (MENU, 0x10000), (CLOCK, 0x10000), (CAMERA, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        for at in (ITEM_BY_ID, ALLOCATION_NAME, ALLOCATE, QUERY_STUB, ACCOUNT_ITEMS, LOG, LOG_FILTER, DECALS,
                   EFFECT_PRESENTER, CAN_MOVE) + tuple(STUB_AT.values()):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        late_join = open(base.late_join_bin(), "rb").read()
        self.preview_state = base.late_join_state_va(late_join) + base.WEAPON_PREVIEW_STATE_OFFSET
        self.state = base.late_join_state_va(late_join) + base.UPGRADE_PREVIEW_STATE_OFFSET
        self.code = base.weapon_camo_va(late_join) + base.UPGRADE_PREVIEW_OFFSET
        self.fresh_scratch = bytes(emu.mem_read(SCRATCH, 0x10000))

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def u16(self, at, value=None):
        if value is None:
            return struct.unpack("<H", self.emu.mem_read(at, 2))[0]
        self.emu.mem_write(at, struct.pack("<H", value))

    def u8(self, at, value=None):
        if value is None:
            return self.emu.mem_read(at, 1)[0]
        self.emu.mem_write(at, bytes([value]))

    def text(self, at):
        out = bytearray()
        while True:
            b = self.u8(at)
            if not b:
                return out.decode()
            out.append(b)
            at += 1

    def ret(self, value, pop):
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.u32(esp))
        self.emu.reg_write(UC_X86_REG_ESP, esp + 4 + pop)

    def stub(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        if address == ITEM_BY_ID:
            dict_id = self.u32(esp + 4)
            self.calls.append(("item_by_id", emu.reg_read(UC_X86_REG_EAX), dict_id))
            self.ret(self.entries.get(dict_id, 0), 4)
        elif address == ALLOCATION_NAME:
            self.calls.append(("allocation_name",))
            self.ret(ALLOCATION_WHERE + 8, 0)
        elif address == ALLOCATE:
            self.calls.append(("allocate", emu.reg_read(UC_X86_REG_EAX)))
            self.ret(SCRATCH, 4)
        elif address == QUERY_STUB:                 # setup_profile copied the profile into esi: the query
            ebx, ebp = emu.reg_read(UC_X86_REG_EBX), emu.reg_read(UC_X86_REG_EBP)
            copy = emu.reg_read(UC_X86_REG_ESI)
            self.calls.append(("query", ebx, (copy - ebx - 0x18) // COPY_SIZE, self.u8(ebp + 12)))
            self.u32(ebx + 0xD2C4, 77)
            self.u8(ebx + 0xD2C8, 0)                # as 0x284BEE, which the stub skips: nothing waits
            emu.reg_write(UC_X86_REG_EIP, SETUP_DONE)
        elif address == ACCOUNT_ITEMS:
            self.calls.append(("account items", emu.reg_read(UC_X86_REG_ESI)))
            self.ret(emu.reg_read(UC_X86_REG_EAX), 0)
        elif address == CAN_MOVE:                   # (dictionary, category, slot): armour of category 2 in slot 2
            category, slot = self.u32(esp + 8), self.u32(esp + 12)
            self.calls.append(("can_move", category, slot))
            self.ret(int(category == 2 and slot == VEST_SLOT), 12)
        elif address in (DECALS, EFFECT_PRESENTER) or address in STUB_AT.values():
            name = {DECALS: "decals", EFFECT_PRESENTER: "effect_presenter"}.get(address) \
                or STUB_NAMES[(address - STUB_CODE) // 0x10]
            ecx = emu.reg_read(UC_X86_REG_ECX)
            pops = {"decals": 8, "effect_presenter": 0, "owner_focus": 0, "focus": 4, "unfocus": 4,
                    "scene_begin": 12, "scene_end": 8, "player_insert": 16, "player_update": 4,
                    "player_remove": 4, "player_detach": 0}[name]
            self.calls.append((name, ecx if name.startswith("player") else self.u32(esp + 4)))
            self.ret(FOCUS if name == "owner_focus" else 0, pops)
        elif address == LOG_FILTER:
            self.calls.append(("filter", self.u32(esp + 4), self.u32(esp + 8)))
            self.ret(1, 0)
        elif address == LOG:
            fmt = self.text(self.u32(esp + 0x24)).replace("%u", "%d")
            args = tuple(self.u32(esp + 0x28 + 4 * i) for i in range(fmt.count("%d")))
            function = self.u32(esp + 4)
            # the function the game's own lines go through (0x29A8E9..0x29A921)
            kind = "log" if (self.u32(function), self.u32(function + 8)) == (LOG_MANAGER | 1, LOG_SINK_VALUE) \
                else "log without the game's function"
            self.calls.append((kind, self.u32(esp + 0x1C), self.u32(esp + 0x20), fmt % args))
            self.ret(0, 0)

    def run(self, start, stop, regs, stack=()):
        emu = self.emu
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        esp = STACK - 0x200
        for i, value in enumerate(stack):
            self.u32(esp + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESP, esp)
        emu.emu_start(start, stop)
        return emu.reg_read(UC_X86_REG_ESP) == esp

    def call(self, address, regs, args=()):
        emu = self.emu
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        esp = STACK - 0x100 - 4 * len(args) - 4
        self.u32(esp, STOP)
        for i, value in enumerate(args):
            self.u32(esp + 4 + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESP, esp)
        emu.emu_start(address, STOP)

    # -- the made-up lobby -------------------------------------------------

    def record(self, at, item, dict_id, upgrades):
        self.u32(at, item)
        self.u16(at + 4, dict_id)
        self.u32(at + 8, 1000000)
        begin = self.u32(at + 0x60)
        for i, (up, up_dict, slot, *value) in enumerate(upgrades):
            self.emu.mem_write(begin + 0x18 * i, struct.pack("<IIQB7x", up, up_dict, value[0] if value else 0, slot))
        self.u32(at + 0x64, begin + 0x18 * len(upgrades))

    def upgrades(self, at):
        begin, end = self.u32(at + 0x60), self.u32(at + 0x64)
        out = []
        for p in range(begin, end, 0x18):
            up, up_dict, value, slot = struct.unpack("<IIQB", self.emu.mem_read(p, 17))
            out.append((up, up_dict, slot) if not value else (up, up_dict, slot, value))
        return out

    def table_record(self, profile, item):
        for i in range(self.u8(profile + ITEMS)):
            if self.u32(profile + TABLE + i * ITEM_SIZE) == item:
                return profile + TABLE + i * ITEM_SIZE
        return None

    def setup(self, kits, table, inventory, items, chosen=0, window=None, fresh=True):
        """kits: [{slot: item id}]; table: [(item id, [upgrades])]: the
        profile's records (an upgrade (id, dict id, slot)); inventory:
        [(item id, dict id, [upgrades])]; items: the dictionary's entries
        [(dict id, category, name, fit)] (fit: None or (scope, [weapons]),
        names or ints)."""
        emu = self.emu
        self.calls = []
        if fresh:
            emu.mem_write(self.preview_state, bytes(base.WEAPON_PREVIEW_STATE_SIZE))
            emu.mem_write(self.state, bytes(base.UPGRADE_PREVIEW_STATE_SIZE))
            emu.mem_write(SCRATCH, self.fresh_scratch)
            for character in (CHARACTER, SQUAD):
                emu.mem_write(character, bytes(0xD558))
                for i in range(3):                  # its three profiles, as lobby_character's ctor makes them
                    self.call(0x2F73F0, {UC_X86_REG_ECX: self.copy(i, character)})
                self.u32(character + 0xD2C4, 0xFFFFFFFF)
        self.u32(CHARACTER + 0xD2BC, MENU)
        self.u32(SQUAD + 0xD2BC, MENU)
        self.u32(MENU + 0x694, CHARACTER)
        self.u32(MENU + 0x6A0, PROFILE)
        self.u32(MENU + 0x9C, CLOCK)
        self.u8(CHARACTER + 0xD515, 1)
        emu.mem_write(PROFILE, bytes(0x4640))
        self.call(0x2F73F0, {UC_X86_REG_ECX: PROFILE})
        self.u8(PROFILE + KITS, len(kits))
        self.u8(PROFILE + CHOSEN, chosen)
        self.u32(PROFILE + 0x4620, KIT_WORD)       # what the handlers load before they send
        index = {item: i for i, (item, _) in enumerate(table)}
        dicts = {item: dict_id for item, dict_id, _ in inventory}
        for i, (item, upgrades) in enumerate(table):
            self.record(PROFILE + TABLE + i * ITEM_SIZE, item, dicts.get(item, item % 1000), upgrades)
        self.u8(PROFILE + ITEMS, len(table))
        for k, kit in enumerate(kits):
            for slot in range(23):
                self.u16(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot, index[kit[slot]] if slot in kit else 0xFFFF)
                self.u16(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot + 2, 1)
            self.u32(PROFILE + KIT0 + k * KIT_SIZE + 0x6FC, 9000 + k)
            self.u32(PROFILE + REVISIONS + 4 * k, 3 + k)
        for i, (item, dict_id, upgrades) in enumerate(inventory):
            at = INVENTORY + i * ITEM_SIZE
            self.call(ITEM_CTOR, {UC_X86_REG_ECX: at})
            self.record(at, item, dict_id, upgrades)
        self.u32(PROFILE + 0x4634, INVENTORY)
        self.u32(PROFILE + 0x4638, INVENTORY + len(inventory) * ITEM_SIZE)
        self.u32(PROFILE + 0x463C, ALLOCATOR)
        self.u32(PROFILE + 0x4640, INVENTORY + len(inventory) * ITEM_SIZE)
        self.u32(GAME_ALLOCATOR, ALLOCATOR)
        self.u32(PROFILE - 0xC84, PLAYERS)
        self.u32(PLAYERS + 0x53A4, DICTIONARY)
        self.u32(LOG_FILTERED, 1)
        self.u32(LOG_SINK, LOG_SINK_VALUE)
        self.dictionary(items)
        if window is not None:
            self.u32(self.preview_state + 4, window)
            self.u8(self.preview_state, 1)

    def dictionary(self, items):
        self.entries = {}
        self.emu.mem_write(CONFIGS, bytes(0x10000))
        self.heap = CONFIGS
        for i, (dict_id, category, name, fit) in enumerate(sorted(items, key=lambda item: item[0])):
            at = ENTRIES + i * 0x3A0
            self.emu.mem_write(at, bytes(0x3A0))
            self.entries[dict_id] = at
            self.u16(at, dict_id)
            self.u32(at + 4, self.string(name))
            self.u8(at + 0x228, category)
            if fit is not None:
                scope, weapons = fit
                block = self.table(3, [("scope", self.value(scope)),
                                       ("weapons", self.indexed([self.value(w) for w in weapons]))])
                root = self.table(3, [("parameters", (3, 0, 0)), ("srp_scope_fit", block)])
                kind, nodes, count = root
                node = self.alloc(0x18)
                self.emu.mem_write(node, struct.pack("<IIIIIHH", nodes, 0, 0, 0, 0, kind, count))
                config = self.alloc(0x110)
                self.u32(config + 0x108, node)
                self.u32(at + 0x114, config)
        self.u32(DICTIONARY + 0x130, ENTRIES)
        self.u32(DICTIONARY + 0x134, len(items))

    def alloc(self, size):
        at = self.heap
        self.heap += (size + 15) & ~15
        return at

    def string(self, text):
        at = self.alloc(len(text) + 1)
        self.emu.mem_write(at, text.encode() + b"\0")
        return at

    def value(self, v):
        return (5, self.string(v), 0) if isinstance(v, str) else (1, v, 0)

    def indexed(self, values):
        nodes = self.alloc(0x18 * len(values))
        for i, (kind, data, count) in enumerate(values):
            self.emu.mem_write(nodes + 0x18 * i, struct.pack("<IIIIIHH", data, 0, 0, 0, 0, kind, count))
        return (4, nodes, len(values))

    def table(self, kind, children):
        children = sorted(children, key=lambda child: zlib.crc32(child[0].encode()))
        nodes = self.alloc(0x18 * len(children))
        for i, (name, (ckind, data, count)) in enumerate(children):
            self.emu.mem_write(nodes + 0x18 * i, struct.pack("<IIIIIHH", data, 0, self.string(name), 0,
                                                             zlib.crc32(name.encode()), ckind, count))
        return (kind, nodes, len(children))

    # -- the sites --------------------------------------------------------

    def relocate(self, host, upgrade, slot, menu=MENU):
        regs = {UC_X86_REG_EDI: host, UC_X86_REG_EBX: upgrade, UC_X86_REG_ECX: slot, UC_X86_REG_ESI: PROFILE,
                UC_X86_REG_EAX: CLIENT, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_EDX: 0x55555555}
        balanced = self.run(RELOCATE_SITE, RELOCATE_NEXT, regs, [0, 0, 0, 0, menu, slot])
        kept = all(self.emu.reg_read(r) == v for r, v in regs.items() if r != UC_X86_REG_EDX) \
            and self.emu.reg_read(UC_X86_REG_EDX) == KIT_WORD
        return balanced and kept

    def drop(self, host, upgrade, menu=MENU):
        args = STACK - 0x800                       # the Flash arguments: +8 the host, +20h the upgrade
        self.u32(args + 8, host)
        self.u32(args + 0x20, upgrade)
        frame = STACK - 0x600
        self.u32(frame + 8, args - 8)              # [ebp+8] -> +8 -> the arguments
        self.u32(args - 8 + 8, args)
        regs = {UC_X86_REG_EDI: upgrade, UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: PROFILE,
                UC_X86_REG_EAX: CLIENT, UC_X86_REG_EBP: frame, UC_X86_REG_ECX: 0x55555555}
        balanced = self.run(DROP_SITE, DROP_NEXT, regs, [0, 0, 0, 0, menu])
        kept = all(self.emu.reg_read(r) == v for r, v in regs.items() if r != UC_X86_REG_ECX) \
            and self.emu.reg_read(UC_X86_REG_ECX) == KIT_WORD
        return balanced and kept

    def account(self):
        regs = dict(SENTINELS)
        regs[UC_X86_REG_ESI] = CLIENT
        balanced = self.run(ACCOUNT_SITE, ACCOUNT_NEXT, regs)
        return balanced and all(self.emu.reg_read(r) == v for r, v in regs.items())

    def refresh(self, character=CHARACTER):
        """The lobby's refresh: setup_profile(the account's profile, the character, the chosen kit)."""
        self.call(SETUP, {UC_X86_REG_EAX: PROFILE}, [character, self.u8(PROFILE + CHOSEN)])

    def made(self, character=CHARACTER):
        """on_player_ready as far as the copies go: the next copy is shown, no query."""
        self.u8(character + 0xD2B4, (self.u8(character + 0xD2B4) + 1) % 3)
        self.u32(character + 0xD2C4, 0xFFFFFFFF)

    def lobby_objects(self):
        """What the movie's handlers and on_player_ready reach besides the
        character: the camera, the owner's input focus, the scene, the
        players' vtable; the window closed (+D515h 0, as the handlers leave it)."""
        self.emu.mem_write(CAMERA, bytes(0x10000))
        vtable = lambda at, slots: [self.u32(at + off, STUB_AT[name]) for off, name in slots]
        self.u32(MENU + 0x10C, CAMERA)
        self.u32(CLOCK, VTABLES)                   # the owner ([menu+9Ch]): vtable+38h its input focus
        vtable(VTABLES, ((0x38, "owner_focus"),))
        self.u32(FOCUS, VTABLES + 0x100)
        vtable(VTABLES + 0x100, ((0x10, "focus"), (0x14, "unfocus")))
        self.u32(CHARACTER + 0xD2C0, SCENE)
        self.u32(SCENE, VTABLES + 0x200)
        vtable(VTABLES + 0x200, ((0x2C, "scene_begin"), (0x38, "scene_end")))
        vtable(VTABLES + 0x300, ((0x1C, "player_insert"), (0x20, "player_update"), (0x24, "player_remove"),
                                 (0x28, "player_detach")))
        self.u32(CHARACTER + 0xD504, 0x5EC0)
        self.u32(CHARACTER + 0xD2B8, 0)
        self.u8(CHARACTER + 0xD515, 0)
        self.players = 0

    def handler(self, entry, item):
        """The movie's start_item_upgrade(item) or exit_item_upgrade(item): the
        delegate (ecx the lobby menu, whose +694h is the character), the
        number at +8 of the first value; it returns true."""
        self.u32(ARGS + 8, ARGS + 0x40)
        self.u32(ARGS + 0x48, item)
        regs = dict(SENTINELS)
        regs[UC_X86_REG_ECX] = MENU
        self.call(entry, regs, [ARGS])
        return self.emu.reg_read(UC_X86_REG_EAX) & 0xFF == 1 \
            and all(self.emu.reg_read(r) == v for r, v in SENTINELS.items())

    def player_ready(self):
        """The game's on_player_ready for the query being made: a new player
        (its vtable the stubs', counted) in the query's result (+12Ch)."""
        self.players += 1
        player = PLAYERS_MADE + 0x800 * self.players
        result = player + 0x400
        self.emu.mem_write(player, bytes(0x400))
        self.u32(player, VTABLES + 0x300)
        self.u32(player + 0xD0, 100)               # held elsewhere too: nothing is destroyed
        self.u32(result + 0x12C, player)
        regs = dict(SENTINELS)
        regs[UC_X86_REG_ECX] = CHARACTER
        self.call(PLAYER_READY, regs, [result])
        return player

    def held(self, character=CHARACTER, index=None):
        """The item id of the weapon the shown copy's (or copy `index`'s) kit
        holds: slot 7, or slot 10 when 7 is empty (find_kit's, the player's)."""
        copy = self.copy(self.u8(character + 0xD2B4) if index is None else index, character)
        kit = copy + KIT0 + self.u8(character + 0xD2B5) * KIT_SIZE + SLOTS
        index = self.u16(kit + 4 * WEAPON_1)
        if index == 0xFFFF:
            index = self.u16(kit + 4 * WEAPON_2)
        return None if index == 0xFFFF else self.u32(copy + TABLE + index * ITEM_SIZE)

    def worn(self, character=CHARACTER, index=None):
        """The item ids the shown copy's (or copy `index`'s) kit has in its
        armour slots 0..6."""
        copy = self.copy(self.u8(character + 0xD2B4) if index is None else index, character)
        kit = copy + KIT0 + self.u8(character + 0xD2B5) * KIT_SIZE + SLOTS
        return {self.u32(copy + TABLE + self.u16(kit + 4 * s) * ITEM_SIZE)
                for s in range(7) if self.u16(kit + 4 * s) != 0xFFFF}

    def copy(self, index, character=CHARACTER):
        return character + 0x18 + index * COPY_SIZE

    def queries(self):
        return [c for c in self.calls if c[0] == "query"]

    def count(self):
        return self.u32(self.state + 4)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    align.install()                 # every Uc from here: aligned-SSE operands checked
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    build, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(),
                          late_join=late_join)
    e = Emu(build)
    failures = 0

    def say(ok, text):
        nonlocal failures
        failures += not ok
        print(("ok    " if ok else "FAIL  ") + text)

    AK, SVD, STASH, CAMO, CAMO2, DECAL, SCOPE, SCOPE2, VEST = 1501, 1502, 1504, 1601, 1602, 1603, 1604, 1605, 1505
    inventory = [(AK, 501, [(DECAL, 603, 0)]), (SVD, 502, []), (STASH, 504, []), (VEST, 505, []),
                 (CAMO, 601, []), (CAMO2, 602, []), (DECAL, 603, []), (SCOPE, 2101, []), (SCOPE2, 2102, [])]
    items = [(501, RIFLE, "ak_x", None), (502, RIFLE, "svd_x", None), (504, RIFLE, "rifle_x", None),
             (505, 2, "vest_x", None), (601, CAMO_CATEGORY, "camo_a", None), (602, CAMO_CATEGORY, "camo_b", None),
             (603, 26, "decal_a", None), (2101, SCOPE_CATEGORY, "shop_scope_pso_1", None),
             (2102, SCOPE_CATEGORY, "shop_scope_acog", None),
             (2001, SCOPE_CATEGORY, "fit_svd_pso_1", ("shop_scope_pso_1", ["svd_x"])),
             (2002, SCOPE_CATEGORY, "fit_ak_pso_1", ("shop_scope_pso_1", ["ak_x_premium", "ak_x"])),
             (2003, SCOPE_CATEGORY, "fit_ak_pso_1_b", ("shop_scope_pso_1", ["ak_x"])),
             (2004, SCOPE_CATEGORY, "fit_rifle_acog", (2102, [504]))]
    kits = [{WEAPON_1: AK, 2: VEST}, {WEAPON_2: SVD}, {}]
    table = [(AK, [(DECAL, 603, 0)]), (VEST, []), (SVD, [])]

    def fresh(window=AK, chosen=1, **kw):
        e.setup(kits, table, inventory, items, chosen=chosen, window=window, **kw)

    def account_before():
        return bytes(e.emu.mem_read(PROFILE, 0x4644)), bytes(e.emu.mem_read(INVENTORY, 0x1000))

    # 1. a camouflage dropped on a kit's weapon
    fresh()
    before = account_before()
    kept = e.relocate(AK, CAMO, 2)
    queries = e.queries()
    copy1 = e.copy(1)
    rec = e.table_record(copy1, AK)
    say(kept and queries == [("query", CHARACTER, 1, 0)] and rec is not None
        and e.upgrades(rec) == [(DECAL, 603, 0), (CAMO, 601, 3, KIT(2))]
        and e.u32(copy1 + REVISIONS) == 0x80000001 and e.u32(copy1 + REVISIONS + 4) == 4
        and account_before() == before and e.count() == 1
        and [c for c in e.calls if c[0] == "log"] == [("log", 0x9FA4E4, 4,
                                                       f"srp: upgrade preview: item {AK}, upgrade {CAMO}, slot 2")],
        "a camouflage dropped on kit 1's AK (chosen kit 2): the character is made again at once, from the "
        "scratch: the AK's record with the camouflage after its decal, as the weapon kit (patch 68: slot 3, its own "
        "logical slot 2 in the value), kit 1's revision 0x80000001, the "
        "other kits' the account's; one log line; the account's profile and inventory untouched; the "
        "handler's registers and stack as the original leaves them")

    # 2. the server's account confirms it while the player is being made
    e.calls = []
    e.record(e.table_record(PROFILE, AK), AK, 501, [(DECAL, 603, 0), (CAMO, 601, 3, KIT(2))])
    e.u32(PROFILE + REVISIONS, 4)
    kept = e.account()
    e.refresh()
    say(kept and e.count() == 0 and [c for c in e.calls if c[0] == "account items"] == [("account items", CLIENT)]
        and not e.queries() and e.u8(CHARACTER + 0xD2C8) == 0 and e.u32(CHARACTER + 0xD2C4) == 77,
        "the server's account (0x87) with it, while that player is being made: the queue empties, the 0x87 "
        "reader goes on (registers kept), and the refresh makes nothing (`again`: the player being made is "
        "of that kit's id and revision), nothing waits")
    e.calls = []
    e.made()
    e.refresh()
    say(not e.queries() and e.u8(CHARACTER + 0xD2C8) == 0 and e.u32(CHARACTER + 0xD2C4) == 0xFFFFFFFF,
        "once made, a refresh returns at setup_profile's own early return (the same revision)")

    # 3. a refusal: the account comes back without it
    e.calls = []
    e.relocate(AK, CAMO2, 2)
    q = e.queries()
    rec = e.table_record(e.copy(2), AK)
    ok1 = q == [("query", CHARACTER, 2, 0)] and e.upgrades(rec) == [(DECAL, 603, 0), (CAMO2, 602, 3, KIT(2))] \
        and e.u32(e.copy(2) + REVISIONS) == 0x80000002
    e.calls = []
    e.account()                                # refused: the account as it was
    e.refresh()
    waiting = e.u8(CHARACTER + 0xD2C8)
    copy_w = e.copy(0)                         # the shown copy is 1, the one made 2, the waiting one 0
    rec_w = e.table_record(copy_w, AK)
    say(ok1 and not e.queries() and waiting == 1 and e.upgrades(rec_w) == [(DECAL, 603, 0), (CAMO, 601, 3, KIT(2))]
        and e.u32(copy_w + REVISIONS) == 0x80000003,
        "another camouflage into slot 2 replaces the first at once; refused, the account's refresh while it "
        "is being made waits for it with the account's upgrades and a revision of its own (on_player_ready "
        "makes that one next)")

    # 4. a take-off, a replace in the full record, no room
    fresh()
    e.calls = []
    kept = e.drop(AK, DECAL)
    rec = e.table_record(e.copy(1), AK)
    say(kept and e.queries() == [("query", CHARACTER, 1, 0)] and e.upgrades(rec) == [],
        "drop_upgrade_item(AK, decal): made again at once without the decal; the handler's registers (ecx "
        "loaded, eax kept for patch 30) and stack the original's")
    full = [(AK, [(DECAL, 603, 0), (1606, 603, 1), (CAMO, 601, 3, KIT(2))]), (VEST, []), (SVD, [])]
    e.setup(kits, full, inventory, items, chosen=1, window=AK)
    e.relocate(AK, CAMO2, 2)
    rec = e.table_record(e.copy(1), AK)
    replaced = e.upgrades(rec) == [(DECAL, 603, 0), (1606, 603, 1), (CAMO2, 602, 3, KIT(2))]
    e.made()
    e.calls = []
    e.relocate(AK, SCOPE, 3)
    rec = e.table_record(e.copy(2), AK)
    say(replaced and e.queries() == [("query", CHARACTER, 2, 0)] and e.count() == 2
        and e.upgrades(rec) == [(DECAL, 603, 0), (1606, 603, 1), (SCOPE, 2002, 3, KIT(3) | 602)],
        "three records held (two decals, the kit): a camouflage into slot 2 replaces the one the kit holds; a "
        "scope into slot 3 then goes in too, four upgrades in three records (the kit its own, the camouflage "
        "named in its value), made again at once")

    # 5. scopes: the variant made for the weapon
    fresh()
    e.relocate(AK, SCOPE, 3)
    rec = e.table_record(e.copy(1), AK)
    by_name = e.upgrades(rec) == [(DECAL, 603, 0), (SCOPE, 2002, 3, KIT(3))]
    fresh(window=STASH, chosen=0)
    e.relocate(STASH, SCOPE2, 3)
    rec = e.table_record(e.copy(1), STASH)
    by_id = rec is not None and e.upgrades(rec) == [(SCOPE2, 2004, 3, KIT(3))] \
        and e.u16(e.copy(1) + KIT0 + SLOTS + 4 * WEAPON_1) == (rec - e.copy(1) - TABLE) // ITEM_SIZE
    fresh(window=SVD, chosen=0)
    e.relocate(SVD, SCOPE2, 3)
    none = not e.queries() and e.count() == 0
    say(by_name and by_id and none,
        "a scope's record holds the variant: the AK's PSO-1 the first (lowest) entry naming the scope and the "
        "weapon by name (2002, not 2003); a rifle only in the stash, shown in the chosen kit's hands, its "
        "ACOG by dict ids (2004); the SVD and the ACOG, no variant: nothing previewed, nothing made")

    # 6. nothing previewed
    for name, prepare, args in (
            ("not the window's item", lambda: fresh(window=SVD), (AK, CAMO, 2)),
            ("no weapon shown (patch 32's mode 0)", lambda: (fresh(), e.u8(e.preview_state, 0)), (AK, CAMO, 2)),
            ("a character without its window", lambda: (fresh(), e.u8(CHARACTER + 0xD515, 0)), (AK, CAMO, 2)),
            ("an upgrade the inventory lacks", lambda: fresh(), (AK, 4444, 2))):
        prepare()
        e.calls = []
        before = account_before()
        kept = e.relocate(*args)
        say(kept and not e.queries() and e.count() == 0 and account_before() == before
            and not [c for c in e.calls if c[0] == "log"],
            f"{name}: nothing queued, nothing made, the handler's registers the original's")

    # 7. the original's ways: a squad mate's character, the 0x87 site with nothing queued, `made`
    fresh()
    e.u8(SQUAD + 0xD2C4, 0)
    e.u32(SQUAD + 0xD2C4, 55)                   # its player being made
    e.calls = []
    e.call(SETUP, {UC_X86_REG_EAX: PROFILE}, [SQUAD, 0])
    squad = e.u8(SQUAD + 0xD2C8) == 1 and not e.queries()
    e.u32(SQUAD + 0xD2C4, 0xFFFFFFFF)
    e.call(SETUP, {UC_X86_REG_EAX: PROFILE}, [SQUAD, 0])
    say(squad and e.queries() == [("query", SQUAD, 1, 0)] and e.u32(e.state + 0x14) == 0,
        "a squad mate's character: setup_profile as the original (waits while its player is made, queries "
        "after), no time kept")
    fresh()
    e.calls = []
    kept = e.account()
    say(kept and e.count() == 0 and [c[0] for c in e.calls] == ["account items"],
        "the 0x87 reader's end with nothing queued: its last call as before, every register kept")
    fresh()
    e.u32(CLOCK + 0x584C, 1000)
    e.refresh()
    e.u32(CLOCK + 0x584C, 1340)
    lines = []
    for character, flag in ((CHARACTER, 0), (CHARACTER, 1), (SQUAD, 0)):
        e.calls = []
        e.u8(character + 0xD2C9, flag)
        regs = dict(SENTINELS)
        regs[UC_X86_REG_EBX] = character
        regs[UC_X86_REG_EAX] = 0x66666666
        balanced = e.run(MADE_SITE, MADE_NEXT, regs)
        zf = (e.emu.reg_read(UC_X86_REG_EFLAGS) >> 6) & 1
        logs = [c[3] for c in e.calls if c[0] == "log"]
        lines.append(balanced and zf == (flag == 0) and all(e.emu.reg_read(r) == v for r, v in regs.items())
                     and logs == (["srp: lobby character's player made in 340 ms"] if character == CHARACTER else []))
    say(all(lines), "on_player_ready: the lobby's own character logs the time since its query (340 ms), a squad "
                    "mate's nothing; the flags of the original's comparison and every register kept")

    # 8. the owner's quick Esc (2026-10-23): a window closed while its scratch's player is being made
    table_state = base.late_join_state_va(late_join) + base.WEAPON_TABLE_STATE_OFFSET
    early_bytes = next(old for va, old, _, _ in base.UPGRADE_PREVIEW_HOOKS if va == EARLY_SITE)
    built_bytes = bytes(e.emu.mem_read(EARLY_SITE, len(early_bytes)))

    def quick_esc(item, fixed=True):
        """The account's chosen kit 1 (the SVD its weapon 2, no armour) shown;
        the window of `item` opened and closed at once; then each player the
        queries make arrives. (the steps' results, the held weapon, the kit's
        armour worn)."""
        e.emu.mem_write(EARLY_SITE, built_bytes if fixed else early_bytes)
        e.emu.ctl_remove_cache(EARLY_SITE, EARLY_SITE + len(early_bytes))   # the site's code translated again
        fresh(window=None, chosen=1)
        e.emu.mem_write(table_state, bytes(base.WEAPON_TABLE_STATE_SIZE))
        e.lobby_objects()
        e.refresh()                                # the lobby shows the chosen kit
        e.player_ready()
        shown = e.held() == SVD and e.u8(CHARACTER + 0xD2B4) == 1
        e.calls = []
        started = e.handler(START_HANDLER, item)
        making = e.queries() == [("query", CHARACTER, 2, 1)] and e.u32(CHARACTER + 0xD2C4) == 77 \
            and e.u8(CHARACTER + 0xD515) == 1 and e.u32(table_state + 0x28) == 0 \
            and item in {e.held(index=2)} | e.worn(index=2)
        e.calls = []
        closed = e.handler(EXIT_HANDLER, item)
        waits = not e.queries() and e.u8(CHARACTER + 0xD2C8) == 1 and e.u8(e.preview_state) == 0 \
            and e.u8(CHARACTER + 0xD515) == 0 and e.u32(table_state) == 0
        e.calls = []
        e.player_ready()                           # the window's player
        again = e.queries() == [("query", CHARACTER, 0, 1)] and e.u8(CHARACTER + 0xD2C8) == 0
        last = e.player_ready() if e.u32(CHARACTER + 0xD2C4) != 0xFFFFFFFF else None
        e.emu.mem_write(EARLY_SITE, built_bytes)
        e.emu.ctl_remove_cache(EARLY_SITE, EARLY_SITE + len(early_bytes))
        return (shown, started and making, closed and waits, again, last is not None
                and e.u32(CHARACTER + 0xD2B8) == last), e.held(), e.worn()

    steps, held, worn = quick_esc(STASH)

    say(all(steps) and held == SVD and STASH not in worn,
        "the owner's quick Esc on a rifle only in the stash (chosen kit 2, the SVD its weapon 2): the start "
        "queries the scratch's player (the rifle the kit's weapon 1), the close while it is being made waits "
        "(+D2C8h) instead of returning early, the window's player arrives and on_player_ready queries the "
        "account's at once (0x2850C5); once that arrives the character holds the SVD")
    steps_bug, held_bug, _ = quick_esc(STASH, fixed=False)
    say(steps_bug[:2] == (True, True) and not steps_bug[2] and held_bug == STASH,
        "the same without `early` (0x284B7C the original's): the close returns early, nothing waits, and the "
        "window's player stays: the stash rifle in the hands, the kit's SVD not (the owner's screenshot)")
    steps, held, worn = quick_esc(VEST)
    say(all(steps) and held == SVD and VEST not in worn,
        "the same for armour the chosen kit does not wear (shown worn from the scratch): after the quick "
        "close the character wears the kit's own again")

    # `early` itself: the flag of the original's test, for whom and when
    rows = []
    for character, waiting, query in itertools.product((CHARACTER, SQUAD), (0, 1), (0xFFFFFFFF, 77)):
        fresh()
        e.u8(character + 0xD2C8, waiting)
        e.u32(character + 0xD2C4, query)
        regs = dict(SENTINELS)
        regs[UC_X86_REG_EBX] = character
        regs[UC_X86_REG_EAX], regs[UC_X86_REG_ECX], regs[UC_X86_REG_EDX] = 0x0A0A0A0A, 0x0C0C0C0C, 0x0D0D0D0D
        balanced = e.run(EARLY_SITE, EARLY_JZ, regs)
        zf = (e.emu.reg_read(UC_X86_REG_EFLAGS) >> 6) & 1
        want = waiting == 0 and (character == SQUAD or query == 0xFFFFFFFF)
        rows.append(balanced and zf == want and all(e.emu.reg_read(r) == v for r, v in regs.items()))
    say(all(rows), "`early` at 0x284B7C: the early return (ZF) when no call waits, for the lobby's own "
                   "character only while no player is being made, for a squad mate's as the original; every "
                   "register and the stack kept")

    align.report(say)                # say counts a failure
    print(f"upgrade preview: {'all checks passed' if not failures else f'{failures} FAILED'}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
