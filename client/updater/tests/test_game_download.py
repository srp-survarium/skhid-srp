"""Real updater download tests on Windows or Wine, with a loopback mirror.

Build ../build.sh first. On Linux set WINEPREFIX to a disposable prefix.
The extractor is the pinned 7zr.exe of ../extractor, put beside the updater
as the Skhid archive puts it. SKHID_TEST_GAME_ARCHIVE enables the tests that
need the pinned 2.9 GB game archive, the full installation among them; no
internet is needed.
"""
import hashlib
import http.server
import io
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import time
import unittest
import zipfile

UPDATER = Path(__file__).resolve().parents[1] / "exe/target/i686-win7-windows-msvc/release/updater_skhid.exe"
EXTRACTOR = Path(__file__).resolve().parents[1] / "extractor/7zr.exe"
EXTRACTOR_SIZE = 602624
GAME_SIZE = 2906544103
EXE_HASH = "ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153"


class GameDownloadTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="skhid-game-download-")
        self.addCleanup(self.temp.cleanup)
        self.game = Path(self.temp.name) / "game"
        self.x86 = self.game / "binaries/x86"
        self.x86.mkdir(parents=True)
        self.exe = self.x86 / "updater_skhid.exe"
        shutil.copyfile(UPDATER, self.exe)
        self.extractor = self.x86 / "7zr.exe"
        shutil.copyfile(EXTRACTOR, self.extractor)
        self.old = b"existing patched game; must survive a failed download"
        (self.x86 / "survarium_skhid.exe").write_bytes(self.old)
        self.overlay = b"verified Skhid overlay"
        bundle = io.BytesIO()
        with zipfile.ZipFile(bundle, "w", zipfile.ZIP_DEFLATED) as z:
            z.writestr("game/binaries/x86/survarium_skhid.exe", self.overlay)
        self.bundle = bundle.getvalue()
        # No patch of resources.db: an empty list.
        self.routes = {"/game": (200, b"<html>Sign in to Google</html>", {}),
                       "/patches": (200, b"{}", {})}
        self.requests = []
        test = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                test.requests.append(self.path)
                if self.path == "/api/client-files":
                    body = f"skhid-client-files 1\nsha256 {hashlib.sha256(test.bundle).hexdigest()}\nsize {len(test.bundle)}\n".encode()
                    status, headers = 200, {}
                elif self.path == "/download":
                    status, body, headers = 200, test.bundle, {}
                else:
                    status, body, headers = test.routes.get(self.path, (404, b"", {}))
                self.send_response(status)
                size = body.stat().st_size if isinstance(body, Path) else len(body)
                self.send_header("Content-Length", str(headers.get("Content-Length", size)))
                for key, value in headers.items():
                    if key != "Content-Length":
                        self.send_header(key, str(value))
                self.end_headers()
                try:
                    if isinstance(body, Path):
                        with body.open("rb") as f:
                            shutil.copyfileobj(f, self.wfile, 1 << 20)
                    else:
                        self.wfile.write(body)
                except (BrokenPipeError, ConnectionResetError):
                    pass

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.server.server_port}"
        (self.x86 / "updater_skhid.url").write_text(self.url)
        (self.x86 / "updater_skhid.game-url").write_text(self.url + "/game")

    def run_updater(self, timeout=60):
        command = [str(self.exe), "check", "quiet"]
        if os.name != "nt":
            command.insert(0, "wine")
        # Wine's background services can inherit pipes beyond updater exit.
        # The updater's own log is the diagnostic source on both platforms.
        result = subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=timeout)
        self.log = (self.x86 / "updater_skhid.log").read_text()
        return result.returncode

    def cache_archive(self):
        # The pinned archive as a finished download in the game folder.
        archive = Path(os.environ["SKHID_TEST_GAME_ARCHIVE"])
        cached = self.game / ".skhid-game.7z"
        try:
            os.link(archive, cached)
        except OSError:
            shutil.copyfile(archive, cached)
        return cached

    def assert_rejected(self, reason):
        self.assertEqual(self.run_updater(), 1, self.log)
        self.assertIn(reason, self.log)
        self.assertEqual((self.x86 / "survarium_skhid.exe").read_bytes(), self.old)
        self.assertFalse((self.x86 / "survarium.exe").exists())
        self.assertFalse((self.game / "resources.db").exists())
        self.assertFalse((self.game / ".skhid-game.7z").exists())
        self.assertNotIn("/download", self.requests)
        self.assertEqual(self.extractor.read_bytes(), EXTRACTOR.read_bytes())

    def test_html_response_is_not_extracted_or_installed(self):
        self.assert_rejected("SHA-256 mismatch")

    def test_truncated_download_is_deleted(self):
        self.routes["/game"] = (200, b"partial", {"Content-Length": GAME_SIZE})
        self.assert_rejected("download failed")

    def test_oversized_download_is_rejected_before_writing(self):
        self.routes["/game"] = (200, b"", {"Content-Length": GAME_SIZE + 1})
        self.assert_rejected("too large")

    def test_redirect_is_not_followed(self):
        self.routes["/game"] = (302, b"", {"Location": self.url + "/sign-in"})
        self.assert_rejected("error 302")
        self.assertNotIn("/sign-in", self.requests)

    def test_http_error_does_not_install(self):
        self.routes["/game"] = (403, b"quota exceeded", {})
        self.assert_rejected("error 403")

    def test_remote_plain_http_is_rejected(self):
        (self.x86 / "updater_skhid.game-url").write_text("http://example.com/game.7z")
        self.assert_rejected("URL is invalid")
        self.assertNotIn("/game", self.requests)

    def test_unverified_cached_archive_is_not_trusted(self):
        (self.game / ".skhid-game.7z").write_bytes(b"untrusted cache")
        self.assert_rejected("SHA-256 mismatch")
        self.assertIn("/game", self.requests)

    def test_existing_game_only_downloads_overlay(self):
        (self.game / "resources.db").write_bytes(b"existing resources")
        (self.x86 / "survarium.exe").write_bytes(b"existing original")
        self.assertEqual(self.run_updater(), 0, self.log)
        self.assertEqual((self.x86 / "survarium_skhid.exe").read_bytes(), self.overlay)
        self.assertNotIn("/game", self.requests)
        self.requests.clear()
        self.assertEqual(self.run_updater(), 0, self.log)
        # The overlay is up to date; the list of resources.db's patches is
        # asked for at every start.
        self.assertEqual(self.requests, ["/api/client-files", "/patches"])

    def test_a_changed_extractor_is_never_run_and_nothing_is_downloaded_for_it(self):
        # The same size, other bytes. The Skhid files are installed first, to
        # repair it; this server's archive has no extractor, so it stays as
        # it is, the updater says so, and the 2.9 GB are not asked for.
        self.extractor.write_bytes(bytes(EXTRACTOR_SIZE))
        self.assertEqual(self.run_updater(), 1, self.log)
        self.assertIn("7zr.exe beside the updater is missing or is not the file", self.log)
        self.assertNotIn("/game", self.requests)
        self.assertFalse((self.game / ".skhid-game.7z").exists())
        self.assertFalse(list(self.game.glob(".skhid-extract-*")))
        self.assertFalse((self.x86 / "survarium.exe").exists())
        self.assertEqual(self.extractor.read_bytes(), bytes(EXTRACTOR_SIZE))

    def test_a_missing_extractor_comes_with_the_skhid_files_before_the_game(self):
        # Deleted by hand or by an antivirus: the archive of the server
        # brings it back, and only then is the game asked for.
        self.extractor.unlink()
        bundle = io.BytesIO()
        with zipfile.ZipFile(bundle, "w", zipfile.ZIP_DEFLATED) as z:
            z.writestr("game/binaries/x86/survarium_skhid.exe", self.overlay)
            z.writestr("game/binaries/x86/7zr.exe", EXTRACTOR.read_bytes())
        self.bundle = bundle.getvalue()
        self.assertEqual(self.run_updater(), 1, self.log)
        self.assertEqual(self.extractor.read_bytes(), EXTRACTOR.read_bytes())
        self.assertEqual((self.x86 / "survarium_skhid.exe").read_bytes(), self.overlay)
        self.assertLess(self.requests.index("/download"), self.requests.index("/game"))
        # The game's mirror answered with a sign-in page: refused as ever.
        self.assertIn("SHA-256 mismatch", self.log)
        self.assertFalse((self.x86 / "survarium.exe").exists())

    def test_what_a_cut_short_attempt_left_is_removed(self):
        # The PC was switched off while the extractor ran: its folder with
        # the half-written game, and the download, are still there.
        left = self.game / ".skhid-extract-123456-0"
        (left / "Survarium/game/binaries/x86").mkdir(parents=True)
        (left / "Survarium/game/binaries/x86/survarium.exe").write_bytes(b"half an exe")
        (left / "Survarium/game/resources.db").write_bytes(b"half the resources")
        (self.game / ".skhid-game.7z").write_bytes(b"a download")
        # Not ours: a folder of that name with a file the game does not have.
        other = self.game / ".skhid-extract-mine"
        other.mkdir()
        (other / "notes.txt").write_text("keep")
        self.assertEqual(self.run_updater(), 1, self.log)  # the mirror: a sign-in page
        self.assertFalse(left.exists())
        self.assertEqual((other / "notes.txt").read_text(), "keep")
        self.assertFalse((self.game / ".skhid-game.7z").exists())

    def test_a_download_left_beside_a_complete_game_is_removed(self):
        (self.game / "resources.db").write_bytes(b"existing resources")
        (self.x86 / "survarium.exe").write_bytes(b"existing original")
        (self.game / ".skhid-game.7z").write_bytes(b"a download nobody needs any more")
        left = self.game / ".skhid-extract-123456-0"
        (left / "Survarium/game").mkdir(parents=True)
        (left / "Survarium/game/resources.db").write_bytes(b"half the resources")
        self.assertEqual(self.run_updater(), 0, self.log)
        self.assertFalse((self.game / ".skhid-game.7z").exists())
        self.assertFalse(left.exists())
        self.assertEqual((self.game / "resources.db").read_bytes(), b"existing resources")
        self.assertNotIn("/game", self.requests)

    @unittest.skipUnless(os.environ.get("SKHID_TEST_GAME_ARCHIVE"), "needs the full-download test environment")
    def test_same_size_corrupt_cached_archive_is_rehashed(self):
        # A sparse cache has the exact expected length but wrong contents.
        with (self.game / ".skhid-game.7z").open("wb") as f:
            f.truncate(GAME_SIZE)
        self.assert_rejected("SHA-256 mismatch")
        self.assertIn("/game", self.requests)

    @unittest.skipUnless(os.environ.get("SKHID_TEST_GAME_ARCHIVE"), "set SKHID_TEST_GAME_ARCHIVE for the full install")
    def test_full_verified_download_extract_and_overlay(self):
        self.routes["/game"] = (200, Path(os.environ["SKHID_TEST_GAME_ARCHIVE"]), {})
        self.assertEqual(self.run_updater(timeout=900), 0, self.log)
        self.assertEqual(hashlib.sha256((self.x86 / "survarium.exe").read_bytes()).hexdigest(), EXE_HASH)
        self.assertEqual((self.game / "resources.db").stat().st_size, 6285533906)
        self.assertEqual((self.x86 / "survarium_skhid.exe").read_bytes(), self.overlay)
        self.assertIn("/game", self.requests)
        self.assertFalse((self.game / ".skhid-game.7z").exists())
        # The extractor is a file of the Skhid archive: it stays.
        self.assertEqual(self.extractor.read_bytes(), EXTRACTOR.read_bytes())
        self.assertFalse(list(self.game.glob(".skhid-extract-*")))
        # What the extractor printed is in the log, and its file is gone.
        self.assertIn(" 7zr: Everything is Ok", self.log)
        self.assertIn("the extractor ended with exit 0", self.log)
        self.assertFalse((self.x86 / "updater_skhid.7zr.txt").exists())

    @unittest.skipUnless(os.environ.get("SKHID_TEST_GAME_ARCHIVE") and os.name == "nt",
                         "needs the pinned archive; has Windows refuse the extractor's files")
    def test_what_a_failed_extractor_printed_is_in_the_log(self):
        self.cache_archive()
        # New folders in the game folder may hold folders but no files: the
        # extractor makes its folders and is refused each of the five files.
        # `binaries` keeps the rights it has, so the updater's own files
        # beside it are written as ever.
        everyone = "*S-1-1-0"
        subprocess.run(["icacls", str(self.game / "binaries"), "/inheritance:d"],
                       check=True, capture_output=True)
        subprocess.run(["icacls", str(self.game), "/deny", everyone + ":(CI)(IO)(WD)"],
                       check=True, capture_output=True)
        self.addCleanup(subprocess.run, ["icacls", str(self.game), "/remove:d", everyone],
                        capture_output=True)
        self.assertEqual(self.run_updater(timeout=900), 1, self.log)
        self.assertIn("the extractor ended with exit 2", self.log)
        self.assertIn("The game extractor failed (exit 2)", self.log)
        refused = [line for line in self.log.splitlines()
                   if " 7zr: ERROR:" in line and line.endswith("survarium.exe")]
        self.assertTrue(refused, self.log)
        self.assertIn(" 7zr: Sub items Errors: 5", self.log)
        self.assertFalse((self.x86 / "updater_skhid.7zr.txt").exists())
        # Nothing was installed, nothing is left, the download is kept.
        self.assertFalse((self.x86 / "survarium.exe").exists())
        self.assertFalse((self.game / "resources.db").exists())
        self.assertEqual((self.x86 / "survarium_skhid.exe").read_bytes(), self.old)
        self.assertFalse(list(self.game.glob(".skhid-extract-*")))
        self.assertEqual((self.game / ".skhid-game.7z").stat().st_size, GAME_SIZE)
        self.assertNotIn("/game", self.requests)

    @unittest.skipUnless(os.environ.get("SKHID_TEST_GAME_ARCHIVE") and os.name == "nt",
                         "needs the pinned archive; kills the updater as Windows' task manager would")
    def test_an_updater_killed_while_extracting_leaves_nothing_for_good(self):
        cached = self.cache_archive()
        updater = subprocess.Popen([str(self.exe), "check", "quiet"],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.addCleanup(updater.kill)

        def extractors():
            # The processes that run this folder's 7zr.exe.
            listed = subprocess.run(
                ["powershell", "-NoProfile", "-Command",
                 "(Get-CimInstance Win32_Process -Filter \"Name='7zr.exe'\").ExecutablePath"],
                capture_output=True, text=True, timeout=60).stdout.splitlines()
            return [path for path in listed if path.strip().lower() == str(self.extractor).lower()]

        # The extractor is at work (7-Zip gives the 6.3 GB file its full
        # size at once, so its size says nothing).
        deadline = time.monotonic() + 180
        while not extractors() and time.monotonic() < deadline:
            if updater.poll() is not None:
                self.fail("the updater ended early: " + (self.x86 / "updater_skhid.log").read_text())
            time.sleep(0.5)
        self.assertTrue(extractors(), "the extractor never started")
        self.assertTrue(list(self.game.glob(".skhid-extract-*/Survarium/game/resources.db")))
        updater.kill()
        updater.wait(30)
        # The extractor goes with it.
        deadline = time.monotonic() + 15
        while extractors() and time.monotonic() < deadline:
            time.sleep(0.5)
        self.assertFalse(extractors(), "the extractor outlived the updater")
        self.assertFalse((self.x86 / "survarium.exe").exists())
        self.assertTrue((self.x86 / "updater_skhid.7zr.txt").exists())
        # The next start removes what was left, takes the archive that is
        # there (hashed again) and installs.
        self.assertEqual(self.run_updater(timeout=900), 0, self.log)
        self.assertIn("what the extractor printed in an attempt that was cut short", self.log)
        self.assertFalse((self.x86 / "updater_skhid.7zr.txt").exists())
        self.assertIn("removed a left-over extraction folder", self.log)
        self.assertIn("reusing verified download", self.log)
        self.assertNotIn("/game", self.requests)
        self.assertEqual(hashlib.sha256((self.x86 / "survarium.exe").read_bytes()).hexdigest(), EXE_HASH)
        self.assertEqual((self.game / "resources.db").stat().st_size, 6285533906)
        self.assertFalse(list(self.game.glob(".skhid-extract-*")))
        self.assertFalse(cached.exists())


if __name__ == "__main__":
    unittest.main()
