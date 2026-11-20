"""Live rename CLI: exact IDs, safe transport, refusal and persistence status."""
import argparse
import contextlib
import importlib.machinery
import importlib.util
import io
import os
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
