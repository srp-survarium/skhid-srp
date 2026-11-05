#!/usr/bin/env python3
"""Exercise Wine key events on an isolated X display, including layout changes."""
import argparse
import ctypes
import ctypes.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wine", required=True, type=Path)
    parser.add_argument("--windows-python", required=True, type=Path)
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--libx11", default=ctypes.util.find_library("X11"))
    parser.add_argument("--libxtst", default=ctypes.util.find_library("Xtst"))
    parser.add_argument("--expect-broken", action="store_true", help="check the stock 11.8 regression")
    args = parser.parse_args()
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    for name in ["Xvfb", "setxkbmap"]:
        if not shutil.which(name):
            parser.error(f"{name} is required")
    if not args.libx11 or not args.libxtst:
        parser.error("libX11 and libXtst paths are required")
    lib = ctypes.CDLL(args.libx11)
    lib.XOpenDisplay.restype = ctypes.c_void_p
    lib.XkbLockGroup.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_uint]
    lib.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    lib.XCloseDisplay.argtypes = [ctypes.c_void_p]
    test = ctypes.CDLL(args.libxtst)
    test.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    display = None
    with (work / "display").open("w+") as display_file:
        server = subprocess.Popen(["Xvfb", "-displayfd", str(display_file.fileno()),
                                   "-screen", "0", "800x600x24", "-nolisten", "tcp", "-noreset"],
                                  pass_fds=[display_file.fileno()], stdout=subprocess.DEVNULL,
                                  stderr=subprocess.DEVNULL)
        try:
            for _ in range(100):
                display_file.seek(0)
                number = display_file.read().strip()
                if number:
                    break
                if server.poll() is not None:
                    raise RuntimeError("Xvfb failed to start")
                time.sleep(.05)
            else:
                raise RuntimeError("Xvfb did not announce a display")
            env = dict(os.environ, DISPLAY=":" + number, WINEDEBUG="-all,+keyboard",
                       WINEDLLOVERRIDES="mscoree,mshtml=")
            for name in ["WAYLAND_DISPLAY", "XDG_SESSION_TYPE", "WINEDLLPATH", "WINELOADER", "WINESERVER"]:
                env.pop(name, None)
            subprocess.run(["setxkbmap", "-layout", "us,ru"], env=env, check=True)
            display = lib.XOpenDisplay(env["DISPLAY"].encode())
            assert display
            fixture = Path(__file__).with_name("keyboard-window.py").resolve()
            windows_fixture = "Z:" + str(fixture).replace("/", "\\")
            for group, name in enumerate(["english", "russian"]):
                lib.XkbLockGroup(display, 0x100, group)
                lib.XSync(display, 0)
                prefix = work / name
                prefix.mkdir(exist_ok=True)
                env["WINEPREFIX"] = str(prefix)
                path = work / f"{name}.log"
                with path.open("w") as log:
                    process = subprocess.Popen([args.wine, args.windows_python, windows_fixture],
                                               env=env, stdout=log, stderr=subprocess.STDOUT)
                    try:
                        for _ in range(600):
                            if '"ready": true' in path.read_text(errors="replace"):
                                break
                            if process.poll() is not None:
                                raise RuntimeError(f"probe exited early: {path}")
                            time.sleep(.1)
                        else:
                            raise RuntimeError(f"probe did not become ready: {path}")
                        for active in [group, 1 - group, group]:
                            lib.XkbLockGroup(display, 0x100, active)
                            lib.XSync(display, 0)
                            time.sleep(.15)
                            for key in [25, 38, 39, 40, 10, 36]:
                                test.XTestFakeKeyEvent(display, key, 1, 0)
                                test.XTestFakeKeyEvent(display, key, 0, 0)
                                lib.XSync(display, 0)
                                time.sleep(.05)
                        assert process.wait(timeout=15) == 0
                    finally:
                        if process.poll() is None:
                            process.kill()
                            process.wait()
                rows = [json.loads(line) for line in path.read_text(errors="replace").splitlines()
                        if line.startswith("{")]
                scans = [97, 108, 109, 110] if args.expect_broken and group else [17, 30, 31, 32]
                assert rows[0]["maps"] == dict(zip("WASD", scans)), (name, rows[0])
                for event in ["0x100", "0x101", "0x102"]:
                    actual = [row["scan"] for row in rows if row.get("event") == event]
                    assert actual == (scans + [2, 28]) * 3, (name, event, actual)
                text = ["wasd1\r", "цфыв1\r"]
                actual = "".join(row["char"] for row in rows if row.get("event") == "0x102")
                assert actual == text[group] + text[1 - group] + text[group], (name, actual)
                print(f"{name} startup: scan codes, key up/down, text and layout switching PASS", flush=True)
        finally:
            if display:
                lib.XCloseDisplay(display)
            server.terminate()
            server.wait()


if __name__ == "__main__":
    main()
