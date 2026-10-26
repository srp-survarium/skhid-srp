"""Generator regressions: python -m unittest test_hooks_026g (dev shell)."""
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest

import hooks_026g as H


def function():
    return {"symbol": "engine_test", "va": 0x2000, "args": [], "params": [], "ret": "unsigned int",
            "cleanup": 0, "result": "EAX", "call": "cdecl", "vostok": {"args": [], "result": "u32"}}


def site(va, kind="call"):
    return {"site": "test.site", "va": va, "kind": kind, "callee": "engine_test", "span": 5,
            "exits": [], "what": "test fixture"}


class Hooks(unittest.TestCase):
    def test_call_displacements(self):
        for va in (0x1000, 0x3000, 0xFFFFFFFC):
            with self.subTest(site=hex(va)):
                h = H.call_hook({"name": "fixture", "patch": 0},
                                {"site": "test.site", "handler": "first"},
                                {"test.site": site(va)}, [function()])
                self.assertEqual(h["original"][0], 0xE8)
                displacement = struct.unpack("<i", h["original"][1:])[0]
                self.assertEqual((va + 5 + displacement) & 0xFFFFFFFF, 0x2000)

    def test_relocation_remakes_relative_control_flow(self):
        # A relative jcc, jmp or call is made again to its absolute target;
        # one into the span itself, a prefixed one (o16/a16: its target
        # truncated), loop/jecxz and xbegin (no rel32 form) are refused.
        va = 0x2000
        for code, line in (("7500", "jnz near 0x2002"), ("0f8500010000", "jnz near 0x2106"),
                           ("e800010000", "call 0x2105"), ("e900010000", "jmp 0x2105"), ("eb00", "jmp 0x2002")):
            with self.subTest(code=code):
                self.assertEqual(H.relocated(bytes.fromhex(code), va, "test"), [f"    {line}"])
        for code in ("660f850001", "66e80001", "e200", "67e300", "c7f800010000", "7401909090", "eb01909090"):
            with self.subTest(code=code), self.assertRaises(H.Bad):
                H.relocated(bytes.fromhex(code), va, "test")

    def test_relocation_accepts_opcode_bytes_in_operands(self):
        for code in ("b870000000", "8b80e8000000", "558bec83e4f883ec28",
                     "c70570000000e8e9eb70", "b80f850000"):
            with self.subTest(code=code):
                lines = H.relocated(bytes.fromhex(code), 0x2000, "test")
                self.assertTrue(all(line.startswith("    db ") for line in lines))

    def test_relocation_requires_whole_instructions(self):
        for code in ("b8", "0f85", "558bec83ec", "66", "f3"):
            with self.subTest(code=code), self.assertRaisesRegex(H.Bad, "invalid or incomplete"):
                H.relocated(bytes.fromhex(code), 0x2000, "test")

    def test_span_hooks_use_instruction_validation(self):
        for kind in ("splice", "lohook"):
            for code, accepted in (("b870000000", True), ("0f8500010000", True), ("7401909090", False),
                                   ("558bec83ec", False)):
                with self.subTest(kind=kind, code=code):
                    span = bytes.fromhex(code)
                    sites = {"test.site": {**site(0x2000, kind), "span": len(span)}}
                    patches = [{"name": "fixture", "patch": 0,
                                "hook": [{"site": "test.site", "handler": "first"}]}]
                    if accepted:
                        H.hooks(patches, [function()], sites, lambda va, n: span)
                    else:
                        with self.assertRaises(H.Bad):
                            H.hooks(patches, [function()], sites, lambda va, n: span)

    def test_multiple_originals_compile_and_dispatch(self):
        # Two calls share the direct Original. Two splices of the same
        # function each need their own trampoline, even beside those calls.
        rows, sites = [], {}
        for i, (handler, kind) in enumerate((("first", "call"), ("second", "call"),
                                             ("splice_a", "splice"), ("splice_b", "splice"))):
            name = f"test.{handler}"
            rows.append({"site": name, "handler": handler})
            sites[name] = {**site(0x1000 + 0x10 * i, kind), "site": name}
        hs = H.hooks([{"name": "fixture", "patch": 0, "hook": rows}], [function()], sites,
                     lambda va, n: b"\x90" * n)
        handlers = []
        for i, h in enumerate(hs):
            handlers.append(f"pub fn {h['handler']}(o: &{h['orig']}) -> u32 {{ o.call() + {i + 1} }}")
        source = """
#![allow(dead_code, unused_variables)]
struct Thread;
impl Thread { unsafe fn new() -> Self { Self } }
mod raw { pub unsafe fn engine_test() -> u32 { 100 } }
mod generated {
    use super::{Thread, raw};
    include!("generated.rs");
}
mod fixture {
    use super::generated::*;
    HANDLERS
}
#[unsafe(no_mangle)]
extern "cdecl" fn fixture_splice_a_tramp() -> u32 { 200 }
#[unsafe(no_mangle)]
extern "cdecl" fn fixture_splice_b_tramp() -> u32 { 300 }
#[test]
fn dispatches_to_each_original() {
    assert_eq!(generated::hook_fixture_first(), 101);
    assert_eq!(generated::hook_fixture_second(), 102);
    assert_eq!(generated::hook_fixture_splice_a(), 203);
    assert_eq!(generated::hook_fixture_splice_b(), 304);
}
""".replace("HANDLERS", "\n".join(handlers))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "generated.rs").write_text(H.rs_text(hs))
            (root / "test.rs").write_text(source)
            linker = os.environ.get("CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_LINKER", "cc")
            done = subprocess.run(["rustc", "--edition=2024", "--test", "--target=i686-unknown-linux-gnu",
                                   "-C", f"linker={linker}", str(root / "test.rs"), "-o", str(root / "test")],
                                  capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)
            done = subprocess.run([str(root / "test")], capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)


if __name__ == "__main__":
    unittest.main()
