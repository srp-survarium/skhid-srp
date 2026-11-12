"""Memory sources for the Bullet world dump (docs/notes/collision_world.md).

Three sources with one interface (read(addr, n) -> bytes or None):

  LiveProcess(pid)   a running 32-bit game, read from outside (ReadProcessMemory). Nothing is
                     written to the game and none of its code is run or changed.
  Recorder(mem)      wraps a source and keeps every range read, to be saved as a capture.
  Capture(path)      a saved capture: the sparse memory image a walk touched.

Capture file (little endian):
  magic  b"SRPBWC1\n"
  then one zlib stream of:
    u32 meta_len, meta (UTF-8 JSON: pid, exe, exe_sha256, module_base, taken_utc, note, ...)
    u32 count, then count x (u32 addr, u32 len, bytes)      ranges sorted, merged, not overlapping
"""
import bisect
import ctypes
import ctypes.wintypes as wt
import json
import struct
import zlib

MAGIC = b"SRPBWC1\n"

PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010
MEM_COMMIT = 0x1000
MEM_PRIVATE = 0x20000
MEM_IMAGE = 0x1000000
PAGE_GUARD = 0x100
PAGE_NOACCESS = 0x01
LIST_MODULES_32BIT = 0x01


class MEMORY_BASIC_INFORMATION64(ctypes.Structure):
    _fields_ = [
        ("BaseAddress", ctypes.c_ulonglong),
        ("AllocationBase", ctypes.c_ulonglong),
        ("AllocationProtect", wt.DWORD),
        ("__alignment1", wt.DWORD),
        ("RegionSize", ctypes.c_ulonglong),
        ("State", wt.DWORD),
        ("Protect", wt.DWORD),
        ("Type", wt.DWORD),
        ("__alignment2", wt.DWORD),
    ]


class Mem:
    """Typed reads over read(addr, n)."""

    def read(self, addr, n):
        raise NotImplementedError

    def need(self, addr, n):
        data = self.read(addr, n)
        if data is None or len(data) != n:
            raise MemoryError(f"unreadable {addr:#x}+{n:#x}")
        return data

    def u8(self, addr):
        return self.need(addr, 1)[0]

    def u16(self, addr):
        return struct.unpack("<H", self.need(addr, 2))[0]

    def i32(self, addr):
        return struct.unpack("<i", self.need(addr, 4))[0]

    def u32(self, addr):
        return struct.unpack("<I", self.need(addr, 4))[0]

    def f32(self, addr):
        return struct.unpack("<f", self.need(addr, 4))[0]

    def f32s(self, addr, count):
        return struct.unpack(f"<{count}f", self.need(addr, 4 * count))

    def u32s(self, addr, count):
        return struct.unpack(f"<{count}I", self.need(addr, 4 * count))

    def cstr(self, addr, limit=260):
        data = self.read(addr, limit)
        if data is None:
            # the string may end before an unreadable page: read byte-wise chunks
            out = b""
            for i in range(0, limit, 16):
                part = self.read(addr + i, 16)
                if part is None:
                    break
                out += part
                if b"\0" in part:
                    break
            data = out
        end = data.find(b"\0")
        if end < 0:
            return None
        try:
            return data[:end].decode("ascii")
        except UnicodeDecodeError:
            return None


class LiveProcess(Mem):
    def __init__(self, pid):
        self.pid = pid
        self.k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        self.psapi = ctypes.WinDLL("psapi", use_last_error=True)
        self.k32.OpenProcess.restype = wt.HANDLE
        self.k32.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
        self.k32.ReadProcessMemory.argtypes = [
            wt.HANDLE, ctypes.c_ulonglong, ctypes.c_void_p, ctypes.c_size_t,
            ctypes.POINTER(ctypes.c_size_t)]
        self.k32.VirtualQueryEx.argtypes = [
            wt.HANDLE, ctypes.c_ulonglong, ctypes.POINTER(MEMORY_BASIC_INFORMATION64),
            ctypes.c_size_t]
        self.k32.VirtualQueryEx.restype = ctypes.c_size_t
        self.handle = self.k32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)
        if not self.handle:
            raise OSError(f"OpenProcess({pid}) failed: {ctypes.get_last_error()}")

    def close(self):
        if self.handle:
            self.k32.CloseHandle(self.handle)
            self.handle = None

    def read(self, addr, n):
        buf = ctypes.create_string_buffer(n)
        got = ctypes.c_size_t(0)
        ok = self.k32.ReadProcessMemory(self.handle, addr, buf, n, ctypes.byref(got))
        if not ok or got.value != n:
            return None
        return buf.raw

    def regions(self):
        """Committed, readable regions below 4 GiB: (base, size, type, protect)."""
        out = []
        addr = 0x10000
        mbi = MEMORY_BASIC_INFORMATION64()
        while addr < 0x1_0000_0000:
            got = self.k32.VirtualQueryEx(self.handle, addr, ctypes.byref(mbi), ctypes.sizeof(mbi))
            if not got:
                break
            size = mbi.RegionSize
            if (mbi.State == MEM_COMMIT and not (mbi.Protect & PAGE_GUARD)
                    and mbi.Protect != PAGE_NOACCESS and mbi.Protect != 0):
                out.append((mbi.BaseAddress, size, mbi.Type, mbi.Protect))
            addr = mbi.BaseAddress + size
        return out

    def modules(self):
        """(base, path) of the 32-bit modules; the exe is first."""
        self.psapi.EnumProcessModulesEx.argtypes = [
            wt.HANDLE, ctypes.c_void_p, wt.DWORD, ctypes.POINTER(wt.DWORD), wt.DWORD]
        self.psapi.GetModuleFileNameExW.argtypes = [wt.HANDLE, ctypes.c_void_p, wt.LPWSTR, wt.DWORD]
        arr = (ctypes.c_void_p * 1024)()
        needed = wt.DWORD(0)
        if not self.psapi.EnumProcessModulesEx(self.handle, arr, ctypes.sizeof(arr),
                                               ctypes.byref(needed), LIST_MODULES_32BIT):
            raise OSError(f"EnumProcessModulesEx failed: {ctypes.get_last_error()}")
        out = []
        for i in range(needed.value // ctypes.sizeof(ctypes.c_void_p)):
            name = ctypes.create_unicode_buffer(520)
            self.psapi.GetModuleFileNameExW(self.handle, arr[i], name, 520)
            out.append((arr[i] or 0, name.value))
        return out

    def scan_u32(self, value, private_only=True):
        """Every 4-aligned address in committed memory holding this dword."""
        needle = struct.pack("<I", value)
        hits = []
        for base, size, kind, _ in self.regions():
            if private_only and kind != MEM_PRIVATE:
                continue
            step = 1 << 22
            for off in range(0, size, step):
                chunk = self.read(base + off, min(step, size - off))
                if chunk is None:
                    # a region may have changed under us: fall back to pages
                    for page in range(off, min(off + step, size), 0x1000):
                        part = self.read(base + page, 0x1000)
                        if part:
                            hits += _find_aligned(part, needle, base + page)
                    continue
                hits += _find_aligned(chunk, needle, base + off)
        return hits


def _find_aligned(chunk, needle, base):
    out = []
    at = chunk.find(needle)
    while at >= 0:
        if at % 4 == 0:
            out.append(base + at)
        at = chunk.find(needle, at + 1)
    return out


class Recorder(Mem):
    """Keeps every range read through it."""

    def __init__(self, mem):
        self.mem = mem
        self.ranges = {}

    def read(self, addr, n):
        data = self.mem.read(addr, n)
        if data is not None:
            old = self.ranges.get(addr)
            if old is None or len(old) < n:
                self.ranges[addr] = data
        return data

    def merged(self):
        out = []
        for addr in sorted(self.ranges):
            data = self.ranges[addr]
            if out and addr <= out[-1][0] + len(out[-1][1]):
                last_addr, last = out[-1]
                keep = addr - last_addr
                if keep + len(data) > len(last):
                    out[-1] = (last_addr, last[:keep] + data)
            else:
                out.append((addr, data))
        return out

    def save(self, path, meta):
        body = bytearray()
        blob = json.dumps(meta, sort_keys=True).encode()
        body += struct.pack("<I", len(blob)) + blob
        ranges = self.merged()
        body += struct.pack("<I", len(ranges))
        for addr, data in ranges:
            body += struct.pack("<II", addr, len(data)) + data
        with open(path, "wb") as out:
            out.write(MAGIC)
            out.write(zlib.compress(bytes(body), 6))
        return len(ranges), len(body)


class Capture(Mem):
    def __init__(self, path):
        with open(path, "rb") as src:
            if src.read(len(MAGIC)) != MAGIC:
                raise ValueError(f"{path}: not a capture")
            body = zlib.decompress(src.read())
        (n,) = struct.unpack_from("<I", body, 0)
        self.meta = json.loads(body[4:4 + n].decode())
        at = 4 + n
        (count,) = struct.unpack_from("<I", body, at)
        at += 4
        self.starts = []
        self.blobs = []
        for _ in range(count):
            addr, size = struct.unpack_from("<II", body, at)
            at += 8
            self.starts.append(addr)
            self.blobs.append(body[at:at + size])
            at += size

    def read(self, addr, n):
        i = bisect.bisect_right(self.starts, addr) - 1
        if i < 0:
            return None
        off = addr - self.starts[i]
        blob = self.blobs[i]
        if off + n > len(blob):
            return None
        return blob[off:off + n]
