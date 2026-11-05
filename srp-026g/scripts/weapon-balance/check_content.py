#!/usr/bin/env python3
"""Round-trip the balance tables through the real imported binary configs, in memory.

python3 check_content.py <content-packs-dir>
No patch folder or release archive is produced.
"""

import contextlib
import hashlib
import io
from pathlib import Path
import sys
import importlib

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "srp-026g/scripts/content-edit"))
ce = importlib.import_module("content_edit")


def check(packs):
    files = {}
    for line in (ROOT / "release/content.tsv").read_text().splitlines():
        cells = line.split("\t")
        if cells[0] != "pack" or cells[1].split("@")[0] not in {
            "weapons-031",
            "weapons-046",
            "weapons-069",
        }:
            continue
        folder = packs / cells[1].replace("@", "-")
        assert (
            hashlib.sha256((folder / "pack.json").read_bytes()).hexdigest() == cells[2]
        ), folder
        for p in (folder / "files/gameplay/items/weapons").rglob("*.options"):
            files[p.relative_to(folder / "files").as_posix()] = bytearray(
                p.read_bytes()
            )
    for name in ("weapon-stats", "ammo"):
        category, columns, rows = ce.read_stats(
            str(ROOT / f"srp-026g/scripts/content-edit/{name}.tsv")
        )
        selected = {
            p: b.copy()
            for p, b in files.items()
            if p in ce.stats_paths(category, files)
        }
        targets = ce.stats_targets(category, selected)
        rows = {key: rows[key] for key in targets}
        before = {p: ce.walk(b) for p, b in selected.items()}
        with contextlib.redirect_stdout(io.StringIO()):
            written, made, _ = ce.stats_write(category, columns, rows, selected, name)
        assert not made
        problems = ce.stats_problems(category, columns, rows, targets, selected, name)
        assert not problems, problems
        for p, b in selected.items():
            old = {k: (t, v) for k, t, v in before[p]}
            new = {k: (t, v) for k, t, v in ce.walk(b)}
            assert old.keys() == new.keys(), p
            for k in old:
                assert old[k][0] == new[k][0], (p, k, "changed type")
                if old[k] != new[k]:
                    assert k in written.get(p, []), (p, k, "unexpected change")
            assert not ce.card_differences(b, category["cards"]), p
        print(
            f"{name}: {len(rows)} real configs round-trip, unchanged types, matching cards"
        )


if __name__ == "__main__":
    check(Path(sys.argv[1]))
