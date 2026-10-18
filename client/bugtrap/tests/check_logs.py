"""ZIP log attachments under Wine. Use a DISPOSABLE WINEPREFIX: this test
redirects that prefix's Documents folder. Build logs_probe.exe beside BugTrap.dll.
No existing Documents files are read or changed.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile

build = Path(sys.argv[1]).resolve()
assert os.environ.get("WINEPREFIX"), "set a disposable WINEPREFIX"
with tempfile.TemporaryDirectory(prefix="srp-log-test-") as temporary:
    root = Path(temporary)
    game_root = root / "game"
    game = game_root / "binaries" / "x86"
    game.mkdir(parents=True)
    install_temp = root / "temp"
    install_temp.mkdir()
    for name in ("BugTrap.dll", "logs_probe.exe"):
        shutil.copy2(build / name, game / name)
    docs = root / "Documents-Жółw"
    folder = docs / "Survarium"
    folder.mkdir(parents=True)
    now = time.time()

    def add(name, data, age=0, where=folder):
        path = where / name
        path.write_bytes(data)
        os.utime(path, (now - age, now - age))
        return path

    def check(expected, mode="none", lock=None, messages=()):
        old = set(game.glob("*.zip"))
        subprocess.run(["wine", str(game / "logs_probe.exe"), "Z:" + str(docs), mode,
                        "Z:" + str(lock or folder)], cwd=root, check=True,
                       capture_output=True, timeout=45)
        [path] = set(game.glob("*.zip")) - old
        with zipfile.ZipFile(path) as archive:
            assert archive.testzip() is None
            [dump] = [n for n in archive.namelist() if n.endswith(".dmp")]
            assert archive.read(dump).startswith(b"MDMP")
            manifest = archive.read("logs/collection.txt").decode("utf-8")
            for message in messages:
                assert message in manifest, (message, manifest)
            logs = {n: archive.read(n) for n in archive.namelist()
                    if n not in (dump, "logs/collection.txt")}
            assert logs == expected, (list(logs), list(expected), manifest)
        assert not list(game.glob("*.dmp")) and not list(game.glob("*.part"))

    folder.rmdir()
    check({}, messages=("documents: scan unavailable", "Included logs: 0"))
    folder.mkdir()
    check({})
    print("PASS missing folder / no logs: valid dump and collection summary", flush=True)
    add("unrelated.log", b"private", 1)
    add("SurvariumUnrelated.log", b"private", 1)
    add("Survarium_secrets.txt", b"private", 1)
    add("updater_skhid.resources.journal", b"not a diagnostic log", 1)
    add("Survarium_stale.log", b"stale", 49 * 3600)
    check({}, messages=("stale=1",))
    print("PASS only stale/unrelated files: no attachments", flush=True)
    small = {"Survarium_older.log": b"older", "Survarium_third.log": b"third",
             "Survarium_Жółw.log": b"unicode", "Survarium_Жółw.journal": b"journal",
             "survarium_launcher_Жółw.log": b"launcher"}
    for index, (name, data) in enumerate(small.items()):
        add(name, data, 30 + index)
    big = b"startup memory arena budget\r\n" * 3000 + b"middle of game log\r\n" * 150000 + b"crash context\r\n" * 5000
    current = add("Survarium_current.log", big, 10)
    expected = {"logs/documents/" + n: data for n, data in small.items()}
    def parts(name, data):
        return {"logs/documents/head-" + name: data[:64 * 1024],
                "logs/documents/tail-" + name: data[-(2 * 1024 * 1024 - 64 * 1024):]}
    expected.update(parts("Survarium_current.log", big))
    boundary = b"whole log boundary\r\n" * 120000
    boundary = boundary[:2 * 1024 * 1024]
    add("Survarium_boundary.log", boundary, 25)
    expected["logs/documents/Survarium_boundary.log"] = boundary
    for where, scope, name in ((game, "game-exe", "updater_skhid.log"),
                               (game, "game-exe", "survarium_skhid_d3d11.log"),
                               (game_root, "game-root", "survarium_skhid_dxgi.log"),
                               (root, "install-root", "launcher.log"),
                               (install_temp, "install-temp", "launcher_skhid.log")):
        add(name, b"diagnostic " + scope.encode(), 20, where)
        expected[f"logs/{scope}/{name}"] = b"diagnostic " + scope.encode()
    # CWD is the install root; arbitrary CWD files are still not collected.
    add("other.log", b"unrelated", 0, root)
    check(expected, "readable", current, ("omitted_middle_offset=65536, omitted_middle_bytes=" + str(len(big) - 2 * 1024 * 1024), "included_bytes=65536, start_offset=0", "included_bytes=2031616, start_offset=", "Included logs: 12" ))
    assert current.read_bytes() == big
    print("PASS 12 logs from Documents/EXE/game/install/temp, journal, Unicode, exact head/tail bytes, whole 2 MiB boundary and active writer", flush=True)
    no_current = {k: v for k, v in expected.items() if "Survarium_current.log" not in k}
    check(no_current, "locked", current, ("skipped logs/documents/Survarium_current.log: open error",))
    for failure in ("headfail", "readfail", "tailfail"):
        check(no_current, failure, current, ("all pieces rolled back",))
    print("PASS exclusive lock, head failure and early/mid-tail failures: valid dump/remaining logs, skipped-file reasons", flush=True)
    # Wine exposes Unix symlinks as ordinary files; they cannot test Windows reparse flags.
    (folder / "Survarium_directory.log").mkdir()
    add("Survarium.log", b"", 0)
    expected["logs/documents/Survarium.log"] = b""
    check(expected, messages=("directories/reparse=1",))
    print("PASS directory exclusion and empty log", flush=True)
    for where in (folder, game, game_root, root, install_temp):
        for path in where.iterdir():
            if path.is_file() and path.suffix in (".log", ".journal"):
                path.unlink()
    expected = {}
    for i in range(33):
        name = f"Survarium_count{i:02}.log"
        add(name, str(i).encode(), i)
        if i < 32:
            expected["logs/documents/" + name] = str(i).encode()
    check(expected, messages=("omitted by 32-file limit: 1", "Included logs: 32"))
    print("PASS newest 32 across a larger set and explicit omitted count", flush=True)
    for path in folder.glob("*.log"):
        if path.is_file():
            path.unlink()
    expected = {}
    for i in range(9):
        name = f"Survarium_budget{i}.log"
        add(name, big, i)
        if i < 8:
            expected.update(parts(name, big))
    check(expected, messages=("total-byte limit", "total log bytes: 16777216"))
    print("PASS 16 MiB total bound with skipped-file summary", flush=True)
    (folder / "Survarium_budget8.log").unlink()
    for i in range(24):
        name = f"Survarium_empty{i:02}.log"
        add(name, b"", 20 + i)
        expected["logs/documents/" + name] = b""
    check(expected, messages=("Included logs: 32; log members: 40", "total log bytes: 16777216"))
    print("PASS 32 sources / 40 log members with unchanged payload bound", flush=True)
    # Remaining budget cannot force a small source to become a partial log.
    # A skipped larger source also must not prevent later whole files fitting.
    for path in folder.glob("*.log"):
        if path.is_file():
            path.unlink()
    expected = {}
    for i in range(7):
        name = f"Survarium_budget{i}.log"
        add(name, big, i)
        expected.update(parts(name, big))
    for name, data, age in (("Survarium_first.log", b"a" * (1536 * 1024), 10),
                            ("Survarium_skip.log", b"b" * (1024 * 1024), 11),
                            ("Survarium_last.log", b"c" * (512 * 1024), 12)):
        add(name, data, age)
        if "skip" not in name:
            expected["logs/documents/" + name] = data
    check(expected, messages=("Survarium_skip.log: total-byte limit", "total log bytes: 16777216"))
    print("PASS whole-source budget admission, later small files still included", flush=True)
