"""Self-test of patch_026g.py. Builds every variant from the ORIGINAL 0.26g
exe (read only), writes the results to SRP_026G_SCRATCH (never the game
folder) and checks them, each build (and the checks of the tools) in a
process of its own, on every core (main). Nothing is run and nothing is
installed. A release runs it (release/release.sh, step 2, its Linux part);
it is not a check of every change.

    python selftest_026g.py [<survarium.exe>]

The general rules, on every build (cert-only, section, hash-mismatch, with
and without an embedded server, the default build with the late join): what
a build changed is what build() recorded (patch_026g.py APPLIED: every write
a named instance's), so a patch that declares its writes needs no line here.
 - the checked-in .bin files are what nasm makes today, the engine library's
   and the hooks' generated files what their tables give, the arena the
   lock's (test_bins, test_late_join_bin);
 - the PE parses: the section table, alignments, SizeOfImage, the data
   directories, the certificate table last and the original's (test_pe and
   the sections of test_late_join);
 - replaying the recorded writes onto the original gives the build: no byte
   outside a declared write differs (test_writes);
 - a base relocation is turned off only under a changed byte, one left on
   under a changed byte names an address of the image, the rest of the table
   is the original's (test_writes);
 - nothing of the original (a rel32 or rel8 branch that is an instruction of
   its function, a relocated value: pointers, jump tables) aims inside the
   bytes a write changed in .text (test_writes);
 - a call into our code returns onto nops to the end of its write
   (test_writes);
 - every function the game calls of ours is declared (INSTALLS) and pops what
   the ABI says its model pops: re-kit abi.tsv and the original's own `ret`;
   our vtables' slots as the game's (test_call_shapes, its negative cases);
 - every branch of our code lands on one of its instructions or a known
   target: a function the re-kit or the engine library names, an install
   row's way back and the branches of the bytes it replaced, a site's named
   exit, patch_026g.py's *_BACKS / *_EXITS / *_RETURN / *_END
   (known_targets; check_piece and each patch's check of its code);
 - every Rust hook's site branches to its generated entry, a lohook's entry
   saves the machine and calls its shim (a shared site's chain in the TOML's
   `order`); the crate's code branches and stores only where it may
   (test_rust_hooks, test_rust_code);
 - every row of patch_026g.py's install tables, every Rust hook and every
   INSTALLS row is written by some build here (main's end);
 - the arena links deterministically and refuses what it must (test_arena),
   apply() and test_writes refuse what they must (test_apply), the engine
   library's thunks against the exe (test_engine, check_engine_026g.py), the
   hook kinds' templates (test_hook_kinds, check_hooks_026g.py).
Each patch's own check (test_<patch>) is of OUR code alone: what guards its
ABI (the registers kept, the instructions it replays, its `ret`), its state
and data where patch_026g.py puts them, the movies or texts it needs; how
the original works there is in client_patch.md, and the patch's Unicorn
check (check_<patch>_026g.py) runs it. Then the build options: the embedded
server, the revision (worked out from the instructions that fill the
version field), --revision and --required-patches. On Windows, imagehlp's
checksum and the certificate the client reads (test_windows).
The engine-function library (engine_026g.py, the arena's block engine.code;
client_patch.md section 0.5): its files are what engine_026g.tsv and the
re-kit's ABI rows give, and engine_026g.py refuses a spec row whose
parameters are not where the re-kit's roles say (each names a word of its
location's role: two parameters swapped are refused) and a cleanup that is
no number. check_engine_026g.py
(run here, test_engine) proves every thunk against the rows and the original
exe: each argument in its register or slot at the real function's entry, the
exe's `ret` against abi.tsv's cleanup (a wrong cleanup fails), the stack
balanced, EBX ESI EDI EBP kept, the result where C takes it; the config's
look-ups, the field writers, the statistics' listener, the pool lookup and
the allocation's name run for real. Not caught here: a re-kit row whose
location is itself wrong (role and all) for a function not run for real;
Research's other ones (malloc, the rule base's constructor, the message,
Invoke, register_cook) run for real in check_research_026g.py (not run here).
"""

import array
import ast
import bisect
import ctypes
import hashlib
import itertools
import csv
import json
import math
import multiprocessing
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import traceback

import engine_026g
import patch_026g as P

HERE = os.path.dirname(os.path.abspath(__file__))
# What the self-test needs of this machine, each from the environment:
# the original 0.26g survarium.exe (or pass it as the first argument), a
# folder to build into (never the game's), the folder of nasm and ndisasm
# (empty: found on PATH), and optionally an exe built with --cert-only to
# compare with.
ORIGINAL = os.environ.get("SRP_026G_EXE", "survarium.exe")
INSTALLED_CERT_ONLY = os.environ.get("SRP_026G_CERT_ONLY_EXE", "")
SCRATCH = os.environ.get("SRP_026G_SCRATCH", os.path.join(tempfile.gettempdir(), "srp-026g-selftest"))
NASM_DIR = os.environ.get("NASM_DIR", "")


def tool(name):
    return os.path.join(NASM_DIR, name) if NASM_DIR else name

failures = []


def check(ok, text):
    print(("  ok    " if ok else "  FAIL  ") + text)
    if not ok:
        failures.append(text)


def ndisasm(code, va):
    """[(address, hex bytes, text)]"""
    with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as chunk:
        chunk.write(code)
    try:
        out = subprocess.run([tool("ndisasm"), "-b", "32", "-o", str(va), chunk.name],
                             capture_output=True, text=True, check=True, timeout=60).stdout
    finally:
        os.unlink(chunk.name)
    rows = []
    for line in out.splitlines():
        parts = line.split(None, 2)
        if len(parts) == 3 and not parts[0].startswith("-"):
            # ndisasm 3 spells the same 32-bit indirect call with explicit
            # size/distance qualifiers; keep the 2.x spelling for checks.
            text = parts[2].replace("call dword near [", "call [")
            rows.append((int(parts[0], 16), parts[1], text))
    return rows


BRANCH = re.compile(r"^(call|jmp|j[a-z]+|loop[a-z]*)\s+(?:near |short )?(0x[0-9a-f]+)$")


def branch_target(text):
    match = BRANCH.match(text)
    return int(match.group(2), 16) if match else None


class Image:
    def __init__(self, data):
        self.data = data
        self.headers = P.Headers(data)
        self.sections = self.headers.sections(data)

    def offset(self, va):
        rva = va - P.IMAGE_BASE
        for _, srva, vsize, raw, rsize, _ in self.sections:
            if srva <= rva < srva + rsize:
                return rva - srva + raw
        raise ValueError(f"{va:#x} is in no section's file data")

    def read(self, va, size):
        at = self.offset(va)
        return bytes(self.data[at:at + size])

    def directory(self, index):
        return struct.unpack_from("<II", self.data, self.headers.optional + 96 + 8 * index)

    def relocations(self):
        """{file offset of the entry: (type, va)}"""
        rva, size = self.directory(5)
        o = self.offset(P.IMAGE_BASE + rva)
        end, out = o + size, {}
        while o < end:
            page, block = struct.unpack_from("<II", self.data, o)
            if block < 8:
                break
            for i in range((block - 8) // 2):
                e = struct.unpack_from("<H", self.data, o + 8 + 2 * i)[0]
                out[o + 8 + 2 * i] = (e >> 12, P.IMAGE_BASE + page + (e & 0xFFF))
            o += block
        return out


def test_bins():
    print("assembled code")
    for mode, cave in P.CAVES.items():
        for stem in ("snapshot_upload_026g", "default_server_026g"):
            path = os.path.join(HERE, f"{stem}.{mode}.bin")
            with tempfile.TemporaryDirectory() as tmp:
                out = os.path.join(tmp, "out.bin")
                subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={cave:#x}",
                                os.path.join(HERE, stem + ".asm"), "-o", out],
                               check=True, timeout=60)
                fresh = open(out, "rb").read()
            check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.SEEK_CLEAR_VA:#x}",
                        os.path.join(HERE, "replay_seek_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.seek_clear_bin(), "rb").read(),
          f"{os.path.basename(P.seek_clear_bin())} is current ({len(fresh)} bytes)")
    path = P.stale_dll_bin()
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.STALE_DLL_ENTRY:#x}",
                        os.path.join(HERE, "stale_dll_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    path = P.forward_scroll_bin()
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.FORWARD_SCROLL_VA:#x}",
                        os.path.join(HERE, "forward_scroll_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    path = P.mount_patches_bin()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.MOUNT_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.MOUNT_PATCHES_VA:#x}", f"-DSTATE={state:#x}",
                        os.path.join(HERE, "mount_patches_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.KIT_CHANGE_VA:#x}",
                        os.path.join(HERE, "kit_change_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.kit_change_bin(), "rb").read(),
          f"{os.path.basename(P.kit_change_bin())} is current ({len(fresh)} bytes)")
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WARMUP_LOOK_STATE_OFFSET
    for source, defines, path in (
            ("warmup_look_026g.asm", [f"-DCAVE={P.WARMUP_LOOK_VA:#x}", f"-DSTATE={state:#x}",
                                      f"-DLIMITS={P.WARMUP_LIMITS_VA:#x}"], P.warmup_look_bin()),
            ("warmup_limits_026g.asm", [f"-DCAVE={P.WARMUP_LIMITS_VA:#x}"], P.warmup_limits_bin()),
            ("warmup_keys_026g.asm", [f"-DCAVE={P.WARMUP_KEYS_VA:#x}",
                                      f"-DSTATE={state - P.WARMUP_LOOK_STATE_OFFSET + P.WARMUP_KEYS_STATE_OFFSET:#x}"],
             P.warmup_keys_bin())):
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "out.bin")
            subprocess.run([tool("nasm"), "-f", "bin", *defines, os.path.join(HERE, source), "-o", out],
                           check=True, timeout=60)
            fresh = open(out, "rb").read()
        check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    late_join = open(P.late_join_bin(), "rb").read()
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.death_camera_va(late_join):#x}",
                        f"-DSTATE={P.late_join_state_va(late_join) + P.DEATH_CAMERA_STATE_OFFSET:#x}",
                        f"-DLOOK={P.WARMUP_LOOK_VA:#x}", f"-DLOOK_STATE={state:#x}",
                        f"-DDRAG_END={P.WARMUP_LIMITS_VA + P.WARMUP_DRAG_END_OFFSET:#x}",
                        os.path.join(HERE, "death_camera_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.death_camera_bin(), "rb").read(),
          f"{os.path.basename(P.death_camera_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.SPOT_VA:#x}",
                        os.path.join(HERE, "spot_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.spot_bin(), "rb").read(),
          f"{os.path.basename(P.spot_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.REPLAY_LINK_VA:#x}",
                        os.path.join(HERE, "replay_link_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.replay_link_bin(), "rb").read(),
          f"{os.path.basename(P.replay_link_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.SHARED_UPGRADES_VA:#x}",
                        os.path.join(HERE, "shared_upgrades_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.shared_upgrades_bin(), "rb").read(),
          f"{os.path.basename(P.shared_upgrades_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", os.path.join(HERE, "read_order_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.read_order_bin(), "rb").read(),
          f"{os.path.basename(P.read_order_bin())} is current ({len(fresh)} bytes)")
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_PREVIEW_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        stash = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_STASH_OFFSET
        table = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_OFFSET
        upgrades = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.UPGRADE_PREVIEW_OFFSET
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.WEAPON_PREVIEW_VA:#x}", f"-DSTATE={state:#x}",
                        f"-DFIND={upgrades + P.UPGRADE_PREVIEW_FIND:#x}", f"-DCAMERA={table:#x}",
                        os.path.join(HERE, "weapon_preview_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_preview_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_preview_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={stash:#x}", f"-DSTATE={state:#x}",
                        f"-DPREVIEW={P.WEAPON_PREVIEW_VA:#x}", os.path.join(HERE, "weapon_stash_026g.asm"),
                        "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_stash_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_stash_bin())} is current ({len(fresh)} bytes)")
    table_state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={table:#x}", f"-DSTATE={table_state:#x}",
                        f"-DPREVIEW={state:#x}", os.path.join(HERE, "weapon_table_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_table_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_table_bin())} is current ({len(fresh)} bytes)")
    camo_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read())
    decals = camo_va + P.WEAPON_DECALS_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={camo_va:#x}", f"-DDECALS={decals:#x}",
                        os.path.join(HERE, "weapon_camo_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_camo_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_camo_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        press = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_DECALS_STATE_OFFSET
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={decals:#x}", f"-DPREVIEW={state:#x}",
                        f"-DTABLE={table_state:#x}", f"-DPRESS={press:#x}",
                        os.path.join(HERE, "weapon_decals_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_decals_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_decals_bin())} is current ({len(fresh)} bytes)")
    upgrade_state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.UPGRADE_PREVIEW_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={upgrades:#x}", f"-DSTATE={upgrade_state:#x}",
                        f"-DPREVIEW={state:#x}", f"-DFIND={stash + P.WEAPON_STASH_FIND:#x}",
                        os.path.join(HERE, "upgrade_preview_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.upgrade_preview_bin(), "rb").read(),
          f"{os.path.basename(P.upgrade_preview_bin())} is current ({len(fresh)} bytes)")
    import engine_026g
    try:
        stale = engine_026g.stale()
    except engine_026g.Refused as error:
        stale = [f"refused: {error}"]
    check(not stale, "engine_026g.h, .inc, .asm and .rs are what engine_026g.tsv and the re-kit's ABI rows give "
          "(engine_026g.py --write)" + (": not " + ", ".join(stale) if stale else ""))
    import hooks_026g
    try:
        hook_stale = hooks_026g.stale()
        hooks_026g.check_layers()
        layers = None
    except hooks_026g.Bad as error:
        hook_stale, layers = [], str(error)
    check(not hook_stale and layers is None,
          "hooks_026g.rs and hook_entries_026g.asm are what the patches' TOML gives (hooks_026g.py --write), and "
          "the Rust patches keep the raw/vostok/patches layers (no address or hand-made binding outside raw, each "
          "patch module forbids unsafe and names only vostok)"
          + (": stale " + ", ".join(hook_stale) if hook_stale else "") + (f": {layers}" if layers else ""))
    # The arena (Research, the artefacts, the engine library) is linked from
    # its sources here, as by every build, and is the lock's.
    linked = P.arena()
    check(P.linker.lock_differs(linked, P.linker.read_lock()) is None,
          f"the arena linked from its sources is {P.linker.LOCK.name}'s ({linked.size:#x} bytes, SHA-256 "
          f"{linked.sha256[:16]}..., blocks {', '.join(f'{b.name} {b.va:#x}' for b in linked.blocks.values())}; "
          "link_026g.py --write)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.WEAPON_DRAW_VA:#x}", f"-DSTATE={state:#x}",
                        os.path.join(HERE, "weapon_draw_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_draw_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_draw_bin())} is current ({len(fresh)} bytes)")


def test_pe(original, image, mode):
    headers, data = image.headers, image.data
    sections = image.sections
    count = len(sections)
    check(count == (8 if mode == "section" else 7), f"{count} sections")
    check(headers.table + 40 * count <= headers.size_of_headers, "the section table fits in the headers")
    previous_end_rva, previous_end_raw = headers.size_of_headers, headers.size_of_headers
    ordered = True
    for name, rva, vsize, raw, rsize, _ in sections:
        ordered &= raw % headers.file_alignment == 0 and rsize % headers.file_alignment == 0
        ordered &= raw == previous_end_raw
        ordered &= rva == (previous_end_rva + headers.section_alignment - 1) // headers.section_alignment \
            * headers.section_alignment
        previous_end_rva, previous_end_raw = rva + vsize, raw + rsize
    check(ordered, "sections are aligned, contiguous in memory and in the file")
    size_of_image = struct.unpack_from("<I", data, headers.size_of_image_at)[0]
    expected = (previous_end_rva + headers.section_alignment - 1) // headers.section_alignment * headers.section_alignment
    check(size_of_image == expected, f"SizeOfImage {size_of_image:#x} = end of the last section")
    entry = struct.unpack_from("<I", data, headers.optional + 16)[0]
    check(entry == struct.unpack_from("<I", original, headers.optional + 16)[0], "entry point unchanged")

    security_offset, security_size = image.directory(4)
    check(security_offset == previous_end_raw and security_offset + security_size == len(data)
          and security_offset % 8 == 0,
          f"certificate table at {security_offset:#x}, size {security_size:#x}: right after the last section, "
          "8-byte aligned, last in the file")
    old_offset, old_size = struct.unpack_from("<II", original, headers.security_at)
    check(bytes(data[security_offset:]) == bytes(original[old_offset:old_offset + old_size]),
          "certificate table bytes identical to the original")
    length, revision, kind = struct.unpack_from("<IHH", data, security_offset)
    check(length <= security_size and revision == 0x200 and kind == 2,
          f"WIN_CERTIFICATE length {length:#x}, revision {revision:#x}, type {kind} (PKCS#7 signed data)")
    inside = True
    for index in range(16):
        if index == 4:
            continue
        rva, size = image.directory(index)
        if size:
            inside &= any(srva <= rva and rva + size <= srva + max(vsize, rsize)
                          for _, srva, vsize, _, rsize, _ in sections)
        inside &= (rva, size) == struct.unpack_from("<II", original, headers.optional + 96 + 8 * index)
    check(inside, "the other 15 data directories are unchanged and inside sections")
    if mode == "section":
        name, rva, vsize, raw, rsize, characteristics = sections[-1]
        check(name == P.SECTION_NAME and rva == P.SECTION_RVA and characteristics == 0x60000020,
              f".srp at RVA {rva:#x} (VA {P.IMAGE_BASE + rva:#x}), virtual {vsize:#x}, file {raw:#x}+{rsize:#x}, "
              "code | execute | read")
    characteristics = struct.unpack_from("<H", data, headers.pe + 22)[0]
    dll = struct.unpack_from("<H", data, headers.optional + 70)[0]
    nx = 0 if mode == "cert-only" else P.IMAGE_DLLCHARACTERISTICS_NX_COMPAT
    check(characteristics == 0x122 and dll == 0x8000 | nx,
          f"file characteristics 0x122, DLL characteristics {0x8000 | nx:#x} (the original's 0x8000, no ASLR "
          "flag; NX_COMPAT except in cert-only)")


def test_code(original, image, mode, cave):
    """The snapshot upload's cave: it disassembles to its last `ret`, every
    branch lands on one of its instructions or a known target, it holds no
    absolute operand, the hooks jump to its entries (each pushad), and it
    replays the instruction the request hook replaced."""
    cave_va = P.CAVES[mode]
    check(image.read(cave_va, len(cave)) == cave, f"the cave is mapped at {cave_va:#x}")
    rows = ndisasm(image.read(cave_va, len(cave)), cave_va)
    starts = {address for address, _, _ in rows}
    check(rows[-1][2] == "ret" and rows[-1][0] + 1 == cave_va + len(cave), "the cave disassembles cleanly to its last ret")
    bad = [f"{address:#x}: {text} (not an instruction start)" for address, _, text in rows
           if (target := branch_target(text)) is not None and cave_va <= target < cave_va + len(cave) and target not in starts]
    bad += [f"{t:#x}" for t in unknown_targets(original, image.data, rows)]
    bad += [f"{address:#x}: {text} (absolute address operand)" for address, _, text in rows
            if branch_target(text) is None and not text.startswith(("call", "jmp")) and re.search(r"\[(dword )?0x", text)]
    check(not bad, "every branch in the cave goes to a known address, no operand is absolute"
          + "".join("\n          " + b for b in bad))
    entries = []
    for va, old in P.HOOKS:
        hook = ndisasm(image.read(va, len(old)), va)
        entries.append(branch_target(hook[0][2]))
        check(hook[0][2].startswith("jmp") and entries[-1] in starts, f"hook {va:#x}: {hook[0][2]} to an instruction")
    text_at = {address: text for address, _, text in rows}
    check(all(text_at[e] == "pusha" for e in entries), "both cave entries start with pushad")
    check("mov ebx,[ebx+0x1f1c48]" in text_at.values(), "the cave replays the replaced `mov ebx,[ebx+1F1C48h]`")


def test_default_server(original, image, mode, server):
    """The embedded default login server: the missing-server error jumps to
    the stub, which keeps every register it uses and goes on where the
    original reads the option; the address is in the build, terminated."""
    va, old = P.DEFAULT_SERVER_HOOK
    if server is None:
        return
    start = P.CAVES[mode] + P.DEFAULT_SERVER_OFFSET
    stub = open(P.default_server_bin(mode), "rb").read()
    hook = ndisasm(image.read(va, len(old)), va)
    check(hook[0][2].startswith("jmp ") and branch_target(hook[0][2]) == start,
          "missing-server error jumps to the embedded-address stub")
    texts = [text for _, _, text in ndisasm(image.read(start, len(stub)), start)]
    check(texts[:4] == ["push ecx", "push edx", "push esi", "push edi"]
          and texts[-5:] == ["pop edi", "pop esi", "pop edx", "pop ecx", f"jmp {P.DEFAULT_SERVER_RETURN:#x}"],
          "the stub keeps the four registers it uses and goes on at the original option read")
    encoded = P.parse_server(server).encode("ascii") + b"\0"
    check(image.read(start + len(stub), len(encoded)) == encoded,
          "embedded host:port is terminated and fits the client buffers")


def test_server_options(original):
    import make_client_026g as overlay

    for value in ("026g.skhid.link:1234", "localhost:1", "a" * 63 + ":65535"):
        check(P.parse_server(value) == value, f"accepts build-time server {value}")
    for value in ("host", "host:0", "host:65536", "host:-1", "host:１２３４",
                  "a" * 64 + ":1234", "host:1234\0", "host:1234\n", "host;other:1234"):
        try:
            P.parse_server(value)
        except SystemExit:
            pass
        else:
            check(False, f"invalid address accepted: {value!r}")
    try:
        P.build(original, "cert-only", server="localhost:1234")
    except SystemExit:
        check(True, "cert-only refuses an embedded server")
    else:
        check(False, "cert-only must refuse an embedded server")
    for embedded in (False, True):
        line = overlay.launch_line("localhost:1234", embedded)
        batch = overlay.bat("localhost:1234", embedded).decode("ascii")
        start = next(line for line in batch.splitlines() if line.startswith('start "" '))
        check(("-server=" in line) != embedded and ("-server=" in start) != embedded
              and "-localization" not in line and "-localization" not in start,
              f"overlay launch line and batch use {'embedded' if embedded else 'explicit'} server, no language")


# sign_in_on_connected: from the copy of the version string to the write of
# the byte after the field (client_patch.md section 6).
VERSION_FIELD = (0x380847, 0x38088F)
REGISTERS = {"eax": ("eax", 4), "ax": ("eax", 2), "al": ("eax", 1),
             "ecx": ("ecx", 4), "cx": ("ecx", 2), "cl": ("ecx", 1),
             "edx": ("edx", 4), "dx": ("edx", 2), "dl": ("edx", 1)}


def version_field(image):
    """The 13 bytes sign_in_on_connected writes at edi after the user name
    (the 12-byte version field and the byte after it), worked out from its
    instructions. A byte is an int, "eula" (the flag read from [ebp+10h]) or
    None (not known). Returns None and says why when an instruction is not
    one of the forms the original has."""
    start, end = VERSION_FIELD
    registers = {"eax": [None] * 4, "ecx": [None] * 4, "edx": [None] * 4}
    xmm0, stack, out = [None] * 8, {}, [None] * 13

    def get(name):
        register, size = REGISTERS[name]
        return registers[register][:size]

    def put(name, values):
        register, size = REGISTERS[name]
        assert len(values) == size
        registers[register][:size] = values

    absolute = re.compile(r"^mov (\w+),\[(?:dword )?0x([0-9a-f]+)\]$")
    to_stack = re.compile(r"^mov \[esp\+0x([0-9a-f]+)\],(\w+)$")
    immediate = re.compile(r"^mov byte \[esp\+0x([0-9a-f]+)\],0x([0-9a-f]+)$")
    to_field = re.compile(r"^mov \[edi\+0x([0-9a-f]+)\],(\w+)$")
    for address, _, text in ndisasm(image.read(start, end - start), start):
        if m := absolute.match(text):
            size = REGISTERS[m.group(1)][1]
            put(m.group(1), list(image.read(int(m.group(2), 16), size)))
        elif text == "pxor xmm0,xmm0":
            xmm0 = [0] * 8
        elif m := re.match(r"^movq \[esp\+0x([0-9a-f]+)\],xmm0$", text):
            for i, value in enumerate(xmm0):
                stack[int(m.group(1), 16) + i] = value
        elif m := re.match(r"^movq xmm0,\[esp\+0x([0-9a-f]+)\]$", text):
            xmm0 = [stack.get(int(m.group(1), 16) + i) for i in range(8)]
        elif m := to_stack.match(text):
            for i, value in enumerate(get(m.group(2))):
                stack[int(m.group(1), 16) + i] = value
        elif m := immediate.match(text):
            stack[int(m.group(1), 16)] = int(m.group(2), 16)
        elif text == "xor eax,eax":
            put("eax", [0] * 4)
        elif text == "mov ecx,[ebp+0xc]":
            put("ecx", [None] * 4)
        elif text == "mov dl,[ebp+0x10]":
            put("dl", ["eula"])
        elif text == "movq [edi],xmm0":
            out[:8] = xmm0
        elif m := to_field.match(text):
            for i, value in enumerate(get(m.group(2))):
                out[int(m.group(1), 16) + i] = value
        elif text == f"call {P.STALE_DLL_ENTRY:#x}":
            # stale_dll_026g.asm: what `mov [edi+8],eax` / `mov [edi+0Ch],dl`
            # did, with its flags in byte 8 (check 26).
            out[8:13] = ["stale_dll", 0, 0, 0] + get("dl")
        elif text not in ("nop", "add edi,esi"):
            check(False, f"{address:#x}: `{text}` is not an instruction of the version field's code")
            return None
    return out


def test_detail(original, image):
    """Native detail hooks, option/label counts, relocations and entrances."""
    stock = Image(original)
    symbols = P.arena().symbols
    for va, old, symbol, opcode in P.DETAIL_HOOKS:
        wanted = bytes([opcode]) + struct.pack('<i', symbols[symbol] - va - 5) + b'\x90' * (len(old) - 5)
        check(stock.read(va, len(old)) == old and image.read(va, len(old)) == wanted,
              f"detail {symbol}: original bytes and symbolic jump at {va:#x}")
        check(not [rva for kind, rva in stock.relocations().values()
                   if kind == 3 and rva + 4 > va and rva < va + len(old)],
              f"detail {symbol}: no relocation under hook")
    for va, old, new in P.DETAIL_PATCHES:
        check(stock.read(va, len(old)) == old and image.read(va, len(new)) == new,
              f"detail: option/label count extended to twenty at {va:#x}")


def test_revision(original, image, mode, revision, late_join=False):
    """The patch revision in the sign-in (client_patch.md section 6), worked
    out from the instructions that fill the version field. late_join: the
    default build, whose sign-in also has the shell_32.dll check."""
    field = version_field(image)
    if mode == "cert-only":
        check(field == list(b"0.26g0" + bytes(6)) + ["eula"], "cert-only: the version field is the original's")
        return
    _, _, new = P.revision_site(revision)
    check(1 <= revision <= 255 and new == bytes([0xC6, 0x44, 0x24, 0x27, revision, 0x90]),
          f"the revision is {revision}: a byte, not 0")
    stale_dll = late_join                       # the build with the late join (from revision 12) has it
    expected = list(b"0.26g0" + bytes([0, revision])) + (["stale_dll"] if stale_dll else [0]) + [0, 0, 0, "eula"]
    check(field == expected,
          f'the version field is "0.26g0", a zero, the revision {revision} in byte 7, '
          + ("the shell_32.dll check's flags in byte 8 and three zero bytes" if stale_dll else "four zero bytes")
          + "; the EULA flag follows as before")


def test_late_join_bin():
    print("late-join code (late_join_026g.section.bin)")
    code = open(P.late_join_bin(), "rb").read()
    kept = json.load(open(os.path.splitext(P.late_join_bin())[0] + ".json", encoding="utf-8"))
    check(kept["size"] == len(code) and kept["sha256"] == hashlib.sha256(code).hexdigest(),
          f"the .bin is the one its .json describes ({len(code)} bytes, built with {kept['built_with']['cc']})")
    names = {"LJ_COMPLETE": "lj_complete", "LJ_PREPARE": "lj_prepare", "LJ_ACTIVATE": "lj_activate",
             "LJ_DEACTIVATE": "lj_deactivate", "LJ_DESTROYED": "lj_player_destroyed",
             "LJ_INSTALL": "lj_native_install", "LJ_ANIMATIONS": "lj_native_animations"}
    c_code = P.LATE_JOIN_VA + P.LATE_JOIN_ASM_SIZE
    defines = [f"-DCAVE={P.LATE_JOIN_VA}", f"-DC_CODE={c_code}"]
    defines += [f"-D{define}={c_code + kept['symbols'][name]}" for define, name in names.items()]
    defines += [f"-DSTATE_VA={P.late_join_state_va(code)}", f"-DUPLOAD_READY={P.SECTION_VA}"]
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", *defines, os.path.join(HERE, "late_join_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(len(fresh) == P.LATE_JOIN_ASM_SIZE and fresh == code[:P.LATE_JOIN_ASM_SIZE],
          f"its first {P.LATE_JOIN_ASM_SIZE} bytes are what nasm makes of late_join_026g.asm today")
    check(all(0 < offset < len(code) - P.LATE_JOIN_ASM_SIZE for offset in kept["symbols"].values()),
          "the C functions the assembly calls are inside the C code")
    entries = sorted({offset for _, _, offset, _ in P.LATE_JOIN_HOOKS + P.TAB_LIST_HOOKS + P.FOV_LOD_HOOKS}
                     | {P.LATE_JOIN_READY_OFFSET})
    check(all(code[offset] != 0xCC and (offset == 0 or code[offset - 1] == 0xCC) for offset in entries),
          f"the {len(entries)} entries the hooks go to start code after int3 padding")
    return code


def instruction(image, va):
    """(length, text) of the instruction at `va` (ndisasm, a window decoded
    at a time and cached on the image), or None."""
    cache = image.__dict__.setdefault("instructions", {})
    if va not in cache:
        try:
            window = image.read(va, 0x200)
        except ValueError:
            return None
        rows = ndisasm(window, va)
        # an instruction's length is the next one's distance: ndisasm prints
        # the bytes of a long one (10 or more) on a second, `-` line, which
        # ndisasm() leaves out
        for (at, code_hex, text), following in zip(rows, rows[1:] + [None]):
            if following is None or following[0] > va + len(window) - 16:
                break
            cache.setdefault(at, (following[0] - at, text))
        if va not in cache and rows:
            cache[va] = (len(rows[0][1]) // 2, rows[0][2])
    return cache.get(va)


def lohook_returns(image, entry):
    """A generated lohook's final ret dispatches a saved continuation, not
    the enclosing function's return address. Follow every literal Flow target
    from the built bytes, including replay and named exits. Ordinary generated
    callable entries have a different restore sequence and still return normally.
    """
    rows = ndisasm(image.read(entry, 0x100), entry)
    texts = [text for _, _, text in rows]
    if texts[:5] != ["lea esp,[esp-0x4]", "pushf", "pusha", "cld", "mov ebp,esp"]:
        return None
    end = next((i for i, text in enumerate(texts) if text.startswith("ret")), None)
    if end is None or texts[max(0, end-5):end] != [
            "mov [ebp+0x24],edx", "fxrstor [edi]", "mov esp,ebp", "popa", "popf"]:
        return None
    targets = [int(text.removeprefix("mov edx,"), 16) for text in texts[:end]
               if re.fullmatch(r"mov edx,0x[0-9a-f]+", text)]
    if texts[end] != "ret" or len(targets) < 2 or "ud2" not in texts[:end]:
        return (), f"{entry:#x}: malformed generated continuation dispatch"
    return targets, None


def function_pops(image, entry, limit=20000):
    """The bytes the function at `entry` pops: every `ret n` reachable from
    its entry, its branches followed (a body split by the compiler, a tail
    jump into the game too; a jump to a generated lohook entry goes on where
    the entry can send it, lohook_dispatch), its calls stepped over.
    ({n, ...}, [what could not be followed])."""
    todo, seen, found, lost = [entry], set(), set(), []
    while todo and len(seen) < limit:
        va = todo.pop()
        while va not in seen:
            seen.add(va)
            got = instruction(image, va)
            if got is None:
                lost.append(f"{va:#x}: outside the image")
                break
            length, text = got
            if text == "lea esp,[esp-0x4]":
                dispatch = lohook_returns(image, va)
                if dispatch is not None:
                    targets, error = dispatch
                    todo.extend(targets)
                    if error is not None:
                        lost.append(error)
                    break
            words = text.split()
            while words and words[0] in ("rep", "repe", "repz", "repne", "repnz", "bnd"):
                words = words[1:]                          # `rep ret`, `bnd ret`: a ret
            op = words[0] if words else ""
            if op in ("ret", "retn"):
                found.add(int(words[1], 16) if len(words) > 1 else 0)
                break
            if op in ("int3", "ud2", "hlt"):
                break
            target = branch_target(text)
            if op == "jmp":
                if target is None:
                    lost.append(f"{va:#x}: {text}")
                    break
                ways = lohook_dispatch(image, target)
                if ways is not None:                       # a generated lohook entry: its `ret` is a jump
                    todo.extend(ways)
                    break
                va = target
                continue
            if (op.startswith("j") or op.startswith("loop")) and target is not None:
                todo.append(target)
            va += length
    if todo or len(seen) >= limit:
        lost.append(f"{entry:#x}: more than {limit} instructions, not all followed")
    return found, lost


def lohook_dispatch(image, va):
    """Where a generated lohook entry at `va` (hooks_026g.lohook_entry) goes
    on, or None for any other code. Its `lea esp,[esp-4]` makes a slot that
    `mov [ebp+24h],edx` fills with one of its `mov edx,<address>` (the
    replay, past the site, a named exit), and its last `ret` pops that slot
    alone: a jump that leaves esp as the site had it, not a return of the
    function the site is in (patch 54's artefact_base::action through the
    medical skills' lohook at 0x313333)."""
    prologue = ["lea esp,[esp-0x4]", "pushf", "pusha", "cld", "mov ebp,esp"]
    epilogue = ["mov [ebp+0x24],edx", "fxrstor [edi]", "mov esp,ebp", "popa", "popf", "ret"]
    texts, at = [], va
    while len(texts) < 64 and (not texts or texts[-1] != "ret"):
        got = instruction(image, at)
        if got is None:
            return None
        texts.append(got[1])
        at += got[0]
    if texts[:5] != prologue or texts[-6:] != epilogue:
        return None
    ways = [int(t.split(",")[1], 16) for t in texts if re.fullmatch(r"mov edx,0x[0-9a-f]+", t)]
    return ways or None


def popped(image, start, end):
    """The bytes the function at `start` pops: its first `ret`'s immediate
    (None without a `ret` before `end`)."""
    for _, _, text in ndisasm(image.read(start, end - start), start):
        if text == "ret":
            return 0
        if text.startswith("ret "):
            return int(text.split()[1], 16)
    return None


def starts_instruction(source, at):
    """Whether `at` starts an instruction of the function around it, read
    from the function's start (after the int3 before it)."""
    start = at
    while source.read(start - 1, 1) != b"\xcc" or start % 16:
        start -= 1
    return at in {address for address, _, _ in ndisasm(source.read(start, at - start + 16), start)}


# The general rules, on every build: what it changed is what build()
# recorded (patch_026g.py APPLIED, each write its named instance's), so a
# patch that declares its writes is checked without a line here.
_BRANCHES = {}


def branches_of(original):
    """Every way the original aims at an address, each a key target << 32 |
    source, sorted: (branches, values) of each rel32 call/jmp/jcc and rel8
    jmp/jcc/loop of .text, read by its bytes (aimed_into decodes a hit),
    and of each value a base relocation names (a pointer, a jump table's
    entry). Read once."""
    key = hashlib.sha256(original).hexdigest()
    if key not in _BRANCHES:
        text = bytes(original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE])
        branches, values = [], []
        for pattern, size in ((rb"[\xE8\xE9]", 5), (rb"\x0F[\x80-\x8F]", 6), (rb"[\x70-\x7F\xE0-\xE3\xEB]", 2)):
            unpack = "<b" if size == 2 else "<i"
            for m in re.finditer(pattern, text):
                at = m.start()
                if at + size <= len(text):
                    target = P.TEXT_VA + at + size + struct.unpack_from(unpack, text, at + size - (1 if size == 2 else 4))[0]
                    branches.append((target & 0xFFFFFFFF) << 32 | P.TEXT_VA + at)
        image = Image(original)
        for kind, va in image.relocations().values():
            if kind == 3:
                values.append(struct.unpack("<I", image.read(va, 4))[0] << 32 | va)
        _BRANCHES[key] = (array.array("Q", sorted(branches)), array.array("Q", sorted(values)))
    return _BRANCHES[key]


def aimed_into(original, lo, hi, own=None):
    """What of the original aims strictly inside (lo, hi): base-relocated
    values, and branches that are instructions of their function; sources
    inside `own` ((lo, hi), the write itself) left out."""
    stock = _ORIGINALS.setdefault(hashlib.sha256(original).hexdigest(), Image(original))
    hits = []
    for keys, decode in zip(branches_of(original), (True, False)):
        for k in range(bisect.bisect_left(keys, (lo + 1) << 32), bisect.bisect_left(keys, hi << 32)):
            source = keys[k] & 0xFFFFFFFF
            if not (own and own[0] <= source < own[1]) and (not decode or starts_instruction(stock, source)):
                hits.append(source)
    return hits


def changed_runs(original, write):
    """[(lo, hi)] VA runs of the bytes one APPLIED write changed."""
    old = original[write.at:write.at + len(write.new)]
    runs, start = [], None
    for i in range(len(write.new) + 1):
        differs = i < len(write.new) and write.new[i] != old[i]
        if differs and start is None:
            start = i
        elif not differs and start is not None:
            runs.append((write.va + start, write.va + i))
            start = None
    return runs


def bookkeeping(headers):
    """The file offsets of the PE header's counters the section adders keep
    (not a patch's write): the count, SizeOfCode, SizeOfInitializedData, SizeOfUninitializedData,
    SizeOfImage, the checksum, the certificate directory, and DllCharacteristics
    (its NX_COMPAT bit, set by every full build)."""
    out = set()
    for at, size in ((headers.count_at, 2), (headers.size_of_code_at, 4), (headers.size_of_initialized_data_at, 4),
                     (headers.optional + 12, 4),
                     (headers.size_of_image_at, 4), (headers.checksum_at, 4), (headers.security_at, 8),
                     (headers.dll_characteristics_at, 2)):
        out.update(range(at, at + size))
    return out


# Every VA a build wrote (main's end: every install table's row is one).
WRITTEN_SEEN = set()


def test_writes(original, data, applied, what):
    """The general rules on a build `data` and its record `applied`
    (P.APPLIED right after P.build): every write is a named instance's and
    none overlaps another; replaying them onto the original gives the build's
    bytes (outside the header's counters and the added sections' headers),
    so no byte changed undeclared; a base relocation is turned off only under
    a changed byte, and one left on under a changed byte names an address of
    the image; nothing of the original aims inside the bytes a write changed
    in .text; a call into our code returns onto nops to the end of its
    write."""
    image = Image(data)
    headers = P.Headers(original)
    writes = [a for a in applied if a.kind == "write"]
    WRITTEN_SEEN.update(a.va for a in writes)
    spans = sorted((a.at, a.at + len(a.new)) for a in applied)
    check(applied and all(a.owner for a in applied) and all(hi <= lo for (_, hi), (lo, _) in zip(spans, spans[1:])),
          f"{what}: every one of its {len(writes)} writes, {len(applied) - len(writes)} relocations and sections is "
          f"a named instance's ({len({a.owner for a in applied})}; patch_026g.py --installs), none over another's")
    end = max(raw + rsize for _, _, _, raw, rsize, _ in headers.sections(original))
    replay = bytearray(original)
    for a in applied:
        if a.kind in ("write", "relocation"):
            replay[a.at:a.at + len(a.new)] = a.new
    skip = bookkeeping(headers)
    for a in applied:
        if a.kind == "section header":
            skip.update(range(a.at, a.at + 40))
    differ = [o for start in range(0, end, 1 << 16) if replay[start:start + (1 << 16)] != data[start:start + (1 << 16)]
              for o in range(start, min(start + (1 << 16), end)) if o not in skip and replay[o] != data[o]]
    check(not differ, f"{what}: the original with the recorded writes replayed is the build's first {end:#x} "
          "bytes (no byte changed but by a declared write)"
          + (f": {len(differ)} differ, the first at file {differ[0]:#x}" if differ else ""))
    runs = [(run, (a.va, a.va + len(a.new))) for a in writes for run in changed_runs(original, a)]
    under = lambda va: [own for (lo, hi), own in runs if va < hi and va + 4 > lo]
    old, new = Image(original).relocations(), image.relocations()
    turned_off = {a.at for a in applied if a.kind == "relocation"}
    top = P.IMAGE_BASE + struct.unpack_from("<I", data, headers.size_of_image_at)[0]
    bad = [f"entry at file {at:#x} ({entry[1]:#x}) changed undeclared" for at, entry in old.items()
           if new.get(at) != entry and not (at in turned_off and entry[0] == 3 and new.get(at) == (0, entry[1]))]
    bad += [f"{entry[1]:#x} turned off under no changed byte" for at, entry in old.items()
            if at in turned_off and not under(entry[1])]
    for at, (kind, va) in new.items():
        owners = under(va) if kind == 3 else []
        value = struct.unpack("<I", image.read(va, 4))[0] if owners else None
        if owners and not (len(owners) == 1 and owners[0][0] <= va and va + 4 <= owners[0][1]
                           and P.IMAGE_BASE <= value < top):
            bad.append(f"{va:#x}: on, under a changed byte, naming {value:#x}")
    check(len(new) == len(old) and not bad,
          f"{what}: {len(turned_off)} base relocations turned off, each under a changed byte; those left on under "
          "one name an address of the image; the rest of the table the original's" + "".join(f"; {b}" for b in bad[:6]))
    text_end = P.TEXT_VA + P.TEXT_VSIZE
    hits = [f"{h:#x} -> ({lo:#x}, {hi:#x})" for (lo, hi), own in runs if P.TEXT_VA <= lo < text_end
            for h in aimed_into(original, lo, hi, own)]
    check(not hits, f"{what}: nothing of the original (a branch, a relocated value) aims inside the bytes a write "
          "changed in .text" + "".join(f"; {h}" for h in hits[:8]))
    ours = P.our_ranges(data, P.WRITES)
    calls = [(va, target) for va, (how, target, _) in P.installs_of(data, P.WRITES).items() if how == "call"]
    loose = []
    for va, target in calls:
        write = next(a for a in writes if a.va <= va < a.va + len(a.new))
        tail = write.new[va + 5 - write.va:]
        if any(lo <= target < hi for lo, hi in ours) and set(tail) - {0x90}:
            loose.append(f"{va:#x}: {tail.hex()}")
    check(not loose, f"{what}: each of its {len(calls)} calls into our code returns onto nops to the end of its write"
          + "".join(f"; {x}" for x in loose))


REKIT = os.path.join(HERE, "..", "..", "tools", "re-kit", "v026g")


_KNOWN = {}


def known_targets(original):
    """Where our code may branch to outside itself, from what declares it:
    the game's functions the re-kit names (functions.tsv, inferred.tsv,
    abi.tsv) and the engine-function library's (engine_026g.tsv); each
    install row's way back, the end of the bytes it replaces, and the
    branches of those bytes, which its code replays (patch_026g.py's install
    tables, the Rust hooks, those installed on demand), and its start, where
    the build's own bytes go on;
    the sites' named exits (sites_026g.tsv); and
    patch_026g.py's named ways back (*_BACKS, *_EXITS, *_RETURN, *_END)."""
    key = hashlib.sha256(original).hexdigest()
    if key in _KNOWN:
        return _KNOWN[key]
    known = set()
    for name in ("functions.tsv", "inferred.tsv", "abi.tsv"):
        with open(os.path.join(REKIT, name), newline="") as f:
            known.update(int(row["va"], 16) for row in csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE))
    known.update(f["va"] for f in engine_026g.every_function())
    rows = [row for name, value in vars(P).items() if name.isupper() for row in P._install_rows(value)]
    rows += [(va, old) for va, old, _, _ in P.rust_hooks(original)]
    # a hook installed on demand (hooks_026g.py) goes back past its site as a built one does
    rows += [(h["site"], h["original"]) for h in P.hooks_026g.hooks(exe=P.hooks_026g.original_reader(original))
             if h.get("on_demand")]
    for va, old, *_ in rows:
        known.update((va, va + len(old)))
        known.update(t for _, _, text in ndisasm(bytes(old), va) if (t := branch_target(text)) is not None)
    known.update(addr for site in P.hooks_026g.read_sites().values() for _, addr in site["exits"])
    for name, value in vars(P).items():
        if name.isupper() and re.search(r"(BACKS|EXITS|RETURN|_END)$", name):
            known.update(v for v in (value if isinstance(value, (tuple, list)) else (value,)) if isinstance(v, int))
    _KNOWN[key] = known
    return known


def unknown_targets(original, data, listing):
    """The branch targets of `listing` (our code, ndisasm rows) outside our
    code (patch_026g.py our_ranges) that known_targets does not name."""
    ours = P.our_ranges(data, P.WRITES)
    known = known_targets(original)
    return sorted({t for _, _, text in listing if (t := branch_target(text)) is not None
                   and not any(lo <= t < hi for lo, hi in ours) and t not in known})


def check_piece(original, image, what, va, code, end=None, entries=()):
    """Our code `code` at `va` (its instructions to `end`, its data after):
    it is what the build holds, every branch inside it lands on one of its
    instructions, every branch out of it goes to our code or where
    known_targets says, and each entry (an offset) starts an instruction.
    Returns its listing."""
    end = len(code) if end is None else end
    listing = ndisasm(code[:end], va)
    starts = {a for a, _, _ in listing}
    inner = [t for _, _, text in listing if (t := branch_target(text)) is not None
             and va <= t < va + len(code) and t not in starts]
    unknown = unknown_targets(original, image.data, listing)
    check(image.read(va, len(code)) == code and not inner and not unknown
          and all(va + at in starts for at in entries),
          f"{what} at {va:#x} ({len(code)} bytes): every branch inside it lands on an instruction, every one out "
          "of it is a known target (known_targets), its entries start instructions"
          + "".join(f"; {t:#x}" for t in inner + unknown))
    return listing


def test_rust_hooks(original, image, before):
    """Every Rust hook (hooks_026g.py, from the patches' TOML and
    sites_026g.tsv; a shared site's chain in its TOML `order`): its site
    jumps to (or calls) its generated entry, in the arena's block
    patches.code; a lohook's entry saves the machine and calls its shim."""
    import hooks_026g
    linked = P.arena()
    block = linked.blocks["patches.code"]
    prologue = ["lea esp,[esp-0x4]", "pushf", "pusha", "cld", "mov ebp,esp"]
    bad = []
    installs = P.rust_hooks(original)
    for va, old, symbol, opcode in installs:
        entry = linked.symbols[symbol]
        if image.read(va, len(old)) != P.hook_bytes(entry, va, old, 0, opcode) or not block.va <= entry < block.va + block.size:
            bad.append(f"{va:#x}: not a branch to {symbol}")
    for h in hooks_026g.hooks(exe=hooks_026g.original_reader(original)):
        if h["kind"] == "lohook":
            entry = linked.symbols[h["entry"]]
            texts = [text for _, _, text in ndisasm(image.read(entry, 0x80), entry)]
            if h.get("only"):
                # `only`: the register tested against its bit set, nothing saved, before the prologue
                _, reg, _ = h["only"]
                only = linked.symbols[h["entry"] + "_only"]
                head, texts = texts[:8], texts[8:]
                if (head[0] != "pushf" or head[1] != f"cmp {reg},0x100" or head[3] != f"bt [dword {only:#x}],{reg}"
                        or [head[5], head[7]] != ["popf", "popf"] or not head[6].startswith("jmp ")):
                    bad.append(f"{h['site_name']}: {h['entry']} does not test {reg} against {h['entry']}_only first")
            if texts[:5] != prologue or f"call {linked.symbols[h['shim']]:#x}" not in texts:
                bad.append(f"{h['site_name']}: {h['entry']} is no lohook entry calling {h['shim']}")
    on_demand = [h for h in hooks_026g.hooks(exe=hooks_026g.original_reader(original)) if h.get("on_demand")]
    for h in on_demand:
        entry = linked.symbols[h["entry"]]
        if image.read(h["site"], len(h["original"])) != h["original"] or not block.va <= entry < block.va + block.size:
            bad.append(f"{h['site_name']}: installed on demand, yet its site is not the game's bytes")
    count = len(installs)
    check(count and not bad, f"each of the {count} Rust hooks' sites branches to its generated entry in patches.code; "
          f"the {len(on_demand)} installed on demand keep the game's bytes; "
          "each lohook's entry saves the machine and calls its shim" + "".join(f"; {b}" for b in bad))


def test_rust_code(original, image, before):
    """The Rust crate's code (the arena's block patches.code; its .text, not
    its .rodata): its direct branches out of the block go to the engine
    library's functions, the arena's memcpy, memmove and memset, or a
    known target (a hook's way back past its site or to its named exit, the
    branches of the bytes a hook replays); its stores to absolute addresses
    are in patches.data and .state, or fields fields_026g.tsv gives it (an
    older patch's state in `.srpd`, a flag of the game's a patch sets)."""
    linked = P.arena()
    block = linked.blocks["patches.code"]
    data = [linked.blocks[name] for name in ("patches.data", "patches.state")]
    sym = linked.symbols
    code = P.linker.block_bytes(linked, "patches.code")
    text_size = sym.get(P.linker.rodata_symbol("patches.code"), block.va + len(code)) - block.va
    texts = [text for _, _, text in ndisasm(code[:text_size], block.va)]
    outside = sorted({branch_target(t) for t in texts if branch_target(t) is not None
                      and not block.va <= branch_target(t) < block.va + len(code)})
    library = {sym[f["symbol"]] if f["call"] in engine_026g.THUNKED else f["va"] for f in engine_026g.every_function()
               if f["call"] != "import" and (f["symbol"] in sym or f["call"] not in engine_026g.THUNKED)}
    # arena_rt exports weak intrinsics so multiple Rust arena crates share
    # them. The final ELF's full symbol map includes the selected weak
    # definitions; the public map intentionally contains strong globals only.
    runtime = {va for name, va, _, _ in linked.every if name in ("memcpy", "memmove", "memset")}
    allowed = library | runtime | known_targets(original)
    check(outside and set(outside) <= allowed,
          "the crate's direct branches out of patches.code: the engine library's functions, the arena's memcpy, "
          "memmove and memset, and known targets (the imports go by pointer)"
          + "".join(f"; {t:#x}" for t in sorted(set(outside) - allowed)))
    stores = [t for t in texts if re.match(r"(mov|add|sub|and|or|xor|inc|dec)\w* (byte |word |dword |qword )?\[(dword )?0x", t)]
    absolute = [int(m, 16) for t in stores for m in re.findall(r"\[(?:dword )?(0x[0-9a-f]+)\]", t)]
    # an `::image` row in our own `.srpd` is an older patch's state the crate shares (patch 45's
    # weapon_table: its FLIGHT word, weapon_flight.rs's alone): a field of that row's kind
    srpd = P.late_join_state_va(open(P.late_join_bin(), "rb").read())
    shared = [(x["offset"], x["target"]) for x in engine_026g.fields() if x["module"] == "image"
              and srpd <= x["offset"] < srpd + P.LATE_JOIN_STATE_SIZE]
    shared_fields = {base + x["offset"] for base, kind in shared for x in engine_026g.fields() if x["kind"] == kind}
    # the game's own data a patch sets on purpose, by its image row: patch 81's reset of the eye
    # adaptation (stage_postprocess's byte, which the render thread reads and clears; lobby-scenes.md)
    game_flags = {x["offset"]: f'{x["field"]}' for x in engine_026g.fields() if x["module"] == "image"
                  and x["field"] in ("postprocess",)}
    # patch 70's records of kept files pushed onto the baker's list as its push_back does (0x132640):
    # the list's end and bytes, the records' ages' counter (baked-textures.md)
    for x in engine_026g.fields():
        if x["module"] == "image" and x["field"] in ("baker", "baker_ages"):
            game_flags.update({x["offset"] + f["offset"]: f'{x["field"]}.{f["field"]}' for f in engine_026g.fields()
                               if f["kind"] == x["target"] and f["of"] != "size"})
    inside = lambda a: any(d.va <= a < d.va + d.size for d in data) or a in shared_fields or a in game_flags
    check(all(inside(a) for a in absolute),
          f"its stores to absolute addresses ({len(absolute)}) are all in patches.data and .state, or fields of "
          f"an older patch's state it shares (`.srpd`: {', '.join(f'{b:#x} {k}' for b, k in shared)}), or a flag "
          f"of the game's a patch sets on purpose ({', '.join(f'{a:#x} {n}' for a, n in game_flags.items())}): "
          "nothing else of the game's is written by address" + "".join(f"; {a:#x}" for a in absolute if not inside(a)))
    trace = sym["SRP_DIAGNOSTICS"]
    check(image.read(trace, 4) == P.DIAGNOSTICS_OFF,
          f"the build's diagnostics are off (SRP_DIAGNOSTICS at {trace:#x} \"OFF\": only --ragdoll-trace makes it "
          "1, a local build)")


def test_clock_factor(original, image, before):
    """Check 16: the game's time_floating_factor: the value is inside the
    range the game allows the setting."""
    source = Image(original)
    value = lambda blob: struct.unpack("<f", blob)[0]
    low, high = value(source.read(P.CLOCK_FACTOR_MIN_VA, 4)), value(source.read(P.CLOCK_FACTOR_MAX_VA, 4))
    check(abs(value(image.read(P.CLOCK_FACTOR_PATCH[0], 4)) - P.CLOCK_FACTOR) < 1e-7 and low <= P.CLOCK_FACTOR <= high,
          f"the default of time_floating_factor is {P.CLOCK_FACTOR:g}, inside the range the game allows "
          f"({low:g}..{high:g})")


def test_weapon_camo(original, image, before):
    """Check 44: a weapon wears the camouflage its item holds (patch 44).
    check_weapon_camo_026g.py runs the site and the code on made-up configs
    and models."""
    camo_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read())
    camo = open(P.weapon_camo_bin(), "rb").read()
    # its last dword, bake_name, is the build's: the arena's entry of weapon_camo.bake_name
    check_piece(original, image, "weapon_camo_026g.section.bin", camo_va,
                P.with_bake_name(camo, len(camo) - 4, P.arena()))


def test_weapon_table(original, image, before):
    """Check 45: a weapon's upgrade window shows it in the air above the
    lobby's workbench (patch 45, WEAPON_TABLE_HOOKS). check_weapon_table_026g.py
    runs the camera's handlers and tick and mt_draw's two sites."""
    table_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_OFFSET
    code = open(P.weapon_table_bin(), "rb").read()
    listing = check_piece(original, image, "the weapon table", table_va, code,
                          code.index(struct.pack("<4f", -0.5, 0.5, -0.5, 0.0)), P.WEAPON_TABLE_ENTRIES)
    check(not [t for _, _, t in listing if re.search(r"\[(?:dword )?0x", t)]
          and P.weapon_preview_state(code, table_va)
          == P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_STATE_OFFSET,
          f"the weapon table's state is at 0x{P.WEAPON_TABLE_STATE_OFFSET:X} of .srpd, and no operand of it is an "
          "absolute address")


def test_weapon_decals(original, image, before):
    """Check 47: decals on a weapon (patch 47, WEAPON_DECALS_HOOKS).
    check_weapon_decals_026g.py runs the bake and the four editor sites."""
    camo_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read())
    decals_va = camo_va + P.WEAPON_DECALS_OFFSET
    code = open(P.weapon_decals_bin(), "rb").read()
    # A packed SSE operation on a 16-byte memory operand faults (#GP) unless the
    # address is 16-byte aligned; Unicorn does not enforce it (the game crashed
    # on patch 47's andps of a misaligned constant, 2026-10-21). Every such
    # operand of .srpw's code at a fixed address must be aligned.
    packed = re.compile(r"\b(andn?ps|x?orps|addps|subps|mulps|divps|minps|maxps|cmpps|sqrtps|rsqrtps|rcpps|"
                        r"unpck[lh]ps|shufps|movaps|movdqa|pand|por|pxor|padd[bwdq]|psub[bwdq]|pcmpeq[bwd]|"
                        r"andpd|xorpd|movapd)\b.*\[(?:dword )?(0x[0-9a-f]+)\]")
    misaligned = []
    for bin_path, base_va in ((P.weapon_camo_bin(), camo_va),
                              (P.weapon_stash_bin(), camo_va + P.WEAPON_STASH_OFFSET),
                              (P.weapon_table_bin(), camo_va + P.WEAPON_TABLE_OFFSET),
                              (P.weapon_decals_bin(), decals_va),
                              (P.upgrade_preview_bin(), camo_va + P.UPGRADE_PREVIEW_OFFSET)):
        for address, _, text in ndisasm(open(bin_path, "rb").read(), base_va):
            m = packed.search(text)
            if m and int(m.group(2), 16) % 16:
                misaligned.append(f"{address:#x} {text}")
    check(not misaligned, "every packed SSE operand of .srpw's code at a fixed address is 16-byte aligned"
          + "".join("; " + m for m in misaligned))
    check_piece(original, image, "patch 47 (weapon_decals_026g.section.bin)", decals_va, code,
                code.index(struct.pack("<4I", 0x20080000, 0x8A30CE00, 0x20080200, 0x8A30CE00)),
                P.WEAPON_DECALS_ENTRIES)


def test_upgrade_preview(original, image, before):
    """Check 48: a weapon's upgrade window shows an upgrade dropped on the
    weapon at once (patch 48, UPGRADE_PREVIEW_HOOKS). check_upgrade_preview_026g.py
    runs the sites with the game's setup_profile, profile copy and config lookups."""
    late_join = open(P.late_join_bin(), "rb").read()
    code_va = P.weapon_camo_va(late_join) + P.UPGRADE_PREVIEW_OFFSET
    stash_find = P.weapon_camo_va(late_join) + P.WEAPON_STASH_OFFSET + P.WEAPON_STASH_FIND
    state = P.late_join_state_va(late_join) + P.UPGRADE_PREVIEW_STATE_OFFSET
    preview_state = P.late_join_state_va(late_join) + P.WEAPON_PREVIEW_STATE_OFFSET
    code = open(P.upgrade_preview_bin(), "rb").read()
    listing = ndisasm(code[:code.index(b"srp_scope_fit\0")], code_va)
    starts = {a for a, _, _ in listing}
    inner = [t for _, _, text in listing if (t := branch_target(text)) is not None
             and code_va <= t < code_va + len(code) and t not in starts]
    unknown = unknown_targets(original, image.data, listing)
    check(image.read(code_va, len(code)) == P.with_kit_apply(code, P.arena()) and not inner and not unknown
          and all(code_va + at in starts for at in P.UPGRADE_PREVIEW_ENTRIES),
          f"the upgrade preview at {code_va:#x} ({len(code)} bytes, its kit_apply dword the build's): every branch "
          "inside it lands on an instruction, every one out of it is a known target, its entries start "
          "instructions" + "".join(f"; {t:#x}" for t in inner + unknown))
    check(image.read(code_va + P.UPGRADE_PREVIEW_KIT_APPLY, 4)
          == struct.pack("<I", P.arena_target(P.arena(), P.WEAPON_KIT_PREVIEW_APPLY))
          and P.upgrade_preview_targets(code, code_va) == (stash_find, state, preview_state),
          f"its kit_apply dword (+{P.UPGRADE_PREVIEW_KIT_APPLY:#x}) is patch 68's weapon_kit_preview_apply; it "
          "calls the stash's find and keeps its state and patch 32's where they are")


def test_research(original, image, before):
    """Check 53: Research, mode 5 (patch 53, RESEARCH_HOOKS; link_026g.py links it,
    check_research_026g.py runs the hooks, the cook and the rule on made-up
    games). Here: the hooks reach the block's four entries, `jmp near` each;
    the stubs' ways out are known targets; the Rust code reaches the game
    through the engine-function library alone; the cook's and the rule's
    RTTI. Each slot of ours popping what tdm's in the same place pops is
    test_call_shapes' (INSTALLED_VTABLES)."""
    source = Image(original)
    plain = lambda text: text.replace("[dword 0x", "[0x").replace("dword [", "[")
    linked = P.arena()
    code = P.linker.block_bytes(linked, "research.code")
    research_va = linked.blocks["research.code"].va
    entries = ndisasm(image.read(research_va, 20), research_va)
    check([text.split()[0] for _, _, text in entries] == ["jmp"] * 4
          and all(research_va <= linked.symbols[symbol] < research_va + 20 for _, _, symbol, _ in P.RESEARCH_HOOKS),
          "the section starts with its four entries, `jmp near` each, which the hooks reach: register, hud, "
          "welcome, icons")
    stubs = ndisasm(code[:code.index(b"st_start_match_welcome_message_5")], research_va)
    unknown = unknown_targets(original, image.data, stubs)
    check(not unknown, "the stubs' ways out are known targets (the replaced call, update_ui's message and its "
          "way on, the hud's update)" + "".join(f"; {t:#x}" for t in unknown))
    # The Rust code reaches the game's functions through the engine-function
    # library (check_engine_026g.py proves each thunk): no library
    # function's own VA in it, and every library call it makes is to a thunk.
    c_code = code[P.RESEARCH_ASM_SIZE:]
    c_listing = [plain(t) for _, _, t in ndisasm(c_code, research_va + P.RESEARCH_ASM_SIZE)]
    thunks = {linked.symbols[f["symbol"]]: f["symbol"] for f in engine_026g.functions()
              if f["symbol"] in linked.symbols}
    called = sorted({thunks[t] for text in c_listing if (t := branch_target(text)) in thunks})
    raw = [f["symbol"] for f in engine_026g.functions()
           if isinstance(f["va"], int) and struct.pack("<I", f["va"]) in c_code]
    check(called and not raw, f"the Rust code calls the library's thunks ({', '.join(called)}), never a library "
          "function's own address" + "".join(f"; {r}" for r in raw))
    read32 = lambda img, va: struct.unpack("<I", img.read(va, 4))[0]
    # The cook's RTTI: its own complete object locator (research_rule_cook, a
    # translate_query_cook as tdm_rule_core_cook is); the rule's the base's.
    def type_name(img, td):
        raw = img.read(td + 8, 64)
        return raw[:raw.index(b"\0")].decode()

    def hierarchy(img, locator):
        signature, offset, _, td, chd = struct.unpack("<5I", img.read(locator, 20))
        _, attributes, count, array = struct.unpack("<4I", img.read(chd, 16))
        bases = [read32(img, array + 4 * i) for i in range(count)]
        return (signature, offset, read32(img, td), type_name(img, td), attributes,
                [(type_name(img, read32(img, b)),) + struct.unpack("<6I", img.read(b + 4, 24)) for b in bases])
    cook_locator = read32(image, linked.symbols["rn_cook_vtable"])
    ours = hierarchy(image, cook_locator)
    core = hierarchy(source, 0xA524C0)                    # tdm_rule_core_cook's locator (vtable 0xA12C14)
    check(ours[:2] == (0, 0) and ours[2] == read32(source, 0xACCC38) and ours[3] == ".?AVresearch_rule_cook@srp@@"
          and ours[4] == core[4] and ours[5][1:] == core[5][1:] and ours[5][0][1:6] == core[5][0][1:6]
          and ours[5][0][0] == ours[3] and research_va <= cook_locator < research_va + len(code),
          f"the cook's RTTI locator ({cook_locator:#x}) is its own: {ours[3]}, a translate_query_cook as "
          f"tdm_rule_core_cook is ({[b[0] for b in ours[5]]})")
    check(read32(image, linked.symbols["rn_rule_vtable"]) == 0xA4C970,
          "the rule's vtable has game_match_rule_base's locator (0xA4C970)")


def test_fov(original, image, before):
    """Check 23: the field-of-view setting (patch 22): fov_projection, which
    the projection's first instruction jumps to, writes no XMM register but
    xmm0 (its `ret` against the original's is test_call_shapes')."""
    entry = P.LATE_JOIN_VA + P.FOV_PROJECTION_HOOK[2]
    rows = ndisasm(image.read(entry, 0xC0), entry)
    texts = [text for _, _, text in rows[:next(i for i, (_, _, text) in enumerate(rows) if text.startswith("ret")) + 1]]
    check(not any("xmm" in text and "xmm0" not in text for text in texts),
          f"fov_projection ({entry:#x}) writes no XMM register but xmm0")


def test_fov_lod(original, image, before):
    """Check 40: a model's LOD and screen factor at a wider field of view
    are the original's (patch 40): each stub does the instruction its call
    replaced, then the same by k when k is not 0; fov_projection alone
    writes k."""
    k = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.FOV_LOD_STATE_OFFSET
    pack = lambda value: struct.pack("<I", value)
    plain = lambda text: text.replace("[dword 0x", "[0x").replace("dword [", "[")
    for va, old, offset, _ in P.FOV_LOD_HOOKS:
        stub = image.read(P.LATE_JOIN_VA + offset, 32)
        times = b"\xf3\x0f\x59\x05" if old[0] == 0xF3 else b"\xd8\x0d"
        expected = old + b"\x83\x3d" + pack(k) + b"\x00" + bytes([0x74, len(times) + 4]) + times + pack(k) + b"\xc3"
        check(stub.startswith(expected) and set(stub[len(expected):]) <= {0xCC},
              f"{va:#x}: calls {P.LATE_JOIN_VA + offset:#x}: `{old.hex()}`, then by k ({k:#x}) when it is not 0, ret")
    body = image.read(P.SECTION_VA, P.late_join_state_va(open(P.late_join_bin(), "rb").read()) - P.SECTION_VA)
    found = [m.start() for m in re.finditer(re.escape(pack(k)), body)]
    entry = P.LATE_JOIN_VA + P.FOV_PROJECTION_OFFSET
    texts = [plain(t) for _, _, t in ndisasm(image.read(entry, 0xC0), entry)]
    check(len(found) == 5 and f"fstp [{k:#x}]" in texts,
          f"k ({k:#x}) is named five times in .srp: written by fov_projection alone, read by the two scales")
    check(P.FOV_LOD_STATE_OFFSET >= P.TAB_LIST_STATE_OFFSET + P.TAB_LIST_STATE_SIZE
          and P.FOV_LOD_STATE_OFFSET % 4 == 0
          and P.FOV_LOD_STATE_OFFSET + P.FOV_LOD_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE,
          f"k is at {P.FOV_LOD_STATE_OFFSET:#x} of the state page, aligned, after the other states "
          "(the scoreboard's the last)")


def test_weapon_random(original, image, before):
    """Check 19: a spawn gives the new kit's weapons a random state of the
    match's own: its hook keeps every register, does the two instructions
    it replaced and writes the state alone."""
    entry = P.LATE_JOIN_VA + P.WEAPON_RANDOM_HOOK[2]
    rows = lambda blob, at, size: ndisasm(blob.read(at, size), at)
    hook = rows(image, entry, 0x80)
    hook = hook[:next(i for i, (_, _, text) in enumerate(hook) if text == "ret") + 1]
    hook_texts = [text for _, _, text in hook]
    pc = next(i for i, (_, _, text) in enumerate(hook) if text.startswith("call"))
    add = re.match(r"add ebx,0x([0-9a-f]+)$", hook_texts[pc + 2])
    stores = [text for text in hook_texts if re.match(r"mov \[", text)]
    inside = all(entry <= branch_target(text) < hook[-1][0] + 1 for text in hook_texts
                 if branch_target(text) is not None)
    check(hook_texts[0] == "pusha" and hook_texts[-4:] == ["popa", "mov edx,[esi]", "mov eax,[edx+0x20]", "ret"]
          and hook_texts[pc + 1] == "pop ebx" and branch_target(hook_texts[pc]) == hook[pc + 1][0]
          and add is not None and (hook[pc + 1][0] + int(add.group(1), 16)) & 0xFFFFFFFF == P.WEAPON_BIND_VA
          and stores == [f"mov [eax+0x{P.WEAPON_RANDOM_FIELD - 0x10:x}],edx"] and inside,
          f"weapon_random_hook ({entry:#x}): pushad; for the 23 slots of the player's inventory, an item whose "
          f"vtable+70h is {P.WEAPON_BIND_VA:#x} (taken relative to the code) gets a state at item+5ACh (weapon+5BCh) "
          "and nothing else is written; popad, the two instructions, ret; no branch leaves it")

def test_kit_state(original, image, before):
    """Check 21: a spawn gives the new kit's weapons the state a weapon
    every game has alike: its hook keeps every register, does the two
    instructions it replaced and writes KIT_STATE_FIELDS alone."""
    entry = P.LATE_JOIN_VA + P.KIT_STATE_HOOK[2]
    rows = lambda blob, at, size: ndisasm(blob.read(at, size), at)
    hook = rows(image, entry, 0xA0)
    hook = hook[:next(i for i, (_, _, text) in enumerate(hook) if text == "ret") + 1]
    hook_texts = [text for _, _, text in hook]
    pc = next(i for i, (_, _, text) in enumerate(hook) if text.startswith("call"))
    add = re.match(r"add ebx,0x([0-9a-f]+)$", hook_texts[pc + 2])
    base = 0x594 + 0x10
    stores = {}
    for text in hook_texts:
        match = re.match(r"(or|mov) (byte |dword )?\[edx(?:\+0x([0-9a-f]+))?\],(.*)$", text)
        if not match:
            continue
        field = base + int(match.group(3) or "0", 16)
        operand = match.group(4)
        if operand == "byte -0x1":
            stores[field] = (4, 0xFFFFFFFF)
        elif operand == "bp":
            stores[field], stores[field + 1] = (1, 0), (1, 0)
        elif operand == "ebp":
            stores[field] = (4, 0)
        elif match.group(2) == "byte " and operand == "0x0":
            stores[field] = (1, 0)
        elif match.group(2) == "dword ":
            stores[field] = (4, int(operand, 16))
        else:
            stores[field] = None
    wanted = {offset: (size, value) for offset, size, value in P.KIT_STATE_FIELDS}
    wanted[P.KIT_STATE_TICK_FIELD] = (4, 0xFFFFFFFF)
    inside = all(entry <= branch_target(text) < hook[-1][0] + 1 for text in hook_texts
                 if branch_target(text) is not None)
    other_writes = [text for text in hook_texts if re.match(r"(mov|or|and|add|sub|xor|inc|dec) [a-z ]*\[", text)
                    and "[edx" not in text]
    check(hook_texts[0] == "pusha" and hook_texts[-4:] == ["popa", "mov edx,[esi]", "mov edx,[edx+0x1c]", "ret"]
          and hook_texts[pc + 1] == "pop ebx" and branch_target(hook_texts[pc]) == hook[pc + 1][0]
          and add is not None and (hook[pc + 1][0] + int(add.group(1), 16)) & 0xFFFFFFFF == P.WEAPON_BIND_VA
          and stores == wanted and not other_writes and inside,
          f"kit_state_hook ({entry:#x}): pushad; for the 23 slots of the player's inventory, an item whose "
          f"vtable+70h is {P.WEAPON_BIND_VA:#x} gets -1 at weapon+5A4h and, when the kit before (the spawn's "
          "[esp+10h]) is not the new one ([esp+18h]), KIT_STATE_FIELDS; nothing else is written; popad, the two "
          "instructions, ret; no branch leaves it")


def test_first_load(original, image, before):
    """Check 20: a first world's inputs and kit choices, in time order:
    first_load_input and first_load_kit call the game's allocator and the
    list's insert and their own search alone, write a node's value and time
    alone, and no branch leaves them."""
    sites = [P.LATE_JOIN_VA + P.FIRST_LOAD_INPUT_HOOK[2], P.LATE_JOIN_VA + P.FIRST_LOAD_KIT_HOOK[2]]
    code = ndisasm(image.read(sites[0], 0xC0), sites[0])
    code = code[:next(i for i, (address, _, text) in enumerate(code)
                      if text == "ret" and address > sites[1] + 0x2E) + 1]
    code_texts = [text for _, _, text in code]
    end = code[-1][0] + 1
    leaving = [text for text in code_texts if branch_target(text) is not None and not text.startswith("call")
               and not sites[0] <= branch_target(text) < end]
    stores = sorted({text for text in code_texts if text.startswith("mov [")})
    check(any(address == sites[1] for address, _, _ in code) and not unknown_targets(original, image.data, code)
          and not leaving and stores == ["mov [eax+0x20],ecx", "mov [eax+0x8],bl", "mov [eax+0x8],ebx",
                                         "mov [eax+0xc],ebp"],
          f"first_load_input ({sites[0]:#x}) and first_load_kit ({sites[1]:#x}): their calls known, a node's value "
          "and time their only writes, no branch leaving them")


def test_stale_dll(original, image, before):
    r"""Check 26: the sign-in says whether %TEMP%\shell_32.dll exists:
    stale_dll keeps every register, calls the game only through its import
    table, does the two writes its call replaced and returns."""
    entry = P.STALE_DLL_ENTRY
    code = open(P.stale_dll_bin(), "rb").read()
    strings = code[code.index(b"kernel32.dll\0"):]
    listing = ndisasm(code[:len(code) - len(strings)], entry)
    texts = [text for _, _, text in listing]
    here = listing[2][0]
    slots = set(imports(Image(original)))
    calls = [text for text in texts if text.startswith("call")]
    indirect = {int(m.group(1), 16) for text in calls
                if (m := re.match(r"^call (?:dword (?:near )?)?\[ebp\+0x([0-9a-f]+)\]$", text))}
    other = [text for text in calls if not re.match(r"^call (?:dword (?:near )?)?\[ebp\+0x", text)]
    jumps = [branch_target(text) for text in texts if text.startswith("j")]
    check(texts[:2] == ["pusha", f"call {here:#x}"]
          and texts[-4:] == ["popa", "mov [edi+0x8],eax", "mov [edi+0xc],dl", "ret"]
          and indirect <= slots and other == [f"call {here:#x}", "call eax"]
          and all(entry <= target < entry + len(code) for target in jumps),
          "it saves every register (pushad / popad), calls itself once for its address, the game's imports through "
          "its import table, branches only inside itself, then does the two writes it replaced and returns")
    check(strings == b"kernel32.dll\0GetTempPathA\0shell_32.dll\0",
          "the names it holds are kernel32.dll, GetTempPathA and shell_32.dll, and nothing else")


def test_mount_patches(original, image, before):
    """Check 27: the game mounts resources.db.patch_1, _2, ... over
    resources.db (mount_patches_026g.asm; check_mount_patches_026g.py runs
    it): its state, its texts, its ways out, and the prologue its jump
    replaced, replayed before it goes back."""
    code = open(P.mount_patches_bin(), "rb").read()
    entry, mounted = P.MOUNT_PATCHES_VA + P.MOUNT_ALL_OFFSET, P.MOUNT_PATCHES_VA + P.MOUNTED_OFFSET
    state = struct.unpack_from("<I", code, len(code) - 4)[0]
    late_join = open(P.late_join_bin(), "rb").read()
    page = P.late_join_state_va(late_join)
    size = P.MOUNT_STATE_SIZE
    check(state == page + P.MOUNT_STATE_OFFSET and P.MOUNT_STATE_OFFSET + size <= P.LATE_JOIN_STATE_SIZE,
          f"its state is {size} bytes at {state:#x}, +{P.MOUNT_STATE_OFFSET:#x} of the late join's state page "
          f"{page:#x}, inside it")
    strings = code[code.index(b"../../%s.db.patch_%u\0"):len(code) - 4 - P.MOUNT_REQUIRED_SIZE]
    check(strings == b"../../%s.db.patch_%u\0"
          b"Cannot mount resources.db.patch_%u. Please run the Skhid launcher again.\0"
          b"Cannot open resources.db.patch_%u (error %u). Please run the Skhid launcher again.\0",
          "the texts it holds are the path's format, ../../%s.db.patch_%u, and the two fatal errors' "
          "(a patch that fails to mount; one that is there and cannot be opened)")
    listing = ndisasm(code[:len(code) - 4 - P.MOUNT_REQUIRED_SIZE - len(strings)], P.MOUNT_PATCHES_VA)
    unknown = unknown_targets(original, image.data, listing)
    slots = set(imports(Image(original)))
    indirect = {int(m.group(1), 16) for _, _, text in listing
                if (m := re.match(r"^call (?:dword (?:near )?)?\[edx\+0x([0-9a-f]+)\]$", text))}
    at = {address: text for address, _, text in listing}
    va, old, _ = P.MOUNTED
    games = [i for i, (_, _, text) in enumerate(listing) if text == "push ebp"]
    tail = [text for _, _, text in listing[games[0]:games[0] + 4]] if len(games) == 1 else []
    check(not unknown and indirect <= slots and at.get(mounted, "").startswith("call") and at.get(entry) == "pusha"
          and tail == [text for _, _, text in ndisasm(old, va)] + [f"jmp {va + len(old):#x}"],
          "its ways out are known targets, the game's imports (through the import table, plus the delta) and the "
          f"bytes on_mounted's jump replaced followed by a jump to {va + len(old):#x}; on_mounted at "
          f"+{P.MOUNTED_OFFSET:#x} starts with a call for its address, mount_all at +{P.MOUNT_ALL_OFFSET:#x} with "
          "pushad" + "".join(f"; {t:#x}" for t in unknown))


def test_forward_scroll(original, image, before):
    """Check 28: the transparent effects scroll as in 0.29
    (forward_scroll_026g.asm; check_forward_scroll_026g.py runs it): its
    data, its ways out, each entry's replay of the instructions its jump
    replaced and its jump back, every register restored."""
    code = open(P.forward_scroll_bin(), "rb").read()
    base = P.FORWARD_SCROLL_VA
    one = code.index(struct.pack("<f", 1.0) + b"transparency_map_scroll_u_direction\0")
    tail = code[one:]
    check(tail == struct.pack("<f", 1.0) + b"transparency_map_scroll_u_direction\0transparency_map_scroll_v_direction\0"
          b"scene_slomo\0" + struct.pack("<I", base) and one % 4 == 0,
          "its data: the float 1.0 (scene_slomo's value), the names transparency_map_scroll_u_direction, "
          "transparency_map_scroll_v_direction and scene_slomo, and the address it was assembled for")
    listing = ndisasm(code[:one], base)
    at = {address: t for address, _, t in listing}
    returns = {base + offset: va + len(old) for va, old, offset, _ in P.FORWARD_SCROLL_HOOKS}
    jumps = sorted({branch_target(t) for _, _, t in listing if t.startswith("j")} - set(range(base, base + len(code))))
    unknown = unknown_targets(original, image.data, listing)
    check(not unknown and jumps == sorted(returns.values()),
          "its calls are known targets, its jumps out the jumps back after each hook"
          + "".join(f"; {t:#x}" for t in unknown))
    entries_ok = True
    for va, old, offset, _ in P.FORWARD_SCROLL_HOOKS:
        entry = base + offset
        body = [t for address, _, t in listing if entry <= address]
        stop = next(i for i, t in enumerate(body) if t.startswith("jmp") and branch_target(t) in returns.values())
        replaced = [t for _, _, t in ndisasm(old, va)]
        entries_ok &= body[stop] == f"jmp {returns[entry]:#x}" and body[stop - len(replaced):stop] == replaced
        entries_ok &= offset == 0 or code[offset - 1] == 0xCC
    check(entries_ok and at.get(base + P.SCENE_SLOMO_OFFSET) == "pusha",
          "each entry ends with the instructions its jump replaced and a jump back to the instruction after them; "
          "the binding's entry starts with pushad")
    check(sum(1 for t in at.values() if t == "pusha") == sum(1 for t in at.values() if t == "popa") == 3,
          "three pushad and three popad: every register is restored before each jump back")


def test_seek_clear(original, image, before):
    """A replay's user seek empties every client weapon's delayed sounds
    (patch revision 11, SEEK_CLEAR_HOOK): seek_clear keeps every register,
    finds the weapons' vtable relative to itself, replays the instruction
    its jump replaced and goes back. check_late_join_026g.py runs it on
    made-up places, kits and weapons."""
    va, old, offset, _ = P.SEEK_CLEAR_HOOK
    cave = P.SEEK_CLEAR_VA + offset
    rows = ndisasm(image.read(cave, 0x97), cave)
    code = [text for _, _, text in rows]
    check(code[:3] == ["pusha", f"call {cave + 6:#x}", "pop ebp"]
          and code[3] == f"add ebp,{(0xA0422C - (cave + 6)) & 0xFFFFFFFF:#x}"
          and code[-3:] == ["popa"] + [text for _, _, text in ndisasm(old, va)] + [f"jmp {va + len(old):#x}"]
          and not unknown_targets(original, image.data, rows),
          f"seek_clear: the vtable found relative to itself, its calls known, the replaced `lea` and back to "
          f"{va + len(old):#x}")


def test_kit_change(original, image, before):
    """Check 32: a kit chosen before the match starts is played at once."""
    # The order of a tick: the late join binds each place with a counter of
    # its own from (place+1) << 16 (bind_slot_reserved, at the core's bind
    # 0x2D3880) and gives the core's counter back after, so the respawn
    # rule's key (from the core's counter, from 0) comes before the places'
    # and a kit choice for a tick is played at the next one. Nothing
    # declares that order: these bytes of the late join's are its guard.
    late = open(P.late_join_bin(), "rb").read()[:P.LATE_JOIN_ASM_SIZE]
    own_keys = bytes.fromhex("0fb6462c40c1e0108945f08b83b08f01008945f48d45f08983b08f0100")
    given_back = bytes.fromhex("8b45f48983b08f0100")
    rows = ndisasm(image.read(0x2D3880, 5), 0x2D3880)
    check(late.count(own_keys) == 1 and late.find(given_back) > late.find(own_keys)
          and branch_target(rows[0][2]) == P.LATE_JOIN_VA,
          "the core's bind (0x2D3880) goes to the late join's bind_slot, which keys each place (place+1) << 16 "
          "from a counter of its own and gives the core's counter back: the respawn rule's key comes first")
    code = open(P.kit_change_bin(), "rb").read()
    rows = [row for row in ndisasm(code, P.KIT_CHANGE_VA) if row[2] != "int3"]
    stores = sorted({text for _, _, text in rows if re.match(r"^mov \[", text)})
    unknown = unknown_targets(original, image.data, rows)
    check(not unknown and stores == [],
          "kit_change's calls and ways out are known targets; it stores nothing but on its stack"
          + "".join(f"; {t:#x}" for t in unknown) + "".join(f"; {text}" for text in stores))


def test_warmup_look(original, image, before):
    """Check 32, the warmup camera before the start (WARMUP_LOOK_*,
    WARMUP_LIMITS_*): each piece's branches land on its instructions or go
    where known, it returns as its caller pops and holds no address; its
    state where patch_026g.py puts it."""
    look = open(P.warmup_look_bin(), "rb").read()
    limits = open(P.warmup_limits_bin(), "rb").read()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WARMUP_LOOK_STATE_OFFSET
    for code, base, rets, name in ((look, P.WARMUP_LOOK_VA, ["ret 0x10"], "warmup_look"),
                                   (limits, P.WARMUP_LIMITS_VA, ["ret", "ret"], "warmup_limits")):
        listing = check_piece(original, image, name, base, code)
        check([text for _, _, text in listing if text.startswith("ret")] == rets
              and not [t for _, _, t in listing if re.search(r"\[(?:dword |byte )?0x", t)],
              f"{name} returns with {', '.join(rets)} and holds no address")
    check(P.warmup_look_state(look) == state and P.warmup_look_calls(look) == [P.WARMUP_LIMITS_VA]
          and P.WEAPON_PREVIEW_STATE_OFFSET + P.WEAPON_PREVIEW_STATE_SIZE <= P.WARMUP_LOOK_STATE_OFFSET
          and P.WARMUP_LOOK_STATE_OFFSET + P.WARMUP_LOOK_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE,
          f"warmup_look keeps its state at {state:#x} (the late join's page +{P.WARMUP_LOOK_STATE_OFFSET:#x}, after "
          "the weapon preview's) and calls the end of a drag at warmup_limits' start")


def test_warmup_keys(original, image, before):
    """Check 32 (continued): a mouse button held from before the start is
    not the player's until it is let go (WARMUP_KEYS_*).
    check_kit_change_026g.py runs the code on made-up events."""
    code = open(P.warmup_keys_bin(), "rb").read()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WARMUP_KEYS_STATE_OFFSET
    listing = check_piece(original, image, "warmup_keys", P.WARMUP_KEYS_VA, code)
    calls = [text for address, _, text in listing if text.startswith("call")
             and not (branch_target(text) == address + 5 and text.startswith("call 0x"))]
    check(not calls and [text for _, _, text in listing if text.startswith("ret")] == ["ret 0xc"]
          and not [t for _, _, t in listing if re.search(r"\[(?:dword |byte )?0x", t)]
          and P.warmup_keys_state(code) == state
          and P.WARMUP_LOOK_STATE_OFFSET + P.WARMUP_LOOK_STATE_SIZE <= P.WARMUP_KEYS_STATE_OFFSET
          and P.WARMUP_KEYS_STATE_OFFSET + P.WARMUP_KEYS_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE,
          f"warmup_keys calls nothing, returns with `ret 0xc`, holds no address and keeps its state at {state:#x} "
          f"(the late join's page +{P.WARMUP_KEYS_STATE_OFFSET:#x}, after the warmup look's)")


def test_death_camera(original, image, before):
    """Check 46: on the death screen a drag turns the camera; the wheel does
    nothing (DEATH_*, death_camera_026g.asm in `.srpc`).
    check_death_camera_026g.py runs it on made-up moves and views."""
    stock = Image(original)
    texts = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    late_join = open(P.late_join_bin(), "rb").read()
    death_va = P.death_camera_va(late_join)
    state = P.late_join_state_va(late_join) + P.DEATH_CAMERA_STATE_OFFSET
    look_state = P.late_join_state_va(late_join) + P.WARMUP_LOOK_STATE_OFFSET
    drag_end = P.WARMUP_LIMITS_VA + P.WARMUP_DRAG_END_OFFSET
    # The view's update, whose start death_view replays before its jump to
    # the third-person step.
    update = texts(stock, 0x2422B0, 0x2422DE - 0x2422B0)
    code = open(P.death_camera_bin(), "rb").read()
    data_at = code.index(struct.pack("<f", math.pi / 180 * 0.1))
    step_value, aspect, yaw_max, yaw_min, pitch_up, pitch_down = struct.unpack_from("<6f", code, data_at)
    identity_at = (data_at + 24 + 15) // 16 * 16
    identity = struct.pack("<16f", 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)
    limits = [round(math.degrees(a), 4) for a in (yaw_max, yaw_min, pitch_up, pitch_down)] == [70, -70, 60, -40]
    pitch_slot = death_va + P.DEATH_CAMERA_PITCH
    parts = ((0, data_at, ["call [eax+0x30]", f"call [dword {pitch_slot:#x}]", f"call {drag_end:#x}"], ["ret 0x10"],
              {P.WARMUP_LOOK_VA}, {0xA74AEC, 0x1F1D9D3, 0xA2D79C, pitch_slot, state, state + 4}
              | {death_va + data_at + 4 * i for i in range(6)}),
             (P.DEATH_VIEW_OFFSET, len(code), None, ["ret 0x8"], {P.DEATH_UPDATE_VA, P.DEATH_THIRD_VA},
              {state, state + 4}))
    built = image.read(death_va, len(code))
    pitch_entry = struct.unpack_from("<I", built, P.DEATH_CAMERA_PITCH)[0]
    good = built[:P.DEATH_CAMERA_PITCH] + bytes(4) + built[P.DEATH_CAMERA_PITCH + 4:] == code and limits \
        and code[P.DEATH_CAMERA_PITCH:P.DEATH_CAMERA_PITCH + 4] == bytes(4) and P.ARENA_VA <= pitch_entry < P.ARENA_VA + P.ARENA_LIMIT \
        and code[identity_at:identity_at + 64] == identity \
        and set(code[data_at + 24:identity_at] + code[identity_at + 64:P.DEATH_VIEW_OFFSET]) <= {0xCC}
    for begin, end, calls, rets, exits, absolute in parts:
        listing = ndisasm(code[begin:end], death_va + begin)
        starts = {address for address, _, _ in listing}
        targets = [t for _, _, text in listing if (t := branch_target(text)) is not None and not text.startswith("call")]
        made = [text for _, _, text in listing if text.startswith("call")]
        inside = lambda t: death_va + begin <= t < death_va + end
        good &= all(t in starts for t in targets if inside(t)) and {t for t in targets if not inside(t)} == exits
        good &= [t for _, _, t in listing if t.startswith("ret")] == rets
        good &= {int(m, 16) for _, _, t in listing for m in re.findall(r"\[(?:dword |byte )?(0x[0-9a-f]+)\]", t)} \
            == absolute
        if calls is None:                          # the third-person step's start, then the jump to it
            third = [branch_target(t) for _, _, t in listing if t.startswith("call")]
            good &= len(third) == 1 and texts(image, third[0], 0x1D) == update[:5] + ["push esi", "push edi",
                                                                                   f"jmp {P.DEATH_THIRD_VA:#x}"]
        else:
            good &= made == calls
    check(good and P.FOV_LOD_STATE_OFFSET + P.FOV_LOD_STATE_SIZE <= P.DEATH_CAMERA_STATE_OFFSET
          and P.DEATH_CAMERA_STATE_OFFSET + P.DEATH_CAMERA_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE
          and abs(step_value - math.pi / 180 * 0.1) < 1e-9 and abs(aspect - 0.95492965) < 1e-7,
          f"the code at {death_va:#x} (.srpc) is death_camera_026g.section.bin ({len(code)} bytes): death_look "
          f"begins with warmup_look's test of the kits, leaves to warmup_look ({P.WARMUP_LOOK_VA:#x}) alone, "
          "calls the hud's pointer (vtable+30h), patch 112's pitch entry through its dword (the build's, in .srpx) "
          "and the end of a drag (not the window's size), keeps the "
          f"drag in warmup_look's state ({look_state:#x}) and the turn at {state:#x} (the late join's page "
          f"+{P.DEATH_CAMERA_STATE_OFFSET:#x}), reads the options sensitivity, mouse_invertion, its two constants "
          "(pi/180 x 0.1, 0.9549), .rdata's 0.5625 (0xA2D79C: patch 112's locked 16:9) and the turn's limits (70 degrees either side, 60 up, 40 down); death_view leaves to "
          "the view's update or its third-person step after the update's own start, which it gives the "
          "identity, then multiplies the turn by the eyes' three rows; every branch inside lands on an "
          "instruction; `ret 10h` and `ret 8`")


def test_weapon_kit(original, image, before):
    """Check 68: a weapon's upgrades in at most three records, the weapon kit
    (patch 68, the Rust patch `weapon_kit`): patch 48's call goes to the
    generated entry, which aligns the stack before it calls the Rust shim.
    check_weapon_kit_026g.py runs each hook from the game's code on made-up
    objects, the original beside."""
    linked = P.arena()
    entry = linked.symbols[P.WEAPON_KIT_PREVIEW_APPLY]
    listing = [t for _, _, t in ndisasm(image.read(entry, 0x50), entry)]
    shim = linked.symbols["hook_weapon_kit_on_preview_apply"]
    calls = [i for i, t in enumerate(listing) if t.startswith("call ")]
    aligned = [i for i, t in enumerate(listing) if t == "and esp,byte -0x10"]
    check(calls and listing[calls[0]] == f"call {shim:#x}" and aligned and aligned[-1] < calls[0]
          and listing[0] == "lea esp,[esp-0x4]" and "ret" in listing,
          f"patch 48's call goes to the generated entry {P.WEAPON_KIT_PREVIEW_APPLY} ({entry:#x}), which aligns "
          "the stack to 16 before it calls the Rust shim and returns with `ret`")


def test_weapon_flight(original, image, before):
    """Check 45 (round 7): patch 45's camera flight, the Rust patch
    `weapon_flight`: patch 45's state is its interface (fields_026g.tsv's
    weapon_table, the NASM's STATE). check_weapon_table_026g.py flies it with
    the game's tick."""
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_STATE_OFFSET
    row = [x for x in engine_026g.fields() if x["module"] == "image" and x["field"] == "weapon_table"]
    check(len(row) == 1 and row[0]["offset"] == state,
          f"fields_026g.tsv's weapon_table is patch 45's STATE ({state:#x}, .srpd+{P.WEAPON_TABLE_STATE_OFFSET:#x})")


def test_artefacts(original, image, before):
    """Check 54: the artefacts (patch 54, ARTEFACTS_*): the block's ways out,
    its data, the hud's text of its hint. check_artefacts_026g.py runs
    Spring's loader and links, Lifebone's active and the action with the
    game's own code on made-up artefacts."""
    linked = P.arena()
    artefacts_va = linked.blocks["artefacts.code"].va
    code = P.linker.block_bytes(linked, "artefacts.code")
    names = code.index(b"aim_time_mod\0")
    listing = [row for row in ndisasm(code, artefacts_va) if row[0] < artefacts_va + names]
    texts = [text for _, _, text in listing]
    unknown = unknown_targets(original, image.data, listing)
    check(P.artefacts_backs(linked) == list(P.ARTEFACTS_BACKS) and not unknown,
          "its entries go on where they came from (" + ", ".join(hex(a) for a in P.ARTEFACTS_BACKS)
          + "); its calls are known targets" + "".join(f"; {t:#x}" for t in unknown))
    heals = artefacts_va + names + 47
    check(code[names:] == b"aim_time_mod\0stopping_power_mod\0pain\0body\0head\0"
          + struct.pack("<III", *(artefacts_va + names + at for at in (32, 37, 42)))
          and f"mov ebp,{artefacts_va + names + 13:#x}" in texts
          and f"mov ebp,{artefacts_va + names:#x}" in texts and f"mov ebx,{heals:#x}" in texts
          and f"cmp ebx,{heals + 12:#x}" in texts,
          "its data: the keys aim_time_mod and stopping_power_mod, and the pools Lifebone's active fills (pain, "
          "body, head)")
    hud_source = open(os.path.join(HERE, "..", "flash", "hud", "src", "HUD.as"), encoding="utf-8").read()
    from localization_026g import CATALOG, LANGUAGES, read_translations
    hints = [read_translations(CATALOG / f'{language}.tsv')['st_onyx_activation_hint']
             for language, *_ in LANGUAGES]
    check('text = text.split("%s").join(String(Math.ceil(time / 1000)));' in hud_source.split(
              "public function set_parameterized_message(")[1].split("public function")[0]
          and len(hints) == len(LANGUAGES) and all(h.count("%s") == 1 and "75" in h for h in hints),
          "the hud's set_parameterized_message writes a message's %s as its time on screen in seconds, and the "
          "all Onyx activation hints say 75 % for %s")


def test_match_keys(original, image, before):
    """Check 58: the kit keys and the match chat option (patch 58,
    MATCH_KEYS_HOOKS, MATCH_KEYS_POLL_SLOT): the block's ways out, its
    state, the movies' side. check_match_keys_026g.py runs the four entries
    from the game's sites on made-up objects."""
    linked = P.arena()
    block = linked.blocks["match_keys.code"]
    code = P.linker.block_bytes(linked, "match_keys.code")
    listing = ndisasm(code[:code.index(b"root.select_kit_key\0")], block.va)
    unknown = [t for t in unknown_targets(original, image.data, listing)
               if t not in {linked.symbols["engine_movie_invoke"], linked.symbols["engine_cc_bool"]}]
    state = linked.blocks["match_keys.state"]
    check(not unknown and all(state.va <= linked.symbols[name] < state.va + state.size
                              for name in ("chat_variable", "hide_chat", "kit_keys", "kit_keys_since")),
          "its calls and ways out are known targets; its variables are match_keys.state's"
          + "".join(f"; {t:#x}" for t in unknown))
    here = lambda *path: open(os.path.join(HERE, "..", *path), encoding="utf-8").read()
    check("public function select_kit_key(index:uint)" in here("flash", "hud", "src", "HUD.as")
          and "public function pickByKey(index:uint)" in here("flash", "hud", "src", "survarium", "player",
                                                               "ProfileSelector.as")
          and 'var live:Array = ["_no_in_batle",' in here("flash", "main_menu", "src", "survarium", "menu",
                                                            "Settings.as")
          and "this._no_in_batle.enabled = true;" in here("flash", "main_menu", "src", "main_menu_fla",
                                                          "settingsTabs_15.as")
          and "\nedit\tlocalization-1.json\t-\t" in open(os.path.join(HERE, "..", "..", "release", "content.tsv"),
                                                            encoding="utf-8").read(),
          "the hud movie has root.select_kit_key, the main menu enables the option's checkbox (_no_in_batle: its "
          "timeline and create_content), "
          "release/content.tsv labels it (localization-1.json)")


def test_voice(original, image, before):
    """Check 66: voice chat (patch 66: voice.toml's eight lohooks; its key the
    game's ptt, a key or a mouse button, the action table the game's), the movies' side."""
    print("voice chat (patch 66)")
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    cstr = lambda va: stock.read(va, 40).split(b"\0")[0]
    check(rows(stock, 0x27A1B0, 5) == ["push ebx", "push ebp", "push esi", "mov esi,ecx"]
          and stock.read(0x122350, 3) == bytes.fromhex("c21400")
          and rows(stock, 0x1F96F0, 9) == ["push edi", "mov edi,eax", "mov eax,[edi+0x9c]"]
          and rows(stock, 0x1F9520, 5) == ["push ebx", "push esi", "push edi", "mov edi,ecx"]
          and rows(stock, 0x208870 + 0x17, 0x10) == ["mov eax,[ecx+0x5344]", "mov edi,0x8", "call 0x207d50"]
          and rows(stock, 0x25CF70, 6) == ["mov edi,eax", "mov eax,[edi]", "mov edx,[eax]"]
          and rows(stock, 0x25CF7F, 4) == ["mov ebp,[esp+0x14]"] and rows(stock, 0x25CF9D, 5) == ["cmp dword [esp+0x18],byte +0x0"],
          "the sites: match_client::connect's start (0x27A1B0; a replay's connect 0x122350 is another, `ret 14h`), "
          "on_finish_match (0x1F96F0) and on_deactivate (0x1F9520) at their starts; the chat handler asks the "
          "game's binder (+5344h) for kind 8 (0x208887); on_mouse_key_action past its keyboard ask (0x25CF70), the "
          "button's key at ESP+14h (0x25CF7F) and its kind at +18h (0x25CF9D)")
    ptt = struct.unpack("<5I", stock.read(0xA74F8C, 20))
    check(cstr(ptt[0]) == b"ptt" and ptt[1:4] == (0x47, 8, 0) and cstr(ptt[4]) == b"kP"
          and image.read(0xA74F8C, 20) == stock.read(0xA74F8C, 20),
          "the action table's entry 0xA74F8C is the game's ptt (47h, kind 8, kP), unchanged in the build")
    linked = P.arena()
    block = linked.blocks["patches.code"]
    declarations = {h["entry"]: h for h in P.hooks_026g.hooks(exe=P.hooks_026g.original_reader(original))
                    if any(m["patch"] == "voice" for m in h.get("chain", [h]))}
    hooks = [h for h in P.rust_hooks(original) if h[2] in declarations]
    check(len(hooks) == 8 and {h[0] for h in hooks} == {0x1EA210, 0x27A1B0, 0x1F96F0, 0x1F9520, 0x1F7C3C, 0x25CD00,
                                                       0x25CF70, 0x233D30},
          "voice.toml's eight lohooks: configs.start, match_client.connect, game_world.finish and .deactivate, "
          "game_world.after_move, tab_cursor.key, game_world.mouse_key (0x25CF70, past patch 31's jump; the key at "
          "ESP+14h, its kind at +18h), hud.callback")
    for va, old, symbol, opcode in hooks:
        target = linked.symbols[symbol]
        hook = P.hook_bytes(target, va, old, 0, opcode)
        check(opcode == 0xE9 and stock.read(va, len(old)) == old == before.read(va, len(old))
              and image.read(va, len(hook)) == hook and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original, `jmp {target:#x}` ({symbol}, patches.code) in the build")
    here = lambda *path: open(os.path.join(HERE, "..", *path), encoding="utf-8").read()
    hud = here("flash", "hud", "src", "HUD.as")
    menu = here("flash", "hud", "src", "survarium", "controls", "PlayerRowMenu.as")
    timeline = here("flash", "main_menu", "src", "main_menu_fla", "settingsTabs_15.as")
    check("public function srp_voice_talking(... names)" in hud
          and "public function srp_voice_menu(place:uint, offered:uint, muted:uint)" in hud
          and "public function srp_voice_mute(place:uint, done:uint)" in hud
          and 'this.call("srp_voice_menu",String(row.id));' in menu
          and 'method:"srp_voice_mute"' in menu
          and 'var live:Array = ["_no_in_batle","_hide_spam","_use_mik"];'
          in here("flash", "main_menu", "src", "survarium", "menu", "Settings.as")
          and "this._hide_spam.enabled = true;" in timeline and "this._use_mik.enabled = true;" in timeline
          and "this._sound_сhat.slider.enabled = false;" not in timeline
          and "this._mic_sens.slider.enabled = false;" not in timeline,
          "the hud has root.srp_voice_talking and the Tab menu's root.srp_voice_menu and "
          "root.srp_voice_mute, its row menu asks srp_voice_menu and sends srp_voice_mute; the options enable "
          "Disable voice chat (_hide_spam), Use microphone, the voice volume and the microphone's sensitivity")

def test_hide_ui(original, image, before):
    """Check 62: the key that hides the interface (patch 62,
    HIDE_UI_HOOKS). check_hide_ui_026g.py runs the three entries from the
    game's sites on made-up objects, with the game's own show_ui and
    get_binded_action."""
    stock = Image(original)
    linked = P.arena()
    block = linked.blocks["hide_ui.code"]
    code = P.linker.block_bytes(linked, "hide_ui.code")
    names = code.index(b"root.toggle_interface\0")
    listing = ndisasm(code[:names], block.va)
    texts = [text for _, _, text in listing]
    pushed = lambda text: f"push dword {block.va + code.index(text + bytes(1)):#x}" in texts
    check(not unknown_targets(original, image.data, listing)
          and f"mov dword [esp],{P.HIDE_UI_BACKS[2]:#x}" in texts and f"mov dword [esp],{P.HIDE_UI_BACKS[3]:#x}" in texts
          and all(pushed(t) for t in (b"root.toggle_interface", b"root.interface_hidden", b"root.hide_interface"))
          and "hide_ui.state" not in linked.blocks,
          "its calls and ways out are known targets; it returns to "
          "0x26CCAE or 0x26CDD2; it invokes root.toggle_interface, "
          "root.interface_hidden and root.hide_interface, and keeps no state of its own")
    here = lambda *path: open(os.path.join(HERE, "..", *path), encoding="utf-8").read()
    # the options' controls: the game's 40 rows, the six functions that read them, the arena's 41
    table, count = P.HIDE_UI_TABLE, P.HIDE_UI_ROWS
    empty = lambda at: struct.pack("<III", at + 0x1C, at + 0x1C, at + 0x3C) + bytes(32) + \
        struct.pack("<III", at + 0x48, at + 0x48, at + 0x68) + bytes(32)
    original_rows = [stock.read(table + 0x68 * i, 0x68) for i in range(count)]
    controls = linked.symbols["controls"]
    data_block = linked.blocks["hide_ui.data"]
    added = len(P.CONTROLS_ADDED)
    arena_rows = [image.read(controls + 0x68 * i, 0x68) for i in range(count + added)]
    new_rows = arena_rows[count:]
    labels = [struct.unpack_from("<I", row, 4)[0] for row in new_rows]
    # push-to-talk (47h): the game's row is the chat group's (1) with its key button disabled (type 0);
    # patch 66's key is a game key that can be set (group 2, type 1)
    moved = lambda head: struct.pack("<4I", *struct.unpack("<2I", head[:8]), 2, 1) \
        if head == struct.pack("<4I", 0x47, 0x9FC900, 1, 0) else head
    check(sum(moved(row[:16]) != row[:16] for row in original_rows) == 1 and data_block.va <= controls and controls + 0x68 * (count + added) <= data_block.va + data_block.size
          and all(arena_rows[i][:16] == moved(original_rows[i][:16]) and arena_rows[i][16:] == empty(controls + 0x68 * i)
                  for i in range(count))
          and [struct.unpack_from("<4I", row) for row in new_rows]
          == [(action, label, 2, 1) for (action, _), label in zip(P.CONTROLS_ADDED, labels)]
          and all(row[16:] == empty(controls + 0x68 * (count + n)) for n, row in enumerate(new_rows))
          and [image.read(label, len(key) + 1) for (_, key), label in zip(P.CONTROLS_ADDED, labels)]
          == [key.encode() + b"\0" for _, key in P.CONTROLS_ADDED],
          f"`controls` ({controls:#x}, hide_ui.data) is the game's 40 rows (push-to-talk's in the game group, "
          "its key settable), each pointing into itself, a 41st: "
          "19h, st_hide_interface_action, and a 42nd: 23h, st_hold_artifact_action, group 2, type 1")
    check("\nedit\tlocalization-1.json\t-\t" in open(os.path.join(HERE, "..", "..", "release", "content.tsv"),
                                                       encoding="utf-8").read(),
          "release/content.tsv makes the label (localization-1.json)")
    # the movies' side: each movie's interface is its root, hidden in one step; every layer the
    # game put on the stage is in the root's _stageLayer (the lobby's: _layers and _overlay)
    tree = lambda movie: {os.path.relpath(os.path.join(d, f), os.path.join(HERE, "..", "flash", movie, "src")):
                          open(os.path.join(d, f), encoding="utf-8").read()
                          for d, _, fs in os.walk(os.path.join(HERE, "..", "flash", movie, "src")) for f in fs
                          if f.endswith(".as")}
    stage_add = re.compile(r"((?:\w+\.)*\w*stage)\.addChild(?:At)?\(([^,)]+)")
    lobby_tree = tree("lobby_menu")
    lobby = lobby_tree["LobbyMenu.as"]
    found = sorted((path, m.group(1), m.group(2)) for path, text in lobby_tree.items()
                   for m in stage_add.finditer(text))
    # Message's `_stage` is the messages' container (Message.initStage(this._top_container)), the
    # name editor's `MainScene.stage` the root (MainScene.stage = this): both inside the root
    allowed = {("survarium/controls/Message.as", "_stage"), ("survarium/char/CharWindow.as", "MainScene.stage")}
    layered = sorted({m.group(1) for m in re.finditer(r"this\._(?:layers|overlay)\.addChild\(([\w.]+)\)", lobby)})
    check(not [f for f in found if (f[0], f[1]) not in allowed]
          and "Message.initStage(this._top_container);" in lobby and "MainScene.stage = this;" in lobby
          and "SrvContextMenu.registerStage(stage,this._layers);" in lobby
          and "DragManager.init(this.stage,this._overlay);" in lobby and "PopUpManager.layer = this._transients;" in lobby
          and "_layer.addChild(_id);" in lobby_tree["survarium/controls/SrvContextMenu.as"]
          and "(layer ? layer : _stage).addChild(_dragCanvas);" in lobby_tree["scaleform/clik/managers/DragManager.as"]
          and "host.addChild(_defaultPopupCanvas);" in lobby_tree["scaleform/clik/managers/PopUpManager.as"]
          and lobby.index("this.makeStageLayer();") < lobby.index("this.init();")
          and "super.addChild(this._stageLayer);" in lobby
          and all(f"override public function {f}" in lobby for f in ("addChild(", "addChildAt(", "setChildIndex("))
          and "this._stageLayer.scaleX = this._stageLayer.scaleY = 1 / scaleFactor;" in lobby
          and layered == ["this._cards", "this._corner", "this._front_container", "this._top2_container",
                          "this._top_container", "this._wheelBack", "this._wheelsMC", "this.fadeBack",
                          "this.lobbyGameLoad", "this.tipContainer"]
          and lobby.index("this._overlay.addChild(this._corner);") < lobby.index("this._overlay.addChild(this._cards);")
          and lobby.count("this._cards.addChild(this.itemHintRenderer") == 5
          and "setTopmostLevel(this.itemHintRenderer" not in lobby
          and "InterfaceHider" not in "".join(lobby_tree.values()),
          "the lobby's interface is its root: nothing of it is added to the stage (Message's container and "
          "MainScene.stage are inside the root); what the game put on the stage is in the root's last child, "
          "_stageLayer, at the stage's scale, made before anything else and kept last (addChild, addChildAt, "
          f"setChildIndex): {', '.join(layered)}, the right-click menu in _layers, CLIK's popup canvas in _transients; "
          "_overlay, above every modal: _corner, _cards (the item cards, no longer topmost), the drag canvas, the "
          "tooltips; no InterfaceHider, no timer")
    toggle = lobby[lobby.index("public function toggle_interface() : void"):]
    toggle = toggle[:toggle.index("public function interface_hidden()")]
    check("if(SrvContextMenu.visisble)\n         {\n            SrvContextMenu.hide();" in toggle
          and toggle.count("SrvContextMenu.hide()") == 1 and "this.hideItemHint(null);" in toggle
          and "visible = false;" in toggle and "_hiddenUi" not in lobby,
          "the lobby's toggle_interface closes a right-click menu only when one is open (none ever opened: "
          "SrvContextMenu.hide's removeChild(null) threw, owner's test of 64ce3ce7) and the item cards (on "
          "Scaleform's topmost level, drawn whatever their parents), then hides the root")
    enable = lobby[lobby.index("public function enable_element(value:*, value2:Boolean = true) : void"):]
    enable = enable[:enable.index("public function highlight_element(")]
    check("setTopmostLevel" not in enable and "this._corner.addChild(button);" in enable
          and "this.statusPanel.addChildAt(button," in enable and "button != this.statusPanel.btn_off" in enable
          and "_topmost" not in lobby and "dispatchEvent(new Event(LAID_OUT));" in lobby_tree["survarium/controls/StatusPanel.as"]
          and "this.statusPanel.addEventListener(StatusPanel.LAID_OUT,this.onStatusPanelLaidOut);" in lobby
          and not any(f"function {t}(" in lobby for t in ("testDragTutor", "testPlayTutor", "testAmmoTutor",
                                                         "testAmmoTutor2", "testFactionPreTutor", "testFactionTutor",
                                                         "testCloseSlide", "showSlideOnVs", "testTutorComplete")),
          "the exe's lobby state calls "
          "root.enable_element(root.statusPanel.btn_off / btn_settings, true), which put the corner buttons on "
          "Scaleform's topmost level (above every modal, and drawn whatever their parents: they stayed in the owner's "
          "test of 335343b6); the lobby's enable_element puts them in _corner instead (above every modal, inside the "
          "root, drawn where the status panel draws them after each of its layouts) and back with false; the "
          "tutorial's private test functions are gone")
    hud = here("flash", "hud", "src", "HUD.as")
    icons = here("flash", "player_icons", "src", "player_icons_fla", "MainTimeline.as")
    own = lambda movie: {p: t for p, t in tree(movie).items() if not p.startswith("scaleform")}
    hud_adds = sorted((p, m.group(1), m.group(2)) for p, t in own("hud").items() for m in stage_add.finditer(t))
    icon_adds = [(p, m.group(1)) for p, t in own("player_icons").items() for m in stage_add.finditer(t)]
    check(all("public function hide_interface(hidden:Boolean) : void" in t and "visible = !hidden;" in t
              for t in (hud, icons)) and "public function interface_hidden() : Boolean" in hud
          and hud_adds == [("HUD.as", "stage", "this._keptLayer"), ("HUD.as", "stage", "this.deathEyes"),
                           ("HUD.as", "stage", "this.effectContainer")]
          and not icon_adds
          and "stage.addChildAt(this.effectContainer,stage.getChildIndex(this));" in hud
          and "stage.addChildAt(this.deathEyes,stage.getChildIndex(this));" in hud
          and "this._stageLayer.addChild(this.gameInfo);" in hud and "place.concat(this.transform.matrix);" in hud
          and all(f"override public function {f}" in hud for f in ("addChild(", "addChildAt(", "setChildIndex("))
          and "InterfaceHider" not in "".join(tree("hud").values()) + "".join(tree("player_icons").values())
          and all(f"this.effects[this.{name}] = " in hud for name in ("BLOOD_EFFECT", "BLOOD_DRIP1", "GRENADE_EFFECT")),
          "the hud's and the player icons' interface is their root, hidden in one step (root.hide_interface; the "
          "hud's interface_hidden); the hud's fps line is in the root's _stageLayer; the two things on the hud's "
          "stage beside the root are the screen effects' container (add_effect's blood, its drips, the grenade's "
          "flash), below the root, placed as the root is scaled: the scene's, so it stays (the owner, 2026-10-24), "
          "and the death fade (DeathEyes, #529) between them, which F11 leaves too")
    check("public function toggle_interface() : void" in lobby and "public function interface_hidden() : Boolean" in lobby,
          "the lobby movie has root.toggle_interface and root.interface_hidden")


def test_kill_feed(original, image, before):
    """Check 67: the kill feed's wall and pain icons (patch 67,
    KILL_FEED_HOOKS, KILL_FEED_ARGUMENT). check_kill_feed_026g.py runs the
    three sites from the game's code on made-up victims, bullets and
    descriptions, with the game's own part lookup."""
    linked = P.arena()
    block = linked.blocks["kill_feed.code"]
    code = P.linker.block_bytes(linked, "kill_feed.code")
    names = code.index(struct.pack("<I", block.va + code.index(b"pain\0")))   # the names' pointers: the data
    listing = ndisasm(code[:names], block.va)
    texts = [text for _, _, text in listing]
    pushed = lambda text: f"push dword {block.va + code.index(text + bytes(1)):#x}" in texts
    check(not unknown_targets(original, image.data, listing) and pushed(b"penetration_icon")
          and pushed(b"ricochet_icon") and pushed(b"pain_icon")
          and "kill_feed.state" not in linked.blocks
          and [t for t in texts if t.startswith("mov [") and "esp" not in t] == ["mov [eax+0x4],bl", "mov [eax+0x5],dl"],
          "its calls are known targets (SetMember through the description's interface); it sets penetration_icon, "
          "ricochet_icon and pain_icon, writes the node's +4 and +5 and nothing else outside its frame, and keeps "
          "no state")
    here = lambda *path: open(os.path.join(HERE, "..", *path), encoding="utf-8").read()
    item = here("flash", "hud", "src", "survarium", "controls", "CombatLogItem.as")
    check("data.penetration_icon == 1" in item and "data.ricochet_icon == 1" in item and "data.pain_icon == 1" in item
          and all(f"new {n}()" in item for n in ("CombatLogWallIcon", "CombatLogRicochetIcon", "CombatLogPainIcon"))
          and all(os.path.isfile(os.path.join(HERE, "..", "flash", "hud", "bitmaps", f"{n}.png"))
                  for n in ("CombatLogWallIcon", "CombatLogRicochetIcon", "CombatLogPainIcon")),
          "the hud movie's kill feed row shows penetration_icon, ricochet_icon and pain_icon (its bitmaps "
          "CombatLogWallIcon, CombatLogRicochetIcon and CombatLogPainIcon)")


def test_spot(original, image, before):
    """Check 38: spotting (patch 38, SPOT_*): the code begins with the
    instructions its jump replaced, leaves only to SPOT_EXITS and holds one
    absolute value. check_spot_026g.py runs the input apply with the game's
    own spottable code on made-up hits."""
    va, old, _, _ = P.SPOT_HOOK
    code = open(P.spot_bin(), "rb").read()
    listing = [row for row in ndisasm(code, P.SPOT_VA) if row[2] != "int3"]
    inside = lambda target: P.SPOT_VA <= target < P.SPOT_VA + len(code)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    starts = {address for address, _, _ in listing}
    texts = [text for _, _, text in listing]
    replaced = [text for _, _, text in ndisasm(old, va)]
    absolute = [text for text in texts if re.search(r"0x[0-9a-f]{5,}", text) and branch_target(text) is None]
    check({t for t in targets if not inside(t)} == set(P.SPOT_EXITS) and all(t in starts for t in targets if inside(t))
          and texts[:len(replaced)] == replaced and absolute == [f"cmp dword [ecx],{P.SPOT_OWNER_VTABLE:#x}"]
          and set(code[listing[-1][0] - P.SPOT_VA + 5:]) <= {0xCC},
          "the code begins with the instructions it replaces, leaves only to SPOT_EXITS, holds one absolute value "
          f"({P.SPOT_OWNER_VTABLE:#x}), every branch inside lands on an instruction, int3 after it")


def imports(image):
    """{IAT slot VA: (dll, function)} of the import table and the delay-load
    table (its addresses are VAs in this exe)."""
    out = {}
    cstr = lambda va: bytes(image.data[image.offset(va):image.data.index(b"\0", image.offset(va))]).decode()
    rva, _ = image.directory(1)
    at = P.IMAGE_BASE + rva
    while True:
        lookup, _, _, name, first = struct.unpack("<5I", image.read(at, 20))
        if not name:
            break
        for index in itertools.count():
            thunk = struct.unpack("<I", image.read(P.IMAGE_BASE + (lookup or first) + 4 * index, 4))[0]
            if not thunk:
                break
            if not thunk & 0x80000000:
                out[P.IMAGE_BASE + first + 4 * index] = (cstr(P.IMAGE_BASE + name), cstr(P.IMAGE_BASE + thunk + 2))
        at += 20
    rva, _ = image.directory(13)
    at = P.IMAGE_BASE + rva
    while True:
        attributes, name, _, iat, names, _, _, _ = struct.unpack("<8I", image.read(at, 32))
        if not name:
            break
        fix = (lambda v: P.IMAGE_BASE + v) if attributes & 1 else (lambda v: v)
        for index in itertools.count():
            thunk = struct.unpack("<I", image.read(fix(names) + 4 * index, 4))[0]
            if not thunk:
                break
            if not thunk & 0x80000000:
                out[fix(iat) + 4 * index] = (cstr(fix(name)), cstr(fix(thunk) + 2))
        at += 32
    return out


def test_tab_list(original, image, before):
    """Check 39: the scoreboard shows the places in the match and no place
    whose player left (patch 39, TAB_LIST_*). check_tab_list_026g.py runs
    the rows' refresh and the lists made again on made-up huds."""
    code = open(P.late_join_bin(), "rb").read()
    for offset, start, size, wanted in ((P.TAB_ROW_OFFSET, P.TAB_ROW_START, 0x2F, {P.LATE_JOIN_VA + 2304}),
                                        (P.TAB_SHOWN_OFFSET, P.TAB_SHOWN_START, 0x62,
                                         {P.LATE_JOIN_VA + 2304, P.LATE_JOIN_VA + 3328})):
        at = P.LATE_JOIN_VA + offset
        listing = ndisasm(code[offset:offset + size], at)
        calls = {branch_target(text) for _, _, text in listing if text.startswith("call 0x")}
        leaving = [text for _, _, text in listing if branch_target(text) is not None
                   and not text.startswith("call") and not at <= branch_target(text) < at + size]
        check(code[offset:offset + len(start)] == start and image.read(at, size) == code[offset:offset + size]
              and calls == wanted and not leaving and listing[-1][2] == "ret"
              and not [t for _, _, t in listing if re.search(r"\[(?:dword )?0x", t)],
              f"{at:#x}: the code calls get_state{' and rebuild_lists' if len(wanted) > 1 else ''} alone, leaves "
              "by its `ret`, and holds no absolute address (the state page is found from its own address)")
    hook = ndisasm(code[3456:3520], P.LATE_JOIN_VA + 3456)
    back = P.LATE_JOIN_VA + P.TAB_SHOWN_OFFSET + code[P.TAB_SHOWN_OFFSET:].index(bytes.fromhex("57e8")) + 6
    pc = P.LATE_JOIN_VA + 3456 + 5
    adds = [int(text.split(",")[1], 16) for _, _, text in hook if text.startswith("add ecx,")]
    check(len(adds) == 2 and (pc + adds[0]) & 0xFFFFFFFF == P.LATE_JOIN_VA + 1120 + 100
          and (pc + adds[0] + adds[1]) & 0xFFFFFFFF == back and code[back - P.LATE_JOIN_VA:][:2] == b"\x8f\x06",
          "lists_hook ends a run of the lists whose return address is install_context's or tab_shown's (the "
          "instruction after its call of rebuild_lists)")


def test_replay_link(original, image, before):
    """"Watch replay" (patch revision 14, REPLAY_LINK_HOOK): the lobby's
    video_tutorial_link_clicked opened its link buffer with ShellExecuteA;
    the build jumps to replay_link instead, which starts updater_skhid.exe
    beside the exe for a replay link alone. check_replay_link_026g.py runs
    the handler on made-up links."""
    stock = Image(original)
    va, old, offset, _ = P.REPLAY_LINK_HOOK
    cave = P.REPLAY_LINK_VA + offset
    table = imports(stock)
    code = open(P.replay_link_bin(), "rb").read()
    body = code[:code.index(b"replay ")]
    listing = ndisasm(body, cave)
    texts = [text for _, _, text in listing]
    pc = cave + 6
    slots = set()
    for _, _, text in listing:
        match = re.fullmatch(r"call \[ebp-0x([0-9a-f]+)\]", text)
        if match:
            slots.add((pc - int(match.group(1), 16)) & 0xFFFFFFFF)
    branches = {branch_target(text) for text in texts if branch_target(text) is not None}
    check(texts[:3] == ["pusha", f"call {pc:#x}", "pop ebp"] and texts[-4:] == ["popa", "mov al,0x1", "pop esi", "ret 0x4"]
          and all(cave <= target < cave + len(body) for target in branches)
          and {text for text in texts if text.startswith("call") and branch_target(text) is None}
          == {f"call [ebp-{pc - slot:#x}]" for slot in slots} | {"call edx"}
          and slots <= set(table)
          and code[len(body):] == (b"replay \x88 \xa0 \xc0 \0"
                                   + "updater_skhid.exe\0".encode("utf-16-le") + b"kernel32.dll\0CreateProcessW\0"),
          "replay_link: pushad, its own address in ebp, the game's imports through their IAT slots (and "
          "CreateProcessW from GetProcAddress), no branch out but its "
          "return (`popad / mov al,1 / pop esi / ret 4`); the link's pattern and the program's fixed name")


def test_shared_upgrades(original, image, before):
    """Check 31: one upgrade on several items (patch 30, SHARED_UPGRADES_HOOKS).
    The lobby's `46 02` and the answer `83 46 02` named no item, and the
    answer took the upgrade off the first item holding it; the build's
    four sites call or jump into shared_upgrades_026g.asm, which names the
    item. check_shared_upgrades_026g.py runs the original's and the build's
    code on inventories with an upgrade on several items."""
    code = open(P.shared_upgrades_bin(), "rb").read()
    listing = [row for row in ndisasm(code, P.SHARED_UPGRADES_VA) if row[2] != "int3"]
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not P.SHARED_UPGRADES_VA <= t < P.SHARED_UPGRADES_VA + len(code)}
    starts = {address for address, _, _ in listing}
    check(not unknown_targets(original, image.data, listing) and all(t in starts for t in targets - outside)
          and all(P.SHARED_UPGRADES_VA + offset in starts for _, _, offset, _ in P.SHARED_UPGRADES_HOOKS)
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|0x[0-9a-f]{6,}\]", text)],
          "the code's calls and jumps out are known targets, every branch inside it lands on an instruction, "
          "and it holds no address (nothing to relocate)")


def test_read_order(original, image, before):
    """Check 34: the order of the archive reads (patch 33, READ_ORDER_*).
    check_read_order_026g.py runs the original's and the build's predicate
    and the game's sort on made-up read queries."""
    va, size = P.READ_ORDER_VA, P.READ_ORDER_SIZE
    code = open(P.read_order_bin(), "rb").read()
    listing = [row for row in ndisasm(code, va) if row[2] != "int3"]
    end = listing[-1][0] + len(listing[-1][1]) // 2
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not va <= t < va + size}
    starts = {address for address, _, _ in listing}
    unknown = [t for t in unknown_targets(original, image.data, listing) if not va <= t < va + size]
    check(not unknown and all(t in starts for t in targets - outside)
          and [t for _, _, t in listing if t.startswith("ret")] == ["ret 0x4"] and set(code[end - va:]) == {0xCC}
          and not [t for _, _, t in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|\[(?:dword )?0x", t)],
          "the new code's calls are known targets; it "
          f"returns by its one `ret 4`, every branch inside lands on an instruction, it holds no absolute address, "
          f"and int3 fills it from {end:#x}")
def test_weapon_preview(original, image, before):
    """Check 33: a weapon's upgrade window shows the weapon (patch 32,
    WEAPON_PREVIEW_HOOKS). check_weapon_preview_026g.py runs the original's
    and the build's code on a made-up lobby."""
    code = open(P.weapon_preview_bin(), "rb").read()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_PREVIEW_STATE_OFFSET
    check(image.read(P.WEAPON_PREVIEW_VA, len(code)) == code and P.weapon_preview_state(code) == state,
          f"the weapon preview at {P.WEAPON_PREVIEW_VA:#x} is weapon_preview_026g.section.bin ({len(code)} bytes), "
          f"its state at {state:#x} (0x{P.WEAPON_PREVIEW_STATE_OFFSET:X} of .srpd, after the patches' mount's)")
    listing = ndisasm(code[:code.index(struct.pack("<f", 1.5))], P.WEAPON_PREVIEW_VA)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not P.WEAPON_PREVIEW_VA <= t < P.WEAPON_PREVIEW_VA + len(code)}
    starts = {address for address, _, _ in listing}
    stash_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_STASH_OFFSET
    table_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_OFFSET
    find_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.UPGRADE_PREVIEW_OFFSET \
        + P.UPGRADE_PREVIEW_FIND
    check(not unknown_targets(original, image.data, listing) and all(t in starts for t in targets - outside)
          and all(P.WEAPON_PREVIEW_VA + offset in starts for _, _, offset, _ in P.WEAPON_PREVIEW_HOOKS)
          and [address - P.WEAPON_PREVIEW_VA for address, _, text in listing
               if branch_target(text) == find_va]
          == list(P.WEAPON_PREVIEW_FIND_CALLS)
          and [address - P.WEAPON_PREVIEW_VA for address, _, text in listing if branch_target(text) == table_va]
          == list(P.WEAPON_PREVIEW_CAMERA_JUMPS)
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|\[(?:dword )?0x", text)],
          "its calls and jumps out are known targets, upgrade_preview_026g.asm's find at its two finds and "
          "weapon_table_026g.asm's camera at start_cam's and exit_cam's ends; every branch inside it lands on an "
          "instruction, and no operand is an absolute address")
    stash = open(P.weapon_stash_bin(), "rb").read()
    check(image.read(stash_va, len(stash)) == stash and P.weapon_preview_state(stash, stash_va) == state,
          f"the weapon stash at {stash_va:#x} (.srpw+{P.WEAPON_STASH_OFFSET:#x}) is weapon_stash_026g.section.bin "
          f"({len(stash)} bytes), its state patch 32's (+0Ch..+17h of it, inside WEAPON_PREVIEW_STATE_SIZE)")
    sites = all(image.read(va, 5) == bytes([opcode]) + struct.pack("<i", stash_va + entry - va - 5)
                and image.read(stash_va + entry, 1) == b"\xE9"
                for (va, _, _, opcode), (entry, _) in ((h, P.WEAPON_STASH_ENTRIES[h[0]]) for h in P.WEAPON_PREVIEW_HOOKS
                                                       if h[0] in P.WEAPON_STASH_ENTRIES))
    check(sites and len(P.WEAPON_STASH_ENTRIES) == 3,
          "patch 32's start (0x265E73), exit (0x265F44) and setup_profile (0x284B10) sites go to the stash's "
          "entries (+0, +5, +0Ah), each a jump")
    listing = ndisasm(stash, stash_va)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not stash_va <= t < stash_va + len(stash)}
    starts = {address for address, _, _ in listing}
    absolute = {int(m, 16) for _, _, text in listing for m in re.findall(r"\[(?:dword )?(0x[0-9a-f]+)\]", text)}
    check(not unknown_targets(original, image.data, listing) and all(t in starts for t in targets - outside) and stash_va in starts
          and absolute == {0x1F1D9C4}
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[", text)],
          "the stash code's calls and ways out are known targets or patch 32's code, every branch inside it lands "
          "on an instruction, and its only absolute operand is the game's allocator (0x1F1D9C4)")
    draw = open(P.weapon_draw_bin(), "rb").read()
    listing = ndisasm(draw, P.WEAPON_DRAW_VA)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    starts = {address for address, _, _ in listing}
    check(image.read(P.WEAPON_DRAW_VA, len(draw)) == draw and P.weapon_draw_state(draw) == state
          and all(P.WEAPON_DRAW_VA <= t < P.WEAPON_DRAW_VA + len(draw) and t in starts for t in targets)
          and [text for _, _, text in listing].count("ret") == 2
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|\[(?:dword )?0x", text)],
          f"the draw at {P.WEAPON_DRAW_VA:#x} is weapon_draw_026g.section.bin ({len(draw)} bytes), reads the state "
          f"at {state:#x}, branches only inside itself, returns at its two ends and holds no absolute address")


def test_revision_override(original, default, server):
    """Check 30: a release's --revision=<n> against the default build of the
    same server: the revision byte and the PE checksum, nothing else."""
    code = open(P.late_join_bin(), "rb").read()
    upload = open(P.default_bin("section"), "rb").read()
    byte, checksum_at = P.file_offset(P.REVISION_VA) + 4, P.Headers(default).checksum_at
    for revision in (P.REVISION + 1, 255):
        data, _ = P.build(original, "section", upload, server, code, revision=revision)
        changed = [i for i in range(max(len(data), len(default)))
                   if i >= len(data) or i >= len(default) or data[i] != default[i]]
        check(len(data) == len(default) and data[byte] == revision
              and all(i == byte or checksum_at <= i < checksum_at + 4 for i in changed),
              f"--revision={revision}: the revision byte (file {byte:#x}) is {revision}; besides it only the PE "
              f"checksum differs from the build of REVISION {P.REVISION} ({len(changed)} bytes)")
        test_revision(original, Image(data), "section", revision, late_join=True)
    for revision, late_join in ((P.REVISION - 1, code), (256, code), (P.REVISION + 1, None)):
        try:
            P.build(original, "section", upload, server, late_join, revision=revision)
            refused = False
        except SystemExit:
            refused = True
        check(refused, f"--revision={revision}{'' if late_join else ' without the late join'} is refused")


def test_required_patches(original, default, server):
    """Check 49: a release's --required-patches= against the default build
    of the same server: the mount's bitmap and the PE checksum, nothing else."""
    code = open(P.late_join_bin(), "rb").read()
    upload = open(P.default_bin("section"), "rb").read()
    mount = open(P.mount_patches_bin(), "rb").read()
    needed = len(mount) - 4 - P.MOUNT_REQUIRED_SIZE
    image = Image(default)
    bitmap_va = P.MOUNT_PATCHES_VA + needed
    check(image.read(bitmap_va - len(P.MOUNT_UNREADABLE), len(P.MOUNT_UNREADABLE)) == P.MOUNT_UNREADABLE
          and image.read(bitmap_va, P.MOUNT_REQUIRED_SIZE) == bytes(P.MOUNT_REQUIRED_SIZE)
          and P.required_patches_of(bytes(default)) == [] and lib_required_patches(default) in ("", None),
          f"the default build requires no patch: its bitmap at {bitmap_va:#x}, after the \"Cannot open\" text, is "
          f"{P.MOUNT_REQUIRED_SIZE} zeros")
    # The probe: a missing file (error 2 or 3) tests its n in the bitmap
    # and goes on to the "Cannot open" fatal's push when the bit is set.
    listing = ndisasm(mount[:needed - 200], P.MOUNT_PATCHES_VA)
    texts = [text for _, _, text in listing]
    bt = [i for i, text in enumerate(texts) if text.startswith("bt ")]
    unreadable = P.MOUNT_PATCHES_VA + mount.index(P.MOUNT_UNREADABLE)
    ok = len(bt) == 1
    if ok:
        i = bt[0]
        fatal_push = listing[i + 2][0]
        ok = (texts[i] == f"bt [edx+{bitmap_va:#x}],ecx" and texts[i - 2] == "mov edx,[esp]"
              and texts[i - 1] == "mov ecx,[esp+0x8]" and texts[i + 1].startswith("jnc ")
              and texts[i - 4] in ("cmp eax,byte +0x3", "cmp eax,0x3"))
        ok = ok and texts[i - 3] == f"jnz {fatal_push:#x}" and texts[i + 2] == "push eax" \
            and texts[i + 5] == f"lea eax,[edx+{unreadable:#x}]" and texts[i + 7] == "call 0x113580"
    check(ok, "the probe tests a missing patch's n (error 2 or 3) with `bt` on the bitmap plus the delta; a set bit "
          "goes on to the \"Cannot open resources.db.patch_%u (error %u)\" fatal as an unreadable file, a clear one "
          "to the next n" + ("" if ok else "\n          " + "\n          ".join(texts[bt[0] - 6:bt[0] + 9] if bt else texts)))
    checksum_at = P.Headers(default).checksum_at
    bitmap_at = image.offset(bitmap_va)
    for numbers in ((1, 12, 14, 17, 255), (12, 14, 17, *range(19, 28))):
        data, description = P.build(original, "section", upload, server, code, required_patches=numbers)
        changed = [i for i in range(max(len(data), len(default)))
                   if i >= len(data) or i >= len(default) or data[i] != default[i]]
        bitmap = Image(data).read(bitmap_va, P.MOUNT_REQUIRED_SIZE)
        names = ", ".join(map(str, numbers))
        check(len(data) == len(default) and bitmap == P.required_bitmap(numbers)
              and all(bitmap[n >> 3] >> (n & 7) & 1 == (n in numbers) for n in range(256))
              and P.required_patches_of(bytes(data)) == list(numbers)
              and lib_required_patches(data) in (" ".join(map(str, numbers)), None)
              and all(checksum_at <= i < checksum_at + 4 or bitmap_at <= i < bitmap_at + P.MOUNT_REQUIRED_SIZE
                      for i in changed)
              and description.endswith(f"without resources.db.patch_{', _'.join(map(str, numbers))}"
                                       + ("" if server is None else f", default login server {server}")),
              f"--required-patches={','.join(map(str, numbers))}: the bitmap has bit n of byte n/8 for {names}; "
              f"besides it only the PE checksum differs ({len(changed)} bytes); patch_026g.py and release/lib.sh "
              "read the list back")
    for numbers, late_join in (((0,), code), ((256,), code), ((12, 12), code), ((12,), None)):
        try:
            P.build(original, "section", upload, server, late_join, required_patches=numbers)
            refused = False
        except SystemExit:
            refused = True
        check(refused, f"--required-patches={','.join(map(str, numbers))}"
              f"{'' if late_join else ' without the late join'} is refused")
    for text in ("14,12", "12,,14", "012", "", "12, 14"):
        try:
            P.parse_required_patches(text)
            refused = False
        except SystemExit:
            refused = True
        check(refused, f"--required-patches={text!r} is refused (increasing whole numbers, commas between)")


def lib_required_patches(data):
    """release/lib.sh's exe_required_patches of the exe `data`: what the
    release reads back from a built exe; None where there is no bash (it
    is read on the release's Linux machine)."""
    if shutil.which("bash") is None or os.name == "nt":
        return None
    path = os.path.join(SCRATCH, f"required-patches.{os.getpid()}.exe")   # one per part (main's)
    with open(path, "wb") as out:
        out.write(data)
    lib = os.path.join(HERE, "..", "..", "release", "lib.sh")
    result = subprocess.run(["bash", "-c", '. "$1" && exe_required_patches "$2"', "-", lib, path],
                            capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else f"failed: {result.stderr.strip()}"


ABI_TSV = os.path.join(HERE, "..", "..", "tools", "re-kit", "v026g", "abi.tsv")


def abi_cleanup():
    """{VA: ret_cleanup} of every re-kit abi.tsv row with one (its number of
    bytes popped, or -1 for several). The models' call shapes come from here
    (the re-kit is the source, not hand-written notes); 0.26g never rebases,
    so a VA is an address."""
    out = {}
    with open(ABI_TSV, newline="") as f:
        for row in csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE):
            cleanup = row["ret_cleanup"]
            if cleanup not in ("", "None"):
                values = {int(c) for c in re.split(r"[ ,;/]+", cleanup.strip()) if c}
                out.setdefault(int(row["va"], 16), -1 if len(values) != 1 else values.pop())
    return out


def effective_cleanup(image, va):
    """The bytes the function at `va` pops (function_pops), or None when its
    rets differ, none is found or a branch could not be followed."""
    if va is None:
        return None
    found, lost = function_pops(image, va)
    return found.pop() if len(found) == 1 and not lost else None


_ORIGINALS = {}


def model_cleanup(original, image, model, abi):
    """The bytes the model pops, read from the original exe; when re-kit
    abi.tsv has the model's row, its ret_cleanup must say the same (the
    reviewed table and the bytes agree, or the check fails: a wrong
    hand-written note is how Research's slot 16 went wrong). (cleanup or None,
    where it came from)."""
    source = _ORIGINALS.setdefault(hashlib.sha256(original).hexdigest(), Image(original))
    read = effective_cleanup(source, model)
    if abi.get(model, -1) >= 0:
        if abi[model] != read:
            return None, f"abi.tsv says {abi[model]}, the original exe {read}"
        return read, "the original exe, as abi.tsv"
    return read, "the original exe; no abi.tsv row"


def callee_pushes(code):
    """The bytes the pushes before a replaced `call` pushed (`push r32`,
    `push imm8`, `push imm32`: four each); None for anything else."""
    pushed, at = 0, 0
    while at < len(code):
        if 0x50 <= code[at] <= 0x57:
            at += 1
        elif code[at] == 0x6A:
            at += 2
        elif code[at] == 0x68:
            at += 5
        else:
            return None
        pushed += 4
    return pushed if at == len(code) else None


# Every site test_call_shapes saw installed, over every variant built: an
# INSTALLS row no variant installs is stale (main checks at its end).
INSTALLED_SEEN = set()


def test_call_shapes(original, data, report=None):
    """Every function a patch makes the game call is declared (INSTALLS), and
    every one the game calls through a pointer our code hands it (a vtable
    slot, a delegate, a callback) pops off the stack exactly what the game's
    own function in its place does: the self-test reads our function's `ret`
    from the built bytes and the model's from re-kit abi.tsv or the original
    exe, and fails on a mismatch. This is the check that would have caught
    Research's rule slot 16 (`ret 4` where the game's rules `ret 8`),
    client_patch.md section 0.4. `report(ok, text)` defaults to the self-test's
    `check`; the negative test passes its own to capture without printing."""
    report = check if report is None else report
    check_, globals()["check"] = check, report      # the sub-checks below use `check`
    try:
        return _call_shapes_body(original, data)
    finally:
        globals()["check"] = check_


def _call_shapes_body(original, data):
    print("  call shapes (every installed function's model)")
    image = Image(data)
    installs = P.installs_of(data, P.WRITES)
    # 1. Every call/jmp/pointer into our code is declared, with a fitting kind.
    try:
        P.audit_installs(data, P.WRITES)
        audited = True
    except SystemExit as exit:
        audited = False
        print(f"        {exit}")
    check(audited and all(va in P.INSTALLS for va in installs),
          f"every one of the {len(installs)} functions the game calls of ours is declared in INSTALLS")
    INSTALLED_SEEN.update(installs)
    # 2. Each function the game calls of ours pops what its model does: a
    # slot's (the function the original's pointer named), an entry's (the
    # function at the site), a callee's (the replaced call's target, less the
    # bytes of the pushes replaced with it), an ABI's (its declared pops),
    # each run-time callback's. Ours is read from the built bytes, every
    # `ret` reachable from its entry (function_pops); the model's from the
    # original exe (abi.tsv agreeing where it has the row).
    abi = abi_cleanup()
    source = Image(original)
    for site, (how, target, old) in sorted(installs.items()):
        kind, what, pops, _ = P.INSTALLS[site]
        if kind in (P.JUMP, P.INLINE):
            continue
        if kind == P.SLOT:
            model = struct.unpack("<I", source.read(site, 4))[0]
            want, whence = model_cleanup(original, image, model, abi)
            whence = f"its model {model:#x}, {whence}"
        elif kind == P.ENTRY:
            want, whence = model_cleanup(original, image, site, abi)
            whence = f"the original's function at the site, {whence}"
        elif kind == P.CALLEE:
            model = (site + len(old) + struct.unpack_from("<i", old, len(old) - 4)[0]) & 0xFFFFFFFF
            pushed = callee_pushes(old[:-5])
            want, whence = model_cleanup(original, image, model, abi)
            whence = f"its model {model:#x}, {whence}" + (f", less the {pushed} bytes pushed at the site"
                                                          if pushed else "")
            want = None if want is None or pushed is None else want - pushed
        else:                                              # ABI
            want, whence = pops, "declared"
        found, lost = function_pops(image, target)
        ours = found.pop() if len(found) == 1 and not lost else None
        check(want is not None and ours == want,
              f"{site:#x} {kind} ({what}): ours ({target:#x}) pops {ours if ours is not None else sorted(found)}, "
              f"as it must: {want} ({whence})" + (f"; not followed: {lost[:2]}" if lost else ""))
    callbacks = []
    linked = P.arena()
    for what, name, before_bytes, model, _why, driven in P.CALLBACKS:
        code = P.linker.block_bytes(linked, name) if name in linked.blocks \
            else open(os.path.join(HERE, f"{name}_026g.section.bin"), "rb").read()
        at = code.find(before_bytes)
        got = -1 if at < 0 or code.find(before_bytes, at + 1) >= 0 \
            else struct.unpack_from("<I", code, at + len(before_bytes))[0]
        callbacks.append((got, what, model, driven))
    for our_va, what, model, driven in callbacks:
        want, whence = model_cleanup(original, image, model, abi)
        ours = effective_cleanup(image, our_va) if our_va not in (None, -1) else None
        check(want is not None and ours == want,
              f"{what}: `ret {ours}` pops what its model {model:#x} does (`ret {want}`, {whence})")
        check_driven(f"{what}", driven)
    # 3. Every vtable our code builds (a rule's, a cook's): each slot of ours
    # pops what the game's vtable pops in the same place (check 53, for
    # every patch).
    check_vtables(original, image, P.INSTALLED_VTABLES, P.arena_symbols(linked))


def made_up(folder, name, text):
    """A made-up block's NASM source in `folder`: its path."""
    path = os.path.join(folder, name)
    with open(path, "w") as out:
        out.write("bits 32\n" + text)
    return path


def check_vtables(original, image, vtables, symbols):
    """Each declared vtable's slots against its model's: a slot that is the
    game's own function is its model; a slot of ours must pop (its first
    `ret` before the next symbol of its code) what the model's slot pops; a
    vtable without a model must hold only the game's functions. The number of
    our own slots is the declaration's (a slot that changes owner is seen)."""
    source = Image(original)
    abi = abi_cleanup()
    read32 = lambda img, va: struct.unpack("<I", img.read(va, 4))[0]
    in_text = lambda va: P.TEXT_VA <= va < P.TEXT_VA + P.TEXT_VSIZE
    for what, json_name, symbol, model, slots, own, driven in vtables:
        table = symbols[json_name]
        ours_at = table[symbol] + 4                        # the RTTI locator, then the slots
        compared = []
        for slot in range(slots):
            ours = read32(image, ours_at + 4 * slot)
            theirs = read32(source, model + 4 * slot) if model is not None else None
            if in_text(ours) and (model is None or ours == theirs):
                continue                                   # the game's own function
            compared.append(slot)
            if model is None:
                check(False, f"{what} vtable slot {slot}: {ours:#x} is ours, but the vtable has no model")
                continue
            mine, lost = function_pops(image, ours)
            want, whence = model_cleanup(original, image, theirs, abi)
            check(want is not None and mine == {want} and not lost,
                  f"{what} vtable slot {slot}: ours ({ours:#x}) pops {sorted(mine)} bytes, the model's "
                  f"({theirs:#x}, vtable {model:#x}) {want} ({whence})"
                  + (f"; not followed: {lost[:2]}" if lost else ""))
            check_driven(f"{what} vtable slot {slot}", driven.get(slot))
        check(len(compared) == own,
              f"{what} vtable ({symbol}): {own} slots of ours, each compared with "
              + (f"vtable {model:#x}'s ({compared})" if model is not None else "none (the game's functions only)"))


def check_driven(what, driven):
    """A function of ours the game calls through a pointer (a vtable slot,
    a callback) has a case of its patch's Unicorn check that reaches it
    through the game's own caller (INSTALLED_VTABLES' and CALLBACKS' last
    field): the `ret` comparison cannot see the order of the arguments
    (Research's slot 7, #297's review). The case must be the label of a
    case of its script: the text of a string argument of a `check(`,
    `say(` or `print(` call there (case_labels), not a comment or a
    docstring; the script runs it (a patch's check is run with the patch)."""
    script, case = driven if driven else (None, None)
    path = os.path.join(HERE, script) if script else ""
    labels = case_labels(path) if script and os.path.exists(path) else []
    check(bool(case) and any(case in label for label in labels),
          f"{what}: reached through the game's own caller in {script}: \"{case}\"" if case
          else f"{what}: no case of its check reaches it through the game's own caller (declare one)")


def case_labels(path):
    """The labels of a check script's cases: every string argument of its
    `check(`, `say(` and `print(` calls (an f-string's literal parts with
    `{}` for each field; `a + b` of strings joined), parsed, so a comment or
    a docstring is no case."""
    def text(node):
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return node.value
        if isinstance(node, ast.JoinedStr):
            return "".join(text(v) if isinstance(v, ast.Constant) else "{}" for v in node.values)
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
            left, right = text(node.left), text(node.right)
            return left + right if left is not None and right is not None else left or right
        if isinstance(node, ast.IfExp):
            return text(node.body)
        return None
    out = []
    for node in ast.walk(ast.parse(open(path).read())):
        if isinstance(node, ast.Call):
            name = getattr(node.func, "id", None) or getattr(node.func, "attr", None)
            if name in ("check", "say", "print"):
                out += [label for label in map(text, node.args) if label]
    return out


def test_lohook_call_shapes():
    """The continuation ret must not mask a bad native exit or a broken shim."""
    h = {"entry": "generated", "shim": "fake_shim", "site": 0x1000,
         "original": b"\x90" * 5, "exits": [("done", 0x1100)]}
    lines = P.hooks_026g.lohook_entry(h)
    # Flat NASM defines the shim locally; the generator emits ELF externs.
    lines = [line for line in lines if not line.startswith(("global ", "extern "))]
    asm = "bits 32\norg 0x1000\njmp generated\nret 8\ntimes 0x100-($-$$) db 0x90\nret 8\n"
    asm += "\n".join(lines) + "\nfake_shim: ret\n"
    with tempfile.TemporaryDirectory(prefix="srp-lohook-flow-") as folder:
        source, output = os.path.join(folder, "flow.asm"), os.path.join(folder, "flow.bin")
        with open(source, "w") as stream:
            stream.write(asm)
        subprocess.run([tool("nasm"), "-f", "bin", source, "-o", output], check=True)
        code = open(output, "rb").read()
    class Bytes:
        def __init__(self, code): self.code = code + b"\xcc" * 0x400
        def read(self, va, size):
            if not 0x1000 <= va < 0x1000 + len(self.code): raise ValueError(va)
            return self.code[va-0x1000:va-0x1000+size]
    check(function_pops(Bytes(code), 0x1000) == ({8}, []),
          "lohook dispatch ret follows replay/skip/exit to the native ret 8")
    bad = bytearray(code); bad[0x101] = 4
    found, lost = function_pops(Bytes(bad), 0x1000)
    check(found == {4, 8} and not lost, "a wrong native exit behind a lohook is still rejected")
    tail = code.index(bytes.fromhex("8955240fae0f89ec619dc3")) + 10
    bad = bytearray(code); bad[tail:tail+3] = bytes.fromhex("c20800")
    _, lost = function_pops(Bytes(bad), 0x1000)
    check(bool(lost), "a lohook dispatch ret 8 cannot masquerade as the enclosing function's ret 8")


def test_call_shapes_negative(original):
    """The call-shape check rejects a wrong `ret` and an undeclared install:
    a fake build whose one installed function pops the wrong number of bytes,
    and one with a call into our code that INSTALLS does not name. It proves
    the check is live (Research's slot 16 was `ret 4` where the model is
    `ret 8`)."""
    test_lohook_call_shapes()
    print("  call shapes: the check rejects a wrong ret and an undeclared install")
    image = Image(original)
    # A made-up rule vtable in made-up `.srpx` blocks (after the arena's own,
    # linked from NASM sources): tdm_rule_core's twenty slots (vtable
    # 0xA15DA4), but slot 16, the deserialize, ours and `ret 4` where tdm's
    # pops 8: Research's first rn_deserialize. (Named `_slots`, not
    # `_vtable`: build() would refuse an undeclared vtable before the check.)
    read32 = lambda va: struct.unpack_from("<I", original, image.offset(va))[0]
    model = read32(0xA15DA4 + 4 * 16)
    want = popped(image, model, model + 0x400)
    code = open(P.default_bin("section"), "rb").read()
    late_join = open(P.late_join_bin(), "rb").read()
    slots = [f"{read32(0xA15DA4 + 4 * i):#x}" for i in range(20)]
    slots[16] = "fake_deserialize"
    for wrong, name in ((bytes.fromhex("c20400"), "pops 4"), (bytes.fromhex("c20800"), "pops 8")):
        with tempfile.TemporaryDirectory(prefix="srp-fake-") as folder:
            blocks = (P.ArenaBlock("fake.code", "code", 0x1000, (made_up(folder, "fake_code.asm", (
                          "section .text\nglobal fake_deserialize\nfake_deserialize:\n    db "
                          + ", ".join(f"{b:#x}" for b in wrong) + "\n")),)),
                      P.ArenaBlock("fake.table", "data", 0x1000, (made_up(folder, "fake_table.asm", (
                          "section .data\nglobal fake_rule_slots\nextern fake_deserialize\nfake_rule_slots:\n"
                          "    dd 0, " + ", ".join(slots) + "\n")),)))
            data, _ = P.build(original, "section", code, None, late_join, arena_blocks=P.ARENA + blocks)
            symbols = P.arena_symbols(P.arena(P.ARENA + blocks))
        table_va = symbols["fake.table"]["fake_rule_slots"]
        captured = []
        saved = globals()["check"]
        globals()["check"] = lambda ok, text: captured.append((ok, text))
        try:
            check_vtables(original, Image(data), (("a made-up rule", "fake.table", "fake_rule_slots",
                                                     0xA15DA4, 20, 1,
                                                     {16: (P.RESEARCH_CHECK,
                                                           "the game's deserialize loop (0x2D2B42)")}),),
                          symbols)
        finally:
            globals()["check"] = saved
        failed = [text for ok, text in captured if not ok]
        if name == "pops 4":
            check(want == 8 and any("slot 16" in text for text in failed),
                  f"a rule vtable slot 16 that pops 4 where tdm_rule_core's ({model:#x}) pops {want} is rejected "
                  "(Research's first rn_deserialize)")
        else:
            check(not failed, "... the same slot popping 8 passes")
    # A vtable symbol of the arena's code with no declaration stops the build.
    try:
        P.audit_vtables(vtables=(), symbols={"fake.table": {"fake_rule_vtable": table_va}})
        refused = False
    except SystemExit:
        refused = True
    check(refused, "a `*_vtable` of the arena's code with no INSTALLED_VTABLES row is refused")
    # A slot of ours with no case through the game's caller, or with a case
    # its script does not have, is refused.
    for driven, name in (({}, "no case"), ({16: (P.RESEARCH_CHECK, "no such case of the check")}, "a missing case")):
        captured = []
        saved = globals()["check"]
        globals()["check"] = lambda ok, text: captured.append((ok, text))
        try:
            check_vtables(original, Image(data), (("a made-up rule", "fake.table", "fake_rule_slots",
                                                     0xA15DA4, 20, 1, driven),), symbols)
        finally:
            globals()["check"] = saved
        check(any(not ok and "caller" in text for ok, text in captured),
              f"a slot of ours with {name} through the game's own caller is refused")
    # A case named only in a comment or a docstring is no case.
    with tempfile.TemporaryDirectory(prefix="srp-driven-") as folder:
        fake = os.path.join(folder, "check_fake.py")
        open(fake, "w").write('def cases():\n    """driven: a docstring\'s case"""\n'
                              '    # driven: a comment\'s case\n    check(True, "driven: a real case")\n')
        captured = []
        saved = globals()["check"]
        globals()["check"] = lambda ok, text: captured.append(ok)
        try:
            for case in ("driven: a docstring's case", "driven: a comment's case", "driven: a real case"):
                check_driven("a made-up slot", (fake, case))
        finally:
            globals()["check"] = saved
        check(captured == [False, False, True],
              "a case named only in a comment or a docstring is refused; a `check(` label passes")
    # An undeclared install: a call into our code with no INSTALLS row.
    fake_writes = [(0x500000, bytes(5), b"\xE8" + struct.pack("<i", P.SECTION_VA - 0x500000 - 5))]
    try:
        P.audit_installs(data, fake_writes, installs={})
        refused = False
    except SystemExit:
        refused = True
    check(refused, "a call into our code with no INSTALLS row is rejected")
    # ... and one written anywhere in a write: a `push imm32` of our code's
    # address in the middle of rewritten bytes.
    fake_writes = [(0x500000, bytes(8), b"\x90\x90\x68" + struct.pack("<I", P.SECTION_VA + 0x10) + b"\x90")]
    found = P.installs_of(data, fake_writes)
    try:
        P.audit_installs(data, fake_writes, installs={})
        refused = False
    except SystemExit:
        refused = True
    check(refused and list(found) == [0x500003] and found[0x500003][0] == "pointer",
          "a `push imm32` of our code's address inside a write (at +3) is found and, undeclared, rejected")


def link_refusal(blocks, tools=None, lock=None):
    """The text the arena of `blocks` (ARENA's when None, compared with
    `lock` when given) is refused with; None when it links."""
    saved = P.linker.read_lock
    if lock is not None:
        P.linker.read_lock = lambda: lock
    try:
        P.arena(blocks, P.linker.toolchain(**tools) if tools else None)
    except (SystemExit, P.linker.LinkError) as stop:
        return str(stop)
    finally:
        P.linker.read_lock = saved
    return None


def test_arena(original):
    """`.srpx`, the arena of the later patches (client_patch.md section 0.3),
    linked from its blocks' sources (link_026g.py). Deterministic, in
    ARENA's order, aligned, none overlapping, every block inside the
    section, code gaps int3, state zeros, a symbol of one block used by
    another, the headers and the PE checksum right; ARENA's blocks in another
    order link and work (the hooks and the library's calls follow the
    symbols, the library's check passes there); with no block there is no
    `.srpx`. Refused: a symbol no source defines, a relocation the arena
    does not link, a section no block takes, a block past ARENA_LIMIT, a
    missing tool, NASM 3, a hook symbol the link lacks, and an arena that is
    not the lock's."""
    print("  the arena (.srpx)")
    code = open(P.default_bin("section"), "rb").read()
    late_join = open(P.late_join_bin(), "rb").read()
    without, _ = P.build(original, "section", code, None, late_join)
    none, _ = P.build(original, "section", code, None, late_join, arena_blocks=())
    check([s[0] for s in Image(none).headers.sections(none)] == [s[0] for s in Image(without).headers.sections(without)][:-2]
          and [s[0] for s in Image(without).headers.sections(without)[-2:]] == [P.ARENA_NAME, P.ARENA_DATA_NAME]
          and bytes(P.add_arena(bytearray(b"MZ"), None)) == b"MZ",
          "with no block at all there is neither `.srpx` nor `.srpxd` (add_arena adds no section)")
    with tempfile.TemporaryDirectory(prefix="srp-arena-") as folder:
        # Blocks of each kind, out of alignment order, after ARENA's own: a
        # code block whose source has state too, a data block pointing at
        # it, a code block calling it.
        a = made_up(folder, "a_part.asm", "section .text\nglobal a_entry\na_entry:\n    times 16 nop\n    ret\n"
                    "section .bss\nglobal a_state\na_state:\n    resb 0x40\n")
        b = made_up(folder, "b_part.asm", "extern a_entry, a_state\nsection .data\nglobal b_data\nb_data:\n"
                    "    db \"hello\", 0\n    dd a_entry, a_state\nsection .text\nglobal b_entry\nb_entry:\n"
                    "    call a_entry\n    ret\n")
        blocks = (P.ArenaBlock("a.code", "code", 0x1000, (a,)), P.ArenaBlock("a.state", "state", 0x1000, (a,)),
                  P.ArenaBlock("b.data", "data", 0x10, (b,)), P.ArenaBlock("b.code", "code", 0x40, (b,)))
        one, _ = P.build(original, "section", code, None, late_join, arena_blocks=P.ARENA + blocks)
        P.linker._links.clear()
        P.linker._objects.clear()
        two, _ = P.build(original, "section", code, None, late_join, arena_blocks=P.ARENA + blocks)
        linked = P.arena(P.ARENA + blocks)
    check(one == two, "the arena is byte for byte deterministic (linked twice from its sources, the same exe)")
    image = Image(one)
    sections = image.headers.sections(one)
    name, rva, vsize, raw, rsize, flags = sections[-2]
    name_d, rva_d, vsize_d, raw_d, rsize_d, flags_d = sections[-1]
    align = lambda v, to: (v + to - 1) // to * to
    prev = sections[-3]
    check(name == P.ARENA_NAME and flags == P.ARENA_CHARACTERISTICS == 0x60000020
          and P.IMAGE_BASE + rva == P.ARENA_VA and rva == align(prev[1] + prev[2], image.headers.section_alignment)
          and name_d == P.ARENA_DATA_NAME and flags_d == P.ARENA_DATA_CHARACTERISTICS == 0xC0000040
          and P.IMAGE_BASE + rva_d == linked.data_va == P.IMAGE_BASE + align(rva + vsize, 0x1000),
          f".srpx at {P.ARENA_VA:#x}: code | execute | read, on the page after .srpc; .srpxd at {linked.data_va:#x}: "
          "initialized data | read | write, on the page after .srpx")
    # Every block inside, aligned, the code blocks in ARENA's order in .srpx,
    # then the data and state blocks in ARENA's order in .srpxd, none
    # overlapping the next.
    order = [linked.blocks[b.name] for b in P.linker.placed(P.ARENA + blocks)]
    home = lambda b: (rva, vsize) if b.kind == "code" else (rva_d, vsize_d)
    inside = all(home(b)[0] <= b.va - P.IMAGE_BASE and b.va + b.size - P.IMAGE_BASE <= sum(home(b)) for b in order)
    aligned = all(linked.blocks[b.name].va % b.align == 0 for b in P.ARENA + blocks)
    in_order = all(x.va + x.size <= y.va for x, y in zip(order, order[1:]))
    code_order = [b.name for b in order if b.kind == "code"]
    check(inside and aligned and in_order and order[0].va == P.ARENA_VA
          and code_order == [b.name for b in P.ARENA + blocks if b.kind == "code"]
          and [b.name for b in order[len(code_order):]] == [b.name for b in P.ARENA + blocks if b.kind != "code"]
          and P.IMAGE_BASE + rva + vsize == max(b.va + b.size for b in order if b.kind == "code")
          and P.IMAGE_BASE + rva_d + vsize_d == P.ARENA_VA + linked.size == order[-1].va + order[-1].size,
          f"the {len(order)} blocks are aligned and do not overlap, the code blocks in ARENA's order inside .srpx "
          f"(VirtualSize {vsize:#x}, to the last code block's end), then the data and state blocks in ARENA's order "
          f"inside .srpxd (VirtualSize {vsize_d:#x}, to the arena's end) "
          f"({', '.join(f'{b.name} @ {b.va:#x}' for b in order)})")
    # The bytes: each code block's bytes, then int3 to the next block (the
    # last to .srpxd's page); data and state blocks and their gaps zeros; a
    # reference across blocks resolved by the link.
    at = lambda va: va - P.ARENA_VA
    body = bytes(one[raw:raw + vsize]) + b"\xCC" * (at(linked.data_va) - vsize) + bytes(one[raw_d:raw_d + vsize_d])
    gaps = all(set(body[at(x.va + x.size):at(y.va)]) <= ({0xCC} if x.kind == "code" else {0})
               for x, y in zip(order, order[1:]))
    states = all(not any(body[at(b.va):at(b.va + b.size)]) for b in order if b.kind == "state")
    s = linked.symbols
    b_data = body[at(s["b_data"]):at(s["b_data"]) + 14]
    call = body[at(s["b_entry"]):at(s["b_entry"]) + 5]
    check(gaps and states and body == linked.image
          and body[at(s["a_entry"]):at(s["a_entry"]) + 17] == b"\x90" * 16 + b"\xc3"
          and b_data == b"hello\0" + struct.pack("<II", s["a_entry"], s["a_state"])
          and call[0] == 0xE8 and s["b_entry"] + 5 + struct.unpack_from("<i", call, 1)[0] == s["a_entry"]
          and linked.blocks["a.state"].size == 0x40 and s["a_state"] == linked.blocks["a.state"].va,
          "each code block's bytes and int3 to the next block, data and state blocks zeros past their bytes; "
          "a.state is a_part.asm's .bss (0x40 bytes); b.data's pointers and b.code's call reach a.code's "
          "a_entry and a.state's a_state through the link")
    # The headers and the PE checksum, as test_pe checks them for the sections.
    get = lambda at: struct.unpack_from("<I", one, at)[0]
    checksum = P.pe_checksum(one, image.headers.checksum_at)
    security = image.directory(4)
    check(get(image.headers.size_of_image_at) == align(rva_d + vsize_d, image.headers.section_alignment)
          and raw == prev[3] + prev[4] and rsize == align(vsize, image.headers.file_alignment)
          and raw_d == raw + rsize and rsize_d == align(vsize_d, image.headers.file_alignment)
          and get(image.headers.checksum_at) == checksum
          and security[0] == raw_d + rsize_d and security[0] + security[1] == len(one),
          "SizeOfImage ends past .srpxd, .srpx's file bytes follow .srpc's and .srpxd's .srpx's, the certificate "
          "table stays last, the PE checksum is right")
    # ARENA's blocks in another order (research.code and artefacts.code
    # swapped): every block elsewhere, and still each hook at its symbol,
    # Research's calls at the library's thunks and the library's check passing.
    names = [b.name for b in P.ARENA]
    swapped = list(P.ARENA)
    i, j = names.index("research.code"), names.index("artefacts.code")
    swapped[i], swapped[j] = swapped[j], swapped[i]
    data, _ = P.build(original, "section", code, None, late_join, arena_blocks=tuple(swapped))
    moved = P.arena(tuple(swapped))
    default = P.arena()
    built = Image(data)
    hooks = [(va, symbol, branch_target(ndisasm(built.read(va, 5), va)[0][2]))
             for va, _, symbol, _ in P.RESEARCH_HOOKS + P.ARTEFACTS_HOOKS]
    research_va = moved.blocks["research.code"].va
    listing = [t for _, _, t in ndisasm(P.linker.block_bytes(moved, "research.code")[P.RESEARCH_ASM_SIZE:],
                                        research_va + P.RESEARCH_ASM_SIZE)]
    calls = {s for s in moved.symbols if s.startswith("engine_") and any(
        t == f"call {moved.symbols[s]:#x}" or t.endswith(f",{moved.symbols[s]:#x}") for t in listing)}
    check(moved.blocks["artefacts.code"].va == default.blocks["research.code"].va != default.blocks["artefacts.code"].va
          and moved.blocks["research.code"].va != default.blocks["research.code"].va
          and all(target == moved.symbols[symbol] != default.symbols[symbol] for _, symbol, target in hooks)
          and len(calls) >= 11 and P.artefacts_backs(moved) == list(P.ARTEFACTS_BACKS)
          and built.read(moved.blocks["research.code"].va, 0x100) == P.linker.block_bytes(moved, "research.code")[:0x100],
          f"ARENA with research.code and artefacts.code swapped links (artefacts.code at "
          f"{moved.blocks['artefacts.code'].va:#x}, research.code at {research_va:#x}): the {len(hooks)} hooks "
          f"reach their symbols there, Research's C calls {len(calls)} library functions at their new thunks, "
          "the artefacts' entries go on where they came from")
    try:
        import check_engine_026g as engine_check
        bad = [text for ok, text in engine_check.run_checks(original, (
            moved.blocks["engine.code"].va, P.linker.block_bytes(moved, "engine.code"),
            {k: v for k, v in moved.symbols.items() if k.startswith("engine_")}))
            if not ok]
    except ImportError as error:
        bad = [f"check_engine_026g.py needs Python unicorn ({error})"]
    check(not bad, "... and check_engine_026g.py passes on the library there" + (": " + "; ".join(bad) if bad else ""))
    # What the link refuses, loudly.
    with tempfile.TemporaryDirectory(prefix="srp-arena-") as folder:
        cases = [
            ("a symbol no source defines", "nowhere_026g", (P.ArenaBlock("lost.code", "code", 16, (made_up(
                folder, "lost.asm", "extern nowhere_026g\nsection .text\n    call nowhere_026g\n"),)),)),
            ("a 16-bit address (R_386_16)", "R_386_16", (P.ArenaBlock("narrow.code", "code", 16, (made_up(
                folder, "narrow.asm", "section .text\nnarrow:\n    dw narrow\n"),)),)),
            ("a block past the arena's limit (ARENA_LIMIT)", "past its limit", (P.ArenaBlock(
                "huge.state", "state", 16, (made_up(folder, "huge.asm", f"section .bss\n    resb {P.ARENA_LIMIT:#x}\n"),)),)),
            ("a symbol defined in two sources", "multiple definition", tuple(
                P.ArenaBlock(f"twice{n}.code", "code", 16, (made_up(folder, f"twice{n}.asm",
                             "section .text\nglobal research_hud\nresearch_hud:\n    ret\n"),)) for n in (1, 2))),
        ]
        c_data = os.path.join(folder, "counter.c")
        with open(c_data, "w") as out:
            out.write("int counter = 1;\nint count(void) { return ++counter; }\n")
        cases.append(("a C source's initialized data (.data) in a code block only", "no data block",
                      (P.ArenaBlock("counter.code", "code", 16, (c_data,)),)))
        for what, word, extra in cases:
            text = link_refusal(P.ARENA + extra)
            said = next((line.strip() for line in (text or "linked").splitlines() if word in line), text or "linked")
            check(text is not None and word in text, f"the link refuses {what}: {said[:160]}")
        text = link_refusal(None, {"cc": os.path.join(folder, "no-such-cc")})
        check(text is not None and "no-such-cc" in text, f"a missing compiler is refused: {(text or 'linked')[:160]}")
        if os.name == "posix":
            fake = os.path.join(folder, "nasm")
            with open(fake, "w") as out:
                out.write("#!/bin/sh\necho 'NASM version 3.01 compiled on Oct 11 2025'\n")
            os.chmod(fake, 0o755)
            text = link_refusal(None, {"nasm": fake})
            check(text is not None and "NASM 2.16" in text, f"NASM 3 is refused: {(text or 'linked')[:160]}")
    try:
        P.arena_target(default, "nowhere_026g")
        text = None
    except SystemExit as stop:
        text = str(stop)
    check(text is not None and "nowhere_026g" in text, "a hook whose symbol the link lacks stops the build")
    lock = P.linker.read_lock()
    text = link_refusal(None, lock=dict(lock, sha256="0" * 64))
    check(lock is not None and text is not None and P.linker.LOCK.name in text,
          f"an arena whose bytes are not the lock's stops the build: {(text or 'built')[:160]}")


def test_apply(original):
    """The build's one writer (patch_026g.py apply(), rust_patches.md "The
    build") and the general rules (test_writes), each shown live: apply()
    refuses an overlap, a write in no named instance and a site whose bytes
    are not the original's; test_writes refuses a byte changed undeclared
    and a write whose changed bytes a branch of the original aims inside."""
    saved = (list(P.APPLIED), P._BUILDING[0])
    try:
        P.APPLIED.clear()
        P._BUILDING[0] = True
        scratch = bytearray(original)
        va, old, new = P.TAB_SELF
        P.patch(scratch, va, old, new)
        try:
            P.apply(scratch, P.file_offset(va), va, new, old, owner="a second patch")
            overlap = False
        except SystemExit:
            overlap = True
        try:
            P.instance_of(0x401000)
            unowned = False
        except SystemExit:
            unowned = True
    finally:
        P.APPLIED[:] = saved[0]
        P._BUILDING[0] = saved[1]
    bad = bytearray(original)
    bad[P.file_offset(P.MEMORY_CHECK[0])] ^= 1
    kept = bytes(bad)
    try:
        P.patch(bad, *P.MEMORY_CHECK)
        changed = False
    except SystemExit:
        changed = bytes(bad) == kept
    check(overlap and unowned and changed, "apply() refuses a second write over TAB_SELF's bytes, a write in no "
          "named instance, and a site whose bytes are not the original's (leaving them as they were)")
    data, _ = P.build(original, "cert-only")
    applied = list(P.APPLIED)

    def refused(data, applied):
        captured, saved = [], globals()["check"]
        globals()["check"] = lambda ok, text: captured.append((ok, text))
        try:
            test_writes(original, data, applied, "a made-up build")
        finally:
            globals()["check"] = saved
        return [text for ok, text in captured if not ok]
    stray = bytearray(data)
    stray[P.file_offset(P.TEXT_VA + 0x100)] ^= 1
    fake = P.Applied("a made-up patch", "write", 0x2845BF, P.file_offset(0x2845BF), b"\x90" * 4)
    aimed = bytearray(data)
    aimed[fake.at:fake.at + len(fake.new)] = fake.new
    check(not refused(data, applied) and any("declared write" in t for t in refused(stray, applied))
          and any("aims inside" in t for t in refused(aimed, applied + [fake])),
          "test_writes passes the cert-only build, and refuses it with a byte changed undeclared, or with a write "
          "over the start of a function the game calls (0x2845C0)")


def test_engine(original):
    """The engine-function library (engine_026g.py; client_patch.md section
    0.5): check_engine_026g.py's cases, each thunk against the re-kit's rows
    and the original exe, and its negative cases, wrong rows the check must
    reject. Needs Python `unicorn` (as the check_*.py scripts): without it the
    check fails, as a wrong row must never pass unseen."""
    print("  the engine-function library: every thunk against the ABI rows and the exe")
    try:
        import check_engine_026g as engine_check
    except ImportError as error:
        check(False, f"check_engine_026g.py needs Python unicorn ({error})")
        return
    results = engine_check.run_checks(original)
    bad = [text for ok, text in results if not ok]
    check(not bad, f"check_engine_026g.py: {len(results)} cases on the {len(engine_check.engine.functions())} "
          "functions of engine_026g.tsv" + (": " + "; ".join(bad) if bad else ""))
    for what, failed in engine_check.negative(original):
        check(failed, f"the library's check rejects {what}")


def test_hook_kinds(original):
    """The generated hook-entry kinds (hooks_026g.py; rust_patches.md): each
    kind's entry template, assembled and run beside the original exe, is
    transparent (check_hooks_026g.py). Needs Python `unicorn` and NASM/ld."""
    print("  the hook-entry kinds: each template transparent against the exe")
    try:
        import check_hooks_026g as hook_check
    except ImportError as error:
        check(False, f"check_hooks_026g.py needs Python unicorn ({error})")
        return
    results = hook_check.run_checks(original)
    for ok, text in results:
        check(ok, text)
    check(bool(results), f"check_hooks_026g.py: {len(results)} entry-kind checks")
    import io
    import unittest
    output = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromNames(["test_hooks_026g", "test_runtime_hooks_026g", "port.test_equiv_026g"])
    result = unittest.TextTestRunner(stream=output).run(suite)
    check(result.wasSuccessful(), f"the hook generator and equivalence build paths: {result.testsRun} tests"
          + ("" if result.wasSuccessful() else "\n" + output.getvalue()))


def test_late_join(original, without, server, source):
    """The default build with the late join against `without`, the build
    of the same server without it (checked by everything above)."""
    print(f"  the late join on top (patch revision {P.REVISION})")
    code = open(P.late_join_bin(), "rb").read()
    upload = open(P.default_bin("section"), "rb").read()
    data, description = P.build(original, "section", upload, server, code)
    print("  " + description)
    test_call_shapes(original, data)       # while P.WRITES still holds this build's
    image, before = Image(data), Image(without)
    headers = image.headers
    sections, old_sections = image.sections, before.sections
    name, rva, vsize, raw, rsize, flags = sections[-5]
    align = lambda value, to: (value + to - 1) // to * to
    weapon_preview = open(P.weapon_preview_bin(), "rb").read()
    weapon_draw = open(P.weapon_draw_bin(), "rb").read()
    first_page = bytes(without[old_sections[-1][3]:old_sections[-1][3] + old_sections[-1][2]])
    check(len(sections) == 12 and sections[:7] == old_sections[:7], "12 sections; the original's seven are untouched")
    check(name == P.SECTION_NAME and rva == P.SECTION_RVA and flags == P.SECTION_CHARACTERISTICS
          and vsize == P.WEAPON_PREVIEW_OFFSET + len(weapon_preview)
          and rsize == align(vsize, headers.file_alignment) and raw == old_sections[-1][3],
          f".srp: virtual {vsize:#x} = one page, the {len(code)} bytes of late-join code and the weapon preview's "
          f"{len(weapon_preview)} at +{P.WEAPON_PREVIEW_OFFSET:#x}, file {raw:#x}+{rsize:#x}")
    body = bytes(data[raw:raw + rsize])
    seek = open(P.seek_clear_bin(), "rb").read()
    stale_dll = open(P.stale_dll_bin(), "rb").read()
    mount_patches = open(P.mount_patches_bin(), "rb").read()
    forward_scroll = open(P.forward_scroll_bin(), "rb").read()
    kit_change = open(P.kit_change_bin(), "rb").read()
    warmup_look = open(P.warmup_look_bin(), "rb").read()
    warmup_limits = open(P.warmup_limits_bin(), "rb").read()
    warmup_keys = open(P.warmup_keys_bin(), "rb").read()
    # The first page's pieces, in order, none overlapping the next.
    link = open(P.replay_link_bin(), "rb").read()
    shared = open(P.shared_upgrades_bin(), "rb").read()
    spot = open(P.spot_bin(), "rb").read()
    pieces = [(0, len(first_page), "the build without the late join's first page"),
              (P.REPLAY_LINK_OFFSET, len(link), "the replay link code"),
              (P.MOUNT_PATCHES_OFFSET, len(mount_patches), "the patches' mount"),
              (P.SHARED_UPGRADES_OFFSET, len(shared), "the shared upgrades' code"),
              (P.SPOT_OFFSET, len(spot), "the spot code"),
              (P.WARMUP_KEYS_OFFSET, len(warmup_keys), "the warmup keys"),
              (P.SEEK_CLEAR_OFFSET, len(seek), "the seek code"),
              (P.WARMUP_LIMITS_OFFSET, len(warmup_limits), "the warmup camera's limits"),
              (P.FORWARD_SCROLL_OFFSET, len(forward_scroll), "the forward effect's scrolling"),
              (P.WEAPON_DRAW_OFFSET, len(weapon_draw), "the weapon preview's draw"),
              (P.WARMUP_LOOK_OFFSET, len(warmup_look), "the warmup look"),
              (P.STALE_DLL_OFFSET, len(stale_dll), "the shell_32.dll check"),
              (P.KIT_CHANGE_OFFSET, len(kit_change), "the kit change"),
              (P.LATE_JOIN_OFFSET, len(code), "the late-join code"),
              (P.WEAPON_PREVIEW_OFFSET, len(weapon_preview), "the weapon preview"),
              (P.late_join_state_va(code) - P.SECTION_VA, 0, "the state page")]
    overlaps = [f"{a[2]} ends at {a[0] + a[1]:#x}, past {b[2]} at {b[0]:#x}"
                for a, b in zip(pieces, pieces[1:]) if a[0] + a[1] > b[0]]
    check(not overlaps, "the first page's code does not overlap: " + ", ".join(
        f"{name} [{start:#x}, {start + size:#x})" for start, size, name in pieces) + "".join("; " + o for o in overlaps))
    check(body[:len(first_page)] == first_page
          and set(body[len(first_page):P.REPLAY_LINK_OFFSET]) <= {0xCC}
          and body[P.REPLAY_LINK_OFFSET:P.REPLAY_LINK_OFFSET + len(link)] == link
          and set(body[P.REPLAY_LINK_OFFSET + len(link):P.MOUNT_PATCHES_OFFSET]) <= {0xCC}
          and body[P.MOUNT_PATCHES_OFFSET:P.MOUNT_PATCHES_OFFSET + len(mount_patches)] == mount_patches
          and set(body[P.MOUNT_PATCHES_OFFSET + len(mount_patches):P.SHARED_UPGRADES_OFFSET]) <= {0xCC}
          and body[P.SHARED_UPGRADES_OFFSET:P.SHARED_UPGRADES_OFFSET + len(shared)] == shared
          and set(body[P.SHARED_UPGRADES_OFFSET + len(shared):P.SPOT_OFFSET]) <= {0xCC}
          and body[P.SPOT_OFFSET:P.SPOT_OFFSET + len(spot)] == spot
          and set(body[P.SPOT_OFFSET + len(spot):P.WARMUP_KEYS_OFFSET]) <= {0xCC}
          and body[P.WARMUP_KEYS_OFFSET:P.WARMUP_KEYS_OFFSET + len(warmup_keys)] == warmup_keys
          and set(body[P.WARMUP_KEYS_OFFSET + len(warmup_keys):P.SEEK_CLEAR_OFFSET]) <= {0xCC}
          and body[P.SEEK_CLEAR_OFFSET:P.SEEK_CLEAR_OFFSET + len(seek)] == seek
          and set(body[P.SEEK_CLEAR_OFFSET + len(seek):P.WARMUP_LIMITS_OFFSET]) <= {0xCC}
          and body[P.WARMUP_LIMITS_OFFSET:P.WARMUP_LIMITS_OFFSET + len(warmup_limits)] == warmup_limits
          and set(body[P.WARMUP_LIMITS_OFFSET + len(warmup_limits):P.FORWARD_SCROLL_OFFSET]) <= {0xCC}
          and body[P.FORWARD_SCROLL_OFFSET:P.FORWARD_SCROLL_OFFSET + len(forward_scroll)] == forward_scroll
          and set(body[P.FORWARD_SCROLL_OFFSET + len(forward_scroll):P.WEAPON_DRAW_OFFSET]) <= {0xCC}
          and body[P.WEAPON_DRAW_OFFSET:P.WEAPON_DRAW_OFFSET + len(weapon_draw)] == weapon_draw
          and set(body[P.WEAPON_DRAW_OFFSET + len(weapon_draw):P.WARMUP_LOOK_OFFSET]) <= {0xCC}
          and body[P.WARMUP_LOOK_OFFSET:P.WARMUP_LOOK_OFFSET + len(warmup_look)] == warmup_look
          and set(body[P.WARMUP_LOOK_OFFSET + len(warmup_look):P.STALE_DLL_OFFSET]) <= {0xCC}
          and body[P.STALE_DLL_OFFSET:P.STALE_DLL_OFFSET + len(stale_dll)] == stale_dll
          and set(body[P.STALE_DLL_OFFSET + len(stale_dll):P.KIT_CHANGE_OFFSET]) <= {0xCC}
          and body[P.KIT_CHANGE_OFFSET:P.KIT_CHANGE_OFFSET + len(kit_change)] == kit_change
          and set(body[P.KIT_CHANGE_OFFSET + len(kit_change):P.LATE_JOIN_OFFSET]) <= {0xCC}
          and body[P.LATE_JOIN_OFFSET:P.LATE_JOIN_OFFSET + len(code)] == code
          and set(body[P.LATE_JOIN_OFFSET + len(code):P.WEAPON_PREVIEW_OFFSET]) <= {0xCC}
          and body[P.WEAPON_PREVIEW_OFFSET:P.WEAPON_PREVIEW_OFFSET + len(weapon_preview)] == weapon_preview
          and set(body[P.WEAPON_PREVIEW_OFFSET + len(weapon_preview):]) <= {0xCC},
          f"its first page is the build's without the late join ({len(first_page)} bytes, then int3) with the "
          f"replay link code at +{P.REPLAY_LINK_OFFSET:#x}, the patches' mount at +{P.MOUNT_PATCHES_OFFSET:#x}, "
          f"the shared upgrades' code at +{P.SHARED_UPGRADES_OFFSET:#x}, the spot code at +{P.SPOT_OFFSET:#x}, "
          f"the warmup keys at +{P.WARMUP_KEYS_OFFSET:#x}, the seek "
          f"code at +{P.SEEK_CLEAR_OFFSET:#x}, the warmup camera's limits at +{P.WARMUP_LIMITS_OFFSET:#x}, the forward effect's scrolling at +{P.FORWARD_SCROLL_OFFSET:#x}, "
          f"the weapon preview's draw at +{P.WEAPON_DRAW_OFFSET:#x}, the warmup look at +{P.WARMUP_LOOK_OFFSET:#x}, "
          f"the shell_32.dll check at +{P.STALE_DLL_OFFSET:#x} "
          f"and the kit change at +{P.KIT_CHANGE_OFFSET:#x}, the late-join code follows, then int3 padding, "
          f"the weapon preview at +{P.WEAPON_PREVIEW_OFFSET:#x} and int3 padding")
    name, state_rva, state_size, state_raw, state_rsize, flags = sections[-4]
    check(name == P.LATE_JOIN_STATE_NAME and flags == P.LATE_JOIN_STATE_CHARACTERISTICS
          and state_size == P.LATE_JOIN_STATE_SIZE and (state_raw, state_rsize) == (0, 0)
          and state_rva == align(rva + vsize, headers.section_alignment)
          and P.IMAGE_BASE + state_rva == P.late_join_state_va(code),
          f".srpd at VA {P.IMAGE_BASE + state_rva:#x}: one page, uninitialized | read | write, no bytes in the "
          "file, on the page after the code (where the code looks for it)")
    death = P.with_death_pitch(open(P.death_camera_bin(), "rb").read(), P.arena())
    camo = open(P.weapon_camo_bin(), "rb").read()
    camo = P.with_bake_name(camo, len(camo) - 4, P.arena())
    camo = camo + b"\xCC" * (P.WEAPON_STASH_OFFSET - len(camo)) + open(P.weapon_stash_bin(), "rb").read()
    camo = camo + b"\xCC" * (P.WEAPON_TABLE_OFFSET - len(camo)) + open(P.weapon_table_bin(), "rb").read()
    camo = camo + b"\xCC" * (P.WEAPON_DECALS_OFFSET - len(camo)) + open(P.weapon_decals_bin(), "rb").read()
    camo = camo + b"\xCC" * (P.UPGRADE_PREVIEW_OFFSET - len(camo)) \
        + P.with_kit_apply(open(P.upgrade_preview_bin(), "rb").read(), P.arena())
    name_c, rva_c, vsize_c, raw_c, rsize_c, flags_c = sections[-3]
    check(name_c == P.DEATH_CAMERA_NAME and flags_c == P.DEATH_CAMERA_CHARACTERISTICS == 0x60000020
          and rva_c == state_rva + state_size and P.IMAGE_BASE + rva_c == P.death_camera_va(code)
          and P.IMAGE_BASE + rva_c + P.DEATH_CAMERA_SIZE == P.weapon_camo_va(code)
          and vsize_c == P.DEATH_CAMERA_SIZE + len(camo) and raw_c == raw + rsize
          and rsize_c == align(vsize_c, headers.file_alignment)
          and bytes(data[raw_c:raw_c + vsize_c]) == death + b"\xCC" * (P.DEATH_CAMERA_SIZE - len(death)) + camo
          and set(data[raw_c + vsize_c:raw_c + rsize_c]) <= {0xCC},
          f".srpc at VA {P.IMAGE_BASE + rva_c:#x}: death_camera_026g.section.bin ({len(death)} bytes, its pitch_entry "
          "dword patch 112's entry of the arena) and int3 to its page's end, then .srpw's code at "
          f"{P.weapon_camo_va(code):#x}: weapon_camo_026g.section.bin, int3 padding, "
          f"weapon_stash_026g.section.bin at +{P.WEAPON_STASH_OFFSET:#x}, int3 padding and "
          f"weapon_table_026g.section.bin at +{P.WEAPON_TABLE_OFFSET:#x}, int3 padding and "
          f"weapon_decals_026g.section.bin at +{P.WEAPON_DECALS_OFFSET:#x}, int3 padding and "
          f"upgrade_preview_026g.section.bin at +{P.UPGRADE_PREVIEW_OFFSET:#x} ({len(camo)} bytes); "
          "code | execute | read, on the page after .srpd, its file bytes right after .srp's")
    linked = P.arena()
    code_end = max(b.va + b.size for b in linked.blocks.values() if b.kind == "code")
    name_r, rva_r, vsize_r, raw_r, rsize_r, flags_r = sections[-2]
    check(name_r == P.ARENA_NAME and flags_r == P.ARENA_CHARACTERISTICS == 0x60000020
          and rva_r == align(rva_c + vsize_c, headers.section_alignment) and P.IMAGE_BASE + rva_r == P.ARENA_VA
          and linked.va == P.ARENA_VA and list(linked.blocks) == [b.name for b in P.linker.placed(P.ARENA)]
          and P.IMAGE_BASE + rva_r + vsize_r == code_end and raw_r == raw_c + rsize_c
          and rsize_r == align(vsize_r, headers.file_alignment)
          and bytes(data[raw_r:raw_r + vsize_r]) == linked.image[:vsize_r],
          f".srpx, the arena's code, at VA {P.IMAGE_BASE + rva_r:#x}: its code blocks ({vsize_r:#x} bytes) "
          + ", ".join(f"{b.name} at {b.va:#x}" for b in linked.blocks.values() if b.kind == "code")
          + "; code | execute | read, on the page after .srpc, its file bytes right after .srpc's")
    name_d, rva_d, vsize_d, raw_d, rsize_d, flags_d = sections[-1]
    check(name_d == P.ARENA_DATA_NAME and flags_d == P.ARENA_DATA_CHARACTERISTICS == 0xC0000040
          and rva_d == align(rva_r + vsize_r, headers.section_alignment) and P.IMAGE_BASE + rva_d == linked.data_va
          and P.IMAGE_BASE + rva_d + vsize_d == linked.va + linked.size and raw_d == raw_r + rsize_r
          and rsize_d == align(vsize_d, headers.file_alignment)
          and bytes(data[raw_d:raw_d + vsize_d]) == linked.image[linked.data_va - linked.va:]
          and not any(data[raw_d + vsize_d:raw_d + rsize_d]),
          f".srpxd, the arena's data, at VA {P.IMAGE_BASE + rva_d:#x}: its data and state blocks ({vsize_d:#x} "
          "bytes) " + ", ".join(f"{b.name} ({b.kind}) at {b.va:#x}" for b in linked.blocks.values() if b.kind != "code")
          + "; initialized data | read | write, on the page after .srpx, its file bytes right after .srpx's")
    get = lambda blob, at: struct.unpack_from("<I", blob, at)[0]
    check(get(data, headers.size_of_image_at) == align(rva_d + vsize_d, headers.section_alignment)
          and get(data, headers.optional + 12) == get(without, headers.optional + 12) + state_size
          and get(data, headers.size_of_code_at) == get(original, headers.size_of_code_at) + rsize + rsize_c + rsize_r
          and get(data, headers.size_of_initialized_data_at) == get(original, headers.size_of_initialized_data_at)
          + rsize_d and headers.table + 40 * len(sections) <= headers.size_of_headers,
          "SizeOfImage ends at .srpxd, SizeOfUninitializedData grew by .srpd's page, SizeOfCode by .srp's, "
          ".srpc's and .srpx's file sizes, SizeOfInitializedData by .srpxd's; the section table fits in the headers "
          "(its last entry)")
    both = [s[0] for s in sections if s[5] & P.IMAGE_SCN_MEM_WRITE and s[5] & P.IMAGE_SCN_MEM_EXECUTE]
    writable_code = bytearray(data)
    struct.pack_into("<I", writable_code, headers.table + 40 * (len(sections) - 2) + 36,
                     P.ARENA_CHARACTERISTICS | P.IMAGE_SCN_MEM_WRITE)
    try:
        P.refuse_writable_code(writable_code)
        refused = None
    except SystemExit as stop:
        refused = str(stop)
    check(not both and refused is not None and ".srpx" in refused,
          "no section is both writable and executable; refuse_writable_code stops a build whose .srpx is: "
          + str(refused))
    security_offset, security_size = image.directory(4)
    old_offset, old_size = struct.unpack_from("<II", original, headers.security_at)
    check(security_offset == raw_d + rsize_d and security_offset + security_size == len(data) and security_offset % 8 == 0
          and bytes(data[security_offset:]) == bytes(original[old_offset:old_offset + old_size]),
          f"certificate table at {security_offset:#x}: right after .srpxd's bytes, 8-byte aligned, last in the file, "
          "identical to the original's")
    same = all(image.directory(index) == before.directory(index) for index in range(16) if index != 4)
    check(same, "the other 15 data directories are those of the build without it")

    test_writes(original, data, list(P.APPLIED), "the default build")

    ready = ndisasm(image.read(P.LATE_JOIN_VA + P.LATE_JOIN_READY_OFFSET, 128), P.LATE_JOIN_VA + P.LATE_JOIN_READY_OFFSET)
    onward = [branch_target(text) for _, _, text in ready if text.startswith("jmp")]
    check(ready[0][2] == "pusha" and onward[:1] == [P.SECTION_VA],
          f"the ready hook's code starts with pushad and jumps on to the upload code at {P.SECTION_VA:#x}")

    test_clock_factor(original, image, before)
    test_fov(original, image, before)
    test_fov_lod(original, image, before)
    test_weapon_camo(original, image, before)
    test_weapon_random(original, image, before)
    test_kit_state(original, image, before)
    test_first_load(original, image, before)
    test_seek_clear(original, image, before)
    test_stale_dll(original, image, before)
    test_mount_patches(original, image, before)
    test_forward_scroll(original, image, before)
    test_replay_link(original, image, before)
    test_shared_upgrades(original, image, before)
    test_kit_change(original, image, before)
    test_warmup_look(original, image, before)
    test_warmup_keys(original, image, before)
    test_death_camera(original, image, before)
    test_read_order(original, image, before)
    test_weapon_preview(original, image, before)
    test_weapon_table(original, image, before)
    test_weapon_decals(original, image, before)
    test_upgrade_preview(original, image, before)
    test_research(original, image, before)
    test_rust_hooks(original, image, before)
    test_rust_code(original, image, before)
    test_tab_list(original, image, before)
    test_spot(original, image, before)
    test_artefacts(original, image, before)
    test_match_keys(original, image, before)
    test_hide_ui(original, image, before)
    test_voice(original, image, before)
    test_kill_feed(original, image, before)
    test_detail(original, image)
    test_weapon_kit(original, image, before)
    test_weapon_flight(original, image, before)
    test_revision(original, image, "section", P.REVISION, late_join=True)
    test_revision_override(original, data, server)
    test_required_patches(original, data, server)
    test_default_server(original, image, "section", server)
    suffix = "" if server is None else (".embedded-max" if len(server) > 63 else ".embedded")
    path = os.path.join(SCRATCH, f"survarium_srp.section{suffix}.late-join.exe")
    with open(path, "wb") as out:
        out.write(data)
    print(f"  written: {path} (sha256 {hashlib.sha256(data).hexdigest()})")
    test_windows(path, source, "section")


def test_windows(path, original_path, mode):
    crypt32 = ctypes.WinDLL("crypt32", use_last_error=True)
    imagehlp = ctypes.WinDLL("imagehlp", use_last_error=True)
    header_sum, check_sum = ctypes.c_uint32(), ctypes.c_uint32()
    result = imagehlp.MapFileAndCheckSumW(ctypes.c_wchar_p(path), ctypes.byref(header_sum), ctypes.byref(check_sum))
    if mode == "cert-only":
        check(result == 0, f"imagehlp reads the file (header checksum kept at {header_sum.value:#x}, "
              f"computed {check_sum.value:#x}; the loader does not check it for an exe)")
    else:
        check(result == 0 and header_sum.value == check_sum.value,
              f"PE checksum {header_sum.value:#x} = imagehlp's {check_sum.value:#x}")

    crypt32.CryptQueryObject.argtypes = [ctypes.c_uint32, ctypes.c_void_p] + [ctypes.c_uint32] * 3 + [ctypes.c_void_p] * 6
    crypt32.CertFindCertificateInStore.restype = ctypes.c_void_p
    crypt32.CertFindCertificateInStore.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32,
                                                   ctypes.c_uint32, ctypes.c_void_p, ctypes.c_void_p]

    def certificate(file):
        store, message = ctypes.c_void_p(), ctypes.c_void_p()
        name = ctypes.create_unicode_buffer(file)
        # The client's call at 0x1335B: CERT_QUERY_OBJECT_FILE (1),
        # CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED (0x400),
        # CERT_QUERY_FORMAT_FLAG_BINARY (2), then store and message out.
        if not crypt32.CryptQueryObject(1, name, 0x400, 2, 0, None, None, None,
                                        ctypes.byref(store), ctypes.byref(message), None):
            return None
        # CertFindCertificateInStore(store, X509 | PKCS7, 0, CERT_FIND_SUBJECT_STR_W, L"survarium.com", 0)
        subject = ctypes.create_unicode_buffer("survarium.com")
        context = crypt32.CertFindCertificateInStore(store, 0x10001, 0, 0x00080007, subject, None)
        if not context:
            return None
        # CERT_CONTEXT {DWORD encoding; BYTE *encoded; DWORD size; ...}, native layout
        encoding, encoded, size = struct.unpack_from("@IPI", ctypes.string_at(context, 3 * ctypes.sizeof(ctypes.c_void_p)))
        return ctypes.string_at(encoded, size)

    ours, theirs = certificate(path), certificate(original_path)
    check(ours is not None and ours == theirs,
          "CryptQueryObject (the client's own call) finds the survarium.com certificate, identical to the "
          f"original's ({len(ours or b'')} bytes, sha1 {hashlib.sha1(ours or b'').hexdigest()})")


def test_variant(original, source, mode, server):
    """One build: `mode` with the embedded `server` (or none), checked and
    written to SCRATCH."""
    print(f"variant {mode}, embedded server {server}")
    code = None if mode == "cert-only" else open(P.default_bin(mode), "rb").read()
    data, description = P.build(original, mode, code, server)
    print("  " + description)
    if mode != "cert-only":                            # the variant's own installs, for the stale check
        P.audit_installs(data, P.WRITES)
        INSTALLED_SEEN.update(P.installs_of(data, P.WRITES))
    check(hashlib.sha256(original).hexdigest() == P.EXE_SHA256, "the original in memory is untouched")
    image = Image(data)
    cave = b"" if code is None else P.split_code(code)[0]
    payload = cave if server is None else cave + open(P.default_server_bin(mode), "rb").read() + server.encode("ascii") + b"\0"
    test_pe(original, image, mode)
    test_writes(original, data, list(P.APPLIED), f"{mode}, server {server}")
    if mode == "section":
        raw, rsize = image.sections[-1][3:5]
        check(bytes(data[raw:raw + rsize]) == payload + b"\xCC" * (rsize - len(payload)),
              f"the new section's file data is the cave ({len(payload)} bytes) + int3 padding")
    test_default_server(original, image, mode, server)
    test_revision(original, image, mode, P.REVISION_WITHOUT_LATE_JOIN)
    if mode == "cert-only":
        if os.path.exists(INSTALLED_CERT_ONLY):
            check(open(INSTALLED_CERT_ONLY, "rb").read() == bytes(data),
                  "identical to the survarium_srp.exe installed in step 0 (read only)")
    else:
        test_code(original, image, mode, cave)
    if mode == "section":
        test_late_join(original, data, server, source)
    suffix = "" if server is None else (".embedded-max" if len(server) > 63 else ".embedded")
    path = os.path.join(SCRATCH, f"survarium_srp.{mode}{suffix}.exe")
    with open(path, "wb") as out:
        out.write(data)
    print(f"  written: {path} (sha256 {hashlib.sha256(data).hexdigest()})")
    test_windows(path, source, mode)


def test_calls(original):
    print("variant section, the arena and the call shapes")
    test_arena(original)
    test_engine(original)
    test_hook_kinds(original)
    test_apply(original)
    test_call_shapes_negative(original)


# main's checks after test_bins, each in a process of its own (_run_part).
PARTS = []


def install_rows():
    """The sites of patch_026g.py's install tables and of every Rust hook, as
    they are now (a build adds to them)."""
    rows = {row[0] for name, value in vars(P).items() if name.isupper() for row in P._install_rows(value)}
    return rows | {va for va, _ in P.hooks_026g.install_sites()}


def _run_part(index):
    """PARTS[index], in a process forked from main's after test_bins (so with
    its caches: the Rust sysroot, the arena's objects): what it printed (fd 1
    and 2, the tools' output too), its failures, the sites it saw installed
    and written, the install rows its builds left (install_rows) and its
    traceback when it stopped."""
    failures.clear()
    INSTALLED_SEEN.clear()
    WRITTEN_SEEN.clear()
    with tempfile.TemporaryFile() as out:
        os.dup2(out.fileno(), 1)
        os.dup2(out.fileno(), 2)
        # New streams: the forked ones may hold main's unwritten text.
        sys.stdout = sys.stderr = open(1, "w", buffering=1, encoding="utf-8", closefd=False)
        stopped = None
        try:
            PARTS[index]()
        except BaseException:                          # sys.exit too: a refusal of build()
            stopped = traceback.format_exc()
        sys.stdout.flush()
        out.seek(0)
        text = out.read().decode("utf-8", "replace")
    return text, list(failures), set(INSTALLED_SEEN), set(WRITTEN_SEEN), install_rows(), stopped


def main():
    source = sys.argv[1] if len(sys.argv) > 1 else ORIGINAL
    os.environ["SRP_026G_EXE"] = os.path.abspath(source)     # the Rust span hooks' bytes (hooks_026g.py)
    original = open(source, "rb").read()
    os.makedirs(SCRATCH, exist_ok=True)
    game = os.path.abspath(os.path.dirname(source))
    try:
        inside = os.path.commonpath([os.path.abspath(SCRATCH), game]) == game
    except ValueError:  # another drive: not inside
        inside = False
    if inside:
        sys.exit("the scratch directory must not be inside the game folder")
    # test_bins here first: it links the arena (the Rust sysroot built once,
    # the objects cached), which every part forked after it then has. The
    # parts share nothing else but what _run_part hands back (each writes
    # its own files in SCRATCH): they run on every core, a fresh fork each,
    # their output printed in this order.
    test_bins()
    variants = [(mode, None) for mode in ("cert-only", "section", "hash-mismatch")]
    variants += [(mode, "026g.skhid.link:1234") for mode in P.CAVES]
    variants += [(mode, "a" * 63 + ":65535") for mode in P.CAVES]
    PARTS[:] = [test_late_join_bin, lambda: test_server_options(original), lambda: test_calls(original)]
    PARTS.extend(lambda mode=mode, server=server: test_variant(original, source, mode, server)
                 for mode, server in variants)
    sys.stdout.flush()
    sys.stderr.flush()
    workers = min(len(PARTS), len(os.sched_getaffinity(0)))
    with multiprocessing.get_context("fork").Pool(workers, maxtasksperchild=1) as pool:
        rows = install_rows()
        for text, failed, installed, written, built, stopped in pool.imap(_run_part, range(len(PARTS))):
            sys.stdout.write(text)
            failures.extend(failed)
            INSTALLED_SEEN.update(installed)
            WRITTEN_SEEN.update(written)
            rows |= built
            if stopped:
                sys.stdout.write(stopped)
                check(False, f"stopped: {stopped.strip().splitlines()[-1]} (the traceback above)")
            sys.stdout.flush()
    stale = sorted(set(P.INSTALLS) - INSTALLED_SEEN)
    check(not stale, f"every INSTALLS row is installed by some variant built here ({len(INSTALLED_SEEN)} sites)"
          + (": stale " + ", ".join(f"{va:#x}" for va in stale) if stale else ""))
    unwritten = sorted(rows - WRITTEN_SEEN)
    check(not unwritten, f"every row of patch_026g.py's install tables and every Rust hook ({len(rows)}) is written "
          "by some build here" + (": not " + ", ".join(f"{va:#x}" for va in unwritten) if unwritten else ""))
    print()
    print(f"{len(failures)} FAILED" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
