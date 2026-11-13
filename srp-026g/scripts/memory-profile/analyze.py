#!/usr/bin/env python3
"""Read patch 127's memory profile (srp_memory_profile.txt, written by a game
started with SKHID_MEMORY_PROFILE=on; docs/notes/memory-profile.md) and print,
for a snapshot: the address-space budget (fixed arenas, thread stacks, images,
mapped files, the profiler's own tables, the rest, and what is left free for
the video driver), Scaleform's heaps by name (the Flash UI: what each holds
and uses), then each arena: its heap's use and largest hole, the free
blocks by size, a managed arena's nodes by resource class, and its top call
sites by live and by peak bytes, named from the re-kit (tools/re-kit/v026g:
the nearest function label at or below the return address; an `?` marks an
unreviewed name).

    python3 analyze.py PROFILE [--run N] [--snapshot N] [--top N] [--arena NAME]

PROFILE is the file in the game's user data folder (Documents\\Survarium; a
local Wine client's prefix: drive_c/users/<user>/Documents/Survarium). A file
holds every run of the game since it was made, each from its `run` line: the
last is read unless --run says (1 the first, -1 the last). The snapshot is the
run's last unless --snapshot says (its number, or `all` for each in turn); a
line per snapshot of the run comes first.
"""
import argparse
import bisect
import csv
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path

KIT = Path(__file__).resolve().parents[3] / "tools" / "re-kit" / "v026g"
MEM_COMMIT, MEM_RESERVE, MEM_FREE = 0x1000, 0x2000, 0x10000
MEM_PRIVATE, MEM_MAPPED, MEM_IMAGE = 0x20000, 0x40000, 0x1000000
PAGE_GUARD = 0x100
HEAP_DEBUG = 0x1000     # a Scaleform heap's flag: a debug heap, which Scaleform's own totals leave out
MIB = 1 << 20


@dataclass
class Counts:
    live: int
    blocks: int
    peak: int
    asked: int
    asked_bytes: int


@dataclass
class Arena:
    index: int
    name: str
    start: int
    size: int
    usage: str                       # heap, counted or unknown
    used: int | None = None
    largest_free: int | None = None
    holes: dict = field(default_factory=dict)       # bucket -> (count, bytes)
    tracked: Counts | None = None
    untracked: int = 0
    untracked_bytes: int = 0
    unseen: int = 0
    other: Counts | None = None
    sites: list = field(default_factory=list)       # (return address, Counts)
    managed: dict = field(default_factory=dict)     # a managed arena's nodes: class id, "more" or "unowned" -> (nodes, bytes)
    managed_torn: bool = False                      # its walk did not reach the end: no sums


@dataclass
class Snapshot:
    number: int
    moment: str
    tick: int
    detail: dict
    free: int = 0
    largest_free: int = 0
    video: int = 0
    blocks: int = 0
    sites_count: int = 0
    arenas: dict = field(default_factory=dict)      # index -> Arena
    regions: list = field(default_factory=list)     # (base, size, state, protect, kind, allocation)
    scaleform: list = field(default_factory=list)   # Scaleform's heaps, a parent before its children: (depth, flags, footprint, used, name)
    scaleform_torn: bool = False                    # the heaps' walk stopped early: the list is a part
    complete: bool = False


@dataclass
class Run:
    process: int
    tick: int
    slots: tuple
    rooms: list = field(default_factory=list)       # (base, bytes)
    snapshots: list = field(default_factory=list)


def number(word):
    return int(word, 0)


def counts(words):
    return Counts(*(number(w) for w in words[:5]))


def parse(text):
    """The runs of a profile's text, each with its snapshots."""
    runs, snapshot = [], None
    for line in text.splitlines():
        words = line.split("\t")
        tag, rest = words[0], words[1:]
        if tag == "run":
            runs.append(Run(number(rest[0]), number(rest[2]), tuple(number(w) for w in rest[4:6])))
            snapshot = None
            continue
        if not runs:
            continue
        run = runs[-1]
        if tag == "room":
            run.rooms.append((number(rest[0]), number(rest[1])))
        elif tag == "snapshot":
            detail = {rest[i]: number(rest[i + 1]) for i in range(4, len(rest) - 1, 2)}
            snapshot = Snapshot(number(rest[0]), rest[1], number(rest[3]), detail)
            run.snapshots.append(snapshot)
        elif snapshot is None:
            continue
        elif tag == "process":
            snapshot.free, snapshot.largest_free, snapshot.video = number(rest[1]), number(rest[3]), number(rest[5])
        elif tag == "blocks":
            snapshot.blocks, snapshot.sites_count = number(rest[0]), number(rest[2])
        elif tag == "arena":
            arena = Arena(number(rest[0]), rest[1], number(rest[2]), number(rest[3]), rest[4])
            if arena.usage == "heap":
                arena.used, arena.largest_free = number(rest[5]), number(rest[6])
            elif arena.usage == "counted":
                arena.used = number(rest[5])
            snapshot.arenas[arena.index] = arena
        elif tag == "holes":
            arena = snapshot.arenas[number(rest[0])]
            figures = [number(w) for w in rest[1:]]
            arena.holes = {figures[i]: (figures[i + 1], figures[i + 2]) for i in range(0, len(figures) - 2, 3)}
        elif tag in ("tracked", "other", "site"):
            index = number(rest[0])
            arena = snapshot.arenas.get(index)
            if arena is None:                       # blocks outside every arena of the list
                arena = snapshot.arenas[index] = Arena(index, "(outside the arenas)", 0, 0, "unknown")
            if tag == "tracked":
                arena.tracked = counts(rest[1:6])
                arena.untracked, arena.untracked_bytes, arena.unseen = (number(w) for w in rest[6:9])
            elif tag == "other":
                arena.other = counts(rest[1:6])
            else:
                arena.sites.append((number(rest[1]), counts(rest[2:7])))
        elif tag == "managed":
            arena = snapshot.arenas[number(rest[0])]
            if rest[1] == "torn":
                arena.managed_torn = True
            else:
                key, figures = (number(rest[2]), rest[3:5]) if rest[1] == "class" else (rest[1], rest[2:4])
                arena.managed[key] = tuple(number(w) for w in figures)
        elif tag == "scaleform":
            if rest[0] == "torn":
                snapshot.scaleform_torn = True
            else:
                snapshot.scaleform.append((*(number(w) for w in rest[:4]), rest[4] if len(rest) > 4 else ""))
        elif tag == "region":
            snapshot.regions.append(tuple(number(w) for w in rest[:6]))
        elif tag == "end":
            snapshot.complete = True
            snapshot = None
    return runs


class Names:
    """The re-kit's function labels, for a return address: the nearest label at
    or below it (in the exe's .text; past it, our patches' sections)."""

    def __init__(self, kit=KIT):
        self.text = (0x11000, 0x11000 + 8775075)
        identity = kit / "identity.json"
        if identity.exists():
            sections = json.loads(identity.read_text())["target"]["sections"]
            text = next((s for s in sections if s["name"] == ".text"), None)
            if text:
                self.text = (int(text["va"], 16), int(text["va"], 16) + text["virtual_size"])
        labels = {}
        for name, status in (("functions.tsv", ""), ("inferred.tsv", "?")):
            path = kit / name
            if not path.exists():
                continue
            with open(path, newline="") as f:
                for row in csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE):
                    va = int(row["va"], 16)
                    if va not in labels or (labels[va][1] and not status):
                        labels[va] = (row["name"], status)
        self.vas = sorted(labels)
        self.labels = labels

    def of(self, address):
        if not self.text[0] <= address < self.text[1]:
            return "(our patches' code)" if 0xA000000 <= address < 0x10000000 else "(not the exe's code)"
        at = bisect.bisect_right(self.vas, address) - 1
        if at < 0:
            return "?"
        va = self.vas[at]
        name, status = self.labels[va]
        if len(name) > 90:
            name = name[:87] + "..."
        far = " (far past it: a function with no label)" if address - va > 0x4000 else ""
        return f"{name}{status}+{address - va:#x}{far}"


def size(bytes_):
    if bytes_ is None:
        return "-"
    if abs(bytes_) >= MIB:
        return f"{bytes_ / MIB:.1f} MiB"
    if abs(bytes_) >= 1024:
        return f"{bytes_ / 1024:.1f} KiB"
    return f"{bytes_} B"


def overlap(base, length, start, end):
    return max(0, min(base + length, end) - max(base, start))


def budget(snapshot, rooms):
    """The address space by what holds it: [(what, bytes)], the free regions."""
    arenas = [(a.start, a.start + a.size) for a in snapshot.arenas.values() if a.start and a.size]
    own = [(base, base + length) for base, length in rooms]
    guarded = {allocation for _, _, state, protect, _, allocation in snapshot.regions
               if state == MEM_COMMIT and protect & PAGE_GUARD}
    out = {"fixed arenas": 0, "profiler's own tables": 0, "thread stacks": 0, "images (exe, dlls)": 0,
           "mapped (files, sections)": 0, "other private (driver, Wine, CRT heaps)": 0}
    free = []
    for base, length, state, protect, kind, allocation in snapshot.regions:
        if state == MEM_FREE:
            free.append(length)
            continue
        in_arenas = sum(overlap(base, length, s, e) for s, e in arenas)
        in_own = sum(overlap(base, length, s, e) for s, e in own)
        out["fixed arenas"] += in_arenas
        out["profiler's own tables"] += in_own
        rest = length - in_arenas - in_own
        if rest <= 0:
            continue
        if kind == MEM_IMAGE:
            out["images (exe, dlls)"] += rest
        elif kind == MEM_MAPPED:
            out["mapped (files, sections)"] += rest
        elif allocation in guarded:
            out["thread stacks"] += rest
        else:
            out["other private (driver, Wine, CRT heaps)"] += rest
    return out, free


def summary_line(snapshot, run):
    seconds = ((snapshot.tick - run.tick) & 0xFFFFFFFF) / 1000
    heaps = [a for a in snapshot.arenas.values() if a.usage == "heap" and a.size]
    tightest = min(heaps, key=lambda a: a.largest_free, default=None)
    tail = "" if tightest is None else \
        f"; tightest heap \"{tightest.name}\" largest hole {size(tightest.largest_free)}"
    detail = "".join(f" {k} {v}" for k, v in snapshot.detail.items())
    return (f"  #{snapshot.number:<3} {snapshot.moment:<13} +{seconds:8.1f} s{detail}: address space free "
            f"{size(snapshot.free)}, largest {size(snapshot.largest_free)}{tail}"
            + ("" if snapshot.complete else " (cut off)"))


def print_snapshot(snapshot, run, names, top, only):
    print(f"\nsnapshot {snapshot.number}: {snapshot.moment}" +
          "".join(f", {k} {v}" for k, v in snapshot.detail.items()))
    if snapshot.moment == "out_of_memory":
        arena = snapshot.arenas.get(snapshot.detail.get("arena"))
        print(f"  the game ends next: arena \"{arena.name if arena else '?'}\" had no room for "
              f"{size(snapshot.detail.get('asked'))}")
    print(f"  address space: free {size(snapshot.free)}, largest free region {size(snapshot.largest_free)}; "
          f"video budget {size(snapshot.video)}")
    print(f"  tracked blocks {snapshot.blocks} of {run.slots[0] * 7 // 8 if run.slots else '?'}, "
          f"sites {snapshot.sites_count} of {run.slots[1] * 7 // 8 if run.slots else '?'}")
    held, free = budget(snapshot, run.rooms)
    print("  address-space budget (4 GiB):")
    for what, bytes_ in held.items():
        print(f"    {what:<44} {size(bytes_):>12}")
    big = sorted(free, reverse=True)
    print(f"    {'free (what the video driver can still get)':<44} {size(sum(free)):>12}  largest "
          f"{size(big[0] if big else 0)}, then {', '.join(size(b) for b in big[1:4]) or '-'}; "
          f"{sum(1 for b in free if b >= 64 * MIB)} regions of 64 MiB or more")
    if (snapshot.scaleform or snapshot.scaleform_torn) and (not only or only.lower() in "scaleform"):
        own = [heap for heap in snapshot.scaleform if not heap[1] & HEAP_DEBUG]
        print(f"\n  Scaleform's heaps (the Flash UI; their blocks are the \"global multithreaded\" arena's): "
              f"{size(sum(h[2] for h in own))} held, {size(sum(h[3] for h in own))} of it in use"
              + (" (the walk stopped early: a part)" if snapshot.scaleform_torn else ""))
        for depth, flags, footprint, used, name in snapshot.scaleform:
            print(f"    {size(footprint):>10} held, {size(used):>10} in use  {'  ' * depth}{name}"
                  + (" (a debug heap, not in the sum)" if flags & HEAP_DEBUG else ""))
    for arena in sorted(snapshot.arenas.values(), key=lambda a: a.index):
        if only and only.lower() not in arena.name.lower():
            continue
        print(f"\n  arena {arena.index} \"{arena.name}\": {size(arena.size)} at {arena.start:#010x}, ", end="")
        if arena.usage == "heap":
            share = f" ({100 * arena.used / arena.size:.1f} %)" if arena.size else ""
            print(f"heap used {size(arena.used)}{share}, largest hole {size(arena.largest_free)}")
        elif arena.usage == "counted":
            print(f"counted {size(arena.used)} in use")
        else:
            print("use unknown")
        if arena.holes:
            print("    holes: " + ", ".join(f"{size(1 << b)}+: {c} ({size(s)})" for b, (c, s) in sorted(arena.holes.items())))
        if arena.tracked:
            t = arena.tracked
            print(f"    tracked: {size(t.live)} live in {t.blocks} blocks, peak {size(t.peak)}; asked {t.asked} "
                  f"blocks, {size(t.asked_bytes)} since the start" +
                  (f"; {arena.untracked} blocks ({size(arena.untracked_bytes)}) untracked, the table full"
                   if arena.untracked else "") +
                  (f"; {arena.unseen} frees of blocks not tracked" if arena.unseen else ""))
        if arena.other:
            print(f"    sites past the table: {size(arena.other.live)} live, peak {size(arena.other.peak)}")
        if arena.managed_torn:
            print("    its nodes: changing under the walk, not told this snapshot")
        if arena.managed:
            nodes = sum(n for n, _ in arena.managed.values())
            print(f"    its nodes by resource class: {nodes} allocated, {size(sum(b for _, b in arena.managed.values()))} "
                  "(each with its 52-byte head)")
            names_of = {"more": "classes past the table", "unowned": "no owner (being made)"}
            for key, (count, bytes_) in sorted(arena.managed.items(), key=lambda item: -item[1][1])[:top]:
                print(f"      {size(bytes_):>10} in {count:>6}  {names_of.get(key) or f'class {key}'}")
        for title, key in (("live", lambda s: s[1].live), ("peak", lambda s: s[1].peak)):
            sites = sorted(arena.sites, key=key, reverse=True)[:top]
            if not sites or key(sites[0]) == 0:
                continue
            print(f"    top sites by {title}:")
            for address, c in sites:
                print(f"      {size(c.live):>10} live in {c.blocks:>6}, peak {size(c.peak):>10}, asked {c.asked:>7}  "
                      f"{address:#010x} {names.of(address)}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0],
                                     formatter_class=argparse.RawDescriptionHelpFormatter, epilog=__doc__)
    parser.add_argument("profile", type=Path)
    parser.add_argument("--run", type=int, default=-1)
    parser.add_argument("--snapshot", default="last")
    parser.add_argument("--top", type=int, default=12)
    parser.add_argument("--arena", help="only the arenas whose name has this")
    parser.add_argument("--kit", type=Path, default=KIT, help="the re-kit folder the names come from")
    args = parser.parse_args(argv)
    runs = parse(args.profile.read_text(errors="replace"))
    if not runs:
        sys.exit(f"{args.profile}: no run in it")
    try:
        run = runs[args.run - 1 if args.run > 0 else args.run]
    except IndexError:
        sys.exit(f"{args.profile}: {len(runs)} runs, no run {args.run}")
    print(f"run {runs.index(run) + 1} of {len(runs)}: process {run.process}, {len(run.snapshots)} snapshots; "
          f"the profiler's tables {size(sum(b for _, b in run.rooms))}")
    for snapshot in run.snapshots:
        print(summary_line(snapshot, run))
    if not run.snapshots:
        return
    if args.snapshot == "all":
        chosen = run.snapshots
    elif args.snapshot == "last":
        chosen = run.snapshots[-1:]
    else:
        chosen = [s for s in run.snapshots if s.number == int(args.snapshot)]
        if not chosen:
            sys.exit(f"no snapshot {args.snapshot} in run {runs.index(run) + 1}")
    names = Names(args.kit)
    for snapshot in chosen:
        print_snapshot(snapshot, run, names, args.top, args.arena)


if __name__ == "__main__":
    main()
