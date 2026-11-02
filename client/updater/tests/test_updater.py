"""End-to-end tests of updater_skhid.exe on Windows: the real program, a
game folder made for each test, a web server on 127.0.0.1 in the place of
the srp one, and this script in the place of the launcher (it makes the
launcher's named pipe, starts the updater with the launcher's command line
and reads what the updater says).

    cd client/updater
    ./build.sh                    (the updater and tests/fake_exe)
    python tests/test_updater.py [-v] [TestName ...]

Nothing outside the temporary folder is touched; no game file is needed.
SKHID_TEST_BUNDLE=<zip> also installs a real players' archive and compares
the result with Python's own unpacking of it.
"""

import _winapi
import ctypes
import hashlib
import http.server
import io
import json
import os
import shutil
import struct
import subprocess
import tempfile
import threading
import time
import unittest
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
RELEASE = os.path.join("target", "i686-win7-windows-msvc", "release")
UPDATER = os.path.join(HERE, "..", "exe", RELEASE, "updater_skhid.exe")
FAKE_EXE = os.path.join(HERE, "fake_exe", RELEASE, "fake-exe.exe")
X86 = os.path.join("game", "binaries", "x86")

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
kernel32.CreateMutexW.restype = ctypes.c_void_p
kernel32.CloseHandle.argtypes = [ctypes.c_void_p]

# The launcher's messages and the updater's states (client/updater/logic/src/pipe.rs).
ABORT = b"\x1b"
CHECKING, CHECK_FAILED, DOWNLOADING, DOWNLOAD_FAILED, DOWNLOADED = 0x04, 0x05, 0x09, 0x0A, 0x0C
LATEST, ABORTED, INSTALLING, COPIED, ERROR = 0x0E, 0x0F, 0x10, 0x13, 0x01

# The updater's own box (Retry and Cancel: a file blocked or held, a server
# not reached), found by its process and answered as a player would.
# (Windows only, as the pipe: test_db_patches.py imports this elsewhere.)
if os.name == "nt":
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    WNDENUMPROC = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    user32.EnumWindows.argtypes = [WNDENUMPROC, ctypes.c_void_p]
    user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
    user32.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
    user32.GetDlgItemTextW.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_wchar_p, ctypes.c_int]
    user32.PostMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
WM_COMMAND, IDCANCEL, IDRETRY = 0x0111, 2, 4


def updater_boxes(pid):
    """The message boxes (dialogs of class #32770) the process `pid` shows:
    [(window, the box's text)]."""
    found = []

    def look(window, _):
        owner = ctypes.c_ulong()
        user32.GetWindowThreadProcessId(window, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(window):
            kind = ctypes.create_unicode_buffer(64)
            user32.GetClassNameW(window, kind, 64)
            if kind.value == "#32770":
                text = ctypes.create_unicode_buffer(4096)
                user32.GetDlgItemTextW(window, 0xFFFF, text, 4096)   # the box's static text
                found.append((window, text.value))
        return True

    user32.EnumWindows(WNDENUMPROC(look), None)
    return found


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def make_zip(files):
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in files.items():
            # A fixed time: the same files are the same archive, byte for byte.
            entry = zipfile.ZipInfo(name, date_time=(2026, 10, 13, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, data)
    return out.getvalue()


class Site:
    """The web server: `/api/client-files` and `/download`, as told; the
    list of resources.db's patches at `/patches` (by default none: `{}`;
    each patch with its `urls`, as srp writes it), and the patch files at
    `/store/patch_<n>`; `/hop/<k>/patch_<n>` redirects k times before it is
    `/store/patch_<n>`."""

    def __init__(self):
        self.archive = b""
        self.manifest = None            # bytes to answer instead of the true manifest
        self.manifest_status = 200
        self.download_statuses = []     # statuses for the next downloads, then 200
        self.cut_downloads = 0          # the next N downloads end half way
        self.slow = 0.0                 # seconds between pieces of a download
        self.blobs = {}                 # /resources/<sha256>: sha256 -> bytes (stock bytes, for the restore)
        self.blob_statuses = {}         # sha256 -> another status than 200
        self.blob_bodies = {}           # sha256 -> other bytes than the blob's
        self.patches = {}               # n -> bytes: listed at /patches, served at /store/patch_<n>
        self.patch_list = None          # bytes to answer /patches with instead of the true list
        self.patch_list_status = 200
        self.patch_base = None          # where the list says the files are (default: this site)
        self.patch_statuses = {}        # n -> another status than 200 for its file
        self.patch_bodies = {}          # n -> other bytes than the listed ones
        self.patch_lengths = {}         # n -> a Content-Length announced in the place of the file
        self.patch_redirects = {}       # n -> a Location answered with 302 in the place of the file
        self.patch_html = set()         # n: a web page (text/html) answered in the place of the file
        self.patch_urls = {}            # n -> the addresses the list gives it (default: one, on patch_base)
        self.old_list = False           # the list in the form before updater 0.3.0: one "url"
        self.patch_cut = {}             # n -> bytes sent of the file, its whole length announced
        self.patch_slow = 0.0           # seconds between pieces of a patch file
        self.replays = {}               # /replays/<token>: token -> bytes
        self.replay_answers = {}        # token -> (status, headers, body) instead
        self.requests = []
        site = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def answer(self, status, body, content_type="text/plain"):
                self.send_response(status)
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                site.requests.append(self.path)
                if self.path == "/api/client-files":
                    body = site.manifest if site.manifest is not None else site.true_manifest()
                    self.answer(site.manifest_status, body if site.manifest_status == 200 else b"no\n")
                elif self.path == "/download":
                    status = site.download_statuses.pop(0) if site.download_statuses else 200
                    if status != 200:
                        self.answer(status, b"")
                        return
                    body = site.archive
                    self.send_response(200)
                    self.send_header("Content-Type", "application/zip")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    if site.cut_downloads:
                        site.cut_downloads -= 1
                        body = body[:len(body) // 2]
                    try:
                        for at in range(0, len(body), 4096):
                            self.wfile.write(body[at:at + 4096])
                            self.wfile.flush()
                            if site.slow:
                                time.sleep(site.slow)
                    except OSError:
                        pass
                elif self.path == "/patches":
                    if site.patch_list_status != 200:
                        self.answer(site.patch_list_status, b"")
                    else:
                        body = site.patch_list if site.patch_list is not None else site.true_patch_list()
                        self.answer(200, body, "application/json")
                elif self.path.startswith("/hop/"):
                    # /hop/<k>/patch_<n>: k redirects, by turns an absolute
                    # address and one on this host, then the file.
                    _, _, hops, name = self.path.split("/", 3)
                    hops = int(hops)
                    if hops == 0:
                        self.path = f"/store/{name}"
                        return self.do_GET()
                    target = f"/hop/{hops - 1}/{name}"
                    if hops % 2:
                        target = site.url + target
                    # As Google's: the redirect itself is a web page; only
                    # the last answer is the file's.
                    body = b"<html><body>Moved</body></html>"
                    self.send_response(302)
                    self.send_header("Location", target)
                    self.send_header("Content-Type", "text/html; charset=UTF-8")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                elif self.path.startswith("/store/patch_"):
                    name = self.path[len("/store/patch_"):]
                    number = int(name) if name.isdigit() else None
                    status = site.patch_statuses.get(number, 200 if number in site.patches else 404)
                    if number in site.patch_redirects:
                        self.send_response(302)
                        self.send_header("Location", site.patch_redirects[number])
                        self.send_header("Content-Length", "0")
                        self.end_headers()
                    elif status != 200:
                        self.answer(status, b"")
                    elif number in site.patch_html:
                        # Google Drive's "cannot scan this file for viruses".
                        self.answer(200, b"<!DOCTYPE html><html><title>Google Drive - Virus scan warning"
                                         b"</title></html>", "text/html; charset=utf-8")
                    elif number in site.patch_cut:
                        # The connection ends before the file does.
                        body = site.patch_bodies.get(number, site.patches[number])
                        self.send_response(200)
                        self.send_header("Content-Length", str(len(body)))
                        self.end_headers()
                        try:
                            self.wfile.write(body[:site.patch_cut[number]])
                        except OSError:
                            pass
                        self.close_connection = True
                    elif number in site.patch_lengths:
                        # A file larger than the disk: its length, a few bytes.
                        self.send_response(200)
                        self.send_header("Content-Length", str(site.patch_lengths[number]))
                        self.end_headers()
                        try:
                            self.wfile.write(b"x" * 1024)
                        except OSError:
                            pass
                        self.close_connection = True
                    elif site.patch_slow:
                        body = site.patch_bodies.get(number, site.patches[number])
                        self.send_response(200)
                        self.send_header("Content-Length", str(len(body)))
                        self.end_headers()
                        try:
                            for at in range(0, len(body), 65536):
                                self.wfile.write(body[at:at + 65536])
                                self.wfile.flush()
                                time.sleep(site.patch_slow)
                        except OSError:
                            pass
                    else:
                        self.answer(200, site.patch_bodies.get(number, site.patches[number]),
                                    "application/octet-stream")
                elif self.path.startswith("/resources/"):
                    name = self.path[len("/resources/"):]
                    status = site.blob_statuses.get(name, 200 if name in site.blobs else 404)
                    if status != 200:
                        self.answer(status, b"")
                    else:
                        self.answer(200, site.blob_bodies.get(name, site.blobs[name]), "application/octet-stream")
                elif self.path.startswith("/replays/"):
                    token = self.path[len("/replays/"):]
                    if token in site.replay_answers:
                        status, headers, body = site.replay_answers[token]
                        self.send_response(status)
                        for key, value in headers.items():
                            self.send_header(key, value)
                        self.send_header("Content-Length", str(len(body)))
                        self.end_headers()
                        self.wfile.write(body)
                    elif token in site.replays:
                        self.answer(200, site.replays[token], "application/octet-stream")
                    else:
                        self.answer(404, b"")
                else:
                    self.answer(404, b"not found\n")

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.url = f"http://127.0.0.1:{self.server.server_address[1]}"
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def true_manifest(self):
        return f"skhid-client-files 1\nsha256 {sha256(self.archive)}\nsize {len(self.archive)}\n".encode()

    def true_patch_list(self):
        base = self.patch_base or self.url
        listed = {}
        for n, data in self.patches.items():
            urls = self.patch_urls.get(n, [f"{base}/store/patch_{n}"])
            if self.old_list:
                (url,) = urls
                listed[f"patch_{n}"] = {"sha256": sha256(data), "url": url}
            else:
                listed[f"patch_{n}"] = {"sha256": sha256(data), "urls": urls}
        return json.dumps(listed).encode()

    def close(self):
        self.server.shutdown()
        self.server.server_close()


class Launcher:
    """The launcher's end: its pipe, the updater's process, what it said."""

    counter = 0

    def __init__(self, x86, action="update"):
        Launcher.counter += 1
        self.product = f"SkhidTest{os.getpid()}x{Launcher.counter}"
        name = rf"\\.\pipe\vostok_updater_pipe-{self.product}"
        # As pipe::accept of the launcher makes it: duplex, bytes, one instance.
        self.pipe = _winapi.CreateNamedPipe(
            name, _winapi.PIPE_ACCESS_DUPLEX | 0x00080000, 0,   # first instance; bytes, waiting
            1, 0x10000, 0x10000, 0, _winapi.NULL)
        exe = os.path.join(x86, "updater_skhid.exe")
        # run_survarium_executable_as_administrator: every argument in quotes
        # (the fourteen of start_updater; only the first three matter).
        arguments = [exe, action, name, "0.26g0", "torrent", "", "C:\\Start Menu", "C:\\Desktop",
                     "C:\\Documents", "", "a user", "C:\\Temp", "skhid.link", ""]
        self.process = subprocess.Popen(" ".join(f'"{argument}"' for argument in arguments), cwd=x86)
        try:
            _winapi.ConnectNamedPipe(self.pipe)
        except OSError as error:
            if error.winerror != 535:       # ERROR_PIPE_CONNECTED: it was faster
                raise
        self.buffer = b""
        self.frames = []
        self.open = True
        # The updater's own boxes, their texts, and how the next ones are
        # answered (then Cancel).
        self.boxes = []
        self.box_answers = []
        self.answered = set()

    def poll(self):
        for window, text in updater_boxes(self.process.pid):
            if window not in self.answered:
                self.answered.add(window)
                self.boxes.append(text)
                answer = self.box_answers.pop(0) if self.box_answers else IDCANCEL
                user32.PostMessageW(window, WM_COMMAND, answer, 0)
        while self.open:
            try:
                waiting, _ = _winapi.PeekNamedPipe(self.pipe, 0)
                if not waiting:
                    break
                data, _ = _winapi.ReadFile(self.pipe, waiting)
                self.buffer += data
            except OSError:
                self.open = False
        while len(self.buffer) >= 2:
            (length,) = struct.unpack_from("<H", self.buffer)
            if len(self.buffer) < 2 + length:
                break
            self.frames.append(self.buffer[2:2 + length])
            self.buffer = self.buffer[2 + length:]

    def send(self, payload):
        _winapi.WriteFile(self.pipe, struct.pack("<H", len(payload)) + payload)

    def states(self):
        return [frame[1] for frame in self.frames if frame[0] == 0x02]

    def texts(self, kind=0x05):
        return [frame[2:2 + frame[1]].decode("ascii") for frame in self.frames if frame[0] == kind]

    def wait_for(self, done, seconds=60, what="the expected messages"):
        until = time.time() + seconds
        while time.time() < until:
            self.poll()
            if done(self):
                return
            if not self.open:
                break
            time.sleep(0.02)
        self.poll()
        if not done(self):
            raise AssertionError(f"no {what}; states {[hex(s) for s in self.states()]}, "
                                 f"texts {self.texts()}, pipe open {self.open}")

    def wait_for_state(self, state, seconds=60):
        self.wait_for(lambda launcher: state in launcher.states(), seconds, f"state {state:#x}")

    def finish(self, abort=True):
        """Closes as the launcher does: abort, then the pipe. Returns the
        updater's exit code."""
        if abort and self.open:
            try:
                self.send(ABORT)
            except OSError:
                pass
        deadline = time.time() + 20
        while self.process.poll() is None and time.time() < deadline:
            self.poll()
            time.sleep(0.02)
        self.poll()
        _winapi.CloseHandle(self.pipe)
        self.open = False
        return self.process.wait(20)


class UpdaterTest(unittest.TestCase):
    def setUp(self):
        for needed in (UPDATER, FAKE_EXE):
            if not os.path.isfile(needed):
                self.fail(f"{needed} is not built: run client/updater/build.sh")
        self.root = tempfile.mkdtemp(prefix="skhid-updater-test-")
        self.top = os.path.join(self.root, "Survarium")
        self.x86 = os.path.join(self.top, X86)
        os.makedirs(self.x86)
        shutil.copy(UPDATER, os.path.join(self.x86, "updater_skhid.exe"))
        # These scenarios exercise overlay updates over an existing game.
        self.write("game/resources.db", b"existing game resources")
        self.write(f"{X86}/survarium.exe", b"existing original game")
        self.site = Site()
        self.write(f"{X86}/updater_skhid.url", self.site.url.encode())
        self.launchers = []
        with open(UPDATER, "rb") as file:
            self.updater_bytes = file.read()
        with open(FAKE_EXE, "rb") as file:
            self.fake = file.read()

    def tearDown(self):
        for launcher in self.launchers:
            if launcher.process.poll() is None:
                launcher.process.kill()
                launcher.process.wait(10)
        self.site.close()
        shutil.rmtree(self.root, ignore_errors=True)

    def path(self, name):
        return os.path.join(self.top, *name.split("/"))

    def write(self, name, data):
        os.makedirs(os.path.dirname(self.path(name)), exist_ok=True)
        with open(self.path(name), "wb") as file:
            file.write(data)

    def read(self, name):
        with open(self.path(name), "rb") as file:
            return file.read()

    def exists(self, name):
        return os.path.exists(self.path(name))

    def launcher(self, action="update"):
        launcher = Launcher(self.x86, action)
        self.launchers.append(launcher)
        return launcher

    def bundle(self, **changes):
        """An archive like the players' one. The updater in it is the one
        that runs, so it is not replaced unless a test changes it."""
        files = {
            f"{X86}/survarium_skhid.exe": self.fake + b"game 1",
            f"{X86}/bugtrap.dll": b"MZ a dll " * 500,
            f"{X86}/play_skhid.bat": b"@echo off\r\nset LANGUAGE=english\r\n",
            f"{X86}/updater_skhid.exe": self.updater_bytes,
            f"{X86}/launcher_skhid/index-en.html": b"<html>offline</html>",
            "game/resources/ssl/survarium_login_server.crt": b"-----BEGIN CERTIFICATE-----\n",
            "README.txt": b"read me\r\n",
        }
        for name, data in changes.items():
            name = name.replace("__", "/")
            if data is None:
                files.pop(name)
            else:
                files[name] = data
        return files

    def offer(self, files):
        self.site.archive = make_zip(files)
        return files

    def leftovers(self):
        found = []
        for folder, _, names in os.walk(self.top):
            found += [name for name in names if ".skhid-new" in name or ".skhid-old" in name]
        return found

    def install(self, files):
        """A whole normal run that ends in "latest"."""
        self.offer(files)
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        launcher.said = launcher.states()       # before it is told to stop
        self.assertEqual(launcher.finish(), 0)
        return launcher

    def log(self):
        return self.read(f"{X86}/updater_skhid.log").decode("utf-8", "replace")

    # --- the normal runs -------------------------------------------------

    def test_a_first_run_installs_everything_and_the_next_finds_it_up_to_date(self):
        files = self.offer(self.bundle())
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        for name, data in files.items():
            self.assertEqual(self.read(name), data, name)
        # What the launcher was told, in order: checking, downloading with
        # progress, downloaded, installing with the file names, then
        # "checking" (which clears the file name line) and "latest".
        self.assertEqual(launcher.states(), [CHECKING, DOWNLOADING, DOWNLOADED, INSTALLING, CHECKING, LATEST])
        self.assertIn("survarium_skhid.exe", launcher.texts(0x07))
        self.assertNotIn("updater_skhid.exe", launcher.texts(0x07))   # already the same bytes
        progress = [struct.unpack("<4Q", frame[1:]) for frame in launcher.frames if frame[0] == 0x01]
        self.assertTrue(any(done == total == len(self.site.archive) for done, total, _, _ in progress))
        self.assertTrue(all(frame[0] in range(1, 24) for frame in launcher.frames), "a message type the launcher would crash on")
        # It stays until the launcher says stop, then says "aborted" and ends.
        time.sleep(0.5)
        self.assertIsNone(launcher.process.poll())
        self.assertEqual(launcher.finish(), 0)
        self.assertEqual(launcher.states()[-1], ABORTED)
        self.assertEqual(self.leftovers(), [])
        state = self.read(f"{X86}/updater_skhid.state").decode()
        self.assertIn(f"archive {sha256(self.site.archive)}", state)
        self.assertIn(f"file {sha256(files['README.txt'])} 9 README.txt", state)

        # The second start: one question, no download.
        self.site.requests.clear()
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        self.assertEqual(launcher.states(), [CHECKING, LATEST])
        self.assertEqual(self.site.requests, ["/api/client-files"])
        self.assertEqual(launcher.finish(), 0)

    def test_a_new_archive_replaces_the_files_and_takes_away_those_it_no_longer_has(self):
        self.install(self.bundle())
        self.write(f"{X86}/play_skhid.bat", b"@echo off\r\nset LANGUAGE=ukrainian\r\n")
        self.write(f"{X86}/my_notes.txt", b"the player's own file")
        files = self.bundle(**{
            f"{X86}/survarium_skhid.exe".replace("/", "__"): self.fake + b"game 2",
            f"{X86}/bugtrap.dll".replace("/", "__"): None,
            "LINUX.txt": b"new in this archive",
        })
        launcher = self.install(files)
        self.assertEqual(launcher.said, [CHECKING, DOWNLOADING, DOWNLOADED, INSTALLING, CHECKING, LATEST])
        for name, data in files.items():
            self.assertEqual(self.read(name), data, name)
        # A new archive brings its own start script, the player's edit goes.
        self.assertIn(b"english", self.read(f"{X86}/play_skhid.bat"))
        # Gone from the archive, and still as it was installed: taken away.
        self.assertFalse(self.exists(f"{X86}/bugtrap.dll"))
        # Never installed by the updater: not its to touch.
        self.assertEqual(self.read(f"{X86}/my_notes.txt"), b"the player's own file")
        self.assertEqual(self.leftovers(), [])

    def test_a_damaged_program_is_repaired_and_an_edited_script_is_left_alone(self):
        files = self.bundle()
        self.install(files)
        edited = b"@echo off\r\nset LANGUAGE=ukrainian\r\n"
        self.write(f"{X86}/play_skhid.bat", edited)
        launcher = self.install(files)
        self.assertEqual(launcher.said, [CHECKING, LATEST], "an edited script is no reason to install")

        self.write(f"{X86}/survarium_skhid.exe", b"damaged")
        os.remove(self.path("game/resources/ssl/survarium_login_server.crt"))
        launcher = self.install(files)
        self.assertIn(DOWNLOADING, launcher.states())
        self.assertEqual(self.read(f"{X86}/survarium_skhid.exe"), files[f"{X86}/survarium_skhid.exe"])
        self.assertTrue(self.exists("game/resources/ssl/survarium_login_server.crt"))
        # A repair brings back what was damaged or lost, and nothing else:
        # the script stays the player's.
        self.assertEqual(sorted(launcher.texts(0x07)), ["survarium_login_server.crt", "survarium_skhid.exe"])
        self.assertEqual(self.read(f"{X86}/play_skhid.bat"), edited)
        self.assertEqual(self.install(files).said, [CHECKING, LATEST])

    def test_the_launcher_and_the_updater_are_replaced_while_they_run(self):
        self.install(self.bundle())
        new_updater = self.updater_bytes + b"\0new build"
        files = self.offer(self.bundle(**{
            f"{X86}/updater_skhid.exe".replace("/", "__"): new_updater,
            f"{X86}/launcher_skhid.exe".replace("/", "__"): self.fake,
        }))
        launcher = self.launcher()
        # The launcher holds its one-at-a-time mutex while it runs.
        mutex = kernel32.CreateMutexW(None, False, f"{launcher.product}_or_its_installer_is_already_running")
        self.assertTrue(mutex)
        try:
            launcher.wait_for_state(ABORTED)
            launcher.wait_for(lambda l: not l.open, 10, "closed pipe")
            self.assertEqual(launcher.states(), [CHECKING, DOWNLOADING, DOWNLOADED, INSTALLING, ABORTED])
            self.assertEqual(self.read(f"{X86}/updater_skhid.exe"), new_updater)
            self.assertEqual(self.read(f"{X86}/launcher_skhid.exe"), self.fake)
            # The new launcher is not started while the old one is there.
            started = f"{X86}/launcher_skhid.exe.started.txt"
            time.sleep(1.5)
            self.assertFalse(self.exists(started))
            self.assertIsNone(launcher.process.poll())
        finally:
            kernel32.CloseHandle(mutex)
        self.assertEqual(launcher.process.wait(20), 0)
        _winapi.CloseHandle(launcher.pipe)
        deadline = time.time() + 10
        while not self.exists(started) and time.time() < deadline:
            time.sleep(0.05)
        command, folder = self.read(started).decode("utf-16").strip().splitlines()
        self.assertEqual(command, f'"{self.x86}\\launcher_skhid.exe" -product_name={launcher.product}')
        self.assertEqual(os.path.normcase(folder), os.path.normcase(self.x86))
        # The file of the updater that ran is deleted at the next start.
        self.assertEqual(self.leftovers(), ["updater_skhid.exe.skhid-old"])
        files[f"{X86}/updater_skhid.exe"] = new_updater
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        self.assertEqual(launcher.states(), [CHECKING, LATEST])
        self.assertEqual(launcher.finish(), 0)
        self.assertEqual(self.leftovers(), [])

    def test_the_real_players_archive_installs_as_python_unpacks_it(self):
        bundle = os.environ.get("SKHID_TEST_BUNDLE")
        if not bundle:
            self.skipTest("SKHID_TEST_BUNDLE is not set")
        with open(bundle, "rb") as file:
            self.site.archive = file.read()
        # By hand: with a launcher's pipe it would start the archive's real
        # launcher once that is installed.
        exe = os.path.join(self.x86, "updater_skhid.exe")
        self.assertEqual(subprocess.run([exe, "check", "quiet"], timeout=120).returncode, 0, self.log())
        with zipfile.ZipFile(bundle) as archive:
            for info in archive.infolist():
                # (The archive's own .url would send the next test run elsewhere.)
                if not info.is_dir():
                    self.assertEqual(self.read(info.filename), archive.read(info), info.filename)
        # The updater that ran was replaced by the archive's, if that is another build.
        self.assertLessEqual(set(self.leftovers()), {"updater_skhid.exe.skhid-old"})
        self.assertIn("\r\nend\r\n", self.read(f"{X86}/updater_skhid.state").decode())

    # --- what can go wrong -----------------------------------------------

    def test_no_server_at_the_first_start_says_so_and_keeps_play_off(self):
        # The files were copied by hand and the updater never installed
        # anything: it knows nothing against them. But the patches of
        # resources.db were never checked: PLAY stays off.
        self.offer(self.bundle())
        self.site.close()
        launcher = self.launcher()
        # The server not reached is said in the updater's own box, in the
        # launcher's language (English here unless this PC's launcher keeps
        # another): its title (blocked on this network when Windows' own
        # test site answers, else no connection), what happened, what to
        # run, the log. Retry runs it all again.
        launcher.box_answers = [IDRETRY]
        launcher.wait_for(lambda l: len(l.boxes) == 2 and ERROR in l.states(), 90, "the updater's boxes")
        self.assertEqual(launcher.states(), ([CHECKING, CHECK_FAILED] * 2 + [CHECKING, ERROR]) * 2)
        for box in launcher.boxes:
            lines = box.splitlines()
            self.assertEqual(len(lines), 4, box)
            self.assertTrue(lines[-1].endswith("updater_skhid.log"), box)
        if launcher.boxes[0].startswith(("Blocked", "No connection")):
            title = launcher.boxes[0].splitlines()[0]
            self.assertIn(title, ("Blocked on this network: 127.0.0.1 did not answer (connection refused)",
                                  "No connection: 127.0.0.1 did not answer (connection refused)"))
            self.assertIn("VPN.txt" if title.startswith("Blocked") else "Skhid-network-setup.cmd",
                          launcher.boxes[0])
        # The box said it: the launcher is not told again.
        self.assertEqual(launcher.texts(), [])
        self.assertEqual(launcher.finish(), 1)
        log = self.log()
        self.assertIn("the player asks to try again", log)
        self.assertRegex(log, r"The game's patches could not be checked: 127\.0\.0\.1 refused the connection\. "
                              r".*VPN\.txt\. PLAY stays off until this is done\.")
        self.site = Site()

    def test_no_server_but_whole_files_lets_the_game_be_played(self):
        self.install(self.bundle())
        self.site.close()
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        # The box first (Cancel: play with what is installed), and nothing
        # more to the launcher.
        self.assertEqual(len(launcher.boxes), 1)
        self.assertEqual(launcher.texts(), [])
        self.assertRegex(self.log(), r"No update check: 127\.0\.0\.1 refused the connection\..*you can play\.")
        self.assertEqual(launcher.states()[-1], LATEST)
        self.assertEqual(launcher.finish(), 0)
        # But not with a program file missing: that is an error, with the
        # file's name and the reason it cannot be repaired.
        os.remove(self.path(f"{X86}/survarium_skhid.exe"))
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.boxes and ERROR in l.states(), 60, "the updater's box")
        self.assertNotIn(LATEST, launcher.states())
        # "Error" in the launcher's line, the updater's box with what to try,
        # and the file in the log.
        self.assertEqual(launcher.states()[-1], ERROR)
        self.assertEqual(launcher.texts(), [])
        self.assertEqual(launcher.finish(), 1)
        self.assertRegex(self.log(), r"x86/survarium_skhid[.]exe is missing or damaged and cannot be "
                                     r"repaired now: 127\.0\.0\.1 refused the connection\.")
        self.site = Site()

    def test_a_server_without_client_files_says_so(self):
        self.site.manifest_status = 404
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.texts(), 30, "error text")
        self.assertEqual(launcher.states(), [CHECKING, ERROR])
        # The patches were never checked: PLAY stays off.
        self.assertEqual(launcher.texts(), ["The game's patches could not be checked: 127.0.0.1 offers no client "
                                            "files now. Try again later. PLAY stays off until this is done."])
        self.assertEqual(launcher.finish(), 1)
        # With the patches checked at an earlier start, the player may try.
        self.site.manifest_status = 200
        self.install(self.bundle())
        self.site.manifest_status = 404
        launcher = self.launcher()
        launcher.wait_for_state(LATEST, 30)
        self.assertEqual(launcher.texts(), ["No update check: 127.0.0.1 offers no client files now. Try again later. "
                                            "The installed files are complete, so you can play."])
        self.assertEqual(launcher.finish(), 0)

    def test_an_answer_that_is_not_the_servers_is_told_apart(self):
        self.site.manifest = b"<html><body>Sign in to the Wi-Fi</body></html>"
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.texts(), 60, "the text")
        self.assertIn("is not the update server's", launcher.texts()[0])
        self.assertEqual(launcher.finish(), 0)
        self.site.manifest = b"skhid-client-files 2\nfuture things\n"
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.texts(), 30, "the text")
        self.assertIn("newer form than this updater reads", launcher.texts()[0])
        self.assertEqual(launcher.finish(), 0)

    def test_a_wrong_checksum_installs_nothing(self):
        self.install(self.bundle())
        before = {name: self.read(name) for name in self.bundle()}
        self.offer(self.bundle(README__txt=b"changed"))
        self.site.manifest = f"skhid-client-files 1\nsha256 {'ab' * 32}\nsize {len(self.site.archive)}\n".encode()
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.texts(), 90, "error text")
        self.assertIn("the checksum is wrong", launcher.texts()[0])
        self.assertEqual(launcher.states().count(DOWNLOADING), 4)
        self.assertEqual(launcher.states()[-1], ERROR)
        self.assertEqual(launcher.finish(), 1)
        for name, data in before.items():
            self.assertEqual(self.read(name), data, name)
        self.assertEqual(self.leftovers(), [])

    def test_a_download_that_breaks_off_is_tried_again(self):
        files = self.offer(self.bundle())
        self.site.cut_downloads = 1
        self.site.download_statuses = [200, 500]
        launcher = self.launcher()
        launcher.wait_for_state(LATEST, 90)
        self.assertEqual(launcher.states(), [CHECKING, DOWNLOADING, DOWNLOAD_FAILED, DOWNLOADING, DOWNLOAD_FAILED,
                                             DOWNLOADING, DOWNLOADED, INSTALLING, CHECKING, LATEST])
        self.assertEqual(launcher.texts(), [])
        self.assertEqual(launcher.finish(), 0)
        self.assertEqual(self.read("README.txt"), files["README.txt"])
        log = self.log()
        self.assertIn("broke off", log)
        self.assertIn("answered /download with error 500", log)

    def test_an_archive_that_would_write_outside_the_game_is_refused(self):
        self.site.archive = make_zip({"README.txt": b"fine", "../outside.txt": b"not fine"})
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.texts(), 30, "error text")
        self.assertIn("cannot be used (BadName)", launcher.texts()[0])
        self.assertEqual(launcher.finish(), 1)
        self.assertFalse(os.path.exists(os.path.join(self.root, "outside.txt")))
        self.assertFalse(self.exists("README.txt"), "no file of a refused archive is written")
        self.site.archive = b"this is not a zip" * 10
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.texts(), 30, "error text")
        self.assertIn("cannot be used (NotZip)", launcher.texts()[0])
        self.assertEqual(launcher.finish(), 1)

    def test_a_file_that_cannot_be_replaced_leaves_everything_as_it_was(self):
        self.install(self.bundle())
        before = {name: self.read(name) for name in self.bundle()}
        self.offer(self.bundle(**{
            "README.txt": b"second",
            f"{X86}/bugtrap.dll".replace("/", "__"): b"MZ new dll",
            f"{X86}/survarium_skhid.exe".replace("/", "__"): self.fake + b"game 2",
        }))
        # Another program holds the game exe and lets nobody rename it.
        held = _winapi.CreateFile(self.path(f"{X86}/survarium_skhid.exe"), _winapi.GENERIC_READ, 1,
                                  _winapi.NULL, _winapi.OPEN_EXISTING, 0, _winapi.NULL)
        try:
            launcher = self.launcher()
            launcher.wait_for(lambda l: l.texts(), 60, "error text")
            self.assertEqual(launcher.states()[-2:], [INSTALLING, ERROR])
            self.assertRegex(launcher.texts()[0], r"^Cannot replace x86\\survarium_skhid\.exe: .* \(error 32\)\. "
                                                  r"Nothing was changed\. Another program holds the file")
            self.assertEqual(launcher.finish(), 1)
        finally:
            _winapi.CloseHandle(held)
        for name, data in before.items():
            self.assertEqual(self.read(name), data, name)
        self.assertEqual(self.leftovers(), [])
        # Once it is let go, the same start succeeds.
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        self.assertEqual(self.read("README.txt"), b"second")
        self.assertEqual(launcher.finish(), 0)

    def test_closing_the_launcher_during_a_download_stops_it_cleanly(self):
        self.offer(self.bundle(**{f"{X86}/big.bin".replace("/", "__"): os.urandom(600_000)}))
        self.site.slow = 0.02
        launcher = self.launcher()
        launcher.wait_for(lambda l: any(frame[0] == 0x01 and struct.unpack_from("<Q", frame, 1)[0] > 0
                                        for frame in l.frames), 30, "download progress")
        self.assertEqual(launcher.finish(abort=True), 0)
        self.assertEqual(launcher.states()[-1], ABORTED)
        self.assertNotIn(INSTALLING, launcher.states())
        self.assertFalse(self.exists("README.txt"))
        self.assertEqual(self.leftovers(), [])

    def test_a_launcher_that_just_goes_away_ends_the_updater_too(self):
        self.install(self.bundle())
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        _winapi.CloseHandle(launcher.pipe)      # no abort: the process was killed
        launcher.open = False
        self.assertEqual(launcher.process.wait(20), 0)

    # --- what the overlay means for the patches -------------------------

    def test_the_updater_that_replaced_itself_leaves_the_rest_to_the_new_one(self):
        """The rest under Wine too: test_db_patches.py."""
        self.site.patches[14] = b"patch 14 " * 1000
        status = f"{X86}/updater_skhid.patches.status"
        self.install(self.bundle())
        self.assertEqual(self.read(status), b"ready\r\n")
        new_updater = self.updater_bytes + b"\0new build"
        self.offer(self.bundle(**{
            f"{X86}/updater_skhid.exe".replace("/", "__"): new_updater,
            f"{X86}/launcher_skhid.exe".replace("/", "__"): self.fake,
        }))
        launcher = self.launcher()
        mutex = kernel32.CreateMutexW(None, False, f"{launcher.product}_or_its_installer_is_already_running")
        try:
            launcher.wait_for_state(ABORTED)
            self.assertEqual(launcher.states(), [CHECKING, DOWNLOADING, DOWNLOADED, INSTALLING, ABORTED])
        finally:
            kernel32.CloseHandle(mutex)
        self.assertEqual(launcher.process.wait(20), 0)
        _winapi.CloseHandle(launcher.pipe)
        self.assertIn("the rest of the update is done by the new one", self.log())
        # The patches are not known to be the new files' until the new one
        # has checked them with the server's list.
        self.assertEqual(self.read(status), b"blocked overlay changed\r\n")
        launcher = self.launcher()
        launcher.wait_for_state(LATEST)
        self.assertEqual(launcher.finish(), 0)
        self.assertEqual(self.read(status), b"ready\r\n")

    # --- the other ways it is started -----------------------------------

    def test_copy_is_done_at_once(self):
        launcher = self.launcher("copy")
        launcher.wait_for_state(COPIED, 10)
        self.assertEqual(launcher.states(), [COPIED])
        self.assertEqual(launcher.finish(), 0)

    def test_by_hand_it_updates_without_a_launcher(self):
        files = self.offer(self.bundle())
        exe = os.path.join(self.x86, "updater_skhid.exe")
        self.assertEqual(subprocess.run([exe, "check", "quiet"], cwd=self.root, timeout=120).returncode, 0)
        self.assertEqual(self.read("README.txt"), files["README.txt"])
        self.site.manifest_status = 500
        os.remove(self.path(f"{X86}/survarium_skhid.exe"))
        self.assertEqual(subprocess.run([exe, "check", "quiet"], timeout=120).returncode, 1)
        self.assertIn("FAILED: x86/survarium_skhid.exe is missing or damaged and cannot be repaired now: "
                      "127.0.0.1 answered /api/client-files with error 500", self.log())
        # Outside game\binaries\x86 it does nothing.
        stray = os.path.join(self.root, "updater_skhid.exe")
        shutil.copy(exe, stray)
        self.assertEqual(subprocess.run([stray, "check", "quiet"], timeout=30).returncode, 1)
        self.assertEqual(os.listdir(self.root).count("updater_skhid.log"), 0)

    def test_a_bad_server_address_is_refused(self):
        self.write(f"{X86}/updater_skhid.url", b"http://example.com")   # plain HTTP to another host
        launcher = self.launcher()
        launcher.wait_for(lambda l: l.texts(), 30, "error text")
        self.assertIn('The update server address "http://example.com" (updater_skhid.url) is not valid',
                      launcher.texts()[0])
        self.assertEqual(self.site.requests, [])
        self.assertEqual(launcher.finish(), 1)


if __name__ == "__main__":
    unittest.main()
