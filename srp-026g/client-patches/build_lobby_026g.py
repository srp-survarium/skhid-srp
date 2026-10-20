#!/usr/bin/env python3
"""Build the lobby movie from ActionScript source: revision 24's tree and every lobby feature.

    python build_lobby_026g.py <revision 24's lobby_menu.swf> <new lobby_menu.swf> [--tools private/actionscript/tools.json]

The lobby movie is one file (`flash_movies/lobby_menu.swf`), and a resources.db
patch carrying it replaces the players' movie whole. It is built one way only,
from source: revision 24's movie rebuilds byte for byte from its exported and
reconstructed ActionScript (tools/actionscript), and every lobby change is a
source feature over that tree, in the order of ../lobby/lobby.json:
repository-authored classes (../lobby/src/SrpRooms.as, ...) and diffs of the
game's classes (../lobby/features/<feature>/). Nothing is patched in bytecode.

Only the classes the features touch are compiled. Each of them first
recompiles unchanged and must give revision 24's movie exactly; then every
method, trait and class no feature names is checked byte for byte against
revision 24's, and every non-script tag is revision 24's
(tools/actionscript/build_feature.py; docs/notes/lobby-source.md).

The input is `flash_movies/lobby_menu.swf` of `resources.db.patch_24` (revision
24, BASE_SHA256); the output is the movie for the next resources.db.patch_<n>
(`srp-game-data pack`). The result must be RESULT_SHA256, the movie last
built: a change of a source or a tool gives another, which the build refuses
until RESULT_SHA256 is set to it on purpose (--any-result builds it anyway,
for development). Requires a JDK, JPEXS FFDec 25.1.3 (its full distribution)
and Apache Flex 4.16.1, named in a tools.json (tools/actionscript/README.md).
The exported source of revision 24 is private (it is the game's code): it is
made from the input movie into --project once (FFDec), and reused while its
pinned hashes match. No game assets or code are stored in this repository.
"""
import argparse
import contextlib
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
TOOLS = REPO / "tools" / "actionscript"
LOBBY = HERE.parent / "lobby" / "lobby.json"
RECONSTRUCTION = TOOLS / "reconstruction" / "026g-rev24" / "manifest.json"
RELEASE = "026g-rev24"
sys.path.insert(0, str(TOOLS))

import build_feature  # noqa: E402
import movie_source  # noqa: E402

# lobby_menu.swf of resources.db.patch_24 (revision 24), which its source rebuilds exactly.
BASE_SHA256 = "3ca94f2aa3d56da554e25c3ce24e3c5e6620368942fab728a90a2528c8b18a60"
# Older movies a release might be handed by mistake, named in the refusal.
OLDER = {
    "12f0ee1dae4301d18405e956678ad1c9aa2c975ce2639c408722f5708548856b": "revision 23's (resources.db.patch_23)",
    "ab6f22d80d32fa4928403d19e3e054bd5316b2370e2550cefec0c9d22b959850": "the retail",
}
# The movie the source build makes. Set it anew, on purpose, when a feature,
# a source or a tool changes. bdd66ed5 (2026-10-20): the first build from
# source, of every lobby change main's bytecode driver had (its e626a01a:
# the rooms through #229 and #233/#234, #213 to #217, #232's player gear);
# every method decompiles as e626a01a's does but for compiler encodings
# (docs/notes/lobby-source.md); NOT yet checked in the game.
RESULT_SHA256 = "bdd66ed5a87c07f325ba177dc052bb81d2591b0222c55e1b875edf335c69ccad"


def project_for(movie, project, tools, report=print):
    """Revision 24's pinned export in `project`, made from `movie` if absent."""
    if not project.exists():
        report(f"Exporting revision 24's ActionScript into {project} (once; FFDec)")
        project.parent.mkdir(parents=True, exist_ok=True)
        pending = project.with_name(project.name + ".partial")
        if pending.exists():
            shutil.rmtree(pending)
        movie_source.init_project(movie, pending, tools, RELEASE)
        pending.rename(project)
    manifest = movie_source.verify_project(project)
    if manifest["movie_sha256"] != BASE_SHA256 or manifest.get("release") != RELEASE:
        raise ValueError(f"{project} is not revision 24's pinned export")
    return project


@contextlib.contextmanager
def kept(path):
    """A new work directory that stays, for review."""
    path.mkdir(parents=True, exist_ok=False)
    yield path


def build(movie, tools, project, work=None, jobs=4, report=print, any_result=False):
    """The lobby movie from source (bytes), and the build's report."""
    digest = hashlib.sha256(movie.read_bytes()).hexdigest()
    if digest in OLDER:
        raise ValueError(f"This is {OLDER[digest]} lobby movie; give revision 24's (resources.db.patch_24)")
    if digest != BASE_SHA256:
        raise ValueError(f"Not revision 24's lobby movie (sha256 {digest}): the source tree is that movie's")
    project = project_for(movie, project, tools, report)
    with (tempfile.TemporaryDirectory(prefix="srp-lobby-") if work is None else kept(work)) as folder:
        output = Path(folder) / "build"
        result = build_feature.build(tools, RECONSTRUCTION, project, LOBBY, output, jobs, log=report)
        built = (output / "feature.swf").read_bytes()
    if not any_result and result["feature_movie_sha256"] != RESULT_SHA256:
        raise ValueError(f"The movie is {result['feature_movie_sha256']}, not RESULT_SHA256 ({RESULT_SHA256}): "
                         "a feature, a source or a tool changed. Check the movie (in the game), then set "
                         "RESULT_SHA256 to it; --any-result builds it anyway")
    return built, result


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="lobby_menu.swf of resources.db.patch_24")
    parser.add_argument("output", type=Path, help="the new lobby_menu.swf, a new file")
    parser.add_argument("--tools", type=Path, default=REPO / "private" / "actionscript" / "tools.json",
                        help="JSON naming java, javac, ffdec (25.1.3) and flex (4.16.1); default %(default)s")
    parser.add_argument("--project", type=Path, default=REPO / "private" / "actionscript" / "rev24",
                        help="revision 24's pinned export (made from the source movie if absent); default %(default)s")
    parser.add_argument("--work", type=Path, help="a new directory to keep the build's reports and compiler output in")
    parser.add_argument("--jobs", type=int, default=4, help="concurrent baseline compilations (1-8)")
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
        movie, result = build(args.source.resolve(), args.tools.resolve(), args.project.resolve(),
                              args.work.resolve() if args.work else None, args.jobs, any_result=args.any_result)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(movie)
    print(f"Created {args.output}: the lobby movie from source ({len(movie)} bytes, sha256 "
          f"{hashlib.sha256(movie).hexdigest()}; {result['changed_methods']} methods changed and "
          f"{result['added_methods']} added over revision 24's, every other one byte for byte)")


if __name__ == "__main__":
    main()
