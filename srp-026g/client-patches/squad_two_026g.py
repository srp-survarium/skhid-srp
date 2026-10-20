#!/usr/bin/env python3
"""Squads of two in the lobby: two places in the squad panel, no invite once full.

    python squad_two_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec <ffdec.jar> --step-only

The owner's of 2026-10-20: squads are back, of two players, and "there
shouldn't even be a button to accept a third person". The squad panel
(survarium.friends.SquadList) had three places, and "Invite to squad" was
offered until three were in. Now (squad_two/PatchSquadTwo.java):

- the panel has two places: with both taken there is no empty row to click
  for its player search;
- PlayerInfo.isSquadFull is set at two members, so the friends list's and the
  chat's right-click menus, which list "Invite to squad" only while it is
  false, list it for no squad of two;
- the panel's player search, if it was open when the second member came,
  invites nobody (it closes as before).

The 2 is the movie's own: the movie has no value of the server's squad size
(`SRP_SQUAD_SIZE`), and a server of squads of three would show two places.
An invite already shown to a player when the inviter's squad fills keeps its
Accept (the invite carries only the sender, no squad size): the server refuses
that acceptance (`SRP_SQUAD_FULL_TEXT`). Accept is off, as before, while the
invited player is in a squad.

A step of build_lobby_026g.py, which is how the movie is built. Three methods
of SquadList change; every other method is checked byte for byte, and no
non-script tag changes. Requires a JDK and JPEXS FFDec (25.1.3). No game assets
are stored in this repository.
"""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

import login_hint_026g as hint

# The movies the step was first built on, by the sha256 of the file (the
# driver passes the one before it instead).
MOVIE_SHA256S = {
    "88fa657a48250bd2729c875ff86f56c20a8acdc42c7ba06bd41fbf11161db1c9":
        "the steps of build_lobby_026g.py up to the room create window's saved settings",
}
RETAIL_SHA256 = "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850"
HERE = Path(__file__).resolve().parent / "squad_two"


def build_movie(original, ffdec, java, javac, work, known=True):
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
                    str(HERE / "PatchSquadTwo.java")], check=True)
    edited = work / "edited.swf"
    subprocess.run([java, "-Xmx4g", "-Djava.awt.headless=true", "-Duser.home=" + str(work),
                    "-cp", str(work) + os.pathsep + str(ffdec), "PatchSquadTwo",
                    str(code), str(edited)], check=True)
    replacements = [t for t in hint.unpack(edited.read_bytes())[2] if t[0] == 82]
    before = [t for t in tags if t[0] == 82]
    changed = sum(a != b for a, b in zip(before, replacements))
    if len(before) != len(replacements) or changed != 1:
        raise ValueError(f"Expected the squad panel's ABC block alone to change, got {changed}")
    replacement_iter = iter(replacements)
    result = hint.pack(version, header, [next(replacement_iter) if t[0] == 82 else t for t in tags])
    _, saved_header, saved_tags = hint.unpack(result)
    if saved_header != header or [t for t in tags if t[0] != 82] != [t for t in saved_tags if t[0] != 82]:
        raise ValueError("Non-script content changed")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="the lobby_menu.swf of the steps before this one")
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
    with tempfile.TemporaryDirectory(prefix="srp-squad-two-") as temporary:
        movie = build_movie(original, args.ffdec.resolve(), args.java, args.javac, Path(temporary),
                            known=not args.any_movie)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output}: lobby movie with squads of two ({len(movie)} bytes, "
          f"sha256 {hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
