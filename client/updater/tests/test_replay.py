"""End-to-end tests of `updater_skhid.exe replay <id> <token> <sha256> <size>`
("Watch replay") on Windows: the real program in a game folder made for each
test (its path has a space in it), the web server of test_updater.py on
127.0.0.1 in the place of the srp one, and tests/fake_exe in the place of
the game (it writes down how it was started and ends; it makes no window).

    cd client/updater
    ./build.sh
    python tests/test_replay.py [-v] [TestName ...]

NO WINDOW: the updater is started only with the trailing word `quiet`, in
which it never calls MessageBox (exe/src/replay.rs: `notify` is the one
place that shows a box, and only when not quiet); `run_updater` refuses any
other command line, and `test_quiet_is_the_only_way_to_a_box` reads the
source to keep it so. Should a window of the updater appear all the same,
the process is killed at once and the test fails. The problem a run met is
read from its log line (`replay: FAILED (<problem>): <text>`); the text is in
the language the launcher keeps for this user, read here and never written.
Nothing outside the temporary folder is touched; SKHID_TEST_TEMP names the
folder the temporary folders go in. The game and the launcher are never
started: the game exe of every test folder is the stand-in.
"""

import ctypes
import ctypes.wintypes as wintypes
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import winreg

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from test_updater import FAKE_EXE, UPDATER, X86, Site  # noqa: E402

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
ENUM = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
user32.EnumWindows.argtypes = [ENUM, wintypes.LPARAM]
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.IsWindowVisible.argtypes = [wintypes.HWND]
kernel32.CreateFileW.restype = wintypes.HANDLE
kernel32.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p,
                                 wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]

SOURCE = os.path.join(HERE, "..", "exe", "src")
ID = "0a1b2c3d"
TOKEN = "0123456789abcdef" * 2
# A word of each message in each launcher language (logic/src/replay.rs).
WORDS = {
    "BadLink": ["not valid", "недействительна", "недійсне", "ungültig", "nieprawidłowy"],
    "AlreadyRunning": ["already being downloaded", "уже загружается", "уже завантажується",
                       "bereits heruntergeladen", "już pobierana"],
    "Network": ["Cannot reach", "Не удаётся соединиться", "Не вдається з'єднатися", "nicht erreichbar",
                "Nie można połączyć"],
    "Expired": ["expired", "устарела", "застаріло", "abgelaufen", "wygasł"],
    "Busy": ["is busy", "занят", "зайнятий", "ausgelastet", "zajęty"],
    "Status": ["error {}", "ошибкой {}", "помилкою {}", "Fehler {}", "błędem {}"],
    "Damaged": ["damaged", "повреждён", "пошкоджений", "beschädigt", "uszkodzona"],
    "NoGame": ["survarium_skhid.exe"] * 5,
    "Path": ["Latin letters", "латинских букв", "латинські літери", "lateinische Buchstaben", "litery łacińskie"],
    "NotReady": ["has not finished", "не закончил", "не завершив", "noch nicht fertig", "nie zakończył"],
}


def launcher_language():
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Vostok Games\Survarium") as key:
            value, kind = winreg.QueryValueEx(key, "language")
            return value if kind == winreg.REG_DWORD and 0 <= value < 5 else 0
    except OSError:
        return 0


LANGUAGE = launcher_language()


def has_window(pid):
    """Whether the process has a visible top-level window."""
    found = []

    def each(hwnd, _):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True

    user32.EnumWindows(ENUM(each), 0)
    return bool(found)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class QuietSource(unittest.TestCase):
    """No process is started here."""

    def test_quiet_is_the_only_way_to_a_box(self):
        with open(os.path.join(SOURCE, "replay.rs"), encoding="utf-8") as file:
            replay = file.read()
        # The one call of the box in the replay's code, behind `if !quiet`.
        self.assertEqual(len(re.findall(r"message_box\(", replay)), 1)
        self.assertRegex(replay, r"fn notify\(quiet: bool, text: &str\) \{\s*if !quiet \{\s*crate::message_box\(text\);")
        self.assertNotRegex(replay, r"MessageBox|CreateWindow|DialogBox")
        # The box is MessageBoxW of main.rs alone, and the replay branch returns
        # before any of main.rs's own calls of it.
        windows = []
        for name in os.listdir(SOURCE):
            with open(os.path.join(SOURCE, name), encoding="utf-8") as file:
                text = file.read()
            if re.search(r"MessageBox\w*\(|CreateWindow|DialogBox", text):
                windows.append(name)
        self.assertEqual(windows, ["main.rs"])
        with open(os.path.join(SOURCE, "main.rs"), encoding="utf-8") as file:
            main = file.read()
        run = main[main.index("fn run() -> u32 {"):]
        branch = run.index("return replay::run(replay_arguments, quiet);")
        self.assertLess(branch, run.index("message_box("))
        self.assertIn("updater_logic::replay::command(&arguments)", run[:branch])

    def test_the_game_starts_only_after_the_ready_check(self):
        with open(os.path.join(SOURCE, "replay.rs"), encoding="utf-8") as file:
            replay = file.read()
        body = replay[replay.index("pub fn run("):replay.index("fn fetch(")]
        # One start of the game, after the check `ready` makes; its refusal
        # returns before it.
        self.assertEqual(body.count("win::start("), 1)
        check = body.index("crate::patches::ready(&layout).is_err()")
        self.assertLess(check, body.index("win::start("))
        self.assertIn("return tell(quiet, language, &Problem::NotReady);", body[check:body.index("win::start(")])


class ReplayTest(unittest.TestCase):
    def setUp(self):
        for needed in (UPDATER, FAKE_EXE):
            if not os.path.isfile(needed):
                self.fail(f"{needed} is not built: run client/updater/build.sh")
        self.root = tempfile.mkdtemp(prefix="skhid replay test ", dir=os.environ.get("SKHID_TEST_TEMP"))
        self.make_game(os.path.join(self.root, "My Games", "Survarium"))
        self.site = Site()
        self.write_url(self.site.url)

    def make_game(self, top):
        self.top = top
        self.x86 = os.path.join(top, X86)
        self.game = os.path.join(top, "game")
        os.makedirs(self.x86, exist_ok=True)
        shutil.copy(UPDATER, os.path.join(self.x86, "updater_skhid.exe"))
        shutil.copy(FAKE_EXE, os.path.join(self.x86, "survarium_skhid.exe"))
        # The last patch step ended `ready` with a list of no patch: the
        # folder of a player whose launcher has run, as `ready` reads it.
        self.write_patches({}, b"ready\r\n")

    def write_url(self, url):
        with open(os.path.join(self.x86, "updater_skhid.url"), "w", encoding="ascii") as file:
            file.write(url)

    def tearDown(self):
        self.site.close()
        shutil.rmtree(self.root, ignore_errors=True)

    def offer(self, data, token=TOKEN):
        self.site.replays[token] = data
        return ["replay", ID, token, sha256(data), str(len(data))]

    def run_updater(self, arguments, seconds=60):
        """(exit code, the problem line of its log or None). Only `replay ...
        quiet`, and only with the stand-in as the game."""
        arguments = [*arguments, "quiet"]
        if arguments[0] != "replay" or arguments[-1] != "quiet":
            raise AssertionError(f"refused to start the updater without quiet: {arguments}")
        game = os.path.join(self.x86, "survarium_skhid.exe")
        if os.path.exists(game):
            with open(game, "rb") as file, open(FAKE_EXE, "rb") as fake:
                if file.read() != fake.read():
                    raise AssertionError("refused: the game exe of the test folder is not the stand-in")
        log = os.path.join(self.x86, "updater_skhid.log")
        if os.path.exists(log):
            os.remove(log)
        process = subprocess.Popen([os.path.join(self.x86, "updater_skhid.exe"), *arguments], cwd=self.x86)
        try:
            until = time.time() + seconds
            while time.time() < until and process.poll() is None:
                if has_window(process.pid):
                    process.kill()
                    raise AssertionError("the updater made a window in quiet mode")
                time.sleep(0.02)
            code = process.wait(20)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(10)
        failed = [line for line in self.log().splitlines() if "replay: FAILED" in line]
        self.assertLessEqual(len(failed), 1, failed)
        self.assertIn("(quiet)", self.log())
        return code, (failed[0] if failed else None)

    def started(self, seconds=5.0):
        """The game's command line and working folder, if it was started."""
        path = os.path.join(self.x86, "survarium_skhid.exe.started.txt")
        for _ in range(int(seconds / 0.05)):
            if os.path.exists(path):
                with open(path, "rb") as file:
                    lines = file.read().decode("utf-16").splitlines()
                if len(lines) >= 2:
                    return lines[0], lines[1]
            time.sleep(0.05)
        return None

    def replays(self):
        folder = os.path.join(self.game, "replays")
        return sorted(os.listdir(folder)) if os.path.isdir(folder) else []

    def log(self):
        with open(os.path.join(self.x86, "updater_skhid.log"), "rb") as file:
            return file.read().decode("utf-8", "replace")

    def assertSaid(self, line, problem, *values):
        self.assertIsNotNone(line, f"no problem logged for {problem}")
        self.assertIn(f"FAILED ({problem}", line)
        self.assertIn(WORDS[problem][LANGUAGE].format(*values), line)

    def assertStartedWith(self, replay_path):
        started = self.started()
        self.assertIsNotNone(started, "the game was not started")
        command, folder = started
        game = os.path.join(self.x86, "survarium_skhid.exe")
        # No -localization: the game keeps its own language (game patch 79).
        self.assertEqual(command, f'"{game}" -mt_allocator=768 '
                                  f'-minimum_resources_memory_size=768 -match_replay="{replay_path}"')
        self.assertEqual(os.path.normcase(folder.rstrip("\\")), os.path.normcase(self.x86))

    def assertNothingDone(self, requests=0):
        self.assertIsNone(self.started(0.5), "the game was started")
        self.assertEqual([name for name in self.replays() if name.startswith("srp_")], [])
        self.assertEqual(len(self.site.requests), requests, self.site.requests)

    # --- the normal runs -------------------------------------------------

    def test_a_replay_is_downloaded_checked_and_played(self):
        data = os.urandom(300_000)
        self.assertEqual(self.run_updater(self.offer(data)), (0, None))
        target = os.path.join(self.game, "replays", f"srp_{ID}.sur")
        with open(target, "rb") as file:
            self.assertEqual(file.read(), data)
        self.assertEqual(self.replays(), [f"srp_{ID}.sur"])
        self.assertEqual(self.site.requests, [f"/replays/{TOKEN}"])
        # The path has spaces: the game reads a value in quotes as one.
        self.assertIn(" ", target)
        self.assertStartedWith(target)
        log = self.log()
        self.assertIn(f"token {TOKEN[:6]}...", log)
        self.assertNotIn(TOKEN[:7], log)

    def test_a_replay_that_is_there_is_used_again_and_a_wrong_one_is_replaced(self):
        data = os.urandom(5000)
        arguments = self.offer(data)
        os.makedirs(os.path.join(self.game, "replays"))
        target = os.path.join(self.game, "replays", f"srp_{ID}.sur")
        with open(target, "wb") as file:
            file.write(data)
        self.assertEqual(self.run_updater(arguments), (0, None))
        self.assertEqual(self.site.requests, [])
        self.assertStartedWith(target)

        os.remove(os.path.join(self.x86, "survarium_skhid.exe.started.txt"))
        with open(target, "wb") as file:
            file.write(b"another replay")
        self.assertEqual(self.run_updater(arguments), (0, None))
        self.assertEqual(self.site.requests, [f"/replays/{TOKEN}"])
        with open(target, "rb") as file:
            self.assertEqual(file.read(), data)
        self.assertStartedWith(target)

    def test_the_ten_newest_replays_stay_and_nothing_else_is_touched(self):
        folder = os.path.join(self.game, "replays")
        os.makedirs(folder)
        old = [f"srp_{n:08x}.sur" for n in range(12)]
        others = ["notes.txt", "srp_zz.sur", "my_match.sur", "srp_0000000a.sur.bak", "SRP_000000FF.SUR"]
        for index, name in enumerate(old + others + ["srp_000000ee.sur.part"]):
            path = os.path.join(folder, name)
            with open(path, "wb") as file:
                file.write(b"x")
            os.utime(path, (1_700_000_000 + index, 1_700_000_000 + index))
        self.assertEqual(self.run_updater(self.offer(b"a replay")), (0, None))
        # The new one and the nine newest of the old; the player's files stay.
        self.assertEqual(sorted(self.replays()), sorted([f"srp_{ID}.sur"] + old[3:] + others))

    def test_a_replay_download_does_not_wait_for_an_update_and_one_at_a_time(self):
        arguments = self.offer(b"a replay")
        update_lock = kernel32.CreateFileW(os.path.join(self.x86, "updater_skhid.lock"), 0x40000000, 0, None, 4,
                                           0x80, None)
        self.assertNotEqual(update_lock, wintypes.HANDLE(-1).value)
        try:
            self.assertEqual(self.run_updater(arguments), (0, None))
        finally:
            kernel32.CloseHandle(update_lock)
        self.assertStartedWith(os.path.join(self.game, "replays", f"srp_{ID}.sur"))
        os.remove(os.path.join(self.x86, "survarium_skhid.exe.started.txt"))
        os.remove(os.path.join(self.game, "replays", f"srp_{ID}.sur"))
        self.site.requests.clear()
        replay_lock = kernel32.CreateFileW(os.path.join(self.x86, "updater_skhid.replay.lock"), 0x40000000, 0,
                                           None, 4, 0x80, None)
        try:
            code, line = self.run_updater(arguments)
        finally:
            kernel32.CloseHandle(replay_lock)
        self.assertEqual(code, 1)
        self.assertSaid(line, "AlreadyRunning")
        self.assertNothingDone()

    # --- what must start nothing ------------------------------------------

    def test_hostile_arguments_start_nothing_and_ask_nothing(self):
        sha = sha256(b"x")
        good = [ID, TOKEN, sha, "1"]
        cases = [
            [], good[:3], good + ["extra"], ["", "", "", ""],
            ["0A1B2C3D", TOKEN, sha, "1"], [ID[:-1], TOKEN, sha, "1"], [ID + "0", TOKEN, sha, "1"],
            [ID, TOKEN.upper(), sha, "1"], [ID, TOKEN[:-1], sha, "1"], [ID, TOKEN + "0", sha, "1"],
            [ID, TOKEN, sha.upper(), "1"], [ID, TOKEN, sha[:-1], "1"], [ID, TOKEN, sha + "0", "1"],
            [ID, TOKEN, sha, "0"], [ID, TOKEN, sha, str(64 * 2**20 + 1)], [ID, TOKEN, sha, "12345678901"],
            [ID, TOKEN, sha, "-1"], [ID, TOKEN, sha, "1 "], [ID, TOKEN, sha, "1e3"],
            [ID, '0123456789abcdef0123456789abcd"x', sha, "1"], [ID, TOKEN[:-1] + "&", sha, "1"],
            [ID, TOKEN[:-1] + "|", sha, "1"], ["..\\..\\..", TOKEN, sha, "1"],
            [ID, "../../../../etc/passwd" + "0" * 10, sha, "1"],
            [ID, "https://example.com/" + "0" * 12, sha, "1"], [ID + " " + TOKEN, sha, "1", "1"],
            [ID, TOKEN, sha, "1", "&", "calc.exe"], [ID, TOKEN + "?x=1", sha, "1"],
            ["quiet", *good],
        ]
        self.site.replays[TOKEN] = b"x"
        for arguments in cases:
            with self.subTest(arguments=arguments):
                code, line = self.run_updater(["replay", *arguments])
                self.assertEqual(code, 1)
                self.assertSaid(line, "BadLink")
                self.assertNothingDone()
                self.assertNotIn(TOKEN, self.log())

    def test_a_server_that_answers_wrong_starts_nothing(self):
        data = os.urandom(10_000)
        arguments = ["replay", ID, TOKEN, sha256(data), str(len(data))]
        cases = [
            ("longer", (200, {}, data + b"more"), "Damaged", ()),
            ("shorter", (200, {}, data[:-1]), "Damaged", ()),
            ("other bytes", (200, {}, os.urandom(len(data))), "Damaged", ()),
            ("a redirect", (302, {"Location": f"/replays/{'f' * 32}"}, b""), "Status", (302,)),
            ("moved for good", (301, {"Location": "https://example.com/x.sur"}, b""), "Status", (301,)),
            ("unknown token", (404, {}, b""), "Expired", ()),
            ("gone", (410, {}, b""), "Expired", ()),
            ("busy", (503, {}, b""), "Busy", ()),
            ("too many", (429, {}, b""), "Busy", ()),
            ("broken", (500, {}, b""), "Status", (500,)),
        ]
        self.site.replays["f" * 32] = data                    # what a redirect would reach
        for label, answer, problem, values in cases:
            with self.subTest(label):
                self.site.requests.clear()
                self.site.replay_answers[TOKEN] = answer
                code, line = self.run_updater(arguments)
                self.assertEqual(code, 1)
                self.assertSaid(line, problem, *values)
                self.assertNothingDone(requests=1)
                self.assertEqual(self.site.requests, [f"/replays/{TOKEN}"])
                self.assertEqual(self.replays(), [], "a download left over")

    def test_an_unreachable_site_is_tried_three_times(self):
        self.write_url("http://127.0.0.1:9")                # nothing listens there
        code, line = self.run_updater(self.offer(b"a replay"), seconds=120)
        self.assertEqual(code, 1)
        self.assertSaid(line, "Network")
        self.assertNothingDone()
        self.assertEqual(self.log().count("replay: download "), 2)

    def test_only_its_own_site_by_its_own_rules(self):
        # Plain HTTP to another host than this computer is refused, as for updates.
        self.write_url("http://example.com")
        code, line = self.run_updater(self.offer(b"a replay"))
        self.assertEqual(code, 1)
        self.assertIn("FAILED (server address)", line)
        self.assertIn("https://host", line)
        self.assertNothingDone()

    # --- the game's files must be in place, as for play_skhid.bat ---------

    def write_patches(self, patches, status):
        """Beside the updater, the list of the last patch step (`patches`:
        n -> the file's bytes) and its status, as `ready` reads them; the
        game folder's patch files are not touched."""
        listed = {f"patch_{n}": {"sha256": sha256(data), "url": f"http://127.0.0.1:9/store/patch_{n}"}
                  for n, data in patches.items()}
        with open(os.path.join(self.x86, "updater_skhid.patches"), "wb") as file:
            file.write(json.dumps(listed).encode("ascii"))
        with open(os.path.join(self.x86, "updater_skhid.patches.status"), "wb") as file:
            file.write(status)

    def write_patch_file(self, n, data):
        os.makedirs(self.game, exist_ok=True)
        with open(os.path.join(self.game, f"resources.db.patch_{n}"), "wb") as file:
            file.write(data)

    def test_patches_not_in_place_start_no_game_and_keep_the_replay(self):
        patch = os.urandom(8192)
        self.write_patches({14: patch}, b"ready\r\n")
        data = os.urandom(20_000)
        arguments = self.offer(data)
        # Listed, missing.
        code, line = self.run_updater(arguments)
        self.assertEqual(code, 1)
        self.assertSaid(line, "NotReady")
        log = self.log()
        self.assertIn("ready: NOT READY:", log)
        self.assertIn("the game is not started", log)
        self.assertIsNone(self.started(0.5), "the game was started")
        # Downloaded and checked all the same: the next ask uses it again.
        target = os.path.join(self.game, "replays", f"srp_{ID}.sur")
        with open(target, "rb") as file:
            self.assertEqual(file.read(), data)
        self.assertEqual(self.site.requests, [f"/replays/{TOKEN}"])
        # Read only: no patch file was fetched or made.
        self.assertFalse(os.path.exists(os.path.join(self.game, "resources.db.patch_14")))

        # Damaged (one byte): not in place either.
        self.write_patch_file(14, patch[:-1] + bytes([patch[-1] ^ 1]))
        code, line = self.run_updater(arguments)
        self.assertEqual(code, 1)
        self.assertSaid(line, "NotReady")
        self.assertIsNone(self.started(0.5), "the game was started")

        # In place, and one patch file more than the list has: not in place.
        self.write_patch_file(14, patch)
        self.write_patch_file(15, b"another")
        code, line = self.run_updater(arguments)
        self.assertEqual(code, 1)
        self.assertSaid(line, "NotReady")
        self.assertIsNone(self.started(0.5), "the game was started")

        # Once the launcher has made the folder its list, the replay kept is played.
        os.remove(os.path.join(self.game, "resources.db.patch_15"))
        self.site.requests.clear()
        self.assertEqual(self.run_updater(arguments), (0, None))
        self.assertIn("ready: the patches are in place", self.log())
        self.assertEqual(self.site.requests, [])
        self.assertStartedWith(target)

    def test_a_patch_step_that_did_not_end_ready_starts_no_game(self):
        arguments = self.offer(b"a replay")
        for status in (b"blocked overlay changed\r\n", b"blocked a download failed\r\n", b""):
            self.write_patches({}, status)
            code, line = self.run_updater(arguments)
            self.assertEqual(code, 1, status)
            self.assertSaid(line, "NotReady")
            self.assertIsNone(self.started(0.5), "the game was started")
        # No status at all (a launcher that never ran the patch step): no game.
        os.remove(os.path.join(self.x86, "updater_skhid.patches.status"))
        code, line = self.run_updater(arguments)
        self.assertEqual(code, 1)
        self.assertSaid(line, "NotReady")
        # A list that cannot be read: no game.
        self.write_patches({}, b"ready\r\n")
        with open(os.path.join(self.x86, "updater_skhid.patches"), "wb") as file:
            file.write(b"{not a list")
        code, line = self.run_updater(arguments)
        self.assertEqual(code, 1)
        self.assertSaid(line, "NotReady")
        self.assertIsNone(self.started(0.5), "the game was started")
        # Ready again: the game starts.
        self.write_patches({}, b"ready\r\n")
        self.assertEqual(self.run_updater(arguments), (0, None))
        self.assertStartedWith(os.path.join(self.game, "replays", f"srp_{ID}.sur"))

    def test_no_game_exe_starts_nothing(self):
        os.remove(os.path.join(self.x86, "survarium_skhid.exe"))
        code, line = self.run_updater(self.offer(b"a replay"))
        self.assertEqual(code, 1)
        self.assertSaid(line, "NoGame")
        self.assertNothingDone()

    def test_a_folder_in_another_alphabet(self):
        # Letters the ANSI code page of this PC may not have: the game gets the
        # path as it is when it has them, else the short (8.3) path, else the
        # player is told.
        shutil.rmtree(self.top)
        self.make_game(os.path.join(self.root, "Ελληνικά 遊戲", "Survarium"))
        self.write_url(self.site.url)
        code, line = self.run_updater(self.offer(b"a replay"))
        target = os.path.join(self.game, "replays", f"srp_{ID}.sur")
        if code == 0:
            command, _ = self.started()
            print(f"\n  another alphabet: the game was given {command.split('-match_replay=', 1)[1]}")
            path = command.split('-match_replay="', 1)[1][:-1]
            path.encode("mbcs", "strict")                       # what GetCommandLineA keeps
            self.assertEqual(os.path.normcase(os.path.realpath(path)), os.path.normcase(os.path.realpath(target)))
            self.assertLessEqual(len(path.encode("mbcs")), 259)
        else:
            self.assertSaid(line, "Path")
            self.assertTrue(os.path.exists(target))
            self.assertIsNone(self.started(0.5))


if __name__ == "__main__":
    unittest.main()
