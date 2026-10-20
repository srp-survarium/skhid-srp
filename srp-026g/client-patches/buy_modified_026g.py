#!/usr/bin/env python3
"""The inventory's right-click menu offers Buy for a modified item too.

    python buy_modified_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec <ffdec.jar>

Right-click on an owned item listed Buy only for an item without
modifications (and not a bundle item): the menu tested the item's rank, which
an owned item's modifications raise. It now lists Buy for an owned item
whatever its rank; shop and bundle entries keep the old rule
(buy_modified/PatchBuyModified.java). Buying goes by the item's
dictionary id and the shop's price, so Buy on a modified item buys a stock
one, unmodified. A buy on an owned item is no longer asked "This item is
already purchased by you. Would you like to buy anyway?": only a buy from the
shop's list is.

The input is the live lobby movie as a file (or the newest one built for the
next patch); the output is a new movie for a new resources.db.patch_<n>
(`srp-game-data pack`). Two methods change, each instruction keeping its
length; every other method is checked byte for byte. Requires a JDK and JPEXS FFDec (tested with
25.1.3). No game assets are stored in this repository.
"""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

import login_hint_026g as hint

# The live movies the patch is built on, by the sha256 of the file.
MOVIE_SHA256S = {
    "f900cc8b369dfb5b3232d376fd04645fd12c5397c8f823895023453c28852609":
        "revision 24's with Get Gold and the exchange update (exchange_update_026g.py)",
}
RETAIL_SHA256 = "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850"
HERE = Path(__file__).resolve().parent / "buy_modified"


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
                    str(HERE / "PatchBuyModified.java")], check=True)
    edited = work / "edited.swf"
    subprocess.run([java, "-Xmx4g", "-Djava.awt.headless=true", "-Duser.home=" + str(work),
                    "-cp", str(work) + os.pathsep + str(ffdec), "PatchBuyModified",
                    str(code), str(edited)], check=True)
    replacements = [t for t in hint.unpack(edited.read_bytes())[2] if t[0] == 82]
    before = [t for t in tags if t[0] == 82]
    changed = sum(a != b for a, b in zip(before, replacements))
    # The inventory list's block and the lobby's (one if they share it).
    if len(before) != len(replacements) or not 1 <= changed <= 2:
        raise ValueError(f"Expected the inventory's and the lobby's ABC blocks to change, got {changed}")
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
    with tempfile.TemporaryDirectory(prefix="srp-buy-modified-") as temporary:
        movie = build_movie(original, args.ffdec.resolve(), args.java, args.javac, Path(temporary),
                            known=not args.any_movie)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output}: lobby movie with Buy for modified items ({len(movie)} bytes, "
          f"sha256 {hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
