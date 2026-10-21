"""End-to-end tests of the patch step and of the restore of resources.db: the
real updater_skhid.exe, `check quiet` (no launcher, no box: the exit code,
the log and the files tell what it did; every patch file hashed) or started
by a stand-in for the launcher on its pipe (the quick check), a web server
on 127.0.0.1 in the place of the srp one that lists resources.db's patches
at `/patches` with the addresses of each file (on itself, or on other
servers), and `ready` and play_skhid.bat after it.

    cd client/updater
    ./build.sh
    python tests/test_db_patches.py [-v] [TestName ...]

SKHID_TEST_OLD_UPDATER=<updater_skhid.exe 0.2.0> (client/updater of main
before 0.3.0, built) also runs the release that brings this updater to a
game of that one.

On Windows the exe runs as it is; elsewhere under Wine (`wine` on the PATH,
or SKHID_TEST_WINE; a prefix of its own in a temporary folder unless
SKHID_TEST_WINEPREFIX names one). What Python cannot do in Windows' place,
hold a file as the game or an antivirus does and be the launcher's end of
its pipe, the tests' stand-in program does (tests/fake_exe). Nothing
outside the temporary folders is touched; no game file is needed.
"""

import json
import os
import random
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import types
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
WINDOWS = sys.platform == "win32"
if not WINDOWS:
    # test_updater.py's harness, without its Windows-only parts (the
    # launcher's pipe): its site and its archive.
    import ctypes
    sys.modules.setdefault("_winapi", types.ModuleType("_winapi"))
    ctypes.WinDLL = lambda *args, **kwargs: types.SimpleNamespace(
        CreateMutexW=types.SimpleNamespace(), CloseHandle=types.SimpleNamespace())
import test_updater as harness  # noqa: E402
# The players' play_skhid.bat, as the release builder writes it.
sys.path.insert(0, os.path.join(HERE, "..", "..", "..", "srp-026g", "client-patches"))
import make_client_026g as builder  # noqa: E402

X86 = harness.X86
sha256 = harness.sha256
CHECKING, DOWNLOADING, LATEST, ABORTED, ERROR = (harness.CHECKING, harness.DOWNLOADING, harness.LATEST,
                                                 harness.ABORTED, harness.ERROR)
WINE = os.environ.get("SKHID_TEST_WINE") or shutil.which("wine")
PREFIX = None


def setUpModule():
    global PREFIX
    if WINDOWS:
        return
    if not WINE:
        raise unittest.SkipTest("no wine: set SKHID_TEST_WINE")
    PREFIX = os.environ.get("SKHID_TEST_WINEPREFIX")
    if not PREFIX:
        PREFIX = tempfile.mkdtemp(prefix="skhid-patches-wine-")
        subprocess.run([WINE, "wineboot", "-i"], env=wine_env(), check=True, timeout=300,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def tearDownModule():
    if PREFIX and not os.environ.get("SKHID_TEST_WINEPREFIX"):
        subprocess.run([str(WINE) + "server", "-k"], env=wine_env(), check=False)
        shutil.rmtree(PREFIX, ignore_errors=True)


def wine_env():
    env = dict(os.environ, WINEPREFIX=PREFIX or "", WINEDEBUG="-all",
               WINEDLLOVERRIDES="mscoree,mshtml=;winemenubuilder.exe=d")
    # No window of any kind.
    for name in ("DISPLAY", "WAYLAND_DISPLAY"):
        env.pop(name, None)
    return env


def windows(command):
    """A command line for a Windows program: as it is on Windows, by Wine
    elsewhere; and its environment."""
    return (command, None) if WINDOWS else ([WINE, *command], wine_env())


def windows_path(path):
    """`path` as the Windows programs see it (Wine's Z: is /)."""
    path = os.path.abspath(path)
    return path if WINDOWS else "Z:" + path.replace("/", "\\")


def frames(data):
    """The updater's messages on the pipe: their bytes, the id first."""
    out = []
    while len(data) >= 2:
        (length,) = struct.unpack_from("<H", data)
        if len(data) < 2 + length:
            break
        out.append(data[2:2 + length])
        data = data[2 + length:]
    return out


class Holder:
    """Another Windows program that holds a file of the game: the game its
    resources.db, an antivirus a file it scans. It may read and write the
    file but not delete or rename it, as theirs."""

    counter = 0

    def __init__(self, test, name, wait=True):
        Holder.counter += 1
        self.held = os.path.join(test.root, f"held-{Holder.counter}")
        self.let_go = os.path.join(test.root, f"release-{Holder.counter}")
        command, env = windows([harness.FAKE_EXE, "hold", windows_path(test.path(name)),
                                windows_path(self.held), windows_path(self.let_go)])
        self.process = subprocess.Popen(command, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        test.addCleanup(self.release)
        if wait:
            self.wait()

    def wait(self, seconds=60):
        until = time.time() + seconds
        while not os.path.exists(self.held):
            if time.time() > until or self.process.poll() is not None:
                raise AssertionError("the stand-in did not get hold of the file")
            time.sleep(0.02)

    def holding(self):
        return os.path.exists(self.held)

    def release(self):
        if self.process.poll() is None:
            open(self.let_go, "w").close()
            try:
                self.process.wait(30)
            except subprocess.TimeoutExpired:
                self.process.kill()


class WineTest(unittest.TestCase):
    setUp = harness.UpdaterTest.setUp
    path = harness.UpdaterTest.path
    write = harness.UpdaterTest.write
    read = harness.UpdaterTest.read
    exists = harness.UpdaterTest.exists
    bundle = harness.UpdaterTest.bundle
    offer = harness.UpdaterTest.offer
    log = harness.UpdaterTest.log

    def tearDown(self):
        for site in getattr(self, "others", []):
            site.close()
        harness.UpdaterTest.tearDown(self)

    def exe(self):
        return os.path.join(self.x86, "updater_skhid.exe")

    def run_updater(self, files=None):
        """One `check quiet` run against the archive offered (by default the
        harness's overlay); its exit code."""
        if files is not None:
            self.offer(files)
        elif not self.site.archive:
            self.offer(self.bundle())
        command, env = windows([self.exe(), "check", "quiet"])
        return subprocess.run(command, cwd=self.x86, env=env, timeout=300,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode

    def start_updater(self):
        """`check quiet`, not waited for."""
        if not self.site.archive:
            self.offer(self.bundle())
        command, env = windows([self.exe(), "check", "quiet"])
        return subprocess.Popen(command, cwd=self.x86, env=env,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def run_launched(self, stop_when=None, seconds=300):
        """One run as the launcher starts it: the stand-in makes the pipe,
        starts the updater and reads it. It says stop (the launcher's abort)
        when `stop_when()` is true, or once the updater says "latest" or
        "error". Returns the exit code and the updater's messages."""
        if not self.site.archive:
            self.offer(self.bundle())
        out = os.path.join(self.root, "pipe.bin")
        stop = os.path.join(self.root, "stop")
        for name in (out, stop):
            if os.path.exists(name):
                os.remove(name)
        Holder.counter += 1
        command, env = windows([harness.FAKE_EXE, "launcher", f"SkhidTest{os.getpid()}x{Holder.counter}",
                                windows_path(self.exe()), windows_path(out), windows_path(stop)])
        process = subprocess.Popen(command, cwd=self.x86, env=env,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        until = time.time() + seconds
        said = []
        while process.poll() is None:
            if time.time() > until:
                process.kill()
                self.fail(f"the run did not end; it said {said}")
            said = self.said(out)
            states = [frame[1] for frame in said if frame[0] == 0x02]
            if not os.path.exists(stop) and (LATEST in states or ERROR in states or (stop_when and stop_when())):
                open(stop, "w").close()
            time.sleep(0.02)
        said = self.said(out)
        return process.returncode, said

    @staticmethod
    def said(out):
        if not os.path.exists(out):
            return []
        with open(out, "rb") as file:
            return frames(file.read())

    def run_ready(self):
        """`updater_skhid.exe ready quiet` (what play_skhid.sh runs): its
        exit code."""
        command, env = windows([self.exe(), "ready", "quiet"])
        return subprocess.run(command, cwd=self.x86, env=env, timeout=300,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode

    def run_bat(self):
        """The players' play_skhid.bat, as make_client_026g.py writes it, run
        by cmd; whether it started the game (the stand-in writes down that
        it was started)."""
        started = os.path.join(self.x86, "survarium_skhid.exe.started.txt")
        if os.path.exists(started):
            os.remove(started)
        self.write(f"{X86}/play_skhid.bat", builder.bat("srp.example:1234", "english", True))
        command, env = windows(["cmd", "/c", "play_skhid.bat"])
        code = subprocess.run(command, cwd=self.x86, env=env, timeout=120,
                              stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        # `start` does not wait for the game.
        for _ in range(100):
            if os.path.exists(started):
                break
            time.sleep(0.1)
        return code.returncode, os.path.exists(started), code.stdout.decode("ascii", "replace")

    def log_since(self, mark):
        return self.log()[mark:]

    def log_size(self):
        return len(self.log()) if self.exists(f"{X86}/updater_skhid.log") else 0


class PatchTest(WineTest):
    def make_patches(self, *numbers, size=200_000):
        patches = {n: random.Random(n).randbytes(size + n) for n in numbers}
        self.site.patches.update(patches)
        return patches

    def patch_name(self, n):
        return f"game/resources.db.patch_{n}"

    def patch_files(self):
        """The patch files in the game folder: name -> bytes."""
        return {name: self.read(f"game/{name}")
                for name in sorted(os.listdir(self.path("game"))) if name.lower().startswith("resources.db.patch_")}

    def assert_patches(self, patches):
        self.assertEqual({name: sha256(data) for name, data in self.patch_files().items()},
                         {f"resources.db.patch_{n}": sha256(data) for n, data in patches.items()})

    def store_requests(self, site=None):
        return [path for path in (site or self.site).requests if path.startswith("/store/")]

    RECORD = f"{X86}/updater_skhid.patches.checked"

    def record(self):
        """The quick check's record: patch file name -> its words after the
        name (sha256, size, written, volume, index, hashed)."""
        if not self.exists(self.RECORD):
            return None
        lines = self.read(self.RECORD).decode().split("\r\n")
        self.assertEqual((lines[0], lines[-2:]), ("skhid-patches-checked 1", ["end", ""]))
        return {line.split(" ")[1]: line.split(" ")[2:] for line in lines[1:-2]}

    def hashed_in(self, run):
        """How many patch files a run's log says it hashed."""
        for line in run.splitlines():
            if "patches: " in line and " file(s), " in line and " hashed in " in line:
                return int(line.split("patches: ")[1].split(" ")[0])
        return 0

    def status(self):
        name = f"{X86}/updater_skhid.patches.status"
        return self.read(name).decode() if self.exists(name) else None

    def assert_blocked(self, *said, status="blocked"):
        """PLAY is off, the player told `said`. `status`: what the status
        file says now: `blocked <why>` (the folder was to change), or as it
        was before (the run changed nothing)."""
        if status == "blocked":
            self.assertTrue((self.status() or "").startswith("blocked "), self.status())
        else:
            self.assertEqual(self.status(), status)
        log = self.log()
        self.assertIn("patches: BLOCKED:", log)
        self.assertIn("PLAY stays off until this is done", log)
        for words in said:
            self.assertIn(words, log)
        self.assertEqual([name for name in os.listdir(self.path("game")) if name.endswith(".skhid-part")], [])

    # --- the normal runs -------------------------------------------------

    def test_a_fresh_install_downloads_every_patch_gaps_and_all_and_then_nothing(self):
        patches = self.make_patches(1, 14, 17, 255)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        self.assertEqual(sorted(self.store_requests()), sorted(f"/store/patch_{n}" for n in patches))
        self.assertEqual(self.status(), "ready\r\n")
        kept = json.loads(self.read(f"{X86}/updater_skhid.patches"))
        self.assertEqual(sorted(kept), ["patch_1", "patch_14", "patch_17", "patch_255"])
        self.assertEqual(kept["patch_14"]["sha256"], sha256(patches[14]))
        self.assertIn("patches: 4 patch(es) in place, checked", self.log())
        self.assertEqual(self.run_ready(), 0, self.log())
        # The next start: every file hashed once, nothing downloaded or
        # written.
        self.site.requests.clear()
        mark = self.log_size()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.store_requests(), [])
        run = self.log_since(mark)
        self.assertIn("0 to download, 0 to delete", run)
        self.assertEqual(run.count("patches: 4 file(s), "), 1, run)
        self.assert_patches(patches)

    def test_the_launcher_is_told_while_the_patches_are_checked(self):
        patches = self.make_patches(14, 17, size=3_000_000)
        self.assertEqual(self.run_updater(), 0, self.log())
        # Without the record of the hashes, the launcher's run hashes both.
        os.remove(self.path(self.RECORD))
        code, said = self.run_launched()
        self.assertEqual(code, 0, self.log())
        states = [frame[1] for frame in said if frame[0] == 0x02]
        self.assertEqual(states[-3:], [CHECKING, LATEST, ABORTED], states)
        self.assertGreaterEqual(states.count(CHECKING), 2, "the overlay's question, then the patches")
        # The bar over the bytes of every patch, to the end.
        total = sum(len(data) for data in patches.values())
        progress = [struct.unpack_from("<QQ", frame, 1) for frame in said if frame[0] == 0x01]
        self.assertIn((total, total), progress)
        # The next one reads neither: no "checking" but the overlay's.
        mark = self.log_size()
        code, said = self.run_launched()
        self.assertEqual(code, 0, self.log())
        states = [frame[1] for frame in said if frame[0] == 0x02]
        self.assertEqual(states.count(CHECKING), 1, states)
        self.assertIn("patches: 2 file(s) as they were when hashed in the last week: not read again",
                      self.log_since(mark))

    def test_an_empty_list_has_nothing_to_do(self):
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.patch_files(), {})
        self.assertEqual(self.status(), "ready\r\n")
        self.assertEqual(self.run_ready(), 0, self.log())

    def test_the_files_may_be_on_another_server(self):
        files = harness.Site()
        self.others = [files]
        patches = self.make_patches(14, 20)
        files.patches.update(patches)
        self.site.patches.clear()
        self.site.patch_list = json.dumps({f"patch_{n}": {"sha256": sha256(data), "url": f"{files.url}/store/patch_{n}"}
                                           for n, data in patches.items()}).encode()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        self.assertEqual(self.store_requests(), [])
        self.assertEqual(sorted(self.store_requests(files)), ["/store/patch_14", "/store/patch_20"])

    def test_one_missing_is_downloaded_alone(self):
        patches = self.make_patches(14, 17, 20)
        self.assertEqual(self.run_updater(), 0, self.log())
        os.remove(self.path(self.patch_name(17)))
        self.assertEqual(self.run_ready(), 1, "a patch is missing")
        self.site.requests.clear()
        mark = self.log_size()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.store_requests(), ["/store/patch_17"])
        self.assert_patches(patches)
        # Hashed once before, and only the new file after.
        self.assertEqual(self.log_since(mark).count("patches: 3 file(s), "), 1)

    def test_one_damaged_here_is_downloaded_again(self):
        patches = self.make_patches(14, 17)
        self.assertEqual(self.run_updater(), 0, self.log())
        with open(self.path(self.patch_name(14)), "r+b") as file:
            file.seek(1000)
            file.write(b"damaged")
        self.assertEqual(self.run_ready(), 1, "a patch is damaged")
        self.site.requests.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.store_requests(), ["/store/patch_14"])
        self.assertIn("resources.db.patch_14 does not have the list's checksum: downloaded again", self.log())
        self.assert_patches(patches)

    def test_a_patch_the_list_has_not_is_deleted(self):
        patches = self.make_patches(14, 15)
        self.assertEqual(self.run_updater(), 0, self.log())
        # Names the game does not take, a download left over, and a patch
        # the list never had.
        for name in ("resources.db.patch_014", "resources.db.patch_0", "resources.db.patch_300",
                     "resources.db.patch_14.skhid-part", "resources.db.patch_16"):
            self.write(f"game/{name}", b"stray")
        self.write("game/resources.db.skhid-keep", b"not a patch file")
        self.assertEqual(self.run_ready(), 1, "extra patch files")
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        self.assertIn("patches: deleted resources.db.patch_16", self.log())
        self.assertTrue(self.exists("game/resources.db.skhid-keep"))
        self.assertEqual(self.run_ready(), 0)
        # A rollback: the server drops patch 15.
        del self.site.patches[15]
        self.site.requests.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches({14: patches[14]})
        self.assertEqual(self.store_requests(), [])
        self.assertEqual(self.read("game/resources.db"), b"existing game resources")

    # --- what keeps PLAY off, and is repaired --------------------------------

    def test_a_server_file_that_is_wrong_keeps_play_off(self):
        patches = self.make_patches(1, 14)
        self.site.patch_bodies[14] = b"not patch 14"
        self.assertEqual(self.run_updater(), 1)
        # Tried once (another file would not be better a second time), never
        # put in place; the good one is.
        self.assertEqual(self.store_requests().count("/store/patch_14"), 1)
        self.assertFalse(self.exists(self.patch_name(14)))
        self.assertEqual(self.read(self.patch_name(1)), patches[1])
        self.assert_blocked("sent game\\resources.db.patch_14 with another checksum than its list says",
                            "(12 bytes) has SHA-256")
        self.assertEqual(self.run_ready(), 1)
        # The server has it right now: the next start does it.
        self.site.patch_bodies.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        self.assertEqual(self.run_ready(), 0)

    def test_a_redirect_to_another_server_is_followed(self):
        patches = self.make_patches(14)
        elsewhere = harness.Site()
        self.others = [elsewhere]
        elsewhere.patches.update(patches)
        self.site.patch_redirects[14] = f"{elsewhere.url}/store/patch_14"
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.store_requests(elsewhere), ["/store/patch_14"])
        self.assert_patches(patches)
        self.assertIn(f"/store/patch_14 redirects to {elsewhere.url}/store/patch_14", self.log())

    # --- several addresses, redirects ------------------------------------

    def two_sites(self):
        others = [harness.Site(), harness.Site()]
        self.others = others
        return others

    def test_the_first_address_down_the_second_gives_the_file(self):
        patches = self.make_patches(14, 20)
        down = harness.Site()
        dead = down.url
        down.close()
        self.site.patch_urls = {n: [f"{dead}/store/patch_{n}", f"{self.site.url}/store/patch_{n}"]
                                for n in patches}
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        self.assertEqual(sorted(self.store_requests()), ["/store/patch_14", "/store/patch_20"])
        log = self.log()
        self.assertIn(f"patches: resources.db.patch_14: address 1 of 2, {dead}/store/patch_14 (try 1 of 2): "
                      "Cannot connect to 127.0.0.1", log)
        self.assertIn(f"patches: resources.db.patch_14: downloaded {self.site.url}/store/patch_14 "
                      "(address 2 of 2), checksum right", log)
        # As srp writes it, the list kept has both, in their order.
        kept = json.loads(self.read(f"{X86}/updater_skhid.patches"))
        self.assertEqual(kept["patch_14"]["urls"], self.site.patch_urls[14])
        self.assertEqual(self.run_ready(), 0)

    def test_a_web_page_in_the_place_of_the_file_moves_on_to_the_next_address(self):
        patches = self.make_patches(14)
        drive, files = self.two_sites()
        drive.patches.update(patches)
        drive.patch_html.add(14)
        files.patches.update(patches)
        self.site.patches.clear()
        self.site.patches[14] = patches[14]
        self.site.patch_urls = {14: [f"{drive.url}/store/patch_14", f"{files.url}/store/patch_14"]}
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        self.assertEqual(self.store_requests(drive), ["/store/patch_14"], "a web page is not asked for twice")
        self.assertEqual(self.store_requests(files), ["/store/patch_14"])
        self.assertIn("127.0.0.1 answered /store/patch_14 with a web page, not the file", self.log())

    def test_a_file_of_another_checksum_moves_on_to_the_next_address(self):
        patches = self.make_patches(14)
        stale, files = self.two_sites()
        stale.patches[14] = b"an older patch 14"
        files.patches.update(patches)
        self.site.patch_urls = {14: [f"{stale.url}/store/patch_14", f"{files.url}/store/patch_14"]}
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        self.assertEqual(self.store_requests(stale), ["/store/patch_14"])
        self.assertIn(f"address 1 of 2, {stale.url}/store/patch_14 (try 1 of 2): {stale.url}/store/patch_14 "
                      "(17 bytes) has SHA-256", self.log())
        self.assertEqual(self.store_requests(), [])

    def test_every_address_failing_keeps_play_off(self):
        patches = self.make_patches(14)
        down = harness.Site()
        dead = down.url
        down.close()
        drive, stale = self.two_sites()
        drive.patches.update(patches)
        drive.patch_html.add(14)
        stale.patches[14] = b"wrong"
        self.site.patch_urls = {14: [f"{dead}/store/patch_14", f"{drive.url}/store/patch_14",
                                     f"{stale.url}/store/patch_14"]}
        self.assertEqual(self.run_updater(), 1)
        self.assertFalse(self.exists(self.patch_name(14)))
        self.assert_blocked("game\\resources.db.patch_14 could not be downloaded from any of its 3 addresses",
                            "The first: Cannot connect to 127.0.0.1",
                            f"address 1 of 3, {dead}/store/patch_14 (try 1 of 2)",
                            f"address 1 of 3, {dead}/store/patch_14 (try 2 of 2)",
                            f"address 2 of 3, {drive.url}/store/patch_14 (try 1 of 2): ",
                            f"address 3 of 3, {stale.url}/store/patch_14 (try 1 of 2): ")
        # The network may come back; a web page or another file would not.
        self.assertEqual(self.store_requests(drive), ["/store/patch_14"])
        self.assertEqual(self.store_requests(stale), ["/store/patch_14"])
        self.assertNotIn("(try 2 of 2): " + f"{stale.url}", self.log())
        self.assertEqual(self.run_ready(), 1)
        # One address put right: the next start does it.
        stale.patches[14] = patches[14]
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_chain_of_five_redirects_is_followed_and_six_are_not(self):
        patches = self.make_patches(14, 20)
        files = harness.Site()
        self.others = [files]
        files.patches.update(patches)
        # Five hops: on this host, to the files' server, down to its plain
        # HTTP (all are, here), by turns absolute and on the host.
        self.site.patch_redirects[14] = f"{files.url}/hop/4/patch_14"
        self.site.patch_urls = {14: [f"{self.site.url}/store/patch_14"],
                                20: [f"{files.url}/hop/6/patch_20", f"{files.url}/hop/5/patch_20"]}
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        log = self.log()
        self.assertIn(f"{self.site.url}/store/patch_14 redirects to {files.url}/hop/4/patch_14", log)
        self.assertIn(f"{self.site.url}/store/patch_14 redirects to {files.url}/hop/3/patch_14", log)
        self.assertIn(f"{self.site.url}/store/patch_14 redirects to {files.url}/hop/0/patch_14", log)
        # Six are refused, and the next address (five) is used.
        self.assertIn(f"address 1 of 2, {files.url}/hop/6/patch_20 (try 1 of 2): 127.0.0.1 redirected "
                      "/hop/6/patch_20 more than 5 times", log)
        self.assertIn(f"downloaded {files.url}/hop/5/patch_20 (address 2 of 2)", log)
        self.assertEqual(files.requests.count("/hop/6/patch_20"), 1, "not tried twice")

    def test_a_redirect_to_an_address_a_list_may_not_name_is_refused(self):
        patches = self.make_patches(14)
        for location, said in [
            ("ftp://127.0.0.1/patch_14", "ftp://127.0.0.1/patch_14"),
            ("http://user:pw@127.0.0.1/store/patch_14", "http://user:pw@127.0.0.1/store/patch_14"),
            ("file:///C:/Windows/win.ini", "file:///C:/Windows/win.ini"),
        ]:
            with self.subTest(location=location):
                self.site.patch_redirects[14] = location
                self.assertEqual(self.run_updater(), 1)
                self.assertFalse(self.exists(self.patch_name(14)))
                self.assert_blocked("127.0.0.1 redirected /store/patch_14 to an address the updater does not "
                                    f"take: {said}")
        self.site.patch_redirects.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    # --- the quick check -------------------------------------------------

    def test_a_file_as_it_was_hashed_is_not_read_again_until_check(self):
        patches = self.make_patches(14, 17, size=2_000_000)
        self.assertEqual(self.run_updater(), 0, self.log())
        record = self.record()
        self.assertEqual(sorted(record), ["resources.db.patch_14", "resources.db.patch_17"])
        self.assertEqual(record["resources.db.patch_14"][0], sha256(patches[14]))
        self.assertEqual(int(record["resources.db.patch_14"][1]), len(patches[14]))
        self.assertNotEqual(record["resources.db.patch_14"][4], "0", "a file index")
        # The launcher's run, and `ready`: neither reads a patch file.
        for run in (lambda: self.run_launched()[0], self.run_ready):
            mark = self.log_size()
            self.assertEqual(run(), 0, self.log())
            self.assertEqual(self.hashed_in(self.log_since(mark)), 0, self.log_since(mark))
            self.assertIn("2 file(s) as they were when hashed in the last week: not read again",
                          self.log_since(mark))
        # Proof that nothing was read: bytes changed in place with the size
        # and the time kept (as no program does by chance) go unseen ...
        path = self.path(self.patch_name(14))
        times = os.stat(path)
        with open(path, "r+b") as file:
            file.seek(1000)
            file.write(b"damaged")
        os.utime(path, ns=(times.st_atime_ns, times.st_mtime_ns))
        self.assertEqual(self.run_launched()[0], 0, self.log())
        self.assertEqual(self.run_ready(), 0)
        # ... until `check`, the repair, hashes every file.
        self.site.requests.clear()
        mark = self.log_size()
        self.assertEqual(self.run_updater(), 0, self.log())
        run = self.log_since(mark)
        self.assertIn("patches: every patch file is hashed (check)", run)
        self.assertEqual(self.hashed_in(run), 2, run)
        self.assertIn("resources.db.patch_14 does not have the list's checksum: downloaded again", run)
        self.assertEqual(self.store_requests(), ["/store/patch_14"])
        self.assert_patches(patches)

    def test_a_file_written_to_or_replaced_is_hashed_again(self):
        patches = self.make_patches(14, 17)
        self.assertEqual(self.run_updater(), 0, self.log())
        # Touched: another time.
        os.utime(self.path(self.patch_name(14)), (time.time() - 100, time.time() - 100))
        mark = self.log_size()
        self.assertEqual(self.run_launched()[0], 0, self.log())
        run = self.log_since(mark)
        self.assertEqual(self.hashed_in(run), 1, run)
        self.assertIn("1 file(s) as they were when hashed", run)
        self.assertIn("0 to download, 0 to delete", run)
        # Replaced by a copy of itself: another file ID, the same bytes.
        path = self.path(self.patch_name(17))
        shutil.copy2(path, path + ".copy")
        os.replace(path + ".copy", path)
        mark = self.log_size()
        self.assertEqual(self.run_launched()[0], 0, self.log())
        self.assertEqual(self.hashed_in(self.log_since(mark)), 1, self.log_since(mark))
        # And the next run reads neither.
        mark = self.log_size()
        self.assertEqual(self.run_launched()[0], 0, self.log())
        self.assertEqual(self.hashed_in(self.log_since(mark)), 0, self.log_since(mark))
        self.assert_patches(patches)

    def test_a_hash_older_than_a_week_is_done_again(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        day = 24 * 3600 * 10_000_000
        name = "resources.db.patch_14"
        for days, hashed in [(6, 0), (8, 1), (-1, 1)]:
            with self.subTest(days=days):
                text = self.read(self.RECORD).decode()
                words = text.split("\r\n")[1].split(" ")
                words[-1] = str(int(words[-1]) - days * day)
                self.write(self.RECORD, text.replace(text.split("\r\n")[1], " ".join(words)).encode())
                mark = self.log_size()
                self.assertEqual(self.run_launched()[0], 0, self.log())
                run = self.log_since(mark)
                self.assertEqual(self.hashed_in(run), hashed, run)
                if hashed:
                    # Hashed now: it stands for a week from now.
                    now = int(self.record()[name][-1])
                    if days > 0:
                        self.assertGreater(now, int(words[-1]) + day // 2)
                    else:
                        self.assertLess(now, int(words[-1]))
        self.assert_patches(patches)

    def test_a_record_lost_or_spoilt_hashes_every_file(self):
        self.make_patches(14, 17)
        self.assertEqual(self.run_updater(), 0, self.log())
        good = self.read(self.RECORD)
        for spoilt in (None, b"", b"skhid-patches-checked 1\r\n", good.replace(b"\r\nend", b"\r\nx\r\nend"),
                       good[:-20], good.replace(b"skhid-patches-checked 1", b"skhid-patches-checked 2")):
            with self.subTest(spoilt=spoilt):
                if spoilt is None:
                    os.remove(self.path(self.RECORD))
                else:
                    self.write(self.RECORD, spoilt)
                mark = self.log_size()
                self.assertEqual(self.run_launched()[0], 0, self.log())
                self.assertEqual(self.hashed_in(self.log_since(mark)), 2, self.log_since(mark))
                self.assertEqual(sorted(self.record()), ["resources.db.patch_14", "resources.db.patch_17"])

    def test_a_new_checksum_in_the_list_is_hashed_and_downloaded(self):
        patches = self.make_patches(14, 17)
        self.assertEqual(self.run_updater(), 0, self.log())
        # The list changed for patch 14 (as no release does: a fix is the
        # next revision; but the list decides).
        patches[14] = b"patch 14, other bytes"
        self.site.patches[14] = patches[14]
        self.site.requests.clear()
        mark = self.log_size()
        self.assertEqual(self.run_launched()[0], 0, self.log())
        run = self.log_since(mark)
        self.assertEqual(self.hashed_in(run), 1, run)
        self.assertEqual(self.store_requests(), ["/store/patch_14"])
        self.assert_patches(patches)
        self.assertEqual(self.record()["resources.db.patch_14"][0], sha256(patches[14]))

    def test_a_download_is_hashed_once_and_known_in_place_by_its_file_id(self):
        self.make_patches(14)
        mark = self.log_size()
        self.assertEqual(self.run_launched()[0], 0, self.log())
        run = self.log_since(mark)
        self.assertIn("patches: resources.db.patch_14: downloaded", run)
        self.assertEqual(self.hashed_in(run), 0, run)
        self.assertIn("patches: 1 patch(es) in place, checked", run)
        self.assertEqual(self.status(), "ready\r\n")
        mark = self.log_size()
        self.assertEqual(self.run_ready(), 0)
        self.assertEqual(self.hashed_in(self.log_since(mark)), 0, self.log_since(mark))

    def test_the_server_down_takes_the_quick_check_too(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.site.close()
        mark = self.log_size()
        self.assertEqual(self.run_launched()[0], 0, self.log())
        run = self.log_since(mark)
        self.assertIn("patches: the server could not be asked; the patches kept are in place", run)
        self.assertEqual(self.hashed_in(run), 0, run)
        # Damaged meanwhile, and `check`: found.
        self.write(self.patch_name(14), b"damaged")
        self.assertEqual(self.run_updater(), 1)
        self.site = harness.Site()
        self.site.patches.update(patches)

    # --- the form of the list before updater 0.3.0 -----------------------

    def test_a_list_of_the_earlier_form_is_still_read(self):
        patches = self.make_patches(14, 17)
        self.site.old_list = True
        self.assertIn(b'"url": ', self.site.true_patch_list())
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)
        kept = json.loads(self.read(f"{X86}/updater_skhid.patches"))
        self.assertEqual(kept["patch_14"], {"sha256": sha256(patches[14]),
                                            "urls": [f"{self.site.url}/store/patch_14"]})

    def test_a_list_kept_by_the_updater_before_is_read_offline_and_by_ready(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        # As 0.2.0 kept it, and no record of hashes (it made none).
        self.write(f"{X86}/updater_skhid.patches", (json.dumps(
            {"patch_14": {"sha256": sha256(patches[14]), "url": f"{self.site.url}/store/patch_14"}}) + "\n").encode())
        os.remove(self.path(self.RECORD))
        self.assertEqual(self.run_ready(), 0, self.log())
        self.site.close()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("patches: the server could not be asked; the patches kept are in place", self.log())
        self.site = harness.Site()

    @unittest.skipUnless(os.environ.get("SKHID_TEST_OLD_UPDATER"), "SKHID_TEST_OLD_UPDATER: the 0.2.0 exe")
    def test_the_updater_before_installs_this_one_which_does_the_patches_of_urls(self):
        """The release of 0.3.0: its ZIP and an srp that lists `urls`, at
        once. A player's 0.2.0 asks for the overlay first, installs the new
        updater and leaves the patch step to it (it never reads the new
        list); the new one does it at once (the launcher restarts) or at the
        next start."""
        patches = self.make_patches(12, 14)
        files = harness.Site()
        self.others = [files]
        files.patches.update(patches)
        self.site.patch_urls = {n: [f"{self.site.url}/hop/1/patch_{n}", f"{files.url}/store/patch_{n}"]
                                for n in patches}
        # The player's game: 0.2.0, checked against an earlier list of
        # patch 12 alone, in its form.
        with open(os.environ["SKHID_TEST_OLD_UPDATER"], "rb") as file:
            old = file.read()
        self.assertNotEqual(old, self.updater_bytes)
        self.write(f"{X86}/updater_skhid.exe", old)
        self.write(self.patch_name(12), patches[12])
        self.write(f"{X86}/updater_skhid.patches", (json.dumps(
            {"patch_12": {"sha256": sha256(patches[12]), "url": f"{self.site.url}/store/patch_12"}}) + "\n").encode())
        self.write(f"{X86}/updater_skhid.patches.status", b"ready\r\n")
        self.offer(self.bundle())
        self.assertEqual(self.run_updater(), 0, self.log())
        log = self.log()
        self.assertIn("---- updater 0.2.0 started", log)
        self.assertIn("the rest of the update is done by the new one", log)
        self.assertNotIn("/patches", self.site.requests, "0.2.0 never reads the new list")
        self.assertEqual(self.read(f"{X86}/updater_skhid.exe"), self.updater_bytes)
        self.assertEqual(self.status(), "blocked overlay changed\r\n")
        self.assertEqual(self.run_ready(), 1)
        # The new one, as the launcher starts it.
        mark = self.log_size()
        code, said = self.run_launched()
        self.assertEqual(code, 0, self.log())
        run = self.log_since(mark)
        self.assertIn("---- updater 0.3.0 started", run)
        self.assertIn("up to date", run)
        self.assertIn(f"downloaded {self.site.url}/hop/1/patch_14 (address 1 of 2)", run)
        self.assert_patches(patches)
        self.assertEqual(self.status(), "ready\r\n")
        self.assertEqual(json.loads(self.read(f"{X86}/updater_skhid.patches"))["patch_14"]["urls"],
                         self.site.patch_urls[14])
        self.assertEqual(self.run_ready(), 0)

    def test_a_file_cut_short_is_never_put_in_place(self):
        patches = self.make_patches(14)
        self.site.patch_cut[14] = 50_000
        self.assertEqual(self.run_updater(), 1)
        self.assertEqual(self.store_requests(), ["/store/patch_14"] * 2)
        self.assertFalse(self.exists(self.patch_name(14)))
        self.assert_blocked("game\\resources.db.patch_14 could not be downloaded.",
                            f"broke off (50000 of {len(patches[14])} bytes)")
        self.assertEqual(self.run_ready(), 1)
        self.site.patch_cut.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_damaged_file_the_server_spoils_too_is_not_left_in_place(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.write(self.patch_name(14), b"damaged")
        self.site.patch_bodies[14] = b"also wrong"
        self.assertEqual(self.run_updater(), 1)
        self.assertFalse(self.exists(self.patch_name(14)), "the damaged file is gone, the wrong one never came")
        self.assert_blocked()
        self.site.patch_bodies.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_list_that_cannot_be_read_changes_nothing_and_keeps_play_off(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.write("game/resources.db.patch_16", b"stray")
        good = sha256(patches[14])
        for body, said in [
            (b"<html>Sign in to the Wi-Fi</html>", "(not JSON at byte 0)"),
            (b'{"patch_14": "' + good.encode() + b'"}', "(bad entry patch_14)"),
            (json.dumps({"patch_014": {"sha256": good, "url": f"{self.site.url}/store/patch_14"}}).encode(),
             "(bad name patch_014)"),
            (json.dumps({"patch_256": {"sha256": good, "url": f"{self.site.url}/store/patch_14"}}).encode(),
             "(bad name patch_256)"),
            (json.dumps({"patch_14": {"sha256": good[:-1], "url": f"{self.site.url}/store/patch_14"}}).encode(),
             "(bad checksum of patch_14)"),
            (json.dumps({"patch_14": {"sha256": good, "url": "ftp://127.0.0.1/patch_14"}}).encode(),
             "(bad address of patch_14)"),
            (json.dumps({"patch_14": {"sha256": good, "url": "file:///C:/patch_14"}}).encode(),
             "(bad address of patch_14)"),
            (json.dumps({"patch_14": {"sha256": good, "url": "../patch_14"}}).encode(),
             "(bad address of patch_14)"),
            (('{"patch_14": {"sha256": "%s", "url": "%s/store/patch_14"}, "patch_14": {"sha256": "%s", '
              '"url": "%s/store/patch_14"}}' % (good, self.site.url, good, self.site.url)).encode(),
             "(patch_14 twice)"),
        ]:
            with self.subTest(said=said):
                self.site.patch_list = body
                self.site.requests.clear()
                self.assertEqual(self.run_updater(), 1)
                self.assertEqual(self.store_requests(), [])
                self.assertEqual(self.read("game/resources.db.patch_16"), b"stray", "nothing deleted")
                self.assertEqual(self.read(self.patch_name(14)), patches[14])
                # Nothing was changed: the last run's word stands.
                self.assert_blocked("The list of the game's patches from 127.0.0.1 cannot be used " + said,
                                    status="ready\r\n")
        self.site.patch_list = None
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_server_without_the_list_keeps_play_off(self):
        self.site.patch_list_status = 404
        self.assertEqual(self.run_updater(), 1)
        # The first start: the overlay was installed, nothing checked.
        self.assert_blocked("127.0.0.1 offers no list of the game's patches", status="blocked overlay changed\r\n")
        self.assertEqual(self.run_ready(), 1)

    def test_a_list_that_cannot_be_fetched_keeps_play_off_and_is_asked_for_next_time(self):
        patches = self.make_patches(14)
        self.site.patch_list_status = 500
        self.assertEqual(self.run_updater(), 1)
        self.assertEqual(self.site.requests.count("/patches"), 3)
        self.assert_blocked("The list of the game's patches could not be fetched. "
                            "127.0.0.1 answered /patches with error 500", status="blocked overlay changed\r\n")
        self.site.patch_list_status = 200
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_list_that_cannot_be_fetched_after_a_checked_run_is_as_a_server_that_is_down(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        # A moment's 500 of the server: the patches checked before may be
        # played, and the status is not touched.
        self.site.patch_list_status = 500
        self.assertEqual(self.run_updater(), 0, self.log())
        log = self.log()
        self.assertIn("patches: the list could not be fetched: 127.0.0.1 answered /patches with error 500", log)
        self.assertIn("patches: the server could not be asked; the patches kept are in place", log)
        self.assertEqual(self.status(), "ready\r\n")
        self.assertEqual(self.run_ready(), 0)
        # One damaged meanwhile: not, and the status still says what the
        # last checked run found.
        self.write(self.patch_name(14), b"damaged")
        self.assertEqual(self.run_updater(), 1)
        self.assert_blocked("The list of the game's patches could not be fetched.", status="ready\r\n")
        self.assertEqual(self.run_ready(), 1)
        self.site.patch_list_status = 200
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_new_skhid_files_are_not_played_with_the_patches_checked_for_the_old(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.status(), "ready\r\n")
        # A release replaces the updater: the new one is to do the rest (the
        # launcher starts anew for it). By hand, the old one never asks for
        # the list itself: it runs the new one at once, which does.
        new_updater = {f"{X86}/updater_skhid.exe".replace("/", "__"): self.updater_bytes + b"\0new build"}
        self.offer(self.bundle(**new_updater))
        self.site.requests.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("the rest of the update is done by the new one", self.log())
        self.assertIn("this updater was replaced: the new one finishes the update", self.log())
        self.assertEqual(self.site.requests, ["/api/client-files", "/download", "/api/client-files", "/patches"])
        self.assertEqual(self.status(), "ready\r\n")
        # A release whose list cannot be fetched: new files with the patches
        # checked for the old ones. `ready` refuses...
        self.offer(self.bundle(**{"README.txt": b"a new readme"}, **new_updater))
        self.site.patch_list_status = 500
        self.assertEqual(self.run_updater(), 1)
        self.assertEqual(self.read("README.txt"), b"a new readme")
        self.assert_blocked("The list of the game's patches could not be fetched.",
                            status="blocked overlay changed\r\n")
        self.assertEqual(self.run_ready(), 1)
        # ... and so does a start without the server.
        self.site.close()
        self.assertEqual(self.run_updater(), 1)
        self.assertIn("the last run did not finish: blocked overlay changed", self.log())
        self.site = harness.Site()
        self.site.patches.update(patches)
        self.site.patch_list_status = 500
        self.write(f"{X86}/updater_skhid.url", self.site.url.encode())
        self.offer(self.bundle(**{"README.txt": b"a new readme"}, **new_updater))
        # The list: checked, ready.
        self.site.patch_list_status = 200
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.status(), "ready\r\n")
        self.assertEqual(self.run_ready(), 0)
        self.assert_patches(patches)

    def test_a_file_server_that_cannot_be_reached_keeps_play_off(self):
        patches = self.make_patches(14)
        down = harness.Site()
        dead = down.url
        down.close()
        self.site.patch_base = dead
        self.assertEqual(self.run_updater(), 1)
        self.assertFalse(self.exists(self.patch_name(14)))
        self.assert_blocked("game\\resources.db.patch_14 could not be downloaded. Cannot connect to 127.0.0.1")
        self.site.patch_base = None
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_server_that_is_down_lets_play_only_with_the_patches_checked_before(self):
        patches = self.make_patches(14)
        # The first start, with the server down: never checked.
        archive = self.bundle()
        self.offer(archive)
        self.site.close()
        self.assertEqual(self.run_updater(), 1)
        self.assertIn("The game's patches could not be checked: Cannot connect to 127.0.0.1", self.log())
        self.assertEqual(self.run_ready(), 1)
        self.assertEqual(self.patch_files(), {})
        # Up again: installed and checked.
        self.site = harness.Site()
        self.site.patches.update(patches)
        self.write(f"{X86}/updater_skhid.url", self.site.url.encode())
        self.offer(archive)
        self.assertEqual(self.run_updater(), 0, self.log())
        # Down: the patches kept are in place, so the game may be played.
        self.site.close()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("patches: the server could not be asked; the patches kept are in place", self.log())
        # Down, and one damaged meanwhile: not.
        self.write(self.patch_name(14), b"damaged")
        self.assertEqual(self.run_updater(), 1)
        self.assertIn("resources.db.patch_14 does not have the list's checksum", self.log())
        self.site = harness.Site()

    def test_not_enough_disk_downloads_nothing(self):
        self.make_patches(14)
        free = shutil.disk_usage(self.path("game")).free
        self.site.patch_lengths[14] = free + (1 << 30)
        if self.site.patch_lengths[14] >= 1 << 40:
            self.skipTest("more than 1 TB free: larger than the largest patch taken")
        self.assertEqual(self.run_updater(), 1)
        self.assertFalse(self.exists(self.patch_name(14)))
        self.assertEqual(self.store_requests(), ["/store/patch_14"], "tried once: the disk does not change")
        needed = ((free + (1 << 30) + (64 << 20)) + (1 << 20) - 1) >> 20
        self.assert_blocked("Not enough free disk space to update the game's files in",
                            f": {needed} MB needed,", "bytes free in")

    def test_the_overlay_may_not_bring_a_patch_file(self):
        for name in ("game/resources.db.patch_14", f"{X86}/resources.db.patch_3"):
            with self.subTest(name=name):
                self.offer(self.bundle(**{name.replace("/", "__"): b"from the zip"}))
                self.assertEqual(self.run_updater(), 1)
                self.assert_blocked(f"the overlay has {name.replace(os.sep, '/')}")
                self.assertEqual(self.run_ready(), 1)

    # --- the launcher says stop ------------------------------------------

    def test_a_download_stopped_half_way_leaves_no_file_and_no_ready(self):
        patches = self.make_patches(14, size=4_000_000)
        self.site.patch_slow = 0.05
        code, said = self.run_launched(stop_when=lambda: "/store/patch_14" in self.site.requests)
        self.assertEqual(code, 0, self.log())
        states = [frame[1] for frame in said if frame[0] == 0x02]
        self.assertEqual(states[-1], ABORTED)
        self.assertIn(DOWNLOADING, states)
        self.assertNotIn(LATEST, states)
        self.assertEqual(self.patch_files(), {})
        self.assertIn("patches: stopped; the rest next time", self.log())
        self.assertEqual(self.status(), "blocked stopped\r\n")
        self.assertEqual(self.run_ready(), 1)
        self.site.patch_slow = 0
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_stop_while_the_list_is_asked_again_keeps_the_last_word(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.site.patch_list_status = 503
        self.site.requests.clear()
        code, said = self.run_launched(stop_when=lambda: "/patches" in self.site.requests)
        self.assertEqual(code, 0, self.log())
        self.assertEqual(self.site.requests.count("/patches"), 1, "stopped in the pause before the next try")
        self.assertEqual([frame[1] for frame in said if frame[0] == 0x02][-1], ABORTED)
        self.assertEqual(self.status(), "ready\r\n")
        self.assertEqual(self.run_ready(), 0)
        self.assert_patches(patches)

    # --- other programs hold files ---------------------------------------

    def test_a_running_game_keeps_the_patches_as_they_are(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.write("game/resources.db.patch_16", b"stray")
        game = Holder(self, "game/resources.db")
        self.assertEqual(self.run_updater(), 1)
        self.assertTrue(self.exists("game/resources.db.patch_16"), "nothing deleted while the game runs")
        self.assert_blocked("The game is running, so Skhid could not update the game's patches",
                            "resources.db is open in another program")
        # With nothing to change, a game that runs is no reason to stop.
        os.remove(self.path("game/resources.db.patch_16"))
        self.assertEqual(self.run_updater(), 0, self.log())
        self.write("game/resources.db.patch_16", b"stray")
        game.release()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    def test_a_patch_file_another_program_holds_a_moment_is_tried_again(self):
        # An antivirus that scans the new file before it is renamed into
        # place: the rename waits for it.
        patches = self.make_patches(14, size=32 << 20)
        scanner = Holder(self, "game/resources.db.patch_14.skhid-part", wait=False)
        updater = self.start_updater()
        until = time.time() + 120
        while updater.poll() is None and time.time() < until:
            if self.exists(f"{X86}/updater_skhid.log") and "cannot be renamed now" in self.log():
                scanner.release()
            time.sleep(0.01)
        self.assertEqual(updater.wait(120), 0, self.log())
        self.assertTrue(scanner.holding(), "the stand-in never got hold of the download")
        log = self.log()
        self.assertIn("resources.db.patch_14.skhid-part cannot be renamed now", log)
        self.assertIn("try 1 of 6: another program holds it", log)
        self.assert_patches(patches)
        self.assertEqual(self.status(), "ready\r\n")

    def test_a_patch_file_another_program_keeps_holding_keeps_play_off(self):
        patches = self.make_patches(14)
        self.assertEqual(self.run_updater(), 0, self.log())
        self.write("game/resources.db.patch_16", b"stray")
        scanner = Holder(self, "game/resources.db.patch_16")
        self.assertEqual(self.run_updater(), 1)
        self.assertIn("try 5 of 6", self.log())
        self.assert_blocked("Another program holds game\\resources.db.patch_16")
        self.assertNotIn("The game is running", self.log())
        scanner.release()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_patches(patches)

    # --- what play_skhid.bat checks before it starts the game: `ready` ----

    def test_play_skhid_bat_starts_the_game_only_when_the_patches_are_checked(self):
        patches = self.make_patches(14)
        code, started, said = self.run_bat()
        self.assertEqual((code, started), (1, False), said)
        self.assertIn("Skhid has not finished updating the game's files", said)
        self.assertIn("ready: NOT READY: patches:", self.log())
        self.site.patch_bodies[14] = b"wrong"
        self.assertEqual(self.run_updater(), 1)
        code, started, said = self.run_bat()
        self.assertEqual((code, started), (1, False), said)
        self.site.patch_bodies.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        code, started, said = self.run_bat()
        self.assertEqual((code, started), (0, True), said)
        # An extra patch file after (copied by hand): not started.
        self.write("game/resources.db.patch_200", b"from somewhere")
        code, started, said = self.run_bat()
        self.assertEqual((code, started), (1, False), said)
        self.assertTrue(self.exists("game/resources.db.patch_200"), "ready only reads")
        self.assert_patches({**patches, 200: b"from somewhere"})


class RestoreTest(WineTest):
    """The one-time restore: a resources.db whose regions an earlier updater
    rewrote in place, with the record and the copies it kept, as the
    players' folders are."""

    DB_SIZE = 4 << 20
    REGIONS = {"main_menu": (900_000, 100_000), "lobby_menu": (1_000_000, 1_500_000),
               "login_menu": (3_000_000, 300_000)}
    RECORD = f"{X86}/updater_skhid.resources.state"
    BACKUP = f"{X86}/updater_skhid.backup"

    def make_db(self, kept=True):
        """The db as shipped, then as the earlier updater left it."""
        self.db = random.Random(2777).randbytes(self.DB_SIZE)
        changed = bytearray(self.db)
        lines = []
        for name, (offset, size) in self.REGIONS.items():
            original = self.db[offset:offset + size]
            target = random.Random(name).randbytes(size)
            changed[offset:offset + size] = target
            lines.append(f"region game/resources.db {self.DB_SIZE} {offset} {size} "
                         f"{sha256(original)} {sha256(target)} {name}")
            self.site.blobs[sha256(original)] = original
            if kept:
                self.write(f"{self.BACKUP}/{sha256(original)}", original)
        self.write("game/resources.db", bytes(changed))
        self.record = "".join(line + "\r\n" for line in ["skhid-resources-state 1", *lines, "end"]).encode()
        self.write(self.RECORD, self.record)
        # The earlier updaters' plan (the ZIP's) and the status of a plan
        # that grew the db.
        self.write(f"{X86}/updater_skhid.resources", b"skhid-resources 1\r\nend\r\n")
        self.write(f"{X86}/updater_skhid.resources.status", b"ready\r\n")
        return bytes(changed)

    def blob_requests(self):
        return [path for path in self.site.requests if path.startswith("/resources/")]

    def assert_stock(self):
        now = self.read("game/resources.db")
        self.assertEqual((len(now), sha256(now)), (len(self.db), sha256(self.db)))

    def assert_nothing_left(self):
        for name in (self.RECORD, self.BACKUP, f"{X86}/updater_skhid.cache", f"{X86}/updater_skhid.resources",
                     f"{X86}/updater_skhid.resources.status", f"{X86}/updater_skhid.resources.journal"):
            self.assertFalse(self.exists(name), name)

    def test_every_region_gets_its_stock_bytes_back_once(self):
        self.make_db()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_stock()
        self.assert_nothing_left()
        self.assertEqual(self.blob_requests(), [])
        log = self.log()
        self.assertIn("resources: 3 region(s) of resources.db changed by an earlier updater", log)
        for name in self.REGIONS:
            self.assertIn(f"resources: {name}: its original is back (from the copy kept)", log)
        # And never again.
        mark = self.log_size()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertNotIn("resources:", self.log_since(mark))
        self.assert_stock()

    def test_without_the_copies_the_stock_bytes_come_from_the_server(self):
        self.make_db(kept=False)
        lobby = sha256(self.db[1_000_000:2_500_000])
        self.site.blob_bodies[lobby] = b"x" * 1_500_000
        self.assertEqual(self.run_updater(), 0, self.log())
        # The two the server has right are back; the lobby stays recorded.
        self.assertIn("the server's blob is not 1500000 bytes with that SHA-256", self.log())
        self.assertIn("resources: 1 region(s) not restored yet (lobby_menu): next time", self.log())
        self.assertIn("lobby_menu", self.read(self.RECORD).decode())
        self.assertNotEqual(sha256(self.read("game/resources.db")), sha256(self.db))
        self.site.blob_bodies.clear()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("resources: lobby_menu: its original is back (from the server)", self.log())
        self.assert_stock()
        self.assert_nothing_left()

    def test_a_running_game_holds_the_db_and_the_restore_waits(self):
        changed = self.make_db()
        game = Holder(self, "game/resources.db")
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(sha256(self.read("game/resources.db")), sha256(changed))
        self.assertEqual(self.read(self.RECORD), self.record)
        self.assertIn("resources: the game is running", self.log())
        game.release()
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assert_stock()
        self.assert_nothing_left()

    def test_a_write_that_was_cut_short_is_repaired(self):
        self.make_db()
        # The PC went off half way through putting the lobby back: half
        # stock, half not, the journal of it, and the record without it.
        offset, size = self.REGIONS["lobby_menu"]
        lines = self.record.decode().split("\r\n")
        lobby = next(line for line in lines if line.endswith(" lobby_menu"))
        self.write(self.RECORD, "\r\n".join(line for line in lines if line != lobby).encode())
        self.write(f"{X86}/updater_skhid.resources.journal",
                   f"skhid-resources-state 1\r\n{lobby}\r\nend\r\n".encode())
        with open(self.path("game/resources.db"), "r+b") as file:
            file.seek(offset)
            file.write(self.db[offset:offset + size // 2])
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertIn("a write of lobby_menu was cut short", self.log())
        self.assert_stock()
        self.assert_nothing_left()

    def test_the_copies_kept_are_enough_without_the_server(self):
        self.make_db()
        self.offer(self.bundle())
        self.site.close()
        # The patches were never checked, so PLAY stays off; the db is
        # given back all the same.
        self.assertEqual(self.run_updater(), 1)
        self.assert_stock()
        self.assert_nothing_left()
        self.site = harness.Site()

    def test_a_db_of_another_build_is_left_alone(self):
        changed = self.make_db()
        with open(self.path("game/resources.db"), "ab") as file:
            file.write(b"!")
        self.assertEqual(self.run_updater(), 0, self.log())
        self.assertEqual(self.read("game/resources.db"), changed + b"!")
        self.assertFalse(self.exists(self.RECORD))
        self.assertIn("lobby_menu holds its stock bytes (or the db is gone or of another build): forgotten",
                      self.log())


if __name__ == "__main__":
    unittest.main()
