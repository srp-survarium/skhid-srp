#!/usr/bin/env python3
"""Runs the launcher's language code (launcher_values_026g.asm, "The
language") under Unicorn, in the patched launcher, at its own base and
rebased, with Windows' functions stubbed and user.cfg in memory.

Usage: python check_language_026g.py ORIGINAL_survarium_launcher.exe

Checks: the start reads user.cfg's `localization` line, else the Windows
user locale's language, else English, and writes nothing; a choice rewrites
only that line of user.cfg (added when missing, the file and its folder
made when missing), through a new file renamed over it, and leaves a file it
cannot read whole alone; the menu lists every language by its own name with
the current one checked and hands the pick to language_selected; the
button's label, the launcher's LANGID and the page's code of every language.
"""
from pathlib import Path
import struct
import sys
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP
import patch_launcher_026g as L

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from localization_026g import LANGUAGES, LAUNCHER_UI  # noqa: E402

STACK, HEAP, STUBS, SETTINGS, MENU, STOP = 0x30008000, 0x40000000, 0x50000000, 0x31000000, 0x31001000, 0x32000000
DOCUMENTS = 'C:\\Users\\p\\Documents'
CFG = DOCUMENTS + '\\survarium\\user.cfg'
# (IAT slot, stub name)
IAT = {0x3B098: 'CloseHandle', 0x3B0EC: 'ReadFile', 0x3B0F8: 'WriteFile', 0x3B118: 'GetProcessHeap',
       0x3B238: 'HeapFree', 0x3B23C: 'HeapAlloc', 0x3B254: 'CreateFileA', 0x3B258: 'GetModuleHandleA',
       0x3B1D8: 'GetProcAddress', 0x3B218: 'GetLastError', 0x3B110: 'DeleteFileA',
       0x3B2BC: 'SHGetFolderPathA', 0x3B2C0: 'SHCreateDirectoryExA', 0x3B394: 'GetWindowRect'}
# Stubs: name -> number of stdcall arguments.
ARGS = {'CloseHandle': 1, 'ReadFile': 5, 'WriteFile': 5, 'GetProcessHeap': 0, 'HeapFree': 3, 'HeapAlloc': 3,
        'CreateFileA': 7, 'GetModuleHandleA': 1, 'GetProcAddress': 2, 'GetLastError': 0, 'DeleteFileA': 1,
        'SHGetFolderPathA': 5, 'SHCreateDirectoryExA': 3, 'GetWindowRect': 2, 'GetUserDefaultLCID': 0,
        'MoveFileExA': 3, 'CreatePopupMenu': 0, 'AppendMenuW': 4, 'TrackPopupMenu': 7, 'DestroyMenu': 1}
NAMES = list(ARGS)
LANGUAGE_SELECTED = 0x19A20
L_CFG_SIZE = 0x10000                    # launcher_values_026g.asm CFG_SIZE


class Launcher:
    def __init__(self, data, delta):
        self.delta = delta
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = L.P.Headers(data)
        for _, rva, vs, raw, rs, _ in headers.sections(data):
            address = L.IMAGE_BASE + delta + rva
            u.mem_map(address, (max(vs, rs) + 4095) & ~4095)
            u.mem_write(address, bytes(data[raw:raw + min(rs, vs or rs)]))
        for address in L.relocated(data):
            self.word(address + delta, self.word(address + delta) + delta)
        for address, size in [(STACK - 0x10000, 0x20000), (HEAP, 0x100000), (STUBS, 0x1000),
                              (SETTINGS, 0x1000), (MENU, 0x1000), (STOP, 0x1000)]:
            u.mem_map(address, size)
        for slot, name in IAT.items():
            self.word(slot + delta, STUBS + 4 * NAMES.index(name))
        self.files, self.folders, self.handles = {}, set(), {}
        self.heap, self.live = HEAP, set()
        self.lcid, self.last_error, self.menu, self.pick, self.selected = 0x0409, 0, [], 0, []
        self.locked = set()
        u.hook_add(UC_HOOK_CODE, self.hook)

    def word(self, address, value=None):
        if value is None:
            return struct.unpack('<I', self.u.mem_read(address, 4))[0]
        self.u.mem_write(address, struct.pack('<I', value & 0xffffffff))

    def ansi(self, address):
        out = bytearray()
        while (byte := self.u.mem_read(address + len(out), 1)[0]):
            out.append(byte)
        return out.decode('latin-1')

    def wide(self, address):
        out = bytearray()
        while (pair := bytes(self.u.mem_read(address + len(out), 2))) != b'\0\0':
            out += pair
        return out.decode('utf-16-le')

    def hook(self, u, address, size, _):
        if address == STOP:
            u.emu_stop()
            return
        if address == LANGUAGE_SELECTED + self.delta:
            context = self.word(u.reg_read(UC_X86_REG_ESP) + 4)
            self.selected.append((self.word(context), self.word(context + 4)))
            self.ret(0, 0)
            return
        if not STUBS <= address < STUBS + 4 * len(NAMES):
            return
        name = NAMES[(address - STUBS) // 4]
        sp = u.reg_read(UC_X86_REG_ESP)
        args = [self.word(sp + 4 + 4 * i) for i in range(ARGS[name])]
        self.ret(getattr(self, name)(*args), ARGS[name])

    def ret(self, value, count):
        sp = self.u.reg_read(UC_X86_REG_ESP)
        self.u.reg_write(UC_X86_REG_EAX, value & 0xffffffff)
        self.u.reg_write(UC_X86_REG_EIP, self.word(sp))
        self.u.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)

    # --- Windows, as far as the code uses it ---
    def SHGetFolderPathA(self, hwnd, csidl, token, flags, path):
        assert csidl == 5 and path
        self.u.mem_write(path, DOCUMENTS.encode() + b'\0')
        return 0

    def SHCreateDirectoryExA(self, hwnd, path, attributes):
        self.folders.add(self.ansi(path).lower())
        return 0

    def CreateFileA(self, path, access, share, security, disposition, flags, template):
        path = self.ansi(path)
        if path.lower() in self.locked:
            self.last_error = 32
            return -1
        if disposition == 3:                        # OPEN_EXISTING, to read
            assert access == 0x80000000
            if path not in self.files:
                folder = path.rsplit('\\', 1)[0].lower()
                self.last_error = 2 if folder in self.folders or not path.startswith(DOCUMENTS) else 3
                return -1
            data = self.files[path]
        else:
            assert disposition == 2 and access == 0x40000000
            assert path.rsplit('\\', 1)[0].lower() in self.folders, 'a file in a folder never made'
            self.files[path] = data = b''
        handle = 0x100 + len(self.handles) * 4
        self.handles[handle] = [path, data, 0]
        return handle

    def ReadFile(self, handle, buffer, size, read, overlapped):
        path, data, at = self.handles[handle]
        part = data[at:at + size]
        self.u.mem_write(buffer, part)
        self.handles[handle][2] += len(part)
        self.word(read, len(part))
        return 1

    def WriteFile(self, handle, buffer, size, written, overlapped):
        path = self.handles[handle][0]
        self.files[path] += bytes(self.u.mem_read(buffer, size))
        self.word(written, size)
        return 1

    def CloseHandle(self, handle):
        del self.handles[handle]
        return 1

    def GetLastError(self):
        return self.last_error

    def DeleteFileA(self, path):
        self.files.pop(self.ansi(path), None)
        return 1

    def MoveFileExA(self, existing, new, flags):
        assert flags == 1
        self.files[self.ansi(new)] = self.files.pop(self.ansi(existing))
        return 1

    def GetProcessHeap(self):
        return 0x7000

    def HeapAlloc(self, heap, flags, size):
        address, self.heap = self.heap, self.heap + ((size + 15) & ~15)
        self.live.add(address)
        return address

    def HeapFree(self, heap, flags, address):
        self.live.remove(address)
        return 1

    def GetModuleHandleA(self, name):
        return {'kernel32.dll': 0x7100, 'user32.dll': 0x7200}[self.ansi(name).lower()]

    def GetProcAddress(self, module, name):
        name = self.ansi(name)
        assert (module, name) in {(0x7100, 'GetUserDefaultLCID'), (0x7100, 'MoveFileExA'),
                                  (0x7200, 'CreatePopupMenu'), (0x7200, 'AppendMenuW'),
                                  (0x7200, 'TrackPopupMenu'), (0x7200, 'DestroyMenu')}, name
        return STUBS + 4 * NAMES.index(name)

    def GetUserDefaultLCID(self):
        return self.lcid

    def CreatePopupMenu(self):
        self.menu = []
        return 0x900

    def AppendMenuW(self, menu, flags, command, text):
        assert menu == 0x900
        self.menu.append((flags, command, self.wide(text)))
        return 1

    def GetWindowRect(self, window, rect):
        assert window == 0x777
        self.u.mem_write(rect, struct.pack('<iiii', 766, 12, 800, 30))
        return 1

    def TrackPopupMenu(self, menu, flags, x, y, reserved, window, rect):
        assert (menu, flags, x, y, window) == (0x900, 0x180, 766, 30, 0x777)
        return self.pick

    def DestroyMenu(self, menu):
        assert menu == 0x900
        self.menu_destroyed = True
        return 1

    # --- the launcher's code ---
    def run(self, start, eax=0, args=(), stop_at=STOP, esi=0):
        sp = STACK - 4 * (len(args) + 1)
        self.word(sp, STOP)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        self.u.reg_write(UC_X86_REG_ESP, sp)
        self.u.reg_write(UC_X86_REG_EAX, eax)
        self.u.reg_write(UC_X86_REG_ESI, esi)
        self.u.emu_start(start + self.delta, stop_at if stop_at == STOP else stop_at + self.delta, count=200000)
        return self.u.reg_read(UC_X86_REG_EAX), self.u.reg_read(UC_X86_REG_ESP) - sp

    def reload(self):
        self.word(SETTINGS + 0x1C, 0xEEEE)
        _, moved = self.run(0x1D852, eax=SETTINGS)
        assert moved == 4, 'reload_language returns as it was called'
        assert not self.live, 'a buffer left unfreed'
        return self.word(SETTINGS + 0x1C)

    def save(self, index):
        self.word(SETTINGS + 0x1C, index)
        _, moved = self.run(0x1D8DA, args=(SETTINGS,))
        assert moved == 8, 'save_language takes its argument (ret 4)'
        assert not self.live and not self.handles, 'a buffer or a file left open'


def check(data, delta):
    names = [name for name, *_ in LANGUAGES]
    index = names.index
    count = 0

    def launcher(files=None, lcid=0x0409, folder=True):
        launcher = Launcher(data, delta)
        launcher.files = dict(files or {})
        if folder:
            launcher.folders.add((DOCUMENTS + '\\survarium').lower())
        launcher.lcid = lcid
        return launcher

    # The start: the line, else the locale, else English; nothing written.
    cases = [
        ({CFG: b'account_name x\r\nlocalization czech\r\nr_fullscreen on\r\n'}, 0x0419, 'czech'),
        ({CFG: b'localization belarusian'}, 0x0409, 'belarusian'),
        ({CFG: b'localization\tukrainian\r\n'}, 0x0409, 'ukrainian'),
        ({CFG: b'localization original\r\n'}, 0x0419, 'russian'),
        ({CFG: b'localization Czech\r\n'}, 0x0411, 'english'),
        ({CFG: b'localizations czech\r\n# localization german\r\n'}, 0x0405, 'czech'),
        ({CFG: b'a 1\nlocalization hungarian\n'}, 0x0409, 'hungarian'),
        ({}, 0x081A, 'serbian'),
        ({}, 0x0C07, 'german'),
        ({}, 0x0411, 'english'),
    ]
    for files, lcid, expected in cases:
        one = launcher(files, lcid)
        assert one.reload() == index(expected), (files, lcid, expected, one.word(SETTINGS + 0x1C))
        assert one.files == files, 'the start writes nothing'
        count += 1
    one = launcher({}, 0x0405, folder=False)
    assert one.reload() == index('czech') and not one.files and not one.folders
    count += 1

    # A choice: the line replaced, added, or the file made; others kept.
    body = b'account_name x\r\nbind artefact kH\r\nlocalization polish\r\nr_fullscreen on\r\n'
    for chosen in names:
        one = launcher({CFG: body})
        one.save(index(chosen))
        assert one.files == {CFG: body.replace(b'polish', chosen.encode())}, one.files
        assert one.reload() == index(chosen)
        count += 1
    for before, after in [
            (b'a 1\r\nb 2\r\n', b'a 1\r\nb 2\r\nlocalization finnish\r\n'),
            (b'a 1\r\nb 2', b'a 1\r\nb 2\r\nlocalization finnish\r\n'),
            (b'', b'localization finnish\r\n'),
            (b'localization   english   \r\nz 1', b'localization finnish\r\nz 1'),
            (b'x 1\nlocalization english\nz 1\n', b'x 1\nlocalization finnish\nz 1\n')]:
        one = launcher({CFG: before})
        one.save(index('finnish'))
        assert one.files == {CFG: after}, (before, one.files)
        count += 1
    one = launcher({}, folder=False)
    one.save(index('czech'))
    assert one.files == {CFG: b'localization czech\r\n'} and (DOCUMENTS + '\\survarium').lower() in one.folders
    count += 1
    big = b'#' * L_CFG_SIZE + b'\r\nlocalization polish\r\n'
    one = launcher({CFG: big})
    one.save(index('czech'))
    assert one.files == {CFG: big}, 'a file too big to read whole is left alone'
    one = launcher({CFG: body})
    one.locked.add(CFG.lower())
    one.save(index('czech'))
    assert one.files == {CFG: body}, 'a file that cannot be opened is left alone'
    count += 2

    # The menu: every language by its own name, the current one checked.
    for current in (0, index('czech')):
        for pick in (0, index('belarusian') + 1, 1):
            one = launcher()
            one.word(SETTINGS + 0x1C, current)
            one.word(MENU + 0x4, 0x777)
            one.word(MENU + 0x154, SETTINGS)
            one.pick = pick
            _, moved = one.run(0x19A08, args=(MENU,))
            assert moved == 4, 'the click returns as it was called (cdecl)'
            assert one.menu == [(8 if i == current else 0, i + 1, native)
                                for i, (_, native, *_) in enumerate(LANGUAGES)], one.menu
            assert one.selected == ([(MENU, pick - 1)] if pick else [])
            count += 1

    # The label, the LANGID and the page's code of every language.
    for i, (name, native, windows, code) in enumerate(LANGUAGES):
        one = launcher()
        label, _ = one.run(0x19663, eax=i, stop_at=0x1966A)
        assert one.wide(label) == code.upper(), (name, one.wide(label))
        one.run(0x150B3, eax=i, stop_at=0x150BB)
        assert one.u.reg_read(UC_X86_REG_ESI) == LAUNCHER_UI.get(code, 0x0409), name
        one.word(0x4819C + delta, i)
        page, _ = one.run(0x1A84B)
        assert one.wide(page) == (code if code in LAUNCHER_UI else 'en'), name
        count += 1
    return count



def main():
    original = Path(sys.argv[1]).read_bytes()
    code = (HERE / 'launcher_values_026g.bin').read_bytes()
    patched, _ = L.build(original, code)
    for delta in (0, 0x100000):
        count = check(bytes(patched), delta)
        print(f'base+{delta:#x}: {count} language cases pass')
    print('ALL LANGUAGE CASES PASSED')


if __name__ == '__main__':
    main()
