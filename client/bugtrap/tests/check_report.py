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

# A game log as 0.26g writes it ([HH:MM:SS.mmm]): 30 records, one of them with
# a password and an unframed continuation line; record 27 with the colon form
# [HH:MM:SS:mmm], which the filter takes too.
LOG = "".join(
    f"game <INFO>\t[12:00:{n:02}{':' if n == 27 else '.'}000]\t" + ("login password=hunter2\r\nsecond-hunter2" if n == 25 else f"record {n}") + "\r\n"
    for n in range(1, 31)
).encode()

# srp_patches.log beside the exe, as vostok/log.rs writes it: 3 MB of old
# lines (the ZIP takes its last 2 MiB from a whole line), then 130 lines, one
# of them with a token.
PATCH_LOG = b"".join(b"srp: old %07d padding padding padding padding\r\n" % n for n in range(60000)) + b"".join(
    (b"srp: login token=SYNTHETIC\r\n" if n == 120 else b"srp: event %d\r\n" % n) for n in range(1, 131))


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
    (game / "srp_patches.log").write_bytes(PATCH_LOG)
    # deep, misaligned: the review of PR 459, a stack 400 KB deep, ESP not
    # 4-aligned; the scan once hung there, and the game with it, with no dump.
    for mode in ("write", "cpp", "deep", "misaligned"):
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
            crumbs = archive.read("breadcrumbs.txt")
            patches = archive.read("logs/game-exe/srp_patches.log")
            manifest = archive.read("logs/collection.txt").decode()
        assert len(patches) <= 2 * 1024 * 1024 and patches.startswith(b"srp: old "), patches[:80]
        assert PATCH_LOG.endswith(patches.replace(b"[SRP: sensitive log record removed]\r\n", b"srp: login token=SYNTHETIC\r\n"))
        assert b"SYNTHETIC" not in patches and patches.endswith(b"srp: event 130\r\n"), patches[-200:]
        assert "tail of logs/game-exe/srp_patches.log: from byte " in manifest, manifest
        text = data.decode("utf-8")
        assert text.startswith("Skhid crash report, "), text
        assert re.match(r"Skhid crash report, \d{4}-\d\d-\d\d \d\d:\d\d:\d\d UTC \(local [+-]\d\d:\d\d\)\r\n", text), text
        assert "\r\n" in text and "\n" not in text.replace("\r\n", ""), "CRLF lines"
        if mode == "write":
            assert "\r\nException: 0xC0000005 ACCESS_VIOLATION (write to 0x00000000)\r\n" in text, text
            module, at = offset(text, "Address:")
            assert module == "report_probe.exe" and rva["fault"] <= at < rva["fault"] + 0x40, (at, rva, text)
        elif mode == "cpp":
            assert "\r\nException: 0xE06D7363 C++ exception (.?AVbad_alloc@std@@)\r\n" in text, text
        else:
            assert "\r\nException: 0xC0000005 ACCESS_VIOLATION (write to 0x00000000)\r\n" in text, text
            assert offset(text, "Address:")[0] == "report_probe.exe", text
            esp = int(re.search(r" ESP=([0-9A-F]{8}) ", text)[1], 16)
            assert (esp % 4 != 0) == (mode == "misaligned"), (mode, text)
        stack = re.findall(r"report_probe\.exe\+0x([0-9A-F]+)", text.split("Stack:", 1)[1].split("Game:", 1)[0])
        if mode in ("write", "cpp"):
            # The caller's return address is on the stack scan, as module+offset.
            assert any(rva["caller"] < int(at, 16) < rva["caller"] + 0x80 for at in stack), (stack, rva, text)
        assert re.search(r"\r\nThread:    \d+\r\nRegisters: EAX=[0-9A-F]{8} ", text), text
        assert "\r\nGame:      report_probe.exe, patch revision unknown, sha256 " in text, text
        assert re.search(r"\r\nWindows:   \d+\.\d+\.\d+ \(Wine [^)]+\)\r\n", text), text
        assert "\r\nDump:      saved\r\n" in text, text
        # The probe's breadcrumbs (its .srpx section): the match in the part
        # Copy copies, the events after the dump's line, and all in the ZIP.
        assert "\r\nMatch:     0x1092, map 12, 8 places, level 5; local place 3\r\n" in text, text
        reading = re.search(r"\r\nReading:   place 3's player state, from byte 420 of its stream, (\d+\.\d{3}) s before the crash\r\n", text)
        assert reading and 0.25 <= float(reading[1]) < 1.25, text
        # What Copy copies (everything to the dump's line, in a code block):
        # the Match and Reading lines, not the events; one Discord message.
        head = text.split("\r\nDump:      saved\r\n", 1)[0] + "\r\nDump:      saved\r\n"
        assert "\r\nMatch:     " in head and "\r\nReading:   " in head and "Events:" not in head, head
        assert len("```\r\n" + head + "```") <= 2000, len(head)
        events = text.split("\r\nDump:      saved\r\n", 1)[1].split("\r\nGame log:", 1)[0]
        lines = events.split("\r\n")
        assert lines[0] == "Events:    3 recorded, the last 3 kept: match 1 world 1 death 1", events
        ages = [float(re.match(r"  -(\d+\.\d{3}) s  ", line)[1]) for line in lines[2:5]]
        assert all(0 <= age - want < 1.0 for age, want in zip(ages, (3, 2, 1.5))), ages
        assert lines[3].endswith("world: 4500 ms, 900 bytes packed, loaded, local place 3")
        assert lines[4].endswith("death: place 4, life 2, at 61000 ms"), events
        assert crumbs.decode().startswith("Match:     0x1092") and crumbs.decode().count(" s  ") == 3, crumbs
        # The game log's last 20 lines, the password's record removed.
        tail = text.split("\r\nGame log:  ", 1)[1]
        assert all(f"record {n}\r\n" in tail for n in (12, 24, 26, 27, 30)), tail
        assert "record 11\r\n" not in tail, tail  # 31 lines: the 20 last start at record 12
        assert "hunter2" not in text and "[SRP: sensitive log record removed]" in tail, tail
        # srp_patches.log's last 100 lines: events 31..130, the token's line removed.
        patch = text.split("\r\nPatch log: last 100 lines of srp_patches.log\r\n", 1)[1]
        assert patch.startswith("srp: event 31\r\n") and patch.endswith("srp: event 130\r\n"), patch
        assert "SYNTHETIC" not in patch and "[SRP: sensitive log record removed]" in patch, patch
        print(f"PASS {mode}: {text.splitlines()[1]} at {offset(text, 'Address:')[0]}+..., "
              f"{len(stack)} probe frames on the stack, log tail filtered", flush=True)
