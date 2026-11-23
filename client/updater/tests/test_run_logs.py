"""The run logs' folder, game/logs (srp-026g/docs/notes/run-logs.md): the
real updater_skhid.exe, `check quiet`, writes its run there and deletes the
old logs earlier updaters and games left beside it.

    cd client/updater
    ./build.sh
    python tests/test_run_logs.py [-v] [TestName ...]

Windows, or Linux under Wine (test_db_patches.py's harness).
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import run_logs  # noqa: E402
import test_db_patches as wine  # noqa: E402
from test_db_patches import X86  # noqa: E402

setUpModule = wine.setUpModule
tearDownModule = wine.tearDownModule


class RunLogsTest(wine.WineTest):
    def test_the_old_logs_beside_it_are_deleted(self):
        old = [f"{X86}/updater_skhid.2026-11-15_14-03-07.log", f"{X86}/srp_patches.2026-11-15_14-03-09.log",
               f"{X86}/srp_patches.2026-11-15_14-03-09_2.log",
               f"{X86}/updater_skhid.seed-logs/updater_skhid.2026-11-15_14-05-00.log",
               # the unstamped logs of before the runs were named, written no more
               f"{X86}/updater_skhid.log", f"{X86}/srp_patches.log", f"{X86}/srp_patches.old.log"]
        for name in old:
            self.write(name, name.encode())
        kept = [f"{X86}/launcher_skhid.log", f"{X86}/updater_skhid.notes.log"]
        for name in kept:
            self.write(name, b"not an old log")
        self.assertEqual(self.run_updater(), 0, self.log())
        for name in old:
            self.assertFalse(self.exists(name), name)
        for name in kept:
            self.assertEqual(self.read(name), b"not an old log")
        self.assertFalse(self.exists(f"{X86}/updater_skhid.seed-logs"))
        self.assertIn("deleted 7 old logs in ", self.log())
        # Its own run's file in game/logs, and the next run deletes nothing.
        self.assertEqual(len(run_logs.paths(self.path("game/logs"))), 1)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.log().count("old logs in "), 1)


if __name__ == "__main__":
    unittest.main()
