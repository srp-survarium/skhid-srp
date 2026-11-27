#!/usr/bin/env python3
"""Run patch `reticle`'s table (rust/patches/src/reticle/table.rs, its three lohooks of
reticle.toml; docs/notes/reticle-choice.md, "The build's contract") in the default build on
made-up game objects, the original beside it.

    python check_reticle_table_026g.py <original survarium.exe>

Requires Python `unicorn`. check_upgrade_value_026g.py's harness (its lobby menu, network and
lobby client, inventory, the lobby link's begin and send and kernel32's log calls stubbed; the
game's own send_apply_decal and copy_account_items run). Stubbed besides: the lobby movie's
CreateObject (0x511700) and CreateArray (0x511710), and the AS3 object interface's SetMember
(0x7C6A20), SetElement (0x798340) and ObjectRelease (0x78E610), each recorded; the values they
make are AS3 ones (their interface's vtable 0x8FC288), managed.

- **The message** (lobby_client::on_message_received 0x274A00, from its start, ECX the lobby
  client, a reader {begin, cursor, size} of the body): srp's A9 is taken before the jump table
  and the function returns (`ret 8`, EBX, ESI, EDI, EBP as at the entry), where the original
  reaches the table's jump (0x274A3F) with ECX 29h; any other id (8Bh, A8h) reaches the jump in
  both, the registers the original's.
- **The window's objects** (setup_inventory_data at 0x28867F, its item's AS3 object [ebp-6Ch]
  filled, run to the items' SetElement call 0x288693): a scope item of the table gets
  `srp_reticle` = a new object {pattern, size, colour, brightness, sizes (uints), patterns,
  colours (arrays of the names)}, the three made values released after; an item of no scope of
  the table, and every item after a message that did not parse, gets nothing; the registers at
  the call the original's.
- **The call** (lobby_menu::scaleform_callback 0x28C2C0): srp_set_reticle(item, pattern, size,
  colour, brightness) of a choice the table holds is sent once as `46 05 <kit> <item> <item>
  <pattern | size << 8 | colour << 16 | brightness << 24 | 52h << 56>` through send_apply_decal
  with (the lobby client, the kit, the item, the item, the value's halves), the inventory's
  records as they were; a pattern, size or colour past its list, a brightness over 10, an item
  the table or the inventory lacks, too few arguments, a string, after a message that did not
  parse: nothing sent; each returns without the native dispatch, which the original reaches.
  srp_set_upgrade_value (patch 207, the chain's first) still sends its own.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EBP, \
    UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_upgrade_value_026g as uv

ON_MESSAGE, JUMP = 0x274A00, 0x274A3F
ITEM_SITE, ITEM_STOP = 0x28867F, 0x288693
CREATE_OBJECT, CREATE_ARRAY, SET_MEMBER, SET_ELEMENT, RELEASE = 0x511700, 0x511710, 0x7C6A20, 0x798340, 0x78E610
AS3 = 0x8FC288
EXTRA = 0x40900000
READER, BODY, IFACE, HOLDER, VIEW, MOVIE, FRAME = (EXTRA + 0x1000 * i for i in range(7))
ITEM_OBJECT = 0x7000
COBRA, NCSTAR, PLAIN = 2108, 2113, 2101
# the account's scope items (of the table) and an item of no scope of it
SCOPE, NC_SCOPE, OTHER = 90, 91, 92
SIZES = (1.0, 0.75, 0.5, 0.35)
RED, GREEN, BLUE = ("red", (1.0, 0.0, 0.0)), ("green", (0.0, 1.0, 0.0)), ("blue", (0.0, 0.0, 1.0))


def text(value):
    return bytes([len(value)]) + value.encode()


def scope(dict_id, patterns, colours):
    out = struct.pack("<HB", dict_id, len(patterns))
    for name in patterns:
        out += text(name) + text("weapons/reticles_069/" + name)
    out += bytes([len(SIZES)]) + b"".join(struct.pack("<f", s) for s in SIZES)
    out += bytes([len(colours)])
    for name, rgb in colours:
        out += text(name) + struct.pack("<3f", *rgb)
    return out


def message(items):
    """A9's body as the server writes it (survarium::v026g::lobby::reticles)."""
    out = bytes([2]) + scope(COBRA, ["dot", "cobra_cross"], [RED, GREEN]) \
        + scope(NCSTAR, ["dot", "ncstar_bullseye", "ncstar_starburst"], [RED, GREEN, BLUE])
    out += struct.pack("<H", len(items))
    for item, dict_id, numbers in items:
        out += struct.pack("<IH", item, dict_id) + bytes(numbers)
    return out


GOOD = message([(SCOPE, COBRA, (1, 2, 1, 9)), (NC_SCOPE, NCSTAR, (0, 0, 0, 6))])


def value(pattern, size, colour, brightness):
    return pattern | size << 8 | colour << 16 | brightness << 24 | 0x52 << 56


def inventory():
    return uv.inventory() + [(SCOPE, COBRA, []), (NC_SCOPE, NCSTAR, []), (OTHER, PLAIN, [])]


class Run(uv.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(EXTRA, 0x10000, UC_PROT_ALL)
        for at in (CREATE_OBJECT, CREATE_ARRAY, SET_MEMBER, SET_ELEMENT, RELEASE):
            self.emu.hook_add(UC_HOOK_CODE, self.gfx, begin=at, end=at)
        self.word(IFACE, AS3)
        self.made, self.members, self.elements, self.released, self.next = [], {}, {}, [], 0x100

    def value_at(self, at):
        kind, low = self.read(at + 4), self.read(at + 8)
        if kind & 0x3F == 4:
            return low
        if kind & 0x3F == 6:
            return self.text(low).decode()
        return ("ref", low)

    def gfx(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address in (CREATE_OBJECT, CREATE_ARRAY):   # thiscall (value, ...): ret 10h, ret 4
            kind = 0x48 if address == CREATE_OBJECT else 0x49
            self.next += 1
            self.emu.mem_write(args[0], struct.pack("<IIII", IFACE, kind, self.next, 0))
            self.made.append(("object" if kind == 0x48 else "array", self.next, emu.reg_read(UC_X86_REG_ECX)))
            pops = 0x10 if address == CREATE_OBJECT else 4
        elif address == SET_MEMBER:                    # (data, name, value, display): ret 10h, al 1
            self.members.setdefault(args[0], {})[self.text(args[1]).decode()] = self.value_at(args[2])
            emu.reg_write(UC_X86_REG_EAX, 1)
            pops = 0x10
        elif address == SET_ELEMENT:                   # (data, index, value): ret 0Ch, al 1
            self.elements.setdefault(args[0], {})[args[1]] = self.value_at(args[2])
            emu.reg_write(UC_X86_REG_EAX, 1)
            pops = 0xC
        else:                                          # ObjectRelease (value, data): ret 8
            self.released.append(args[1])
            pops = 8
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def dispatch(self, id, body):
        """on_message_received(id, reader) from its start: where it stopped, what it popped, the
        registers there."""
        self.emu.mem_write(BODY, body)
        self.emu.mem_write(READER, struct.pack("<III", BODY, BODY, len(body)))
        sp = hide.STACK + 0x8000
        self.emu.mem_write(hide.STACK, bytes(0x10000))
        for i, word in enumerate((hide.SENTINEL, id, READER)):
            self.word(sp + 4 * i, word)
        regs, esp = self.run(ON_MESSAGE, (hide.SENTINEL, JUMP), sp, {**hide.GAME, UC_X86_REG_ECX: uv.LOBBY})
        return self.stopped, esp - sp, regs

    def item(self, index):
        """setup_inventory_data at 0x28867F with the inventory's item `index`, to its SetElement call:
        the gfx calls, the registers there."""
        self.word(uv.MENU + 0x690, HOLDER)
        self.word(HOLDER + 0x108, VIEW)
        self.word(VIEW + 4, MOVIE)
        ebp = FRAME + 0x800
        self.emu.mem_write(FRAME, bytes(0x1000))
        self.emu.mem_write(ebp - 0x6C, struct.pack("<IIII", IFACE, 0x48, ITEM_OBJECT, 0))
        self.word(ebp - 0x54, IFACE)
        self.made, self.members, self.elements, self.released = [], {}, {}, []
        regs, esp = self.run(ITEM_SITE, (ITEM_STOP,), hide.STACK + 0x8000,
                             {**hide.GAME, UC_X86_REG_EDI: uv.INVENTORY + 0xB4 * index, UC_X86_REG_EBX: uv.MENU,
                              UC_X86_REG_EBP: ebp})
        assert self.stopped == ITEM_STOP
        return list(self.made), dict(self.members), dict(self.elements), list(self.released), regs, esp


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(bytes(built))
    for r in (stock, ours):
        r.lobby(inventory())
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    kept = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
    entry = tuple(hide.GAME[r] for r in kept)

    print("the message (lobby_client::on_message_received 0x274A00)")

    def message_case(what, id, body, taken):
        a, b = stock.dispatch(id, body), ours.dispatch(id, body)
        stock_jumps = a[0] == JUMP and a[2][UC_X86_REG_ECX] == id - 0x80
        if taken:
            ok = b[0] == hide.SENTINEL and b[1] == 4 + 8 and tuple(b[2][r] for r in kept) == entry
        else:
            ok = b[0] == JUMP and a[1] == b[1] and all(a[2][r] == b[2][r] for r in a[2])
        report(ok and stock_jumps, what)

    message_case("A9 (srp's reticles) taken, the function returns; the original jumps through its table",
                 0xA9, GOOD, True)
    message_case("8B (the money) to its case, as the original", 0x8B, b"\x10\x27\0\0", False)
    message_case("A8 (the premium store) to its case, as the original", 0xA8, b"", False)

    print("the window's objects (setup_inventory_data at 0x28867F)")
    index = {id: i for i, (id, _, _) in enumerate(inventory())}

    def item_case(what, item, want):
        a, b = stock.item(index[item]), ours.item(index[item])
        same = a[5] == b[5] and all(a[4][r] == b[4][r] for r in a[4]) and a[4][UC_X86_REG_EAX] == SET_ELEMENT
        stock_quiet = a[:4] == ([], {}, {}, [])
        made, members, elements, released, _, _ = b
        if want is None:
            ok = (made, members, elements, released) == ([], {}, {}, [])
        else:
            pattern, size, colour, brightness, patterns, sizes, colours = want
            kinds = [kind for kind, _, _ in made]
            movie_ok = all(movie == MOVIE for _, _, movie in made)
            obj, pat, col = (data for _, data, _ in made) if len(made) == 3 else (None, None, None)
            ok = kinds == ["object", "array", "array"] and movie_ok \
                and members.get(ITEM_OBJECT) == {"srp_reticle": ("ref", obj)} \
                and members.get(obj) == {"pattern": pattern, "size": size, "colour": colour,
                                         "brightness": brightness, "sizes": sizes,
                                         "patterns": ("ref", pat), "colours": ("ref", col)} \
                and elements.get(pat) == dict(enumerate(patterns)) \
                and elements.get(col) == dict(enumerate(colours)) \
                and sorted(released) == sorted([obj, pat, col])
        report(ok and same and stock_quiet,
               f"{what}: {members.get(ITEM_OBJECT, 'nothing')}; the registers at the items' SetElement the original's")

    item_case("the Cobra scope item", SCOPE, (1, 2, 1, 9, ["dot", "cobra_cross"], 4, ["red", "green"]))
    item_case("the NcSTAR scope item, its default", NC_SCOPE,
              (0, 0, 0, 6, ["dot", "ncstar_bullseye", "ncstar_starburst"], 4, ["red", "green", "blue"]))
    item_case("a scope item of no list (PSO-1)", OTHER, None)
    item_case("a weapon", uv.A, None)

    print("srp_set_reticle (lobby_menu::scaleform_callback 0x28C2C0)")
    uint = lambda n: (uv.GFX_UINT, n)
    event = lambda count: (uv.LOGIC, uv.CALLER, uv.TEXT, uv.ARGS, count, 0)

    def call_case(what, method, arguments, want):
        results = []
        for r in (stock, ours):
            records = {i: r.records(i) for i in (uv.A, uv.B, uv.C, SCOPE)}
            results.append((r.call(method, arguments), records))
        (a, _), (b, before) = results
        popped, regs, events, begun, sent, applied, copies, _ = b
        native = a[0] == 4 + 0x10 and a[2] == [event(len(arguments))] and not (a[3] or a[4] or a[5])
        taken = popped == 4 + 0x10 and events == [] and tuple(regs[r] for r in kept) == entry \
            and regs[UC_X86_REG_EAX] == 1
        if want is None:
            ok = not (begun or sent or applied or copies)
        else:
            item, number = want
            ok = begun == [uv.LOBBY + 8] and sent == [(uv.LOBBY + 8, uv.request(item, item, number))] \
                and applied == [(uv.LOBBY, uv.KIT, item, item, number & 0xFFFFFFFF, number >> 32)] \
                and set(copies) <= {uv.LOBBY}
        ok = ok and all(ours.records(i) == before[i] for i in before)
        report(ok and taken and native,
               f"{what}: {'request ' + sent[0][1].hex(' ') if sent else 'nothing sent'}; the original dispatches it")

    method = "srp_set_reticle"
    call_case("the Cobra item to the cross, size 3, green, 10", method,
              tuple(map(uint, (SCOPE, 1, 3, 1, 10))), (SCOPE, value(1, 3, 1, 10)))
    call_case("the NcSTAR item to the starburst, blue, 0, as Numbers", method,
              ((uv.GFX_NUMBER, float(NC_SCOPE)),) + tuple((uv.GFX_NUMBER, float(n)) for n in (2, 0, 2, 0)),
              (NC_SCOPE, value(2, 0, 2, 0)))
    for what, args in (("a pattern past the Cobra's two", (SCOPE, 2, 0, 0, 6)),
                       ("a size past the four", (SCOPE, 0, 4, 0, 6)),
                       ("a colour past the Cobra's two", (SCOPE, 0, 0, 2, 6)),
                       ("a brightness 11", (SCOPE, 0, 0, 0, 11)),
                       ("an item of no list", (OTHER, 0, 0, 0, 6)),
                       ("an item the inventory lacks", (999, 0, 0, 0, 6))):
        call_case(f"{what}: nothing sent", method, tuple(map(uint, args)), None)
    call_case("four arguments: nothing sent", method, tuple(map(uint, (SCOPE, 0, 0, 0))), None)
    call_case("a string as the item: nothing sent", method,
              ((uv.GFX_STRING, "90"),) + tuple(map(uint, (0, 0, 0, 6))), None)
    results = []
    for r in (stock, ours):
        r.lobby(inventory())
        results.append(r.call("srp_set_upgrade_value", (uint(uv.A), uint(uv.SIGHT_A), uint(1))))
    sent = results[1][4]
    report(sent == [(uv.LOBBY + 8, uv.request(uv.A, uv.SIGHT_A, 1))] and results[0][2] == [event(3)],
           "srp_set_upgrade_value (patch 207's, first in the chain): still its own 46 05")

    print("a message that does not parse leaves the table empty")
    message_case("A9 one byte short taken", 0xA9, GOOD[:-1], True)
    item_case("the Cobra scope item after it", SCOPE, None)
    call_case("srp_set_reticle after it: nothing sent", method, tuple(map(uint, (SCOPE, 1, 3, 1, 10))), None)
    message_case("A9 again, whole", 0xA9, GOOD, True)
    item_case("the Cobra scope item again", SCOPE, (1, 2, 1, 9, ["dot", "cobra_cross"], 4, ["red", "green"]))

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
