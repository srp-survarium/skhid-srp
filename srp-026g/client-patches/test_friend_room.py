import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import friend_room_026g as patch
import login_hint_026g as swf


def tag(kind, payload=b""):
    return kind, struct.pack("<HI", (kind << 6) | 63, len(payload)) + payload


def movie(tags):
    # Minimal RECT followed by frame rate and frame count.
    return swf.pack(10, b"\x08\x00\x00\x18\x01\x00", tags + [tag(0)])


class FriendRoomTests(unittest.TestCase):
    def test_unknown_movie_fails_before_compiler(self):
        with mock.patch.object(patch.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "Not a known lobby movie"):
                patch.build_movie(b"unknown", Path("unused"), "java", "javac", Path("unused"))
            run.assert_not_called()

    def test_the_source_defines_each_method_once(self):
        patch.check_source((patch.HERE / "SrpFriendRoom.as").read_text(encoding="utf-8"))
        with self.assertRaisesRegex(ValueError, "join"):
            patch.check_source("function join():void {}\nprivate static function join():void {}")
        # A getter and a setter of one name are not two methods.
        patch.check_source("function get x():int {}\nfunction set x(v:int):void {}")

    def test_output_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "lobby_menu.swf"
            output.write_bytes(b"keep")
            with mock.patch("sys.argv", ["friend_room_026g.py", "unused", str(output), "--ffdec=unused",
                                         "--step-only"]):
                with self.assertRaises(SystemExit):
                    patch.main()
            self.assertEqual(output.read_bytes(), b"keep")

    def test_transplant_preserves_graphics_and_unrelated_abc(self):
        graphics = tag(87, b"Scaleform data FFDec must never rewrite")
        original = movie([tag(69, b"flags"), graphics, tag(82, b"friends"), tag(82, b"rooms")])
        edited = movie([tag(69, b"flags"), tag(82, b"new friends"), tag(82, b"rooms")])
        result = swf.unpack(patch.transplant(original, edited))
        self.assertEqual(result[:2], swf.unpack(original)[:2])
        self.assertEqual(result[2], [tag(69, b"flags"), graphics, tag(82, b"new friends"),
                                     tag(82, b"rooms"), tag(0)])

    def test_transplant_refuses_missing_extra_or_unexpected_changes(self):
        original = movie([tag(82, b"friends"), tag(82, b"rooms")])
        for blocks in ([b"friends", b"rooms"], [b"new friends"],
                       [b"new friends", b"changed rooms"], [b"new friends", b"rooms", b"extra"]):
            with self.subTest(blocks=blocks), self.assertRaisesRegex(ValueError, "exactly one"):
                patch.transplant(original, movie([tag(82, b) for b in blocks]))


if __name__ == "__main__":
    unittest.main()
