"""End-to-end tests of `updater_skhid.exe replay <id> <token> <sha256> <size>`
("Save replay"): the real program in a game folder made for each test (its
path has a space in it), the web server of test_updater.py on 127.0.0.1 in
the place of the srp one, serving replay files made here (the header the
server writes, then stand-in bytes).

    cd client/updater
    ./build.sh
    python tests/test_replay.py [-v] [TestName ...]

On Windows the exe runs as it is; elsewhere under Wine, as test_db_patches.py
runs it (`wine` on the PATH or SKHID_TEST_WINE; a prefix of its own unless
SKHID_TEST_WINEPREFIX names one). The stand-in program (tests/fake_exe) holds
the replay's lock where a test needs another updater holding it.

NO WINDOW: the updater is started only with the trailing word `quiet`, in
which it never calls MessageBox (exe/src/replay.rs: `notify` is the one
place that shows a box, and only when not quiet); `run_updater` refuses any
other command line, and `test_quiet_is_the_only_way_to_a_box` reads the
source to keep it so. On Windows, should a window of the updater appear all
the same, the process is killed at once and the test fails. The outcome of a
run is read from its log (`replay: saved <path>`, or `replay: FAILED
(<problem>): <text>`, the text in the game's language: English under Wine,
whose fresh prefix has no user.cfg). Nothing outside the temporary folders
is touched; SKHID_TEST_TEMP names the folder they go in.
"""

import hashlib
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import test_db_patches as wine  # noqa: E402  (imports test_updater's harness on any host)
from test_db_patches import WINDOWS, Holder, windows  # noqa: E402
from test_updater import FAKE_EXE, UPDATER, X86, Site  # noqa: E402

setUpModule = wine.setUpModule
tearDownModule = wine.tearDownModule

SOURCE = os.path.join(HERE, "..", "exe", "src")
ID = "0a1b2c3d"
TOKEN = "0123456789abcdef" * 2
MADE = 1_794_174_000
# A word of each message in each language (logic/src/replay.rs).
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
}


def game_language():
    """The messages' language: user.cfg's `localization` line (the game's and
    the launcher's), its number in the updater's five, else English (0)."""
    if not WINDOWS:
        return 0
    import ctypes
    documents = ctypes.create_unicode_buffer(260)
    if ctypes.windll.shell32.SHGetFolderPathW(None, 5, None, 0, documents):
        return 0
    try:
        with open(os.path.join(documents.value, "survarium", "user.cfg"), "rb") as file:
            lines = file.read().split(b"\n")
    except OSError:
        return 0
    for line in lines:
        words = line.split()
        if words and words[0] == b"localization" and len(words) > 1 and line[12:13] in (b" ", b"\t"):
            names = [b"english", b"russian", b"ukrainian", b"german", b"polish"]
            return names.index(words[1]) if words[1] in names else 0
    return 0


LANGUAGE = game_language()


def has_window(pid):
    """Whether the process has a visible top-level window (Windows only)."""
    if not WINDOWS:
        return False
    import ctypes
    import ctypes.wintypes as wintypes
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    enum = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    found = []

    def each(hwnd, _):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True

    user32.EnumWindows(enum(each), 0)
    return bool(found)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def replay_file(match=ID, slug="Rudnya", header=None, body=None):
    """A replay file as the server writes it: "SRPRPL01", the header's
    length and its lines, then (stand-ins here) the events and the replay."""
    if header is None:
        header = (f"format 1\nmatch {match}\nmade {MADE}\nlength 912\nmap 24\nmap_slug {slug}\nmode 4\n"
                  "score 87 100\nplace 0 0 4 7 Some One\n")
    head = header.encode("utf-8")
    body = os.urandom(3000) if body is None else body
    return b"SRPRPL01" + struct.pack("<I", len(head)) + head + struct.pack("<I", 0) + struct.pack("<I", len(body)) + body


def saved_name(slug="Rudnya", copy=1):
    """The name the updater gives a replay of MADE: the start in this
    computer's local time (Wine's is the host's)."""
    stamp = time.strftime("%Y-%m-%d_%H%M", time.localtime(MADE))
    return f"{stamp}_{slug}_{ID}" + (f"_{copy}" if copy > 1 else "") + ".srpreplay"


class QuietSource(unittest.TestCase):
    """No process is started here."""

    def test_quiet_is_the_only_way_to_a_box(self):
        with open(os.path.join(SOURCE, "replay.rs"), encoding="utf-8") as file:
            replay = file.read()
        # The one place of the boxes in the replay's code, behind `if !quiet`.
        self.assertEqual(len(re.findall(r"_box\(", replay)), 1)
        self.assertRegex(replay, r"fn notify\(quiet: bool, text: &str\) \{\s*if !quiet \{\s*crate::message_box\(text\);")
        self.assertNotRegex(replay, r"MessageBox|CreateWindow|DialogBox")
        # The box is MessageBoxW of main.rs alone, and the replay branch returns
        # before any of main.rs's own calls of it.
        windows_made = []
        for name in os.listdir(SOURCE):
            with open(os.path.join(SOURCE, name), encoding="utf-8") as file:
                text = file.read()
            if re.search(r"MessageBox\w*\(|CreateWindow|DialogBox", text):
                windows_made.append(name)
        self.assertEqual(windows_made, ["main.rs"])
        with open(os.path.join(SOURCE, "main.rs"), encoding="utf-8") as file:
            main = file.read()
        run = main[main.index("fn run() -> u32 {"):]
        branch = run.index("return replay::run(replay_arguments, quiet);")
        self.assertLess(branch, run.index("message_box("))
        self.assertIn("updater_logic::replay::command(&arguments)", run[:branch])

    def test_no_game_is_started(self):
        with open(os.path.join(SOURCE, "replay.rs"), encoding="utf-8") as file:
            replay = file.read()
        # A save starts no game: only a local file (`run_local`) does.
        save = replay[replay.index("pub fn run("):replay.index("pub fn run_local(")]
        for start in ("win::start(", "match_replay", "CreateProcess", "launch("):
            self.assertNotIn(start, save)
        body = replay[replay.index("fn launch("):]
        # One start of the game, after the check `ready` makes; its refusal
        # returns before it.
        self.assertEqual(replay.count("win::start("), 1)
        check = body.index("crate::patches::ready(layout).is_err()")
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
        self.folder = os.path.join(self.game, "replays")
        os.makedirs(self.x86, exist_ok=True)
        shutil.copy(UPDATER, os.path.join(self.x86, "updater_skhid.exe"))

    def path(self, name):
        return os.path.join(self.top, *name.split("/"))

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
        quiet`."""
        arguments = [*arguments, "quiet"]
        if arguments[0] != "replay" or arguments[-1] != "quiet":
            raise AssertionError(f"refused to start the updater without quiet: {arguments}")
        log = os.path.join(self.x86, "updater_skhid.log")
        if os.path.exists(log):
            os.remove(log)
        command, env = windows([os.path.join(self.x86, "updater_skhid.exe"), *arguments])
        process = subprocess.Popen(command, cwd=self.x86, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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

    def replays(self):
        return sorted(os.listdir(self.folder)) if os.path.isdir(self.folder) else []

    def read(self, name):
        with open(os.path.join(self.folder, name), "rb") as file:
            return file.read()

    def put(self, name, data):
        os.makedirs(self.folder, exist_ok=True)
        with open(os.path.join(self.folder, name), "wb") as file:
            file.write(data)

    def log(self):
        with open(os.path.join(self.x86, "updater_skhid.log"), "rb") as file:
            return file.read().decode("utf-8", "replace")

    def assertSaid(self, line, problem, *values):
        self.assertIsNotNone(line, f"no problem logged for {problem}")
        self.assertIn(f"FAILED ({problem}", line)
        self.assertIn(WORDS[problem][LANGUAGE].format(*values), line)

    def assertSaved(self, name):
        saved = [line for line in self.log().splitlines() if "replay: saved " in line]
        self.assertEqual(len(saved), 1, self.log())
        self.assertTrue(saved[0].endswith(f"\\replays\\{name}; exit 0"), saved[0])

    def assertNothingDone(self, requests=0):
        self.assertEqual([name for name in self.replays() if "srpreplay" in name], [])
        self.assertEqual(len(self.site.requests), requests, self.site.requests)

    # --- the normal runs -------------------------------------------------

    def test_a_replay_is_saved_under_its_start_map_and_match(self):
        data = replay_file()
        self.assertEqual(self.run_updater(self.offer(data)), (0, None))
        self.assertEqual(self.replays(), [saved_name()])
        self.assertEqual(self.read(saved_name()), data)
        self.assertEqual(self.site.requests, [f"/replays/{TOKEN}"])
        self.assertSaved(saved_name())
        log = self.log()
        self.assertIn(f"token {TOKEN[:6]}...", log)
        self.assertNotIn(TOKEN[:7], log)

    def test_a_replay_saved_before_is_kept_and_another_gets_the_next_name(self):
        data = replay_file()
        arguments = self.offer(data)
        self.assertEqual(self.run_updater(arguments), (0, None))
        # The same again: found by its bytes, nothing asked of the site.
        self.site.requests.clear()
        self.assertEqual(self.run_updater(arguments), (0, None))
        self.assertEqual(self.site.requests, [])
        self.assertSaved(saved_name())
        # The player renamed it: still found, under its own name.
        os.rename(os.path.join(self.folder, saved_name()), os.path.join(self.folder, f"mine_{ID}.srpreplay"))
        self.assertEqual(self.run_updater(arguments), (0, None))
        self.assertEqual(self.site.requests, [])
        self.assertSaved(f"mine_{ID}.srpreplay")
        # Other bytes under the name it would take: the next name, both kept.
        self.put(saved_name(), b"another replay")
        other = replay_file()
        self.assertEqual(self.run_updater(self.offer(other, token="f" * 32)), (0, None))
        self.assertEqual(self.read(saved_name(copy=2)), other)
        self.assertEqual(self.read(saved_name()), b"another replay")
        self.assertEqual(self.replays(), sorted([saved_name(), saved_name(copy=2), f"mine_{ID}.srpreplay"]))

    def test_a_map_name_is_kept_to_plain_letters(self):
        data = replay_file(slug="..\\..\\Vostok Radar")
        self.assertEqual(self.run_updater(self.offer(data)), (0, None))
        self.assertEqual(self.replays(), [saved_name("VostokRadar")])

    def test_saved_replays_and_other_files_stay_and_only_other_downloads_go(self):
        others = ["notes.txt", "srp_00000001.sur", "srp_00000002.sur.part", f"srp_{ID}.sur",
                  "2026-01-01_1200_Rudnya_00000003.srpreplay", "SRP_00000004.SRPREPLAY.PART",
                  "srp_0000000.srpreplay.part"]
        for name in others + ["srp_000000ee.srpreplay.part"]:
            self.put(name, b"x")
        self.assertEqual(self.run_updater(self.offer(replay_file())), (0, None))
        self.assertEqual(self.replays(), sorted(others + [saved_name()]))

    def test_one_save_at_a_time_and_none_waits_for_an_update(self):
        arguments = self.offer(replay_file())
        for name in ("updater_skhid.lock", "updater_skhid.replay.lock"):
            with open(os.path.join(self.x86, name), "wb"):
                pass
        update = Holder(self, f"{X86}/updater_skhid.lock", mode="lock")
        self.assertEqual(self.run_updater(arguments), (0, None))
        update.release()
        os.remove(os.path.join(self.folder, saved_name()))
        self.site.requests.clear()
        replay = Holder(self, f"{X86}/updater_skhid.replay.lock", mode="lock")
        code, line = self.run_updater(arguments)
        replay.release()
        self.assertEqual(code, 1)
        self.assertSaid(line, "AlreadyRunning")
        self.assertNothingDone()

    def test_a_game_folder_in_another_alphabet(self):
        shutil.rmtree(self.top)
        self.make_game(os.path.join(self.root, "Ελληνικά 遊戲", "Survarium"))
        self.write_url(self.site.url)
        data = replay_file()
        self.assertEqual(self.run_updater(self.offer(data)), (0, None))
        self.assertEqual(self.read(saved_name()), data)

    # --- what must save nothing -------------------------------------------

    def test_hostile_arguments_save_nothing_and_ask_nothing(self):
        sha = sha256(b"x")
        good = [ID, TOKEN, sha, "1"]
        cases = [
            [], good[:3], good + ["extra"], ["", "", "", ""],
            ["0A1B2C3D", TOKEN, sha, "1"], [ID[:-1], TOKEN, sha, "1"], [ID, TOKEN.upper(), sha, "1"],
            [ID, TOKEN[:-1], sha, "1"], [ID, TOKEN, sha.upper(), "1"], [ID, TOKEN, sha[:-1], "1"],
            [ID, TOKEN, sha, "0"], [ID, TOKEN, sha, str(12 * 2**20 + 1)], [ID, TOKEN, sha, "12345678901"],
            [ID, TOKEN, sha, "-1"], [ID, TOKEN, sha, "1e3"], [ID, TOKEN[:-1] + "&", sha, "1"],
            ["..\\..\\..", TOKEN, sha, "1"], [ID, "../../../../etc/passwd" + "0" * 10, sha, "1"],
            [ID, TOKEN, sha, "1", "&", "calc.exe"], ["quiet", *good],
        ]
        self.site.replays[TOKEN] = b"x"
        for arguments in cases:
            with self.subTest(arguments=arguments):
                code, line = self.run_updater(["replay", *arguments])
                self.assertEqual(code, 1)
                self.assertSaid(line, "BadLink")
                self.assertNothingDone()
                self.assertNotIn(TOKEN, self.log())

    def test_a_server_that_answers_wrong_saves_nothing(self):
        data = replay_file()
        arguments = ["replay", ID, TOKEN, sha256(data), str(len(data))]
        cases = [
            ("longer", (200, {}, data + b"more"), "Damaged", ()),
            ("shorter", (200, {}, data[:-1]), "Damaged", ()),
            ("other bytes", (200, {}, os.urandom(len(data))), "Damaged", ()),
            ("a redirect", (302, {"Location": f"/replays/{'f' * 32}"}, b""), "Status", (302,)),
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
                self.assertEqual(self.replays(), [], "a download left over")

    def test_a_file_that_is_no_replay_of_this_match_is_not_kept(self):
        # Each as the link says (size and checksum right), its header not.
        for label, data in [
            ("another match", replay_file(match="0a1b2c3e")),
            ("no replay file", b"PK\x03\x04" + os.urandom(2000)),
            ("a header past the file", b"SRPRPL01" + struct.pack("<I", 8000) + b"match 0a1b2c3d\n"),
            ("a header over the cap", b"SRPRPL01" + struct.pack("<I", 8193) + b"x" * 9000),
            ("a header with no start", replay_file(header=f"match {ID}\nmap 24\n")),
            ("a control byte", replay_file(header=f"match {ID}\nmade {MADE}\nmap 24\nmap_slug a\x1b[2J\n")),
        ]:
            with self.subTest(label):
                code, line = self.run_updater(self.offer(data))
                self.assertEqual(code, 1)
                self.assertSaid(line, "Damaged")
                self.assertEqual(self.replays(), [], "a download left over")

    def test_an_unreachable_site_is_tried_three_times(self):
        self.write_url("http://127.0.0.1:9")                # nothing listens there
        code, line = self.run_updater(self.offer(replay_file()), seconds=120)
        self.assertEqual(code, 1)
        self.assertSaid(line, "Network")
        self.assertNothingDone()
        self.assertEqual(self.log().count("replay: download "), 2)

    def test_only_its_own_site_by_its_own_rules(self):
        # Plain HTTP to another host than this computer is refused, as for updates.
        self.write_url("http://example.com")
        code, line = self.run_updater(self.offer(replay_file()))
        self.assertEqual(code, 1)
        self.assertIn("FAILED (server address)", line)
        self.assertIn("https://host", line)
        self.assertNothingDone()


if __name__ == "__main__":
    unittest.main()
