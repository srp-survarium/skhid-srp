#!/usr/bin/env python3
"""Compile, link and assemble the complete pinned ActionScript source tree."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

import movie_source as movie


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--reconstruction", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("project", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        parser.error("--jobs must be between 1 and 8")
    project, output = args.project.resolve(), args.output.resolve()
    movie.verify_project(project)
    output.mkdir(parents=True, exist_ok=False)
    common = ["--tools", str(args.tools.resolve())]
    compiled, linked, assembled = (output / name for name in ("compiled", "linked", "assembled"))
    result = subprocess.run([
        sys.executable, str(movie.HERE / "movie_source.py"), *common,
        "measure", str(project), str(compiled), "--compiler", "reconstruction",
        "--source-patches", str(args.reconstruction.resolve()), "--jobs", str(args.jobs), "--compact",
    ])
    # Different unlinked layouts are expected. Failed compilation is not.
    if result.returncode not in (0, 1) or not (compiled / "report.json").is_file():
        raise RuntimeError("Source compilation did not produce a complete report")
    report = json.loads((compiled / "report.json").read_text())
    if not report["complete"] or not report["classes"] or any(
            row["status"] not in ("exact", "different") for row in report["classes"]):
        raise RuntimeError("Some scripts failed to compile; inspect compiled/report.json")
    subprocess.run([
        sys.executable, str(movie.HERE / "link_layout.py"), *common,
        "--jobs", str(args.jobs), "--compact", str(project), str(compiled), str(linked),
    ], check=True)
    subprocess.run([
        sys.executable, str(movie.HERE / "assemble_baseline.py"), *common,
        str(project), str(compiled), str(linked), str(assembled),
    ], check=True)
    print(f"Verified whole-tree movie: {assembled / 'rebuilt.swf'}")


if __name__ == "__main__":
    main()
