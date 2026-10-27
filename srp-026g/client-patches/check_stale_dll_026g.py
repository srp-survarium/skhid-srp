#!/usr/bin/env python3
"""Run patch 25's sign-in check (the Rust patch stale_dll, until its port
stale_dll_026g.asm) in the default build on made-up machines.

    python check_stale_dll_026g.py <original survarium.exe>

Requires Python `unicorn`. The hook at 0x380889 and the code it calls run
as machine code, the exe's sections loaded as they are (the exe is never
rebased, and the Rust arena's code holds absolute addresses: the asm's run
1 MiB higher went with the asm); GetModuleHandleA, GetProcAddress, GetTempPathA,
CreateFileA and CloseHandle are stubs. Each machine must give its flags in
byte 8 of the version field (80h, 81h with %TEMP%\\shell_32.dll), zeros in
bytes 9..11, the EULA byte in 12, every register as the game had it but
eax, the stack as it was, and every handle closed; CreateFileA is asked
for no access to the file. Says nothing about what the real Windows or
Wine answers.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

SITE, ORIGINAL = 0x380889, bytes.fromhex("894708" "88570c")    # sites_026g.tsv sign_in.version_tail
AFTER = SITE + len(ORIGINAL)
IMPORTS = {0x87003C: "GetModuleHandleA", 0x870040: "CloseHandle", 0x870048: "CreateFileA",
           0x870084: "GetProcAddress"}
ARGUMENTS = {"GetModuleHandleA": 1, "CloseHandle": 1, "CreateFileA": 7, "GetProcAddress": 2,
             "GetTempPathA": 2}
STUBS, MESSAGE, STACK = 0x30000000, 0x31000000, 0x32000000
KERNEL32 = 0x40100000
TEMP = "C:\\Users\\player\\AppData\\Local\\Temp\\"
GAME = {UC_X86_REG_ECX: 0x11111111, UC_X86_REG_EBX: 0x22222222, UC_X86_REG_EBP: 0x0019FF00,
        UC_X86_REG_ESI: 0x33333333}
EULA = 0x5A


class Run:
    def __init__(self, data, delta, files, temp=TEMP, temp_path=True):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        self.files, self.temp, self.temp_path = set(files), temp, temp_path
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva + delta
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (STUBS, MESSAGE, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.stubs = {STUBS + 0x10 * i: name for i, name in enumerate(ARGUMENTS)}
        self.address = {name: at for at, name in self.stubs.items()}
        for slot, name in IMPORTS.items():
            self.word(slot + delta, self.address[name])
        self.delta, self.opened, self.closed = delta, [], []
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=STUBS, end=STUBS + 0xFFF)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def text(self, at):
        out = bytearray()
        while (byte := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(byte)
        return out.decode("ascii")

    def stub(self, emu, address, size, _):
        name = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 + 4 * i) for i in range(ARGUMENTS[name])]
        result = getattr(self, name)(*args)
        # stdcall: the callee pops its arguments; ecx and edx are not kept.
        emu.reg_write(UC_X86_REG_EAX, result & 0xFFFFFFFF)
        emu.reg_write(UC_X86_REG_ECX, 0xDEAD0001)
        emu.reg_write(UC_X86_REG_EDX, 0xDEAD0002)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * len(args))

    def GetModuleHandleA(self, name):
        return KERNEL32 if self.text(name).lower() == "kernel32.dll" else 0

    def GetProcAddress(self, module, name):
        assert (module, self.text(name)) == (KERNEL32, "GetTempPathA")
        return self.address["GetTempPathA"] if self.temp_path else 0

    def GetTempPathA(self, size, buffer):
        path = self.temp.encode("ascii")
        if len(path) + 1 > size:
            return len(path) + 1
        self.emu.mem_write(buffer, path + b"\0")
        return len(path)

    def CreateFileA(self, path, access, share, security, disposition, attributes, template):
        assert (access, disposition) == (0, 3), "no access, an existing file only"
        if self.text(path) not in self.files:
            return 0xFFFFFFFF
        self.opened.append(0x100 + 4 * len(self.opened))
        return self.opened[-1]

    def CloseHandle(self, handle):
        assert handle in self.opened and handle not in self.closed
        self.closed.append(handle)
        return 1

    def go(self):
        emu = self.emu
        field = b"0.26g0\0" + bytes([base.REVISION])  # bytes 0..7 as the sign-in has them
        emu.mem_write(MESSAGE + 0x100, field + b"\xEE" * 8)
        for register, value in GAME.items():
            emu.reg_write(register, value)
        emu.reg_write(UC_X86_REG_EAX, 0)                # `xor eax,eax` at 0x380868
        emu.reg_write(UC_X86_REG_EDX, 0x77777700 | EULA)
        emu.reg_write(UC_X86_REG_EDI, MESSAGE + 0x100)
        emu.reg_write(UC_X86_REG_ESP, STACK + 0x8000)
        emu.emu_start(SITE + self.delta, AFTER + self.delta, count=100000)
        message = bytes(emu.mem_read(MESSAGE + 0x100, 16))
        expected = GAME | {UC_X86_REG_EDX: 0x77777700 | EULA, UC_X86_REG_EDI: MESSAGE + 0x100,
                           UC_X86_REG_ESP: STACK + 0x8000}
        kept = {register: emu.reg_read(register) for register in expected}
        assert kept == expected, f"registers changed: {kept} != {expected}"
        assert message[:8] == field and message[9:12] == b"\0\0\0" and message[12] == EULA \
            and message[13:] == b"\xEE" * 3, message.hex()
        assert self.closed == self.opened, "a handle left open"
        assert emu.reg_read(UC_X86_REG_EAX) == message[8]
        return message[8]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    upload = open(base.default_bin("section"), "rb").read()
    data, _ = base.build(original, "section", upload, None, open(base.late_join_bin(), "rb").read())
    without, _ = base.build(original, "section", upload)
    stale = TEMP + "shell_32.dll"
    long_temp = "C:\\" + "t" * 254 + "\\"        # 258 letters: the name still fits
    too_long = "C:\\" + "t" * 255 + "\\"         # 259: it does not
    machines = [
        ("no shell_32.dll", dict(files=()), 0x80),
        ("%TEMP%\\shell_32.dll", dict(files={stale}), 0x81),
        ("shell_32.dll elsewhere", dict(files={"C:\\shell_32.dll", TEMP + "shell32.dll"}), 0x80),
        ("no GetTempPathA", dict(files={stale}, temp_path=False), 0x80),
        ("a temporary folder of 258 letters", dict(files={long_temp + "shell_32.dll"}, temp=long_temp), 0x81),
        ("a temporary folder too long for the name", dict(files={too_long + "shell_32.dll"}, temp=too_long), 0x80),
    ]
    failed = 0
    for delta in (0,):
        for name, machine, flags in machines:
            got = Run(data, delta, **machine).go()
            ok = got == flags
            failed += not ok
            print(f"{'ok  ' if ok else 'FAIL'} {name}{' (rebased)' if delta else ''}: {got:#04x}"
                  + ("" if ok else f", not {flags:#04x}"))
    at = base.file_offset(SITE)
    hook = [h for h in base.rust_hooks(original) if h[0] == SITE]
    ok = len(hook) == 1 and data[at:at + len(ORIGINAL)] != ORIGINAL and data[at] == 0xE9 \
        and without[at:at + len(ORIGINAL)] == ORIGINAL
    failed += not ok
    print(f"{'ok  ' if ok else 'FAIL'} {SITE:#x}: the Rust lohook's `jmp` + nop in the default build, the "
          "original in the build without the late join")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
