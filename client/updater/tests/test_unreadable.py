"""End-to-end tests of a Skhid file the updater cannot read
(`logic/src/file_error.rs`): never "up to date" with it. The real
updater_skhid.exe, `check quiet` (no box: the exit code and the log say what
it did), against a web server on 127.0.0.1 in the place of srp's.

    cd client/updater
    ./build.sh
    python tests/test_unreadable.py [-v] [TestName ...]

Windows, or Linux under Wine (test_db_patches.py's harness). Windows
Security's own errors (225, 226) cannot be made here: what they become is
the unit tests' (`file_error`, `manifest`); a file another program holds
from everyone takes the same way (the stop, PLAY off, nothing downloaded).

What the players met (2026-10-21): survarium_skhid.exe could not be read
(error 225, Windows Security), the updater said "up to date", PLAY was on,
and the launcher crashed at PLAY.
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import test_db_patches as wine  # noqa: E402
from test_db_patches import X86  # noqa: E402

setUpModule = wine.setUpModule
tearDownModule = wine.tearDownModule
EXE = f"{X86}/survarium_skhid.exe"


class UnreadableTest(wine.WineTest):
    def setUp(self):
        super().setUp()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.site.requests.clear()
        self.mark = self.log_size()

    def test_a_file_another_program_holds_from_everyone_stops_the_update_with_play_off(self):
        locker = wine.Holder(self, EXE, mode="lock")
        self.assertEqual(self.run_updater(), 1, self.log())
        run = self.log_since(self.mark)
        self.assertIn("survarium_skhid.exe: cannot be read now: ", run)
        self.assertIn("(error 32), try 5 of 6", run)
        self.assertIn("survarium_skhid.exe: cannot be read: ", run)
        self.assertIn("STOPPED: survarium_skhid.exe is held by another program: PLAY stays off", run)
        self.assertNotIn("up to date", run)
        self.assertNotIn("/download", self.site.requests)
        # Let go: the next start finds it as it was.
        locker.release()
        mark = self.log_size()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("up to date", self.log_since(mark))
        self.assertNotIn("/download", self.site.requests)

    def test_a_file_that_cannot_be_read_for_another_reason_is_repaired_or_named(self):
        if os.name == "nt":
            self.skipTest("a file made unreadable by its mode is Wine's")
        if os.geteuid() == 0:
            self.skipTest("root reads any file")
        data = self.read(EXE)
        # No read permission: Wine says access denied (error 5), and does
        # not let it be renamed either, so the repair fails: loudly, the
        # file and Windows' words named, PLAY off.
        os.chmod(self.path(EXE), 0)
        self.assertEqual(self.run_updater(), 1, self.log())
        run = self.log_since(self.mark)
        self.assertIn("game/binaries/x86/survarium_skhid.exe: cannot be read: ", run)
        self.assertIn("(error 5)", run)
        self.assertIn("repairing game/binaries/x86/survarium_skhid.exe", run)
        self.assertIn("/download", self.site.requests)
        self.assertIn("FAILED: Cannot replace x86\\survarium_skhid.exe: ", run)
        self.assertNotIn("up to date", run)
        self.assertNotIn("state 0x0e", run)
        # Readable again: as it was.
        os.chmod(self.path(EXE), 0o644)
        self.assertEqual(self.read(EXE), data)
        mark = self.log_size()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("up to date", self.log_since(mark))


if __name__ == "__main__":
    unittest.main()
