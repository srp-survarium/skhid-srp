#!/usr/bin/env python3
"""Patch 125 (module_report): the watch Windows calls from the process's
first instruction on (docs/notes/module-report.md; module_watch_026g.asm,
rust/patches/src/vostok/module_watch.rs).

    python check_module_report_026g.py <original survarium.exe>

The built exe's own code runs (Unicorn), kernel32's and ntdll's functions
stubbed:
1. the TLS directory's AddressOfCallBacks names module_watch_callbacks, a
   list of module_watch_tls and its terminator (the original's list was the
   terminator alone);
2. module_watch_tls called as Windows calls a TLS callback (stdcall: the
   module, the reason, reserved), from each of the four stack skews: it pops
   its three arguments and keeps every register and flag;
3. the process's call (reason 1) registers module_watch_dll with
   LdrRegisterDllNotification, once, a second one not again;
4. a thread's call (reason 2) and a thread or the process ending (3, 0)
   call nothing, before the process's call and after it;
5. module_watch_dll called as the loader calls a notification (stdcall: the
   reason, the data, the context): it pops its three arguments and keeps
   every register and flag; a DLL loaded outside the Windows folder is kept
   by name in the watch's state, with its header's link time (read with
   ReadProcessMemory), one of the Windows folder is not, nor any other
   reason's (an unload is 2).

Not covered here: the report's walks (the modules, the code that is no
module, the windows over the game), which need a game.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

TLS_DIRECTORY = 0xA365A8
IMPORTS = {0x87003C: ("GetModuleHandleA", 1), 0x870084: ("GetProcAddress", 2), 0x870118: ("GetCurrentThreadId", 0),
           0x870090: ("GetTickCount", 0), 0x870168: ("VirtualQuery", 3)}
# what GetProcAddress gives, of kernel32 and of ntdll: stubs of their own
KERNEL32_FUNCTIONS = {"GetModuleFileNameW": 3, "CreateFileW": 7, "SetFilePointer": 4, "CreateToolhelp32Snapshot": 2,
                      "Module32FirstW": 2, "Module32NextW": 2, "GetTempPathW": 2, "GetWindowsDirectoryW": 2,
                      "GetCurrentProcess": 0, "ReadProcessMemory": 5, "LoadLibraryA": 1, "LoadLibraryW": 1,
                      "LoadLibraryExA": 3, "LoadLibraryExW": 3}
NTDLL_FUNCTIONS = {"LdrLoadDll": 4, "NtQueryInformationThread": 5, "LdrRegisterDllNotification": 4}
KERNEL32, NTDLL = 0x7B000000, 0x7BC00000
EXE = "C:\\Games\\Skhid\\game\\binaries\\x86\\survarium.exe"
TEMP = "C:\\users\\ivan\\Temp\\"
WINDOWS = "C:\\windows"

DATA = 0x30000000
STUBS, NOTE = DATA, DATA + 0x2000
RETURN = STUBS + 0xF00
STACK = 0x21000000
PRIVATE = 0x40000000                             # memory VirtualQuery tells as MEM_PRIVATE
KEPT = {UC_X86_REG_EAX: 0xA0A0A0A0, UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ECX: 0x0C0C0C0C,
        UC_X86_REG_EDX: 0x0D0D0D0D, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}
FLAGS = 0x8D5                                    # CF PF AF ZF SF OF


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x20000, UC_PROT_ALL)
        self.stubs, self.functions = {}, {}
        for i, (slot, (name, count)) in enumerate(sorted(IMPORTS.items())):
            self.stubs[STUBS + 0x10 * i] = (name, count)
            emu.mem_write(slot, struct.pack("<I", STUBS + 0x10 * i))
        for module, functions in ((KERNEL32, KERNEL32_FUNCTIONS), (NTDLL, NTDLL_FUNCTIONS)):
            for name, count in sorted(functions.items()):
                at = STUBS + 0x100 + 0x10 * len(self.functions)
                self.stubs[at] = (name, count)
                self.functions[module, name] = at
        for at in self.stubs:
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, lambda emu, *_: emu.emu_stop(), begin=RETURN, end=RETURN)
        self.calls, self.thread_start, self.tick = [], 0, 1000
        self.images = {}                         # what ReadProcessMemory reads: a base to its bytes

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def string(self, at):
        out = bytearray()
        while (b := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(b)
        return out.decode()

    def wide(self, at, text, room):
        """A kernel32 path call's answer: the text and its terminator, the units written."""
        assert len(text) < room
        self.emu.mem_write(at, (text + "\0").encode("utf-16-le"))
        return len(text)

    def stub(self, emu, address, size, _):
        name, count = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 + 4 * i) for i in range(count)]
        result = 0
        if name == "GetModuleHandleA":
            result = {"kernel32.dll": KERNEL32, "ntdll.dll": NTDLL}[self.string(args[0]).lower()]
        elif name == "GetProcAddress":
            result = self.functions.get((args[0], self.string(args[1])), 0)
            assert result, f"GetProcAddress({args[0]:#x}, {self.string(args[1])!r})"
        elif name == "GetCurrentThreadId":
            result = 7
        elif name == "GetTickCount":
            self.tick += 2000
            result = self.tick
        elif name == "GetModuleFileNameW":
            assert args[0] == 0, "only the exe's own path is asked here"
            result = self.wide(args[1], EXE, args[2])
        elif name == "GetTempPathW":
            result = self.wide(args[1], TEMP, args[0])
        elif name == "GetWindowsDirectoryW":
            result = self.wide(args[0], WINDOWS, args[1])
        elif name == "LdrRegisterDllNotification":
            self.calls.append((name, args[0], args[1], args[2]))
            emu.mem_write(args[3], struct.pack("<I", 0xC00C1E))
        elif name == "NtQueryInformationThread":
            self.calls.append((name, args[0], args[1], args[3]))
            emu.mem_write(args[2], struct.pack("<I", self.thread_start))
        elif name == "GetCurrentProcess":
            result = 0xFFFFFFFF
        elif name == "ReadProcessMemory":
            process, at, room, size, read = args
            assert process == 0xFFFFFFFF
            image = next((image[at - start:at - start + size] for start, image in self.images.items()
                          if start <= at and at + size <= start + len(image)), None)
            if image is not None:
                emu.mem_write(room, image)
                emu.mem_write(read, struct.pack("<I", size))
                result = 1
        elif name == "VirtualQuery":
            at, info, length = args
            assert length == 28
            self.calls.append((name, at))
            kind = 0x20000 if PRIVATE <= at < PRIVATE + 0x10000 else 0x1000000
            emu.mem_write(info, struct.pack("<7I", at & ~0xFFF, at & ~0xFFFF, 0x20, 0x1000, 0x1000, 0x20, kind))
            result = 28
        else:
            raise AssertionError(f"{name} is not called by the watch")
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def call(self, function, args, skew=0):
        """`function` called stdcall with `args` from a stack `skew` bytes
        off 16: it returns to its caller with the arguments popped and every
        register and flag as it was. The calls its stubs noted."""
        sp = STACK + 0x10000 + skew
        self.emu.mem_write(sp, struct.pack(f"<{1 + len(args)}I", RETURN, *args))
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202 | FLAGS)
        self.calls = []
        self.emu.emu_start(function, 0, count=20_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN, hex(self.emu.reg_read(UC_X86_REG_EIP))
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 4 * len(args), "the arguments are popped (stdcall)"
        assert {reg: self.emu.reg_read(reg) for reg in KEPT} == KEPT, "every register is kept"
        assert self.emu.reg_read(UC_X86_REG_EFLAGS) & FLAGS == FLAGS, "the flags are kept"
        return self.calls

    def notification(self, path, dll_base, size):
        """An LDR_DLL_NOTIFICATION_DATA for the DLL of the full path `path`."""
        units = path.encode("utf-16-le")
        self.emu.mem_write(NOTE + 0x100, units + b"\0\0")
        self.emu.mem_write(NOTE + 0x40, struct.pack("<HHI", len(units), len(units) + 2, NOTE + 0x100))
        self.emu.mem_write(NOTE, struct.pack("<5I", 0, NOTE + 0x40, NOTE + 0x40, dll_base, size))
        return NOTE


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    tls, dll, callbacks = (linked.symbols[name] for name in ("module_watch_tls", "module_watch_dll",
                                                             "module_watch_callbacks"))
    # the watch's static: in the Rust patches' data or state
    rooms = [linked.blocks[name] for name in ("patches.data", "patches.state")]

    run = Run(bytes(built))
    assert Run(original).read(TLS_DIRECTORY + 0xC) == 0x884AA4 and Run(original).read(0x884AA4) == 0
    assert run.read(TLS_DIRECTORY + 0xC) == callbacks
    assert (run.read(callbacks), run.read(callbacks + 4)) == (tls, 0)
    print(f"ok the TLS directory's callbacks are module_watch_callbacks ({callbacks:#x}): module_watch_tls "
          f"({tls:#x}), then the terminator")

    for skew in range(0, 16, 4):
        assert run.call(tls, [base.IMAGE_BASE, 2, 0], skew) == []
        assert run.call(tls, [base.IMAGE_BASE, 3, 0], skew) == []
    print("ok before the process's call a thread's start calls nothing; from each stack skew the callback pops "
          "its three arguments and keeps every register and flag")

    assert run.call(tls, [base.IMAGE_BASE, 1, 0]) == [("LdrRegisterDllNotification", 0, dll, 0)]
    assert run.call(tls, [base.IMAGE_BASE, 1, 0]) == [], "the watch starts once"
    print(f"ok the process's call registers module_watch_dll ({dll:#x}) with LdrRegisterDllNotification, once")

    for reason in (2, 3, 0):
        assert run.call(tls, [base.IMAGE_BASE, reason, 0]) == []
    print("ok after it a thread's call and an end call nothing")

    def kept(name):
        return any(name.encode() in bytes(run.emu.mem_read(room.va, room.size)) for room in rooms)

    def header(stamp):
        """A 32-bit PE's first bytes, linked at `stamp`."""
        nt = b"PE\0\0" + struct.pack("<HHI", 0x14C, 1, stamp) + bytes(12) + struct.pack("<H", 0x10B) + bytes(66)
        return b"MZ" + bytes(0x3A) + struct.pack("<I", 0x80) + bytes(0x40) + nt

    def held(stamp):
        return any(struct.pack("<I", stamp) in bytes(run.emu.mem_read(room.va, room.size)) for room in rooms)

    run.images = {0x12340000: header(0x4B4B4321), 0x15000000: header(0x5A5A1234)}
    for skew in range(0, 16, 4):
        assert run.call(dll, [0, run.notification("D:\\tools\\never.dll", 0x12340000, 0x5000), 0], skew) == []
        assert run.call(dll, [3, run.notification("D:\\tools\\never.dll", 0x12340000, 0x5000), 0], skew) == []
        assert run.call(dll, [1, run.notification("C:\\Windows\\SysWOW64\\dxgi.dll", 0x70000000, 0x9000), 0], skew) == []
    assert not kept("never.dll") and not kept("dxgi.dll") and not held(0x4B4B4321)
    assert run.call(dll, [1, run.notification("D:\\tools\\overlay.dll", 0x12340000, 0x5000), 0]) == []
    assert run.call(dll, [1, run.notification(EXE.rsplit("\\", 1)[0] + "\\bugtrap.dll", 0x10000000, 0x7000), 0]) == []
    assert kept("overlay.dll") and kept("bugtrap.dll") and held(0x4B4B4321) and not held(0x5A5A1234)
    assert run.call(dll, [2, run.notification("D:\\tools\\gone.dll", 0x15000000, 0x5000), 0]) == []
    assert not kept("gone.dll") and not held(0x5A5A1234)
    print("ok the notification pops its three arguments and keeps every register and flag; a DLL loaded "
          "outside the Windows folder is kept by name with its header's link time, one of the Windows "
          "folder and another reason's (an unload) are not")


if __name__ == "__main__":
    main()
