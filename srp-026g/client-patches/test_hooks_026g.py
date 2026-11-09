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

    def test_relocation_refuses_relative_control_flow(self):
        # Short/near/prefixed branches, calls, loop and transactional abort.
        for code in ("7500", "0f8500010000", "660f850001", "e800010000", "66e80001",
                     "e900010000", "eb00", "e200", "67e300", "c7f800010000"):
            with self.subTest(code=code), self.assertRaisesRegex(H.Bad, "relative branch"):
                H._branch_free(bytes.fromhex(code), "test")

    def test_lohook_replays_a_call_to_its_target(self):
        self.assertEqual(H.replay_lines(bytes.fromhex("e85b3a0000"), 0x2D1050, "test"), ["    call 0x2d4ab0"])
        self.assertEqual(H.replay_lines(bytes.fromhex("508bc5e89bdf0400"), 0x2D387D, "test"),
                         ["    db 0x50", "    db 0x8b, 0xc5", "    call 0x321820"])
        for code in ("7500", "e900010000", "66e80001"):
            with self.subTest(code=code), self.assertRaisesRegex(H.Bad, "relative branch"):
                H.replay_lines(bytes.fromhex(code), 0x1000, "test")

    def test_exits_that_pop_the_calls_arguments(self):
        h = {"entry": "e", "shim": "s", "site": 0x2D3880, "original": bytes.fromhex("e89bdf0400"),
             "exits": [("bound", 0x2D3885)], "pops": {"bound": 4}}
        lines = H.lohook_entry(h)
        self.assertIn("    mov edx, e_bound", lines)
        at = lines.index("e_bound:")
        self.assertEqual(lines[at + 1:at + 3], ["    lea esp, [esp + 4]", "    jmp 0x2d3885"])

    def test_natives(self):
        tomls = [{"name": "p", "patch": 1, "native": [{"symbol": "p_rebuild", "call": "stdcall",
                                                        "arguments": ["hud: GameWorldUi", "o: MatchOptions"]}]}]
        self.assertEqual(H.natives(tomls), [{"patch": "p", "symbol": "p_rebuild", "call": "stdcall",
                                             "args": [("hud", "GameWorldUi"), ("o", "MatchOptions")]}])
        self.assertIn('unsafe extern "stdcall" {', H.rs_text([], (), H.natives(tomls)))
        for bad in ({"symbol": "other_f", "call": "stdcall"}, {"symbol": "p_f", "call": "fastcall"},
                    {"symbol": "p_f", "call": "cdecl", "arguments": ["a: *mut u8"]}):
            with self.subTest(bad=bad), self.assertRaises(H.Bad):
                H.natives([{"name": "p", "patch": 1, "native": [bad]}])

    def test_relocation_accepts_opcode_bytes_in_operands(self):
        for code in ("b870000000", "8b80e8000000", "558bec83e4f883ec28",
                     "c70570000000e8e9eb70", "b80f850000"):
            with self.subTest(code=code):
                H._branch_free(bytes.fromhex(code), "test")

    def test_relocation_requires_whole_instructions(self):
        for code in ("b8", "0f85", "558bec83ec", "66", "f3"):
            with self.subTest(code=code), self.assertRaisesRegex(H.Bad, "invalid or incomplete"):
                H._branch_free(bytes.fromhex(code), "test")

    def test_span_hooks_use_instruction_validation(self):
        for kind in ("splice", "lohook"):
            for code, accepted in (("b870000000", True), ("0f8500010000", False), ("558bec83ec", False)):
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
    // SAFETY: the test stands in for each shim's entry.
    unsafe {
        assert_eq!(generated::hook_fixture_first(), 101);
        assert_eq!(generated::hook_fixture_second(), 102);
        assert_eq!(generated::hook_fixture_splice_a(), 203);
        assert_eq!(generated::hook_fixture_splice_b(), 304);
    }
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
