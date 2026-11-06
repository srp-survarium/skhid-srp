#!/usr/bin/env python3
"""Runs the four functions of the patched launcher that read the installer's
registry key (patch 4) under Unicorn, at its own base and rebased, with
`Software\\Vostok Games\\<product>` nowhere (a player whose launcher never
made it: nothing does since the language is in user.cfg), in HKCU only (a
player of an older launcher) and in HKLM only (an old installation).

Usage: python check_registry_026g.py ORIGINAL_survarium_launcher.exe

Checks that, with no registry call at all (no RegOpenKeyExW, RegCloseKey or
RegQueryValueExW) and no message box:
- get_local_version (0x15213) reads `file to execute`, and both its ends
  get past RegCloseKey with the stack as it was;
- start_updater (0x15997) reads `update server` (the original said "Can't
  open survarium registry key" here), and its end gets past RegCloseKey;
- restart_launcher (0x162F4) reads `launcher`, and both ways on get past
  RegCloseKey;
- get_game_launch_parameters (0x17092), whole, gives PLAY's command line
  and folder, and the end of a failed read gets past RegCloseKey.
Then revision 39's patch 4 (the opens moved to HKCU's product key, no jumps)
must show "Can't open survarium registry key": the check sees the failure
it guards against.
"""
from pathlib import Path
import struct
import sys
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDI,
                               UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP)
import patch_launcher_026g as L

HERE = Path(__file__).resolve().parent
STACK, HEAP, STUBS, STOP = 0x30008000, 0x40000000, 0x50000000, 0x32000000
HKCU, HKLM = 0x80000001, 0x80000002
PRODUCT_KEY = 'Software\\Vostok Games\\Survarium'
FOLDER = 'C:\\Games\\Survarium\\game\\binaries\\x86'
SELF = FOLDER + '\\' + L.LAUNCHER_NAME
# (IAT slot, stub name, stdcall arguments; None: cdecl)
IAT = {0x3B028: ('RegOpenKeyExW', 5), 0x3B024: ('RegCloseKey', 1), 0x3B020: ('RegQueryValueExW', 6),
       0x3B0BC: ('GetModuleFileNameW', 3), 0x3B388: ('MessageBoxW', 4)}
NAMES = [name for name, _ in IAT.values()]
COUNT = dict(IAT.values())
PRODUCT_KEY_PATH = 0x14080              # get_product_registry_path, stubbed for the unfixed build
SWPRINTF = 0x264E0                      # _snwprintf(buffer, count, format, ...), cdecl
CANT_OPEN = "Can't open survarium registry key"


class Launcher:
    def __init__(self, data, delta, keys):
        self.delta, self.keys = delta, keys
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vs, raw, rs, _ in L.P.Headers(data).sections(data):
            address = L.IMAGE_BASE + delta + rva
            u.mem_map(address, (max(vs, rs) + 4095) & ~4095)
            u.mem_write(address, bytes(data[raw:raw + min(rs, vs or rs)]))
        for address in L.relocated(data):
            self.word(address + delta, self.word(address + delta) + delta)
        for address, size in [(STACK - 0x20000, 0x30000), (HEAP, 0x100000), (STUBS, 0x1000), (STOP, 0x1000)]:
            u.mem_map(address, size)
        for slot, (name, _) in IAT.items():
            self.word(slot + delta, STUBS + 4 * NAMES.index(name))
        self.calls, self.boxes, self.stopped_at = [], [], None
        self.heap = HEAP
        u.hook_add(UC_HOOK_CODE, self.hook)

    def word(self, address, value=None):
        if value is None:
            return struct.unpack('<I', self.u.mem_read(address, 4))[0]
        self.u.mem_write(address, struct.pack('<I', value & 0xffffffff))

    def wide(self, address):
        out = bytearray()
        while (pair := bytes(self.u.mem_read(address + len(out), 2))) != b'\0\0':
            out += pair
        return out.decode('utf-16-le')

    def new_wide(self, text):
        at, self.heap = self.heap, self.heap + 2 * len(text) + 2
        self.u.mem_write(at, text.encode('utf-16-le') + b'\0\0')
        return at

    def ret(self, value, count):
        sp = self.u.reg_read(UC_X86_REG_ESP)
        self.u.reg_write(UC_X86_REG_EAX, value & 0xffffffff)
        self.u.reg_write(UC_X86_REG_EIP, self.word(sp))
        self.u.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)

    def hook(self, u, address, size, _):
        if address == STOP:
            u.emu_stop()
            return
        sp = u.reg_read(UC_X86_REG_ESP)
        if address == PRODUCT_KEY_PATH + self.delta:
            self.ret(self.new_wide(PRODUCT_KEY), 0)
            return
        if address == SWPRINTF + self.delta:
            buffer, count, form, *rest = [self.word(sp + 4 + 4 * i) for i in range(5)]
            assert self.wide(form) == '"%s" ', self.wide(form)
            text = f'"{self.wide(rest[0])}" '
            assert len(text) < count
            u.mem_write(buffer, text.encode('utf-16-le') + b'\0\0')
            self.ret(len(text), 0)
            return
        if not STUBS <= address < STUBS + 4 * len(NAMES):
            return
        name = NAMES[(address - STUBS) // 4]
        args = [self.word(sp + 4 + 4 * i) for i in range(COUNT[name])]
        self.ret(getattr(self, name)(*args), COUNT[name])

    # --- Windows, as far as the code uses it ---
    def RegOpenKeyExW(self, hive, subkey, options, sam, out):
        path = self.wide(subkey) if subkey else ''
        self.calls.append(('RegOpenKeyExW', hive, path))
        if (hive, path.lower()) not in self.keys:
            return 2                    # ERROR_FILE_NOT_FOUND
        self.word(out, 0x1004)
        return 0

    def RegCloseKey(self, key):
        self.calls.append(('RegCloseKey', key))
        return 0

    def RegQueryValueExW(self, key, name, reserved, kind, data, size):
        self.calls.append(('RegQueryValueExW', self.wide(name)))
        return 2

    def GetModuleFileNameW(self, module, buffer, size):
        assert module == 0
        self.u.mem_write(buffer, SELF.encode('utf-16-le') + b'\0\0')
        return len(SELF)

    def MessageBoxW(self, window, text, caption, flags):
        self.boxes.append(self.wide(text))
        self.u.emu_stop()
        return 1

    # --- the launcher's code ---
    def run(self, start, stop_at, args=(), frame=False, **registers):
        """Runs from start to stop_at; gives al and whether the stack is as it was."""
        sp = STACK - 4 * (len(args) + 1)
        self.word(sp, STOP)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        if frame:                       # a start inside a function: its frame, as its prologue made it
            self.u.reg_write(UC_X86_REG_EBP, sp)
            sp -= 0x1800
        self.u.reg_write(UC_X86_REG_ESP, sp)
        for name, value in registers.items():
            self.u.reg_write({'eax': UC_X86_REG_EAX, 'ebx': UC_X86_REG_EBX, 'esi': UC_X86_REG_ESI,
                              'edi': UC_X86_REG_EDI}[name], value)
        self.u.emu_start(start + self.delta, stop_at if stop_at == STOP else stop_at + self.delta, count=200000)
        self.stopped_at = self.u.reg_read(UC_X86_REG_EIP) - self.delta
        # A whole function returns (cdecl: its caller takes its arguments off); a part leaves the stack as it was.
        balanced = self.u.reg_read(UC_X86_REG_ESP) == (sp + 4 if stop_at == STOP else sp)
        return self.u.reg_read(UC_X86_REG_EAX) & 0xff, balanced

    def buffer(self, offset):
        return self.wide(self.u.reg_read(UC_X86_REG_EBP) + offset)


def start_updater(data, delta, keys):
    """From where it builds the key's path to its read of `update server`."""
    one = Launcher(data, delta, keys)
    object_ = one.heap
    one.heap += 0x3000
    # esi is the buffer size from 0x157A4 on, edi 0.
    al, _ = one.run(0x15997, 0x159E5, frame=True, edi=0, ebx=object_, esi=0x104)
    return one, al == 1 and not one.boxes and one.buffer(-0x132C) == 'skhid.link'


def closes(data, delta, keys, start, stop_at, **registers):
    """A RegCloseKey site: from the push of its handle on, it gets to stop_at, the stack as it was."""
    one = Launcher(data, delta, keys)
    _, balanced = one.run(start, stop_at, frame=True, **registers)
    return one, balanced and one.stopped_at == stop_at


def check(data, delta, keys):
    """{where: (the run, whether it did its part)}"""
    out = {}
    # get_local_version: up to its read of `file to execute`, then both its ends.
    one = Launcher(data, delta, keys)
    al, _ = one.run(0x15213, 0x1526B)
    out['get_local_version: read'] = (one, al == 1 and one.buffer(-0x268) == FOLDER + '\\' + L.DEFAULT_GAME_EXE)
    out['get_local_version: end, no version'] = closes(data, delta, keys, 0x153FE, 0x15407)
    out['get_local_version: end, a version'] = closes(data, delta, keys, 0x1541E, 0x15427)
    out['start_updater: read'] = start_updater(data, delta, keys)
    out['start_updater: end'] = closes(data, delta, keys, 0x15A4D, 0x15A56)
    # restart_launcher: up to its read of `launcher`, then its two ways on.
    one = Launcher(data, delta, keys)
    al, _ = one.run(0x162F4, 0x16338, frame=True, esi=0)
    out['restart_launcher: read'] = (one, al == 1 and one.buffer(-0x694) == SELF)
    out['restart_launcher: read, goes on'] = closes(data, delta, keys, 0x16338, 0x163AD, eax=1)
    out['restart_launcher: no value, the assertion'] = closes(data, delta, keys, 0x16338, 0x16345, eax=0)
    # get_game_launch_parameters, whole, and the end of a failed read.
    one = Launcher(data, delta, keys)
    line, folder = one.heap, one.heap + 0x10000
    one.heap += 0x20000
    al, balanced = one.run(0x17092, STOP, args=(line, one.new_wide('english'), folder))
    out['get_game_launch_parameters'] = (
        one, al == 1 and balanced
        and one.wide(line) == f'"{FOLDER}\\{L.DEFAULT_GAME_EXE}" {L.DEFAULT_ARGUMENTS}'
        and one.wide(folder) == FOLDER)
    out['get_game_launch_parameters: end, failed'] = closes(data, delta, keys, 0x17123, 0x1712C)
    return out


REGISTRIES = {
    'no product key': set(),
    'the key in HKCU (an older launcher made it)': {(HKCU, PRODUCT_KEY.lower())},
    'the key in HKLM only (an old installation)': {(HKLM, PRODUCT_KEY.lower())},
}
# Revision 39's patch 4: the four opens went to HKCU's product key.
REV39_ROWS = [(va, '6802000080', '6801000080', 'HKLM becomes HKCU')
              for va in (0x15231, 0x159A4, 0x16301, 0x170AD)]
FIX = {0x15223, 0x153FE, 0x1541E, 0x15997, 0x15A4D, 0x162F4, 0x16338, 0x1709F, 0x17123, 0x1714F}


def main():
    original = Path(sys.argv[1]).read_bytes()
    code = (HERE / 'launcher_values_026g.bin').read_bytes()
    assert FIX <= {row[0] for row in L.PATCHES}
    patched, _ = L.build(original, code, site=L.DEFAULT_SITE)
    failures = 0
    for delta in (0, 0x100000):
        for registry, keys in REGISTRIES.items():
            for where, (one, ok) in check(bytes(patched), delta, keys).items():
                ok = ok and not one.calls and not one.boxes
                failures += not ok
                print(f"{'ok  ' if ok else 'FAIL'} base+{delta:#x}, {registry}: {where}"
                      + ('' if ok else f": registry calls {one.calls}, boxes {one.boxes}"))

    # Revision 39's launcher, built the same way, shows the box.
    rows = L.PATCHES
    L.PATCHES = [row for row in rows if row[0] not in FIX] + REV39_ROWS
    try:
        rev39, _ = L.build(original, code, site=L.DEFAULT_SITE)
    finally:
        L.PATCHES = rows
    one, ok = start_updater(bytes(rev39), 0, set())
    seen = not ok and one.boxes == [CANT_OPEN] and one.calls == [('RegOpenKeyExW', HKCU, PRODUCT_KEY)]
    failures += not seen
    print(f"{'ok  ' if seen else 'FAIL'} revision 39's launcher, no product key: start_updater calls "
          f"{one.calls}, shows {one.boxes}")
    if failures:
        sys.exit(f'{failures} REGISTRY CASE(S) FAILED')
    print('ALL REGISTRY CASES PASSED')


if __name__ == '__main__':
    main()
