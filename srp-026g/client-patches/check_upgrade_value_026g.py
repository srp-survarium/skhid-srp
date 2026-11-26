#!/usr/bin/env python3
"""Run patch 207 (rust/patches/src/upgrade_value.rs, its lohook of
upgrade_value.toml: the lobby movie's srp_set_upgrade_value(host, upgrade,
value) becomes the decal editor's lobby request 46 05; client_patch.md 3.111)
in the default build on made-up game objects, the original beside it.

    python check_upgrade_value_026g.py <original survarium.exe>

Requires Python `unicorn`. check_hide_ui_026g.py's harness (the exe's sections
as they are, the arena with them). Stubbed: lobby_menu_logic::call (0x2812E0,
the menu's native dispatch: recorded, al 0, `ret 8`), the lobby link's
message begin (0x37A0E0: a message of ours, its buffer holding the request's
first byte, `ret 4`) and send (0x379640: the message's bytes recorded,
`ret 8`); kernel32's CreateFileA, WriteFile, CloseHandle, GetTickCount and
GetCurrentProcessId (the patches' log: its lines recorded). The game's own
send_apply_decal (0x270950), its buffer growth (0x7AB30), find_item_by_id
(0x277AA0) and copy_account_items (0x270320) run; the entries of the last two
of them are recorded.

lobby_menu::scaleform_callback (0x28C2C0) runs from its start to its return
with ECX a made-up lobby menu's flash external handler (the menu+F0h: its
logic +5B8h, the logic's menu +2A24h, the menu's game +9Ch, the network
client +53ACh, its lobby client +1E0h; the lobby client's active kit
+1EA964h and its inventory +1EA978h..+1EA97Ch of B4h item records, each
record's upgrade records' vector at +60h):

- srp_set_upgrade_value(host, upgrade, 1) and (.., 0), for a weapon whose
  second sight heads its kit (wide kit, its own slot 6) and for one whose kit
  is its scope's and names the second sight by its dict id (beside a decal):
  the request 46 05 <kit> <host> <upgrade> <value as u64> sent once, through
  send_apply_decal with (the lobby client, the kit, the host, the upgrade,
  the value, 0); the host's records then the kit with the new side in bit 55
  (send_apply_decal's own write put the bare value into the record of that
  id: patched over), the other records as they were; copy_account_items run
  after; the movie's ids as GFx uints, ints and Numbers alike;
- a value 2 (and 0xFFFFFFFF): nothing sent, one log line naming the value at
  the first only; an upgrade the host does not hold, a host the inventory
  lacks, an item with no second sight, a string argument, too few
  arguments: nothing sent;
- each of them returns as the function does (`ret 10h`, EBX, ESI, EDI and
  EBP as at the entry, EAX 1) without the native dispatch, which the original
  reaches for each (the menu's logic and the event {movie, name, arguments,
  count, 0}) and sends nothing;
- another method (start_decal_edit): the native dispatch with the same
  `this` and event, the registers and stack as the original's, nothing sent.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EBP, \
    UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide

CALLBACK, DISPATCH = 0x28C2C0, 0x2812E0
SEND_APPLY, BEGIN, SEND, COPY = 0x270950, 0x37A0E0, 0x379640, 0x270320
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT, PROCESS_ID = 0x870048, 0x870108, 0x870040, 0x870090, 0x8701B8
MENU, GAME, NC = 0x40600000, 0x40620000, 0x40000000
HANDLER, LOGIC = MENU + 0xF0, MENU + 0x6A8
LOBBY = NC + 0x1E0
INVENTORY, MESSAGE, BUFFER, DATA = 0x40800000, 0x40810000, 0x40811000, 0x40812000
ARGS, TEXT, CALLER, KERNEL = 0x40700000, 0x40701000, 0x40702000, 0x40703000
KIT = 0x11223344
GFX_INT, GFX_UINT, GFX_NUMBER, GFX_STRING = 3, 4, 5, 6
# the dict ids: an optic, a red dot (the second sights), a decal
OPTIC, RED_DOT, DECAL = 640, 641, 900
# items: a weapon whose kit is its second sight's (A), one whose kit is its scope's naming its second
# sight (B), an armour with a decal (C); the upgrades' own items
A, B, C, SIGHT_A, SIGHT_B, SCOPE_B, DECAL_B, DECAL_C = 1001, 1002, 1003, 72, 71, 70, 80, 81
WIDE = 0xA8 << 56


def kit(own, side, sight=0):
    """A wide kit value: its own slot, the second sight's side, the second sight's dict id named."""
    return WIDE | own << 48 | side << 55 | sight << 36


def inventory(side_a=0, side_b=1):
    """The items: (id, dict, [(upgrade id, dict, value, slot)])."""
    return [
        (SIGHT_B, RED_DOT, []),
        (A, 300, [(SIGHT_A, RED_DOT, kit(6, side_a), 3)]),
        (B, 301, [(DECAL_B, DECAL, 0x1234567890, 0), (SCOPE_B, OPTIC, kit(3, side_b, RED_DOT), 3)]),
        (C, 500, [(DECAL_C, DECAL, 0x55, 0)]),
        (SIGHT_A, RED_DOT, []),
        (SCOPE_B, OPTIC, []),
    ]


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        for at, size in ((NC, 0x200000), (MENU, 0x10000), (GAME, 0x10000), (INVENTORY, 0x10000),
                         (MESSAGE, 0x4000), (ARGS, 0x4000)):
            self.emu.mem_map(at, size, UC_PROT_ALL)
        for at in (DISPATCH, BEGIN, SEND):
            self.emu.hook_add(UC_HOOK_CODE, self.game, begin=at, end=at)
        for at in (SEND_APPLY, COPY):
            self.emu.hook_add(UC_HOOK_CODE, self.entered, begin=at, end=at)
        for n, slot in enumerate((CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, TICK_COUNT, PROCESS_ID)):
            self.word(slot, KERNEL + 0x10 * n)
            self.emu.hook_add(UC_HOOK_CODE, self.kernel32, begin=KERNEL + 0x10 * n, end=KERNEL + 0x10 * n)
        self.lines, self.ticks = [], 0

    def kernel32(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        n = (address - KERNEL) // 0x10
        if n == 0:                         # CreateFileA: a handle
            pops, result = 28, 0x1234
        elif n == 1:                       # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])).decode(errors="replace").strip())
            self.word(args[3], args[2])
            pops, result = 20, 1
        elif n == 2:                       # CloseHandle
            pops, result = 4, 1
        elif n == 3:                       # GetTickCount: a second on at each call
            self.ticks += 1000
            pops, result = 0, self.ticks
        else:                              # GetCurrentProcessId
            pops, result = 0, 4242
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def game(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == DISPATCH:            # stdcall (logic, event), al the handler's
            event = self.read(sp + 8)
            self.events.append((self.read(sp + 4),) + tuple(self.read(event + 4 * i) for i in range(5)))
            emu.reg_write(UC_X86_REG_EAX, 0)
            pops = 8
        elif address == BEGIN:             # eax the link (lobby client +8), the request id; ret 4
            self.word(MESSAGE + 0x2BC, BUFFER)
            self.word(BUFFER + 4, DATA)
            self.word(BUFFER + 8, 0x100)
            self.word(BUFFER + 0xC, 1)
            emu.mem_write(DATA, bytes([self.read(sp + 4) & 0xFF]))
            self.begun.append(emu.reg_read(UC_X86_REG_EAX))
            emu.reg_write(UC_X86_REG_EAX, MESSAGE)
            pops = 4
        else:                              # send (link, message), ret 8
            buffer = self.read(self.read(sp + 8) + 0x2BC)
            self.sent.append((self.read(sp + 4), bytes(emu.mem_read(self.read(buffer + 4), self.read(buffer + 0xC)))))
            pops = 8
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def entered(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == SEND_APPLY:
            self.applied.append(tuple(self.read(sp + 4 * i) for i in range(1, 7)))
        else:
            self.copies.append(emu.reg_read(UC_X86_REG_ESI))

    def lobby(self, items):
        self.emu.mem_write(MENU, bytes(0x10000))
        self.word(MENU + 0x9C, GAME)
        self.word(LOGIC + 0x2A24, MENU)
        self.word(GAME + 0x53AC, NC)
        self.word(LOBBY + 0x1EA964, KIT)
        self.emu.mem_write(LOBBY + 0x1E78A8, b"\0")       # no account items: copy_account_items copies none
        self.emu.mem_write(INVENTORY, bytes(0x10000))
        for i, (id, dict_id, records) in enumerate(items):
            at = INVENTORY + 0xB4 * i
            self.word(at, id)
            self.emu.mem_write(at + 4, struct.pack("<H", dict_id))
            first = at + 0x6C
            for j, (rid, rdict, value, slot) in enumerate(records):
                self.emu.mem_write(first + 0x18 * j, struct.pack("<IIQB", rid, rdict, value, slot))
            self.word(at + 0x60, first)
            self.word(at + 0x64, first + 0x18 * len(records))
            self.word(at + 0x68, first + 0x18 * 3)
        self.word(LOBBY + 0x1EA978, INVENTORY)
        self.word(LOBBY + 0x1EA97C, INVENTORY + 0xB4 * len(items))
        self.items = len(items)

    def records(self, id):
        for i in range(self.items):
            at = INVENTORY + 0xB4 * i
            if self.read(at) == id:
                first, end = self.read(at + 0x60), self.read(at + 0x64)
                return [struct.unpack("<IIQB", self.read(first + 0x18 * j, 0x11))
                        for j in range((end - first) // 0x18)]
        return None

    def call(self, method, arguments):
        """scaleform_callback(caller, method, arguments, count) on the menu's handler: what it popped,
        the registers at its return, the dispatches, the requests begun and sent, the send_apply_decal
        calls, the copies. `arguments`: (GFx type, value) each."""
        self.emu.mem_write(TEXT, method.encode() + b"\0")
        for i, (kind, value) in enumerate(arguments):
            if kind == GFX_STRING:
                self.emu.mem_write(TEXT + 0x100 * (i + 1), value.encode() + b"\0")
                value = struct.pack("<II", TEXT + 0x100 * (i + 1), 0)
            elif kind == GFX_NUMBER:
                value = struct.pack("<d", value)
            else:
                value = struct.pack("<II", value & 0xFFFFFFFF, 0)
            self.emu.mem_write(ARGS + 0x18 * i, struct.pack("<II", 0, kind) + value + bytes(8))
        sp = hide.STACK + 0x8000
        for i, value in enumerate((hide.SENTINEL, CALLER, TEXT, ARGS, len(arguments))):
            self.word(sp + 4 * i, value)
        self.events, self.begun, self.sent, self.applied, self.copies, self.lines = [], [], [], [], [], []
        regs, esp = self.run(CALLBACK, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: HANDLER})
        assert self.stopped == hide.SENTINEL, method
        return esp - sp, regs, list(self.events), list(self.begun), list(self.sent), list(self.applied), \
            list(self.copies), list(self.lines)


def request(host, upgrade, value):
    return bytes([0x46, 5]) + struct.pack("<IIIQ", KIT, host, upgrade, value)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    kept = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
    entry = tuple(hide.GAME[r] for r in kept)
    method = "srp_set_upgrade_value"
    event = lambda count: (LOGIC, CALLER, TEXT, ARGS, count, 0)
    uint = lambda value: (GFX_UINT, value)

    def native(a, count):
        """The original: the dispatch with the event, nothing sent, the stack as the function leaves it."""
        popped, regs, events, begun, sent, applied, copies, _ = a
        return popped == 4 + 0x10 and events == [event(count)] and not (begun or sent or applied or copies) \
            and tuple(regs[r] for r in kept) == entry

    def taken(b):
        popped, regs, events, *_ = b
        return popped == 4 + 0x10 and events == [] and tuple(regs[r] for r in kept) == entry \
            and regs[UC_X86_REG_EAX] == 1

    print("srp_set_upgrade_value (lobby_menu::scaleform_callback 0x28C2C0)")
    # (what, the inventory's sides (A, B), the arguments, the request wanted (host, upgrade, value) or
    # None, the host's records after (None: as they were))
    a_kit = lambda side: [(SIGHT_A, RED_DOT, kit(6, side), 3)]
    b_kit = lambda side: [(DECAL_B, DECAL, 0x1234567890, 0), (SCOPE_B, OPTIC, kit(3, side, RED_DOT), 3)]
    cases = [
        ("A (the second sight its kit's own) to the right", (0, 1), (uint(A), uint(SIGHT_A), uint(1)),
         (A, SIGHT_A, 1), (A, a_kit(1))),
        ("A back to the left", (1, 1), (uint(A), uint(SIGHT_A), uint(0)), (A, SIGHT_A, 0), (A, a_kit(0))),
        ("B (the scope's kit names the second sight) to the left, its decal kept", (0, 1),
         (uint(B), uint(SIGHT_B), uint(0)), (B, SIGHT_B, 0), (B, b_kit(0))),
        ("B to the right", (0, 0), (uint(B), uint(SIGHT_B), uint(1)), (B, SIGHT_B, 1), (B, b_kit(1))),
        ("the same side again: sent (the server answers the account)", (0, 1), (uint(A), uint(SIGHT_A), uint(0)),
         (A, SIGHT_A, 0), (A, a_kit(0))),
        ("ids and value as GFx ints", (0, 1), ((GFX_INT, A), (GFX_INT, SIGHT_A), (GFX_INT, 1)),
         (A, SIGHT_A, 1), (A, a_kit(1))),
        ("ids and value as Numbers", (0, 1), ((GFX_NUMBER, float(B)), (GFX_NUMBER, float(SIGHT_B)),
                                              (GFX_NUMBER, 0.0)), (B, SIGHT_B, 0), (B, b_kit(0))),
        ("a value 2: nothing sent", (0, 1), (uint(A), uint(SIGHT_A), uint(2)), None, None),
        ("a value 0xFFFFFFFF: nothing sent", (0, 1), (uint(A), uint(SIGHT_A), uint(0xFFFFFFFF)), None, None),
        ("a Number 0.5: nothing sent", (0, 1), (uint(A), uint(SIGHT_A), (GFX_NUMBER, 0.5)), None, None),
        ("an int -1 as the value: nothing sent", (0, 1), (uint(A), uint(SIGHT_A), (GFX_INT, -1)), None, None),
        ("B's second sight on A: nothing sent", (0, 1), (uint(A), uint(SIGHT_B), uint(1)), None, None),
        ("A's scope item as B's second sight (its scope, not its sight): nothing sent", (0, 1),
         (uint(B), uint(SCOPE_B), uint(0)), None, None),
        ("a host the inventory lacks: nothing sent", (0, 1), (uint(999), uint(SIGHT_A), uint(1)), None, None),
        ("an upgrade the inventory lacks: nothing sent", (0, 1), (uint(A), uint(998), uint(1)), None, None),
        ("C, armour with a decal: nothing sent", (0, 1), (uint(C), uint(DECAL_C), uint(1)), None, None),
        ("a string as the host: nothing sent", (0, 1), ((GFX_STRING, "1001"), uint(SIGHT_A), uint(1)), None, None),
        ("two arguments: nothing sent", (0, 1), (uint(A), uint(SIGHT_A)), None, None),
    ]
    side_lines = 0
    for what, sides, arguments, want, after in cases:
        results = []
        for r in (stock, ours):
            r.lobby(inventory(*sides))
            results.append(r.call(method, arguments))
        a, b = results
        _, _, _, begun, sent, applied, copies, lines = b
        if want is None:
            ok = taken(b) and not (begun or sent or applied or copies) \
                and all(ours.records(i) == stock.records(i) for i in (A, B, C))
        else:
            host, upgrade, value = want
            ok = taken(b) and begun == [LOBBY + 8] and sent == [(LOBBY + 8, request(host, upgrade, value))] \
                and applied == [(LOBBY, KIT, host, upgrade, value, 0)] and copies == [LOBBY, LOBBY] \
                and ours.records(after[0]) == after[1] \
                and all(ours.records(i) == stock.records(i) for i in (A, B, C) if i != after[0])
        said = [line for line in lines if "is no side" in line]
        side_lines += len(said)
        report(ok and native(a, len(arguments)),
               f"{what}: {'request ' + sent[0][1].hex(' ') if sent else 'nothing sent'}"
               f"{'; ' + said[0] if said else ''}; the original dispatches it, sends nothing")
    report(side_lines == 1, f"a value that is no side: {side_lines} log line(s) in the run (one wanted)")

    print("another method: the menu's own dispatch")
    for args in ((uint(A), uint(SIGHT_A)), ()):
        results = []
        for r in (stock, ours):
            r.lobby(inventory())
            results.append(r.call("start_decal_edit", args))
        a, b = results
        same = a[0] == b[0] and a[2] == b[2] and all(a[1][r] == b[1][r] for r in a[1])
        report(native(a, len(args)) and native(b, len(args)) and same,
               f"start_decal_edit with {len(args)} arguments: the dispatch ({b[2]}), registers and stack as "
               "the original's, nothing sent")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
