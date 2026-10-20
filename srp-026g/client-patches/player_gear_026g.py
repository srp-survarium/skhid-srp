#!/usr/bin/env python3
"""Show player's information: a right-click on a row of the Rankings window.

    python player_gear_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec <ffdec.jar> --step-only

The owner's of 2026-10-20: "you should be able to go to rankings and rmb to do
the same", "for every single player". A right-click on any row of the Rankings
window (either tab, any page, the player's own place too) opens the game's
right-click menu with its own "Show player's information"
(`show_player_info_button_hint`); it asks as the results screen's double-click
on a row does (`victory_screen_request_player_data`, `3D`), with a player no
results row has (`GEAR_BY_PLACE`, as srp's History asks for a replay), and the
answer (`93`) opens the game's own window ("player_info") with that player's
kits (player_gear/SrpPlayerGear.as). The server answers from the account as it
is now (lobby-server `gear.rs`; docs/notes/player_gear.md). `SrpPlayerGear.menu`
is public: other lists of the movie can open the same menu.

Two methods change and a class is added (player_gear/PatchPlayerGear.java):
`LobbyMenu.fill_victory_screen_player` hands the answer to an ask of ours to the
helper first, and `PlayerRanking.onActivate` ends with a call to it; every other
method is checked byte for byte. The input is the movie of the steps before it
(build_lobby_026g.py); the output is a new movie for a new
resources.db.patch_<n> (`srp-game-data pack`). Requires a JDK and JPEXS FFDec
(tested with 25.1.3). No game assets are stored in this repository.
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

# The live movies the patch is built on, by the sha256 of the file.
MOVIE_SHA256S = {
    "88fa657a48250bd2729c875ff86f56c20a8acdc42c7ba06bd41fbf11161db1c9":
        "build_lobby_026g.py's of 2026-10-20 (the rooms' remembered create window and the steps after it)",
}
RETAIL_SHA256 = "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850"
HERE = Path(__file__).resolve().parent / "player_gear"


def check_source(source):
    """FFDec's compiler takes two methods of one name and keeps the last: a
    call to the first then runs the second. Refuse that."""
    names = collections.Counter(re.findall(r"\bfunction\s+(?!get\b|set\b)(\w+)\s*\(", source))
    twice = sorted(name for name, count in names.items() if count > 1)
    if twice:
        raise ValueError("SrpPlayerGear.as defines these more than once: " + ", ".join(twice))


def build_movie(original, ffdec, java, javac, work, known=True):
    check_source((HERE / "SrpPlayerGear.as").read_text(encoding="utf-8"))
    digest = hashlib.sha256(original).hexdigest()
    if digest == RETAIL_SHA256:
        raise ValueError("This is the retail lobby movie; use the live one")
    if known and digest not in MOVIE_SHA256S:
        raise ValueError(f"Not a known live lobby movie (sha256 {digest}); --any-movie to try it")
    version, header, tags = hint.unpack(original)
    # Keep Scaleform graphics out of the editor: transplant only the edited ABC.
    code = work / "code.swf"
    code.write_bytes(hint.pack(version, header, [t for t in tags if t[0] in (69, 82, 0)], False))
    subprocess.run([javac, "-encoding", "UTF-8", "-cp", str(ffdec), "-d", str(work),
                    str(HERE / "PatchPlayerGear.java")], check=True)
    edited = work / "edited.swf"
    subprocess.run([java, "-Xmx4g", "-Djava.awt.headless=true", "-Duser.home=" + str(work),
                    "-cp", str(work) + os.pathsep + str(ffdec), "PatchPlayerGear",
                    str(code), str(HERE / "SrpPlayerGear.as"), str(edited)], check=True)
    replacements = [t for t in hint.unpack(edited.read_bytes())[2] if t[0] == 82]
    before = [t for t in tags if t[0] == 82]
    changed = sum(a != b for a, b in zip(before, replacements))
    # LobbyMenu's block and the ranking page's (one if they share it).
    if len(before) != len(replacements) or not 1 <= changed <= 2:
        raise ValueError(f"Expected LobbyMenu's and the ranking page's ABC blocks to change, got {changed}")
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
    with tempfile.TemporaryDirectory(prefix="srp-player-gear-") as temporary:
        movie = build_movie(original, args.ffdec.resolve(), args.java, args.javac, Path(temporary),
                            known=not args.any_movie)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output}: lobby movie with Show player's information in the Rankings ({len(movie)} bytes, "
          f"sha256 {hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
