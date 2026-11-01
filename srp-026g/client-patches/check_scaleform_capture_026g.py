#!/usr/bin/env python3
"""Run patch 93 (the Rust patch `scaleform_capture`: the render thread's walk
of a DrawableImageContext's command queues never reaches a queue being
destroyed) in the default build on made-up contexts, the original beside.

    python check_scaleform_capture_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are; the
game's own code runs from DrawableImageContext::OnNextCapture's start
(0x572660, ECX the context's capture notify, context+8; the notify at
ESP+4) to its `ret 4`, and from DICommandQueue's destructor's start
(0x5ED1E0) past its own RemoveCaptureNotify (0x5ED1F9), with the game's
RefCountImpl::Release (0x505CB0) and RemoveCaptureNotify (0x5726F0).
Stubbed: kernel32's Enter/LeaveCriticalSection (each section's depth kept)
and InterlockedExchangeAdd (done here), the context's ExecuteNextCapture
(its vtable+4: recorded), DICommandQueue::OnNextCapture (0x5EE080: the
visit recorded, with what the main thread does meanwhile), and a queue's
scalar deleting destructor (its vtable+0, Release's at 0: what the patched
destructor does, the queue out of the list, then its memory poisoned and
freed).

The walk, each case: ExecuteNextCapture once, first, with the context's lock
not held; the queues visited in the list's order with the capture's notify,
never with the lock held, never one whose count was 0 or whose memory was
freed; every list link and count the patched walk reads (context+4Ch, a
queue's +0Ch and +4) read with the lock held; the lock entered and left as
often, every reference the walk took let go (a queue's count after is its
count before, less what the main thread let go), a last reference's
destruction with no lock held; EBX, ESI, EDI, EBP as they were, the notify
popped, back at the caller. The cases: three live queues; none; a queue
whose count is 0 (its destructor waiting for the lock); the next queue's
last reference let go by the main thread while the walk is at the queue
before it (the rev-40 crash: the original goes on to the freed queue); the
walk's own reference to a queue the last.

The destructor: at the stock body's write of the count 2 (0x5ED1ED) the
queue is out of its context's list already (the original: still in it), the
lock left; at 0x5ED1F9 both builds have the list without it, its links 0,
its count 2, EBX/ESI/EDI/EBP/ESP/EAX the same.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_MEM_READ, UcError
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import os

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

ON_NEXT_CAPTURE = 0x572660                  # DrawableImageContext::OnNextCapture (ECX context+8; ret 4)
QUEUE_NEXT_CAPTURE = 0x5EE080               # DICommandQueue::OnNextCapture (ECX the queue; ret 4)
DESTRUCTOR, AT_COUNT_2, AFTER_REMOVE = 0x5ED1E0, 0x5ED1ED, 0x5ED1F9
ENTER, LEAVE, XADD = 0x8700D0, 0x8700CC, 0x870094   # kernel32's IAT slots

DATA, STACK, STUBS, SENTINEL = 0x30000000, 0x31000000, 0x32000000, 0x33000000
CONTEXT = DATA                              # +0 vtable, +24h lock, +40h the list's end
CONTEXT_VTABLE, QUEUE_VTABLE = DATA + 0x800, DATA + 0x900
QUEUES = [DATA + 0x1000 * (i + 1) for i in range(4)]
QUEUE_SIZE = 0x100
NOTIFY = 0x0B0B1F1C
STUB_ENTER, STUB_LEAVE, STUB_XADD, STUB_EXECUTE, STUB_DESTROY = (STUBS + 0x10 * i for i in range(5))
GAME = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}
POISON = 0xDD


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (DATA, STACK, STUBS, SENTINEL):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        # each stub a `ret n`, its effect the hook's
        for stub, pops in ((STUB_ENTER, 4), (STUB_LEAVE, 4), (STUB_XADD, 8), (STUB_EXECUTE, 4), (STUB_DESTROY, 4)):
            emu.mem_write(stub, b"\xC2" + struct.pack("<H", pops))
        for slot, stub in ((ENTER, STUB_ENTER), (LEAVE, STUB_LEAVE), (XADD, STUB_XADD)):
            self.word(slot, stub)
        for at in (STUB_ENTER, STUB_LEAVE, STUB_XADD, STUB_EXECUTE, STUB_DESTROY, QUEUE_NEXT_CAPTURE):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_MEM_READ, self.read_hook, begin=DATA, end=DATA + 0x10000)
        self.watch = {}                     # address: what, the reads that must hold the lock
        self.checked_reads = True

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    # ---- the made-up context and queues ----

    def setup(self, counts):
        """A context whose list holds a queue for each count, in order."""
        emu = self.emu
        emu.mem_write(DATA, bytes(0x10000))
        self.word(CONTEXT, CONTEXT_VTABLE)
        self.word(CONTEXT_VTABLE + 4, STUB_EXECUTE)
        self.word(QUEUE_VTABLE, STUB_DESTROY)
        self.queues = QUEUES[:len(counts)]
        self.link([*self.queues])
        for queue, count in zip(self.queues, counts):
            self.word(queue, QUEUE_VTABLE)
            self.word(queue + 4, count)
            self.word(queue + 0x34, CONTEXT)
        self.depth, self.log, self.freed, self.bad_reads = 0, [], set(), []
        self.during = {}                    # queue: what the main thread does while the walk is at it
        self.entered = self.left = 0
        self.watch = {CONTEXT + 0x4C: "the first link"}
        for queue in self.queues:
            self.watch[queue + 0xC] = f"queue {self.name(queue)}'s next"
            self.watch[queue + 4] = f"queue {self.name(queue)}'s count"

    def link(self, queues):
        end = CONTEXT + 0x40
        ring = [end, *queues]
        for i, node in enumerate(ring):
            self.word(node + 8, ring[i - 1])
            self.word(node + 0xC, ring[(i + 1) % len(ring)])

    def listed(self):
        """The queues the list holds, from the end's next."""
        out, at, end = [], self.read(CONTEXT + 0x4C), CONTEXT + 0x40
        while at != end and len(out) < 8:
            out.append(at)
            at = self.read(at + 0xC)
        return out

    def name(self, queue):
        return QUEUES.index(queue) if queue in QUEUES else hex(queue)

    def unlink(self, queue):
        """RemoveCaptureNotify's writes."""
        prev, nxt = self.read(queue + 8), self.read(queue + 0xC)
        if nxt:
            self.word(prev + 0xC, nxt)
            self.word(nxt + 8, prev)
        self.word(queue + 8, 0)
        self.word(queue + 0xC, 0)

    def destroy(self, queue):
        """The patched destructor and the free: out of the list, then its
        memory poisoned (what a freed queue reads as)."""
        self.unlink(queue)
        self.freed.add(queue)
        self.emu.mem_write(queue, bytes([POISON]) * QUEUE_SIZE)

    # ---- the stubs ----

    def ret(self, emu, pops):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = self.read(sp + 4)
        ecx = emu.reg_read(UC_X86_REG_ECX)
        if address == STUB_ENTER:
            assert arg == CONTEXT + 0x24, hex(arg)
            self.depth += 1
            self.entered += 1
        elif address == STUB_LEAVE:
            assert arg == CONTEXT + 0x24, hex(arg)
            self.depth -= 1
            self.left += 1
            assert self.depth >= 0, "a lock left that was not entered"
        elif address == STUB_XADD:
            old = self.read(arg)
            self.word(arg, old + self.read(sp + 8))
            emu.reg_write(UC_X86_REG_EAX, old)
        elif address == STUB_EXECUTE:
            self.log.append(("execute", ecx, arg, self.depth))
        elif address == STUB_DESTROY:
            self.log.append(("destroy", self.name(ecx), self.depth))
            self.destroy(ecx)
        elif address == QUEUE_NEXT_CAPTURE:
            freed = ecx in self.freed
            self.log.append(("visit", self.name(ecx), arg, self.depth,
                             None if freed else self.read(ecx + 4), freed))
            self.ret(emu, 4)
            if not freed and ecx in self.during:
                self.checked_reads = False
                self.during.pop(ecx)()
                self.checked_reads = True
            return
        self.ret(emu, {STUB_XADD: 8}.get(address, 4))

    def read_hook(self, emu, access, address, size, value, _):
        if not self.checked_reads:
            return
        for at, what in self.watch.items():
            if at <= address < at + 4 and self.depth == 0:
                self.bad_reads.append(what)
        for queue in self.freed:
            if queue <= address < queue + QUEUE_SIZE:
                self.bad_reads.append(f"freed queue {self.name(queue)} +{address - queue:#x}")

    # ---- the runs ----

    def walk(self):
        """OnNextCapture from its start to its return: what it did."""
        emu = self.emu
        sp = STACK + 0x8000
        self.word(sp, SENTINEL)
        self.word(sp + 4, NOTIFY)
        for reg, value in GAME.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ECX, CONTEXT + 8)
        emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            emu.emu_start(ON_NEXT_CAPTURE, SENTINEL, count=200000)
        except UcError as error:
            return {"fault": f"{error} at {emu.reg_read(UC_X86_REG_EIP):#x}", "log": self.log}
        return {"fault": None, "log": self.log, "eip": emu.reg_read(UC_X86_REG_EIP),
                "esp": emu.reg_read(UC_X86_REG_ESP) == sp + 8,
                "regs": all(emu.reg_read(r) == v for r, v in GAME.items()),
                "balanced": self.entered == self.left and self.depth == 0, "entered": self.entered,
                "bad_reads": self.bad_reads, "counts": [None if q in self.freed else self.read(q + 4)
                                                        for q in self.queues],
                "listed": [self.name(q) for q in self.listed()]}

    def destructor(self, victim):
        """The destructor of queue `victim` (count 0, in the list) from its
        start to just after its own RemoveCaptureNotify."""
        emu = self.emu
        self.watch = {}
        seen = {}

        def at_count_2(emu, *_):
            seen["listed"] = [self.name(q) for q in self.listed()]
            seen["depth"] = self.depth
        hook = emu.hook_add(UC_HOOK_CODE, at_count_2, begin=AT_COUNT_2, end=AT_COUNT_2)
        sp = STACK + 0x8000
        self.word(sp, SENTINEL)
        for reg, value in GAME.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ECX, victim)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(DESTRUCTOR, AFTER_REMOVE, count=200000)
        emu.hook_del(hook)
        regs = {r: emu.reg_read(r) for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP,
                                             UC_X86_REG_ESP, UC_X86_REG_EAX)}
        return {"at_2": seen, "listed": [self.name(q) for q in self.listed()],
                "links": (self.read(victim + 8), self.read(victim + 0xC)), "count": self.read(victim + 4),
                "regs": regs, "balanced": self.entered == self.left and self.depth == 0,
                "entered": self.entered}


def visits(result):
    return [(e[1], e[4]) for e in result["log"] if e[0] == "visit"]


def walk_ok(result, expect_visits, expect_counts, destroyed=()):
    log = result["log"]
    problems = []
    if result["fault"]:
        return [result["fault"]]
    if result["eip"] != SENTINEL or not result["esp"] or not result["regs"]:
        problems.append("not back at the caller with its registers and the notify popped")
    if not log or log[0] != ("execute", CONTEXT, NOTIFY, 0) or [e[0] for e in log].count("execute") != 1:
        problems.append(f"ExecuteNextCapture not once first, outside the lock: {log[:1]}")
    v = [e for e in log if e[0] == "visit"]
    if [e[1] for e in v] != expect_visits:
        problems.append(f"visits {[e[1] for e in v]}, expected {expect_visits}")
    if any(e[2] != NOTIFY or e[3] != 0 or e[5] for e in v):
        problems.append("a visit with another notify, the lock held, or of a freed queue")
    if any(e[0] == "destroy" and e[2] != 0 for e in log):
        problems.append("a queue destroyed with the lock held")
    if sorted(e[1] for e in log if e[0] == "destroy") != sorted(destroyed):
        problems.append(f"destroyed {[e[1] for e in log if e[0] == 'destroy']}, expected {list(destroyed)}")
    if not result["balanced"] or not result["entered"]:
        problems.append("the lock not entered, or not left as often")
    if result["bad_reads"]:
        problems.append(f"read without the lock or freed: {sorted(set(result['bad_reads']))}")
    if result["counts"] != expect_counts:
        problems.append(f"counts {result['counts']}, expected {expect_counts}")
    return problems


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    def case(run, counts, during=None):
        run.setup(counts)
        for queue, action in (during or {}).items():
            run.during[QUEUES[queue]] = lambda r=run, a=action: a(r)
        return run.walk()

    # 1. three live queues
    a, b = case(stock, [1, 2, 1]), case(patched, [1, 2, 1])
    problems = walk_ok(b, [0, 1, 2], [1, 2, 1])
    report(not problems and [x[0] for x in visits(a)] == [0, 1, 2],
           f"three live queues: visited {[x[0] for x in visits(b)]} (original {[x[0] for x in visits(a)]}), "
           f"counts kept, the lock entered {b['entered']} times, never across a call {problems or ''}")

    # 2. none
    b = case(patched, [])
    problems = walk_ok(b, [], [])
    report(not problems, f"no queue: ExecuteNextCapture alone {problems or ''}")

    # 3. a queue whose count is 0 (being destroyed, its destructor waiting for the lock)
    a, b = case(stock, [1, 0, 3]), case(patched, [1, 0, 3])
    problems = walk_ok(b, [0, 2], [1, 0, 3])
    report(not problems and [x[0] for x in visits(a)] == [0, 1, 2],
           f"a queue at count 0 passed over: visited {[x[0] for x in visits(b)]} "
           f"(original {[x[0] for x in visits(a)]}: the dying queue too) {problems or ''}")

    # 4. the main thread lets the next queue's last reference go while the walk is at the queue before
    def drop_next(run):
        run.word(QUEUES[1] + 4, 0)
        run.log.append(("destroy", 1, run.depth))
        run.destroy(QUEUES[1])
    a, b = case(stock, [1, 1, 1], {0: drop_next}), case(patched, [1, 1, 1], {0: drop_next})
    problems = walk_ok(b, [0, 2], [1, None, 1], destroyed=[1])
    stock_freed = any(e[0] == "visit" and e[5] for e in a["log"]) or a["fault"] or a["bad_reads"]
    report(not problems and bool(stock_freed),
           f"the next queue destroyed meanwhile: visited {[x[0] for x in visits(b)]}, no freed memory read "
           f"(original: {'the freed queue reached' if stock_freed else 'not reached?'}) {problems or ''}")

    # 5. the walk's own reference is the last one
    def let_go(run):
        run.word(QUEUES[1] + 4, run.read(QUEUES[1] + 4) - 1)
    b = case(patched, [1, 1, 2], {1: let_go})
    problems = walk_ok(b, [0, 1, 2], [1, None, 2], destroyed=[1])
    report(not problems, f"the walk's reference the last: queue 1 destroyed once, after its visit, no lock held, "
           f"the walk on to queue 2 {problems or ''}")

    # 6. the destructor
    stock.setup([1, 0, 1])
    patched.setup([1, 0, 1])
    a, b = stock.destructor(QUEUES[1]), patched.destructor(QUEUES[1])
    ok = (b["at_2"] == {"listed": [0, 2], "depth": 0} and a["at_2"]["listed"] == [0, 1, 2]
          and a["listed"] == b["listed"] == [0, 2] and a["links"] == b["links"] == (0, 0)
          and a["count"] == b["count"] == 2 and a["regs"] == b["regs"] and a["balanced"] and b["balanced"]
          and (a["entered"], b["entered"]) == (1, 2))
    report(ok, f"the destructor: out of the list at the count's 2 (original: listed {a['at_2'].get('listed')}), "
           f"then as the original, the second removal a no-op ({b})" if not ok else
           "the destructor: the queue out of the list before the count's 2 (original: still listed), then "
           "the list, links, count and registers as the original's, its own removal a no-op")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
