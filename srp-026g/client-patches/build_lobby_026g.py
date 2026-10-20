#!/usr/bin/env python3
"""Build the lobby movie: every lobby change, in order, on revision 23's movie.

    python build_lobby_026g.py <revision 23's lobby_menu.swf> <new lobby_menu.swf> --ffdec <ffdec.jar>

The lobby movie is one file (`flash_movies/lobby_menu.swf`), and a resources.db
patch carrying it replaces the players' movie whole. Like patch_026g.py for the
exe, this always starts from the same released movie and applies every change
the repository holds, in order, so the movie that ships is the sum of them all:
nothing is in it because someone ran a step, nothing is missing because someone
did not. A change of a step's source (SrpRooms.as, …) is built in by building
again; a new lobby change is a new entry of STEPS.

The input is `flash_movies/lobby_menu.swf` of `resources.db.patch_23` (revision
23, BASE_SHA256); the output is the movie for the next resources.db.patch_<n>
(`srp-game-data pack`). Each step checks its own work (every method it does not
change byte for byte, no non-script tag changed). The result must be
RESULT_SHA256, the movie last checked: a change of a source or of FFDec gives
another, which the build refuses until RESULT_SHA256 is set to it on purpose
(--any-result builds it anyway, for development). Requires a JDK and JPEXS
FFDec 25.1.3. No game assets are stored in this repository.

Frozen in the base: the login hint (login_hint_026g.py) and the exchange's
first build (exchange_026g.py) are in revision 23's movie already; a change of
theirs needs a new base, not a step. The exchange's later changes are a step
(exchange_update_026g.py).
"""
import argparse
import contextlib
import hashlib
from pathlib import Path
import tempfile

import buy_gold_026g
import buy_modified_026g
import craft_menu_026g
import exchange_update_026g
import rooms_026g
import scope_context_menu_026g

# lobby_menu.swf of resources.db.patch_23 (revision 23): the lobby with the
# matches list, the exchange and the login hint, before the rooms.
BASE_SHA256 = "12f0ee1dae4301d18405e956678ad1c9aa2c975ce2639c408722f5708548856b"
RETAIL_SHA256 = "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850"
# The movie the steps make (FFDec 25.1.3). Set it anew, on purpose, when a
# step changes. 88fa657a (2026-10-20): the create window starts with the
# player's last room made by hand (the server's `S1;F`);
# NOT yet checked in the game. Before it, 57258f9e (#209,
# 2026-10-20): the rooms window tells a member
# waiting during its room's match why it cannot go in (`J;-1;<why>`); NOT yet
# checked in the game. Before it, a56cd0ed was checked in the game by the
# owner (2026-10-20): the rooms' Leave room / Leave queue (red, white text)
# and the yellow tab in a room or a queue (#211: the owner checked d7907536;
# then the review fixes: no double-click into Play, Leave on whatever the
# game says of Play).
RESULT_SHA256 = "88fa657a48250bd2729c875ff86f56c20a8acdc42c7ba06bd41fbf11161db1c9"
# Every lobby change, in the order they are built (each on the one before).
STEPS = [
    ("the rooms window (rooms/SrpRooms.as)", rooms_026g),
    ("Get Gold opens the exchange (#213)", buy_gold_026g),
    ("the exchange window's amounts (#214)", exchange_update_026g),
    ("Buy on owned items (#215)", buy_modified_026g),
    ("the craft window's menu and buttons (#216)", craft_menu_026g),
    ("Optics in a weapon's menu (#217)", scope_context_menu_026g),
]


def build(original, ffdec, java="java", javac="javac", work=None, report=print, any_result=False):
    """The movie of every step on `original` (revision 23's), and each step's
    sha256 on the way."""
    digest = hashlib.sha256(original).hexdigest()
    if digest == RETAIL_SHA256:
        raise ValueError("This is the retail lobby movie; give revision 23's (resources.db.patch_23)")
    if digest != BASE_SHA256:
        raise ValueError(f"Not revision 23's lobby movie (sha256 {digest}): the steps are built on that one")
    movie = original
    hashes = []
    scratch = contextlib.nullcontext(work) if work else tempfile.TemporaryDirectory(prefix="srp-lobby-")
    with scratch as temporary:
        root = Path(temporary)
        for number, (name, step) in enumerate(STEPS, 1):
            folder = root / f"step{number}"
            folder.mkdir(parents=True, exist_ok=True)
            # The steps' own lists of known inputs name the movies they were
            # first built on; here each takes the one before it.
            movie = step.build_movie(movie, ffdec, java, javac, folder, known=False)
            hashes.append(hashlib.sha256(movie).hexdigest())
            report(f"{number}. {name}: sha256 {hashes[-1]}")
    if not any_result and hashes[-1] != RESULT_SHA256:
        raise ValueError(f"The movie is {hashes[-1]}, not RESULT_SHA256 ({RESULT_SHA256}): a step's source or "
                         "FFDec changed. Check the movie (in the game), then set RESULT_SHA256 to it; "
                         "--any-result builds it anyway")
    return movie, hashes


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="lobby_menu.swf of resources.db.patch_23")
    parser.add_argument("output", type=Path, help="the new lobby_menu.swf, a new file")
    parser.add_argument("--ffdec", required=True, type=Path, help="path to ffdec.jar")
    parser.add_argument("--java", default="java")
    parser.add_argument("--javac", default="javac")
    parser.add_argument("--any-result", action="store_true",
                        help="write the movie even if it is not RESULT_SHA256 (development)")
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error("Output already exists; choose a new path")
    movie, _ = build(args.source.read_bytes(), args.ffdec.resolve(), args.java, args.javac,
                     any_result=args.any_result)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output}: the lobby movie of {len(STEPS)} steps ({len(movie)} bytes, "
          f"sha256 {hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
