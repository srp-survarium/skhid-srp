"""Every step of every character controller of a running game, taken at the step itself.

Attaches to the game as a debugger and uses the CPU's four hardware breakpoints (the debug registers of each
thread's context): no byte of the game's code or data is changed. At each stop the thread's registers and the
controller's memory are read and the game goes on. Every stop is a round trip to this script, so the game may
run slower than real time while traced; the order and the values of its steps are its own.

    python step_trace.py <pid> --seconds 60 --out steps.bdt        (the pid `bw_run_replay.py --keep` prints)

Stops (0.26g, image base 0x10000; the stock exe's bytes in every patch revision so far):
  0x7F6D0   bullet_character_controller::set_transform, entry   eax the new transform (the feet), [esp+4] the
                                                                controller, [esp] the caller
  0x7B7A9   updateAction, just before player_step               esi the controller, [ebp+0Ch] the dt
  0x7B7B7   updateAction, just after player_step                esi the controller (the walk vector still there)
  0x2D1E20  game_world_core::atomic_tick, entry                 edi the match time the step ends at, [esp+8] its
                                                                length in ms

The file: step_file.py. What the game's steps are: docs/notes/body_trace.md.
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import step_file  # noqa: E402
from fpu_sample import WOW64_CONTEXT  # noqa: E402

STOPS = [(0x7F6D0, "set_transform"), (0x7B7A9, "pre_step"), (0x7B7B7, "post_step"), (0x2D1E20, "atomic_tick")]
A_LO, A_HI, B_LO, B_HI = step_file.A_LO, step_file.A_HI, step_file.B_LO, step_file.B_HI

EXCEPTION_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT, CREATE_PROCESS_DEBUG_EVENT = 1, 2, 3
EXIT_THREAD_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT, LOAD_DLL_DEBUG_EVENT = 4, 5, 6
DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED = 0x00010002, 0x80010001
SINGLE_STEP = (0x80000004, 0x4000001E)   # the second: a 32-bit thread's, as a 64-bit debugger sees it
BREAKPOINT = (0x80000003, 0x4000001F)
WOW64_CONTROL, WOW64_INTEGER, WOW64_DEBUG = 0x10001, 0x10002, 0x10010
TF = 0x100


class EXCEPTION_RECORD(ctypes.Structure):
    _fields_ = [("ExceptionCode", wt.DWORD), ("ExceptionFlags", wt.DWORD), ("ExceptionRecord", ctypes.c_void_p),
                ("ExceptionAddress", ctypes.c_void_p), ("NumberParameters", wt.DWORD),
                ("ExceptionInformation", ctypes.c_void_p * 15)]


class EXCEPTION_DEBUG_INFO(ctypes.Structure):
    _fields_ = [("ExceptionRecord", EXCEPTION_RECORD), ("dwFirstChance", wt.DWORD)]


class CREATE_THREAD_DEBUG_INFO(ctypes.Structure):
    _fields_ = [("hThread", wt.HANDLE), ("lpThreadLocalBase", ctypes.c_void_p), ("lpStartAddress", ctypes.c_void_p)]


class CREATE_PROCESS_DEBUG_INFO(ctypes.Structure):
    _fields_ = [("hFile", wt.HANDLE), ("hProcess", wt.HANDLE), ("hThread", wt.HANDLE),
                ("lpBaseOfImage", ctypes.c_void_p), ("dwDebugInfoFileOffset", wt.DWORD), ("nDebugInfoSize", wt.DWORD),
                ("lpThreadLocalBase", ctypes.c_void_p), ("lpStartAddress", ctypes.c_void_p),
                ("lpImageName", ctypes.c_void_p), ("fUnicode", wt.WORD)]


class LOAD_DLL_DEBUG_INFO(ctypes.Structure):
    _fields_ = [("hFile", wt.HANDLE), ("lpBaseOfDll", ctypes.c_void_p), ("dwDebugInfoFileOffset", wt.DWORD),
                ("nDebugInfoSize", wt.DWORD), ("lpImageName", ctypes.c_void_p), ("fUnicode", wt.WORD)]


class DEBUG_UNION(ctypes.Union):
    _fields_ = [("Exception", EXCEPTION_DEBUG_INFO), ("CreateThread", CREATE_THREAD_DEBUG_INFO),
                ("CreateProcessInfo", CREATE_PROCESS_DEBUG_INFO), ("LoadDll", LOAD_DLL_DEBUG_INFO),
                ("pad", ctypes.c_ubyte * 160)]


class DEBUG_EVENT(ctypes.Structure):
    _fields_ = [("dwDebugEventCode", wt.DWORD), ("dwProcessId", wt.DWORD), ("dwThreadId", wt.DWORD), ("u", DEBUG_UNION)]


class Tracer:
    def __init__(self, pid, writer, seconds):
        self.pid, self.seconds, self.writer = pid, seconds, writer
        self.k32 = k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        k32.OpenProcess.restype = wt.HANDLE
        k32.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
        k32.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_ulonglong, ctypes.c_void_p, ctypes.c_size_t,
                                          ctypes.POINTER(ctypes.c_size_t)]
        k32.WaitForDebugEvent.argtypes = [ctypes.POINTER(DEBUG_EVENT), wt.DWORD]
        k32.ContinueDebugEvent.argtypes = [wt.DWORD, wt.DWORD, wt.DWORD]
        k32.Wow64GetThreadContext.argtypes = [wt.HANDLE, ctypes.POINTER(WOW64_CONTEXT)]
        k32.Wow64SetThreadContext.argtypes = [wt.HANDLE, ctypes.POINTER(WOW64_CONTEXT)]
        k32.Wow64SuspendThread.argtypes = [wt.HANDLE]
        k32.ResumeThread.argtypes = [wt.HANDLE]
        k32.CloseHandle.argtypes = [wt.HANDLE]
        self.process = k32.OpenProcess(0x0400 | 0x0010, False, pid)  # query, read
        if not self.process:
            raise OSError(f"OpenProcess({pid}): {ctypes.get_last_error()}")
        self.threads = {}        # thread id -> handle
        self.stepping = set()    # threads running the one instruction of a stop (that stop's register is off)
        self.delta = 0
        self.addr_of = {}
        self.ctx = WOW64_CONTEXT()
        self.counts = {name: 0 for _, name in STOPS}
        self.parts = 0
        self.passed_on = {}

    def read(self, addr, n):
        buf = ctypes.create_string_buffer(n)
        got = ctypes.c_size_t(0)
        if not self.k32.ReadProcessMemory(self.process, addr, buf, n, ctypes.byref(got)) or got.value != n:
            return None
        return buf.raw

    def arm(self, handle, on=True, skip=None):
        """The four stops into a thread's debug registers (execute, one byte); `skip` leaves one off."""
        ctx = WOW64_CONTEXT()
        ctx.ContextFlags = WOW64_DEBUG
        if not self.k32.Wow64GetThreadContext(handle, ctypes.byref(ctx)):
            return False
        dr7 = 0x400
        for i, (addr, _) in enumerate(STOPS):
            ctx.Dr[i] = (addr + self.delta) if on else 0
            if on and i != skip:
                dr7 |= 1 << (2 * i)
        ctx.Dr[4] = 0
        ctx.Dr[5] = dr7
        ctx.ContextFlags = WOW64_DEBUG
        return bool(self.k32.Wow64SetThreadContext(handle, ctypes.byref(ctx)))

    def stop(self, tid, index):
        ctx = self.ctx
        name = STOPS[index][1]
        self.counts[name] += 1
        if name in ("pre_step", "post_step"):
            dt = self.read(ctx.Ebp + 0xC, 4) or bytes(4)
            a = self.read(ctx.Esi + A_LO, A_HI - A_LO)
            b = self.read(ctx.Esi + B_LO, B_HI - B_LO)
            if a is None or b is None:
                payload = dt
            else:
                if name == "pre_step" and dt != b[4:8]:
                    self.parts += 1
                payload = step_file.step_payload(dt, a, b, self.read(ctx.Ebp, 16) or b"")
        elif name == "set_transform":
            payload = (self.read(ctx.Esp, 32) or bytes(32)) + (self.read(ctx.Eax, 0x40) or b"")
        else:
            payload = self.read(ctx.Esp, 32) or b""
        regs = (ctx.Eax, ctx.Ecx, ctx.Edx, ctx.Ebx, ctx.Esp, ctx.Ebp, ctx.Esi, ctx.Edi, ctx.Eip)
        self.writer.record(index, time.perf_counter() - self.start, tid, regs, payload)

    def exception(self, ev):
        tid = ev.dwThreadId
        code = ev.u.Exception.ExceptionRecord.ExceptionCode
        handle = self.threads.get(tid)
        if code in BREAKPOINT:
            return DBG_CONTINUE  # the attach's own break
        if code not in SINGLE_STEP or handle is None:
            self.passed_on[code] = self.passed_on.get(code, 0) + 1
            return DBG_EXCEPTION_NOT_HANDLED
        ctx = self.ctx
        ctx.ContextFlags = WOW64_CONTROL | WOW64_INTEGER
        if not self.k32.Wow64GetThreadContext(handle, ctypes.byref(ctx)):
            return DBG_CONTINUE
        if tid in self.stepping:
            # the stop's own instruction has run: put the stop back
            self.stepping.discard(tid)
            self.arm(handle)
            ctx.EFlags &= ~TF
            ctx.ContextFlags = WOW64_CONTROL
            self.k32.Wow64SetThreadContext(handle, ctypes.byref(ctx))
        index = self.addr_of.get(ctx.Eip)
        if index is not None:
            # an execute breakpoint stops before its instruction: read, then run that one instruction with the
            # stop off (trap flag), and put it back at the single step
            self.stop(tid, index)
            self.arm(handle, skip=index)
            self.stepping.add(tid)
            ctx.EFlags |= TF
            ctx.ContextFlags = WOW64_CONTROL
            self.k32.Wow64SetThreadContext(handle, ctypes.byref(ctx))
        return DBG_CONTINUE

    def run(self):
        k32 = self.k32
        if not k32.DebugActiveProcess(self.pid):
            raise OSError(f"DebugActiveProcess({self.pid}): {ctypes.get_last_error()}")
        k32.DebugSetProcessKillOnExit(False)
        ev = DEBUG_EVENT()
        self.start = time.perf_counter()
        alive = True
        try:
            while alive and time.perf_counter() - self.start < self.seconds:
                if not k32.WaitForDebugEvent(ctypes.byref(ev), 200):
                    continue
                status = DBG_CONTINUE
                code = ev.dwDebugEventCode
                if code == CREATE_PROCESS_DEBUG_EVENT:
                    info = ev.u.CreateProcessInfo
                    self.delta = (info.lpBaseOfImage or 0x10000) - 0x10000
                    self.addr_of = {addr + self.delta: i for i, (addr, _) in enumerate(STOPS)}
                    self.threads[ev.dwThreadId] = info.hThread
                    if info.hFile:
                        k32.CloseHandle(info.hFile)
                    self.arm(info.hThread)
                elif code == CREATE_THREAD_DEBUG_EVENT:
                    self.threads[ev.dwThreadId] = ev.u.CreateThread.hThread
                    self.arm(ev.u.CreateThread.hThread)
                elif code == EXIT_THREAD_DEBUG_EVENT:
                    self.threads.pop(ev.dwThreadId, None)
                    self.stepping.discard(ev.dwThreadId)
                elif code == LOAD_DLL_DEBUG_EVENT:
                    if ev.u.LoadDll.hFile:
                        k32.CloseHandle(ev.u.LoadDll.hFile)
                elif code == EXIT_PROCESS_DEBUG_EVENT:
                    alive = False
                elif code == EXCEPTION_DEBUG_EVENT:
                    status = self.exception(ev)
                k32.ContinueDebugEvent(ev.dwProcessId, ev.dwThreadId, status)
        finally:
            if alive:
                alive = self.leave(ev)
            self.writer.close()
        return alive

    def leave(self, ev):
        """Leaves the game as it was: every thread held, its stops and single step taken off, what was already
        queued let go, then detached."""
        k32 = self.k32
        ctx = WOW64_CONTEXT()
        for handle in self.threads.values():
            k32.Wow64SuspendThread(handle)
        for handle in self.threads.values():
            self.arm(handle, on=False)
            ctx.ContextFlags = WOW64_CONTROL
            if k32.Wow64GetThreadContext(handle, ctypes.byref(ctx)) and ctx.EFlags & TF:
                ctx.EFlags &= ~TF
                k32.Wow64SetThreadContext(handle, ctypes.byref(ctx))
        for handle in self.threads.values():
            k32.ResumeThread(handle)
        while k32.WaitForDebugEvent(ctypes.byref(ev), 100):
            status = DBG_CONTINUE
            if ev.dwDebugEventCode == EXCEPTION_DEBUG_EVENT:
                code = ev.u.Exception.ExceptionRecord.ExceptionCode
                if code not in SINGLE_STEP and code not in BREAKPOINT:
                    status = DBG_EXCEPTION_NOT_HANDLED
            elif ev.dwDebugEventCode == CREATE_THREAD_DEBUG_EVENT:
                self.arm(ev.u.CreateThread.hThread, on=False)
            k32.ContinueDebugEvent(ev.dwProcessId, ev.dwThreadId, status)
            if ev.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT:
                return False
        k32.DebugActiveProcessStop(self.pid)
        return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pid", type=int)
    ap.add_argument("--seconds", type=float, default=60.0)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    with open(args.out, "wb") as out:
        writer = step_file.Writer(out, STOPS, pid=args.pid)
        tracer = Tracer(args.pid, writer, args.seconds)
        alive = tracer.run()
    print(f"stops: {tracer.counts}; steps with a dt that is not the controller's own: {tracer.parts}; "
          f"other exceptions passed on to the game: { {hex(k): v for k, v in tracer.passed_on.items()} }; "
          f"game {'left running' if alive else 'ended by itself'} -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
