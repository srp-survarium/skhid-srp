#!/usr/bin/env python3
"""Run patch 83 (input_devices_026g.asm and .c: a failed DirectInput step at
the start stops the game with a message instead of a crash) and the
original's create_devices on made-up DirectInput results.

    python check_input_devices_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4) and the arena's tools (`nix develop
./srp-026g`: the build links the arena). The exe's sections are loaded as
they are, the arena with them. The game's own code runs:
vostok::input::input_world::create_devices (0x3B82F0) from its entry, on a
made-up input world, the window and its second argument (0: the DirectInput
path), to 0x3B843E (the mouse stored at +28h), with the gamepad's
(0x3B8790), the keyboard's (0x3B8BC0) and the mouse's (0x3B94F0)
constructors of the game. Stubs: GetModuleHandleA, DirectInput8Create (its
delay-load IAT slot: writes the interface or not, returns the HRESULT
given), malloc_impl 0xE4F90 (a bump allocator), the keyboard's memset
0x424500, a made-up IDirectInput8 (CreateDevice, slot 3) and its devices
(SetDataFormat, SetCooperativeLevel, SetProperty: recorded), and the imports
the patch calls (MessageBoxW: recorded; GetCurrentProcess;
TerminateProcess: recorded, the run stops).

- Every step succeeds (S_OK, and S_FALSE from CreateDevice), on either
  path: the build ends as the original does: the same world, keyboard and
  mouse objects, the same device calls, the same callee-saved registers
  and stack, and no message box.
- DirectInput8Create fails (REGDB_E_CLASSNOTREG, E_NOINTERFACE,
  DIERR_OLDDIRECTINPUTVERSION, E_ACCESSDENIED, E_FAIL; also a failure that
  wrote an interface, and S_OK with none) on the DirectInput path: the
  original reads through the NULL interface in the keyboard's constructor
  (0x3B8BFD); the build shows one message box, owned by the game's window,
  with the step, the HRESULT in hex and the advice for it (Russian, then
  English), then TerminateProcess(GetCurrentProcess(), 1); no constructor
  of a device runs. On the other path (the second argument 1) the build
  does what the original does.
- The keyboard's or the mouse's CreateDevice fails: the original reads
  through the NULL device (0x3B8C0C, 0x3B9524); the build shows the
  keyboard's or the mouse's message and ends; the mouse is never made after
  a failed keyboard, and no SetDataFormat of a failed device is called.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EIP,
                               UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

CREATE_DEVICES, DONE = 0x3B82F0, 0x3B843E          # its entry; past `mov [edi+28h],eax` (the mouse)
GAMEPAD, KEYBOARD, MOUSE = 0x3B8790, 0x3B8BC0, 0x3B94F0
CRASHES = {"direct_input": 0x3B8BFD, "keyboard": 0x3B8C0C, "mouse": 0x3B9524}   # the original's NULL reads
MALLOC, MEMSET = 0xE4F90, 0x424500
GET_MODULE_HANDLE, DINPUT8_CREATE = 0x87003C, 0xAEFE1C   # IAT slots (the second a delay-load one)
MESSAGE_BOX, GET_CURRENT_PROCESS, TERMINATE_PROCESS = 0xAEFECC, 0x870034, 0x870030
GUID_KEYBOARD, GUID_MOUSE, IID_DINPUT8 = 0x88E34C, 0x88E33C, 0x88E2EC

MEMORY, HEAP, STACK, STUBS, STOP = 0x30000000, 0x30100000, 0x30208000, 0x30300000, 0x30400000
WORLD, DINPUT, DINPUT_VT, DEVICE_VT = MEMORY, MEMORY + 0x1000, MEMORY + 0x1100, MEMORY + 0x1200
KEYBOARD_DEVICE, MOUSE_DEVICE = MEMORY + 0x2000, MEMORY + 0x2100
WINDOW, INSTANCE = 0x00C0FFEE, 0x00400000
STUB_NAMES = ("get_module_handle", "dinput8_create", "message_box", "get_current_process", "terminate_process",
              "create_device", "set_data_format", "set_cooperative_level", "set_property", "malloc", "memset")
STUB = {name: STUBS + 0x10 * i for i, name in enumerate(STUB_NAMES)}

S_OK, S_FALSE = 0, 1
CLASSNOTREG, NOINTERFACE, OLDVERSION = 0x80040154, 0x80004002, 0x8007047E
ACCESSDENIED, FAIL = 0x80070005, 0x80004005


class Stopped(Exception):
    pass


class Run:
    """create_devices of `data` with DirectInput8Create answering `dinput`
    (hr, writes the interface) and CreateDevice answering `devices`
    {GUID: (hr, writes the device)}; the path byte `path`."""

    def __init__(self, data, dinput=(S_OK, True), devices=None, path=0):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (MEMORY, HEAP, STACK - 0x8000, STUBS, STOP):
            emu.mem_map(at, 0x100000 if at == HEAP else 0x10000, UC_PROT_ALL)
        for name, at in STUB.items():
            emu.mem_write(at, b"\xCC")
        for slot, name in ((GET_MODULE_HANDLE, "get_module_handle"), (DINPUT8_CREATE, "dinput8_create"),
                           (MESSAGE_BOX, "message_box"), (GET_CURRENT_PROCESS, "get_current_process"),
                           (TERMINATE_PROCESS, "terminate_process")):
            self.word(slot, STUB[name])
        self.word(DINPUT, DINPUT_VT)
        self.word(DINPUT_VT + 0xC, STUB["create_device"])
        for slot, name in ((0x2C, "set_data_format"), (0x34, "set_cooperative_level"), (0x18, "set_property")):
            self.word(DEVICE_VT + slot, STUB[name])
        self.word(KEYBOARD_DEVICE, DEVICE_VT)
        self.word(MOUSE_DEVICE, DEVICE_VT)
        self.dinput = dinput
        self.devices = {GUID_KEYBOARD: (S_OK, True), GUID_MOUSE: (S_OK, True), **(devices or {})}
        self.heap = HEAP
        self.calls, self.made, self.boxes, self.exit = [], [], [], None
        self.path = path
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=STUBS, end=STUBS + 0x10 * len(STUB))
        emu.hook_add(UC_HOOK_CODE, self.constructor, begin=GAMEPAD, end=GAMEPAD)
        emu.hook_add(UC_HOOK_CODE, self.constructor, begin=KEYBOARD, end=KEYBOARD)
        emu.hook_add(UC_HOOK_CODE, self.constructor, begin=MOUSE, end=MOUSE)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=MALLOC, end=MALLOC)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=MEMSET, end=MEMSET)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def args(self, n):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        return [self.read(sp + 4 + 4 * i) for i in range(n)]

    def ret(self, pop, value):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def wide(self, at):
        out = bytearray()
        while True:
            pair = bytes(self.emu.mem_read(at + len(out), 2))
            if pair == b"\0\0":
                return out.decode("utf-16-le")
            out += pair

    def constructor(self, uc, address, size, user):
        self.made.append({GAMEPAD: "gamepad", KEYBOARD: "keyboard", MOUSE: "mouse"}[address])

    def stub(self, uc, address, size, user):
        name = {MALLOC: "malloc", MEMSET: "memset"}.get(address) or STUB_NAMES[(address - STUBS) // 0x10]
        if name == "get_module_handle":
            assert self.args(1) == [0], "GetModuleHandleA(NULL)"
            self.ret(4, INSTANCE)
        elif name == "dinput8_create":
            instance, version, iid, out, outer = self.args(5)
            assert (instance, version, iid, outer) == (INSTANCE, 0x800, IID_DINPUT8, 0), "DirectInput8Create's args"
            assert out == WORLD + 0x1C, "the interface goes to the world's +1Ch"
            hr, writes = self.dinput
            self.word(out, DINPUT if writes else 0)
            self.ret(20, hr)
        elif name == "create_device":
            this, guid, out, outer = self.args(4)
            assert this == DINPUT and outer == 0 and guid in self.devices, (hex(this), hex(guid))
            hr, writes = self.devices[guid]
            device = KEYBOARD_DEVICE if guid == GUID_KEYBOARD else MOUSE_DEVICE
            self.word(out, device if writes else 0)
            self.calls.append(("CreateDevice", guid, out))
            self.ret(16, hr)
        elif name in ("set_data_format", "set_cooperative_level", "set_property"):
            n = 2 if name == "set_data_format" else 3
            self.calls.append((name, *self.args(n)))
            self.ret(4 * n, S_OK)
        elif name == "malloc":
            size = self.emu.reg_read(UC_X86_REG_EAX)
            block, self.heap = self.heap, (self.heap + size + 0xF) & ~0xF
            self.ret(4, block)
        elif name == "memset":
            self.calls.append(("memset", *self.args(3)))
            self.ret(0, self.args(1)[0])
        elif name == "message_box":
            window, text, caption, kind = self.args(4)
            self.boxes.append((window, self.wide(text), self.wide(caption), kind))
            self.ret(16, 1)                                   # IDOK
        elif name == "get_current_process":
            self.ret(0, 0xFFFFFFFF)
        elif name == "terminate_process":
            self.exit = tuple(self.args(2))
            uc.emu_stop()

    def go(self):
        """'done' at 0x3B843E, 'ended' in TerminateProcess, or the fault's
        address."""
        emu = self.emu
        esp = STACK - 12
        emu.mem_write(esp, struct.pack("<III", STOP, WINDOW, self.path))
        emu.reg_write(UC_X86_REG_ESP, esp)
        emu.reg_write(UC_X86_REG_EAX, WORLD)
        for reg, value in ((UC_X86_REG_EBX, 0x1111), (UC_X86_REG_ESI, 0x2222), (UC_X86_REG_EDI, 0x3333),
                           (UC_X86_REG_EBP, 0x4444)):
            emu.reg_write(reg, value)
        try:
            emu.emu_start(CREATE_DEVICES, DONE, count=100000)
        except UcError:
            return ("fault", emu.reg_read(UC_X86_REG_EIP))
        if self.exit is not None:
            return ("ended",)
        assert emu.reg_read(UC_X86_REG_EIP) == DONE, hex(emu.reg_read(UC_X86_REG_EIP))
        return ("done",)

    def state(self):
        """What create_devices made, by value: the world's devices and
        everything they hold, the device calls, and create_devices' frame
        (its callee-saved registers pushed, esp)."""
        emu = self.emu
        world = bytes(emu.mem_read(WORLD, 0x40))
        objects = bytes(emu.mem_read(HEAP, self.heap - HEAP))
        sp = emu.reg_read(UC_X86_REG_ESP)
        regs = [emu.reg_read(r) for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)]
        return world, objects, self.calls, self.made, sp, bytes(emu.mem_read(sp, 0x24)), regs


def expect_message(run, step, hresult, advice):
    """The one box and the end the build must have for `step` failed with
    `hresult`, `advice` "repair" or "restart"."""
    assert len(run.boxes) == 1, run.boxes
    window, text, caption, kind = run.boxes[0]
    assert window == WINDOW, "owned by the game's window"
    assert caption == "Survarium" and kind == 0x50010, (caption, hex(kind))   # MB_ICONERROR|SETFOREGROUND|TOPMOST
    ru, en = text.split("\r\n\r\n")
    code = f"0x{hresult:08X}"
    lead = {"direct_input": ("не запустила DirectInput", "Windows could not start DirectInput"),
            "keyboard": ("не открыл клавиатуру", "could not open the keyboard"),
            "mouse": ("не открыл мышь", "could not open the mouse")}[step]
    assert ru.startswith("Survarium не может запуститься: ") and lead[0] in ru.split("\r\n")[0], ru
    assert en.startswith("Survarium cannot start: ") and lead[1] in en.split("\r\n")[0], en
    assert ru.split("\r\n")[0].endswith(f"(ошибка {code}).") and en.split("\r\n")[0].endswith(f"(error {code})."), text
    fix = ("sfc /scannow" in ru and "DirectX End-User Runtime" in ru and "sfc /scannow" in en
           and "DirectX End-User Runtime" in en)
    other = "survarium_skhid.exe" in ru and "survarium_skhid.exe" in en and "Unplug game controllers" in en
    assert (fix, other) == ((True, False) if advice == "repair" else (False, True)), text
    assert "discord.gg/Pxa4prXfwu" in ru and "discord.gg/Pxa4prXfwu" in en
    assert all(len(line) <= 90 for line in text.split("\r\n")), "one short line per point"
    assert run.exit == (0xFFFFFFFF, 1), run.exit
    return text


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    def attempt(text, check):
        try:
            check()
        except AssertionError as error:
            report(False, f"{text}: {error}")
        else:
            report(True, text)

    print("every step succeeds: the build is the original")
    for label, kw in (("S_OK everywhere, the DirectInput path", {}),
                      ("S_FALSE from both CreateDevice", {"devices": {GUID_KEYBOARD: (S_FALSE, True),
                                                                      GUID_MOUSE: (S_FALSE, True)}}),
                      ("the other path (second argument 1)", {"path": 1})):
        def same(kw=kw):
            stock, ours = Run(original, **kw), Run(built, **kw)
            assert stock.go() == ours.go() == ("done",)
            assert stock.state() == ours.state(), "the world, its devices, the calls or the frame differ"
            assert not ours.boxes and ours.exit is None
            if not kw.get("path"):
                assert ours.made == ["gamepad", "keyboard", "mouse"]
                assert [c[0] for c in ours.calls] == ["CreateDevice", "set_data_format", "set_cooperative_level",
                                                      "set_property", "memset", "CreateDevice", "set_data_format",
                                                      "set_cooperative_level"], ours.calls
                assert ("set_cooperative_level", KEYBOARD_DEVICE, WINDOW, 6) in ours.calls
                assert ("set_cooperative_level", MOUSE_DEVICE, WINDOW, 6) in ours.calls
        attempt(label, same)

    print("DirectInput8Create fails on the DirectInput path")
    for hresult, writes, advice in ((CLASSNOTREG, False, "repair"), (NOINTERFACE, False, "repair"),
                                    (OLDVERSION, False, "repair"), (ACCESSDENIED, False, "restart"),
                                    (FAIL, False, "restart"), (FAIL, True, "restart"), (S_OK, False, "repair")):
        def dinput(hresult=hresult, writes=writes, advice=advice):
            stock, ours = Run(original, dinput=(hresult, writes)), Run(built, dinput=(hresult, writes))
            if not writes:
                assert stock.go() == ("fault", CRASHES["direct_input"]), "the original reads through NULL"
            assert ours.go() == ("ended",)
            expect_message(ours, "direct_input", hresult, advice)
            assert ours.made == [] and not ours.calls, "no device is made"
            return ours
        attempt(f"{hresult:#010x}{', an interface written' if writes else ''}: the original "
                f"{'crashes at 0x3B8BFD' if not writes else 'goes on'}, the build stops with the {advice} advice",
                dinput)

    def other_path():
        stock, ours = Run(original, dinput=(CLASSNOTREG, False), path=1), Run(built, dinput=(CLASSNOTREG, False), path=1)
        assert stock.go() == ours.go() == ("done",)
        assert stock.state() == ours.state() and not ours.boxes and ours.exit is None
    attempt("a failed DirectInput8Create on the other path: as the original (no DirectInput device is made)",
            other_path)

    print("a device's CreateDevice fails")
    for step, guid in (("keyboard", GUID_KEYBOARD), ("mouse", GUID_MOUSE)):
        for hresult, advice in ((CLASSNOTREG, "repair"), (ACCESSDENIED, "restart"), (S_OK, "repair")):
            def device(step=step, guid=guid, hresult=hresult, advice=advice):
                kw = {"devices": {guid: (hresult, False)}}
                stock, ours = Run(original, **kw), Run(built, **kw)
                assert stock.go() == ("fault", CRASHES[step]), "the original reads through NULL"
                assert ours.go() == ("ended",)
                expect_message(ours, step, hresult, advice)
                assert ours.made == (["gamepad", "keyboard"] if step == "keyboard"
                                     else ["gamepad", "keyboard", "mouse"])
                device_va = KEYBOARD_DEVICE if step == "keyboard" else MOUSE_DEVICE
                assert not any(c[0] == "set_data_format" and c[1] == device_va for c in ours.calls)
                assert [c[0] for c in ours.calls].count("CreateDevice") == (1 if step == "keyboard" else 2)
            attempt(f"{step}, {hresult:#010x}: the original crashes at {CRASHES[step]:#x}, the build stops with "
                    f"the {advice} advice", device)

    print("the message (the keyboard, REGDB_E_CLASSNOTREG):")
    run = Run(built, devices={GUID_KEYBOARD: (CLASSNOTREG, False)})
    run.go()
    for line in run.boxes[0][1].split("\r\n"):
        print("    " + line)
    print()
    print(f"{failed} FAILED" if failed else "all checks passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
