"""Exercise crash_probe.exe under Wine and inspect the resulting zipped minidumps.

Usage: python3 check_dumps.py /absolute/path/to/build [case ...]
The build directory must contain crash_probe.exe and BugTrap.dll.
Set WINEPREFIX to an isolated prefix, and DISPLAY to an X server (or Xvfb).
"""
import hashlib
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zipfile


def unzip(path):
    """The one dump inside SRP-crash-...-N.zip, checked as a ZIP."""
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None, path
        [entry] = [e for e in archive.infolist() if e.filename.endswith(".dmp")]
        assert all(e.filename == entry.filename or e.filename.startswith("logs/") for e in archive.infolist())
        assert len(archive.infolist()) <= 34
        assert entry.filename == path.stem + ".dmp", entry.filename
        assert entry.compress_type == zipfile.ZIP_DEFLATED and not entry.flag_bits & 0x800
        return archive.read(entry)


def exception_stream(path, snapshot=False):
    data = unzip(path) if path.suffix == ".zip" else path.read_bytes()
    assert data[:4] == b"MDMP", path
    count, directory = struct.unpack_from("<II", data, 8)
    exception = None
    modules = []
    comment = None
    for index in range(count):
        kind, size, offset = struct.unpack_from("<III", data, directory + index * 12)
        assert offset + size <= len(data), "truncated stream"
        if kind == 6:
            thread, _, code = struct.unpack_from("<III", data, offset)
            exception = thread, code
        if kind == 4:
            module_count = struct.unpack_from("<I", data, offset)[0]
            assert 4 + module_count * 108 <= size
            for index in range(module_count):
                name_rva = struct.unpack_from("<I", data, offset + 4 + index * 108 + 20)[0]
                length = struct.unpack_from("<I", data, name_rva)[0]
                modules.append(data[name_rva+4:name_rva+4+length].decode("utf-16le").lower())
        if kind == 10:
            comment = data[offset:offset+size]
    if not snapshot:
        assert exception is not None, "dump has no exception stream"
    assert any(m.endswith("crash_probe.exe") for m in modules), "missing EXE module"
    assert any(m.endswith("bugtrap.dll") for m in modules), "missing BugTrap module"
    assert comment == b"SRP probe: preserved assertion message\0", "missing game error message"
    return exception


def zip_blocked(exe, cwd, fallback, env):
    """A ZIP that cannot be written keeps the dump. The test DbgHelp holds the
    writer (concurrent mode) while the empty dump file already exists, so its
    name is known: a folder in the place of the temporary ZIP blocks it."""
    before, zips = set(exe.glob("*.dmp")), set(exe.glob("*.zip"))
    process = subprocess.Popen(["wine", str(exe / "crash_probe.exe"), "concurrent", "Z:" + str(fallback)],
                               cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    blocker = None
    deadline = time.monotonic() + 30
    while blocker is None and time.monotonic() < deadline and process.poll() is None:
        for dump in set(exe.glob("*.dmp")) - before:
            blocker = dump.with_suffix(".zip.part")
            blocker.mkdir()
        time.sleep(0.005)
    stdout, stderr = process.communicate(timeout=45)
    output = stdout.decode(errors="replace")
    assert blocker and "concurrent_crash_waited_for_dump_only" in output, (output, process.returncode)
    [dump] = set(exe.glob("*.dmp")) - before
    assert set(exe.glob("*.zip")) == zips and blocker.is_dir() and not list(blocker.iterdir()), list(exe.iterdir())
    thread = int(re.search(r"fault_thread=(\d+)", output)[1])
    assert exception_stream(dump) == (thread, 0xC0000005)
    blocker.rmdir()
    dump.unlink()
    print("PASS zip-blocked: the dump is kept when its ZIP cannot be written", flush=True)


def main():
    build = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="srp-crash-test-") as temporary:
        root = Path(temporary)
        exe = root / "game-Żółw"
        exe.mkdir()
        cwd = root / "different-working-directory"
        cwd.mkdir()
        fallback = root / "fallback"
        fallback.mkdir()
        for name in ("crash_probe.exe", "BugTrap.dll"):
            shutil.copy2(build / name, exe / name)
        env = os.environ.copy()
        previous = {}
        modes = sys.argv[2:] or ("silent", "cpp", "thread", "pre-handler", "stack", "cancel", "discord", "silent",
                     "continue-cpp", "continue-seh", "continue-net",
                     "execute-cpp", "execute-seh", "execute-net", "recursive", "concurrent", "snapshot-overlap",
                     "zip-blocked", "fallback", "failure")
        for mode in modes:
            if mode == "zip-blocked":
                shutil.copy2(build / "delayed_dbghelp.dll", exe / "dbghelp.dll")
                run_env = dict(env, WINEDLLOVERRIDES=env.get("WINEDLLOVERRIDES", "") + ";dbghelp=n,b")
                try:
                    zip_blocked(exe, cwd, fallback, run_env)
                finally:
                    (exe / "dbghelp.dll").unlink(missing_ok=True)
                continue
            folder = fallback if mode == "fallback" else exe
            before = set(folder.glob("*.zip"))
            run_env = env.copy()
            if mode in ("concurrent", "snapshot-overlap"):
                shutil.copy2(build / "delayed_dbghelp.dll", exe / "dbghelp.dll")
                run_env["WINEDLLOVERRIDES"] = env.get("WINEDLLOVERRIDES", "") + ";dbghelp=n,b"
            if mode in ("fallback", "failure"):
                exe.chmod(0o555)
            if mode == "failure":
                fallback.chmod(0o555)
            try:
                result = subprocess.run(
                    ["wine", str(exe / "crash_probe.exe"),
                     "silent" if mode == "fallback" else mode, "Z:" + str(fallback)],
                    cwd=cwd, env=run_env, capture_output=True, timeout=45,
                )
            finally:
                exe.chmod(0o755)
                fallback.chmod(0o755)
                (exe / "dbghelp.dll").unlink(missing_ok=True)
            output = result.stdout.decode(errors="replace")
            new = set(folder.glob("*.zip")) - before
            # A zipped dump leaves neither the dump nor the temporary ZIP.
            assert not list(folder.glob("*.dmp")) and not list(folder.glob("*.part")), list(folder.iterdir())
            if mode == "failure":
                assert not new and "failed_dump_dialog" in output, (mode, output, result.returncode)
                print("PASS failure: explicit failure dialog when neither folder is writable", flush=True)
                continue
            if mode == "snapshot-overlap":
                assert len(new) == 2, (mode, output, result.returncode, new)
                snapshots = [p for p in new if exception_stream(p, snapshot=True) is None]
                assert len(snapshots) == 1, "expected a snapshot and a separate crash dump"
                snapshot = snapshots[0]
                previous[snapshot] = hashlib.sha256(snapshot.read_bytes()).digest()
                new.remove(snapshot)
                assert "snapshot_saved" in output and "recursive_capture_rejected" in output, output
            assert len(new) == 1, (mode, result.returncode, output, result.stderr.decode(errors="replace"), new)
            dump = new.pop()
            thread = int(re.search(r"fault_thread=(\d+)", output)[1])
            code = 0xC00000FD if mode == "stack" else 0xC0000005
            assert exception_stream(dump) == (thread, code), mode
            if mode == "pre-handler":
                assert result.returncode == 23, result.returncode
            if mode in ("cancel", "discord"):
                assert "closed_dialog" in output, (output, result.returncode)
            if mode == "discord":
                assert "discord_invite_opened_dialog_kept" in output, output
            expected = ("outer_handler_reached" if mode.startswith("continue-") else
                        "synthetic_returned" if mode.startswith("execute-") else
                        "recursive_capture_rejected" if mode == "recursive" else
                        "concurrent_crash_waited_for_dump_only" if mode == "concurrent" else
                        "snapshot_overlap_completed" if mode == "snapshot-overlap" else None)
            if expected:
                assert expected in output and result.returncode == 0, (mode, output, result.returncode)
            assert not list(cwd.iterdir()), "dump used CWD instead of EXE directory"
            for old, digest in previous.items():
                assert hashlib.sha256(old.read_bytes()).digest() == digest, "old dump changed or deleted"
            previous[dump] = hashlib.sha256(dump.read_bytes()).digest()
            print(f"PASS {mode}: valid zipped dump, faulting thread {thread}, previous dumps retained", flush=True)


if __name__ == "__main__":
    main()
