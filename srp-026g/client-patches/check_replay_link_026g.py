#!/usr/bin/env python3
"""Run the original and patch 29's lobby link callback
(video_tutorial_link_clicked, 0x28CDA0) on made-up links.

    python check_replay_link_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). The handler runs as machine code from its
first instruction, with its owner and the lobby client made up; the Windows
functions it reaches (GetModuleFileNameW, GetModuleHandleA, GetProcAddress,
CreateProcessW, CloseHandle, ShellExecuteA) and enable_element (0x22A660)
are stubs that write down how they were called. The default build of
patch_026g.py must start `<the exe's folder>\\updater_skhid.exe`, the command
line `updater_skhid.exe <link>`, for a well-formed replay link, and start, open or enable
nothing for any other text; the original opens every text. Says nothing
about what Windows does with the start (the updater's own tests do).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP, \
    UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_EFLAGS

import patch_026g as base

HANDLER = base.REPLAY_LINK_HANDLER
ENABLE_ELEMENT = 0x22A660
IAT = {"GetModuleHandleA": 0x87003C, "CloseHandle": 0x870040, "GetModuleFileNameW": 0x870074,
       "GetProcAddress": 0x870084, "ShellExecuteA": 0xAEFE74}
THIS, OWNER, OWNER2, VTABLE = 0x20000000, 0x20001000, 0x20002000, 0x20003000
CLIENT = 0x30000000                           # the lobby client; its link at +1E6144h
STACK, STOP, STUB = 0x21008000, 0x22000000, 0x22000100
KERNEL32 = 0x7C800000
REGISTERS = (UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI)
FOLDER = "C:\\Games\\Survarium\\game\\binaries\\x86"
EXE = FOLDER + "\\survarium_skhid.exe"

ID, TOKEN, SHA = "0a1b2c3d", "0123456789abcdef" * 2, "fedcba9876543210" * 4
VALID = f"replay {ID} {TOKEN} {SHA} 1048576"


class Handler:
    def __init__(self, data, link, module_path=EXE, units=None, kernel32=True, create=True):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(THIS, 0x10000, UC_PROT_ALL)
        emu.mem_map(0x21000000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x10000, UC_PROT_ALL)
        emu.mem_map(CLIENT, 0x1F0000, UC_PROT_ALL)
        for at, value in ((THIS + 0x9C, OWNER), (OWNER + 0x53AC, OWNER2), (OWNER2, VTABLE),
                          (VTABLE + 0x3C, STUB)):
            self.word(at, value)
        for index, (name, slot) in enumerate(IAT.items()):
            self.word(slot, STUB + 0x10 * (index + 1))
        self.stubs = {STUB + 0x10 * (index + 1): name for index, name in enumerate(IAT)}
        self.stubs[STUB] = "client"
        self.stubs[STUB + 0x80] = "CreateProcessW"
        self.stubs[ENABLE_ELEMENT] = "enable_element"
        raw = link if isinstance(link, bytes) else link.encode("utf-8")
        emu.mem_write(CLIENT + base.REPLAY_LINK_BUFFER, raw + b"\0")
        self.module_path, self.units = module_path, units
        self.kernel32, self.create = kernel32, create
        self.calls = []
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def ascii(self, at):
        out = b""
        while (c := bytes(self.emu.mem_read(at + len(out), 1))) != b"\0":
            out += c
        return out.decode("latin-1")

    def wide(self, at):
        out = []
        while (unit := struct.unpack("<H", self.emu.mem_read(at + 2 * len(out), 2))[0]) != 0:
            out.append(unit)
        return struct.pack(f"<{len(out)}H", *out).decode("utf-16-le")

    def returns(self, value, pop=0):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        name = self.stubs.get(address)
        if name is None:
            return
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        # A Windows function may change eax, ecx and edx: make sure the code
        # does not count on them.
        emu.reg_write(UC_X86_REG_ECX, 0xBADC0DE1)
        if name == "client":                                  # thiscall, no argument
            self.returns(CLIENT)
            return
        if name == "GetModuleFileNameW":
            module, buffer, units = arg(0), arg(1), arg(2)
            assert module == 0
            path = self.module_path.encode("utf-16-le")
            self.calls.append((name, units))
            if self.units is not None:                        # Windows fails
                self.returns(self.units, 12)
            elif len(path) // 2 < units:
                emu.mem_write(buffer, path + b"\0\0")
                self.returns(len(path) // 2, 12)
            else:                                             # cut to the buffer, as Windows does
                emu.mem_write(buffer, path[:2 * (units - 1)] + b"\0\0")
                self.returns(units, 12)
        elif name == "GetModuleHandleA":
            self.calls.append((name, self.ascii(arg(0))))
            self.returns(KERNEL32 if self.kernel32 else 0, 4)
        elif name == "GetProcAddress":
            self.calls.append((name, arg(0), self.ascii(arg(1))))
            self.returns(STUB + 0x80 if arg(0) == KERNEL32 else 0, 8)
        elif name == "CreateProcessW":
            startup, process = arg(8), arg(9)
            self.calls.append((name, self.wide(arg(0)), self.wide(arg(1)), arg(2), arg(3), arg(4), arg(5),
                               arg(6), self.wide(arg(7)), self.read(startup),
                               bytes(emu.mem_read(startup + 4, 0x40)) == b"\0" * 0x40))
            if self.create:
                self.word(process, 0x111)
                self.word(process + 4, 0x222)
            self.returns(int(self.create), 40)
        elif name == "CloseHandle":
            self.calls.append((name, arg(0)))
            self.returns(1, 4)
        elif name == "ShellExecuteA":
            self.calls.append((name, self.ascii(arg(1)), self.ascii(arg(2)), arg(3), arg(4), arg(5)))
            self.returns(42, 24)
        elif name == "enable_element":
            self.calls.append((name, self.ascii(arg(0)), arg(1)))
            self.returns(0, 8)

    def run(self):
        emu = self.emu
        for at, value in ((STACK, STOP), (STACK + 4, 0x5A5A5A5A)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, THIS)
        for register in REGISTERS:
            emu.reg_write(register, 0x12345678)
        emu.emu_start(HANDLER, STOP, count=200000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 8, \
            "the handler returns with `ret 4`"
        assert all(emu.reg_read(register) == 0x12345678 for register in REGISTERS), "callee-saved registers"
        assert emu.reg_read(UC_X86_REG_EAX) & 0xFF == 1, "it returns true"
        assert not emu.reg_read(UC_X86_REG_EFLAGS) & 0x400, "the direction flag is clear"
        return self.calls


def started(calls, folder=FOLDER, link=VALID):
    app = folder + "\\updater_skhid.exe"
    return calls == [
        ("GetModuleFileNameW", 0x208),
        ("GetModuleHandleA", "kernel32.dll"),
        ("GetProcAddress", KERNEL32, "CreateProcessW"),
        ("CreateProcessW", app, f"updater_skhid.exe {link}", 0, 0, 0, 0, 0, folder + "\\", 0x44, True),
        ("CloseHandle", 0x222), ("CloseHandle", 0x111)]


def hostile():
    """Links that must start nothing."""
    good = [("replay", " "), (ID, " "), (TOKEN, " "), (SHA, " "), ("1048576", "")]
    yield "", "an empty buffer"
    yield VALID.replace("replay", "Replay"), "another prefix (case)"
    yield VALID.replace("replay", "replays"), "another prefix"
    yield "x" + VALID, "a character before"
    yield " " + VALID, "a space before"
    yield VALID + " ", "a space after"
    yield VALID + "\n", "a line end after"
    yield VALID + "0000", "an eleventh size digit"
    yield f"replay {ID} {TOKEN} {SHA} 12345678901", "eleven size digits"
    yield f"replay {ID} {TOKEN} {SHA} ", "no size"
    yield f"replay {ID} {TOKEN} {SHA}", "no size, no space"
    yield f"replay {ID} {TOKEN} {SHA} -1", "a signed size"
    yield f"replay {ID} {TOKEN} {SHA} +1", "a plus sign"
    yield f"replay {ID} {TOKEN} {SHA} 1e6", "an exponent"
    yield f"replay {ID} {TOKEN} {SHA} 0x10", "a hex size"
    yield f"replay  {ID} {TOKEN} {SHA} 1", "two spaces"
    yield f"replay {ID}  {TOKEN} {SHA} 1", "two spaces after the id"
    yield f"replay {ID} {TOKEN} {SHA}  1", "two spaces before the size"
    yield f"replay\t{ID} {TOKEN} {SHA} 1", "a tab"
    for index, (field, _) in enumerate(good[1:4], 1):
        for label, value in (("short", field[:-1]), ("long", field + "0"), ("upper", field.upper().replace("0", "A")),
                             ("upper letter", field[:-1] + "A"), ("non-hex", field[:-1] + "g"),
                             ("quote", field[:-1] + '"'), ("ampersand", field[:-1] + "&"),
                             ("pipe", field[:-1] + "|"), ("backslash", field[:-1] + "\\"),
                             ("slash", field[:-1] + "/"), ("colon", field[:-1] + ":"), ("dot", field[:-1] + "."),
                             ("percent", field[:-1] + "%"), ("caret", field[:-1] + "^"),
                             ("non-ASCII", field[:-2] + "\u00e9"), ("zero inside", field[:-1] + "\0")):
            parts = [part for part, _ in good]
            parts[index] = value
            yield " ".join(parts), f"field {index}: {label}"
    yield f'replay {ID} {TOKEN} {SHA} 1" & calc.exe', "a quote and a command"
    yield f"replay {ID} {TOKEN} {SHA} 1 & calc.exe", "a second command"
    yield f"replay {ID} {TOKEN} {SHA} 1|calc", "a pipe"
    yield f"replay {ID} {TOKEN} {SHA} ..\\..\\x.exe", "a path for the size"
    yield "C:\\Windows\\System32\\calc.exe", "a program"
    yield "calc.exe", "a program by name"
    yield "..\\..\\..\\Windows\\notepad.exe", "a relative path"
    yield "\\\\host\\share\\x.exe", "a network path"
    yield "https://www.youtube.com/watch?v=x", "a URL (what the original opened)"
    yield "file:///C:/Windows/System32/calc.exe", "a file URL"
    yield f"https://026g.skhid.link/replays/{TOKEN}", "the replay's own URL"
    yield f"updater_skhid.exe replay {ID} {TOKEN} {SHA} 1", "the program's name in front"
    yield "a" * 4000, "an over-long text"
    yield VALID + "x" * 200, "a valid link with more after it"
    yield "replay", "the prefix alone"
    yield b"replay \xff", "a byte that is no UTF-8"


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = True
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0

    # The original opens any text: the link with ShellExecuteA, then the element.
    for link in (VALID, "https://www.youtube.com/watch?v=x", "C:\\Windows\\System32\\calc.exe"):
        calls = Handler(original, link).run()
        assert calls == [("ShellExecuteA", "open", link, 0, 0, 1),
                         ("enable_element", "root.statusPanel.videoTutorial", 1)], calls
        checks += 1

    # The build: a replay link starts the updater, beside the exe, with the link.
    assert started(Handler(patched, VALID).run())
    for size in ("1", "0", "9", "1234567890", "9999999999", "0000000001", "67108864"):
        link = f"replay {ID} {TOKEN} {SHA} {size}"
        assert started(Handler(patched, link).run(), link=link), size
        checks += 1
    for digits in ("0123456789abcdef", "fedcba9876543210", "0000000000000000", "ffffffffffffffff"):
        link = f"replay {digits[:8]} {digits * 2} {digits * 4} 5"
        assert started(Handler(patched, link).run(), link=link), digits
        checks += 1
    # Folders with spaces, quotes cannot be in one; letters of any alphabet
    # (the path is read and given on as UTF-16).
    for folder in ("D:\\My Games\\Survarium 0.26g\\game\\binaries\\x86",
                   "C:\\\u0406\u0433\u0440\u0438\\\u0421\u0443\u0440\u0432\u0430\u0440\u0456\u0443\u043c\\game\\binaries\\x86",
                   "\\\\server\\share\\game\\binaries\\x86", "E:\\x86"):
        assert started(Handler(patched, VALID, folder + "\\survarium_skhid.exe").run(), folder), folder
        checks += 1
    # The longest path that fits: the folder and the name in 519 units.
    folder = "C:\\" + "d" * (0x208 - 1 - 18 - 3)
    assert len(folder + "\\updater_skhid.exe") == 0x208 - 1
    assert started(Handler(patched, VALID, folder + "\\x.exe").run(), folder)
    checks += 1

    # ... and nothing else, whatever the text.
    for link, label in hostile():
        calls = Handler(patched, link).run()
        assert calls == [], (label, calls)
        checks += 1

    # Windows saying no: nothing is started, nothing is closed that was not opened.
    for label, handler, upto in (
            ("a path cut short", Handler(patched, VALID, "C:\\" + "p" * 0x300 + "\\s.exe"), 1),
            ("a folder too long for the name", Handler(patched, VALID, "C:\\" + "d" * (0x208 - 21) + "\\s.exe"), 1),
            ("no folder", Handler(patched, VALID, "survarium_skhid.exe"), 1),
            ("no path", Handler(patched, VALID, "", units=0), 1),
            ("no kernel32", Handler(patched, VALID, kernel32=False), 2),
            ("the start fails", Handler(patched, VALID, create=False), 4)):
        calls = handler.run()
        assert len(calls) == upto and not [c for c in calls if c[0] in ("CloseHandle", "ShellExecuteA",
                                                                        "enable_element")], (label, calls)
        checks += 1
    print(f"patch 29: {checks} links as expected (the original opens all; the build starts the updater "
          f"for a replay link only)")


if __name__ == "__main__":
    main()
