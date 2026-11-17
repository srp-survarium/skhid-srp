"""The run logs' folder, game/logs (srp-026g/docs/notes/run-logs.md): the
real updater_skhid.exe, `check quiet`, moves into it the run logs earlier
updaters and games left beside it, and writes its own there.

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
    def test_the_run_logs_beside_it_move_into_the_logs_folder(self):
        old = {f"{X86}/updater_skhid.2026-11-15_14-03-07.log": "game/logs/updater_skhid.2026-11-15_14-03-07.log",
               f"{X86}/srp_patches.2026-11-15_14-03-09.log": "game/logs/srp_patches.2026-11-15_14-03-09.log",
               f"{X86}/srp_patches.2026-11-15_14-03-09_2.log": "game/logs/srp_patches.2026-11-15_14-03-09_2.log",
               f"{X86}/updater_skhid.seed-logs/updater_skhid.2026-11-15_14-05-00.log":
                   "game/logs/seed/updater_skhid.2026-11-15_14-05-00.log"}
        # and the unstamped logs of before the runs were named, written no more
        old.update({f"{X86}/{name}": f"game/logs/{name}"
                    for name in ("updater_skhid.log", "srp_patches.log", "srp_patches.old.log")})
        for name in old:
            self.write(name, name.encode())
        kept = [f"{X86}/launcher_skhid.log", f"{X86}/updater_skhid.notes.log"]
        for name in kept:
            self.write(name, b"not a log that moves")
        # A name the logs folder has already stays where it is.
        taken = f"{X86}/srp_patches.2026-11-15_14-03-09_2.log"
        self.write(old[taken], b"there first")
        self.assertEqual(self.run_updater(), 0, self.log())
        for name, moved in old.items():
            if name == taken:
                self.assertEqual(self.read(name), name.encode())
                self.assertEqual(self.read(moved), b"there first")
            else:
                self.assertFalse(self.exists(name), name)
                self.assertEqual(self.read(moved), name.encode())
        for name in kept:
            self.assertEqual(self.read(name), b"not a log that moves")
        self.assertFalse(self.exists(f"{X86}/updater_skhid.seed-logs"))
        self.assertEqual(run_logs.paths(self.x86), [])
        self.assertIn("moved 6 earlier runs' logs from ", self.log())
        # Its own run's file is there, and the next run moves nothing.
        self.assertEqual(len(run_logs.paths(self.path("game/logs"))), 2)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.log().count("earlier runs' logs"), 1)


if __name__ == "__main__":
    unittest.main()
