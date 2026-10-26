"""The equivalence spec of patch 29, "Watch replay" (equiv_026g.py;
replay_link_026g.asm at 0x220 of `.srp` before the Rust port,
rust/patches/src/replay_link.rs after).

The hook: video_tutorial_link_clicked's 0x28CDB6 `push 1 / push 0 / push 0`
(EAX the lobby client, its link at +1E6144h; the function pushed ESI, then
the return address and its argument). The asm checks the link against
`replay <8 hex> <32 hex> <64 hex> <1..10 digits>`; a link that fits starts
<this exe's folder>\\updater_skhid.exe with `updater_skhid.exe <link>`:
GetModuleFileNameW(0, path, 520), GetModuleHandleA("kernel32.dll"),
GetProcAddress(it, "CreateProcessW"), CreateProcessW(app, command, 0, 0, 0,
0, 0, folder, &startup, &process), CloseHandle(thread), CloseHandle(process);
then `popad / mov al,1 / pop esi / ret 4`. The port leaves at the
function's own 0x28CDDD (`mov al,1 / pop esi / ret 4`): both runs go on to
the return address (STOP, `until`), where EAX (AL 1, the rest the client),
EBX, ESI (the function's own, popped), EDI, EBP and ESP are compared. The
texts the calls get are on each run's own stack: compared by what they
hold.

Inputs: links that fit and every way one does not (each piece, the hex
case, the digits' count, the end), exe paths of every length around the
room (0, 1, 518, 519, 520), with no folder, kernel32 or CreateProcessW
missing, a start that fails; the import slots name stubs.
"""
import random

import equiv_026g as E

CLIENT = 0x30000000
LINK = CLIENT + 0x1E6144
STUBS = 0x31000000
KERNEL32 = 0x40100000
IMPORTS = {0x870074: "GetModuleFileNameW", 0x87003C: "GetModuleHandleA", 0x870084: "GetProcAddress",
           0x870040: "CloseHandle"}
STUB = {name: STUBS + 0x10 * i for i, name in enumerate(list(IMPORTS.values()) + ["CreateProcessW"])}
FOLDER = "C:\\Games\\Survarium\\game\\binaries\\x86"
EXE = FOLDER + "\\survarium_skhid.exe"
ID, TOKEN, SHA = "0a1b2c3d", "0123456789abcdef" * 2, "fedcba9876543210" * 4
VALID = f"replay {ID} {TOKEN} {SHA} 1048576"


def module_file_name(machine, args, rng):
    size = args[1]
    buffer = machine.read32(machine.reg("esp") + 8)      # on the run's own stack: not an input compared
    path = machine.case_path
    if path is None:
        machine.set_reg("eax", 0)
        return
    units = path.encode("utf-16-le")
    if len(path) >= size:                          # cut short: size units, no zero (as XP does)
        machine.uc.mem_write(buffer, units[:2 * size])
        machine.set_reg("eax", size)
    else:
        machine.uc.mem_write(buffer, units + b"\0\0")
        machine.set_reg("eax", len(path))


def module_handle(machine, args, rng):
    machine.set_reg("eax", KERNEL32 if machine.case_kernel32 else 0)


def proc_address(machine, args, rng):
    machine.set_reg("eax", STUB["CreateProcessW"] if machine.case_create else 0)


def create_process(machine, args, rng):
    started = machine.case_starts
    machine.set_reg("eax", int(started))
    if started:
        process = machine.read32(machine.reg("esp") + 4 + 4 * 9)
        machine.uc.mem_write(process, (0x120).to_bytes(4, "little") + (0x124).to_bytes(4, "little")
                             + (0x1000).to_bytes(4, "little") + (0x1004).to_bytes(4, "little"))


EXTERNALS = {
    STUB["GetModuleFileNameW"]: E.External("GetModuleFileNameW", ["arg0", "arg2"], pops=12, model=module_file_name),
    STUB["GetModuleHandleA"]: E.External("GetModuleHandleA", ["arg0:str"], pops=4, model=module_handle),
    STUB["GetProcAddress"]: E.External("GetProcAddress", ["arg0", "arg1:str"], pops=8, model=proc_address),
    STUB["CreateProcessW"]: E.External("CreateProcessW", ["arg0:wstr", "arg1:wstr", "arg2", "arg3", "arg4", "arg5",
                                                          "arg6", "arg7:wstr", "arg8:mem:68", "arg9:mem:16"],
                                       pops=40, model=create_process),
    STUB["CloseHandle"]: E.External("CloseHandle", ["arg0"], pops=4),
}
LIVE = ("eax", "ebx", "esi", "edi", "ebp")
CLICKED = E.Hook("the link clicked", 0x28CDB6, LIVE, until=E.STOP)
HOOKS = [CLICKED]


def ranges(P, linked):
    """Where the patch's code is in a build: replay_link_026g.asm in `.srp`,
    or (the port) the patches crate's block (and the block whose memset the
    arena kept)."""
    if "replay_link_bin" in dir(P):
        return [(P.SECTION_VA + 0x220, P.SECTION_VA + 0x220 + len(open(P.replay_link_bin(), "rb").read()))]
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code"
              or b.kind == "code" and b.va <= linked.symbols.get("memset", 0) < b.va + b.size]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "replay_link_bin" in dir(P):
        return [P.SECTION_VA + 0x220]
    return [linked.symbols["replay_link_on_clicked"]]


class Case(E.Case):
    """A click: the link, the exe's path (None: GetModuleFileNameW fails),
    kernel32, CreateProcessW, whether the start succeeds."""

    def __init__(self, what, rng, link, path=EXE, kernel32=True, create=True, starts=True):
        memory = {slot: STUB[name].to_bytes(4, "little") for slot, name in IMPORTS.items()}
        memory[LINK] = link.encode("latin-1") + b"\0" + E.made_up(rng, 16)
        regs = {"eax": CLIENT, "ecx": rng.getrandbits(32), "edx": rng.getrandbits(32), "ebx": rng.getrandbits(32),
                "esi": rng.getrandbits(32), "edi": rng.getrandbits(32), "ebp": rng.getrandbits(32),
                "eflags": rng.choice([0x202, 0x246, 0x283])}
        stack = [rng.getrandbits(32), E.STOP, rng.getrandbits(32)] + [rng.getrandbits(32) for _ in range(13)]
        super().__init__(what, regs, stack, memory)
        self.machine = (path, kernel32, create, starts)


def install():
    if getattr(E.Machine, "_replay_link", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old, **more):
        self.case_path, self.case_kernel32, self.case_create, self.case_starts = case.machine
        return run(self, hook, case, seed, old, **more)
    E.Machine.run, E.Machine._replay_link = wrapped, True


install()


def cases(seed, count):
    rng = random.Random(seed)
    bad = [VALID.upper(), VALID[:-1], VALID + "x", VALID.replace(" 1048576", " 12345678901"),
           VALID.replace(" 1048576", " "), VALID.replace(" 1048576", ""), VALID.replace(ID, ID[:-1]),
           VALID.replace(ID, ID + "0"), VALID.replace(TOKEN, TOKEN.upper()), VALID.replace("replay ", "replay"),
           VALID.replace(SHA, SHA[:63] + "g"), "", "https://example.com/replay", "replay", VALID.replace(" ", "  ", 1)]
    good = [VALID, VALID.replace(" 1048576", " 0"), VALID.replace(" 1048576", " 1234567890")]
    for link in bad:
        yield CLICKED, Case(f"not a replay link: {link[:24]!r}", rng, link)
    for link in good:
        for path, what in ((EXE, "the exe"), ("C:\\a.exe", "a short path"), ("x.exe", "no folder"),
                           (None, "no path"), ("", "an empty path"),
                           ("C:\\" + "d" * 497 + "\\survarium.exe", "a 514-unit path"),
                           ("C:\\" + "d" * 500 + "\\" + "e" * 15, "a 519-unit path"),
                           ("C:\\" + "d" * 501 + "\\" + "e" * 15, "a 520-unit path, cut short"),
                           ("C:\\" + "d" * 497 + "\\x", "a folder of 501 units"),
                           ("C:\\" + "d" * 498 + "\\x", "a folder of 502 units"),
                           ("C:\\" + "d" * 499 + "\\x", "a folder of 503 units")):
            yield CLICKED, Case(f"{link[-10:]} {what}", rng, link, path=path)
        yield CLICKED, Case("no kernel32", rng, link, kernel32=False)
        yield CLICKED, Case("no CreateProcessW", rng, link, create=False)
        yield CLICKED, Case("the start fails", rng, link, starts=False)
    hexd = lambda n: "".join(rng.choice("0123456789abcdef") for _ in range(n))
    for _ in range(max(0, count - 60)):
        link = f"replay {hexd(8)} {hexd(32)} {hexd(64)} {rng.randrange(10 ** rng.randint(1, 10))}"
        if rng.random() < 0.5:
            at = rng.randrange(len(link))
            link = link[:at] + rng.choice("0aAgZ :\x01") + link[at + 1:]
        folder = "C:\\" + "\\".join("d" * rng.randint(1, 60) for _ in range(rng.randint(0, 9)))
        path = rng.choice([folder + "\\survarium.exe", folder, None])
        yield CLICKED, Case("random click", rng, link, path=path, kernel32=rng.random() < 0.95,
                            create=rng.random() < 0.95, starts=rng.random() < 0.8)
