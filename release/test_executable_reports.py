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

if __name__ == '__main__':
    unittest.main()
