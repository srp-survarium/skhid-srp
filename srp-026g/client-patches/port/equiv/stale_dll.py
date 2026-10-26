"""The equivalence spec of patch 25, the stale shell_32.dll check
(equiv_026g.py; stale_dll_026g.asm at 0xE00 of `.srp` before the Rust
port, rust/patches/src/stale_dll.rs after).

The hook: sign_in_on_connected's 0x380889 `mov [edi+8],eax` / `mov
[edi+0Ch],dl` (EDI the version field, EAX 0, DL the EULA byte). The asm,
reached by `call` + nop, asks GetModuleHandleA("kernel32.dll"),
GetProcAddress(it, "GetTempPathA"), GetTempPathA(259, a 272-byte buffer),
then CreateFileA(the folder + "shell_32.dll", 0, 7, 0, 3, 80h, 0) and
CloseHandle of what it gave; EAX = 80h | 1 if it opened; then the two
writes, and `ret` onto the nop at 0x38088E (alias: the port's 0x38088F).
Live: every register (EAX the flags, the others as at the site); the
flags are not (0x38088F `mov`, then `add edi,0Ch`). The path buffer is on
the run's own stack: what the calls are given is compared by its text.

Inputs: kernel32 found or not, GetTempPathA found or not, temporary
folders of every length around the room (258, 259, 260 letters and more,
GetTempPathA's answer of the size it needs), 0 (it failed), the file there
or not; the import slots name stubs.
"""
import random

import equiv_026g as E

FIELD = 0x20000000
STUBS = 0x30000000
KERNEL32 = 0x40100000
IMPORTS = {0x87003C: "GetModuleHandleA", 0x870040: "CloseHandle", 0x870048: "CreateFileA",
           0x870084: "GetProcAddress"}
STUB = {name: STUBS + 0x10 * i for i, name in enumerate(list(IMPORTS.values()) + ["GetTempPathA"])}
TEMP = "C:\\Users\\player\\AppData\\Local\\Temp\\"


def module_handle(machine, args, rng):
    machine.set_reg("eax", KERNEL32 if machine.case_kernel32 else 0)


def proc_address(machine, args, rng):
    machine.set_reg("eax", STUB["GetTempPathA"] if machine.case_temp_path else 0)


def temp_path(machine, args, rng):
    size, buffer = args[0], machine.read32(machine.reg("esp") + 8)
    path = machine.case_folder
    if path is None:
        machine.set_reg("eax", 0)
    elif len(path) + 1 > size and not machine.case_lies:
        machine.set_reg("eax", len(path) + 1)
    else:
        machine.uc.mem_write(buffer, path.encode() + b"\0")
        machine.set_reg("eax", len(path))


def create_file(machine, args, rng):
    machine.set_reg("eax", 0x100 + rng.randrange(64) * 4 if machine.case_exists else 0xFFFFFFFF)


EXTERNALS = {
    STUB["GetModuleHandleA"]: E.External("GetModuleHandleA", ["arg0:str"], pops=4, model=module_handle),
    STUB["GetProcAddress"]: E.External("GetProcAddress", ["arg0", "arg1:str"], pops=8, model=proc_address),
    STUB["GetTempPathA"]: E.External("GetTempPathA", ["arg0"], pops=8, model=temp_path),
    STUB["CreateFileA"]: E.External("CreateFileA", ["arg0:str"] + [f"arg{i}" for i in range(1, 7)], pops=28,
                                    model=create_file),
    STUB["CloseHandle"]: E.External("CloseHandle", ["arg0"], pops=4),
}
LIVE = ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")
SIGN_IN = E.Hook("the version field's tail", 0x380889, LIVE, aliases={0x38088E: 0x38088F})
HOOKS = [SIGN_IN]


def ranges(P, linked):
    """Where the patch's code is in a build: stale_dll_026g.asm in `.srp`,
    or (the port) the patches crate's block."""
    if "stale_dll_bin" in dir(P):
        return [(P.SECTION_VA + 0xE00, P.SECTION_VA + 0xE00 + len(open(P.stale_dll_bin(), "rb").read()))]
    # the crate's block, and the one whose memset the arena kept (a C
    # patch's, rust_patches.md: one of them in the arena)
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code"
              or b.kind == "code" and b.va <= linked.symbols.get("memset", 0) < b.va + b.size]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "stale_dll_bin" in dir(P):
        return [P.SECTION_VA + 0xE00]
    return [linked.symbols["stale_dll_on_version_tail"]]


class Case(E.Case):
    """A machine: kernel32, GetTempPathA, the folder (None: it fails), the
    file; the models read them from the machine (set by `run` below)."""

    def __init__(self, what, rng, kernel32=True, temp_path=True, folder=TEMP, exists=False, lies=False):
        memory = {slot: STUB[name].to_bytes(4, "little") for slot, name in IMPORTS.items()}
        memory[FIELD] = b"0.26g0\0\x22" + bytes(rng.getrandbits(8) for _ in range(8))
        regs = {"edi": FIELD, "eax": 0, "edx": rng.getrandbits(32), "ecx": rng.getrandbits(32),
                "ebx": rng.getrandbits(32), "esi": rng.getrandbits(32), "ebp": rng.getrandbits(32),
                "eflags": rng.choice([0x202, 0x246, 0x283])}
        super().__init__(what, regs, [rng.getrandbits(32) for _ in range(16)], memory)
        self.machine = (kernel32, temp_path, folder, exists, lies)


def install():
    """The models read the case's machine: Machine.run is wrapped to set it."""
    if getattr(E.Machine, "_stale_dll", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old):
        (self.case_kernel32, self.case_temp_path, self.case_folder, self.case_exists,
         self.case_lies) = case.machine
        return run(self, hook, case, seed, old)
    E.Machine.run, E.Machine._stale_dll = wrapped, True


install()


def cases(seed, count):
    rng = random.Random(seed)
    long = lambda n: "C:\\" + "t" * (n - 4) + "\\"                   # a folder of n letters
    fixed = [("no shell_32.dll", {}), ("%TEMP%\\shell_32.dll", {"exists": True}),
             ("no kernel32", {"kernel32": False, "exists": True}), ("no GetTempPathA", {"temp_path": False}),
             ("GetTempPathA fails", {"folder": None, "exists": True}), ("an empty folder", {"folder": ""}),
             ("258 letters", {"folder": long(258), "exists": True}), ("259 letters", {"folder": long(259), "exists": True}),
             ("260 letters", {"folder": long(260), "exists": True}), ("300 letters", {"folder": long(300)})]
    for what, machine in fixed:
        for _ in range(5):
            yield SIGN_IN, Case(what, rng, **machine)
    # a GetTempPathA that writes past the size it was given and answers the
    # whole length (the boundary at the room, 259, and past it)
    for n in range(255, 272):
        yield SIGN_IN, Case(f"a lying GetTempPathA, {n} letters", rng, folder=long(n), exists=True, lies=True)
    for _ in range(max(0, count - 5 * len(fixed) - 17)):
        n = rng.choice([rng.randint(0, 270), rng.randint(250, 262)])
        yield SIGN_IN, Case("random machine", rng, kernel32=rng.random() < 0.9, temp_path=rng.random() < 0.9,
                            folder=None if rng.random() < 0.05 else ("" if n == 0 else long(max(n, 4))),
                            exists=rng.random() < 0.5)
