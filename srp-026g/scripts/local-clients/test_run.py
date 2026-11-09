"""Local launcher regressions; no Wine, game files or network required."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest.mock import patch

import run

# The exe reader itself, for the test of the revision check; the other tests have no exe files.
READ_REVISION = run.exe_revision


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        revision = patch.object(run, "exe_revision", return_value=None)
        revision.start()
        self.addCleanup(revision.stop)
        self.path = self.root / "config.json"
        self.data = {
            "id": "12345678-1234-1234-1234-123456789abc",
            "game_dir": "game with spaces", "wine": "bin/wine", "wineserver": "bin/wineserver",
            "prefix_template": "template", "accounts": [
                {"name": "Alice", "password": "test $() password"},
                {"name": "Bob", "password": "test-password"}],
            "server": {"binary": "bin/srp", "working_dir": "server"},
        }
        self.config = self.load()

    def load(self):
        run.write_json(self.path, self.data)
        return run.Config.read(self.path)

    def test_credentials_are_literal_arguments_and_profiles_are_distinct(self):
        command = self.config.client_command(0)
        self.assertIn("-account_name=Alice", command)
        self.assertIn("-account_password=test $() password", command)
        self.assertNotEqual(self.config.prefix(0), self.config.prefix(1))
        self.assertEqual(self.config.client_environment(1)["WINEPREFIX"], str(self.config.prefix(1)))
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)

    def test_the_memory_profile_reaches_a_client_only_when_run_py_has_it(self):
        # patch 127's switch (docs/notes/memory-profile.md): off unless the one starting run.py sets it
        with patch.dict(run.os.environ, {}, clear=False):
            run.os.environ.pop("SKHID_MEMORY_PROFILE", None)
            self.assertNotIn("SKHID_MEMORY_PROFILE", self.config.client_environment(0))
        with patch.dict(run.os.environ, {"SKHID_MEMORY_PROFILE": "on"}):
            self.assertEqual(self.config.client_environment(0)["SKHID_MEMORY_PROFILE"], "on")

    def test_clients_start_directly_unless_a_virtual_desktop_is_asked_for(self):
        command = self.config.client_command(0)
        self.assertEqual(command[1], "survarium_skhid.exe")
        self.assertNotIn("explorer", command)
        self.data["virtual_desktop"] = True
        command = self.load().client_command(0)
        self.assertEqual(command[1:3], ["explorer", "/desktop=SRP-1,1280x720"])
        self.assertIn("survarium_skhid.exe", command)
        self.data["virtual_desktop"] = "yes"
        with self.assertRaises(ValueError):
            self.load()

    def test_invalid_accounts_ports_and_template_are_rejected_at_load(self):
        for changes in [
            {"accounts": []},
            {"accounts": [self.data["accounts"][0]] * 2},
            {"accounts": [{"name": "Alice", "password": "x" * 48}]},
            {"ports": [1234, 1234, 1236, 8088]},
            {"ports": [1234, 1235, 1236, "8088"]},
            {"prefix_template": "runtime/client-1"},
            {"prefix_template": "."},
        ]:
            with self.subTest(changes=changes):
                run.write_json(self.path, self.data | changes)
                with self.assertRaises(ValueError):
                    run.Config.read(self.path)

    def test_copied_documents_do_not_write_outside_the_new_profile(self):
        template = self.config.template
        user = template / "drive_c/users/tester"
        user.mkdir(parents=True)
        external = self.root / "original-documents"
        external.mkdir()
        (user / "Documents").symlink_to(external, target_is_directory=True)
        legacy = user / "My Documents/Survarium"
        legacy.mkdir(parents=True)
        cfg = legacy / "user.cfg"
        original = "  account_digest\tencrypted-old-value\n\taccount_name\tOld\nr_resolution 1920x1080\nsound_volume 0.5\n"
        cfg.write_text(original)
        with run.locked(self.config):
            run.prepare_prefix(self.config, 0)
        cloned = self.config.prefix(0) / "drive_c/users/tester"
        self.assertFalse((cloned / "Documents").is_symlink())
        self.assertEqual(list(external.iterdir()), [])
        self.assertEqual(cfg.read_text(), original)
        settings = (cloned / "My Documents/Survarium/user.cfg").read_text()
        self.assertNotIn("account_digest", settings)
        self.assertNotIn("account_name", settings)
        self.assertIn("r_resolution 1280x720", settings)
        self.assertIn("sound_volume 0.5", settings)
        self.assertIn("store_user_password on", settings)

    def test_unmanaged_prefix_is_neither_modified_nor_stopped(self):
        prefix = self.config.prefix(0)
        prefix.mkdir(parents=True)
        with self.assertRaises(ValueError):
            run.prepare_prefix(self.config, 0)
        with patch.object(run.subprocess, "run") as execute:
            with self.assertRaises(ValueError):
                run.stop(self.config, {"id": self.config.identity, "clients": {"0": {}}})
            execute.assert_not_called()

    def test_stop_cleans_state_after_user_already_closed_window(self):
        prefix = self.config.prefix(0)
        prefix.mkdir(parents=True)
        (prefix / ".srp-local-client").write_text(self.config.identity)
        state = {"id": self.config.identity, "clients": {"0": {"pid": 123, "started": "old"}}}
        with patch.object(run.subprocess, "run", side_effect=[
            subprocess.CompletedProcess([], 1), subprocess.CompletedProcess([], 0)
        ]) as execute:
            run.stop(self.config, state)
        self.assertEqual(state["clients"], {})
        self.assertEqual(execute.call_args_list[1].args[0][-1], "-w")

    def test_stale_pid_does_not_signal_an_unrelated_process(self):
        with patch.object(run, "process_start", return_value="different-start"), patch.object(run.os, "kill") as kill:
            run.stop(self.config, {"id": self.config.identity, "clients": {}, "server": {"pid": 123, "started": "old-start"}})
            kill.assert_not_called()

    def test_partial_launch_stops_its_first_client_and_retains_cleanup_state(self):
        state = {"id": self.config.identity, "clients": {}}
        with patch.object(run, "preflight"), patch.object(run, "prepare_prefix"), \
             patch.object(run.socket, "create_connection"), \
             patch.object(run, "launch", side_effect=[{"pid": 123, "started": "1"}, OSError("cannot launch")]), \
             patch.object(run, "stop") as stop, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(OSError):
                run.start(self.config, state, 2, True)
            self.assertEqual(state["clients"]["0"]["pid"], 123)
            self.assertIn("1", state["clients"])
            stop.assert_called_once_with(self.config, state)

    def test_loopback_and_normal_tutorial_override_inherited_environment(self):
        state = {"id": self.config.identity, "clients": {}}
        captured = []
        def launch(command, cwd, env, log):
            captured.append(env)
            return {"pid": 123, "started": "1"}
        with patch.dict(run.os.environ, {"SRP_TUTORIAL": "off", "SRP_LOGIN_BIND": "0.0.0.0"}), \
             patch.object(run, "preflight"), patch.object(run, "prepare_prefix"), \
             patch.object(run.socket, "create_connection"), patch.object(run, "alive", return_value=True), \
             patch.object(run, "launch", side_effect=launch), patch.object(run.time, "sleep"), \
             contextlib.redirect_stdout(io.StringIO()):
            run.start(self.config, state, 1, False)
        self.assertEqual(captured[0]["SRP_LOGIN_BIND"], "127.0.0.1")
        self.assertEqual(captured[0]["SRP_TUTORIAL"], "on")
        self.assertEqual(captured[0]["SRP_DATA_DIR"], str(self.root / "data"))

    def server_env(self):
        captured = []
        def launch(command, cwd, env, log):
            captured.append(env)
            return {"pid": 123, "started": "1"}
        with patch.object(run, "preflight"), patch.object(run, "prepare_prefix"), \
             patch.object(run.socket, "create_connection"), patch.object(run, "alive", return_value=True), \
             patch.object(run, "launch", side_effect=launch), patch.object(run.time, "sleep"), \
             contextlib.redirect_stdout(io.StringIO()):
            run.start(self.config, {"id": self.config.identity, "clients": {}}, 1, False)
        return captured[0]

    def test_matches_are_recorded_into_the_world_corpus_unless_the_config_says_off(self):
        env = self.server_env()
        self.assertEqual(env["SRP_RECORD_DIR"], str(run.WORLD_CORPUS / "local" / self.root.name))
        self.assertEqual(env["SRP_RECORD_MIN_PLAYERS"], "1")
        self.data["server"]["env"] = {"SRP_RECORD_DIR": "off"}
        self.config = self.load()
        self.assertEqual(self.server_env()["SRP_RECORD_DIR"], "off")

    def test_invalid_count_does_not_launch_anything(self):
        with patch.object(run, "launch") as launch:
            for count in [0, 3]:
                with self.assertRaises(ValueError):
                    run.start(self.config, {"id": self.config.identity, "clients": {}}, count, False)
            launch.assert_not_called()

    def exe(self, name, revision):
        """A minimal PE whose one section holds the revision site (None: the original's bytes)."""
        image_base, rva, raw = 0x10000, 0x1000, 0x200
        site = run.REVISION_ORIGINAL if revision is None else bytes([0xC6, 0x44, 0x24, 0x27, revision, 0x90])
        data = bytearray(raw + 0x400000)
        data[0x3C:0x40] = (0x80).to_bytes(4, "little")
        data[0x80:0x84] = b"PE\0\0"
        data[0x86:0x88] = (1).to_bytes(2, "little")
        data[0x94:0x96] = (0xE0).to_bytes(2, "little")
        data[0x98 + 28:0x98 + 32] = image_base.to_bytes(4, "little")
        section = 0x98 + 0xE0
        for offset, value in [(8, 0x400000), (12, rva), (16, 0x400000), (20, raw)]:
            data[section + offset:section + offset + 4] = value.to_bytes(4, "little")
        at = raw + run.REVISION_VA - image_base - rva
        data[at:at + 6] = site
        path = self.root / "game with spaces/binaries/x86" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    @patch.object(run, "exe_revision", READ_REVISION)
    def test_an_exe_older_than_the_server_takes_is_refused_before_anything_starts(self):
        self.data["server"]["env"] = {"SRP_MIN_PATCH_REVISION": "54"}
        self.data["accounts"][0]["executable"] = "new.exe"
        self.data["accounts"][1]["executable"] = "old.exe"
        self.exe("new.exe", 54)
        self.exe("old.exe", 51)
        config = self.load()
        self.assertEqual([revision for _, revision in run.check_revisions(config, 1)], [54])
        with self.assertRaisesRegex(ValueError, "Bob plays old.exe \\(patch revision 51\\)"):
            run.check_revisions(config, 2)
        self.exe("old.exe", None)
        with self.assertRaisesRegex(ValueError, "Bob plays old.exe"):
            run.check_revisions(config, 2)
        with patch.object(run, "preflight"), patch.object(run, "prepare_prefix"), \
                patch.object(run, "launch") as launch:
            with self.assertRaises(ValueError):
                run.start(config, {"id": config.identity, "clients": {}}, 2, False)
            launch.assert_not_called()
        # No least revision configured: anything starts, as the server takes anything.
        del self.data["server"]["env"]
        self.assertEqual([revision for _, revision in run.check_revisions(self.load(), 2)], [54, None])


if __name__ == "__main__":
    unittest.main()
