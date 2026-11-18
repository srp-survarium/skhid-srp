#!/usr/bin/env python3
"""Patch 150 (lobby_seeding): the game shares by BitTorrent on the login
screen and in the lobby while its own enable_torrent_seeding ("Allow
torrent", a line of user.cfg) is on, its default now off (README "The game
shares in the lobby", client_patch.md).

    python check_lobby_seeding_026g.py <original survarium.exe>

The original and the built exe run the game's own code, kernel32 stubbed
through the import table (and GetProcAddress's), the game's callees around
the sites stubbed and written down:

- torrent_client::start_seeding_impl (0x373C20, thiscall, `ret 8`) from its
  entry on a made-up torrent client with no pipe. The original makes the
  pipe (engine_malloc 0xE4F90, pipe::pipe 0x373090), accepts on its garbled
  name (pipe::accept 0x373230), binds the message handler 0x373BD0 and goes
  on to the service (OpenSCManagerA, where it is stopped). The built exe: the
  same pipe and handler, the name \\\\.\\pipe\\skhid_seed-<process id>, then
  CreateProcessW of <exe folder>\\updater_skhid.exe `seed <pipe>` from that
  folder, no window, both handles closed, no service, the return with ESP
  past its two arguments and EBX, ESI, EDI, EBP kept.
- enable_torrent_seeding (0xA712B6): 1 in the original's data, 0 in the
  built exe's; game::switch_to_lobby (0x1ED1E0) and switch_to_login
  (0x1ED260) call start_seeding (0x3737F0) only while it is on, in both;
  its callback (0x1EE580) starts (on) or stops (off) it, in both.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

START_SEEDING_IMPL, START_SEEDING, STOP_SEEDING = 0x373C20, 0x3737F0, 0x373910
SWITCH_TO_LOBBY, SWITCH_TO_LOGIN, ENABLE_CALLBACK = 0x1ED1E0, 0x1ED260, 0x1EE580
ENABLE_TORRENT_SEEDING = 0xA712B6
MALLOC, ALLOCATION_NAME, PIPE_NEW, PRODUCT, SWPRINTF, CATEGORY = 0xE4F90, 0x420789, 0x373090, 0xE4390, 0x3745B0, 0x445490
ACCEPT, MAKE_FUNCTION, ASSIGN_FUNCTION, HANDLER = 0x373230, 0x374670, 0x219470, 0x373BD0
LOGIN_SCENE_CALL = 0x21EEA0
OPEN_SC_MANAGER_CALL = 0x373D5B
GAME_LOBBY, GAME_LOGIN, GAME_SCENE, GAME_NETWORK = 0x5390, 0x5394, 0x539C, 0x53AC

SCRATCH, STACK, STUBS, HEAP = 0x41000000, 0x41100000, 0x41200000, 0x42000000
CLIENT, WORLD, PRODUCT_NAME = SCRATCH, SCRATCH + 0x100, SCRATCH + 0x200
GAME, NETWORK, NETWORK_VTABLE, SCENE, SCENE_VTABLE = (SCRATCH + 0x1000 * i for i in range(1, 6))
SP, RETURN = STACK + 0x8000, STUBS + 0xF00
PID = 4321
FOLDER = r"C:\Games\Survarium\game\binaries\x86"
REGS = {UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: 0x2222, UC_X86_REG_EDI: 0x3333, UC_X86_REG_EBP: 0x4444}
# kernel32's functions the code may call: their arguments (stdcall)
ARGUMENTS = {"GetCurrentProcessId": 0, "GetModuleHandleA": 1, "GetProcAddress": 2,
             "CloseHandle": 1, "GetLastError": 0, "CreateFileA": 7, "WriteFile": 5, "GetTickCount": 0,
             "GetCurrentThreadId": 0, "FindFirstFileA": 2, "FindClose": 1, "SetFilePointer": 4,
             "GetFileSize": 2, "SwitchToThread": 0, "Sleep": 1}
DYNAMIC = {"GetModuleFileNameW": 3, "CreateProcessW": 10, "MoveFileExA": 3}


def imports(data):
    """{IAT slot VA: function name} of the exe's import table."""
    headers = base.Headers(data)
    rva = struct.unpack_from("<I", data, headers.optional + 96 + 8)[0]
    sections = headers.sections(data)

    def offset(va):
        return next(va - base.IMAGE_BASE - rva + raw for _, rva, vsize, raw, rsize, _ in sections
                    if rva <= va - base.IMAGE_BASE < rva + max(vsize, rsize))
    cstr = lambda va: data[offset(va):data.index(b"\0", offset(va))].decode()
    out, at = {}, base.IMAGE_BASE + rva
    while True:
        lookup, _, _, name, first = struct.unpack_from("<5I", data, offset(at))
        if not name:
            return out
        index = 0
        while thunk := struct.unpack_from("<I", data, offset(base.IMAGE_BASE + (lookup or first) + 4 * index))[0]:
            if not thunk & 0x80000000:
                out[base.IMAGE_BASE + first + 4 * index] = cstr(base.IMAGE_BASE + thunk + 2)
            index += 1
        at += 20


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((SCRATCH, 0x10000), (STACK, 0x10000), (STUBS, 0x1000), (HEAP, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.kernel32 = {}
        for i, (slot, name) in enumerate(sorted(imports(data).items())):
            stub = STUBS + 4 * i
            self.kernel32[stub] = name
            self.word(slot, stub)
        self.dynamic = {name: STUBS + 0xE00 + 0x10 * i for i, name in enumerate(DYNAMIC)}
        self.kernel32.update({at: name for name, at in self.dynamic.items()})
        self.game = {
            MALLOC: (4, lambda: ("malloc", self.emu.reg_read(UC_X86_REG_EAX)), HEAP),
            ALLOCATION_NAME: (0, None, 0),
            PIPE_NEW: (0, lambda: ("pipe", self.emu.reg_read(UC_X86_REG_ESI)), None),
            PRODUCT: (0, None, PRODUCT_NAME),
            SWPRINTF: (0, self.swprintf, None),
            CATEGORY: (0, None, 0),
            ACCEPT: (4, lambda: ("accept", self.emu.reg_read(UC_X86_REG_ECX), self.wide(self.emu.reg_read(UC_X86_REG_EAX))),
                     None),
            MAKE_FUNCTION: (8, self.make_function, None),
            ASSIGN_FUNCTION: (0, lambda: ("handler bound", self.emu.reg_read(UC_X86_REG_EDI)), None),
            START_SEEDING: (4, lambda: ("start_seeding", self.arg(0)), None),
            STOP_SEEDING: (4, lambda: ("stop_seeding", self.arg(0)), None),
            LOGIN_SCENE_CALL: (4, None, None),
            NETWORK_VTABLE + 0x100: (0, None, 1),          # vtable +20h: a network client (not a replay)
            NETWORK_VTABLE + 0x110: (0, None, CLIENT),     # vtable +48h: its torrent client
            SCENE_VTABLE + 0x100: (0, lambda: ("scene", self.emu.reg_read(UC_X86_REG_ECX)), None),
        }
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=STUBS, end=STUBS + 0xFFF)
        for at in self.game:
            if not STUBS <= at < STUBS + 0x1000:
                emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, self.service, begin=OPEN_SC_MANAGER_CALL, end=OPEN_SC_MANAGER_CALL)
        emu.mem_write(PRODUCT_NAME, b"survarium\0")

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def arg(self, n):
        return self.read(self.emu.reg_read(UC_X86_REG_ESP) + 4 + 4 * n)

    def narrow(self, at):
        out = bytearray()
        while (b := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(b)
        return out.decode()

    def wide(self, at):
        out = bytearray()
        while (unit := bytes(self.emu.mem_read(at + len(out), 2))) != b"\0\0":
            out += unit
        return out.decode("utf-16-le")

    def swprintf(self):
        # the game's L"%s-%s" of the pipe's prefix and the narrow product name read as wide
        buffer = self.emu.reg_read(UC_X86_REG_EDX)
        self.emu.mem_write(buffer, "\\\\.\\pipe\\vostok_updater_pipe-\u7573\u7672".encode("utf-16-le") + b"\0\0")
        return None

    def make_function(self):
        self.emu.mem_write(self.emu.reg_read(UC_X86_REG_EAX), bytes(0x20))   # an empty function: nothing to destroy
        return ("handler", self.arg(0), self.arg(1))

    def service(self, emu, address, size, _):
        self.calls.append(("OpenSCManagerA",))
        emu.emu_stop()

    def stub(self, emu, address, size, _):
        if address in self.kernel32:
            return self.kernel(address, self.kernel32[address])
        pops, record, result = self.game[address]
        if record is not None and (entry := record()) is not None:
            self.calls.append(entry)
        sp = emu.reg_read(UC_X86_REG_ESP)
        if result is not None:
            emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def kernel(self, address, name):
        emu = self.emu
        count = DYNAMIC.get(name, ARGUMENTS.get(name))
        if count is None:
            raise AssertionError(f"kernel32's {name} called, which the check does not know")
        args = [self.arg(i) for i in range(count)]
        result = 0
        if name == "GetCurrentProcessId":
            result = PID
        elif name == "GetModuleHandleA":
            result = 0x7B000000 if self.narrow(args[0]).lower() == "kernel32.dll" else 0
        elif name == "GetProcAddress":
            result = self.dynamic.get(self.narrow(args[1]), 0)
        elif name == "GetModuleFileNameW":
            path = (FOLDER + r"\survarium_skhid.exe").encode("utf-16-le")
            assert args[0] == 0 and args[2] > len(path) // 2, args
            emu.mem_write(args[1], path + b"\0\0")
            result = len(path) // 2
        elif name == "CreateProcessW":
            application, command, _, _, inherit, flags, environment, folder, startup, information = args
            assert self.read(startup) == 0x44, "STARTUPINFOW's size"
            self.calls.append(("CreateProcessW", self.wide(application), self.wide(command), inherit, flags,
                               environment, self.wide(folder)))
            emu.mem_write(information, struct.pack("<4I", 0x500, 0x504, 77, 78))
            result = 1
        elif name == "CloseHandle":
            self.calls.append(("CloseHandle", args[0]))
            result = 1
        elif name == "CreateFileA":
            result = 0x900
        elif name == "WriteFile":
            self.log.append(bytes(emu.mem_read(args[1], args[2])).decode())
            if args[3]:
                self.word(args[3], args[2])
            result = 1
        elif name in ("GetTickCount", "GetCurrentThreadId"):
            result = 7
        elif name == "FindFirstFileA":
            result = 0xFFFFFFFF
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)

    def fresh(self):
        self.emu.mem_write(SCRATCH, bytes(0x10000))
        self.emu.mem_write(STACK, bytes(0x10000))
        self.emu.mem_write(PRODUCT_NAME, b"survarium\0")
        self.calls, self.log = [], []
        for reg, value in REGS.items():
            self.emu.reg_write(reg, value)

    def start_seeding_impl(self, stop_at_service=False):
        """The impl on a client with no pipe; (calls, the client's pipe)."""
        self.fresh()
        self.word(CLIENT, WORLD)                  # +0 the network world, its +4 the io_service
        self.word(WORLD + 4, 0x5150)
        self.emu.mem_write(SP, struct.pack("<3I", RETURN, 1, 1024))
        self.emu.reg_write(UC_X86_REG_ECX, CLIENT)
        self.emu.reg_write(UC_X86_REG_ESP, SP)
        self.emu.emu_start(START_SEEDING_IMPL, RETURN, count=2_000_000)
        if not stop_at_service:
            assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN, hex(self.emu.reg_read(UC_X86_REG_EIP))
            assert self.emu.reg_read(UC_X86_REG_ESP) == SP + 12, "not start_seeding_impl's `ret 8`"
            assert all(self.emu.reg_read(r) == v for r, v in REGS.items()), "a callee-saved register changed"
        return self.calls, self.read(CLIENT + 4)

    def switch(self, function, enabled, pops):
        """switch_to_lobby or switch_to_login with enable_torrent_seeding as given: the calls."""
        self.fresh()
        self.emu.mem_write(ENABLE_TORRENT_SEEDING, bytes([enabled]))
        self.word(GAME + GAME_NETWORK, NETWORK)
        self.word(NETWORK, NETWORK_VTABLE)
        self.word(NETWORK_VTABLE + 0x20, NETWORK_VTABLE + 0x100)
        self.word(NETWORK_VTABLE + 0x48, NETWORK_VTABLE + 0x110)
        for scene in (GAME_LOBBY, GAME_LOGIN):
            self.word(GAME + scene, SCENE)
        self.word(GAME + GAME_SCENE, SCENE)        # the scene active already: no scene change
        self.emu.mem_write(SP, struct.pack("<2I", RETURN, 0))
        self.emu.reg_write(UC_X86_REG_EAX, GAME)
        self.emu.reg_write(UC_X86_REG_ESP, SP)
        self.emu.emu_start(function, RETURN, count=100_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN and self.emu.reg_read(UC_X86_REG_ESP) == SP + 4 + pops
        return self.calls

    def callback(self, enabled):
        self.fresh()
        self.emu.mem_write(ENABLE_TORRENT_SEEDING, bytes([enabled]))
        self.word(GAME + GAME_NETWORK, NETWORK)
        self.word(NETWORK, NETWORK_VTABLE)
        self.word(NETWORK_VTABLE + 0x20, NETWORK_VTABLE + 0x100)
        self.word(NETWORK_VTABLE + 0x48, NETWORK_VTABLE + 0x110)
        self.emu.mem_write(SP, struct.pack("<2I", RETURN, 0))
        self.emu.reg_write(UC_X86_REG_ECX, GAME)
        self.emu.reg_write(UC_X86_REG_ESP, SP)
        self.emu.emu_start(ENABLE_CALLBACK, RETURN, count=100_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN and self.emu.reg_read(UC_X86_REG_ESP) == SP + 8
        assert all(self.emu.reg_read(r) == v for r, v in REGS.items() if r != UC_X86_REG_EBP)
        return self.calls


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Game(original), Game(bytes(built))

    # 1. start_seeding_impl, the original: its pipe on the garbled name, then the service.
    calls, pipe = old.start_seeding_impl(stop_at_service=True)
    garbled = "\\\\.\\pipe\\vostok_updater_pipe-\u7573\u7672"
    assert calls == [("malloc", 0x10D0), ("pipe", HEAP), ("accept", HEAP, garbled), ("handler", HANDLER, CLIENT + 0xC),
                     ("handler bound", HEAP), ("OpenSCManagerA",)], calls
    assert pipe == HEAP
    print("ok the original: start_seeding_impl makes its pipe on the garbled name and goes on to the service")

    # 2. The built exe: our pipe, the updater beside the exe, no service.
    pipe_name = f"\\\\.\\pipe\\skhid_seed-{PID}"
    updater = FOLDER + "\\updater_skhid.exe"
    for run in range(2):
        calls, pipe = new.start_seeding_impl()
        assert calls == [("malloc", 0x10D0), ("pipe", HEAP), ("accept", HEAP, pipe_name), ("handler", HANDLER, CLIENT + 0xC),
                         ("handler bound", HEAP),
                         ("CreateProcessW", updater, f'"{updater}" seed {pipe_name}', 0, 0x08000000, 0, FOLDER),
                         ("CloseHandle", 0x500), ("CloseHandle", 0x504)], calls
        assert pipe == HEAP
        # the log's line: the first run's (the same line again is counted, not written)
        assert run or any("lobby_seeding: updater_skhid.exe seed " + pipe_name + " started" in t for t in new.log), new.log
    print(f"ok the built exe: the pipe {pipe_name}, CreateProcessW of {updater} `seed <pipe>` from its folder, "
          "no window, its handles closed, no service; `ret 8`, registers kept")

    # 3. The setting: off by default; the game's own gates as they were.
    at = next(ENABLE_TORRENT_SEEDING - base.IMAGE_BASE - rva + raw for _, rva, vsize, raw, _, _ in base.Headers(original).sections(original)
              if rva <= ENABLE_TORRENT_SEEDING - base.IMAGE_BASE < rva + vsize)
    assert original[at] == 1 and built[at] == 0, (original[at], built[at])
    for game in (old, new):
        for function, pops in ((SWITCH_TO_LOBBY, 0), (SWITCH_TO_LOGIN, 4)):
            assert game.switch(function, 1, pops) == [("start_seeding", CLIENT)]
            assert game.switch(function, 0, pops) == []
        assert game.callback(1) == [("start_seeding", CLIENT)] and game.callback(0) == [("stop_seeding", CLIENT)]
    print("ok enable_torrent_seeding is off until user.cfg says on (the original: on); switch_to_lobby and "
          "switch_to_login start the seeding only while it is on, its callback starts and stops it, as in the original")
    print("patch 150: the game's lobby seeding starts updater_skhid.exe seed \\\\.\\pipe\\skhid_seed-<pid> beside "
          "itself, while enable_torrent_seeding is on")


if __name__ == "__main__":
    main()
