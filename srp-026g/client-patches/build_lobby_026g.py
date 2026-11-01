"""Build the lobby movie from its ActionScript source tree (srp-026g/flash/lobby_menu).

    python build_lobby_026g.py <revision 24's lobby_menu.swf> <new lobby_menu.swf> [--tools private/actionscript/tools.json]

The lobby movie is one file (`flash_movies/lobby_menu.swf`), and a resources.db
patch carrying it replaces the players' movie whole. It is built from source
only: srp-026g/flash/lobby_menu/src is the movie's complete ActionScript, with
the classes the build makes of its tables/ (the shop filters), and every lobby
change is an edit of them (docs/notes/lobby-source.md). The whole
tree is compiled in one Flex run, and the code goes into revision 24's movie in
place of its code (tools/actionscript/flash_tree.py build): the graphics,
fonts and symbol bindings are revision 24's. About 5 seconds.

The input is `flash_movies/lobby_menu.swf` of `resources.db.patch_24` (revision
24, BASE_SHA256). The output is the movie for the next resources.db.patch_<n>
(`srp-game-data pack`). The build says whether the movie is RESULT_SHA256, the
one last checked in the game (the build is deterministic: the same tree and
tools give the same bytes). Requires a JDK and Apache Flex 4.16.1, named in a
tools.json (tools/actionscript/README.md).
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

# lobby_menu.swf of resources.db.patch_24 (revision 24): the graphics the tree's code goes into.
BASE_SHA256 = "3ca94f2aa3d56da554e25c3ce24e3c5e6620368942fab728a90a2528c8b18a60"
# Older movies a release might be handed by mistake, named in the refusal.
OLDER = {
    "12f0ee1dae4301d18405e956678ad1c9aa2c975ce2639c408722f5708548856b": "revision 23's (resources.db.patch_23)",
    "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850": "the retail",
}
# The movie last checked in the game: the build says whether it made this one
# (a note, not a gate). Set it when the owner has checked a new one, with a
# line on what it holds. RESULT (2026-11-01): the tree released in revision 41
# (tag 026g-rev41, main 958eaa8: the lobby work since revision 40, #982 #983
# #984 #985 #999 #1003 #1004), one Flex run; the owner checked it in the game
# before the release.
RESULT_SHA256 = "fd4f38d47fc2ff3d1b36039682a9ab13868dc899dee66ce8097bfc9158a1f75e"


def build(movie, tools, report=print):
    """The lobby movie from source (bytes)."""
    digest = hashlib.sha256(movie.read_bytes()).hexdigest()
    if digest in OLDER:
        raise ValueError(f"This is {OLDER[digest]} lobby movie; give revision 24's (resources.db.patch_24)")
    if digest != BASE_SHA256:
        raise ValueError(f"Not revision 24's lobby movie (sha256 {digest}): the source tree's graphics are that movie's")
    return flash_tree.build(TREE, movie, tools, report)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="lobby_menu.swf of resources.db.patch_24")
    parser.add_argument("output", type=Path, help="the new lobby_menu.swf, a new file")
    parser.add_argument("--tools", type=Path, default=REPO / "private" / "actionscript" / "tools.json",
                        help="JSON naming java and flex (4.16.1); default %(default)s")
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error("Output already exists; choose a new path")
    if not args.tools.is_file():
        parser.error(f"No tools file {args.tools} (tools/actionscript/README.md)")
    try:
        movie = build(args.source.resolve(), args.tools.resolve())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    sha = hashlib.sha256(movie).hexdigest()
    checked = ("RESULT_SHA256, the movie last checked in the game" if sha == RESULT_SHA256 else
               "a new movie, not RESULT_SHA256: check it in the game, then set RESULT_SHA256 to it")
    print(f"Created {args.output}: the lobby movie from source ({len(movie)} bytes, sha256 {sha}): {checked}")


if __name__ == "__main__":
    main()
