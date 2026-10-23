#!/usr/bin/env python3
"""bcw writes what patch_14 (London) holds, byte for byte.

    python test_bcw.py <resources.db.patch_14> <0.31a2 db with the fx/artifacts particles>

Needs dbgrab (tools/re-kit/common/dbgrab; DBGRAB names another). Checks:
London's three client projects read and written again are the same bytes;
four particles of 0.31a2 converted as build.py converts them (MarginMultiplier
out, spring's big_sphere -> big_sphere_l09) are patch_14's own, byte for byte.
"""
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bcw  # noqa: E402
import build  # noqa: E402


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    patch_14, release = sys.argv[1:]
    failed = 0
    with tempfile.TemporaryDirectory() as tmp:
        for day in (2, 3, 4):
            path = f"projects/level_09/day_{day}/client_project"
            data = build.Archive([patch_14]).read(path, tmp)
            same = bcw.dump(bcw.loads(data)) == data
            failed += not same
            print("ok  " if same else "FAIL", path, "written again: the same bytes", f"({bcw.count(bcw.read(data))} values)")
        for name in ("pre_onix2_indoor", "pre_zhivokost2", "pre_zhivokost2_indoor", "pre_spring2_indoor"):
            path = f"particles/fx/artifacts/{name}.particle"
            want = build.Archive([patch_14]).read(path, tmp)
            got = build.convert_particle(build.Archive([release]).read(path, tmp))
            failed += got != want
            print("ok  " if got == want else "FAIL", path, "converted from 0.31a2: patch_14's bytes")
    if failed:
        sys.exit(f"{failed} failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
