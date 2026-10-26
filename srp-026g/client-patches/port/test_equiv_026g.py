"""Exercise both build subprocesses without needing a game exe or Unicorn."""
import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from port import equiv_026g as E


class BuildPaths(unittest.TestCase):
    def test_gate_builds_current_client_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            client, old = root / "client-patches", root / "old-client-patches"
            port = client / "port"
            (port / "equiv").mkdir(parents=True)
            old.mkdir()
            original = root / "original.exe"
            original.write_bytes(b"original")
            # Deliberately no patch_026g in port/: each real subprocess
            # must import the patcher beside the source it is building.
            for folder, marker in ((client, b"new"), (old, b"old")):
                (folder / "patch_026g.py").write_text(
                    "from pathlib import Path\n"
                    "from types import SimpleNamespace\n"
                    "def default_bin(mode): return Path(__file__)\n"
                    "def late_join_bin(): return Path(__file__)\n"
                    f"def build(*args): return {marker!r}, ''\n"
                    "def arena(): return SimpleNamespace(blocks={})\n")
            (port / "equiv" / "fixture.py").write_text(
                "def ranges(*args): return []\n"
                "def entries(*args): return []\n"
                "def cases(*args): return []\n")
            with (patch.object(E, "HERE", port), patch.object(E, "CLIENT", client),
                  patch.object(E, "ARENA_SCRATCH", root), patch.object(E, "old_tree", return_value=old),
                  patch.object(E, "Machine") as machine, patch.object(E, "blocks_of", return_value=set()),
                  patch.object(E.sys, "argv", [str(E.__file__), "fixture", str(original)]),
                  patch.object(E.sys, "path", list(E.sys.path)), patch.dict(E.sys.modules),
                  contextlib.redirect_stdout(io.StringIO())):
                E.main()
            self.assertEqual([call.args[0] for call in machine.call_args_list], [b"old", b"new"])


if __name__ == "__main__":
    unittest.main()
