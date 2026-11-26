#!/usr/bin/env python3
"""Patch 202 (mouse_acquire): a line in the patches' log when the game cannot
take the mouse, and when it has it again (client_patch.md section 3.106).

    python check_mouse_acquire_026g.py <original survarium.exe>

mouse::execute (0x3B9680) runs whole, in the original and in the patched exe,
on a made-up platform mouse whose DirectInput device is stubbed:
GetDeviceState and Acquire answer each poll's codes in turn. The engine's
"mouse device is lost" line (its filter 0x111860, log_message 0x111ED0, the
functor's 0x11140 and 0x12640) is stubbed, each leaving a failed HRESULT's
look in EAX. kernel32's CreateFileA, WriteFile and CloseHandle (the patches'
log) record the lines; GetModuleHandleA and GetProcAddress give user32's
GetForegroundWindow, a stub answering the window in front.

In every poll both exes make the same device and engine calls in the same
order, leave the same mouse state (+4..+24h) and its lock free, return
(ESP) and keep EBX ESI EDI EBP. The original logs nothing; the patched exe:
1. a read: nothing;
2. a lost device whose Acquire is refused: the code's name, the code, the
   level and the window in front; refused again: nothing; another code: a line;
3. a read again: "taken again", once;
4. Acquire taken, the read after it lost again (the engine's line, filtered
   or written): nothing, whatever EAX holds;
5. a read failing with a code execute answers with no Acquire, the first
   read or the one after an Acquire: a line, a code without a name in hex.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

EXECUTE, MOUSE_VTABLE = 0x3B9680, 0xA25518
LOG_FILTERED = 0x1F1D60C                # not 0: execute asks the filter before its line
# engine functions the lost-device line calls (cdecl) -> (name, EAX they leave)
ENGINE = {0x111860: ("log_passes", None), 0x111ED0: ("log_message", 0x80BAD001),
          0x11140: ("functor_drop", 0x80BAD002), 0x12640: ("functor_manager", 0x80BAD003)}
# kernel32 slots -> (name, stack arguments)
IMPORTS = {0x870118: ("GetCurrentThreadId", 0), 0x870048: ("CreateFileA", 7), 0x870108: ("WriteFile", 5),
           0x870040: ("CloseHandle", 1), 0x87003C: ("GetModuleHandleA", 1), 0x870084: ("GetProcAddress", 2)}
DEVICE_SLOTS = {0x1C: ("Acquire", 1), 0x24: ("GetDeviceState", 3)}

OK, S_FALSE = 0, 1
OTHERAPPHASPRIO, E_HANDLE, NOTACQUIRED, NOTINITIALIZED, INPUTLOST = (
    0x80070005, 0x80070006, 0x8007000C, 0x80070015, 0x8007001E)
UNNAMED = 0x80001234

DATA = 0x30000000
MOUSE, DEVICE, DEVICE_VTABLE = DATA, DATA + 0x1000, DATA + 0x2000
STUBS = DATA + 0x10000
RETURN, FOREGROUND = STUBS, STUBS + 0x10
IMPORT_STUBS, DEVICE_STUBS = STUBS + 0x100, STUBS + 0x400
USER32, WINDOW, OTHER_WINDOW = 0x75000000, 0x00010042, 0x00020040
STACK = 0x21000000
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x20000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        self.stubs = {}
        for i, (slot, (name, count)) in enumerate(sorted(IMPORTS.items())):
            self.stub_at(IMPORT_STUBS + 0x10 * i, name, count)
            self.word(slot, IMPORT_STUBS + 0x10 * i)
        for i, (slot, (name, count)) in enumerate(sorted(DEVICE_SLOTS.items())):
            self.stub_at(DEVICE_STUBS + 0x10 * i, name, count)
            self.word(DEVICE_VTABLE + slot, DEVICE_STUBS + 0x10 * i)
        self.stub_at(FOREGROUND, "GetForegroundWindow", 0)
        for address in ENGINE:
            emu.hook_add(UC_HOOK_CODE, self.engine, begin=address, end=address)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=RETURN, end=RETURN)
        self.word(MOUSE, MOUSE_VTABLE)
        self.word(MOUSE + 0x24, WINDOW)
        self.word(MOUSE + 0x28, DEVICE)
        self.word(DEVICE, DEVICE_VTABLE)
        self.lines = []

    def stub_at(self, address, name, count):
        self.stubs[address] = (name, count)
        self.emu.hook_add(UC_HOOK_CODE, self.stub, begin=address, end=address)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def string(self, at):
        out = bytearray()
        while (b := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(b)
        return out.decode()

    def stub(self, emu, address, size, _):
        name, count = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 + 4 * i) for i in range(count)]
        result, record = 0, True
        if name == "GetCurrentThreadId":
            result, record = 1, False
        elif name == "CreateFileA":
            result, record = 0x1234, False
        elif name == "WriteFile":
            self.lines.append(bytes(emu.mem_read(args[1], args[2])).decode())
            self.word(args[3], args[2])
            result, record = 1, False
        elif name == "CloseHandle":
            result, record = 1, False
        elif name == "GetModuleHandleA":
            args = [self.string(args[0]).lower()]
            result = USER32 if args == ["user32.dll"] else 0
        elif name == "GetProcAddress":
            args = [args[0], self.string(args[1])]
            result = FOREGROUND if args == [USER32, "GetForegroundWindow"] else 0
        elif name == "GetForegroundWindow":
            result = self.front
        elif name == "Acquire":
            assert args == [DEVICE], args
            result = self.acquires.pop(0)
        elif name == "GetDeviceState":
            device, size_, buffer = args
            assert (device, size_) == (DEVICE, 0x14), args
            result = self.reads.pop(0)
            if result == OK:
                self.state = bytes((self.polls * 7 + i) & 0xFF for i in range(0x14))
                emu.mem_write(buffer, self.state)
            args = [device, size_]
        if record:
            self.calls.append((name, *args))
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def engine(self, emu, address, size, _):
        """A cdecl engine function of the lost-device line: recorded, returns."""
        name, eax = ENGINE[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        if name == "log_passes":
            eax = 0x80BAD000 | self.passes
        self.calls.append((name,))
        emu.reg_write(UC_X86_REG_EAX, eax)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def poll(self, reads, acquires=(), exclusive=1, front=WINDOW, filtered=0, passes=0):
        """One execute: the device's answers in turn, the mouse's level, the
        window in front, the engine line's filter. Its calls, the mouse's
        state after it, and the lines it logged."""
        self.reads, self.acquires = list(reads), list(acquires)
        self.front, self.passes = front, passes
        self.polls = getattr(self, "polls", 0) + 1
        self.calls, self.lines = [], []
        self.emu.mem_write(MOUSE + 0x34, bytes([exclusive]))
        self.word(LOG_FILTERED, filtered)
        sp = STACK + 0x8000
        self.word(sp, RETURN)
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ECX, MOUSE)
        self.emu.reg_write(UC_X86_REG_EAX, 0xA0A0A0A0)
        self.emu.reg_write(UC_X86_REG_EDX, 0xD0D0D0D0)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(EXECUTE, 0, count=2000000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 4, "ret"
        for reg, value in KEPT.items():
            assert self.emu.reg_read(reg) == value, hex(reg)
        assert self.read(MOUSE + 0x30) == 0, "the lock released"
        assert not self.reads and not self.acquires, (self.reads, self.acquires)
        calls = [c for c in self.calls if c[0] not in ("GetModuleHandleA", "GetProcAddress", "GetForegroundWindow")]
        return calls, bytes(self.emu.mem_read(MOUSE + 4, 0x20)), self.lines


def line(text):
    return f"srp: mouse: {text}\r\n"


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))

    def poll(what, want, *args, **kwargs):
        o_calls, o_mouse, o_lines = old.poll(*args, **kwargs)
        n_calls, n_mouse, n_lines = new.poll(*args, **kwargs)
        assert (n_calls, n_mouse) == (o_calls, o_mouse), (what, o_calls, n_calls)
        assert o_lines == [], (what, o_lines)
        assert n_lines == [line(w) for w in want], (what, n_lines)
        print(f"ok {what}" + "".join(f"\n     {w}" for w in want))

    poll("1. a read: nothing", [], [OK])
    poll("2. lost, Acquire refused, exclusive, another window in front",
         ["cannot take the mouse: DIERR_OTHERAPPHASPRIO (0x80070005), exclusive, another window in front"],
         [NOTACQUIRED], [OTHERAPPHASPRIO], front=OTHER_WINDOW)
    poll("2. refused again: nothing", [], [INPUTLOST], [OTHERAPPHASPRIO], front=OTHER_WINDOW)
    poll("2. refused with another code, non-exclusive, the game in front",
         ["cannot take the mouse: E_HANDLE (0x80070006), non-exclusive, the game in front"],
         [INPUTLOST], [E_HANDLE], exclusive=0)
    poll("3. a read again", ["taken again"], [OK])
    poll("3. and again: nothing", [], [OK])
    poll("4. Acquire taken, lost again, the engine's line filtered out: nothing", [],
         [INPUTLOST, NOTACQUIRED], [OK], filtered=1, passes=0)
    poll("4. the same, the engine's line written: nothing", [],
         [NOTACQUIRED, INPUTLOST], [S_FALSE], filtered=1, passes=1)
    poll("4. the same, the engine's line unfiltered: nothing", [], [NOTACQUIRED, NOTACQUIRED], [OK])
    poll("5. a read failing with another code, no window in front",
         ["cannot read the mouse: DIERR_NOTINITIALIZED (0x80070015), exclusive, no window in front"],
         [NOTINITIALIZED], front=0)
    poll("5. after an Acquire, a code without a name",
         ["cannot read the mouse: 0x80001234, exclusive, the game in front"],
         [INPUTLOST, UNNAMED], [S_FALSE])
    poll("5. then refused", ["cannot take the mouse: DIERR_OTHERAPPHASPRIO (0x80070005), exclusive, the game in front"],
         [NOTACQUIRED], [OTHERAPPHASPRIO])
    poll("3. Acquire taken, read", ["taken again"], [INPUTLOST, OK], [OK])
    print("ok every poll made the original's calls, left its mouse state, returned as it, kept EBX ESI EDI EBP")


if __name__ == "__main__":
    main()
