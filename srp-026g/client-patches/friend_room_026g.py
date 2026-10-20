#!/usr/bin/env python3
"""Join room in a friend's right-click menu (the owner's of 2026-10-20).

    python friend_room_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec <ffdec.jar> --step-only

"It should also be possible to rmb on friend and join his room, if he is in a
room." FriendListRender.showContextMenu shows a friend's menu as before, then
calls SrpFriendRoom.opened (friend_room/PatchFriendRoom.java: four
instructions after its `SrvContextMenu.show(items)`, no jump added). The
helper asks the lobby which friends are members of a room (`S1;Y`, lobby.md
2.16.3) and, when this one is, adds Join room (Queue for room for a full one)
to the open menu; it joins through SrpRooms.joinFriend, as the Rooms list's
Join does. One method changes and a class is added; every other method of
every block is checked byte for byte.

A step of build_lobby_026g.py, after the rooms window (it calls into SrpRooms).
Requires a JDK and JPEXS FFDec 25.1.3. No game assets are stored in this
repository.
"""
import argparse
import collections
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tempfile

import login_hint_026g as swf

HERE = Path(__file__).resolve().parent / "friend_room"
# The movie this step was first built on: every step before it
# (build_lobby_026g.py's RESULT_SHA256 before this one).
MOVIE_SHA256S = {
    "83894684d06e07d3dcdf43532a88c8799aab173f05b37067a6d4437c8d1125d9":
        "revision 23's with the rooms window, Get Gold, the exchange update, Buy on owned items, "
        "the craft window and Optics",
}


def check_source(source):
    """FFDec's compiler takes two methods of one name and keeps the last."""
    names = collections.Counter(re.findall(r"\bfunction\s+(?!get\b|set\b)(\w+)\s*\(", source))
    twice = sorted(name for name, count in names.items() if count > 1)
    if twice:
        raise ValueError("SrpFriendRoom.as defines these more than once: " + ", ".join(twice))


def transplant(original, edited):
    version, header, tags = swf.unpack(original)
    replacements = [t for t in swf.unpack(edited)[2] if t[0] == 82]
    before = [t for t in tags if t[0] == 82]
    if len(before) != len(replacements) or sum(a != b for a, b in zip(before, replacements)) != 1:
        raise ValueError("Expected exactly one changed ABC block (FriendListRender's)")
    replacement_iter = iter(replacements)
    result = swf.pack(version, header, [next(replacement_iter) if t[0] == 82 else t for t in tags])
    _, saved_header, saved_tags = swf.unpack(result)
    if header != saved_header or [t for t in tags if t[0] != 82] != [t for t in saved_tags if t[0] != 82]:
        raise ValueError("Non-script content changed")
    return result


def build_movie(original, ffdec, java, javac, work, known=True):
    check_source((HERE / "SrpFriendRoom.as").read_text(encoding="utf-8"))
    if known and hashlib.sha256(original).hexdigest() not in MOVIE_SHA256S:
        raise ValueError("Not a known lobby movie (or already patched)")
    version, header, tags = swf.unpack(original)
    code = work / "code.swf"
    # Never round-trip Scaleform graphics through FFDec.
    code.write_bytes(swf.pack(version, header, [t for t in tags if t[0] in (69, 82, 0)], False))
    subprocess.run([javac, "-encoding", "UTF-8", "-cp", str(ffdec), "-d", str(work),
                    str(HERE / "PatchFriendRoom.java")], check=True)
    edited = work / "edited.swf"
    subprocess.run([java, "-Xmx4g", "-Djava.awt.headless=true", "-Duser.home=" + str(work),
                    "-cp", str(work) + os.pathsep + str(ffdec), "PatchFriendRoom",
                    str(code), str(HERE / "SrpFriendRoom.as"), str(edited)], check=True)
    return transplant(original, edited.read_bytes())


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--ffdec", required=True, type=Path)
    parser.add_argument("--java", default="java")
    parser.add_argument("--javac", default="javac")
    parser.add_argument("--step-only", action="store_true",
                        help="write this one step's movie (development); a release ships build_lobby_026g.py's movie only")
    args = parser.parse_args()
    if not args.step_only:
        parser.error("this is one step of the lobby movie; a movie of it alone lacks the other steps. Build the movie with build_lobby_026g.py (or pass --step-only for a development build)")
    if args.output.exists() or args.output.is_symlink():
        parser.error("Output already exists; choose a new path")
    with tempfile.TemporaryDirectory(prefix="srp-friend-room-") as temporary:
        movie = build_movie(args.source.read_bytes(), args.ffdec.resolve(),
                            args.java, args.javac, Path(temporary))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output} (sha256 {hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
