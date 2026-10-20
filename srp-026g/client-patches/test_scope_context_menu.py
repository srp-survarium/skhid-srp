import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import login_hint_026g as swf
import scope_context_menu_026g as patch


def tag(kind, payload=b""):
    return kind, struct.pack("<HI", (kind << 6) | 63, len(payload)) + payload


def movie(tags):
    # Minimal RECT followed by frame rate and frame count.
    return swf.pack(10, b"\x08\x00\x00\x18\x01\x00", tags + [tag(0)])


class ScopeContextMenuTests(unittest.TestCase):
    def test_unknown_movie_fails_before_compiler(self):
        with mock.patch.object(patch.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "Not a known live lobby movie"):
                patch.build_movie(b"unknown", Path("unused"), "java", "javac", Path("unused"))
            run.assert_not_called()

    def test_output_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "lobby_menu.swf"
            output.write_bytes(b"keep")
            with mock.patch("sys.argv", ["scope_context_menu_026g.py", "unused", str(output), "--ffdec=unused", "--step-only"]):
                with self.assertRaises(SystemExit):
                    patch.main()
            self.assertEqual(output.read_bytes(), b"keep")

    def test_transplant_preserves_graphics_and_unrelated_abc(self):
        graphics = tag(87, b"Scaleform data FFDec must never rewrite")
        original = movie([tag(69, b"flags"), graphics, tag(82, b"inventory"), tag(82, b"scope editor")])
        edited = movie([tag(69, b"flags"), tag(82, b"new inventory"), tag(82, b"scope editor")])
        result = swf.unpack(patch.transplant(original, edited))
        self.assertEqual(result[:2], swf.unpack(original)[:2])
        self.assertEqual(result[2], [tag(69, b"flags"), graphics, tag(82, b"new inventory"),
                                     tag(82, b"scope editor"), tag(0)])

    def test_transplant_refuses_missing_extra_or_unexpected_changes(self):
        original = movie([tag(82, b"inventory"), tag(82, b"scope editor")])
        for blocks in ([b"inventory", b"scope editor"], [b"new inventory"],
                       [b"new inventory", b"changed editor"], [b"new inventory", b"scope editor", b"extra"]):
            with self.subTest(blocks=blocks), self.assertRaisesRegex(ValueError, "exactly one"):
                patch.transplant(original, movie([tag(82, b) for b in blocks]))


if __name__ == "__main__":
    unittest.main()
