"""The executable's hash stays distinct from PE marks and counts accounts once."""
import runpy
import unittest
from pathlib import Path

moderate = runpy.run_path(str(Path(__file__).with_name('moderate')))

class ExecutableReports(unittest.TestCase):
    def test_same_hash_multiple_matches_and_accounts(self):
        digest = 'ab' * 32
        rows = [f'2026-11-12T12:00:00Z [INFO] {name} (account {account}), match 0x{match:x}, executable sha256 {digest} size 4321'
                for name, account, match in [('one', 1, 1), ('one', 1, 2), ('two', 2, 2)]]
        held, starts, dirs = moderate['module_reports']('\n'.join(rows))
        key, accounts = next(iter(held.items()))
        self.assertEqual(key, ('exe', digest, 4321))
        self.assertEqual(set(accounts), {1, 2})
        self.assertIn(f'sha256 {digest}', moderate['module_line'](key, accounts, (), ()))
        self.assertEqual((starts, dirs), ({}, {}))

    def test_a_windows_exe_file_is_told_by_its_hash_or_why_it_has_none(self):
        digest = 'cd' * 32
        row = ('2026-11-12T12:00:00Z [INFO] %s (account %d), match 0x1, minute 1, part 1/1 of 2 entries: '
               r'exe other in "C:\\Tools" "over lay.exe" %s | exe game "big.exe" too big')
        text = '\n'.join(row % (name, account, 'sha256 %s size 77' % digest) for name, account in [('one', 1), ('two', 2)])
        held, starts, dirs = moderate['module_reports'](text + '\n' + row % ('two', 2, 'unread'))
        hashed = ('file', 'other', 'over lay.exe', 'hashed', digest, 77)
        self.assertEqual(set(held), {hashed, ('file', 'other', 'over lay.exe', 'unread', '', 0), ('file', 'game', 'big.exe', 'too big', '', 0)})
        self.assertEqual((set(held[hashed]), dirs[hashed]), ({1, 2}, {'C:\\Tools'}))
        self.assertIn('sha256 %s  size 77 bytes' % digest, moderate['module_line'](hashed, held[hashed], (), dirs[hashed]))

if __name__ == '__main__':
    unittest.main()
