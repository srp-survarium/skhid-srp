#!/usr/bin/env python3
"""The equivalence gate of a port (docs/notes/rust_patches.md, "Proving a
port"): the exe built with the OLD patch (asm or C, at a git revision) and
the exe built with the NEW one (the working tree, Rust), each hook run from
the same input states in both, everything observable compared.

    python equiv_026g.py <patch> <original survarium.exe> [--old REV]
        [--cases N] [--seed S] [--states DIR] [--min-coverage PCT]

<patch> names equiv/<patch>.py, the port's spec: its hooks (where the game
enters them), the game functions they call (each one's inputs, and a stub
or the game's own code), its input domain (the patch's check cases, edge
values, seeded random states) and where its code is in a build. --old is
the revision whose client-patches build the old exe (HEAD~1 by default: the
commit before the port; HEAD to test an uncommitted port). --states adds
captured states (JSON files: hook, regs, stack, memory), for example from
private/local-clients/runtime.

Per case, both exes start at the hook's site in the game's own code, from
the same registers, stack and memory; memory the case does not give reads
as bytes made from the case's seed and the address (lazily mapped pages),
the same in both. Compared:
- how the run left the patch: the exit's address and, there, the registers
  the spec names live and ESP;
- every call into the game, in order: its address and inputs (stubbed by
  the spec's model, or run as the game's own code);
- every byte written outside the run's private stack (below the entry's
  ESP), its final value; a byte of an arena block by block and offset, so
  old and new layouts compare.
Any difference fails, printed as a diff. Then the coverage of the OLD code:
its basic blocks (recursive descent from the hooks' entries, ndisasm)
against the blocks the old runs entered (Unicorn's block hook); below
--min-coverage (100 by default) fails, the blocks missed listed.

Requires Python `unicorn` (2.1.4), the dev shell (the builds link the
arena), ndisasm and git. Exit 0: zero diffs and the coverage met.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent

# A case's own stack and the stop address, away from the exe and the heap.
STACK_TOP, STACK_SIZE = 0x7F000000, 0x40000
STOP = 0x7EF00000
PAGE = 0x1000
REG_NAMES = ("eax", "ecx", "edx", "ebx", "ebp", "esi", "edi")


# ---- the spec's vocabulary -------------------------------------------------

class External:
    """A game function a hook calls: `inputs` what it takes ("eax".. a
    register, "arg0".. the dword at [esp+4+4i] at its entry), `pops` the
    bytes it pops; a stub sets EAX from the run's generator, then
    `model(machine, args, rng)` may set the return registers and write
    memory; `real` runs the game's own code instead."""

    def __init__(self, name, inputs, pops=0, model=None, real=False):
        self.name, self.inputs, self.pops, self.model, self.real = name, tuple(inputs), pops, model, real


class Hook:
    """A way the game enters the patch: a run starts at `site` (the game's
    instruction that calls or jumps to the patch) and ends at the first
    leaving of the patch's code that is not a call (a jump or a return into
    the game); `live` the registers compared there (ESP always);
    `externals` this hook's {VA: External} (else the spec's EXTERNALS)."""

    def __init__(self, name, site, live=("ebx", "esi", "edi", "ebp"), externals=None):
        self.name, self.site, self.live, self.externals = name, site, tuple(live), externals


class Case:
    """An input state: registers, the stack's dwords from ESP up, memory
    {address: bytes}."""

    def __init__(self, what, regs=None, stack=(), memory=None):
        self.what, self.regs, self.stack, self.memory = what, dict(regs or {}), list(stack), dict(memory or {})

    @staticmethod
    def from_json(data):
        number = lambda v: int(v, 0) if isinstance(v, str) else v
        return Case(data.get("what", "captured"), {k: number(v) for k, v in data.get("regs", {}).items()},
                    [number(v) for v in data.get("stack", [])],
                    {int(a, 0): bytes.fromhex(b) for a, b in data.get("memory", {}).items()})


# ---- the two builds ----------------------------------------------------------

def load_spec(path):
    sys.modules.setdefault("equiv_026g", sys.modules[__name__])
    spec = importlib.util.spec_from_file_location(f"equiv_spec_{Path(path).stem}", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def build_into(out_dir, spec_path):
    """In a client-patches folder (the cwd): the default build of the
    original $EQUIV_ORIGINAL names, and what the spec reads of it (its
    patch's code ranges and entries, the arena's blocks), into out_dir."""
    sys.path.insert(0, os.getcwd())
    import patch_026g as base
    original = open(os.environ["EQUIV_ORIGINAL"], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    spec = load_spec(spec_path)
    linked = base.arena()
    meta = {"ranges": [list(r) for r in spec.ranges(base, linked)],
            "entries": list(spec.entries(base, linked)),
            "blocks": {b.name: [b.va, b.size] for b in linked.blocks.values()}}
    Path(out_dir, "exe").write_bytes(bytes(built))
    Path(out_dir, "meta.json").write_text(json.dumps(meta))


def old_tree(rev, into):
    """client-patches (and the crates and re-kit its builds read) at `rev`."""
    archive = subprocess.run(["git", "-C", str(REPO), "archive", rev, "srp-026g/client-patches", "srp-026g/crates",
                              "tools/re-kit"], capture_output=True, check=True).stdout
    into.mkdir(parents=True)
    subprocess.run(["tar", "-x", "-C", str(into)], input=archive, check=True)
    shared = HERE.parent / "target-arena"                  # the sysroot, keyed by rustc: shared
    shared.mkdir(exist_ok=True)
    os.symlink(shared, into / "srp-026g" / "target-arena")
    return into / "srp-026g" / "client-patches"


def build(tree, original, spec_path, out_dir):
    out_dir.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "EQUIV_ORIGINAL": str(Path(original).resolve())}
    done = subprocess.run([sys.executable, str(Path(__file__).resolve()), "--build-into", str(out_dir),
                           "--spec", str(spec_path)], cwd=tree, env=env, capture_output=True, text=True)
    if done.returncode:
        sys.exit(f"the build in {tree} failed:\n{done.stdout}{done.stderr}")
    return Path(out_dir, "exe").read_bytes(), json.loads(Path(out_dir, "meta.json").read_text())


# ---- the machine -------------------------------------------------------------

class Machine:
    """An exe's sections in Unicorn, memory past them made lazily from the
    case's seed; every write and every leaving of the patch's code recorded."""

    def __init__(self, data, meta, spec):
        from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_BLOCK, UC_HOOK_MEM_WRITE, \
            UC_HOOK_MEM_READ_UNMAPPED, UC_HOOK_MEM_WRITE_UNMAPPED, UC_HOOK_MEM_FETCH_UNMAPPED
        from unicorn import x86_const
        import patch_026g as base
        self.regs = {name: getattr(x86_const, f"UC_X86_REG_{name.upper()}")
                     for name in REG_NAMES + ("esp", "eip", "eflags")}
        self.uc = uc = Uc(UC_ARCH_X86, UC_MODE_32)
        self.prot = UC_PROT_ALL
        self.pristine = []
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            size = (max(vsize, rsize) + PAGE - 1) & ~(PAGE - 1)
            uc.mem_map(start, size, UC_PROT_ALL)
            body = bytes(data[raw:raw + min(rsize, vsize or rsize)])
            uc.mem_write(start, body)
            self.pristine.append((start, size, body))
        uc.mem_map(STACK_TOP - STACK_SIZE, STACK_SIZE, UC_PROT_ALL)
        uc.mem_map(STOP, PAGE, UC_PROT_ALL)
        self.spec = spec
        self.ours = [tuple(r) for r in meta["ranges"]]
        if "engine.code" in meta["blocks"]:                # the library's thunks: the way to the game
            va, size = meta["blocks"]["engine.code"]
            self.ours.append((va, va + size))
        self.blocks = {name: (va, size) for name, (va, size) in meta["blocks"].items()}
        uc.hook_add(UC_HOOK_BLOCK, self.on_block)
        uc.hook_add(UC_HOOK_MEM_WRITE, self.on_write)
        uc.hook_add(UC_HOOK_MEM_READ_UNMAPPED | UC_HOOK_MEM_WRITE_UNMAPPED | UC_HOOK_MEM_FETCH_UNMAPPED,
                    self.on_unmapped)
        self.lazy, self.dirty = set(), set()

    def ours_at(self, va):
        return any(lo <= va < hi for lo, hi in self.ours)

    def read32(self, va):
        return struct.unpack("<I", self.uc.mem_read(va, 4))[0]

    def write32(self, va, value):
        self.uc.mem_write(va, struct.pack("<I", value & 0xFFFFFFFF))

    def reg(self, name):
        return self.uc.reg_read(self.regs[name])

    def set_reg(self, name, value):
        self.uc.reg_write(self.regs[name], value & 0xFFFFFFFF)

    # -- lazily made memory --
    def page_bytes(self, page):
        digest = hashlib.blake2b(struct.pack("<QI", self.seed, page), digest_size=16).digest()
        return random.Random(digest).randbytes(PAGE)

    def map_lazy(self, page):
        self.uc.mem_map(page, PAGE, self.prot)
        self.uc.mem_write(page, self.page_bytes(page))
        self.lazy.add(page)

    def on_unmapped(self, uc, access, address, size, value, user):
        if address < 0x10000 or STACK_TOP - STACK_SIZE - PAGE <= address < STACK_TOP + PAGE:
            return False                                   # the null page and past the stack stay faults
        for page in range(address & ~(PAGE - 1), address + size, PAGE):
            self.map_lazy(page)
        return True

    # -- what a run did --
    def on_write(self, uc, access, address, size, value, user):
        self.dirty.add(address & ~(PAGE - 1))
        if not (STACK_TOP - STACK_SIZE <= address < self.entry_esp):
            self.written.update(range(address, address + size))

    def on_block(self, uc, address, size, user):
        if self.old:
            self.hit.add(address)
        if self.ours_at(address):
            self.inside = True
            return
        if self.stopped or not self.inside or address == STOP:
            return                                         # the game's site, before the patch
        esp = self.reg("esp")
        back = self.read32(esp)
        if self.ours_at(back):                             # a call into the game
            external = (getattr(self.hook, "externals", None) or self.spec.EXTERNALS).get(address)
            if external is None:
                self.error = f"a call of {address:#x}, which the spec does not name (EXTERNALS)"
                uc.emu_stop()
                return
            args = [self.read32(esp + 4 + 4 * int(i[3:])) if i.startswith("arg") else self.reg(i)
                    for i in external.inputs]
            self.calls.append((external.name, args))
            if external.real:
                self.inside = False                        # the game's code, until it returns to ours
                return
            self.set_reg("eax", self.rng.getrandbits(32))
            if external.model:
                external.model(self, args, self.rng)
            self.set_reg("esp", esp + 4 + external.pops)
            self.set_reg("eip", back)
            return
        # left the patch: a jump or a return into the game
        self.exit = (f"{address:#x}", {r: self.reg(r) for r in self.hook.live}, (esp - self.entry_esp) & 0xFFFFFFFF)
        self.stopped = True
        uc.emu_stop()

    def run(self, hook, case, seed, old):
        from unicorn import UcError
        uc = self.uc
        self.seed, self.old, self.hook = seed, old, hook
        self.rng = random.Random(seed * 7919 + 17)
        self.written, self.calls, self.hit = set(), [], set()
        self.exit, self.error, self.stopped, self.inside = None, None, False, False
        esp = self.entry_esp = STACK_TOP - 0x8000
        for address, data in case.memory.items():
            for page in range(address & ~(PAGE - 1), address + len(data), PAGE):
                if not any(start <= page < start + size for start, size, _ in self.pristine) \
                        and page not in self.lazy:
                    self.map_lazy(page)
                self.dirty.add(page)
            uc.mem_write(address, data)
        for i, value in enumerate(case.stack):
            self.write32(esp + 4 * i, value)
        for name in REG_NAMES:
            self.set_reg(name, case.regs.get(name, 0))
        self.set_reg("eflags", case.regs.get("eflags", 0x202))
        self.set_reg("esp", esp)
        self.written.clear()
        try:
            uc.emu_start(hook.site, STOP, count=2_000_000)
        except UcError as error:
            self.error = self.error or f"{error} at {self.reg('eip'):#x}"
        if self.exit is None and self.error is None:
            self.error = f"no exit (stopped at {self.reg('eip'):#x})"
        # the run's machine keeps its memory until reset(): compare() reads it
        return {"exit": self.exit, "error": self.error, "calls": self.calls,
                "written": {self.normal(a) for a in self.written}}

    def normal(self, address):
        """An address as both builds name it: an arena block's by block and
        offset."""
        for name, (va, size) in self.blocks.items():
            if va <= address < va + size:
                return f"{name}+{address - va:#x}"
        return f"{address:#x}"

    def address(self, normal):
        name, _, offset = normal.rpartition("+")
        return self.blocks[name][0] + int(offset, 16) if name else int(normal, 16)

    def final(self, normal):
        """The byte at a place after the run (a page the run did not touch
        as it would have read)."""
        address = self.address(normal)
        try:
            return self.uc.mem_read(address, 1)[0]
        except Exception:
            return self.page_bytes(address & ~(PAGE - 1))[address & (PAGE - 1)]

    def reset(self):
        for page in self.lazy:
            self.uc.mem_unmap(page, PAGE)
        for page in self.dirty - self.lazy:
            for start, size, body in self.pristine:
                if start <= page < start + size:
                    chunk = body[page - start:page - start + PAGE]
                    self.uc.mem_write(page, chunk + bytes(PAGE - len(chunk)))
        self.uc.mem_write(STACK_TOP - STACK_SIZE, bytes(STACK_SIZE))
        self.lazy, self.dirty = set(), set()


# ---- coverage ------------------------------------------------------------------

def blocks_of(data, base, entries, ranges):
    """The first addresses of the basic blocks of the code in `ranges`, by
    recursive descent from `entries` (ndisasm): an entry, a branch target
    inside, the instruction after a branch or a call."""
    sections = [(base.IMAGE_BASE + rva, bytes(data[raw:raw + rsize]))
                for _, rva, _, raw, rsize, _ in base.Headers(data).sections(data)]

    def code_at(va, size):
        return next((body[va - start:va - start + size] for start, body in sections
                     if start <= va < start + len(body)), b"")
    listing = {}
    for lo, hi in ranges:
        text = subprocess.run(["ndisasm", "-b", "32", "-o", str(lo), "-"], input=code_at(lo, hi - lo),
                              capture_output=True, check=True).stdout.decode()
        for line in text.splitlines():
            m = re.match(r"([0-9A-F]{8})\s+([0-9A-F]+)\s+(.*)", line)
            if m:
                listing[int(m.group(1), 16)] = (len(m.group(2)) // 2, m.group(3).strip())
    inside = lambda va: any(lo <= va < hi for lo, hi in ranges)
    leaders, seen, todo = set(entries), set(), list(entries)
    while todo:
        va = todo.pop()
        while va in listing and va not in seen:
            seen.add(va)
            size, text = listing[va]
            word = text.split()[0]
            target = re.search(r"\b(0x[0-9a-f]+)$", text)
            if word.startswith(("j", "loop", "call")):
                if target and inside(int(target.group(1), 16)):
                    leaders.add(int(target.group(1), 16))
                    todo.append(int(target.group(1), 16))
                leaders.add(va + size)
            if word in ("ret", "jmp", "int3", "ud2", "hlt"):
                break
            va += size
    return {va for va in leaders if va in seen}


# ---- the gate ----------------------------------------------------------------

def compare(old, new, a, b):
    """The differences of two runs, as lines: the exits, the calls, and the
    final bytes of every place either run wrote (a byte rewritten with its
    own value is no difference)."""
    out = []
    for key in ("error", "exit", "calls"):
        if a[key] != b[key]:
            out.append(f"  {key}: old {a[key]!r}\n  {' ' * len(key)}  new {b[key]!r}")
    diff = [f"{k}: old {old.final(k)} new {new.final(k)}" for k in sorted(a["written"] | b["written"])
            if old.final(k) != new.final(k)]
    if diff:
        out.append("  memory: " + "; ".join(diff[:12]) + (" ..." if len(diff) > 12 else ""))
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("patch", nargs="?")
    parser.add_argument("original", nargs="?")
    parser.add_argument("--old", default="HEAD~1")
    parser.add_argument("--cases", type=int, default=3000)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--states")
    parser.add_argument("--min-coverage", type=float, default=100.0)
    parser.add_argument("--build-into")
    parser.add_argument("--spec")
    args = parser.parse_args()
    if args.build_into:
        build_into(args.build_into, args.spec)
        return
    if not args.patch or not args.original:
        parser.error("name the patch (equiv/<patch>.py) and the original exe")
    spec_path = HERE / "equiv" / f"{args.patch}.py"
    sys.path.insert(0, str(HERE))
    spec = load_spec(spec_path)
    import patch_026g as base
    with tempfile.TemporaryDirectory(prefix="srp-equiv-", dir=HERE.parent / "target-arena") as folder:
        folder = Path(folder)
        print(f"building the old exe ({args.old}) and the new one (the working tree)", flush=True)
        old_exe, old_meta = build(old_tree(args.old, folder / "tree"), args.original, spec_path, folder / "old")
        new_exe, new_meta = build(HERE, args.original, spec_path, folder / "new")
    old, new = Machine(old_exe, old_meta, spec), Machine(new_exe, new_meta, spec)
    cases = list(spec.cases(args.seed, args.cases))
    if args.states:
        for path in sorted(Path(args.states).glob("*.json")):
            data = json.loads(path.read_text())
            cases += [(hook, Case.from_json(data)) for hook in spec.HOOKS if data.get("hook") == hook.name]
    failed, faults, hit = 0, 0, set()
    for i, (hook, case) in enumerate(cases):
        seed = args.seed * 1_000_003 + i
        a = old.run(hook, case, seed, old=True)
        hit |= old.hit
        b = new.run(hook, case, seed, old=False)
        faults += a["error"] is not None
        diff = compare(old, new, a, b)
        old.reset()
        new.reset()
        if diff:
            failed += 1
            if failed <= 10:
                print(f"DIFF case {i} ({hook.name}: {case.what}):\n" + "\n".join(diff))
    total = blocks_of(old_exe, base, old_meta["entries"], [tuple(r) for r in old_meta["ranges"]])
    covered = total & hit
    percent = 100.0 * len(covered) / len(total) if total else 100.0
    print(f"{len(cases)} cases, {failed} with a difference ({faults} ended in a fault, alike in both)")
    print(f"coverage of the old code: {len(covered)}/{len(total)} basic blocks ({percent:.1f}%)"
          + ("" if covered == total else "; missed " + ", ".join(f"{va:#x}" for va in sorted(total - covered))))
    if failed or percent < args.min_coverage:
        sys.exit("the equivalence gate: FAILED")
    print("the equivalence gate: passed")


if __name__ == "__main__":
    main()
