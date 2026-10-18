"""Intentionally crash an installed 0.26g Skhid client; restore its DLL afterward.

Requires the three test binaries from build.cmd in --build. Run only against an
idle, local test installation, with its own Wine prefix. Evidence stays in --output.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import zipfile


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_dump(dump, expected_thread):
    # The DLL zips its dump: SRP-crash-...-N.zip holds SRP-crash-...-N.dmp.
    with zipfile.ZipFile(dump) as archive:
        assert archive.testzip() is None
        [entry] = [e for e in archive.infolist() if e.filename.endswith(".dmp")]
        assert "logs/collection.txt" in archive.namelist()
        assert entry.filename == dump.stem + ".dmp"
        data = archive.read(entry)
    assert data[:4] == b"MDMP"
    count, directory = struct.unpack_from("<II", data, 8)
    modules = []
    exception = None
    for i in range(count):
        kind, size, offset = struct.unpack_from("<III", data, directory + i * 12)
        assert offset + size <= len(data)
        if kind == 6:
            thread, _, code = struct.unpack_from("<III", data, offset)
            address = struct.unpack_from("<Q", data, offset + 24)[0]
            params = struct.unpack_from("<QQ", data, offset + 40)
            context_size, context = struct.unpack_from("<II", data, offset + 160)
            assert context_size >= 204 and context + context_size <= len(data)
            assert struct.unpack_from("<I", data, context + 184)[0] == address
            exception = thread, code, address, params
        if kind == 4:
            module_count = struct.unpack_from("<I", data, offset)[0]
            assert 4 + module_count * 108 <= size
            for j in range(module_count):
                record = offset + 4 + j * 108
                base, image_size = struct.unpack_from("<QI", data, record)
                name_rva = struct.unpack_from("<I", data, record + 20)[0]
                name_bytes = struct.unpack_from("<I", data, name_rva)[0]
                assert name_rva + 4 + name_bytes <= len(data)
                name = data[name_rva + 4:name_rva + 4 + name_bytes].decode("utf-16le")
                cv_size, cv_rva = struct.unpack_from("<II", data, record + 76)
                assert cv_rva + cv_size <= len(data)
                modules.append({"name": name, "base": base, "size": image_size,
                                "codeview": data[cv_rva:cv_rva + min(cv_size, 24)].hex()})
    assert exception is not None
    thread, code, address, params = exception
    assert thread == expected_thread and code == 0xC0000005 and params == (1, 0)
    fault = next((m for m in modules if m["base"] <= address < m["base"] + m["size"]), None)
    assert fault and fault["name"].lower().endswith("d3d11.dll")
    assert any(m["name"].lower().endswith("survarium_skhid.exe") for m in modules)
    # The DLL is linked without a PDB (reproducible build): no CodeView record.
    assert any(m["name"].lower().endswith("bugtrap.dll") for m in modules)
    return {"dump": str(dump), "dump_bytes": len(data), "exception_code": hex(code),
            "exception_address": hex(address), "faulting_thread": thread,
            "fault_module": fault["name"], "modules": modules}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("game", type=Path, help="directory containing survarium_skhid.exe")
    p.add_argument("--wine", default="wine")
    p.add_argument("--prefix", required=True, type=Path)
    p.add_argument("--build", type=Path, default=Path(__file__).resolve().parents[1] / "target/i686-pc-windows-msvc/release")
    p.add_argument("--output", required=True, type=Path)
    args = p.parse_args()
    game, build, output, prefix = (v.resolve() for v in (args.game, args.build, args.output, args.prefix))
    output.mkdir(parents=True, exist_ok=True)
    original, replacement = game / "bugtrap.dll", build / "BugTrap.dll"
    exe = game / "survarium_skhid.exe"
    original_bytes, original_hash, exe_hash = original.read_bytes(), digest(original), digest(exe)
    proxy_names = ("d3d11.dll", "srp_test_d3d11_original.dll")
    assert all(not (game / name).exists() for name in proxy_names), "Test DLL names are already occupied"
    for name in ("BugTrap.dll", "game_crash_test.exe", "d3d11.dll"):
        assert (build / name).is_file(), name
    d3d11 = prefix / "drive_c/windows/syswow64/d3d11.dll"
    assert d3d11.is_file()
    before = {p: digest(p) for p in game.glob("SRP-crash-*.zip")}
    dumps_before = set(game.glob("SRP-crash-*.dmp"))
    log = output / "game-test.log"
    thread_file = output / "crash-thread.txt"
    thread_file.unlink(missing_ok=True)
    env = os.environ.copy()
    env.update(WINEPREFIX=str(prefix), WINEDEBUG="-all",
               WINEDLLOVERRIDES="mscoree,mshtml,qwave=;winemenubuilder.exe=d;d3d11,dxgi=n,b",
               SRP_CRASH_TEST_THREAD_FILE="Z:" + str(thread_file))
    for name in ("WINEARCH", "WINELOADER", "WINESERVER"):
        env.pop(name, None)
    # The exclusive marker prevents two copies of this harness modifying the same game.
    marker = game / ".srp-crash-test.lock"
    lock = marker.open("x")
    lock.close()
    try:
        (output / "bugtrap.original.dll").write_bytes(original_bytes)
        shutil.copy2(build / "d3d11.dll", game / "d3d11.dll")
        shutil.copy2(d3d11, game / "srp_test_d3d11_original.dll")
        shutil.copy2(replacement, game / ".bugtrap.srp-test.dll")
        os.replace(game / ".bugtrap.srp-test.dll", original)
        with log.open("w") as f:
            result = subprocess.run([args.wine, str(build / "game_crash_test.exe"), "Z:" + str(game)],
                                    cwd=output, env=env, stdout=f, stderr=subprocess.STDOUT, timeout=240)
    except subprocess.TimeoutExpired:
        match = re.search(r"GAME_STARTED pid=(\d+)", log.read_text(errors="replace"))
        if match:
            subprocess.run([args.wine, "taskkill", "/pid", match[1], "/f"], env=env, timeout=20)
        raise
    finally:
        (game / ".bugtrap.restore.dll").write_bytes(original_bytes)
        os.replace(game / ".bugtrap.restore.dll", original)
        for name in proxy_names:
            (game / name).unlink(missing_ok=True)
        marker.unlink()
        assert digest(original) == original_hash
        assert digest(exe) == exe_hash
    content = log.read_text(errors="replace")
    print(content)
    assert result.returncode == 0, "Controller failed; see game-test.log"
    assert "DISCORD_BUTTON=1" in content and "CRASH_DIALOG_CLOSED" in content
    assert "CRASH_DIALOG " in content  # The status text follows Windows' UI language.
    dumps = set(game.glob("SRP-crash-*.zip")) - set(before)
    assert len(dumps) == 1, dumps
    assert set(game.glob("SRP-crash-*.dmp")) == dumps_before, "the zipped dump was kept"
    for old, hash_before in before.items():
        assert digest(old) == hash_before, "An earlier dump changed"
    archive = dumps.pop()
    summary = verify_dump(archive, int(thread_file.read_text().strip()))
    summary.update(zip=str(archive), zip_bytes=archive.stat().st_size)
    summary.update(exe_sha256=exe_hash, replacement_dll_sha256=digest(replacement),
                   original_dll_restored=True, temporary_test_dlls_removed=True,
                   discord_button_present=True, dump_survived_dialog_close=True)
    (output / "validation.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("PASS: installed 0.26g crash, exception/register context, module list with BugTrap, persistent dump, no submission, original files restored")


if __name__ == "__main__":
    main()
