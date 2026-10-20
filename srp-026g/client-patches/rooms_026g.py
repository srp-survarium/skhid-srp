#!/usr/bin/env python3
"""Add rooms (the players' "servers") to the live 0.26g lobby movie.

    python rooms_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec <ffdec.jar>

The top menu's Map tab (which the game leaves off) becomes "Servers" and opens
one window over the lobby: the list of the rooms, the making of one, and the player's own room
with its countdown and map vote (rooms/SrpRooms.as). The server side and the
wire are srp's (servers/lobby-server/src/v026g/rooms.rs, docs/notes/lobby.md
2.16.3); with SRP_ROOMS off the tab says so and nothing else changes.

The input is the live lobby movie as a file (from the resources.db patch that
carries it: `flash_movies/lobby_menu.swf`), never the retail one: the rooms tab
sits after the Matches tab, which only the live movie has. The output is a new
compressed movie, of any size: it goes into a new resources.db.patch_<n>
(`srp-game-data pack`), not over the old entry. Only the ABC blocks of the
five classes hooked change (rooms/PatchRooms.java checks every other method
byte for byte). Requires a JDK and JPEXS FFDec (tested with 25.1.3). No game
assets are stored in this repository.
"""
import argparse
import collections
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tempfile

import login_hint_026g as hint

# The live movies the rooms are built on: lobby_menu.swf as the patches carry
# it, by the sha256 of the file.
MOVIE_SHA256S = {
    "12f0ee1dae4301d18405e956678ad1c9aa2c975ce2639c408722f5708548856b": "resources.db.patch_23 (revision 23)",
}
RETAIL_SHA256 = "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850"
HERE = Path(__file__).resolve().parent / "rooms"
# Classes whose blocks may change: the bottom panel's (SrpRooms goes in it),
# the ranking page's, the lobby's, the status panel's and the top menu's.
HOOKED = 5


def check_source(source):
    """FFDec's compiler takes two methods of one name and keeps the last: a
    call to the first then runs the second. Refuse that."""
    names = collections.Counter(re.findall(r"\bfunction\s+(?!get\b|set\b)(\w+)\s*\(", source))
    twice = sorted(name for name, count in names.items() if count > 1)
    if twice:
        raise ValueError("SrpRooms.as defines these more than once: " + ", ".join(twice))


def build_movie(original, ffdec, java, javac, work, known=True):
    check_source((HERE / "SrpRooms.as").read_text(encoding="utf-8"))
    digest = hashlib.sha256(original).hexdigest()
    if digest == RETAIL_SHA256:
        raise ValueError("This is the retail lobby movie; use the live one (with the Matches tab)")
    if known and digest not in MOVIE_SHA256S:
        raise ValueError(f"Not a known live lobby movie (sha256 {digest}); --any-movie to try it")
    version, header, tags = hint.unpack(original)
    # Keep Scaleform graphics out of the editor: transplant only the edited ABC.
    code = work / "code.swf"
    code.write_bytes(hint.pack(version, header, [t for t in tags if t[0] in (69, 82, 0)], False))
    subprocess.run([javac, "-encoding", "UTF-8", "-cp", str(ffdec), "-d", str(work),
                    str(HERE / "PatchRooms.java")], check=True)
    edited = work / "edited.swf"
    subprocess.run([java, "-Xmx4g", "-Djava.awt.headless=true", "-Duser.home=" + str(work),
                    "-cp", str(work) + os.pathsep + str(ffdec), "PatchRooms",
                    str(code), str(HERE / "SrpRooms.as"), str(edited)], check=True)
    replacements = [t for t in hint.unpack(edited.read_bytes())[2] if t[0] == 82]
    before = [t for t in tags if t[0] == 82]
    changed = sum(a != b for a, b in zip(before, replacements))
    if len(before) != len(replacements) or not 1 <= changed <= HOOKED:
        raise ValueError(f"Expected 1 to {HOOKED} changed ABC blocks, got {changed}")
    replacement_iter = iter(replacements)
    updated = [next(replacement_iter) if t[0] == 82 else t for t in tags]
    result = hint.pack(version, header, updated)
    _, saved_header, saved_tags = hint.unpack(result)
    if saved_header != header or [t for t in tags if t[0] != 82] != [t for t in saved_tags if t[0] != 82]:
        raise ValueError("Non-script content changed")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="the live lobby_menu.swf")
    parser.add_argument("output", type=Path, help="the new lobby_menu.swf, a new file")
    parser.add_argument("--ffdec", required=True, type=Path, help="path to ffdec.jar")
    parser.add_argument("--java", default="java")
    parser.add_argument("--javac", default="javac")
    parser.add_argument("--any-movie", action="store_true",
                        help="build on a lobby movie whose hash is not listed (development)")
    parser.add_argument("--step-only", action="store_true",
                        help="write this one step's movie (development); a release ships build_lobby_026g.py's movie only")
    args = parser.parse_args()
    if not args.step_only:
        parser.error("this is one step of the lobby movie; a movie of it alone lacks the other steps. Build the movie with build_lobby_026g.py (or pass --step-only for a development build)")
    if args.output.exists() or args.output.is_symlink():
        parser.error("Output already exists; choose a new path")
    original = args.source.read_bytes()
    with tempfile.TemporaryDirectory(prefix="srp-rooms-") as temporary:
        movie = build_movie(original, args.ffdec.resolve(), args.java, args.javac, Path(temporary),
                            known=not args.any_movie)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output}: lobby movie with rooms ({len(movie)} bytes, "
          f"sha256 {hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
