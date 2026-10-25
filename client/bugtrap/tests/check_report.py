"""The crash's text report from a real, unhandled crash under Wine.

Usage: WINEPREFIX=/path/to/disposable-prefix python3 tests/check_report.py BUILD
BUILD holds BugTrap.dll and report_probe.exe. The probe redirects that
prefix's Documents folder to a temporary one holding a game log.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

build = Path(sys.argv[1]).resolve()
assert os.environ.get("WINEPREFIX"), "set a disposable WINEPREFIX"

# A game log as the game writes it: 30 records, one of them with a password.
# (The filter frames records by [HH:MM:SS.mmm]; the game writes [HH:MM:SS:mmm],
# so the records after a removed one go with it: omission, the safe side.)
LOG = "".join(
    f"game <INFO>\t[12:00:{n:02}:000]\t" + ("login password=hunter2" if n == 25 else f"record {n}") + "\r\n"
    for n in range(1, 31)
).encode()


def offset(text, line):
    """module+offset of a report line, as (module, offset)."""
    found = re.search(line + r"\s+(\S+)\+0x([0-9A-F]+)", text)
    assert found, (line, text)
    return found[1], int(found[2], 16)


with tempfile.TemporaryDirectory(prefix="srp-report-test-") as temporary:
    root = Path(temporary)
    game = root / "game"
    game.mkdir()
    for name in ("BugTrap.dll", "report_probe.exe"):
        shutil.copy2(build / name, game / name)
    docs = root / "Documents"
    (docs / "Survarium").mkdir(parents=True)
    (docs / "Survarium" / "Survarium_tester.log").write_bytes(LOG)
    for mode in ("write", "cpp"):
        before = set(game.iterdir())
        result = subprocess.run(["wine", str(game / "report_probe.exe"), "Z:" + str(docs), mode],
                                cwd=root, capture_output=True, timeout=60)
        output = result.stdout.decode()
        rva = {name: int(value) for name, value in re.findall(r"(\w+)_rva=(\d+)", output)}
        new = set(game.iterdir()) - before
        [text_file] = [p for p in new if p.suffix == ".txt"]
        [archive_file] = [p for p in new if p.suffix == ".zip"]
        assert text_file.stem == archive_file.stem, new
        data = text_file.read_bytes()
        with zipfile.ZipFile(archive_file) as archive:
            assert archive.testzip() is None
            assert archive.read("crash.txt") == data
        text = data.decode("utf-8")
        assert text.startswith("Skhid crash report, "), text
        assert "\r\n" in text and "\n" not in text.replace("\r\n", ""), "CRLF lines"
        if mode == "write":
            assert "\r\nException: 0xC0000005 ACCESS_VIOLATION (write to 0x00000000)\r\n" in text, text
            module, at = offset(text, "Address:")
            assert module == "report_probe.exe" and rva["fault"] <= at < rva["fault"] + 0x40, (at, rva, text)
        else:
            assert "\r\nException: 0xE06D7363 C++ exception (.?AVbad_alloc@std@@)\r\n" in text, text
        # The caller's return address is on the stack scan, as module+offset.
        stack = re.findall(r"report_probe\.exe\+0x([0-9A-F]+)", text.split("Stack:", 1)[1].split("Game:", 1)[0])
        assert any(rva["caller"] < int(at, 16) < rva["caller"] + 0x80 for at in stack), (stack, rva, text)
        assert re.search(r"\r\nThread:    \d+\r\nRegisters: EAX=[0-9A-F]{8} ", text), text
        assert "\r\nGame:      report_probe.exe, patch revision unknown, sha256 " in text, text
        assert re.search(r"\r\nWindows:   \d+\.\d+\.\d+ \(Wine [^)]+\)\r\n", text), text
        assert "\r\nDump:      saved\r\n" in text, text
        # The game log's last 20 lines, the password's record removed.
        tail = text.split("\r\nGame log:  ", 1)[1]
        assert "record 11\r\n" in tail and "record 24\r\n" in tail and "record 10\r\n" not in tail, tail
        assert "hunter2" not in text and "[SRP: sensitive log record removed]" in tail, tail
        print(f"PASS {mode}: {text.splitlines()[1]} at {offset(text, 'Address:')[0]}+..., "
              f"{len(stack)} probe frames on the stack, log tail filtered", flush=True)
