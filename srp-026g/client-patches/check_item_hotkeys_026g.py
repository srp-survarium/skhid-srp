#!/usr/bin/env python3
"""Run patch 149 (a key per item type uses it: item_hotkeys_026g.asm's moved
action table, rust/patches/src/item_hotkeys.rs at the input builder's end) in
the default build on made-up game objects, beside the original.

    python check_item_hotkeys_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; check_hide_ui_026g.py's and check_blackout_026g.py's
harnesses. The game's own code runs:

- the action table: the build's `actions` is the game's 60 rows (artefact's
  default key kH, patch 78's), then the sixteen (4Ah..59h); id_to_action_name
  (0x2079D0) and action_name_to_ptr (0x207AD0) answer every row of the build,
  the game's as the original;
- the key binder's bindings, 60h of them: the game's init allocates the
  binder 60h x 0Ch + 4 bytes (0x1EA265; the original 364h); its construction
  (0x207540 to 0x207573) clears the 60h, gives every id the original gives a
  description the same row's bytes (artefact's key aside) and the sixteen
  their rows, 48h, 49h and 58h.. none, and puts the game after the bindings,
  nothing past it; the binder's bind (0x207DC0, `use_onyx kF5` and
  `use_oxygen_tank kF6` as user.cfg's lines have them) makes them those
  actions' for get_binded_action (0x207D50) with the game world's kind 1 and
  the input handler's 4, not the lobby's 2 or the chat's 8; the bind
  command's save (0x207450) writes both lines; `bind quick_use_1 kF6` takes F6
  from 57h (bind_key's pass over the bindings); unbind (0x207FF0) clears
  4Ah's key; an unbind of an unknown name clears binding 49h's, inside the
  build's binder (past the original's); the options (game_options 0x2013A0,
  0x201F20): a key set for 4Ah is kept in the 43rd row and the accept runs
  `bind use_onyx kF5`;
- the input: process_first_person_mode (0x241250 to its `ret 8`, no keyboard
  look) on a made-up handler whose game world's network client's local
  player holds a kit (pockets, gear, artefact containers: the game's own
  quick_slot_to_inventory_slot 0x2FE7F0 orders them; an oxygen pack has the
  game's oxygen_tank vtable 0xA1C874). For each of the fourteen type keys,
  each kit, each event (down, held, up, held from before) the build's mask
  for that key is the original's for the quick-use key (39h + n) of the
  first quick slot holding its item, and the original's mask with nothing
  pressed when no quick slot of the six does; the group keys (the next
  artefact, the next medkit) on three kits of ready and busy items (a
  medicine active or a used-up stack, the antitoxin and the painkiller no
  medkit; an artefact active, cooling, or a fifth copy past the 4 of its
  kind that count) are the quick key of the first
  quick slot a press would use, nothing when none; two keys at once are both
  quick keys; with H (patch 78's carry key) held the build's masks for the
  key are its masks for that quick key, tick by tick.
"""
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBP, UC_X86_REG_ESI, \
    UC_X86_REG_EDI

import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout

GAME_TABLE, GAME_ROWS, ROWS = base.ITEM_HOTKEYS_TABLE, base.ITEM_HOTKEYS_GAME_ROWS, base.ITEM_HOTKEYS_ROWS
ARTEFACT, KH = 0x23, 0x9FDCA8                       # patch 78's carry key, "kH"
CONSTRUCT, CONSTRUCTED = 0x207540, 0x207573         # the binder's construction: its rows into the bindings
ID_TO_NAME, NAME_TO_PTR, GET_BOUND = 0x2079D0, 0x207AD0, 0x207D50
BUILDER = 0x241250                                  # process_first_person_mode (handler, keyboard look), ret 8
PUSH_BACK, CARRY_KEY = 0x242580, (0x23FE42, 0x23FE48)
QUICK_USE = 0x39                                    # quick_use_1; _n is 39h + n - 1
DOWN, UP, HOLD = 0, 1, 2                            # an event's state (player_input_handler::action_state_enum)
WORLD = 0x26000000                                  # the made-up game world and what it holds
GW, GAMEOBJ, CLIENT, PLAYER = (WORLD + 0x100 * i for i in range(4))
INVENTORY, CONTAINERS = WORLD + 0x400, WORLD + 0x600  # the containers their own allocation, past the inventory's 188h
ITEMS, VTABLE, RET0 = WORLD + 0x1000, WORLD + 0x800, WORLD + 0x900
HELD = 0x2A8
# (name, id, item's dict id) as item_hotkeys.rs has them
OXYGEN, OXYGEN_VTABLE = "an oxygen pack", 0xA1C874       # any item of oxygen_tank's class (its ctor 0x33E029)
KEYS = (("use_onyx", 0x4A, 76), ("use_spring", 0x4B, 74), ("use_lifebone", 0x4C, 54),
        ("use_bandages", 0x4D, 66), ("use_medkit", 0x4E, 67), ("use_military_medkit", 0x4F, 390),
        ("use_blackmarket_medkit", 0x50, 1196), ("use_science_medkit", 0x51, 391),
        ("use_antitoxin", 0x52, 82), ("use_painkiller", 0x53, 65), ("use_rgd5", 0x54, 5), ("use_rdgm", 0x55, 219),
        ("use_trap", 0x56, 68), ("use_oxygen_tank", 0x57, OXYGEN))
GROUPS = (("use_next_artefact", 0x58), ("use_next_medkit", 0x59))   # a group's first ready item
MEDKIT_VTABLE = 0xA1C79C                            # every drug's (medkit's ctor 0x33CAE6)
ONYX_VT, SPRING_VT = WORLD + 0xA00, WORLD + 0xB00    # made-up artefact vtables: +80h their kind's enable_active
# Group kits: (slot or "c" for the next container, dict id, vtable, {offset: (struct format, value)}), then
# the quick ordinal (0-based) each group key must press, None for none. A medicine: +2B0h active, its
# entry's +22Ah stack, +260h count (medkit::action 0x33D700); an artefact: +280h state (3 passive, 4
# active), +28Ch cooldown left (artefact_base::action 0x313300).
ACTIVE, STACK, COUNT, STATE, COOLDOWN = 0x2B0, -0x22A, 0x260, 0x280, 0x28C   # STACK: in the item's entry
GROUP_KITS = (
    ("antitoxin; a Medikit in use; Military; an active Onyx, a cooling Onyx, a ready Spring",
     [(13, 82, MEDKIT_VTABLE, {}), (14, 67, MEDKIT_VTABLE, {ACTIVE: ("B", 1)}), (15, 390, MEDKIT_VTABLE, {}),
      ("c", 76, None, {STATE: ("I", 4)}), ("c", 76, None, {STATE: ("I", 3), COOLDOWN: ("I", 5000)}),
      ("c", 74, None, {STATE: ("I", 3)})], {"use_next_medkit": 2, "use_next_artefact": 5}),
    ("used-up bandages; a painkiller; a ready Lifebone, a cooling Spring",
     [(13, 66, MEDKIT_VTABLE, {STACK: ("B", 1), COUNT: ("H", 0)}), (14, 65, MEDKIT_VTABLE, {}),
      ("c", 54, None, {STATE: ("I", 3)}), ("c", 74, None, {STATE: ("I", 3), COOLDOWN: ("I", 1)})],
     {"use_next_medkit": None, "use_next_artefact": 2}),
    ("five Onyx, the first four cooling, the fifth ready (past the 4 that count, patch 109); a ready Spring",
     [("c", 76, ONYX_VT, {STATE: ("I", 3), COOLDOWN: ("I", 9000)})] * 4 + [("c", 76, ONYX_VT, {STATE: ("I", 3)}),
                                                                       ("c", 74, SPRING_VT, {STATE: ("I", 3)})],
     {"use_next_medkit": None, "use_next_artefact": 5}),
    ("a Science Medkit (a stack of 2); no artefact",
     [(13, 391, MEDKIT_VTABLE, {STACK: ("B", 1), COUNT: ("H", 2)})], {"use_next_medkit": 0, "use_next_artefact": None}),
)
ONYX, BANDAGES, OXYGEN_KEY = 0x4A, 0x4D, 0x57
BINDINGS = base.ITEM_HOTKEYS_BINDINGS               # the build's bindings (the game's 48h)
ALLOCATE, ALLOCATED, ALLOCATOR = 0x1EA265, 0x1EA26F, 0xE4F90     # the game's init: mov eax,size / call alloc
UNBIND = 0x207FF0                                   # unbind (thiscall binder; name, which; ret 8)
UNKNOWN = 0x208019                                  # its way for a name of no row (esi the binder)
# Kits: {fixed slot: dict id}, then the artefact containers (None: empty). The
# quick slots in the game's order: pockets 0Dh..12h, the gear's, then every
# container (an empty one too); only the first six have a key.
KITS = (
    ("pockets bandages, medkit, painkiller, antitoxin; Onyx, an empty container, Spring, Onyx",
     {13: 66, 14: 67, 15: 65, 16: 82}, [76, None, 74, 76]),
    ("pockets military, (empty), black market, science (18); Lifebone, Spring",
     {13: 390, 15: 1196, 18: 391}, [54, 74]),
    ("pockets RGD-5, RDG-M, a trap; an oxygen pack (scav_oxygen_1, the back); Spring, Onyx",
     {13: 5, 14: 219, 15: 68, 3: (9, OXYGEN_VTABLE)}, [74, 76]),
    ("pocket RDG-M (17); an oxygen pack (ren_back_5); Lifebone",
     {17: 219, 3: (133, OXYGEN_VTABLE)}, [54]),
)


def quick_order(fixed, containers):
    """What the quick slots hold in the game's order: a dict id, OXYGEN, None (an empty container)."""
    kind = lambda item: OXYGEN if isinstance(item, tuple) else item
    order = [kind(fixed[s]) for s in (13, 14, 15, 16, 17, 18, 3, 0, 1, 2, 4, 5, 6) if s in fixed]
    return order + list(containers)


class Game(blackout.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(WORLD, 0x10000, UC_PROT_ALL)
        self.emu.mem_write(RET0, b"\x31\xc0\xc3")             # xor eax,eax / ret: an item's +58h
        self.word(VTABLE + 0x58, RET0)

    def kit(self, fixed, containers):
        """The local player's inventory: `fixed` {slot: dict id}, `containers` [dict id or None]."""
        emu = self.emu
        emu.mem_write(WORLD, bytes(0x10000))
        emu.mem_write(RET0, b"\x31\xc0\xc3")
        self.word(VTABLE + 0x58, RET0)
        for vtable, enable_active in ((ONYX_VT, 0x3128C0), (SPRING_VT, 0x3118D0)):
            self.word(vtable + 0x58, RET0)
            self.word(vtable + 0x80, enable_active)
        self.word(INVENTORY + 0x174, PLAYER + 0x108)   # its holder (base_player's interface, +108h)
        self.word(GW + 0x9C, GAMEOBJ)
        self.word(GAMEOBJ + 0x53AC, CLIENT)
        self.word(CLIENT + 4, PLAYER)
        self.word(PLAYER + 0x10C, INVENTORY)
        n = 0

        def item(dict_id):
            """An item: a dict id (a made-up class, its +58h 0), (dict id, its game vtable), or
            (dict id, vtable or None, {offset: (format, value)}: negative an offset of its entry)."""
            nonlocal n
            spec = dict_id if isinstance(dict_id, tuple) else (dict_id,)
            dict_id, vtable, fields = spec[0], spec[1] if len(spec) > 1 else None, spec[2] if len(spec) > 2 else {}
            vtable = vtable or VTABLE
            at, entry = ITEMS + 0x800 * n, ITEMS + 0x800 * n + 0x400
            n += 1
            self.word(at, vtable)
            self.word(at + 0x24C, 1)
            self.word(at + 0x25C, entry)
            self.word(at + 0x250, INVENTORY)
            emu.mem_write(entry, struct.pack("<H", dict_id))
            for offset, (form, value) in fields.items():
                emu.mem_write(entry - offset if offset < 0 else at + offset, struct.pack("<" + form, value))
            return at
        for slot, dict_id in fixed.items():
            self.word(INVENTORY + 0x10C + 4 * slot, item(dict_id))
        for i, dict_id in enumerate(containers):
            self.word(CONTAINERS + 4 * i, item(dict_id) if dict_id is not None else 0)
        self.word(INVENTORY + 0x168, CONTAINERS)
        emu.mem_write(INVENTORY + 0x16C, bytes([len(containers)]))

    def push(self, action, state):
        pair, sp = blackout.WORK + 0x6000, hide.STACK + 0x8000
        self.word(pair, action)
        self.word(pair + 4, state)
        self.word(sp, hide.SENTINEL)
        self.run(PUSH_BACK, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: hide.HANDLER + 0x98,
                                                   UC_X86_REG_EDX: pair})

    def build(self, events=(), held=(), carry=None):
        """process_first_person_mode on the handler with `events` [(action, state)] and `held`
        actions; `carry`: H's event this tick (pushed, and given to patch 78's key site)."""
        handler = hide.HANDLER
        self.emu.mem_write(handler, bytes(0x1000))
        self.word(handler + 0x334, GW)
        for action, state in events:
            self.push(action, state)
        if carry is not None:
            self.push(ARTEFACT, carry)
            frame, sp = hide.STACK + 0xA000, hide.STACK + 0x8000
            self.word(frame + 0x10, carry + 1)
            self.run(CARRY_KEY[0], (CARRY_KEY[1],), sp, {**hide.GAME, UC_X86_REG_EDI: ARTEFACT,
                                                         UC_X86_REG_EBP: frame})
            assert self.stopped == CARRY_KEY[1], hex(self.stopped or 0)
        items = handler + 0x800
        for i, action in enumerate(held):
            self.word(items + 4 * i, action)
        self.word(handler + HELD, items)
        self.word(handler + HELD + 4, items + 4 * len(held))
        sp = hide.STACK + 0x8000
        for i, value in enumerate((hide.SENTINEL, handler, 0)):
            self.word(sp + 4 * i, value)
        regs, esp = self.run(BUILDER, (hide.SENTINEL,), sp, dict(hide.GAME))
        kept = self.stopped == hide.SENTINEL and esp == sp + 12 \
            and all(regs[r] == hide.GAME[r] for r in (UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP,
                                                     blackout.UC_X86_REG_EBX))
        return self.read(handler + blackout.INPUT_MASK), kept


class Binder(hide.Options):
    """The options' harness, its binder made by the exe's own construction."""

    def __init__(self, data):
        super().__init__(data)
        self.descriptions, _ = self.construct(hide.BINDER)

    def construct(self, binder):
        """The construction on 600h bytes of 0x5A: the bindings' descriptions, and what it wrote."""
        self.emu.mem_write(binder, b"\x5a" * 0x600)
        sp = hide.STACK + 0x8000
        self.run(CONSTRUCT, (CONSTRUCTED,), sp, {**hide.GAME, UC_X86_REG_EAX: 0x12345678, UC_X86_REG_ESI: binder})
        assert self.stopped == CONSTRUCTED, hex(self.stopped or 0)
        written = bytes(self.emu.mem_read(binder, 0x600))
        return [self.read(binder + 12 * i) for i in range(BINDINGS)], written

    def allocation(self):
        """The size the game's init asks the allocator for the binder."""
        asked = []

        def allocator(emu, *_):
            asked.append(emu.reg_read(UC_X86_REG_EAX))
            sp = emu.reg_read(hide.UC_X86_REG_ESP)
            emu.reg_write(UC_X86_REG_EAX, hide.OUT + 0x4000)
            emu.reg_write(hide.UC_X86_REG_ESP, sp + 8)
            emu.reg_write(hide.UC_X86_REG_EIP, self.read(sp))
        hook = self.emu.hook_add(hide.UC_HOOK_CODE, allocator, begin=ALLOCATOR, end=ALLOCATOR)
        sp = hide.STACK + 0x8000
        self.word(sp, 0)                               # the allocator argument the init pushed
        self.run(ALLOCATE, (ALLOCATED,), sp, dict(hide.GAME))
        self.emu.hook_del(hook)
        return asked, self.stopped == ALLOCATED and self.emu.reg_read(hide.UC_X86_REG_ESP) == sp + 4

    def unbind(self, name, which=0):
        self.emu.mem_write(hide.OUT + 0x500, name.encode() + b"\0")
        _, popped = self.call(UNBIND, [hide.OUT + 0x500, which], {UC_X86_REG_ECX: hide.BINDER})
        return popped

    def unbind_unknown(self, which):
        """unbind past action_name_to_ptr's answer for a name of no row (0x208019; its log not run)."""
        sp = hide.STACK + 0x8000
        for i, value in enumerate((0x55555555, hide.SENTINEL, hide.OUT + 0x500, which)):
            self.word(sp + 4 * i, value)
        self.run(UNKNOWN, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ESI: hide.BINDER})
        return self.stopped == hide.SENTINEL

    def bind(self, line):
        self.emu.mem_write(hide.OUT + 0x100, line.encode() + b"\0")
        self.call(hide.BIND_KEY, [hide.OUT + 0x100, 0], {UC_X86_REG_ECX: hide.BINDER})

    def string(self, at):
        return self.text(at).decode() if at else None

    def id_to_name(self, action):
        out, _ = self.call(ID_TO_NAME, [], {UC_X86_REG_EDX: action})
        return self.string(out[UC_X86_REG_EAX])

    def name_to_ptr(self, name):
        self.emu.mem_write(hide.OUT + 0x500, name.encode() + b"\0")
        out, popped = self.call(NAME_TO_PTR, [hide.OUT + 0x500])
        return out[UC_X86_REG_EAX], popped

    def bound(self, key, kinds):
        toggle = hide.OUT + 0x600
        self.word(toggle, 0x7E7E7E7E)
        out, _ = self.call(GET_BOUND, [toggle], {UC_X86_REG_EAX: hide.BINDER, UC_X86_REG_EDI: kinds,
                                                 UC_X86_REG_ESI: key})
        return out[UC_X86_REG_EAX], self.read(toggle)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    actions = base.arena().symbols["actions"]
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the action table (item_hotkeys.data's `actions`; the key binder 0x207540, 0x2079D0, 0x207AD0, 0x207D50)")
    stock, patched = Binder(original), Binder(built)
    row = lambda run, at: struct.unpack("<5I", run.emu.mem_read(at, 20))
    game = [row(stock, GAME_TABLE + 20 * i) for i in range(GAME_ROWS)]
    moved = [row(patched, actions + 20 * i) for i in range(ROWS)]
    named = lambda run, r: (run.string(r[0]),) + r[1:]
    want = [named(stock, r[:4] + (KH,) if r[1] == ARTEFACT else r) for r in game] \
        + [(name, action, 5, 0, 0) for name, action, _ in KEYS] + [(name, action, 5, 0, 0) for name, action in GROUPS]
    report([named(patched, r) for r in moved] == want
           and game == [row(patched, GAME_TABLE + 20 * i) for i in range(GAME_ROWS)],
           f"`actions` ({actions:#x}): the game's {GAME_ROWS} rows (artefact's default key kH), then the sixteen "
           "(4Ah..59h, kinds 5, no toggle, no default key); the game's table unchanged in the build")

    print(f"the key binder's bindings: {BINDINGS:#x} (the game's 48h; 0x1EA265, 0x207540, 0x207D50, 0x207DC0, "
          "0x207450, 0x207FF0)")
    asked = (stock.allocation(), patched.allocation())
    report(asked == (([0x364], True), ([12 * BINDINGS + 4], True)),
           f"the game's init allocates the binder: {asked[1][0][0]:#x} bytes ({BINDINGS:#x} bindings and the game), "
           f"the original {asked[0][0][0]:#x}")
    (a, a_bytes), (b, b_bytes) = stock.construct(hide.BINDER), patched.construct(hide.BINDER)
    new = {action for _, action, _ in KEYS} | {action for _, action in GROUPS}
    at = {action: actions + 20 * i for i, (_, action, *_) in enumerate(moved)}
    game_place = lambda written, n: struct.unpack_from("<I", written, 12 * n)[0]

    def same(action, x, y):
        if not x:
            return not y
        want = row(stock, x)
        return y and row(patched, y) == (want[:4] + (KH,) if action == ARTEFACT else want)
    report(all(same(i, x, y) for i, (x, y) in enumerate(zip(a[:0x48], b)))
           and all(b[action] == at[action] for action in new) and not any(b[0x48:0x4A]) and not any(b[0x5A:])
           and all(actions <= y < actions + 20 * ROWS for y in b if y)
           and game_place(b_bytes, BINDINGS) == 0x12345678 and b_bytes[12 * BINDINGS + 4:] == b"\x5a" * (0x600 - 12 * BINDINGS - 4)
           and game_place(a_bytes, 0x48) == 0x12345678 and a_bytes[0x364:] == b"\x5a" * (0x600 - 0x364),
           f"the construction: {BINDINGS:#x} bindings cleared, every game id's description the same row as the "
           "original's (in `actions`), 4Ah..59h theirs, 48h, 49h and 5Ah.. none; the game at "
           f"+{12 * BINDINGS:#x} (the original's +360h), nothing past it")
    names = [(patched.string(r[0]), r[1]) for r in moved]
    report(all(patched.id_to_name(action) == name for name, action in names)
           and all(stock.id_to_name(r[1]) == stock.string(r[0]) for r in game),
           "id_to_action_name: every row's id its name, the game's as the original")
    found = [(patched.name_to_ptr(name), actions + 20 * i) for i, (name, _) in enumerate(names)]
    report(all(ptr == want and popped == 8 for (ptr, popped), want in found)
           and all(stock.name_to_ptr(stock.string(r[0]))[0] == GAME_TABLE + 20 * i for i, r in enumerate(game)),
           "action_name_to_ptr: every name its row of `actions` (its argument popped), the original its own "
           "table's")
    key = lambda name: hide.key_entry(original, name=name)
    dik = lambda entry: struct.unpack("<I", patched.emu.mem_read(entry + 4, 4))[0]
    f5, f6 = key(b"kF5"), key(b"kF6")
    patched.bind("use_onyx kF5")
    patched.bind("use_oxygen_tank kF6")
    kinds = {k: patched.bound(dik(f5), k) for k in (1, 4, 2, 8)}
    report(patched.read(hide.BINDER + 12 * ONYX + 4) == f5 and patched.bound(dik(f6), 1) == (OXYGEN_KEY, 0)
           and kinds[1] == kinds[4] == (ONYX, 0) and kinds[2][0] == kinds[8][0] == 0x49,
           f"bind `use_onyx kF5` and `use_oxygen_tank kF6` (0x207DC0): F5 is 4Ah's for the game world (kind 1) and "
           f"the input handler (4), toggle 0, F6 57h's (the last); none for the lobby (2) or the chat (8): {kinds}")
    # the bind command's save (as check_hide_ui_026g.py runs it)
    command, saved, allocator = hide.CONSOLE + 0x600, hide.OUT + 0x2000, hide.OUT + 0x3000
    patched.word(command + 0x10, hide.CONSOLE + 0x800)
    patched.word(command + 0x60, 0)
    patched.word(command + 0x64, hide.BINDER)
    patched.word(allocator, allocator + 0x20)
    patched.word(allocator + 0x20 + 0x10, hide.STUBS + 0x40)
    for field, value in ((0, hide.OUT + 0x2100), (4, hide.OUT + 0x2100), (0xC, hide.OUT + 0x2400), (0x10, allocator)):
        patched.word(saved + field, value)
    patched.call(hide.BIND_SAVE, [saved], {UC_X86_REG_ECX: command})
    lines = [patched.text(patched.read(at)) for at in range(hide.OUT + 0x2100, patched.read(saved + 4), 4)]
    report(lines == [b"bind use_onyx kF5", b"bind use_oxygen_tank kF6"],
           f"the bind command's save (0x207450) writes {lines}")
    # bind_key clears a key from every other binding of its kinds, the new ones too
    patched.bind("quick_use_1 kF6")
    report(patched.read(hide.BINDER + 12 * OXYGEN_KEY + 4) == 0 and patched.bound(dik(f6), 1) == (QUICK_USE, 0),
           "bind `quick_use_1 kF6` takes F6 from use_oxygen_tank (57h): bind_key's pass over the bindings reaches it")
    popped = patched.unbind("use_onyx")
    report(popped == 12 and patched.read(hide.BINDER + 12 * ONYX + 4) == 0 and patched.bound(dik(f5), 1)[0] == 0x49,
           "unbind `use_onyx` (0x207FF0) clears its key; F5 is no action's")
    for run in (stock, patched):
        run.construct(hide.BINDER)
        assert run.unbind_unknown(1)
    tail = lambda run, size: bytes(run.emu.mem_read(hide.BINDER + size, 0x10))
    report(patched.read(hide.BINDER + 12 * 0x49 + 8) == 0 and patched.read(hide.BINDER + 12 * BINDINGS) == 0x12345678
           and stock.read(hide.BINDER + 12 * 0x49 + 8) == 0 and 12 * 0x49 + 8 >= 0x364,
           "unbind of a name of no row (0x208019) clears binding 49h's second key: inside the build's binder (the game's "
           f"+{12 * 0x49 + 8:#x} past its {0x364:#x} bytes in the original)")
    options = Binder(built)
    options.emu.mem_write(hide.OUT, b"kF5\0")
    options.call(hide.KEY_SET, [hide.OPTIONS, ONYX], {UC_X86_REG_EAX: hide.OUT})
    controls = base.arena().symbols["controls"]
    row43 = base.HIDE_UI_ROWS + next(i for i, (action, _) in enumerate(base.CONTROLS_ADDED) if action == ONYX)
    options.lines = []
    options.call(hide.APPLY, [])
    report(options.text(options.read(controls + 0x68 * row43 + 0x10)) == b"kF5"
           and options.lines == [(b"bind", b"use_onyx kF5")],
           f"the options: kF5 set for 4Ah (0x2013A0) is kept in row {row43 + 1}; the accept (0x201F20) runs "
           f"{options.lines}")

    print("the input (process_first_person_mode 0x241250, its end's chain: patch 149, then patch 78's chord)")
    stock, patched = Game(original), Game(built)
    for what, fixed, containers in KITS:
        order = quick_order(fixed, containers)
        for run in (stock, patched):
            run.kit(fixed, containers)
        nothing, _ = stock.build()
        lines, wrong = [], 0
        for name, action, dict_id in KEYS:
            slot = next((n for n, d in enumerate(order[:6]) if d == dict_id), None)
            quick = None if slot is None else QUICK_USE + slot
            for state, how in ((DOWN, "down"), (HOLD, "held"), (UP, "up"), (None, "held from before")):
                events = [] if state is None else [(action, state)]
                keys = [] if state is None else [(quick, state)]
                held = [action] if state is None else []
                mask, kept = patched.build(events, held)
                want, stock_kept = stock.build(keys if quick else [], [quick] if quick and state is None else [])
                ok = kept and stock_kept and mask == want and (quick is not None or mask == nothing)
                wrong += not ok
                if not ok:
                    print(f"  FAIL  {what}: {name} {how}: {mask:#x}, the original's quick key {want:#x}")
            lines.append(f"{name} {'slot %d' % (slot + 1) if slot is not None else 'nothing'}")
        report(not wrong, f"{what}: down, held, up and held from before as the original's quick key of "
                          + ", ".join(lines))
    # the group keys: the first quick slot whose item a press would use now
    for what, items, want_slots in GROUP_KITS:
        fixed = {slot: (d, v, f) for slot, d, v, f in items if slot != "c"}
        containers = [(d, v, f) for slot, d, v, f in items if slot == "c"]
        for run in (stock, patched):
            run.kit(fixed, containers)
        nothing, _ = stock.build()
        wrong = 0
        for name, action in GROUPS:
            slot = want_slots[name]
            quick = None if slot is None else QUICK_USE + slot
            for state in (DOWN, HOLD, UP, None):
                events = [] if state is None else [(action, state)]
                mask, kept = patched.build(events, [action] if state is None else [])
                want, stock_kept = stock.build([] if quick is None or state is None else [(quick, state)],
                                               [quick] if quick and state is None else [])
                ok = kept and stock_kept and mask == want and (quick is not None or mask == nothing)
                wrong += not ok
                if not ok:
                    print(f"  FAIL  {what}: {name} state {state}: {mask:#x}, the original's quick key {want:#x}")
        report(not wrong, f"{what}: " + ", ".join(f"{name} {'slot %d' % (want_slots[name] + 1) if want_slots[name] is not None else 'nothing'}"
                                                 for name, _ in GROUPS) + ", as the original's quick key")
    # two keys at once
    stock.kit(*KITS[0][1:])
    patched.kit(*KITS[0][1:])
    mask, kept = patched.build([(ONYX, DOWN), (BANDAGES, DOWN)])
    want, _ = stock.build([(QUICK_USE + 4, DOWN), (QUICK_USE, DOWN)])
    report(kept and mask == want == (1 << 18 | 1 << 14), f"Onyx and bandages at once: {mask:#x}, the original's "
                                                          f"quick keys 5 and 1 {want:#x}")
    # H held (patch 78's chord): the key's masks are the quick key's, tick by tick, in two games
    with_key, with_quick = Game(built), Game(built)
    for run in (with_key, with_quick):
        run.kit(*KITS[0][1:])
    ticks = [((DOWN,), DOWN), ((HOLD,), HOLD), ((HOLD,), None), ((UP,), UP), ((), None), ((DOWN,), None), ((UP,), None),
             ((), DOWN), ((DOWN,), HOLD), ((UP,), UP)]
    masks = []
    for states, carry in ticks:
        a, ka = with_key.build([(ONYX, s) for s in states], carry=carry)
        b, kb = with_quick.build([(QUICK_USE + 4, s) for s in states], carry=carry)
        masks.append((a, b, ka and kb))
    report(all(a == b and k for a, b, k in masks) and any(a & 1 << 29 for a, _, _ in masks),
           "H with use_onyx is H with quick key 5, tick by tick (the chord's marker bit 29 among them): "
           + ", ".join(f"{a:#x}" for a, _, _ in masks))

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
