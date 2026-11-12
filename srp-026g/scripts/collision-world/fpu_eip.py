"""The float control words of a running game's engine threads at the moments their EIP is inside the
walk tick's code or Bullet's cast (no stack guessing: the instruction pointer itself).

    python fpu_eip.py <pid> [--rounds 3000]

Each sample suspends one engine thread (start address threading's entry 0xE0910) for one GetThreadContext and
resumes it. Counts (x87 control word, MXCSR without its sticky flags) per code range.
"""
import argparse
import collections
import ctypes
import ctypes.wintypes as wt
import json
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bwmem  # noqa: E402
import fpu_sample as fs  # noqa: E402

RANGES = [
    ("character controller (player_step, its sweeps and callbacks)", 0x7A930, 0x89C00),
    ("btCollisionWorld (convexSweepTest, objectQuerySingle, rays)", 0x9FC00, 0xA4000),
    ("btContinuousConvexCollision, triangle cast callbacks", 0xB8400, 0xB9A00),
    ("simplex solver, GJK pair detector, EPA", 0xD1000, 0xD9000),
    ("bullet_physics_world::tick", 0x75550, 0x758B0),
]
ENGINE_THREAD_START = 0xE0910


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pid", type=int)
    ap.add_argument("--rounds", type=int, default=3000)
    ap.add_argument("--out")
    args = ap.parse_args()
    live = bwmem.LiveProcess(args.pid)
    delta = live.modules()[0][0] - 0x10000
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    ntdll = ctypes.WinDLL("ntdll")
    k32.OpenThread.restype = wt.HANDLE
    k32.OpenThread.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
    k32.Wow64GetThreadContext.argtypes = [wt.HANDLE, ctypes.POINTER(fs.WOW64_CONTEXT)]
    k32.Wow64SuspendThread.argtypes = [wt.HANDLE]
    k32.ResumeThread.argtypes = [wt.HANDLE]
    handles = {}
    for tid in fs.thread_ids(k32, args.pid):
        handle = k32.OpenThread(fs.THREAD_ACCESS, False, tid)
        if not handle:
            continue
        start = ctypes.c_ulonglong(0)
        ntdll.NtQueryInformationThread(wt.HANDLE(handle), 9, ctypes.byref(start), 8, None)
        if start.value - delta == ENGINE_THREAD_START:
            handles[tid] = handle
        else:
            k32.CloseHandle(wt.HANDLE(handle))
    counts = {name: collections.Counter() for name, _, _ in RANGES}
    everywhere = collections.Counter()
    by_thread = collections.defaultdict(collections.Counter)
    ctx = fs.WOW64_CONTEXT()
    for _ in range(args.rounds):
        for tid, handle in handles.items():
            if k32.Wow64SuspendThread(wt.HANDLE(handle)) == 0xFFFFFFFF:
                continue
            try:
                ctx.ContextFlags = fs.WOW64_CONTEXT_ALL
                ok = k32.Wow64GetThreadContext(wt.HANDLE(handle), ctypes.byref(ctx))
            finally:
                k32.ResumeThread(wt.HANDLE(handle))
            if not ok:
                continue
            cw = ctx.ControlWord & 0xFFFF
            mxcsr = struct.unpack_from("<I", bytes(ctx.ExtendedRegisters), 24)[0] & 0xFFC0  # sticky flags off
            key = f"cw {cw:#06x} mxcsr {mxcsr:#06x}"
            everywhere[key] += 1
            eip = ctx.Eip - delta
            for name, lo, hi in RANGES:
                if lo <= eip < hi:
                    counts[name][key] += 1
                    by_thread[tid][name] += 1
    for handle in handles.values():
        k32.CloseHandle(wt.HANDLE(handle))
    out = {
        "engine_threads": len(handles), "rounds": args.rounds,
        "all_samples": dict(everywhere.most_common()),
        "by_code_range": {name: dict(c.most_common()) for name, c in counts.items()},
        "threads_seen_in_ranges": {str(tid): dict(c) for tid, c in by_thread.items()},
    }
    text = json.dumps(out, indent=1)
    if args.out:
        with open(args.out, "w", newline="") as f:
            f.write(text)
    print(text)


if __name__ == "__main__":
    main()
