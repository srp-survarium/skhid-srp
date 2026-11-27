"""Live rename CLI: exact IDs, safe transport, refusal and persistence status."""
import argparse
import contextlib
import importlib.machinery
import importlib.util
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

loader = importlib.machinery.SourceFileLoader("moderate", str(Path(__file__).with_name("moderate")))
spec = importlib.util.spec_from_loader(loader.name, loader)
moderate = importlib.util.module_from_spec(spec)
loader.exec_module(moderate)


class RenameTests(unittest.TestCase):
    def args(self, **changes):
        return argparse.Namespace(**dict(dict(account_id=777, nickname="Mamoru_777", reason="duplicate name",
                                             dry_run=False, json=False), **changes))

    def test_id_is_never_resolved_as_a_nickname(self):
        for text in ("777", "#777"):
            self.assertEqual(moderate.rename_id(text), 777)
        for text in ("Mamoru", "0", "-1", "4294967296", "777x"):
            with self.assertRaises(argparse.ArgumentTypeError):
                moderate.rename_id(text)

    def test_cli_dry_run_uses_no_ssh_and_exposes_no_token(self):
        out = io.StringIO()
        with patch.dict(os.environ, {"SKHID_SITE": "https://example.invalid", "SKHID_JUMP": "root@jump.invalid",
                                     "SRP_ADMIN_TOKEN": "secret-token", "SRP_ADMIN_PATH": "secret-path"}), \
                patch.object(moderate.sys, "argv", ["moderate", "rename", "#777", "Mamoru_777",
                                                   "--reason", "duplicate name", "--dry-run"]), \
                patch.object(moderate.subprocess, "run") as run, \
                contextlib.redirect_stdout(out):
            moderate.main()
        run.assert_not_called()
        self.assertIn("rename_id=777", out.getvalue())
        self.assertIn("rename_name=Mamoru_777", out.getvalue())
        self.assertNotIn("secret-token", out.getvalue())
        self.assertNotIn("secret-path", out.getvalue())

    def test_rename_posts_exact_id_and_unicode_using_existing_transport(self):
        args = self.args(nickname="Мамору_777")
        answer = {"answer": "Account #777 renamed: Mamoru → Мамору_777.", "what": "rename"}
        with patch.object(moderate, "admin", return_value=answer) as admin, \
                patch.object(moderate, "remote") as remote, contextlib.redirect_stdout(io.StringIO()):
            moderate.rename(args)
        remote.assert_not_called()
        fields = dict(admin.call_args.args[1])
        self.assertEqual(fields, {"tab": "rename", "rename_id": "777", "rename_name": "Мамору_777",
                                  "rename_reason": "duplicate name"})

    def test_refusal_and_unsaved_change_are_not_reported_as_success(self):
        for message, code in [
            ("That nickname or a Cyrillic/Latin lookalike is already taken.", 1),
            ("No such account ID.", 1),
            ("Account #777 renamed: Mamoru → Mamoru_777. Error: the change stands in memory only: disk full.", 2),
            ("Account #777 renamed: Mamoru → Mamoru_777. The database was busy: srp's saver writes it within seconds.", 2),
        ]:
            with patch.object(moderate, "admin", return_value={"answer": message}), \
                    contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit) as caught:
                moderate.rename(self.args())
            self.assertEqual(caught.exception.code, code)

    def test_empty_reason_does_not_send(self):
        with patch.object(moderate, "admin") as admin, self.assertRaises(SystemExit):
            moderate.rename(self.args(reason=" "))
        admin.assert_not_called()


class JoinLineTests(unittest.TestCase):
    def test_a_relayed_sign_in_is_the_clients_address(self):
        for route in ("", " via relay 203.0.113.8"):
            for line, kind in ((f"Sign-in: account=0x1f from 198.51.100.9:5000{route} session=0x2, sent", "joined"),
                               (f"Sign-in of bob from 198.51.100.9:5000{route} refused: TooManyFromAddress", "refused")):
                found = [(k, p.match(line)["addr"]) for k, _, p in moderate.JOIN_LINES if p.match(line)]
                self.assertEqual(found[0], (kind, "198.51.100.9:5000"))


class ResourcesTests(unittest.TestCase):
    """`moderate resources`: the match server's resources.log lines, the
    differing ones by default (resource_checks.rs writes the lines)."""

    LOG = "\n".join([
        "2026-11-26T17:00:01.123Z [INFO] player (account 7), match 0x1234, mounts 12 14 73: as the release",
        "2026-11-26T17:00:02.000Z [INFO] player (account 7), match 0x1234, level_01/day: 4 files as the release",
        "2026-11-26T17:00:03.000Z [INFO] two words (account 9), match 0x1235, mounts 12 14 200: not as the release: also 200",
        "2026-11-26T17:00:04.000Z [INFO] two words (account 9), match 0x1235, level_01/day: 1 of 4 files not as "
        "the release: textures/a.dds missing",
        "2026-11-26T17:00:05.000Z [INFO] two words (account 9), match 0x1235, level_01/day: answered 1 files, 4 asked",
        "not a line of the log",
    ])

    def run_resources(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with patch.object(moderate, "remote", return_value={"text": self.LOG}), \
                patch.object(moderate.sys, "argv", ["moderate", "resources", *argv]), \
                contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            moderate.main()
        return out.getvalue().splitlines(), err.getvalue()

    def test_the_differing_lines_by_default(self):
        lines, err = self.run_resources()
        self.assertEqual([line.split("  ")[1] for line in lines], ["two words (9)"] * 3)
        self.assertIn("also 200", lines[0])
        self.assertIn("3 of 5 lines differ", err)

    def test_all_lines_and_one_account(self):
        lines, _ = self.run_resources("--all", "--account", "7")
        self.assertEqual(len(lines), 2)
        self.assertTrue(all("player (7)" in line for line in lines))

    def test_local_log_file(self):
        with tempfile.NamedTemporaryFile(mode="w") as log:
            log.write(self.LOG)
            log.flush()
            with patch.object(moderate, "remote", side_effect=AssertionError("unexpected remote read")), \
                    patch.object(moderate.sys, "argv", ["moderate", "resources", "--log-file", log.name]), \
                    contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()):
                moderate.main()
        self.assertIn("textures/a.dds missing", out.getvalue())


if __name__ == "__main__":
    unittest.main()
