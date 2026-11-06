"""tools/check.py against a temporary repository with dummy checks: python3 tools/test_check.py"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECK = Path(__file__).resolve().parent / 'check.py'

MANIFEST = '''# test manifest
name\tinputs\tcommand\tnotes
ignore\tnotes/**\t-
a\ta/** !a/*.skip\techo a >> "$COUNTS/a"\tthe a check
b\tb/**\techo b >> "$COUNTS/b"
fails\tf/**\techo f >> "$COUNTS/f"; echo broken; exit 3
b\tshared/**\t-
mutates\tm/**\techo more >> m/x
'''


class CheckRunner(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        base = Path(self.temp.name)
        self.repo, self.counts, self.cache = base / 'repo', base / 'counts', base / 'cache'
        self.counts.mkdir()
        for path, text in {'tools/checks.tsv': MANIFEST, 'a/x': '1', 'b/x': '1', 'f/x': '1',
                           'shared/x': '1', 'notes/n': '1', 'm/x': '1'}.items():
            (self.repo / path).parent.mkdir(parents=True, exist_ok=True)
            (self.repo / path).write_text(text)
        self.git('init', '-q', '-b', 'main')
        self.git('add', '.')
        self.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'init')
        self.git('update-ref', 'refs/remotes/origin/main', 'HEAD')

    def tearDown(self):
        self.temp.cleanup()

    def git(self, *args):
        subprocess.run(['git', '-C', str(self.repo), *args], check=True, capture_output=True)

    def write(self, path, text):
        (self.repo / path).parent.mkdir(parents=True, exist_ok=True)
        (self.repo / path).write_text(text)

    def check(self, *args):
        env = dict(os.environ, SKHID_CHECKS_CACHE=str(self.cache), COUNTS=str(self.counts))
        result = subprocess.run([sys.executable, str(CHECK), *args], cwd=self.repo, env=env,
                                capture_output=True, text=True)
        return result.returncode, result.stdout.splitlines()

    def runs(self, name):
        path = self.counts / name
        return len(path.read_text().splitlines()) if path.exists() else 0

    def test_nothing_changed_runs_nothing(self):
        status, lines = self.check()
        self.assertEqual(status, 0)
        self.assertEqual(lines, ['not affected a', 'not affected b', 'not affected fails', 'not affected mutates'])

    def test_selects_by_diff_and_caches_passes(self):
        self.write('a/x', '2')
        status, lines = self.check()
        self.assertEqual(status, 0)
        self.assertIn('not affected b', lines)
        self.assertTrue(any(line.startswith('ran pass a ') for line in lines), lines)
        status, lines = self.check()
        self.assertTrue(any(line.startswith('cached pass a (') for line in lines), lines)
        self.assertEqual(self.runs('a'), 1)
        self.assertEqual(self.runs('b'), 0)

    def test_reruns_on_input_change_and_a_revert_is_cached(self):
        self.write('a/x', '2')
        self.check()
        self.write('a/x', '3')
        self.check()
        self.assertEqual(self.runs('a'), 2)
        self.write('a/x', '2')
        status, lines = self.check()
        self.assertTrue(any(line.startswith('cached pass a') for line in lines), lines)
        self.assertEqual(self.runs('a'), 2)

    def test_committed_staged_and_untracked_changes_count(self):
        self.write('a/x', '2')
        self.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qam', 'a')
        self.check()
        self.assertEqual(self.runs('a'), 1)
        self.write('a/new', 'n')
        self.check()
        self.assertEqual(self.runs('a'), 2)
        self.git('add', 'a/new')
        status, lines = self.check()
        self.assertTrue(any(line.startswith('cached pass a') for line in lines), lines)
        self.write('a/y.skip', 'excluded')
        status, lines = self.check()
        self.assertTrue(any(line.startswith('cached pass a') for line in lines), lines)
        self.assertIn('not covered: a/y.skip', lines)

    def test_failures_are_never_cached(self):
        self.write('f/x', '2')
        status, lines = self.check()
        self.assertEqual(status, 1)
        self.assertIn('  | broken', lines)
        self.assertTrue(any(line.startswith('ran FAIL fails (exit 3') for line in lines), lines)
        status, lines = self.check()
        self.assertEqual(status, 1)
        self.assertEqual(self.runs('f'), 2)

    def test_prints_uncovered_files_but_not_ignored_ones(self):
        self.write('c.txt', 'x')
        self.write('notes/n', '2')
        status, lines = self.check()
        self.assertEqual(status, 0)
        self.assertIn('not covered: c.txt', lines)
        self.assertNotIn('not covered: notes/n', lines)

    def test_extra_inputs_row_and_named_checks(self):
        self.write('shared/x', '2')
        self.check()
        self.assertEqual(self.runs('b'), 1)
        status, lines = self.check('a')
        self.assertEqual(status, 0)
        self.assertEqual(self.runs('a'), 1)
        self.assertNotIn('not affected b', lines)
        status, lines = self.check('a')
        self.assertTrue(any(line.startswith('cached pass a') for line in lines), lines)
        self.check('--no-cache', 'a')
        self.assertEqual(self.runs('a'), 2)
        self.assertEqual(self.check('nope')[0], 2)

    def test_a_changed_row_reruns(self):
        self.write('a/x', '2')
        self.check()
        self.write('tools/checks.tsv', MANIFEST.replace('the a check', 'the a check, noted'))
        status, lines = self.check()
        self.assertIn('not covered: tools/checks.tsv', lines)
        self.assertTrue(any(line.startswith('cached pass a') for line in lines), 'notes are not in the key')
        self.write('tools/checks.tsv', MANIFEST.replace('echo a >>', 'echo A >>'))
        self.check()
        self.assertEqual(self.runs('a'), 2)

    def test_a_pass_on_inputs_changed_while_it_ran_is_not_recorded(self):
        status, lines = self.check('mutates')
        self.assertEqual(status, 0)
        self.assertTrue(any('not recorded' in line for line in lines), lines)
        status, lines = self.check('mutates')
        self.assertTrue(any(line.startswith('ran pass mutates') for line in lines), lines)

    def test_list(self):
        self.write('b/x', '2')
        status, lines = self.check('--list')
        self.assertEqual(status, 0)
        self.assertEqual([line.split()[:2] for line in lines[:3]], [['a', '-'], ['b', 'affected'], ['fails', '-']])
        self.assertEqual(self.runs('b'), 0)


if __name__ == '__main__':
    unittest.main()
