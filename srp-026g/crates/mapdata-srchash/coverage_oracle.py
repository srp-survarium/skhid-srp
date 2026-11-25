#!/usr/bin/env python3
"""The srchash coverage oracle: every mapdata function a build executes in a digest's part is in that digest's closure.

A key narrower than the code its part runs would let a changed function go unseen and a stale grid be read as current. This
check compares what really ran (LLVM coverage of a `--cfg srp_coverage -C instrument-coverage` build, `mapdata/src/coverage.rs`)
with what `mapdata-srchash` puts in each digest (`--spans`).

    coverage_oracle.py --spans <mapdata-srchash <tree> --spans output> --profiles <SRP_COVERAGE_DIR> --binary <instrumented exe>
                       [--llvm-profdata llvm-profdata] [--llvm-cov llvm-cov] [--out <report>]

The profile files are named `<n>-<label>[.grid].profraw` (`coverage.rs`): a digest's files are those of its label (a memo site's
id, `world`, `sight`); the grid's are every `.grid` file. For each digest that has files: its executed mapdata functions (count
> 0), each placed in the innermost srchash unit whose lines hold its first line, must be units of that digest's closure. A
function no unit holds is an error too (srchash would not see a change to it). Exit 1 on any. The windows' own code
(`coverage.rs`, `#[cfg(srp_coverage)]`, in no digest: only this build has it, with keys of its own) is left out.
"""
import argparse
import json
import pathlib
import subprocess
import sys
from collections import defaultdict

# The coverage windows themselves (`#[cfg(srp_coverage)]`): no digest holds them.
INSTRUMENT = "crates/mapdata/src/coverage.rs"


def units_of(spans_path):
    """file -> [(first, last, name, {digests})], from `--spans`."""
    out = defaultdict(list)
    for line in pathlib.Path(spans_path).read_text(encoding="utf-8").splitlines():
        file, first, last, name, held = line.split("\t")
        out[file].append((int(first), int(last), name, set() if held == "-" else set(held.split(","))))
    return out


def unit_at(units, file, line):
    """The innermost unit of `file` holding `line`."""
    holding = [u for u in units.get(file, []) if u[0] <= line <= u[1]]
    return min(holding, key=lambda u: u[1] - u[0]) if holding else None


def mapdata_file(path):
    """`crates/mapdata/src/...` of a source path, or None for any other crate's file."""
    p = path.replace("\\", "/")
    at = p.rfind("/crates/mapdata/src/")
    return p[at + 1:] if at >= 0 else None


def executed(binary, profdata, llvm_cov):
    """(file, first line, function name) of every mapdata function with a count over 0."""
    text = subprocess.run([llvm_cov, "export", "-format=text", "-skip-expansions", f"-instr-profile={profdata}", binary],
                          check=True, capture_output=True, text=True).stdout
    seen = set()
    for f in json.loads(text)["data"][0]["functions"]:
        if f["count"] <= 0 or not f["regions"]:
            continue
        own = [r for r in f["regions"] if r[5] == 0] or f["regions"]
        file = mapdata_file(f["filenames"][own[0][5]])
        if file is not None and file != INSTRUMENT:
            seen.add((file, min(r[0] for r in own), f["name"]))
    return seen


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--spans", required=True)
    ap.add_argument("--profiles", required=True)
    ap.add_argument("--binary", required=True)
    ap.add_argument("--llvm-profdata", default="llvm-profdata")
    ap.add_argument("--llvm-cov", default="llvm-cov")
    ap.add_argument("--out")
    a = ap.parse_args()
    units = units_of(a.spans)
    files = sorted(pathlib.Path(a.profiles).glob("[0-9]*-*.profraw"))
    groups = defaultdict(list)
    for p in files:
        stem = p.name[: -len(".profraw")]
        label = stem.split("-", 1)[1]
        grid = label.endswith(".grid")
        label = label[: -len(".grid")] if grid else label
        if label != "between":
            groups[label].append(p)
        if grid:
            groups["grid"].append(p)
    report, failed = [], False
    for digest in sorted(groups):
        profdata = pathlib.Path(a.profiles) / f"merged-{digest}.profdata"
        subprocess.run([a.llvm_profdata, "merge", "-sparse", "-o", str(profdata), *map(str, groups[digest])], check=True)
        ran = executed(a.binary, profdata, a.llvm_cov)
        outside, unplaced, placed = [], [], set()
        for file, line, name in sorted(ran):
            u = unit_at(units, file, line)
            if u is None:
                unplaced.append(f"{file}:{line} {name}")
            elif digest in u[3]:
                placed.add((file, u[2]))
            else:
                outside.append(f"{file}:{line} {u[2]} (function {name})")
        ok = not outside and not unplaced
        failed |= not ok
        report.append(f"{digest}: {'PASS' if ok else 'FAIL'}: {len(ran)} functions executed in {len(groups[digest])} segments, "
                      f"{len(placed)} units, all in the closure" if ok else
                      f"{digest}: FAIL: {len(outside)} executed units outside the closure, {len(unplaced)} functions in no unit")
        report += [f"  outside {x}" for x in outside] + [f"  no unit {x}" for x in unplaced]
    text = "\n".join(report) + "\n"
    print(text, end="")
    if a.out:
        pathlib.Path(a.out).write_text(text, encoding="utf-8")
    return 1 if failed or not groups else 0


if __name__ == "__main__":
    sys.exit(main())
