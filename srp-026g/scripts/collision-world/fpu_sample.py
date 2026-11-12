"""The x87 control word and MXCSR of a running 32-bit game's threads, sampled from outside (step 0.4).

Each sample suspends one thread for the time of one GetThreadContext, reads its float state and the top of
its stack, and resumes it. A sample is marked `in_controller` when a return address into the character
controller's code (0x7A950..0x89C00: updateAction 0x7B730, player_step 0x7BDA0, the sweeps) lies on the
stack's top 16 KiB, or EIP is there: the control word the walk tick runs under.

    python fpu_sample.py <pid> [--rounds 200] [--gap 0.02]
"""
import collections
import ctypes
import ctypes.wintypes as wt
import struct
import sys
import time

TH32CS_SNAPTHREAD = 0x4
THREAD_ACCESS = 0x0002 | 0x0008 | 0x0040 | 0x0800  # suspend/resume, get context, query, limited
WOW64_CONTEXT_ALL = 0x10000 | 0x1 | 0x2 | 0x8 | 0x20  # control, integer, floating point, extended

CONTROLLER_LO, CONTROLLER_HI = 0x7A950, 0x89C00
TICK_LO, TICK_HI = 0x75550, 0x758B0  # bullet_physics_world::tick


class THREADENTRY32(ctypes.Structure):
    _fields_ = [("dwSize", wt.DWORD), ("cntUsage", wt.DWORD), ("th32ThreadID", wt.DWORD),
                ("th32OwnerProcessID", wt.DWORD), ("tpBasePri", wt.LONG), ("tpDeltaPri", wt.LONG),
                ("dwFlags", wt.DWORD)]


class WOW64_CONTEXT(ctypes.Structure):
    _fields_ = [
        ("ContextFlags", wt.DWORD), ("Dr", wt.DWORD * 6),
        ("ControlWord", wt.DWORD), ("StatusWord", wt.DWORD), ("TagWord", wt.DWORD),
        ("ErrorOffset", wt.DWORD), ("ErrorSelector", wt.DWORD), ("DataOffset", wt.DWORD),
        ("DataSelector", wt.DWORD), ("RegisterArea", ctypes.c_ubyte * 80), ("Cr0NpxState", wt.DWORD),
        ("SegGs", wt.DWORD), ("SegFs", wt.DWORD), ("SegEs", wt.DWORD), ("SegDs", wt.DWORD),
        ("Edi", wt.DWORD), ("Esi", wt.DWORD), ("Ebx", wt.DWORD), ("Edx", wt.DWORD), ("Ecx", wt.DWORD),
        ("Eax", wt.DWORD), ("Ebp", wt.DWORD), ("Eip", wt.DWORD), ("SegCs", wt.DWORD), ("EFlags", wt.DWORD),
        ("Esp", wt.DWORD), ("SegSs", wt.DWORD), ("ExtendedRegisters", ctypes.c_ubyte * 512),
    ]


def thread_ids(k32, pid):
    k32.CreateToolhelp32Snapshot.restype = wt.HANDLE
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    entry = THREADENTRY32()
    entry.dwSize = ctypes.sizeof(entry)
    out = []
    ok = k32.Thread32First(wt.HANDLE(snap), ctypes.byref(entry))
    while ok:
        if entry.th32OwnerProcessID == pid:
            out.append(entry.th32ThreadID)
        ok = k32.Thread32Next(wt.HANDLE(snap), ctypes.byref(entry))
    k32.CloseHandle(wt.HANDLE(snap))
    return out


def sample(pid, mem, delta=0, rounds=200, gap=0.02):
    """mem: a bwmem.LiveProcess of the same pid (for the stacks). Returns a JSON-able summary."""
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    ntdll = ctypes.WinDLL("ntdll")
    k32.OpenThread.restype = wt.HANDLE
    k32.OpenThread.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
    k32.Wow64GetThreadContext.argtypes = [wt.HANDLE, ctypes.POINTER(WOW64_CONTEXT)]
    k32.Wow64SuspendThread.argtypes = [wt.HANDLE]
    k32.ResumeThread.argtypes = [wt.HANDLE]
    handles = {}
    starts = {}
    for tid in thread_ids(k32, pid):
        handle = k32.OpenThread(THREAD_ACCESS, False, tid)
        if handle:
            handles[tid] = handle
            start = ctypes.c_ulonglong(0)
            ntdll.NtQueryInformationThread(wt.HANDLE(handle), 9, ctypes.byref(start), 8, None)
            starts[tid] = start.value
    all_words = collections.Counter()
    in_controller = collections.Counter()
    in_tick = collections.Counter()
    per_thread = collections.defaultdict(collections.Counter)
    ctx = WOW64_CONTEXT()
    for _ in range(rounds):
        for tid, handle in handles.items():
            if k32.Wow64SuspendThread(wt.HANDLE(handle)) == 0xFFFFFFFF:
                continue
            try:
                ctx.ContextFlags = WOW64_CONTEXT_ALL
                if not k32.Wow64GetThreadContext(wt.HANDLE(handle), ctypes.byref(ctx)):
                    continue
                cw = ctx.ControlWord & 0xFFFF
                mxcsr = struct.unpack_from("<I", bytes(ctx.ExtendedRegisters), 24)[0]
                eip = ctx.Eip - delta
                stack = mem.read(ctx.Esp, 0x4000) or mem.read(ctx.Esp, 0x400) or b""
            finally:
                k32.ResumeThread(wt.HANDLE(handle))
            key = (f"{cw:#06x}", f"{mxcsr:#06x}")
            all_words[key] += 1
            per_thread[tid][key] += 1
            words = struct.unpack_from(f"<{len(stack) // 4}I", stack) if stack else ()
            rets = [w - delta for w in words]
            if CONTROLLER_LO <= eip < CONTROLLER_HI or any(CONTROLLER_LO <= r < CONTROLLER_HI for r in rets):
                in_controller[key + (tid,)] += 1
            if TICK_LO <= eip < TICK_HI or any(TICK_LO <= r < TICK_HI for r in rets):
                in_tick[key + (tid,)] += 1
        time.sleep(gap)
    for handle in handles.values():
        k32.CloseHandle(wt.HANDLE(handle))
    return {
        "rounds": rounds,
        "threads": len(handles),
        "all": {f"cw {k[0]} mxcsr {k[1]}": n for k, n in all_words.most_common()},
        "in_controller": {f"cw {k[0]} mxcsr {k[1]} thread {k[2]}": n for k, n in in_controller.most_common()},
        "in_physics_tick": {f"cw {k[0]} mxcsr {k[1]} thread {k[2]}": n for k, n in in_tick.most_common()},
        "per_thread": {f"{tid} start {starts.get(tid, 0):#x}": {f"cw {k[0]} mxcsr {k[1]}": n for k, n in c.items()}
                       for tid, c in per_thread.items()},
    }


if __name__ == "__main__":
    import json
    import os
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import bwmem
    pid = int(sys.argv[1])
    live = bwmem.LiveProcess(pid)
    delta = live.modules()[0][0] - 0x10000
    print(json.dumps(sample(pid, live, delta), indent=1))
