"""What memory a crash dump holds: runs memory_probe.exe under Wine and reads its dump.

Usage: python3 check_memory.py /absolute/path/to/build
The build directory holds memory_probe.exe, callback_dbghelp.dll and
BugTrap.dll: a player's build, or one with the full-memory-dumps feature
(the crash report's Reporter line says which). Set WINEPREFIX to an isolated
prefix. No window is opened.

Two runs. With callback_dbghelp.dll as dbghelp.dll: the dump type BugTrap
asks for, and the ranges its memory callback gives. With Wine's own dbghelp:
the dump itself. Wine's MiniDumpWriteDump ignores DataSegs,
IndirectlyReferencedMemory and memory callbacks, so there a player's dump
holds neither probe object; Windows' writes both.
  A player's build: DataSegs | IndirectlyReferencedMemory, a callback range
  [ESI - 256, ESI + 2048) around the object behind ESI, zipped; no
  unreferenced heap memory.
  A full-memory build: FullMemory | FullMemoryInfo | HandleData | ThreadInfo
  | UnloadedModules, no callback, a .dmp not zipped, holding all three objects.
"""
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import zipfile

PLAYER = 0x1 | 0x40
FULL = 0x2 | 0x800 | 0x4 | 0x1000 | 0x20


def marker(name):
    return f"SRP-MEMORY-PROBE-{name}".encode().ljust(32, b"#")


def exception_esi(data):
    assert data[:4] == b"MDMP"
    count, directory = struct.unpack_from("<II", data, 8)
    for index in range(count):
        kind, _, offset = struct.unpack_from("<III", data, directory + index * 12)
        if kind == 6:
            # MINIDUMP_EXCEPTION_STREAM: thread, alignment, a 152-byte
            # MINIDUMP_EXCEPTION, then the context's location; x86 CONTEXT.Esi
            # is at 0xA0.
            _, context = struct.unpack_from("<II", data, offset + 160)
            return struct.unpack_from("<I", data, context + 0xA0)[0]
    raise AssertionError("no exception stream")


def run(build, folder, shim):
    folder.mkdir()
    for name in ("memory_probe.exe", "BugTrap.dll"):
        shutil.copy2(build / name, folder / name)
    env = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY")}
    env["WINEDEBUG"] = "-all"
    if shim:
        shutil.copy2(build / "callback_dbghelp.dll", folder / "dbghelp.dll")
        env["WINEDLLOVERRIDES"] = env.get("WINEDLLOVERRIDES", "") + ";dbghelp=n,b"
    result = subprocess.run(["wine", str(folder / "memory_probe.exe")], cwd=folder, env=env,
                            capture_output=True, timeout=300)
    assert result.returncode == 0, (result.returncode, result.stderr.decode(errors="replace")[-2000:])
    [report] = folder.glob("SRP-crash-*.txt")
    full = "full-memory dumps" in report.read_text(errors="replace")
    if full:
        assert not list(folder.glob("SRP-crash-*.zip")), "a full-memory dump is not zipped"
        [dump] = folder.glob("SRP-crash-*.dmp")
        data, stored = dump.read_bytes(), dump.stat().st_size
    else:
        assert not list(folder.glob("SRP-crash-*.dmp")), "a player's dump is zipped"
        [archive] = folder.glob("SRP-crash-*.zip")
        with zipfile.ZipFile(archive) as z:
            [entry] = [e for e in z.infolist() if e.filename.endswith(".dmp")]
            data = z.read(entry)
        stored = archive.stat().st_size
    return full, data, stored


def main():
    build = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="srp-memory-test-") as temporary:
        root = Path(temporary)
        full, data, _ = run(build, root / "shim", True)
        lines = (root / "shim" / "dbghelp-request.txt").read_text().split()
        kind = int(lines[0].removeprefix("type="))
        ranges = [tuple(map(int, line.removeprefix("range=").split("+"))) for line in lines[1:]]
        esi = exception_esi(data)
        print(f"{'full-memory' if full else 'player'} build: dump type {kind:#x}, "
              f"callback ranges {[(hex(a), s) for a, s in ranges]}, ESI {esi:#x}")
        if full:
            assert kind == FULL and not ranges, (hex(kind), ranges)
        else:
            assert kind == PLAYER, hex(kind)
            assert (esi - 256, 256 + 2048) in ranges, "no range around the object behind ESI"
            assert len(ranges) <= 7

        full, data, stored = run(build, root / "wine", False)
        found = {name: marker(name) in data for name in ("register", "stack", "heap")}
        print(f"Wine's dbghelp: dump {len(data)} bytes, {stored} as stored; probe objects {found}")
        if full:
            assert all(found.values()), "a full-memory dump misses heap memory"
        else:
            assert not found["heap"], "a player's dump holds unreferenced heap memory"
        print("PASS", flush=True)


if __name__ == "__main__":
    main()
