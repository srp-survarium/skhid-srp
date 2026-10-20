import difflib
import json
from pathlib import Path
import tempfile
import unittest

import source_patches as patches
import link_layout


class SourcePatchTests(unittest.TestCase):
    def test_method_reservations_and_saved_plan_authentication(self):
        self.assertIsNone(patches.reservation_plan({}))
        entry = {'compiler': 'flex-library', 'method_reservations': {'C/method': {'stack': 3, 'locals': 4}}}
        plan = patches.reservation_plan(entry)
        self.assertEqual(plan, b'C/method\t3\t4\n')
        for spec in ({}, [], None, {'x': {}}, {'x': {'code': 3}}, {'x': {'stack': True}},
                     {'x': {'locals': -1}}, {'x': {'stack': 65536}}, {'x': {'stack': 0}},
                     {'x': {'stack': '3'}}, {'x\n': {'stack': 3}}, {3: {'stack': 3}}):
            with self.assertRaises(ValueError):
                patches.reservation_plan({'method_reservations': spec})
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'method-reservations.tsv').write_bytes(plan)
            (root / 'unreserved-code.swf').write_bytes(b'compiler output')
            row = {'compiler': 'flex-library', 'reservations_sha256': patches.sha(plan),
                   'unreserved_code_sha256': patches.sha(b'compiler output')}
            link_layout.verify_compiler_plans(row, entry, root)
            for name, changed in [('method-reservations.tsv', b'C/method\t9\t4\n'),
                                  ('unreserved-code.swf', b'changed compiler output')]:
                saved = (root / name).read_bytes()
                (root / name).write_bytes(changed)
                with self.assertRaises(ValueError):
                    link_layout.verify_compiler_plans(row, entry, root)
                (root / name).write_bytes(saved)
            with self.assertRaises(ValueError):
                link_layout.verify_compiler_plans(dict(row, reservations_sha256='stale'), entry, root)
            with self.assertRaises(ValueError):
                link_layout.verify_compiler_plans(row, {}, root)
            mixed = {'compiler': 'flex-mixed', 'composition': {'ffdec_methods': ['C/method']}}
            composition = patches.composition_plan(mixed)
            (root / 'compiler-composition.tsv').write_bytes(composition)
            mixed_row = {'compiler': 'flex-mixed', 'composition_sha256': patches.sha(composition)}
            link_layout.verify_compiler_plans(mixed_row, mixed, root)
            with self.assertRaises(ValueError):
                link_layout.verify_compiler_plans({'compiler': 'flex-mixed'}, {}, root)
            with self.assertRaises(ValueError):
                link_layout.verify_compiler_plans(dict(mixed_row, compiler='ffdec'), mixed, root)
            (root / 'compiler-composition.tsv').write_bytes(b'C/other\n')
            with self.assertRaises(ValueError):
                link_layout.verify_compiler_plans(mixed_row, mixed, root)

    def test_composition_plan_requires_explicit_unique_method_choices(self):
        owner = 'QName(PackageNamespace(""),"C")/instance/1/QName(PackageNamespace(""),"f")'
        entry = {'compiler': 'flex-mixed',
                 'composition': {'ffdec_methods': [owner], 'script_traits': 'ffdec'}}
        self.assertEqual(patches.composition_plan(entry), ('script_traits=ffdec\n' + owner + '\n').encode())
        source_entry = dict(entry, composition=dict(entry['composition'], ffdec_compiler='ffdec-source'))
        self.assertEqual(patches.composition_plan(source_entry),
                         ('ffdec_compiler=ffdec-source\nscript_traits=ffdec\n' + owner + '\n').encode())
        hidden_entry = dict(source_entry, composition=dict(source_entry['composition'],
                            ffdec_hidden_static=['example.Dependency/helper']))
        self.assertEqual(patches.composition_plan(hidden_entry),
                         ('ffdec_compiler=ffdec-source\nffdec_hide_static=example.Dependency/helper\n'
                          'script_traits=ffdec\n' + owner + '\n').encode())
        for value in ('Dependency/helper', ['Dependency/helper', 'Dependency/helper'],
                      ['../Dependency/helper'], ['Dependency/helper\n'], ['Dependency/field/subfield']):
            with self.assertRaises(ValueError):
                patches.composition_plan(dict(source_entry, composition=dict(source_entry['composition'],
                                         ffdec_hidden_static=value)))
        self.assertIsNone(patches.composition_plan({'compiler': 'flex-library'}))
        for broken in [
            {'compiler': 'flex-mixed'},
            dict(entry, compiler='flex-library'),
            dict(entry, composition={'ffdec_methods': []}),
            dict(entry, composition={'ffdec_methods': owner}),
            dict(entry, composition={'ffdec_methods': [owner, owner]}),
            dict(entry, composition={'ffdec_methods': [owner + '\n']}),
            dict(entry, composition={'ffdec_methods': [owner], 'script_traits': 'reference'}),
            dict(entry, composition={'ffdec_methods': [owner], 'fallback': 'reference'}),
            dict(entry, composition={'ffdec_methods': [owner], 'ffdec_compiler': 'reference'}),
            dict(entry, composition={'ffdec_methods': ['ffdec_compiler=ffdec-source']}),
            dict(entry, composition={'ffdec_methods': ['script_traits=ffdec']}),
            dict(entry, composition={'ffdec_methods': ['ffdec_hide_static=Dependency/helper']}),
            dict(entry, composition={'ffdec_methods': [owner], 'ffdec_hidden_static': ['Dependency/helper']}),
        ]:
            with self.assertRaises(ValueError):
                patches.composition_plan(broken)

    def patch(self, before, after):
        return ''.join(difflib.unified_diff(before.splitlines(keepends=True), after.splitlines(keepends=True), fromfile='a/C.as', tofile='b/C.as')).encode()

    def test_exact_positions_and_line_endings(self):
        before, after = 'a\nb\nc\n', 'a\nchanged\nc\nextra\n'
        diff = self.patch(before, after)
        self.assertEqual(patches.apply_patch(before.encode(), diff, 'C.as'), after.encode())
        self.assertEqual(patches.apply_patch(before.replace('\n', '\r\n').encode(), diff, 'C.as'), after.replace('\n', '\r\n').encode())
        for bad in (b'prefix\n' + before.encode(), b'a\nwrong\nc\n', b'a\r\nb\nc\n'):
            with self.assertRaises(ValueError):
                patches.apply_patch(bad, diff, 'C.as')
        for bad in (diff.replace(b'b/C.as', b'b/other.as'), diff.replace(b'+1,4', b'+2,4'), diff.replace(b'-1,3', b'-1,2'), diff + b'\\ No newline at end of file\n'):
            with self.assertRaises(ValueError):
                patches.apply_patch(before.encode(), bad, 'C.as')

    def test_empty_range_insertions_and_deletions(self):
        for before, after in [('a\n', 'first\na\n'), ('a\nb\n', 'a\n'), ('a\n', 'a\nlast\n')]:
            diff = ''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True), fromfile='a/C.as', tofile='b/C.as', n=0)).encode()
            self.assertEqual(patches.apply_patch(before.encode(), diff, 'C.as'), after.encode())

    def test_manifest_authenticates_movie_source_patch_and_result(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'pristine').mkdir()
            before, after = b'class C {}\n', b'class C { public var value:int; }\n'
            source = root / 'pristine/C.as'
            source.write_bytes(before)
            diff = self.patch(before.decode(), after.decode())
            (root / 'C.patch').write_bytes(diff)
            pinned = {'release': 'test', 'movie_sha256': 'movie', 'sources': {'C.as': patches.sha(before)}}
            entry = {'class': 'C', 'patch': 'C.patch', 'patch_sha256': patches.sha(diff), 'pristine_sha256': patches.sha(before), 'result_sha256': patches.sha(after)}
            spec = {'schema': 1, 'release': 'test', 'movie_sha256': 'movie', 'classes': [entry]}
            manifest = root / 'manifest.json'
            manifest.write_text(json.dumps(spec))
            self.assertEqual(patches.load_sources(manifest, root, pinned)[0], {'C.as': after})
            self.assertEqual(source.read_bytes(), before)
            for field in ('patch_sha256', 'pristine_sha256', 'result_sha256'):
                broken = dict(spec, classes=[dict(entry, **{field: 'changed'})])
                manifest.write_text(json.dumps(broken))
                with self.assertRaises(ValueError):
                    patches.load_sources(manifest, root, pinned)
            for path in ('../outside.patch', '/tmp/outside.patch'):
                manifest.write_text(json.dumps(dict(spec, classes=[dict(entry, patch=path)])))
                with self.assertRaises(ValueError):
                    patches.load_sources(manifest, root, pinned)
            manifest.write_text(json.dumps(dict(spec, classes=[entry, entry])))
            with self.assertRaises(ValueError):
                patches.load_sources(manifest, root, pinned)
            manifest.write_text(json.dumps(dict(spec, movie_sha256='different')))
            with self.assertRaises(ValueError):
                patches.load_sources(manifest, root, pinned)
            manifest.write_text(json.dumps(dict(spec, classes=[dict(entry, compiler='unknown')])))
            with self.assertRaisesRegex(ValueError, 'compiler'):
                patches.load_sources(manifest, root, pinned)


if __name__ == '__main__':
    unittest.main()
