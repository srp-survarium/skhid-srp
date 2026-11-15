"""End-to-end tests of the game's sharing (`exe/src/seed.rs`,
`srp-026g/docs/notes/torrent-downloads.md`, "Sharing from the game"): the
real updater_skhid.exe started as the patched game starts it,
`seed \\\\.\\pipe\\skhid_seed-<pid>`, by the tests' stand-in for the game
(tests/fake_exe `game`: its pipe, the bytes the updater sends, what the game
says, the pipe closed as entering a match closes it), against
test_torrent.py's tracker and libtorrent leeches.

    cd client/updater
    SKHID_TEST_PUBLIC_KEY=$(python tests/test_torrent.py --key) \\
        SKHID_TEST_TRACKERS="$(python tests/test_torrent.py --trackers)" ./build.sh
    python tests/test_seed.py [-v] [TestName ...]

Windows, or Linux under Wine; Python with libtorrent and cryptography (as
test_torrent.py).
"""

import os
import random
import socket
import struct
import sys
import tempfile
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import test_db_patches as wine  # noqa: E402
import test_torrent as t  # noqa: E402
from test_db_patches import X86, windows, windows_path  # noqa: E402

setUpModule = t.setUpModule
tearDownModule = t.tearDownModule
harness = wine.harness
CAP = 1 << 20


class SeedTest(wine.WineTest):
    # test_torrent.py's leech, not its tests.
    leech_from_updater = t.TorrentTest.leech_from_updater

    def setUp(self):
        super().setUp()
        self.tracker = t.Tracker()
        self.addCleanup(self.tracker.close)
        self.peers_dir = tempfile.mkdtemp(prefix="skhid-seed-peers-")
        self.sessions = []
        self.game = None
        self.port = t.free_port()
        self.set_port(self.port)

    def tearDown(self):
        if self.game and self.game.poll() is None:
            self.close_pipe()
        self.sessions.clear()
        super().tearDown()

    def set_port(self, port):
        """The launcher's kept port (`port_number`), the port it shares on."""
        command, env = windows(["reg", "add", r"HKCU\Software\Vostok Games\Survarium", "/v", "port_number",
                                "/t", "REG_SZ", "/d", str(port), "/f"])
        wine.subprocess.run(command, env=env, check=True, timeout=120,
                            stdout=wine.subprocess.DEVNULL, stderr=wine.subprocess.DEVNULL)

    def installed(self, size=200_000):
        """A folder as an update left it: the revision's files in their
        places and its torrent beside the updater. The torrent and its files."""
        overlay = self.bundle()
        files = {f"game/resources.db.patch_{n}": random.randbytes(size) for n in (12, 14)}
        files["README.txt"] = overlay["README.txt"]
        files[f"{X86}/bugtrap.dll"] = overlay[f"{X86}/bugtrap.dll"]
        torrent = t.make_torrent(tempfile.mkdtemp(dir=self.peers_dir), files, self.tracker.urls)
        for name, data in files.items():
            self.write(name, data)
        self.write(f"{X86}/skhid.torrent", torrent)
        return torrent, files

    def start_game(self):
        """The stand-in game: its pipe, the updater it started, left running."""
        self.out = os.path.join(self.root, "game-pipe.bin")
        self.stop_file = os.path.join(self.root, "stop")
        self.say_file = os.path.join(self.root, "say")
        for name in (self.out, self.stop_file, self.say_file):
            if os.path.exists(name):
                os.remove(name)
        command, env = windows([harness.FAKE_EXE, "game", windows_path(self.exe()), windows_path(self.out),
                                windows_path(self.stop_file), windows_path(self.say_file)])
        self.game = wine.subprocess.Popen(command, cwd=self.x86, env=env,
                                          stdout=wine.subprocess.DEVNULL, stderr=wine.subprocess.DEVNULL)

    def seed_log(self):
        name = f"{X86}/updater_skhid.seed.log"
        return self.read(name).decode("utf-8", "replace") if self.exists(name) else ""

    log = seed_log

    def close_pipe(self, seconds=60):
        """The game enters a match: its pipe closed. The updater's exit code."""
        open(self.stop_file, "w").close()
        try:
            return self.game.wait(seconds)
        except wine.subprocess.TimeoutExpired:
            self.game.kill()
            self.fail(f"the updater did not end on the closed pipe; log:\n{self.seed_log()[-3000:]}")

    def game_said(self, *payloads):
        part = self.say_file + ".part"
        with open(part, "wb") as file:
            for payload in payloads:
                file.write(struct.pack("<H", len(payload)) + payload)
        os.replace(part, self.say_file)
        t.wait(lambda: not os.path.exists(self.say_file), 30, "the game's words read")

    def ticks(self):
        return self.said(self.out)

    def sharing(self):
        t.wait(lambda: "torrent: sharing" in self.seed_log() or self.game.poll() is not None, 60, "sharing")
        self.assertIsNone(self.game.poll(), self.seed_log()[-3000:])

    # --- the tests -------------------------------------------------------

    def test_the_game_shares_the_folder_within_the_cap_until_its_pipe_closes(self):
        # Six MiB: a leech of the updater alone needs five seconds at the cap
        # past its first second's burst.
        torrent, files = self.installed(size=3 << 20)
        self.start_game()
        self.sharing()
        self.assertIn("torrent: sharing 4 of its 4 files", self.seed_log())
        started = time.time()
        self.leech_from_updater(torrent, files)
        took = time.time() - started
        total = sum(len(data) for data in files.values())
        self.assertGreater(took, (total - CAP) / CAP * 0.9, f"{total} bytes in {took:.1f} s: over the cap")
        # No word to the game without its stats on.
        self.assertEqual(self.ticks(), [])
        self.assertEqual(self.close_pipe(), 0, self.seed_log()[-3000:])
        log = self.seed_log()
        self.assertIn("seed: the game closed the pipe: done", log)
        # Its own log alone: a launcher's updater's is never written to.
        self.assertFalse(self.exists(f"{X86}/updater_skhid.log"))

    def test_the_stats_are_ticks_only_and_only_while_shown(self):
        self.installed()
        self.start_game()
        self.sharing()
        # The game's limit, as it sends it on connect: of no use.
        self.game_said(bytes([0x1E]) + struct.pack("<Q", 256 * 1024))
        time.sleep(2)
        self.assertEqual(self.ticks(), [])
        self.game_said(bytes([0x22, 0xFF]))
        t.wait(lambda: len(self.ticks()) >= 3, 30, "three ticks")
        for tick in self.ticks():
            self.assertEqual(len(tick), 14, tick.hex())
            self.assertEqual(tick[:2], bytes([0x18, 0xFF]))
            upload, limit, peers = struct.unpack("<III", tick[2:])
            self.assertEqual(limit, CAP, "the cap, not the game's limit")
            self.assertLessEqual(upload, CAP * 2)
            self.assertLess(peers, 50)
        self.game_said(bytes([0x22, 0]))
        time.sleep(1.5)
        hidden = len(self.ticks())
        time.sleep(3)
        self.assertEqual(len(self.ticks()), hidden, "a tick after the stats were hidden")
        self.assertEqual(self.close_pipe(), 0)

    def test_a_port_another_updater_holds_is_given_up_quietly(self):
        self.installed()
        busy = socket.socket()
        busy.bind(("0.0.0.0", self.port))
        busy.listen()
        self.addCleanup(busy.close)
        self.start_game()
        started = time.time()
        try:
            code = self.game.wait(60)
        except wine.subprocess.TimeoutExpired:
            self.fail(f"still running; log:\n{self.seed_log()[-3000:]}")
        self.assertEqual(code, 0)
        self.assertGreater(time.time() - started, 8, "the port was tried for a while")
        self.assertIn("is held (another updater shares): not shared", self.seed_log())
        self.assertEqual(self.ticks(), [])

    def test_a_folder_without_a_torrent_shares_nothing(self):
        self.start_game()
        self.assertEqual(self.game.wait(60), 0)
        self.assertIn("seed: nothing to share", self.seed_log())

    def test_only_the_games_pipe_is_taken(self):
        for arguments in (["seed"], ["seed", r"\\.\pipe\vostok_updater_pipe-Survarium"],
                          ["seed", r"\\.\pipe\skhid_seed-1", "quiet"]):
            with self.subTest(arguments):
                command, env = windows([self.exe(), *arguments])
                code = wine.subprocess.run(command, cwd=self.x86, env=env, timeout=120,
                                           stdout=wine.subprocess.DEVNULL,
                                           stderr=wine.subprocess.DEVNULL).returncode
                self.assertEqual(code, 1)
        self.assertIn("seed: not the game's command line", self.seed_log())


if __name__ == "__main__":
    unittest.main()
