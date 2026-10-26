"""The equivalence spec of patches 27 and 49, resources.db's patches mounted
over it and the ones an exe requires (equiv_026g.py; mount_patches_026g.asm
at 0x400 of `.srp`, its state at 0x800 of the late join's state page, before
the Rust port; rust/patches/src/mount_patches.rs after).

- query, 0xE7E3E: query_db's `call 0E7EF0h` (ESI the allocator, [esp] to
  [esp+10h] the virtual path, the file table's and the archive's path
  strings, the descriptor, the callback). The asm is called there (pushad
  ... popad, `ret` to 0xE7E43); the port's lohook skips the call and jumps
  to 0xE7E43. Both probe ..\\..\\resources.db.patch_1 to _255 (the path
  formatting 0x18190 with "../../%s.db.patch_%u", CreateFileA, GetLastError
  after a failure, CloseHandle), stop at the fatal 0x113580 for a file that
  cannot be opened or a required one not there, else queue resources.db
  (query_mount_archive 0xE7EF0, the game's five arguments) and each patch
  found (its own path string twice). Live at 0xE7E43: every register.
- mounted, 0x147E0: on_mounted_resources' start (cdecl: [esp+4] where the
  mount is kept, [esp+8] the next step, [esp+0Ch] &the mount). The game's
  own path (no start-up mount pending, or resources.db's mount failed): the
  asm jumps to its `game` label, the six bytes and `jmp 147E6h`; the port's
  lohook returns to its replay of them. Both are left out of the ranges, so
  the run ends where each enters them, with the stack as at the site (alias:
  0x147E0). The last callback calls the game's callback with the state's
  slot 0: the asm its `game` label, the port 0x147E0 (the hook again, which
  sends it to the game's code): one External at both. Live at either exit:
  EBX, ESI, EDI, EBP (cdecl).

The state compares by place: remaining, seen, the 256 mounts and the 255
numbers (the asm's at the state's +0, +4, +8, +408h; the port's statics),
and the bitmap of the required patches after the "Cannot open" text (the
asm's `needed`; the port's SRP_REQUIRED_PATCHES), which a case writes.
"""
import random
import struct

import equiv_026g as E

STUBS = 0x30000000
IMPORTS = {0x870040: "CloseHandle", 0x870048: "CreateFileA", 0x87005C: "GetLastError"}
STUB = {name: STUBS + 0x10 * i for i, name in enumerate(IMPORTS.values())}
FORMAT_PATH, QUERY_MOUNT_ARCHIVE, FATAL, UNMOUNT, ON_MOUNTED = 0x18190, 0xE7EF0, 0x113580, 0xDFE30, 0x147E0
OLD_CODE = 0xA425400                    # the asm's place in `.srp`
OLD_GAME = OLD_CODE + 0x7D              # its `game` label: push ebp / mov ebp,esp / and esp,-8 / jmp 147E6h
OLD_STATE = 0xA428800
ALLOCATOR, VIRTUAL, ARCHIVE, DESCRIPTOR, CALLBACK = 0x0A7B688, 0x21000000, 0x21000200, 0x21000400, 0x21000500
HOLDER, BOUND, RESULT = 0x21100000, 0x21100100, 0x21100200
MOUNTS = 0x21200000                     # made-up mounts, 40h apart
DB = "..\\..\\resources.db"


def path_string(at, text, separator):
    raw = text.encode("ascii")
    return struct.pack("<III", at + 12, at + 12 + len(raw), at + 0x110) + raw.ljust(0x104, b"\0") \
        + separator.encode("ascii")


def string(machine, at):
    """A path string's text (begin to end), or the game's own by its name."""
    if at in (VIRTUAL, ARCHIVE):
        return f"the game's {at:#x}"
    begin, end = machine.read32(at), machine.read32(at + 4)
    return machine.text(begin, 0x110).decode("latin-1"), end - begin


def format_path(machine, args, rng):
    """0x18190: the text from the string's begin, each / the separator; EAX
    the string."""
    at = machine.at_call["esi"]
    begin, capacity = machine.read32(at), machine.read32(at + 8)
    separator = machine.uc.mem_read(at + 0x110, 1)[0]
    text = args[1].replace("%s", args[2], 1).replace("%u", str(args[3]), 1).encode("ascii")
    text = text.replace(b"/", bytes([separator]))[:capacity - begin - 1]
    machine.uc.mem_write(begin, text + b"\0")
    machine.write32(at + 4, begin + len(text))
    machine.set_reg("eax", at)
    clobber(machine)


def layout(machine):
    """What the formatting is given: the string's begin and buffer end
    (by its own address) and its separator. Not its end: 0x18190 sets it
    to begin before any read (mov eax,[esi] / mov [esi+4],eax / mov byte
    [eax],0), and the asm leaves the last text's there, the port begin."""
    at = machine.reg("esi")
    return machine.read32(at) - at, machine.read32(at + 8) - at, machine.uc.mem_read(at + 0x110, 1)[0]


def clobber(machine):
    machine.set_reg("ecx", machine.rng.getrandbits(32))
    machine.set_reg("edx", machine.rng.getrandbits(32))


def create_file(machine, args, rng):
    name = args[0]
    n = int(name.rsplit("_", 1)[1]) if name.startswith(DB + ".patch_") else None
    error = machine.case_files.get(n, 2)
    machine.case_error = error
    machine.set_reg("eax", 0x100 + rng.randrange(64) * 4 if error is None else 0xFFFFFFFF)
    clobber(machine)


def last_error(machine, args, rng):
    machine.set_reg("eax", machine.case_error)
    clobber(machine)


def fatal(machine, args, rng):
    machine.exit = ("fatal", {}, 0)
    machine.stopped = True
    machine.uc.emu_stop()


def fatal_second(machine):
    """The format's second %u, when it has one (the patch that cannot be
    opened: its error; the failed mount's format has one %u)."""
    esp = machine.reg("esp")
    return machine.read32(esp + 12) if machine.text(machine.read32(esp + 4), 200).count(b"%u") > 1 else None


def queued_path(index):
    return lambda machine: string(machine, machine.read32(machine.reg("esp") + 4 + 4 * index))


def on_mounted(machine, args, rng):
    """The game's callback: engine_world takes a reference of the mount (or,
    a case's made-up game, does not)."""
    mount = machine.read32(machine.read32(machine.reg("esp") + 12))
    if machine.case_takes and mount:
        machine.write32(mount + 0x1C, machine.read32(mount + 0x1C) + 1)
    clobber(machine)


def handed(machine):
    """The callback's third argument: a place of the state, and the mount it
    holds."""
    at = machine.read32(machine.reg("esp") + 12)
    return machine.normal(at, places_only=True), machine.read32(at)


ON_MOUNTED_EXTERNAL = E.External("on_mounted_resources", ["arg0", "arg1", handed], pops=0, model=on_mounted)
EXTERNALS = {
    STUB["CreateFileA"]: E.External("CreateFileA", ["arg0:str"] + [f"arg{i}" for i in range(1, 7)], pops=28,
                                    model=create_file),
    STUB["GetLastError"]: E.External("GetLastError", [], pops=0, model=last_error),
    STUB["CloseHandle"]: E.External("CloseHandle", ["arg0"], pops=4, model=lambda m, a, r: clobber(m)),
    FORMAT_PATH: E.External("format_path", [layout, "arg0:str", "arg1:str", "arg2"], pops=0, model=format_path),
    QUERY_MOUNT_ARCHIVE: E.External("query_mount_archive", ["esi", "arg0", queued_path(1), queued_path(2), "arg3",
                                                            "arg4"], pops=0, model=lambda m, a, r: clobber(m)),
    FATAL: E.External("fatal", ["arg0:str", "arg1", fatal_second], pops=0, model=fatal),
    UNMOUNT: E.External("unmount", ["eax"], pops=0, model=lambda m, a, r: clobber(m)),
    ON_MOUNTED: ON_MOUNTED_EXTERNAL,
    OLD_GAME: ON_MOUNTED_EXTERNAL,
}
QUERY = E.Hook("query", 0xE7E3E, ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp"))
# (the fatal is a tail: the asm's call of it returns onto its `game` label,
# out of the ranges)
MOUNTED = E.Hook("mounted", 0x147E0, ("ebx", "esi", "edi", "ebp"), aliases={OLD_GAME: 0x147E0},
                 tails=(FATAL,))
HOOKS = [QUERY, MOUNTED]


def old(P):
    return "mount_patches_bin" in dir(P)


def replay(linked):
    """The port's replay of the six bytes (its lohook's `_replay`: the six
    bytes and a jmp 147E6h, 11 bytes)."""
    return next(va for name, va, _, _ in linked.every if name == "mount_patches_on_mounted_replay")


def ranges(P, linked):
    if old(P):
        size = len(open(P.mount_patches_bin(), "rb").read())
        assert open(P.mount_patches_bin(), "rb").read()[OLD_GAME - OLD_CODE:][:7] == bytes.fromhex("5589e583e4f8e9")
        return [(OLD_CODE, OLD_GAME), (OLD_GAME + 11, OLD_CODE + size)]
    blocks = [b for b in linked.blocks.values() if b.name in ("patches.code", "engine.code")
              or b.kind == "code" and b.va <= linked.symbols.get("memset", 0) < b.va + b.size]
    out = []
    at = replay(linked)
    for b in blocks:
        if b.va <= at < b.va + b.size:
            out += [(b.va, at), (at + 11, b.va + b.size)]
        else:
            out.append((b.va, b.va + b.size))
    return out


def aliases(P, linked):
    """The port's way into the game's own: its replay of the six bytes, for
    0x147E0 (the asm's `game` label is the hook's alias)."""
    return {} if old(P) else {replay(linked): 0x147E0}


def entries(P, linked):
    if old(P):
        return [OLD_CODE, OLD_CODE + 0x88]
    return [linked.symbols["mount_patches_on_query"], linked.symbols["mount_patches_on_mounted"]]


def places(P, linked):
    if old(P):
        code = open(P.mount_patches_bin(), "rb").read()
        return {"remaining": (OLD_STATE, 4), "seen": (OLD_STATE + 4, 4), "mounts": (OLD_STATE + 8, 0x400),
                "numbers": (OLD_STATE + 0x408, 255), "required": (OLD_CODE + len(code) - 4 - 32, 32)}
    statics = {name: va for name, va, _, _ in linked.every if "mount_patches" in name}
    named = lambda word: next(va for name, va in statics.items() if word in name)
    return {"remaining": (named("REMAINING"), 4), "seen": (named("SEEN"), 4), "mounts": (named("MOUNTS"), 0x400),
            "numbers": (named("NUMBERS"), 255),
            "required": (linked.symbols["SRP_REQUIRED_PATCHES"] + len(P.MOUNT_UNREADABLE), 32)}


def install():
    """The models read the case's machine: Machine.run is wrapped to set it."""
    if getattr(E.Machine, "_mount_patches", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old):
        self.case_files, self.case_takes = case.files, case.takes
        self.case_error = None
        return run(self, hook, case, seed, old)
    E.Machine.run, E.Machine._mount_patches = wrapped, True


install()


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283])
    out.update(fixed)
    return out


def bitmap(numbers):
    out = bytearray(32)
    for n in numbers:
        out[n >> 3] |= 1 << (n & 7)
    return bytes(out)


class Case(E.Case):
    def __init__(self, what, regs_, stack, memory, files=None, takes=True):
        super().__init__(what, regs_, stack, memory)
        self.files, self.takes = dict(files or {}), takes


def query_case(rng, what, files, required=()):
    """`files`: n -> None (there) or CreateFileA's error; any other n is
    not there (error 2)."""
    memory = {slot: STUB[name].to_bytes(4, "little") for slot, name in IMPORTS.items()}
    memory[VIRTUAL] = path_string(VIRTUAL, "resources", "/")
    memory[ARCHIVE] = path_string(ARCHIVE, DB, "\\")
    memory[DESCRIPTOR] = b"resources\0"
    memory["required+0x0"] = bitmap(required)
    memory["remaining+0x0"] = struct.pack("<I", rng.choice([0, 0, 3]))
    memory["seen+0x0"] = struct.pack("<I", rng.choice([0, 0, 2]))
    memory["numbers+0x0"] = bytes(rng.getrandbits(8) for _ in range(255))
    stack = [VIRTUAL, ARCHIVE, ARCHIVE, DESCRIPTOR, CALLBACK] + [rng.getrandbits(32) for _ in range(11)]
    return QUERY, Case(what, regs(rng, esi=ALLOCATOR), stack, memory, files)


def mounted_case(rng, what, remaining, seen, mount, slot0=None, references=1, takes=True, numbers=None):
    """A callback: `mount` its count (None: a failed mount), the state's
    remaining and seen, slot 0's mount's count."""
    memory = {"remaining+0x0": struct.pack("<I", remaining), "seen+0x0": struct.pack("<I", seen),
              "numbers+0x0": bytes(numbers or [rng.randrange(1, 256) for _ in range(255)])}
    memory["mounts+0x0"] = b"".join(struct.pack("<I", MOUNTS + 0x40 * (i + 1) if i < seen else 0)
                                    for i in range(256))
    for i in range(seen):
        memory[MOUNTS + 0x40 * (i + 1) + 0x1C] = struct.pack("<I", slot0 if i == 0 and slot0 is not None else 1)
    memory[RESULT] = struct.pack("<I", 0 if mount is None else MOUNTS)
    if mount is not None:
        memory[MOUNTS + 0x1C] = struct.pack("<I", mount)
    stack = [rng.getrandbits(32), HOLDER, BOUND, RESULT] + [rng.getrandbits(32) for _ in range(12)]
    return MOUNTED, Case(what, regs(rng), stack, memory, takes=takes)


def cases(seed, count):
    rng = random.Random(seed)
    fixed = [
        ("no patch", {}, ()), ("patches 1 to 3", {1: None, 2: None, 3: None}, ()),
        ("patch 255 alone", {255: None}, ()), ("every patch", {n: None for n in range(1, 256)}, ()),
        ("2 path not found", {1: None, 2: 3, 3: None}, ()),
        ("3 a folder (error 5)", {1: None, 2: None, 3: 5}, ()), ("255 in use (error 32)", {14: None, 255: 32}, ()),
        ("1 access denied", {1: 5, 2: None}, ()),
        ("required 1, 12, 14, 17, 255, all there", {n: None for n in (1, 12, 14, 17, 255)}, (1, 12, 14, 17, 255)),
        ("required, 14 missing", {n: None for n in (1, 12, 17, 255)}, (1, 12, 14, 17, 255)),
        ("required, 12 path not found", {1: None, 12: 3, 14: None}, (1, 12, 14)),
        ("required, 13 path not found, not required", {1: None, 13: 3}, (1,)),
        ("required 255, missing", {1: None}, (255,)),
    ]
    out = []
    for what, files, required in fixed:
        out.append(query_case(rng, what, files, required))
    for remaining, seen, mount, slot0, takes, what in (
            (0, 0, 1, None, True, "not a start-up mount"), (0, 5, None, None, True, "not a start-up mount, failed"),
            (3, 0, None, None, True, "resources.db failed"), (2, 1, None, None, True, "patch 1 failed"),
            (2, 255, None, None, True, "patch 255 failed"), (3, 0, 1, None, True, "resources.db held"),
            (2, 1, 1, None, True, "a patch held"), (1, 0, 1, None, True, "resources.db alone, the last"),
            (1, 3, 1, 1, True, "the last of four"), (1, 3, 1, 1, False, "the last, the game takes none"),
            (1, 255, 1, 1, True, "the last of 256"), (1, 2, 1, 5, True, "the last, slot 0 held more")):
        for _ in range(4):
            out.append(mounted_case(rng, what, remaining, seen, mount, slot0, takes=takes))
    for case in out:
        yield case
    for i in range(max(0, count - len(out))):
        if i % 3 == 0:
            files = {n: rng.choice([None, None, 2, 3, 5, 32]) for n in rng.sample(range(1, 256), rng.randrange(0, 12))}
            required = tuple(rng.sample(range(1, 256), rng.randrange(0, 4)))
            if rng.random() < 0.5:
                files.update({n: None for n in required})
            yield query_case(rng, "random folder", files, required)
        else:
            remaining = rng.choice([0, 1, 1, 2, 5])
            seen = rng.choice([0, 1, 2, rng.randrange(0, 256)])
            yield mounted_case(rng, "random callback", remaining, seen, rng.choice([None, 1, 1, 2]),
                               rng.choice([1, 1, 2]) if seen else None, takes=rng.random() < 0.8)
