#!/usr/bin/env python3
"""Add Optics to weapons' right-click menu in the live 0.26g lobby.

Input/output are extracted lobby_menu.swf files. The output belongs in a new
resources.db patch, not the original archive. Requires JDK and FFDec 25.1.3
(the hashes in weapon_scopes.md are its builds; 26.0.0 built the first version).
"""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

import login_hint_026g as swf

HERE = Path(__file__).resolve().parent / "scope_context_menu"
# Exact live movies: refuse older lobbies that lack the scope editor, and
# unknown movies until their context-menu layout has been checked. The newest
# of the lobby patches' chain (docs/notes/weapon_scopes.md section 5).
MOVIE_SHA256S = {
    "1091a19d809186d1db82e1c41744f64208d47cc04bb44a9555369c47da214918":
        "revision 24's with Get Gold, the exchange update, Buy on owned items and the craft window",
}


def transplant(original, edited):
    version, header, tags = swf.unpack(original)
    replacements = [t for t in swf.unpack(edited)[2] if t[0] == 82]
    before = [t for t in tags if t[0] == 82]
    if len(before) != len(replacements) or sum(a != b for a, b in zip(before, replacements)) != 1:
        raise ValueError("Expected exactly one changed ABC block (InventoryList)")
    replacement_iter = iter(replacements)
    result = swf.pack(version, header, [next(replacement_iter) if t[0] == 82 else t for t in tags])
    _, saved_header, saved_tags = swf.unpack(result)
    if header != saved_header or [t for t in tags if t[0] != 82] != [t for t in saved_tags if t[0] != 82]:
        raise ValueError("Non-script content changed")
    return result


def build_movie(original, ffdec, java, javac, work, known=True):
    if known and hashlib.sha256(original).hexdigest() not in MOVIE_SHA256S:
        raise ValueError("Not a known live lobby movie (or already patched)")
    version, header, tags = swf.unpack(original)
    code = work / "code.swf"
    # Never round-trip Scaleform graphics through FFDec.
    code.write_bytes(swf.pack(version, header, [t for t in tags if t[0] in (69, 82, 0)], False))
    subprocess.run([javac, "-encoding", "UTF-8", "-cp", str(ffdec), "-d", str(work),
                    str(HERE / "PatchScopeContextMenu.java")], check=True)
    edited = work / "edited.swf"
    subprocess.run([java, "-Xmx4g", "-Djava.awt.headless=true", "-Duser.home=" + str(work),
                    "-cp", str(work) + os.pathsep + str(ffdec), "PatchScopeContextMenu",
                    str(code), str(HERE / "SrpScopeContextMenu.as"), str(edited)], check=True)
    return transplant(original, edited.read_bytes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
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
    with tempfile.TemporaryDirectory(prefix="srp-scope-context-") as temporary:
        movie = build_movie(args.source.read_bytes(), args.ffdec.resolve(),
                            args.java, args.javac, Path(temporary))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output} (sha256 {hashlib.sha256(movie).hexdigest()})")


if __name__ == "__main__":
    main()
