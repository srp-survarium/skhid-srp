#!/usr/bin/env python3
"""Run patch 217 (rust/patches/src/additional_camo.rs, the hook of
additional_camo.toml: an additional model whose entry has
`camo_available: true` takes its item's camouflage; docs/notes/exo-suits.md,
Camouflage) in the default build, the original exe beside, and a broken
build (the site's bytes the original's again: the hook not installed) that
must fail.

    python check_additional_camo_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them. The game's own code runs: the cook data's constructor
(0x127C30, its 20 records 0x127CB0 and their camouflage_decal_cook_parameters
0x119760), profile_skin_visual_cook::on_configs_loaded from its read of an
item's additional_models (0x2054C9) through the slot's end and the site
(0x20563D) to the loop's test (0x205645): the additional models' records
(0x205520..0x205607, the config lookups 0xF6C00/0xF6B10 and assign
0x129BB0), the item's bake name (0x204500, 0x41580); then bake_decals'
loop over the records (0x1298CC..0x1299B8), whose list of bakes is read
back: each bake's parameters and the surface it bakes. Stubbed:
GetCurrentThreadId (one thread), QueryPerformanceCounter/-Frequency and the
log's kernel32 (its lines kept). The item's own record is filled as the
native fill leaves it (0x204BEC..0x204E4A: the camouflage's name, `_nmap`,
no fresnel, `_smap`; its file name and its decals' part in the bake's name;
the flag).

Checked:
- the site in place (`jmp` to its entry, the original's bytes there);
- the Duty body (exoskeleton_s, back_s, sign_s, legs_s with the key): the
  item's own record as the original's (its bake name ended by the native
  with its model's file name); the legs' record bakes, with the item's four names, both matrices, and the
  name `<camouflage's file name>duty_03_legs_s1`, no decals; the other
  three records as the original's; bake_decals bakes the item's surface and
  the legs' (the original: the item's alone);
- the Freedom body (legs `ported/freedom_026/freedom_exo_legs_s1`): its own
  name;
- an entry the native skips (no base_model) before the legs, and a HUD
  entry (base_model_hud with part_name_hud: its model's name);
- the same cook data, byte for byte the original's, for: entries without
  the key (a stock vest), `camo_available: false`, an item whose record
  does not bake (camo_available false), an item that bakes with no
  camouflage named (its decals, or its surface's own `_camo`), a slot that
  made no record (the empty slot's jump to the site), and entries that do
  not tell which record is whose (a boolean among them, which the native
  skips: one log line);
- the registers and the stack after the site as the original's (its span
  replayed: EAX and [esp+70h] the slot's [esp+6Ch]);
- a build without the hook fails the Duty and Freedom cases.
"""
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

import patch_026g as base

SITE, SPAN, AFTER = 0x20563D, 8, 0x205645    # skin_cook.item_done and the loop's test after its span
ADDITIONAL = 0x2054C9                        # the item's additional_models read (mov esi,"additional_models")
COOK_CTOR = 0x127C30                         # skeleton_combined_cook_data's constructor (ESI, ret 4)
BAKE_LOOP, BAKE_END = 0x1298CC, 0x1299B8     # bake_decals' loop over the records (xor edi,edi .. test edi,edi)
THREAD_SLOT, QPC_SLOT, QPF_SLOT = 0x870118, 0x870158, 0x870150
CREATE_FILE_SLOT, WRITE_FILE_SLOT, CLOSE_SLOT = 0x870048, 0x870108, 0x870040
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
CONFIG, TEXTS, COOK, BAKES = HEAP, HEAP + 0x40000, HEAP + 0x80000, HEAP + 0xA0000
SP, BP, SENTINEL = STACK + 0x8000, STACK + 0xC000, STUBS + 0xF00
RECORD, RECORDS, COUNT, TAG, FLAG = 0xC0C, 0x234, 0xF324, 0xC08, 0xC04
PARAMS = 0x34C
# the camouflage's four names, its two matrices and the bake's name, by the record (P = +34Ch)
NAMES = {"diffuse": 0x86C, "normal": 0x8F8, "fresnel": 0x984, "smoothness": 0xA10}
MATRICES = (PARAMS + 0x4E0, PARAMS + 0x750)
BAKE_NAME = PARAMS + 0x7A4
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
CAMO = "camo_atlas/camo_atlas_m81_woodland"
TORSO = "ported/duty_026/duty_03_torso_s1"

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((HEAP, 0x100000), (STUBS, 0x1000), (STACK, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        for slot, stub, value in ((QPC_SLOT, STUBS + 0xC0, 0), (QPF_SLOT, STUBS + 0xE0, 10_000_000)):
            self.word(slot, stub)
            # mov eax,[esp+4] / mov dword [eax],value / mov dword [eax+4],0 / mov eax,1 / ret 4
            emu.mem_write(stub, b"\x8B\x44\x24\x04\xC7\x00" + struct.pack("<I", value)
                          + b"\xC7\x40\x04\x00\x00\x00\x00\xB8\x01\x00\x00\x00\xC2\x04\x00")
        self.logged = b""
        for slot, stub, code in ((CREATE_FILE_SLOT, STUBS + 0x90, b"\xB8\x07\x00\x00\x00\xC2\x1C\x00"),
                                 (WRITE_FILE_SLOT, STUBS + 0xA0, b"\xB8\x01\x00\x00\x00\xC2\x14\x00"),
                                 (CLOSE_SLOT, STUBS + 0xB0, b"\xB8\x01\x00\x00\x00\xC2\x04\x00")):
            self.word(slot, stub)
            emu.mem_write(stub, code)
        emu.hook_add(UC_HOOK_CODE, self.write_file, begin=STUBS + 0xA0, end=STUBS + 0xA0)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def write_file(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.logged += self.read(self.read(sp + 8), self.read(sp + 12))

    def put_text(self, text):
        at = self.texts
        self.emu.mem_write(at, text.encode() + b"\0")
        self.texts += (len(text) + 16) & ~15
        return at

    def records(self, count):
        at = self.configs
        self.configs += (24 * count + 15) & ~15
        return at

    def node(self, value, name=None):
        """A loaded binary config node (24 bytes: data, name, CRC, type, count): a boolean (0), a text (5), a
        table (3, its children sorted by their names' CRC, as cfg_child's binary search needs) or an array (4)."""
        key = (self.put_text(name), zlib.crc32(name.encode())) if name is not None else (0, 0)
        if isinstance(value, bool):
            return struct.pack("<QQIHH", int(value), key[0], key[1], 0, 0)
        if isinstance(value, str):
            return struct.pack("<QQIHH", self.put_text(value), key[0], key[1], 5, len(value) + 1)
        if isinstance(value, dict):
            kids = sorted(value.items(), key=lambda item: zlib.crc32(item[0].encode()))
            kind = 3
        else:
            kids = [(None, v) for v in value]
            kind = 4
        first = self.records(len(kids))
        for i, (kid, v) in enumerate(kids):
            self.emu.mem_write(first + 24 * i, self.node(v, kid))
        return struct.pack("<QQIHH", first if kids else 0, key[0], key[1], kind, len(kids))

    def go(self, start, stop, regs, sp):
        emu = self.emu
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            emu.emu_start(start, stop, count=2_000_000)
        except Exception as error:
            sys.exit(f"FAIL: {start:#x}: {error} at {emu.reg_read(UC_X86_REG_EIP):#x}")
        return {reg: emu.reg_read(reg) for reg in REGS}, emu.reg_read(UC_X86_REG_ESP)

    def text(self, string):
        begin, end = self.read(string), self.read(string + 4)
        return self.read(begin, end - begin).decode() if end > begin else ""

    def assign(self, string, text):
        begin = self.read(string)
        self.emu.mem_write(begin, text.encode() + b"\0")
        self.word(string + 4, begin + len(text))

    def record(self, i):
        return COOK + RECORDS + i * RECORD

    def slot(self, entry, item, made=True, before=1):
        """A kit slot of on_configs_loaded: a new cook data (the game's constructor) whose records 1.. are
        `before` other items' and, when `made`, the slot's item's next, filled as the native fill leaves it
        (`item`: its flag, camouflage name and decals' part of the bake name); the frame's entry `entry` (its
        additional_models). Run from the additional models' read through the site to the loop's test, or,
        when the slot made no record, from the site (the empty slot's jump). What came of it: every record's
        fields, the cook data's bytes, the registers and ESP after, and bake_decals' bakes (each its
        parameters' record and its surface)."""
        self.configs, self.texts = CONFIG, TEXTS
        self.emu.mem_write(COOK, bytes(0xF328))
        self.word(SENTINEL - 4, 0)
        sp = STACK + 0x4000
        self.word(sp, SENTINEL)
        self.word(sp + 4, 0)
        self.go(COOK_CTOR, SENTINEL, {UC_X86_REG_ESI: COOK}, sp)
        for i in range(1, 1 + before):
            r = self.record(i)
            self.assign(r, f"models/other_{i}")
            self.assign(r + 0x114, f"other_{i}")
            self.word(r + TAG, i)
        index = 1 + before
        r = self.record(index)
        if made:
            self.assign(r, TORSO)
            self.assign(r + 0x114, "torso_s")
            self.word(r + TAG, 2)
            flag, camo, decals = item
            self.emu.mem_write(r + FLAG, bytes([flag]))
            if camo:
                self.assign(r + NAMES["diffuse"], camo)
                self.assign(r + NAMES["normal"], camo + "_nmap")
                self.assign(r + NAMES["smoothness"], camo + "_smap")
            self.assign(r + BAKE_NAME, (camo.rsplit("/", 1)[-1] if camo else "") + decals)
            for at, value in zip(MATRICES, (0x3F000000, 0x3F800000)):   # the item's matrices, apart from a new one's
                self.emu.mem_write(r + at, struct.pack("<16I", *([value] * 16)))
        self.emu.mem_write(COOK + COUNT, bytes([index + made]))
        self.emu.mem_write(SP, bytes(0x600))
        self.word(SP + 0x18, r)
        self.word(SP + 0x1C, COOK)
        self.emu.mem_write(SP + 0x20, self.node(entry))
        self.word(SP + 0x6C, 2)
        self.emu.mem_write(BP, bytes(0x40))
        regs = {reg: 0x0A0A0A0A + i for i, reg in enumerate(REGS)}
        regs.update({UC_X86_REG_EBX: r, UC_X86_REG_EBP: BP})
        after, esp = self.go(ADDITIONAL if made else SITE, AFTER, regs, SP)
        count = self.read(COOK + COUNT, 1)[0]
        made_records = []
        for i in range(count):
            rec = self.record(i)
            made_records.append({
                "model": self.text(rec), "part": self.text(rec + 0x114), "tag": self.read(rec + TAG),
                "flag": self.read(rec + FLAG, 1)[0], "name": self.text(rec + BAKE_NAME),
                **{key: self.text(rec + at) for key, at in NAMES.items()},
                "matrices": tuple(self.read(rec + at, 64) for at in MATRICES)})
        state = self.read(COOK, 0xF328)
        return made_records, state, after, esp, self.read(SP + 0x70), self.bakes(count)

    def bakes(self, count):
        """bake_decals' loop (0x1298CC..0x1299B8) over the cook data's `count` records, its frame's arrays
        made here: the bakes it asks for, (the record of the parameters, the surface)."""
        self.emu.mem_write(BAKES, bytes(0x4000))
        holder, types, entries, list_ = BAKES, BAKES + 0x200, BAKES + 0x400, BAKES + 0x1000
        frame = STACK + 0xE000
        self.emu.mem_write(frame - 0x80, bytes(0x100))
        self.word(frame + 0x10, holder)                  # [ebp+10h]: the render instance's holder, +108h 0
        self.word(frame - 0x34, types)                   # 8 bytes a record: the bake's type
        self.word(frame - 0x1C, entries)                 # 30h a record: its parameters' box
        self.word(frame - 0x28, list_)                   # 4 a record: the boxes asked for
        self.word(frame - 0x0C, COOK)
        regs = {reg: 0 for reg in REGS}
        regs.update({UC_X86_REG_EBX: count, UC_X86_REG_EBP: frame})
        after, _ = self.go(BAKE_LOOP, BAKE_END, regs, frame - 0x80)
        out = []
        for i in range(after[UC_X86_REG_EDI]):
            parameters = self.read(self.read(list_ + 4 * i) + 8)
            record, rest = divmod(parameters - PARAMS - COOK - RECORDS, RECORD)
            out.append((record if rest == 0 else None, self.read(parameters + 0x794)))
        return out


def duty(legs=True, key=True):
    entry = {"base_model": "ported/duty_026/duty_03_legs_s1", "part_name": "legs_s"}
    if key is not None:
        entry["camo_available"] = key
    return {"base_model": TORSO, "part_name": "torso_s", "additional_models": [
        {"base_model": TORSO, "part_name": "exoskeleton_s"}, {"base_model": TORSO, "part_name": "back_s"},
        {"base_model": TORSO, "part_name": "sign_s"}] + ([entry] if legs else [])}


FREEDOM = {"base_model": "ported/freedom_026/freedom_exo_torso_s1", "part_name": "torso_s", "additional_models": [
    {"base_model": "ported/freedom_026/freedom_exo_torso_s1", "part_name": "exoskeleton_s"},
    {"base_model": "ported/freedom_026/freedom_exo_torso_s1", "part_name": "back_s"},
    {"base_model": "ported/freedom_026/freedom_exo_legs_s1", "part_name": "legs_s", "camo_available": True}]}
PAINTED = (1, CAMO, "4_7783_1286_")


def cases(data):
    """Each case's results in this exe."""
    game = Game(data)
    out = {}
    for name, entry, item, made, before in (
            ("duty", duty(), PAINTED, True, 1),
            ("freedom", FREEDOM, PAINTED, True, 2),
            ("skipped and hud", {"base_model": TORSO, "part_name": "torso_s", "additional_models": [
                {"part_name": "no_model_s"},
                {"base_model_hud": "hud/exo_legs_hud", "part_name_hud": "legs_hud_s", "base_model": "x",
                 "part_name": "y", "camo_available": True},
                {"base_model": "ported/duty_026/duty_03_legs_s1", "part_name": "legs_s", "camo_available": True}]},
             PAINTED, True, 1),
            ("no key", duty(key=None), PAINTED, True, 1),
            ("key false", duty(key=False), PAINTED, True, 1),
            ("no flag", duty(), (0, CAMO, ""), True, 1),
            ("no camouflage", duty(), (1, "", "4_7783_1286_"), True, 1),
            ("no record", duty(), PAINTED, False, 1),
            ("unsure", {"base_model": TORSO, "part_name": "torso_s", "additional_models": [
                False, {"base_model": "ported/duty_026/duty_03_legs_s1", "part_name": "legs_s",
                           "camo_available": True}]}, PAINTED, True, 1)):
        game.logged = b""
        out[name] = game.slot(entry, item, made, before) + (game.logged,)
    return out


def painted_as_item(record, item, model):
    return (record["flag"] == 1 and all(record[k] == item[k] for k in NAMES)
            and record["matrices"] == item["matrices"]
            and record["name"] == CAMO.rsplit("/", 1)[-1] + model)


def judge(got, stock):
    """{case: (passed, what)} of a build's results against the original's."""
    verdict = {}
    for name in got:
        records, state, after, esp, slot70, bakes, logged = got[name]
        o_records, o_state, o_after, o_esp, o_slot70, o_bakes, o_logged = stock[name]
        regs = after == o_after and esp == o_esp and slot70 == o_slot70 == 2 and after[UC_X86_REG_EAX] == 2
        if name in ("duty", "freedom", "skipped and hud"):
            item_index = 2 if name != "freedom" else 3
            item = records[item_index]
            legs = len(records) - 1
            model = {"duty": "duty_03_legs_s1", "freedom": "freedom_exo_legs_s1",
                     "skipped and hud": "duty_03_legs_s1"}[name]
            others = [i for i in range(len(records)) if i not in (legs,)]
            painted = [legs]
            if name == "skipped and hud":
                painted = [legs - 1, legs]
                others = [i for i in others if i != legs - 1]
            ok = (regs and records[item_index] == o_records[item_index]
                  and item["name"] == "camo_atlas_m81_woodland4_7783_1286_duty_03_torso_s1"
                  and all(records[i] == o_records[i] for i in others)
                  and painted_as_item(records[legs], item, model)
                  and (name != "skipped and hud" or (records[legs - 1]["model"] == "hud/exo_legs_hud"
                                                     and painted_as_item(records[legs - 1], item, "exo_legs_hud")))
                  and bakes == sorted(o_bakes + [(i, i) for i in painted])
                  and o_bakes == [(item_index, item_index)] and not o_records[legs]["flag"] and logged == b"")
            verdict[name] = (ok, f"records {painted} bake with the item's camouflage, named "
                                 f"{records[legs]['name']!r}; bakes {bakes} (the original: {o_bakes}); "
                                 "the others and the registers as the original's")
        elif name == "unsure":
            ok = regs and state == o_state and bakes == o_bakes and b"do not tell which record" in logged
            verdict[name] = (ok, f"nothing painted, the cook data the original's, logged {logged.strip()!r}")
        else:
            ok = regs and state == o_state and bakes == o_bakes and logged == b""
            verdict[name] = (ok, f"the cook data byte for byte the original's, bakes {bakes}")
    return verdict


def without(built, original):
    broken = bytearray(built)
    at = base.file_offset(SITE)
    broken[at:at + SPAN] = original[at:at + SPAN]
    return bytes(broken)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    reader = base.hooks_026g.original_reader(original)
    ours = [h for h in base.hooks_026g.hooks(exe=reader)
            if any(m["patch"] == "additional_camo" for m in h.get("chain", [h]))]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    check([h["site"] for h in ours] == [SITE], f"additional_camo's one hook, at {SITE:#x}")
    for h in ours:
        va, old, symbol, opcode = installs[h["site"]]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old == bytes.fromhex("8b44246c89442470")
              and built[at:at + len(new)] == new,
              f"{va:#x}: {old.hex()} (mov eax,[esp+6Ch] / mov [esp+70h],eax) in the original, `jmp {symbol}` here")
    stock = cases(original)
    for name, (ok, what) in judge(cases(built), stock).items():
        check(ok, f"{name}: {what}")
    broke = judge(cases(without(built, original)), stock)
    passed = [name for name in ("duty", "freedom", "skipped and hud") if broke[name][0]]
    check(not passed, "a build without the hook fails duty, freedom, skipped and hud"
          + (f": it passed {passed}" if passed else ""))
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
