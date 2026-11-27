#!/usr/bin/env python3
"""Run patch 213, the first person out of the hi-Z occlusion
(rust/patches/src/first_person_occlusion.rs, vostok/occlusion.rs;
../docs/notes/renderer-026g.md 5.7), in the default build beside the original exe.

    python check_first_person_occlusion_026g.py <original survarium.exe>

Requires Python `unicorn`. stage_visibility::get_results_and_prepare_bounds_models
(0x1B7A70, the function of the site `hiz.model_answer` 0x1B7AB7) runs whole, from
its entry to its `ret 8`, in both exes, over a made-up visibility stage, scene
view and camera list of seven surfaces (each with its render surface's box, an
identity world matrix and a model instance), the last query's answers, the
query's counter and its bounds buffer; every import a failure, GetCurrentThreadId
1.

- The original stores each surface's answer at +61h: occluded where the last
  query's byte at its index (+40h) is 0, visible where it is not or where the
  surface was never queried (-1). A first-person surface (its model's foreground
  byte +114h, or its own +63h) takes the world's answer like any other.
- The build stores every first-person surface visible, and every world surface's
  answer as the original does.
- Everything else is the original's in the build: the indices the surfaces take
  (+40h), the counter, the spheres written to the bounds buffer and its pointer,
  the registers the caller keeps and the stack pointer after the `ret 8`.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

PREPARE_MODELS, SITE = 0x1B7A70, 0x1B7AB7
THREAD_SLOT = 0x870118
ENGINE_TSV = "engine_026g.tsv"
DATA, STACK, STUBS, RETURN = 0x30000000, 0x21000000, 0x31000000, 0x32000000
FORBIDDEN, THREAD = STUBS, STUBS + 0x10
STAGE, RENDERER, VIEW = DATA, DATA + 0x100, DATA + 0x10000
LIST, RESULTS, BOUNDS, BOUNDS_AT, COUNTER = DATA + 0x20000, DATA + 0x20400, DATA + 0x20800, DATA + 0x21000, DATA + 0x21010
MATRIX = DATA + 0x21100
SURFACES, RENDER_SURFACES, MODELS = DATA + 0x22000, DATA + 0x24000, DATA + 0x26000
SP = STACK + 0x8000
COUNTER_START = 40                                            # the kinds before models took 0..39
NEVER = 0xFFFFFFFF
# (name, the model's foreground byte, the surface's own, its last index, the last query's byte there)
CASES = (
    ("a world surface answered occluded", 0, 0, 0, 0),
    ("a world surface answered visible", 0, 0, 1, 1),
    ("a first-person model's surface answered occluded", 1, 0, 2, 0),
    ("a first-person surface (its own byte) answered occluded", 0, 1, 3, 0),
    ("a first-person model's surface answered visible", 1, 0, 4, 1),
    ("a first-person surface never queried", 1, 0, NEVER, None),
    ("a world surface never queried", 0, 0, NEVER, None),
)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((DATA, 0x30000), (STACK, 0x10000), (STUBS, 0x1000), (RETURN, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.forbidden = []
        for line in open(ENGINE_TSV, encoding="utf-8"):
            cells = line.rstrip("\n").split("\t")
            if len(cells) > 4 and cells[4] == "import":
                self.word(int(cells[1], 16), FORBIDDEN)
        self.word(THREAD_SLOT, THREAD)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=FORBIDDEN, end=FORBIDDEN)
        self.site_runs = 0
        emu.hook_add(UC_HOOK_CODE, self.at_site, begin=SITE, end=SITE)
        self.made_up()

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", bytes(self.emu.mem_read(at, 4)))[0]

    def stub(self, emu, address, size, _):
        self.forbidden.append(hex(self.read(emu.reg_read(UC_X86_REG_ESP))))
        emu.emu_stop()

    def at_site(self, emu, address, size, _):
        self.site_runs += 1

    def made_up(self):
        emu = self.emu
        self.word(STAGE + 4, RENDERER)
        self.word(STAGE + 0x1C, RESULTS)                         # the last query's answers, a byte an index
        self.word(RENDERER + 0x4170, VIEW)
        emu.mem_write(MATRIX, struct.pack("<16f", *(1.0 if i % 5 == 0 else 0.0 for i in range(16))))
        emu.mem_write(RESULTS, bytes([0xFF]) * 0x100)
        for i, (_, model_byte, own_byte, index, answer) in enumerate(CASES):
            surface, render, model = SURFACES + 0x100 * i, RENDER_SURFACES + 0x100 * i, MODELS + 0x200 * i
            self.word(LIST + 4 * i, surface)
            self.word(surface + 0x38, render)
            self.word(surface + 0x3C, model)
            self.word(surface + 0x40, index)
            self.word(surface + 0x50, MATRIX)
            emu.mem_write(surface + 0x61, b"\x5a")                # a stale answer, stored over
            emu.mem_write(surface + 0x63, bytes([own_byte]))
            emu.mem_write(model + 0x114, bytes([model_byte]))
            # the render surface's box, min then max: one a metre apart along x
            emu.mem_write(render + 0x20, struct.pack("<6f", i - 0.25, -0.5, 1.0, i + 0.25, 0.5, 2.0))
            if answer is not None:
                emu.mem_write(RESULTS + index, bytes([answer]))
        self.word(VIEW + 0x44A0, LIST)
        self.word(VIEW + 0x44A4, LIST + 4 * len(CASES))
        self.word(BOUNDS_AT, BOUNDS)
        self.word(COUNTER, COUNTER_START)

    def run(self):
        emu = self.emu
        emu.mem_write(SP, struct.pack("<III", RETURN, STAGE, BOUNDS_AT))
        for reg in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX):
            emu.reg_write(reg, 0x5A5A5A5A)
        kept = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: 0x22222222, UC_X86_REG_EDI: COUNTER,
                UC_X86_REG_EBP: 0x0019FF00}
        for reg, value in kept.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.emu_start(PREPARE_MODELS, RETURN, count=2_000_000)
        assert not self.forbidden, f"an import called: {self.forbidden}"
        assert emu.reg_read(UC_X86_REG_EIP) == RETURN, hex(emu.reg_read(UC_X86_REG_EIP))
        surfaces = [(bytes(emu.mem_read(SURFACES + 0x100 * i + 0x61, 1))[0], self.read(SURFACES + 0x100 * i + 0x40))
                    for i in range(len(CASES))]
        rest = {
            "kept registers": {reg: emu.reg_read(reg) for reg in KEPT},
            "stack pointer": emu.reg_read(UC_X86_REG_ESP) - SP,
            "counter": self.read(COUNTER),
            "bounds pointer": self.read(BOUNDS_AT) - BOUNDS,
            "bounds": bytes(emu.mem_read(BOUNDS, 0x10 * len(CASES))),
            "indices": [index for _, index in surfaces],
            "models and surfaces": bytes(emu.mem_read(MODELS, 0x200 * len(CASES)))
            + b"".join(bytes(emu.mem_read(SURFACES + 0x100 * i, 0x60)) for i in range(len(CASES))),
        }
        return [answer for answer, _ in surfaces], rest


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def case(what, ok, detail=""):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f"\n      {detail}" if not ok and detail else ""))

    stock_run, build_run = Run(original), Run(bytes(built))
    (stock, stock_rest), (new, new_rest) = stock_run.run(), build_run.run()
    case("the site runs once a surface in the build", build_run.site_runs == len(CASES), build_run.site_runs)
    for (what, model_byte, own_byte, index, answer), was, now in zip(CASES, stock, new):
        occluded_by_query = index != NEVER and answer == 0
        case(f"{what}: the original stores {int(occluded_by_query)}", was == int(occluded_by_query), was)
        first_person = model_byte or own_byte
        want = 0 if first_person else was
        case(f"{what}: the build stores {want}", now == want, now)
    for key in stock_rest:
        case(f"the {key} as the original's", stock_rest[key] == new_rest[key],
             f"{stock_rest[key]!r} / {new_rest[key]!r}")
    case("each surface takes the next index", new_rest["indices"] == list(range(COUNTER_START, COUNTER_START + len(CASES))),
         new_rest["indices"])
    case("a sphere a surface", new_rest["bounds pointer"] == 0x10 * len(CASES), new_rest["bounds pointer"])
    if failed:
        raise SystemExit(f"patch 213: {failed} failed")
    print(f"patch 213: {len(CASES)} surfaces through get_results_and_prepare_bounds_models: the first-person ones "
          f"stored visible whatever the hi-Z answered, the world's as the original; indices, spheres, counter, "
          f"registers and stack the original's")


if __name__ == "__main__":
    main()
