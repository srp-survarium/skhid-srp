#!/usr/bin/env python3
"""Build the lobby movie from its ActionScript source tree (srp-026g/flash/lobby_menu).

    python build_lobby_026g.py <revision 24's lobby_menu.swf> <new lobby_menu.swf> [--tools private/actionscript/tools.json]

The lobby movie is one file (`flash_movies/lobby_menu.swf`), and a resources.db
patch carrying it replaces the players' movie whole. It is built one way only,
from source: srp-026g/flash/lobby_menu/src is the movie's complete ActionScript
(revision 24's, which rebuilds byte for byte: tools/actionscript/flash_tree.py
full), and every lobby change is an ordinary edit of that tree: of the game's
classes, and new classes only for new UI (the rooms window, survarium/rooms;
srp.json names them).
Nothing is patched in bytecode.

Only the files that are not the baseline's are compiled, each whole; the others
keep revision 24's compiled code, and the graphics are revision 24's
(tools/actionscript/build_feature.py; docs/notes/lobby-source.md).

The input is `flash_movies/lobby_menu.swf` of `resources.db.patch_24` (revision
24, BASE_SHA256): its graphics are not source. The output is the movie for the
next resources.db.patch_<n> (`srp-game-data pack`). The result must be
RESULT_SHA256, the movie last built: a change of the tree or a tool gives
another, which the build refuses until RESULT_SHA256 is set to it on purpose
(--any-result builds it anyway, for development). Requires a JDK, JPEXS FFDec
25.1.3 (its full distribution) and Apache Flex 4.16.1, named in a tools.json
(tools/actionscript/README.md).
"""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
TREE = HERE.parent / "flash" / "lobby_menu"
sys.path.insert(0, str(REPO / "tools" / "actionscript"))

import flash_tree  # noqa: E402

# lobby_menu.swf of resources.db.patch_24 (revision 24), which its source rebuilds exactly.
BASE_SHA256 = "3ca94f2aa3d56da554e25c3ce24e3c5e6620368942fab728a90a2528c8b18a60"
# Older movies a release might be handed by mistake, named in the refusal.
OLDER = {
    "12f0ee1dae4301d18405e956678ad1c9aa2c975ce2639c408722f5708548856b": "revision 23's (resources.db.patch_23)",
    "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850": "the retail",
}
# The movie the source build makes. Set it anew, on purpose, when the tree
# or a tool changes. RESULT (2026-10-21): the tree of cbeb0f89 (the lobby's
# features in the game's classes, the rooms in survarium/rooms, right-click
# menus owned by their window, the craft and upgrade windows' own menus),
# every changed file compiled whole; not yet checked in the game.
RESULT_SHA256 = "a1aee55d14621b91cd9d866b14b4cdb0ece47143cf928d1ab0508c5bbfb253a9"


def build(movie, tools, work=None, jobs=4, report=print, any_result=False):
    """The lobby movie from source (bytes), and the build's report."""
    digest = hashlib.sha256(movie.read_bytes()).hexdigest()
    if digest in OLDER:
        raise ValueError(f"This is {OLDER[digest]} lobby movie; give revision 24's (resources.db.patch_24)")
    if digest != BASE_SHA256:
        raise ValueError(f"Not revision 24's lobby movie (sha256 {digest}): the source tree is that movie's")
    built, result = flash_tree.build(TREE, movie, tools, work, jobs, report)
    sha = hashlib.sha256(built).hexdigest()
    if not any_result and sha != RESULT_SHA256:
        raise ValueError(f"The movie is {sha}, not RESULT_SHA256 ({RESULT_SHA256}): the tree or a tool "
                         "changed. Check the movie (in the game), then set RESULT_SHA256 to it; "
                         "--any-result builds it anyway")
    return built, result


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="lobby_menu.swf of resources.db.patch_24")
    parser.add_argument("output", type=Path, help="the new lobby_menu.swf, a new file")
    parser.add_argument("--tools", type=Path, default=REPO / "private" / "actionscript" / "tools.json",
                        help="JSON naming java, javac, ffdec (25.1.3) and flex (4.16.1); default %(default)s")
    parser.add_argument("--work", type=Path, help="a new directory to keep the build's reports and compiler output in")
    parser.add_argument("--jobs", type=int, default=4, help="ABC blocks compiled at once (1-8)")
    parser.add_argument("--any-result", action="store_true",
                        help="write the movie even if it is not RESULT_SHA256 (development)")
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error("Output already exists; choose a new path")
    if not args.tools.is_file():
        parser.error(f"No tools file {args.tools} (tools/actionscript/README.md)")
    if not 1 <= args.jobs <= 8:
        parser.error("--jobs must be between 1 and 8")
    try:
        movie, _ = build(args.source.resolve(), args.tools.resolve(),
                         args.work.resolve() if args.work else None, args.jobs, any_result=args.any_result)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output}: the lobby movie from source ({len(movie)} bytes, sha256 "
          f"{hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
