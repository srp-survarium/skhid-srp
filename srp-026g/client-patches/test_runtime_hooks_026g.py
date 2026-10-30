"""Runtime-vtable copies and explicitly ordered shared detours."""
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
import hooks_026g as H
from test_hooks_026g import function, site


class RuntimeHooks(unittest.TestCase):
    def fixture(self):
        patches = [{"name": "fixture", "patch": 0,
                    "hook": [{"site": "test.site", "handler": "insert", "proof": "native insert"}],
                    "vtable": [{"name": "visual", "source": "visual_methods", "check": "check_fixture.py"}]}]
        sites = {"test.site": site(0x30000, "vtable")}
        hs = H.hooks(patches, [function()], sites)
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "fields.tsv"
            p.write_text("kind\tfield\toffset\ttype\tabout\n"
                         "test::image\tvisual_methods\t0x30000\tobj<test::methods>\tfixture\n"
                         "test::methods\trtti\t-4\tu32\tRTTI\n"
                         "test::methods\tsize\t8\tsize\ttwo slots\n")
            fields = H.engine_026g.fields(p)
        return patches, hs, fields

    def test_vtable_preserves_rtti_and_unmodified_slots_checks_native_callee(self):
        patches, hs, fields = self.fixture()
        table, = H.runtime_vtables(patches, hs, fields)
        native = lambda va, n: struct.pack('<III', 0x45000, 0x2000, 0x35000)
        self.assertEqual(H.vtable_words(table, native), [0x45000, "fixture_insert", 0x35000])
        asm = H.asm_text(hs, [table], native)
        self.assertIn('dd 0x45000\n    dd fixture_insert\n    dd 0x35000', asm)
        rs = H.rs_text(hs, [table])
        self.assertIn('fixture_visual_vtable_handle()', rs)
        audit, = H.installed_vtables(patches, hs, fields)
        self.assertEqual(audit[3:6], (0x30000, 2, 1))
        self.assertEqual(audit[-1], {0: ('check_fixture.py', 'native insert')})
        with self.assertRaisesRegex(H.Bad, 'declared native callee'):
            H.vtable_words(table, lambda va, n: struct.pack('<III', 0x45000, 1, 0x35000))
        with self.assertRaisesRegex(H.Bad, 'truncated'):
            H.vtable_words(table, lambda va, n: bytes(n - 1))

    def test_vtable_declarations_refuse_missing_ambiguous_and_unproved_ownership(self):
        patches, hs, fields = self.fixture()
        with self.assertRaisesRegex(H.Bad, 'RTTI'):
            H.runtime_vtables(patches, hs, [f for f in fields if f['field'] != 'rtti'])
        with self.assertRaisesRegex(H.Bad, 'no runtime copy'):
            H.runtime_vtables([{**patches[0], 'vtable': []}], hs, fields)
        with self.assertRaisesRegex(H.Bad, 'several copies'):
            H.runtime_vtables([{**patches[0], 'vtable': patches[0]['vtable'] * 2}], hs, fields)
        with self.assertRaisesRegex(H.Bad, 'proof'):
            H.runtime_vtables(patches, [{**hs[0], 'proof': None}], fields)
        with self.assertRaisesRegex(H.Bad, 'replaces no slots'):
            H.runtime_vtables(patches, [{**hs[0], 'site': 0x30010}], fields)

    def chain(self, orders=(0, 1, 2)):
        patches = [{'name': name, 'patch': i, 'hook': [{'site': 'test.site', 'handler': 'run', 'order': order}]}
                   for i, (name, order) in enumerate(zip(('first', 'second', 'third'), orders))]
        sites = {'test.site': site(0x2000, 'lohook')}
        return H.hooks(patches, [], sites, lambda va, n: b'\x90' * n)

    def test_chain_requires_explicit_unique_order(self):
        for orders in ((None, None, None), (0, 0, 2), (False, 1, 2)):
            with self.assertRaisesRegex(H.Bad, 'explicit orders'):
                self.chain(orders)
        chain, = self.chain((2, 0, 1))
        self.assertEqual([h['patch'] for h in chain['chain']], ['second', 'third', 'first'])
        self.assertEqual(H.asm_text([chain]).count('fxsave [esp]'), 1)

    def test_chain_compiles_and_observes_context_edits_and_early_stop(self):
        hs = self.chain()
        source = '''
#![allow(dead_code)]
struct Frame { value: u32 }
struct Context<'a>(&'a mut Frame);
impl<'a> Context<'a> { unsafe fn new(f: &'a mut Frame) -> Self { Self(f) } }
enum NoExit {}
enum Flow<E> { Continue, Skip, Exit(E) }
mod generated { use super::*; include!("generated.rs"); }
mod first {
    use super::*;
    pub fn run(ctx: &mut Context<'_>) -> Flow<NoExit> {
        ctx.0.value += 10; Flow::Continue
    }
}
mod second {
    use super::*;
    pub fn run(ctx: &mut Context<'_>) -> Flow<NoExit> {
        if ctx.0.value == 11 { Flow::Skip } else { ctx.0.value += 20; Flow::Continue }
    }
}
mod third {
    use super::*;
    pub fn run(ctx: &mut Context<'_>) -> Flow<NoExit> { ctx.0.value += 30; Flow::Continue }
}
#[test]
fn all_run_in_order_but_skip_stops_the_chain() {
    let mut full = Frame { value: 0 };
    assert_eq!(generated::hook_first_run_chain(&mut full), 0);
    assert_eq!(full.value, 60);
    let mut early = Frame { value: 1 };
    assert_eq!(generated::hook_first_run_chain(&mut early), 1);
    assert_eq!(early.value, 11);
}
'''
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / 'generated.rs').write_text(H.rs_text(hs))
            (root / 'test.rs').write_text(source)
            linker = os.environ.get('CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_LINKER', 'cc')
            result = subprocess.run(['rustc', '--edition=2024', '--test', '--target=i686-unknown-linux-gnu',
                                     '-C', f'linker={linker}', str(root / 'test.rs'), '-o', str(root / 'test')],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            result = subprocess.run([str(root / 'test')], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_void_native_conventions_generate_unit_original_and_zero_shim_result(self):
        f = {**function(), 'ret': 'void', 'result': None, 'vostok': {'args': [], 'result': None}}
        h = H.convention_hook({'name': 'fixture', 'patch': 0}, {'site': 'test.site', 'handler': 'clear'},
                              {'test.site': site(0x20000, 'vtable')}, [f], 'vtable')
        rs = H.rs_text([h])
        self.assertNotIn('-> ()', rs)
        self.assertIn('{ crate::fixture::clear(&orig); 0 }', rs)


if __name__ == '__main__':
    unittest.main()
