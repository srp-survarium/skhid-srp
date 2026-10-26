"""The equivalence spec of patch 70, the baked textures kept and baked or
loaded once (equiv_026g.py; bake_cache_026g.asm and .c in the arena's block
bake_cache.code before the Rust port, rust/patches/src/bake_cache.rs after).

The hooks, each from the game's own site (sites_026g.tsv):
- game::game's delete, 0x1E9455 `call 0x1E8DA0` (EAX the folder's
  buffer_string): the folder kept, or deleted (0x1E8DA0, stubbed: it empties
  the made-up folder) and stamped; back at 0x1E945A;
- check_loaded's CRC compare, 0x131C90 `cmp eax,[edi+114h]`: back at
  0x131C96 with the compare's flags (the old build's call returns to its
  padding nop 0x131C95: the same place);
- translate_query's look-up, from 0x12D4C5 (`lea edx,[esp+1B8h] / push edx
  / call 0x131ED0`): the look-up 0x131ED0 (a stub; the old build reaches it
  by a tail jump, the new by its replayed call: both return to 0x12D4D2), or
  translate_query's end 0x12D42D;
- the textures made, 0x12F18F and 0x12DDB4 `call 0x420789` (ECX 0xAC3334):
  back at 0x12F194 / 0x12DDB9, EAX ECX+8;
- finish_query, 0xF0D00 (stdcall, `ret 10h`): its body at 0xF0D06 (the
  prologue done), or a return to the caller after the body (a stub at
  0xF0D00 or 0xF0D06, by which the old build's own copy of the prologue
  enters it) and the waiting queries made again;
- the CRC-32 step, 0x42050 (EAX &the CRC, EDX the bytes, [esp+4] the
  count; `ret 4`).
The state (owners, waiters, held textures; the old build's CRC tables) is
the builds' own (PRIVATE): a case that needs it is a sequence of the hooks
(the look-ups that make an owner and its waiters, then a texture made or a
finish), compared at each step.

Live registers at the exits:
- 0x1E945A: EBX, ESI, EDI, EBP. EAX, ECX and EDX are dead: the original
  `call 0x1E8DA0` (void; it ends in `call [0xAEFE78]`, SHFileOperationA,
  stdcall) leaves them undefined, and the code after writes each before it
  reads it (0x1E9461 `mov edx,[ebp+0Ch]`; then 0x1E9477 `mov eax,1E9BE0h`,
  or 0x1E94FD.. 0x1E9506 `mov eax,[ebx+5360h]`, 0x1E9575 `mov eax,4`,
  0x1E9588 `lea edx`, 0x1E9592 `mov ecx`); the old build's EAX, ECX and EDX
  were its C's leftovers.
- 0x131C96: every register and EFLAGS (the compare's, which the jz reads).
- 0x12D4D2 (after the look-up's stub): EAX (the look-up's al), EBX, ESI,
  EDI, EBP; ECX and EDX are written by 0x131ED0 itself before it reads them
  (0x131ED5 `mov ecx,[esi+234h]`, 0x131EE6 `imul ecx`), the stub stands in.
- 0x12D42D: EBX, ESI, EDI, EBP; EAX, ECX, EDX written before read
  (0x12D42D `mov eax,[esp+0A8h]`, 0x12D449 `mov ecx,eax`, 0x12D44B
  `or edx,-1`; the other way, 0x12DA56, pops and returns).
- 0x12F194 / 0x12DDB9: EAX, ECX, EBX, ESI, EDI, EBP. EDX is dead: the
  original's 0x420789 keeps it, the old build's share() clobbered it (its C),
  and the code after writes it (0x12F1D5 / 0x12DDFB `mov edx,..`) before it
  reads it; between, 0xE4F90 takes ESI, EAX, [esp+4] and 0x12DE30 EAX
  (re-kit abi_inputs.tsv).
- 0xF0D06 (the function's body): every register when no path has an owner
  (both builds go straight on); EBX, ESI, EDI, EBP when one has (the old
  build's bc_take, C, clobbered EAX, ECX, EDX first): finish_query is
  stdcall, its arguments on the stack (re-kit: ESP+4..+16 only).
- the return to the caller (`ret 10h`, `ret 4`): finish_query's EBX, ESI,
  EDI, EBP (void, abi.tsv: no result; the old build returned its last
  call's EAX); the CRC step's EAX, ECX, EDX, EBX, ESI, EDI, EBP.

The kernel32 functions are stubs at made-up addresses (the IAT slots point
there), over a made-up folder (the case's, in memory at FS as JSON); a
pointer into the patch's own frame is recorded as what it points to.
GetCurrentThreadId (the new build's lock) is silent. The tables' lock (the
place "lock": the old build's spin word, the crate's TABLES owner word) is
held for another thread in one case, let go at the first `pause`, so both
waits run.
"""
import json
import random
import struct

import equiv_026g as E

FS = 0x6E000000                    # the case's folder, as JSON (length, bytes)
NOW = 0x6D000000                   # GetTickCount
STUBS = 0x6F000000                 # kernel32's stand-ins
IAT = {"FindFirstFileA": 0x870238, "FindNextFileA": 0x87023C, "FindClose": 0x870228, "CreateFileA": 0x870048,
       "ReadFile": 0x870064, "WriteFile": 0x870108, "CloseHandle": 0x870040, "DeleteFileA": 0x87021C,
       "CreateDirectoryA": 0x870220, "GetTickCount": 0x870090, "GetCurrentThreadId": 0x870118}
POPS = {"FindFirstFileA": 8, "FindNextFileA": 8, "FindClose": 4, "CreateFileA": 28, "ReadFile": 20,
        "WriteFile": 20, "CloseHandle": 4, "DeleteFileA": 4, "CreateDirectoryA": 8, "GetTickCount": 0,
        "GetCurrentThreadId": 0}
INVALID = 0xFFFFFFFF
LIST_BEGIN, LIST_END, LIST_CAPACITY, LIST_BYTES, AGES = 0xA2650B0, 0xA2650B4, 0xA2650B8, 0xA2AE0BC, 0x2260920
RECORD = 0x124
STORAGE = 0x21000000               # the list's records
COOK = 0xA264E80
NAMES = 0xAC3334
PREFIX = "user_data/streamed_textures/"
STAMP = "srp_baked_textures.txt"
FOLDER = "c:\\users\\player\\documents\\survarium\\streamed_textures"
ESP = E.STACK_TOP - 0x8000         # the run's ESP at the site
BACK = 0x401000                    # a caller's return address (game code)


def dword(value):
    return struct.pack("<I", value & 0xFFFFFFFF)


def ascii(machine, at, n=600):
    return bytes(machine.uc.mem_read(at, n)).split(b"\0")[0].decode("latin-1")


# ---- the made-up folder --------------------------------------------------------

def fs(machine):
    """The run's folder: the case's (FS), made once per case (a sequence
    keeps it)."""
    state = machine.__dict__.setdefault("bake_fs", {})
    if state.get("seed") != machine.seed:
        size = struct.unpack("<I", machine.uc.mem_read(FS, 4))[0]
        data = json.loads(bytes(machine.uc.mem_read(FS + 4, size)))
        state.clear()
        state.update(seed=machine.seed, files={p: [s, bytes.fromhex(h)] for p, (s, h) in data["files"].items()},
                     dirs=set(data["dirs"]), locked=set(data["locked"]), now=data["now"], handles={}, next=0x100,
                     fail_reads=data.get("fail_reads", 0), fail_allocs=data.get("fail_allocs", 0),
                     heap=0x30000000)
    return state


def windows(name):
    def model(machine, a, rng):
        state = fs(machine)
        set_eax = lambda v: machine.set_reg("eax", v)
        if name == "GetTickCount":
            return set_eax(machine.read32(NOW))
        if name == "GetCurrentThreadId":
            return set_eax(0x1D0C)
        if name == "CreateFileA":
            path, disposition = ascii(machine, a[0]), a[4]
            folder = path.rsplit("\\", 1)[0]
            if disposition == 3 and path in state["files"]:
                pass
            elif disposition == 2 and folder in state["dirs"]:
                state["files"][path] = [0, b""]
            else:
                return set_eax(INVALID)
            handle = state["next"]
            state["next"] += 4
            state["handles"][handle] = [path, 0]
            return set_eax(handle)
        if name == "ReadFile":
            path, at = state["handles"][a[0]]
            if state["fail_reads"] and path.endswith(".dds"):
                state["fail_reads"] -= 1
                return set_eax(0)
            size, head = state["files"][path]
            got = (head + bytes(max(0, size - len(head))))[at:at + a[2]]
            machine.uc.mem_write(a[1], got)
            machine.write32(a[3], len(got))
            state["handles"][a[0]][1] += len(got)
            return set_eax(1)
        if name == "WriteFile":
            path, at = state["handles"][a[0]]
            got = bytes(machine.uc.mem_read(a[1], a[2]))
            size, head = state["files"][path]
            state["files"][path] = [at + len(got), head[:at] + got]
            machine.write32(a[3], len(got))
            return set_eax(1)
        if name in ("CloseHandle", "FindClose"):
            state["handles"].pop(a[0], None)
            return set_eax(1)
        if name == "DeleteFileA":
            path = ascii(machine, a[0])
            if path in state["locked"]:
                return set_eax(0)
            return set_eax(1 if state["files"].pop(path, None) is not None else 0)
        if name == "CreateDirectoryA":
            state["dirs"].add(ascii(machine, a[0]))
            return set_eax(1)
        if name == "FindFirstFileA":
            folder, mask = ascii(machine, a[0]).rsplit("\\", 1)
            names = sorted(p[len(folder) + 1:] for p in state["files"]
                           if p.startswith(folder + "\\") and p.lower().endswith(".dds"))
            names += sorted(d[len(folder) + 1:] + "\\" for d in state["dirs"]
                            if d.startswith(folder + "\\") and d.lower().endswith(".dds"))
            if mask != "*.dds" or not names:
                return set_eax(INVALID)
            handle = state["next"]
            state["next"] += 4
            state["handles"][handle] = [folder, names]
            find_data(machine, state, handle, a[1])
            return set_eax(handle)
        if name == "FindNextFileA":
            return set_eax(1 if find_data(machine, state, a[0], a[1]) else 0)
        raise AssertionError(name)
    return model


def find_data(machine, state, handle, at):
    folder, names = state["handles"][handle]
    if not names:
        return False
    name = names.pop(0)
    directory = name.endswith("\\")
    name = name.rstrip("\\")
    size = 0 if directory else state["files"][folder + "\\" + name][0]
    record = bytearray(320)
    struct.pack_into("<I", record, 0, 0x10 if directory else 0x80)
    struct.pack_into("<II", record, 28, size >> 32, size & 0xFFFFFFFF)
    raw = name.encode("latin-1")[:259]
    record[44:44 + len(raw)] = raw
    machine.uc.mem_write(at, bytes(record))
    return True


def recorded(name):
    """What a kernel32 call's run keeps: the strings and bytes, not the
    pointers into the patch's frame."""
    def record(machine, a):
        if name == "CreateFileA":
            return [ascii(machine, a[0]), a[1], a[2], a[3], a[4], a[5], a[6]]
        if name in ("DeleteFileA", "CreateDirectoryA"):
            return [ascii(machine, a[0])] + a[1:]
        if name == "FindFirstFileA":
            return [ascii(machine, a[0])]
        if name in ("FindNextFileA", "FindClose", "CloseHandle"):
            return [a[0]]
        if name == "ReadFile":
            return [a[0], a[2], a[4]]
        if name == "WriteFile":
            return [a[0], bytes(machine.uc.mem_read(a[1], a[2])).hex(), a[4]]
        return a
    return record


# ---- the game's functions --------------------------------------------------------

def wipe(machine, a, rng):
    """0x1E8DA0: the folder at the buffer_string EAX deleted, but a held file."""
    state = fs(machine)
    begin, end = machine.read32(a[0]), machine.read32(a[0] + 4)
    if not begin or end < begin:
        return
    folder = bytes(machine.uc.mem_read(begin, min(end - begin, 400))).decode("latin-1").rstrip("\\/")

    for path in [p for p in state["files"] if p.startswith(folder + "\\") and p not in state["locked"]]:
        del state["files"][path]
    if not any(p.startswith(folder + "\\") for p in state["files"]):
        state["dirs"].discard(folder)


def malloc(machine, a, rng):
    state = fs(machine)
    if state["fail_allocs"]:
        state["fail_allocs"] -= 1
        machine.set_reg("eax", 0)
        return
    machine.set_reg("eax", state["heap"])
    state["heap"] += 0x200


def resource(machine, a, rng):
    for page in (a[0] & ~0xFFF, (a[0] + 0xD3) & ~0xFFF):
        if not any(lo <= page <= hi for lo, hi, _ in machine.uc.mem_regions()):
            machine.map_lazy(page)
    machine.write32(a[0] + 0xD0, 0)
    machine.set_reg("eax", a[0])


def found(machine, a, rng):
    machine.set_reg("eax", rng.getrandbits(32) & ~0xFF | rng.choice([0, 1]))


EXTERNALS = {
    0x1E8DA0: E.External("delete folder", ["eax"], model=wipe),
    0x420789: E.External("allocation name", ["ecx"], real=True),
    0xE4F90: E.External("malloc", ["esi", "eax", "arg0"], pops=4, model=malloc),
    0x12DE30: E.External("baked texture resource", ["eax"], model=resource),
    0xF0080: E.External("set unmanaged resource", ["edx", "arg0", "arg1", "arg2"], pops=12),
    0xF0D00: E.External("finish query", ["arg0", "arg1", "arg2", "arg3"], pops=16),
    0xF0D06: E.External("finish query", ["arg0", "arg1", "arg2", "arg3"], pops=16, frame=True),
    0x3BEE00: E.External("release texture", ["eax", "arg0"], pops=4),
    0xE0210: E.External("destroy model", ["ecx", "eax"]),
    0x12D1D0: E.External("translate query", ["ecx", "arg0"], pops=4),
    0x131ED0: E.External("find baked texture", ["eax", "arg0"], pops=4, model=found, tail=True),
}
for _i, _name in enumerate(IAT):
    EXTERNALS[STUBS + 0x10 * _i] = E.External(_name, [f"arg{k}" for k in range(POPS[_name] // 4)], pops=POPS[_name],
                                              model=windows(_name), record=recorded(_name),
                                              silent=_name == "GetCurrentThreadId")

ALL = ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")
CALLEE = ("ebx", "esi", "edi", "ebp")
START = E.Hook("game::game's delete", 0x1E9455, CALLEE)
CHECK = E.Hook("check_loaded's compare", 0x131C90, ALL + ("eflags",), aliases={0x131C95: 0x131C96})
LOOKUP = E.Hook("translate_query's look-up", 0x12D4C5, ("eax",) + CALLEE, live_at={0x12D42D: CALLEE})
BAKED = E.Hook("process_compression's texture", 0x12F18F, ("eax", "ecx") + CALLEE)
LOADED = E.Hook("on_baked_texture_loaded's texture", 0x12DDB4, ("eax", "ecx") + CALLEE)
FINISH_QUICK = E.Hook("finish_query, no owner listed", 0xF0D00, ALL)
FINISH = E.Hook("finish_query", 0xF0D00, CALLEE)
CRC = E.Hook("the CRC-32 step", 0x42050, ALL)
HOOKS = [START, CHECK, LOOKUP, BAKED, LOADED, FINISH_QUICK, FINISH, CRC]
PRIVATE = ("bake_cache.state", "patches.state")


def ranges(P, linked):
    """Where the patch's code is in a build: the asm and C's block, or (the
    port) the patches crate's."""
    if "bake_cache.code" in linked.blocks:
        block = linked.blocks["bake_cache.code"]
        return [(block.va, block.va + block.size)]
    # the crate's memcpy and memset are weak: the link's are another block's
    # (research's C) until that is Rust too; theirs is the crate's code here
    homes = {b.name for b in linked.blocks.values() for name in ("memcpy", "memset", "memmove", "bcmp", "strlen")
             if name in linked.symbols and b.va <= linked.symbols[name] < b.va + b.size}
    return [(b.va, b.va + b.size) for b in linked.blocks.values() if b.name in homes | {"patches.code"}]


def places(P, linked):
    """The tables' lock word: the old C's `lock` (its `xchg [lock],eax`), the
    crate's TABLES (vostok::Lock, repr(C): its owner word first)."""
    if "bake_cache.code" in linked.blocks:
        code, state = P.linker.block_bytes(linked, "bake_cache.code"), linked.blocks["bake_cache.state"]
        words = {struct.unpack_from("<I", code, at + 2)[0] for at in range(len(code) - 6)
                 if code[at:at + 2] == b"\x87\x05"}
        words = {w for w in words if state.va <= w < state.va + state.size}
        assert len(words) == 1, words
        return {"lock": (words.pop(), 4)}
    va = next(va for name, va, _, _ in linked.every if name.endswith("10bake_cache6TABLES"))
    return {"lock": (va, 4)}


def entries(P, linked):
    if "bake_cache.code" in linked.blocks:
        names = ("bake_cache_start", "bake_cache_adopt", "bake_cache_lookup", "bake_cache_baked", "bake_cache_loaded",
                 "bake_cache_finish", "bake_cache_crc32")
    else:
        names = ("bake_cache_on_game_start", "bake_cache_on_check_crc", "bake_cache_on_lookup", "bake_cache_on_baked",
                 "bake_cache_on_loaded", "bake_cache_on_finish", "bake_cache_on_crc_step")
    return [linked.symbols[n] for n in names]


# ---- the cases' pieces ---------------------------------------------------------------

def regs(rng, **given):
    out = {r: 0x5000_0000 | rng.getrandbits(24) for r in ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283, 0x893])
    out.update(given)
    return out


def folder_memory(files=None, dirs=(), locked=(), now=1000, fail_reads=0, fail_allocs=0):
    data = json.dumps({"files": {p: [s, h.hex()] for p, (s, h) in (files or {}).items()}, "dirs": sorted(dirs),
                       "locked": sorted(locked), "now": now, "fail_reads": fail_reads,
                       "fail_allocs": fail_allocs}).encode()
    memory = {FS: dword(len(data)) + data, NOW: dword(now)}
    for i, name in enumerate(IAT):
        memory[IAT[name]] = dword(STUBS + 0x10 * i)
    return memory


def baker_memory(records=(), capacity=1024, ages=0, total=0):
    """The baker's list: `records` [(path, crc, size)] at STORAGE."""
    memory = {}
    for i, (path, crc, size) in enumerate(records):
        at = STORAGE + RECORD * i
        raw = path.encode("latin-1")
        record = bytearray(RECORD)
        struct.pack_into("<III", record, 0, at + 0xC, at + 0xC + len(raw), at + 0xC + 0x104)
        record[0xC:0xC + len(raw)] = raw
        record[0x110] = ord("/")
        struct.pack_into("<IIIB", record, 0x114, crc, size, i + 1, 0)
        memory[at] = bytes(record)
    end = STORAGE + RECORD * len(records)
    memory.update({LIST_BEGIN: dword(STORAGE), LIST_END: dword(end), LIST_CAPACITY: dword(STORAGE + RECORD * capacity),
                   LIST_BYTES: dword(total), AGES: dword(ages), end: bytes(RECORD)})
    return memory


def dds(width, height, linear, fourcc=b"DXT1", magic=b"DDS ", size=124):
    header = bytearray(128)
    struct.pack_into("<4sIIIIIII", header, 0, magic, size, 0xA1007, height, width, linear, 0, 9)
    header[84:88] = fourcc
    return bytes(header)


def string_memory(text, at):
    raw = text.encode("latin-1")
    return {at: dword(at + 0xC) + dword(at + 0xC + len(raw)) + dword(at + 0xC + 0x104) + raw + b"\0"}


def revision():
    import patch_026g as P
    return P.REVISION if hasattr(P, "REVISION") else None


# ---- the cases ----------------------------------------------------------------------

STRING = 0x20100000


def start_case(rng, what, files=None, dirs=(), locked=(), folder=FOLDER, capacity=1024, fail_reads=0,
               begin_end=None):
    memory = {**folder_memory(files, dirs, locked, fail_reads=fail_reads), **baker_memory(capacity=capacity,
                                                                                       ages=rng.randrange(100))}
    memory.update(string_memory(folder, STRING))
    if begin_end is not None:
        memory[STRING] = dword(begin_end[0]) + dword(begin_end[1])
    return START, E.Case(what, regs(rng, eax=STRING), [BACK] + [rng.getrandbits(32) for _ in range(8)], memory)


def stamp_of(rev):
    return f"Skhid baked textures, revision {rev}\r\n".encode()


def start_cases(rng, rev):
    stamp = stamp_of(rev)
    a, b = dds(2048, 1024, 1398097), dds(1024, 1024, 1398097, b"DXT5")
    big = 128 + 1398097
    f = lambda name: FOLDER + "\\" + name
    yield start_case(rng, "this revision's folder: kept, the cut one deleted", {
        f(STAMP): (len(stamp), stamp), f("a_s1d.dds"): (big, a), f("b_s1n.dds"): (big, b),
        f("cut_s1d.dds"): (big - 4096, a), f("notes.txt"): (1, b"x")}, dirs={FOLDER, f("folder.dds")})
    yield start_case(rng, "another revision's", {f(STAMP): (len(stamp_of(rev - 1)), stamp_of(rev - 1)),
                                                f("a_s1d.dds"): (big, a)}, dirs={FOLDER})
    yield start_case(rng, "no stamp", {f("a_s1d.dds"): (big, a)}, dirs={FOLDER})
    yield start_case(rng, "no folder")
    yield start_case(rng, "a stamp of the right length, other text",
                     {f(STAMP): (len(stamp), stamp.replace(b"Skhid", b"Skhix"))}, dirs={FOLDER})
    yield start_case(rng, "a stamp with a NUL in it", {f(STAMP): (len(stamp), stamp[:5] + b"\0" + stamp[6:])},
                     dirs={FOLDER})
    yield start_case(rng, "a stamp too long", {f(STAMP): (len(stamp) + 3, stamp + b"abc")}, dirs={FOLDER})
    yield start_case(rng, "a stamp whose read fails, and a dds", {f(STAMP): (len(stamp), stamp), f("a_s1d.dds"):
                                                                    (big, a)}, dirs={FOLDER}, fail_reads=1)
    yield start_case(rng, "held file left: not stamped", {f(STAMP): (3, b"old"), f("a_s1d.dds"): (big, a),
                                                         f("held_s1d.dds"): (big, b)}, dirs={FOLDER},
                     locked={f("held_s1d.dds")})
    yield start_case(rng, "the folder path ends in a separator", {(FOLDER + "\\" + STAMP): (len(stamp), stamp),
                                                                  f("a_s1d.dds"): (big, a)}, dirs={FOLDER},
                     folder=FOLDER + "\\")
    yield start_case(rng, "the folder path ends in a slash, no stamp", {f("a_s1d.dds"): (big, a)}, dirs={FOLDER},
                     folder=FOLDER + "/")
    yield start_case(rng, "an empty path", begin_end=(STRING + 0xC, STRING + 0xC))
    yield start_case(rng, "no begin", begin_end=(0, 0x100))
    yield start_case(rng, "an end before the begin", begin_end=(STRING + 0x20, STRING + 0xC))
    long_folder = "c:\\" + "x" * 262
    yield start_case(rng, "a folder path that does not fit", folder="c:\\" + "y" * 270)
    yield start_case(rng, "a folder path that just fits, a long name", {
        long_folder + "\\" + STAMP: (len(stamp), stamp), long_folder + "\\" + "n" * 40 + ".dds": (big, a),
        long_folder + "\\s.dds": (big, a)}, dirs={long_folder}, folder=long_folder)
    yield start_case(rng, "a folder path that just fits, no stamp", {long_folder + "\\" + "n" * 40 + ".dds":
                                                                       (big, a)}, dirs={long_folder},
                     folder=long_folder)
    held = long_folder + "\\" + "h" * 40 + ".dds"
    yield start_case(rng, "a folder path that just fits, a long name held open and a folder: not stamped",
                     {held: (big, a), long_folder + "\\s.dds": (big, a)},
                     dirs={long_folder, long_folder + "\\d.dds"}, locked={held}, folder=long_folder)
    yield start_case(rng, "another revision's folder with a folder in it", {f(STAMP): (3, b"old"),
                                                                           f("a_s1d.dds"): (big, a)},
                     dirs={FOLDER, f("d.dds")})
    bad = {"magic": dds(64, 64, 2048, magic=b"DDT "), "size": dds(64, 64, 2048, size=120),
           "fourcc": dds(64, 64, 2048, b"DXT3"), "narrow": dds(2, 64, 2048), "wide": dds(8192, 64, 2048),
           "short": dds(64, 2, 2048), "tall": dds(64, 8192, 2048), "nothing": dds(64, 64, 0)}
    files = {f(STAMP): (len(stamp), stamp)}
    for name, head in bad.items():
        files[f(f"{name}_s1d.dds")] = (128 + 2048, head)
    files[f("tiny_s1d.dds")] = (60, b"DDS " + bytes(56))
    files[f("huge_s1d.dds")] = ((1 << 32) + 128 + 2048, dds(64, 64, 2048))
    files[f("ok_s1d.dds")] = (128 + 2048, dds(64, 64, 2048))
    yield start_case(rng, "headers that are no whole DDS", files, dirs={FOLDER})
    files = {f(STAMP): (len(stamp), stamp), f("a_s1d.dds"): (big, a), f("b_s1d.dds"): (big, b),
             f("c_s1d.dds"): (big, a)}
    yield start_case(rng, "a list with room for two", files, dirs={FOLDER}, capacity=2)
    yield start_case(rng, "a read of a dds that fails", files, dirs={FOLDER}, fail_reads=2)
    files = {f(STAMP): (len(stamp), stamp), f("p" * 240 + ".dds"): (big, a)}
    yield start_case(rng, "a name too long for a record", files, dirs={FOLDER, f("sub")})
    small = dds(64, 64, 2048)
    files = {f(STAMP): (len(stamp), stamp)}
    for i in range(530):
        files[f(f"t{i:03}_s1d.dds")] = (128 + 2048, small)
    yield start_case(rng, "530 kept files: 512 in the list", files, dirs={FOLDER})
    huge = dds(4096, 4096, 300 << 20)
    files = {f(STAMP): (len(stamp), stamp), f("h1_s1d.dds"): (128 + (300 << 20), huge),
             f("h2_s1d.dds"): (128 + (300 << 20), huge)}
    yield start_case(rng, "two files of 300 MB: past 512 MB the second deleted", files, dirs={FOLDER})


def check_cases(rng):
    record = 0x22000000
    for crc in (0, 0, 0x64308FFD, 0x0BADF11E):
        for loaded in (0, 0x64308FFD, rng.getrandbits(32)):
            memory = {record + 0x114: dword(crc)}
            yield CHECK, E.Case(f"record {crc:#x}, loaded {loaded:#x}", regs(rng, eax=loaded, edi=record),
                                [rng.getrandbits(32) for _ in range(8)], memory)


class World:
    """Made-up queries, surfaces, parameters, model instances and textures,
    and the steps of a sequence over them."""

    def __init__(self, rng, now=1000, fail_allocs=0):
        self.rng, self.at, self.memory, self.now = rng, 0x24000000, {}, now
        self.steps = []
        self.memory.update(folder_memory(now=now, fail_allocs=fail_allocs))

    def new(self, size):
        at, self.at = self.at, self.at + ((size + 0xFF) & ~0xFF)
        self.memory[at] = bytes(size)
        return at

    def request(self, kind, model_refs=5, model=True):
        """(query, surface, parameters, model)."""
        model_at = self.new(0x100) if model else 0
        if model:
            self.memory[model_at + 0xD0] = dword(model_refs)
        parameters = self.new(0x800)
        self.memory[parameters + 0x7A0] = dword(kind)
        self.memory[parameters + 0x790] = dword(model_at)
        return self.new(0x100), self.new(0x40), parameters, model_at

    def texture(self, references=1):
        at = self.new(0x40)
        self.memory[at + 4] = dword(references)
        return at

    def take_memory(self):
        memory, self.memory = self.memory, {}
        return memory

    def lookup(self, path, request, cook=COOK, what="look-up"):
        query, surface, parameters, _ = request
        frame = self.new(0x40)
        memory = {**self.take_memory(), frame + 8: dword(query),
                  **(string_memory(path, 0x23000000) if path is not None else {})}
        stack = [self.rng.getrandbits(32) for _ in range(0x1C0 // 4)]
        stack[0x1C // 4] = parameters
        stack[0x1B8 // 4] = 0x2300000C if path is not None else 0
        self.steps.append((LOOKUP, E.Case(what, regs(self.rng, eax=cook, ebx=surface, ebp=frame), stack, memory)))

    def baked(self, query, texture, surface, kind, what="baked"):
        stack = [self.rng.getrandbits(32) for _ in range(0x624 // 4)]
        for off, value in ((0x10, texture), (0x604, query), (0x610, surface), (0x620, kind)):
            stack[off // 4] = value
        self.steps.append((BAKED, E.Case(what, regs(self.rng, ecx=NAMES), stack, self.take_memory())))

    def loaded(self, request, texture, what="loaded"):
        query, surface, parameters, _ = request
        frame = self.new(0x40)
        memory = {**self.take_memory(), frame + 0xC: dword(parameters) + dword(surface) + dword(query)}
        self.steps.append((LOADED, E.Case(what, regs(self.rng, ecx=NAMES, edi=texture, ebp=frame),
                                          [self.rng.getrandbits(32) for _ in range(8)], memory)))

    def finish(self, query, result=3, what="finish"):
        stack = [BACK, query, result, 1, 0] + [self.rng.getrandbits(32) for _ in range(4)]
        self.steps.append((FINISH, E.Case(what, regs(self.rng), stack, self.take_memory())))

    def slot(self, surface, kind, texture):
        self.memory[surface + 4 * kind] = dword(texture)

    def clock(self, now):
        """GetTickCount from here on (the folder's JSON again, its files none)."""
        self.memory[NOW] = dword(now)

    def case(self, what):
        *before, (hook, last) = self.steps
        return hook, E.Case(what, last.regs, last.stack, last.memory, before=before)


def sequence_cases(rng):
    x = PREFIX + "scavengers_02_torso_s3d.dds"
    y = PREFIX + "scavengers_02_legs_s3d.dds"

    def world(records=(), now=1000, fail_allocs=0):
        w = World(rng, now, fail_allocs)
        w.memory.update(baker_memory(records))
        return w

    # a waiter's resource not allocated; a waiter whose model's last reference goes
    w = world(fail_allocs=1)
    q1, q2, q3 = w.request(0), w.request(0, model_refs=0), w.request(0)
    w.lookup(x, q1)
    w.lookup(x, q2)
    w.lookup(x, q3)
    t = w.texture()
    w.slot(q1[1], 0, t)
    w.baked(q1[0], t, q1[1], 0)
    yield w.case("a bake shared: the first waiter's resource not allocated, its model's last reference let go")
    w = world()
    q1, q2 = w.request(1), w.request(1, model_refs=0)
    w.lookup(y, q1)
    w.lookup(y, q2)
    w.finish(q1[0], 1)
    yield w.case("an owner that fails: its waiter made again, its model's last reference let go")

    # a bake shared: an owner, two waiters (one with an old texture in its slot, one with no model)
    w = world()
    q1, q2, q3 = w.request(0), w.request(0), w.request(0, model=False)
    w.lookup(x, q1)
    w.lookup(x, q2)
    w.lookup(x, q3)
    t = w.texture()
    old = w.texture(1)
    w.slot(q1[1], 0, t)
    w.slot(q2[1], 0, old)
    w.baked(q1[0], t, q1[1], 0)
    yield w.case("a bake shared with two waiting queries")
    w.finish(q1[0])
    yield w.case("the owner's finish after its bake was shared")

    # an owner that fails: its waiters made again
    w = world()
    q1, q2, q3 = w.request(1), w.request(1, model_refs=1), w.request(1)
    w.lookup(y, q1)
    w.lookup(y, q2)
    w.lookup(y, q3)
    w.finish(q1[0], 1)
    yield w.case("an owner that ends with an error: its waiters made again")

    # an owner with no waiters ending; a query owning nothing while owners exist
    w = world()
    q1 = w.request(2)
    w.lookup(x, q1)
    w.finish(0x2AAA0000)
    yield w.case("finish_query of a query that owns nothing, an owner listed")
    w.finish(q1[0])
    yield w.case("an owner with no waiters ends")
    w.finish(0)
    yield w.case("finish_query of no query, no owner left")

    # a texture made that is not in the owner's slot, or of another type or surface
    for what, kind_made, slot_texture, surface_other, texture_none in (
            ("not in the slot", 2, False, False, False), ("of another type", 1, True, False, False),
            ("of another surface", 2, True, True, False), ("no texture", 2, True, False, True),
            ("a type past 3", 7, True, False, False)):
        w = world()
        q1, q2 = w.request(kind_made if kind_made == 7 else 2), w.request(kind_made if kind_made == 7 else 2)
        w.lookup(x, q1)
        w.lookup(x, q2)
        t = w.texture()
        if slot_texture:
            w.slot(q1[1], kind_made & 3, t)
        surface = w.new(0x40) if surface_other else q1[1]
        w.baked(q1[0], 0 if texture_none else t, surface, kind_made)
        w.finish(q1[0], 3)
        yield w.case(f"a texture made {what}: nothing shared, the waiter made again at the finish")

    # a load: a kept file's record, the waiter gets the texture, it is held; a respawn gets it at once
    records = [(PREFIX + "kept_s1s.dds", 0, 1000), (PREFIX + "big_a_s1d.dds", 0, 200 << 20),
               (PREFIX + "big_b_s1d.dds", 0, 200 << 20), (PREFIX + "huge_s1d.dds", 0, 300 << 20)]
    w = world(records)
    p1, p2 = w.request(3), w.request(3)
    w.lookup(PREFIX + "kept_s1s.dds", p1)
    w.lookup(PREFIX + "kept_s1s.dds", p2)
    t = w.texture()
    w.slot(p1[1], 3, t)
    w.loaded(p1, t)
    yield w.case("a load shared with its waiter, the texture held")
    w.finish(p1[0])
    p3 = w.request(3)
    w.lookup(PREFIX + "kept_s1s.dds", p3)
    yield w.case("a respawn's query of a held texture: given at once")
    p4 = w.request(4)
    w.lookup(PREFIX + "kept_s1s.dds", p4)
    yield w.case("a type past 3 of a held texture's path: looked up")
    # past 256 MB: the least used let go (its last reference released, and one kept)
    held = []
    for name, refs in (("big_a_s1d.dds", 1), ("big_b_s1d.dds", 2)):
        r = w.request(0)
        w.lookup(PREFIX + name, r)
        tex = w.texture(refs)
        w.slot(r[1], 0, tex)
        w.loaded(r, tex)
        w.finish(r[0])
        held.append(tex)
    yield w.case("two textures of 200 MB held: the first let go")
    r = w.request(0)
    w.lookup(PREFIX + "huge_s1d.dds", r)
    tex = w.texture()
    w.slot(r[1], 0, tex)
    w.loaded(r, tex)
    yield w.case("a texture of 300 MB loaded: not held")
    r = w.request(0)
    w.lookup(PREFIX + "big_b_s1d.dds", r)
    tex = w.texture()
    w.slot(r[1], 0, tex)
    w.loaded(r, tex)
    yield w.case("a texture held already, loaded again: not held twice")
    r = w.request(0)
    w.lookup(PREFIX + "unlisted_s1d.dds", r)
    tex = w.texture()
    w.slot(r[1], 0, tex)
    w.loaded(r, tex)
    yield w.case("a loaded texture with no record: not held")

    # 256 held textures: the least used let go for the next
    many = [(PREFIX + f"k{i:03}_s1d.dds", 0, 1000) for i in range(258)]
    w = world(many)
    for i in range(258):
        r = w.request(0)
        w.lookup(many[i][0], r)
        tex = w.texture()
        w.slot(r[1], 0, tex)
        w.loaded(r, tex)
        if i == 3:
            again = w.request(0)
            w.lookup(many[0][0], again)              # the first used again: the second goes first
    yield w.case("258 held textures: the least used let go")

    # an owner that never ends: taken over after 10 s
    w = world()
    s1, s2, s3 = w.request(0), w.request(0), w.request(0)
    w.lookup(x, s1)
    w.lookup(x, s2)
    w.clock(1000 + 10001)
    w.lookup(x, s3)
    t = w.texture()
    w.slot(s3[1], 0, t)
    w.baked(s3[0], t, s3[1], 0)
    w.finish(s1[0], 3)
    yield w.case("an owner listed over 10 s taken over")

    # sixteen waiters, the 17th goes on; a type past 3 goes on
    w = world()
    for i in range(18):
        w.lookup(x, w.request(0))
    w.lookup(x, w.request(5))
    yield w.case("one owner and 16 waiters; the 17th and a type past 3 go on")
    w.lookup(x, w.request(1))
    yield w.case("the same path of another type goes on")

    # 64 owners, a 65th path goes on unowned; 192 waiters, then none free
    w = world()
    for i in range(65):
        w.lookup(PREFIX + f"o{i:02}_s1d.dds", w.request(0))
    yield w.case("65 paths: 64 owners")
    for i in range(12):
        for _ in range(16):
            w.lookup(PREFIX + f"o{i:02}_s1d.dds", w.request(0))
    w.lookup(PREFIX + "o20_s1d.dds", w.request(0))
    yield w.case("192 waiters: none free for the next")

    # look-ups that go on: no path, no query, no parameters, a path too long for an owner
    w = world()
    w.lookup(None, w.request(0))
    yield w.case("a look-up with no path")
    w.lookup(x, (0,) + w.request(0)[1:])
    yield w.case("a look-up with no query")
    q = list(w.request(0))
    q[2] = 0
    w.lookup(x, q)
    yield w.case("a look-up with no parameters")
    w.lookup(PREFIX + "z" * 240, w.request(0))
    yield w.case("a path too long for an owner")

    # the tables' lock held by another thread: the look-up waits, then owns its path
    w = world()
    w.lookup(x, w.request(0))
    w.lookup(x, w.request(0))
    hook, case = w.case("the lock held by another thread, let go: the look-up waits, then waits on the owner")
    case.memory["lock+0x0"], case.release = dword(0x2EEE), "lock+0x0"
    yield hook, case

    # a bake or a load with no owner; a finish with no owner listed
    w = world()
    r = w.request(0)
    w.baked(r[0], w.texture(), r[1], 0)
    yield w.case("a bake of a query that owns nothing")
    w.loaded(r, w.texture())
    yield w.case("a load of a query that owns nothing")
    w = world()
    w.steps.append((FINISH_QUICK, E.Case("finish, no owner", regs(rng), [BACK, 0x2BBB0000, 3, 1, 0],
                                         w.take_memory())))
    yield w.case("finish_query, no owner listed")


def crc_cases(rng, count):
    buffer = 0x26000000
    data = bytes(rng.getrandbits(8) for _ in range(0x2000))
    def one():
        length = rng.choice([0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 64, 100, 255, 1000, 4093])
        start = buffer + rng.randrange(8)
        crc_at = 0x27000000 + 4 * rng.randrange(16)
        memory = {buffer: data, crc_at: dword(rng.getrandbits(32))}
        given = {"eax": crc_at, "edx": start}
        if length == 0 and rng.random() < 0.5:
            given["eax"] = 0                           # nothing read: not even the CRC
        return E.Case(f"{length} bytes from +{start - buffer}", regs(rng, **given),
                      [BACK, length] + [rng.getrandbits(32) for _ in range(6)], memory)
    for i in range(count):
        case = one()
        if i % 4 == 0:                                 # after another: the old build's tables made already
            first = one()
            case = E.Case(case.what + ", after another step", case.regs, case.stack, case.memory,
                          before=[(CRC, first)])
        yield CRC, case


def cases(seed, count):
    import patch_026g as P
    rng = random.Random(seed)
    rev = P.REVISION
    out = list(start_cases(rng, rev)) + list(check_cases(rng)) + list(sequence_cases(rng))
    out += list(crc_cases(rng, max(0, count - len(out))))
    return out
