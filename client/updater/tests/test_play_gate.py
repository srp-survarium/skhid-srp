"""End-to-end tests of what keeps PLAY from starting a game the server
refuses (`exe/src/gate.rs`, README "PLAY and the server's revision"): the
real updater_skhid.exe, started by the stand-in for the launcher on its pipe
(tests/fake_exe), against a web server on 127.0.0.1 in the place of srp's.

    cd client/updater
    ./build.sh
    python tests/test_play_gate.py [-v] [TestName ...]

Windows, or Linux under Wine (as test_db_patches.py, whose harness this
uses). The game exe of the archive is a small PE file whose revision site
(`logic/src/revision.rs`) says the revision a test gives it.

The scenario the players met (2026-10-21): a release put the ZIP in place
after the server's restart. In between, the server listed the new patch and
let in only the new revision while it still offered the old ZIP: the
updater found the old ZIP current, downloaded the new patch and enabled
PLAY, and the game was refused at its sign-in ("You're using an outdated
version of the game.").
"""

import os
import struct
import sys
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import test_db_patches as wine  # noqa: E402
from test_db_patches import X86, sha256, windows, windows_path  # noqa: E402

harness = wine.harness
CHECKING, LATEST, ABORTED, ERROR = harness.CHECKING, harness.LATEST, harness.ABORTED, harness.ERROR
MAINTENANCE = 0x16
setUpModule = wine.setUpModule
tearDownModule = wine.tearDownModule


def game_exe(revision):
    """A PE32 (base 0x10000) whose one section holds the revision site
    (0x380862) at file offset 0xE62, saying `revision`; `None`: the
    original game's instruction."""
    exe = bytearray(0x1600)
    exe[:2] = b"MZ"
    struct.pack_into("<I", exe, 0x3C, 0x80)
    exe[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<H", exe, 0x86, 1)          # one section
    struct.pack_into("<H", exe, 0x94, 0xE0)       # the optional header's size
    struct.pack_into("<H", exe, 0x98, 0x10B)      # PE32
    struct.pack_into("<I", exe, 0x98 + 28, 0x10000)
    section = 0x98 + 0xE0
    exe[section:section + 5] = b".text"
    struct.pack_into("<IIII", exe, section + 8, 0x1000, 0x370000, 0x1000, 0x600)
    site = bytes.fromhex("8a0d9eae9d00") if revision is None else bytes([0xC6, 0x44, 0x24, 0x27, revision, 0x90])
    exe[0xE62:0xE68] = site
    return bytes(exe)


class Launched:
    """The stand-in launcher with the updater it started, left running: what
    the updater said so far, and its end."""

    def __init__(self, test, boxes=False):
        """`boxes`: as the real launcher, it holds its instance mutex and
        shows the updater's words in a modal box, reading nothing more until
        [`click`]."""
        Launched.counter = getattr(Launched, "counter", 0) + 1
        self.test = test
        self.product = f"SkhidGate{os.getpid()}x{Launched.counter}"
        self.out = os.path.join(test.root, f"pipe-{Launched.counter}.bin")
        self.stop = os.path.join(test.root, f"stop-{Launched.counter}")
        self.clicked = os.path.join(test.root, f"clicked-{Launched.counter}")
        command, env = windows([harness.FAKE_EXE, "launcher", self.product, windows_path(test.exe()),
                                windows_path(self.out), windows_path(self.stop),
                                *([windows_path(self.clicked)] if boxes else [])])
        self.process = wine.subprocess.Popen(command, cwd=test.x86, env=env,
                                             stdout=wine.subprocess.DEVNULL, stderr=wine.subprocess.DEVNULL)
        test.launched.append(self)

    def said(self):
        return wine.WineTest.said(self.out)

    def states(self):
        return [frame[1] for frame in self.said() if frame[0] == 0x02]

    def texts(self):
        return [frame[2:2 + frame[1]].decode("ascii") for frame in self.said() if frame[0] == 0x05]

    def wait_for(self, done, seconds=120, what="it"):
        until = time.time() + seconds
        while time.time() < until:
            if done():
                return
            time.sleep(0.05)
        self.test.fail(f"no {what} in {seconds} s; states {[hex(s) for s in self.states()]}, texts {self.texts()}, "
                       f"log:\n{self.test.log()[-3000:]}")

    def wait_for_state(self, state, seconds=120):
        self.wait_for(lambda: state in self.states(), seconds, f"state {state:#x}")

    def ended(self, seconds=120):
        self.wait_for(lambda: self.process.poll() is not None, seconds, "end")
        return self.process.returncode

    def click(self):
        """OK on the box."""
        open(self.clicked, "w").close()

    def close(self):
        if self.process.poll() is None:
            self.click()
            open(self.stop, "w").close()
            try:
                self.process.wait(60)
            except wine.subprocess.TimeoutExpired:
                self.process.kill()


class GateTest(wine.WineTest):
    # The patch step's helpers, not its tests.
    patch_files = wine.PatchTest.patch_files
    assert_patches = wine.PatchTest.assert_patches
    status = wine.PatchTest.status
    STARTED = f"{X86}/launcher_skhid.exe.started.txt"

    def setUp(self):
        super().setUp()
        self.launched = []
        # The launcher the updater starts anew: the stand-in, which writes
        # down how it was started. In place before the first run, so that
        # the archive does not replace it.
        self.launcher_bytes = self.fake + b"launcher"
        self.write(f"{X86}/launcher_skhid.exe", self.launcher_bytes)

    def release(self, revision, *patches, least=None):
        """What the server offers: the archive with a game exe of
        `revision`, the patches `patches` listed, and (with `least`) the
        least revision it lets in."""
        self.offer(self.bundle(**{f"{X86}/survarium_skhid.exe": game_exe(revision),
                                  f"{X86}/launcher_skhid.exe": self.launcher_bytes}))
        self.site.patches = {n: self.patch_data(n) for n in patches}
        manifest = self.site.true_manifest()
        if least is not None:
            manifest += f"min-revision {least}\n".encode()
        self.site.manifest = manifest

    @staticmethod
    def patch_data(n):
        return bytes([n]) * (100_000 + n)

    def restarted(self):
        if not self.exists(self.STARTED):
            return None
        return self.read(self.STARTED).decode("utf-16")

    def tearDown(self):
        # The launchers closed while their folder is there.
        for launched in self.launched:
            launched.close()
        super().tearDown()

    def assert_restarted(self, launched, code=0):
        """The updater had the launcher start anew and ended with `code`
        (1: it had said why PLAY stays off)."""
        launched.wait_for(lambda: self.restarted() is not None, 120, "launcher started anew")
        self.assertIn(f"-product_name={launched.product}", self.restarted())
        self.assertEqual(launched.ended(), code)
        self.assertEqual(launched.states()[-1], ABORTED)

    # --- the gate --------------------------------------------------------

    def test_a_server_that_lets_in_a_newer_game_than_it_offers_keeps_play_off(self):
        # The old release is installed and played.
        self.release(26, 26, least=26)
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        launched.close()
        # The release half put in place: the new patch listed and revision
        # 27 asked for, the old ZIP still offered.
        self.release(26, 26, 27, least=27)
        launched = Launched(self)
        launched.wait_for_state(ERROR)
        self.assertNotIn(LATEST, launched.states())
        self.assertTrue(any("lets in game revision 27 and up; this game is revision 26" in text
                            for text in launched.texts()), launched.texts())
        self.assertIn("survarium_skhid.exe says revision 26; the server lets in 27 and up: PLAY stays off",
                      self.log())
        # The ZIP comes: the open launcher is started anew, and its updater
        # installs it, the patch with it.
        self.release(27, 26, 27, least=27)
        self.assert_restarted(launched, code=1)
        self.assertIn("the server offers archive", self.log())
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        self.assert_patches({n: self.patch_data(n) for n in (26, 27)})
        self.assertIn("survarium_skhid.exe says revision 27; the server lets in 27 and up", self.log())

    def test_without_the_gate_the_half_release_enabled_play(self):
        # What the players met: a server that does not say its least
        # revision (srp before min-revision) and the same half release. The
        # new patch is downloaded and PLAY is enabled for the old exe.
        self.release(26, 26)
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        launched.close()
        self.release(26, 26, 27)
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        self.assert_patches({n: self.patch_data(n) for n in (26, 27)})
        self.assertNotIn("PLAY stays off", self.log())

    def test_an_exe_this_updater_does_not_know_is_left_to_the_server(self):
        self.release(27, least=27)
        self.site.manifest = None
        self.offer(self.bundle(**{f"{X86}/launcher_skhid.exe": self.launcher_bytes}))
        self.site.manifest = self.site.true_manifest() + b"min-revision 27\n"
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        self.assertIn("its revision cannot be read (not an exe this updater knows)", self.log())

    def test_the_original_exe_says_revision_zero(self):
        self.release(None, least=1)
        launched = Launched(self)
        launched.wait_for_state(ERROR)
        self.assertTrue(any("this game is revision 0" in text for text in launched.texts()), launched.texts())

    def test_a_launcher_with_a_box_open_is_waited_for_and_never_doubled(self):
        # The real launcher shows the updater's words in a modal box and
        # reads its pipe, and closes, only once the box is dismissed; it
        # holds its instance mutex meanwhile. A second launcher started
        # then would say "already running" and end, and the first close on
        # the click: none left.
        self.release(26, 26, least=27)
        launched = Launched(self, boxes=True)
        launched.wait_for_state(ERROR)
        self.release(27, 26, 27, least=27)
        launched.wait_for(lambda: "the launcher starts anew" in self.log(), 60, "the restart")
        # Longer than the 15 s the wait once had: nothing started beside it.
        time.sleep(25)
        # (The updater waits first in the flush of its last message, which
        # the launcher reads only after the click, then for the mutex.)
        self.assertIsNone(self.restarted(), "a second launcher while the first holds its mutex")
        self.assertIsNone(launched.process.poll())
        # OK: it closes on the closed pipe, and the new one starts.
        launched.click()
        self.assert_restarted(launched, code=1)

    # --- the watch -------------------------------------------------------

    def test_an_open_launcher_is_started_anew_when_the_server_offers_another_archive(self):
        self.release(26, 26, least=26)
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        time.sleep(12)
        self.assertIsNone(self.restarted(), "nothing changed: nothing is started")
        self.assertEqual(launched.process.poll(), None)
        self.release(27, 26, 27, least=27)
        self.assert_restarted(launched)

    def test_play_goes_off_while_the_server_does_not_answer_and_the_launcher_restarts_when_it_does(self):
        self.release(26, 26, least=26)
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        # srp26 stopped: Caddy answers 502.
        self.site.manifest_status = 502
        launched.wait_for_state(MAINTENANCE, 60)
        self.assertIn("the server does not answer: PLAY off until it does", self.log())
        self.assertIsNone(self.restarted())
        # Back, with nothing new: the launcher is started anew all the same
        # (PLAY is off until an updater says latest again).
        self.site.manifest_status = 200
        self.assert_restarted(launched)
        self.assertIn("the server answers again", self.log())

    def test_an_offline_start_keeps_play_and_updates_once_the_server_answers(self):
        self.release(26, 26, least=26)
        launched = Launched(self)
        launched.wait_for_state(LATEST)
        launched.close()
        # The server down at the start: PLAY with what was checked, as
        # before; no "maintenance" while it stays down.
        self.site.manifest_status = 502
        self.site.patch_list_status = 502
        launched = Launched(self)
        launched.wait_for_state(LATEST, 180)
        self.assertTrue(any("No update check" in text for text in launched.texts()), launched.texts())
        time.sleep(25)
        self.assertNotIn(MAINTENANCE, launched.states())
        self.assertIsNone(self.restarted())
        # The release is out when it is back: the launcher starts anew.
        self.release(27, 26, 27, least=27)
        self.site.manifest_status = 200
        self.site.patch_list_status = 200
        self.assert_restarted(launched)

    # --- the updater replaced by `check` ---------------------------------

    def test_check_by_hand_that_replaces_the_updater_finishes_with_the_new_one(self):
        self.release(27, 27, least=27)
        files = self.bundle(**{f"{X86}/survarium_skhid.exe": game_exe(27),
                               f"{X86}/launcher_skhid.exe": self.launcher_bytes,
                               f"{X86}/updater_skhid.exe": self.updater_bytes + b"\0another build"})
        self.offer(files)
        self.site.manifest = self.site.true_manifest() + b"min-revision 27\n"
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("this updater was replaced: the new one finishes the update", self.log())
        self.assertIn("the new updater ended with 0", self.log())
        self.assertEqual(sha256(self.read(f"{X86}/updater_skhid.exe")), sha256(files[f"{X86}/updater_skhid.exe"]))
        self.assert_patches({27: self.patch_data(27)})
        self.assertEqual(self.status(), "ready\r\n")
        self.assertEqual(self.run_ready(), 0, self.log())


if __name__ == "__main__":
    unittest.main()
