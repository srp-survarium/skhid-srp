"""Starts the game on its own hidden desktop with a replay, waits for the map's Bullet world, captures it,
and ends that one process (its own child, by handle; never by image name).

    python bw_run_replay.py --replay <name or full path .sur> --out <capture.bwc> [--exe <survarium_skhid.exe>]
                            [--timeout 420] [--min-objects 300] [--settle 5] [--keep] [--visible]

Nothing is written into the game and no game file is changed. The game shares the user's
Documents\\Survarium (its log and user.cfg); user.cfg's bytes are compared before and after and restored
if the game changed them. Refuses to start while another survarium process runs.
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import datetime
import hashlib
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bw_walk  # noqa: E402
import bwmem  # noqa: E402
import fpu_sample  # noqa: E402

DEFAULT_EXE = os.environ.get("SKHID_GAME_EXE")  # <game>\binaries\x86\survarium_skhid.exe
DESKTOP = "srp-cw-dump"


class STARTUPINFOW(ctypes.Structure):
    _fields_ = [
        ("cb", wt.DWORD), ("lpReserved", wt.LPWSTR), ("lpDesktop", wt.LPWSTR), ("lpTitle", wt.LPWSTR),
        ("dwX", wt.DWORD), ("dwY", wt.DWORD), ("dwXSize", wt.DWORD), ("dwYSize", wt.DWORD),
        ("dwXCountChars", wt.DWORD), ("dwYCountChars", wt.DWORD), ("dwFillAttribute", wt.DWORD),
        ("dwFlags", wt.DWORD), ("wShowWindow", wt.WORD), ("cbReserved2", wt.WORD),
        ("lpReserved2", ctypes.c_void_p), ("hStdInput", wt.HANDLE), ("hStdOutput", wt.HANDLE),
        ("hStdError", wt.HANDLE),
    ]


class PROCESS_INFORMATION(ctypes.Structure):
    _fields_ = [("hProcess", wt.HANDLE), ("hThread", wt.HANDLE), ("dwProcessId", wt.DWORD),
                ("dwThreadId", wt.DWORD)]


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as src:
        for block in iter(lambda: src.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def game_running():
    out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout
    return [line.split(",")[0].strip('"') for line in out.splitlines()
            if line.lower().startswith('"survarium')]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--replay", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--exe", default=DEFAULT_EXE)
    ap.add_argument("--timeout", type=int, default=420)
    ap.add_argument("--min-objects", type=int, default=300)
    ap.add_argument("--settle", type=int, default=5, help="polls with a settled object count")
    ap.add_argument("--drift", type=int, default=8, help="how many objects the count may move in those polls")
    ap.add_argument("--keep", action="store_true", help="leave the game running")
    ap.add_argument("--visible", action="store_true", help="the user's own desktop, not a hidden one")
    ap.add_argument("--note", default="")
    ap.add_argument("--cpus", type=int, default=2, help="logical CPUs the game may use (the last ones); 0: all")
    ap.add_argument("--fpu", type=int, default=0,
                    help="after the capture, wait this many seconds, then sample the threads' float control words")
    ap.add_argument("--fpu-rounds", type=int, default=300)
    args = ap.parse_args()
    if not args.exe:
        ap.error("--exe (or SKHID_GAME_EXE): the game's survarium_skhid.exe")

    running = game_running()
    if running:
        print(f"a game is running ({', '.join(running)}): not starting another; stop")
        return 3

    user_cfg = os.path.join(os.path.expanduser("~"), "Documents", "Survarium", "user.cfg")
    cfg_before = open(user_cfg, "rb").read() if os.path.exists(user_cfg) else None

    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    u32 = ctypes.WinDLL("user32", use_last_error=True)
    hdesk = None
    si = STARTUPINFOW()
    si.cb = ctypes.sizeof(si)
    if not args.visible:
        u32.CreateDesktopW.restype = wt.HANDLE
        u32.CreateDesktopW.argtypes = [wt.LPCWSTR, wt.LPCWSTR, ctypes.c_void_p, wt.DWORD, wt.DWORD,
                                       ctypes.c_void_p]
        hdesk = u32.CreateDesktopW(DESKTOP, None, None, 0, 0x10000000, None)  # GENERIC_ALL
        if not hdesk:
            print(f"CreateDesktop failed: {ctypes.get_last_error()}")
            return 4
        si.lpDesktop = "WinSta0\\" + DESKTOP

    exe = os.path.abspath(args.exe)
    cwd = os.path.dirname(exe)
    command = (f'"{exe}" -localization=english -splash_screen=0 -minimum_resources_memory_size=768 '
               f'-mt_allocator=768 -match_replay={args.replay}')
    os.environ["__COMPAT_LAYER"] = "HighDpiAware"
    pi = PROCESS_INFORMATION()
    k32.CreateProcessW.argtypes = [wt.LPCWSTR, wt.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, wt.BOOL,
                                   wt.DWORD, ctypes.c_void_p, wt.LPCWSTR, ctypes.POINTER(STARTUPINFOW),
                                   ctypes.POINTER(PROCESS_INFORMATION)]
    # Light on the machine (benches run beside it): idle priority, and only the last `--cpus` logical CPUs.
    ok = k32.CreateProcessW(exe, ctypes.create_unicode_buffer(command), None, None, False, 0x40, None, cwd,
                            ctypes.byref(si), ctypes.byref(pi))
    if not ok:
        print(f"CreateProcess failed: {ctypes.get_last_error()}")
        return 5
    pid = pi.dwProcessId
    count = os.cpu_count() or 1
    if 0 < args.cpus < count <= 64:
        mask = ((1 << args.cpus) - 1) << (count - args.cpus)
        k32.SetProcessAffinityMask.argtypes = [wt.HANDLE, ctypes.c_size_t]
        if not k32.SetProcessAffinityMask(wt.HANDLE(pi.hProcess), mask):
            print(f"SetProcessAffinityMask failed: {ctypes.get_last_error()} (the game runs on every CPU)")
    print(f"started pid {pid}: {command}", flush=True)

    rc = 1
    try:
        live = bwmem.LiveProcess(pid)
        started = time.time()
        world = None
        history = []
        delta = None
        while time.time() - started < args.timeout:
            time.sleep(3)
            code = wt.DWORD(0)
            k32.GetExitCodeProcess(wt.HANDLE(pi.hProcess), ctypes.byref(code))
            if code.value != 259:
                print(f"the game ended by itself, exit code {code.value:#x}")
                return 6
            try:
                if delta is None:
                    mods = live.modules()
                    delta = mods[0][0] - 0x10000
                    print(f"module base {mods[0][0]:#x} ({mods[0][1]})", flush=True)
                found = bw_walk.find_worlds(live, delta)
            except OSError as error:
                print(f"  not readable yet: {error}", flush=True)
                continue
            if not found:
                history = []
                continue
            world = max(found, key=lambda w: w[2])
            history.append(world[2])
            print(f"  {time.time() - started:5.0f} s  worlds {[(hex(a), n) for a, _, n in found]}", flush=True)
            tail = history[-args.settle:]
            # a running match adds and removes a few objects (grenades, batteries): settled is "no longer loading"
            if len(tail) == args.settle and max(tail) - min(tail) <= args.drift and min(tail) >= args.min_objects:
                break
        else:
            print("timeout: the world never settled")
            return 7

        rec = bwmem.Recorder(live)
        walker = bw_walk.Walker(rec, delta)
        t0 = time.time()
        data = walker.world(world[0])
        # a second pass tells whether the world changed while it was read
        again = bw_walk.Walker(live, delta, chase=False).world(world[0])
        same = [o["addr"] for o in data["objects"]] == [o["addr"] for o in again["objects"]]
        meta = {
            "pid": pid,
            "exe": exe,
            "exe_sha256": sha256(exe),
            "module_base": delta + 0x10000,
            "world": world[0],
            "replay": args.replay,
            "objects": len(data["objects"]),
            "stable_between_passes": same,
            "taken_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "seconds_after_start": round(time.time() - started),
            "problems": walker.problems[:200],
            "note": args.note,
        }
        ranges, size = rec.save(args.out, meta)
        print(f"captured {len(data['objects'])} objects, {len(walker.shapes)} shapes, {len(walker.meshes)} meshes, "
              f"{ranges} ranges, {size / 1e6:.1f} MB raw in {time.time() - t0:.1f} s -> {args.out}")
        print(f"problems: {len(walker.problems)}; stable between passes: {same}")
        if args.fpu:
            import json
            time.sleep(args.fpu)
            words = fpu_sample.sample(pid, live, delta, rounds=args.fpu_rounds)
            words["seconds_after_start"] = round(time.time() - started)
            words["objects_now"] = max((n for _, _, n in bw_walk.find_worlds(live, delta)), default=0)
            with open(args.out + ".fpu.json", "w", newline="") as out:
                json.dump(words, out, indent=1)
            print(f"float control words: all {words['all']}; in the controller {words['in_controller']}; "
                  f"in the physics tick {words['in_physics_tick']}")
        rc = 0
    finally:
        if not args.keep:
            k32.TerminateProcess(wt.HANDLE(pi.hProcess), 0)
            k32.WaitForSingleObject(wt.HANDLE(pi.hProcess), 10000)
            print(f"ended pid {pid}")
        k32.CloseHandle(wt.HANDLE(pi.hThread))
        k32.CloseHandle(wt.HANDLE(pi.hProcess))
        if hdesk:
            u32.CloseDesktop(wt.HANDLE(hdesk))
        if cfg_before is not None:
            now = open(user_cfg, "rb").read() if os.path.exists(user_cfg) else None
            if now != cfg_before:
                with open(user_cfg, "wb") as out:
                    out.write(cfg_before)
                print("user.cfg was changed by the game: restored to its bytes from before the run")
    return rc


if __name__ == "__main__":
    sys.exit(main())
