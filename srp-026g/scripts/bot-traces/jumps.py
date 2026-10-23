#!/usr/bin/env python3
"""What the bots' jumps came to, from the decision traces.

The server writes `bots_<match>.jsonl` to `SRP_BOT_TRACE_DIR` (on the live
host `/srv/srp/data/bot-traces`). Each jump a bot presses is a line

    {"t", "bot", "jump": "route" | "hop" | "unstick", "pos", "firing", "aiming"}

and what followed it, at most 3 s later, another

    {"t", "bot", "jumped": <kind>, "after_ms", "then", "moved_m", "rose_m"}

where `then` is "went on" (3 s passed), "stuck: <recovery>", "jumped again"
or "a new life" (`docs/notes/player_bots_stuck.md` §6). Only the server's
side is known: the player's game simulates the jump, so a jump the game
refused looks like one that went flat. Whether one helped is read from where
the bot was 3 s later.

The report, by kind:
- how many, and how often (per bot-hour of traced play);
- what followed: freed (went on, at least FREED_M away), stayed (went on,
  less), stuck, jumped again, died or a new life, unknown (the trace ended);
- climbed: went on and at least CLIMB_M higher;
- the jumps made while firing (`firing`: the trigger down then or within
  300 ms), held with the trigger let go since PR #343. Before it the game
  refused them (`weapon_core::can_jump` 0x2E3A70), so these are what the
  hold gained: compare their outcomes with the same kind's other jumps;
- for hops, the spots they repeat at (one 3 m cell of a map): a spot whose
  hops keep coming to nothing is a grid or tuning problem, not a hop.

    python jumps.py <trace files or dirs> [--logs <host logs>] [--since <match ms>]

`--logs` reads the host's daily logs (plain or .gz) for each match's map
(`Match 0x.. made: map N (<project>, mode m)`); without it spots are named
by match. Nothing here names a player: bots are places.
"""

import argparse
import gzip
import json
import math
import re
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path

# A jump "freed" the bot when it went on this far (m) in the 3 s after.
FREED_M = 2.0
# A jump "climbed" when the bot stood this much higher (m) 3 s after.
CLIMB_M = 0.25
# Hop spots: this size (m) of cell, and at least this many hops to list one.
CELL_M = 3.0
SPOT_MIN = 3

KINDS = ("route", "hop", "unstick")
OUTCOMES = ("freed", "stayed", "stuck", "jumped again", "new life", "unknown")


def opener(path):
    return gzip.open(path, "rt", encoding="utf-8", errors="replace") if path.suffix == ".gz" \
        else open(path, encoding="utf-8", errors="replace")


def trace_files(paths):
    for path in map(Path, paths):
        if path.is_dir():
            yield from sorted(path.glob("bots_*.jsonl*"))
        else:
            yield path


def match_of(path):
    m = re.match(r"bots_([0-9a-f]+)\.jsonl", path.name)
    return int(m.group(1), 16) if m else None


MADE = re.compile(r"Match 0x([0-9a-f]+) made: map \d+ \(([^,]+), mode (\d+)\)")


def maps_from_logs(paths):
    """Match id -> "<project> mode <m>" from the host's logs."""
    maps = {}
    for path in map(Path, paths):
        files = sorted(path.glob("*.log*")) if path.is_dir() else [path]
        for file in files:
            with opener(file) as f:
                for line in f:
                    if "made: map" in line:
                        m = MADE.search(line)
                        if m:
                            maps[int(m.group(1), 16)] = f"{m.group(2)} mode {m.group(3)}"
    return maps


def outcome(follow):
    if follow is None:
        return "unknown"
    then = follow.get("then", "")
    if then == "went on":
        return "freed" if (follow.get("moved_m") or 0.0) >= FREED_M else "stayed"
    if then.startswith("stuck"):
        return "stuck"
    if then == "jumped again":
        return "jumped again"
    if then == "a new life":
        return "new life"
    return "unknown"


def read(path, since):
    """The jumps of one trace (each with its follow-up) and its bot time (ms)."""
    jumps = {}  # (bot, t) -> jump line
    follows = {}  # (bot, jump t) -> follow-up line
    seen = {}  # bot -> (first t, last t)
    with opener(path) as f:
        for line in f:
            try:
                v = json.loads(line)
            except ValueError:
                continue  # a line cut by the match's end
            if not isinstance(v, dict) or "bot" not in v or "t" not in v:
                continue
            bot, t = v["bot"], v["t"]
            if t < since:
                continue
            first, last = seen.get(bot, (t, t))
            seen[bot] = (min(first, t), max(last, t))
            if "jump" in v:
                jumps[(bot, t)] = v
            elif "jumped" in v:
                follows[(bot, t - v.get("after_ms", 0))] = v
    bot_ms = sum(last - first for first, last in seen.values())
    out = []
    for key, jump in sorted(jumps.items()):
        out.append((jump, follows.get(key)))
    return out, bot_ms


def attempts(items):
    """Jumps chained by "jumped again" (a route's jump pressed again each
    second until the body is over, an unstick's next try) as one attempt:
    its first jump, the last one's follow-up and the presses it took."""
    out, chain = [], []
    for jump, follow in sorted(items, key=lambda i: (i[0]["_match"], i[0]["bot"], i[0]["t"])):
        if chain and (chain[-1][0]["_match"], chain[-1][0]["bot"]) != (jump["_match"], jump["bot"]):
            out.append(chain)
            chain = []
        chain.append((jump, follow))
        if not (follow and follow.get("then") == "jumped again"):
            out.append(chain)
            chain = []
    if chain:
        out.append(chain)
    return [(c[0][0], c[-1][1], len(c)) for c in out]


def share(n, total):
    return f"{100.0 * n / total:5.1f} %" if total else "    -  "


def median(values):
    return f"{statistics.median(values):.1f}" if values else "-"


def table(title, rows):
    """Rows of (label, [(jump, follow)])."""
    print(f"\n{title}")
    head = f"  {'':22} {'jumps':>7}" + "".join(f" {o:>13}" for o in OUTCOMES) \
        + f" {'climbed':>9} {'moved m':>8} {'rose m':>7}"
    print(head)
    for label, items in rows:
        if not items:
            continue
        counts = Counter(outcome(f) for _, f in items)
        known = len(items) - counts["unknown"]
        went = [f for _, f in items if f and f.get("then") == "went on"]
        climbed = sum(1 for f in went if (f.get("rose_m") or 0.0) >= CLIMB_M)
        cells = "".join(
            f" {share(counts[o], known if o != 'unknown' else len(items)):>13}" for o in OUTCOMES
        )
        print(
            f"  {label:22} {len(items):7}{cells} {share(climbed, known):>9}"
            f" {median([f.get('moved_m') or 0.0 for f in went]):>8}"
            f" {median([f.get('rose_m') or 0.0 for f in went]):>7}"
        )


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("traces", nargs="+", help="bots_<match>.jsonl files (or .gz) or folders of them")
    ap.add_argument("--logs", nargs="*", default=[], help="the host's daily logs, for each match's map")
    ap.add_argument("--since", type=int, default=0, help="skip lines before this match time (ms)")
    ap.add_argument("--spots", type=int, default=15, help="hop spots to list (0: none)")
    args = ap.parse_args()

    maps = maps_from_logs(args.logs) if args.logs else {}
    by_kind = defaultdict(list)
    spots = defaultdict(list)  # (map, cell) -> [(jump, follow)]
    matches, bot_ms, files = set(), 0, 0
    for path in trace_files(args.traces):
        files += 1
        match = match_of(path)
        jumps, ms = read(path, args.since)
        bot_ms += ms
        if match is not None:
            matches.add(match)
        place = maps.get(match, f"match {match:08x}" if match is not None else path.name)
        for jump, follow in jumps:
            kind = jump["jump"]
            jump["_match"] = match
            by_kind[kind].append((jump, follow))
            if kind == "hop" and jump.get("pos"):
                x, _, z = jump["pos"]
                spots[(place, math.floor(x / CELL_M), math.floor(z / CELL_M))].append((jump, follow))

    if not files:
        sys.exit("no traces found")
    hours = bot_ms / 3_600_000
    total = sum(len(v) for v in by_kind.values())
    print(f"{files} traces, {len(matches)} matches, {hours:.1f} bot-hours traced, {total} jumps")
    if maps:
        print(f"maps from the logs: {sum(1 for m in matches if m in maps)} of {len(matches)} matches")
    print(f"\nfreed: went on at least {FREED_M} m in the 3 s after; stayed: went on, less;"
          f" climbed: went on and at least {CLIMB_M} m higher.\nShares are of the jumps whose"
          " outcome is known; unknown is of all (the trace ended within 3 s).")

    print("\nHow often")
    for kind in KINDS + tuple(k for k in by_kind if k not in KINDS):
        n = len(by_kind.get(kind, []))
        rate = f"{n / hours:6.1f} per bot-hour" if hours else ""
        print(f"  {kind:10} {n:7}  {rate}")

    table("What followed each press, by kind", [(k, by_kind.get(k, [])) for k in KINDS])

    # A chain's last follow-up is never "jumped again", but a chain cut by
    # another kind's jump is (that jump is its own attempt).
    tries = {k: attempts(by_kind.get(k, [])) for k in KINDS}
    table("Attempts (presses chained by \"jumped again\" as one), by kind",
          [(k, [(j, f) for j, f, _ in tries[k]]) for k in KINDS])
    print(f"  {'presses per attempt':22} " + "  ".join(
        f"{k} median {median([n for *_, n in tries[k]])}, 90th "
        f"{sorted(n for *_, n in tries[k])[int(0.9 * (len(tries[k]) - 1))] if tries[k] else '-'}"
        for k in KINDS))

    rows = []
    for kind in KINDS:
        items = by_kind.get(kind, [])
        rows.append((f"{kind}, firing", [i for i in items if i[0].get("firing")]))
        rows.append((f"{kind}, not firing", [i for i in items if not i[0].get("firing")]))
    table("Jumps made while firing (refused by the game before PR #343) against the others", rows)
    aiming = [i for k in KINDS for i in by_kind.get(k, []) if i[0].get("aiming")]
    table("Jumps made while aiming (any kind)", [("aiming", aiming)])

    if args.spots and spots:
        listed = sorted(
            ((place, items) for place, items in spots.items() if len(items) >= SPOT_MIN),
            key=lambda p: -len(p[1]),
        )[: args.spots]
        print(f"\nHop spots ({CELL_M:.0f} m cells with {SPOT_MIN}+ hops), most hops first")
        print(f"  {'map':34} {'cell x, z (m)':>16} {'hops':>5} {'bots':>5} {'freed':>8} {'stuck':>8}")
        for (place, cx, cz), items in listed:
            counts = Counter(outcome(f) for _, f in items)
            known = len(items) - counts["unknown"]
            bots = len({(j["_match"], j["bot"]) for j, _ in items})
            print(
                f"  {place[:34]:34} {cx * CELL_M:7.0f},{cz * CELL_M:7.0f} {len(items):5} {bots:5}"
                f" {share(counts['freed'], known):>8} {share(counts['stuck'], known):>8}"
            )
        if not listed:
            print("  none")


if __name__ == "__main__":
    main()
