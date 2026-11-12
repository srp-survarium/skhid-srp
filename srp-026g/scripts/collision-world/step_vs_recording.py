"""A step trace of a replay against the recording the replay was made from: the bodies the real clients
uploaded (`srp-rec-table`'s rec_worlds.csv: every player block of every uploaded world) against the bodies the
replaying game computed itself at the same match time.

    python step_vs_recording.py steps.bdt rec_worlds.csv [--out rows.tsv]

A world's block carries the controller's `m_last_pos`: the capsule's centre where its last step began. For
every upload row inside the trace: the controller that stands for the row's place (the one nearest most often;
a controller keeps its place through deaths), and the distance from the uploaded centre to that controller's
origin one grid step before the world's time, at it and around it. The table has three decimals, so "the
same" is within 0.0005 m a coordinate.
"""
import argparse
import collections
import csv
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import step_file  # noqa: E402

RELATIONS = (("two grid steps before the world's time", -100), ("one grid step before the world's time", -50),
             ("at the world's time", 0), ("one grid step after the world's time", 50))


def upload_rows(path):
    rows = []
    with open(path, newline="") as f:
        for row in csv.DictReader(f):
            if row["alive"] in ("1", "true") and row["cx"]:
                # the table's frame is the map's: z is the physics' -z
                rows.append((int(row["time"]), int(row["place"]), (float(row["cx"]), float(row["cy"]), -float(row["cz"]))))
    return rows


def origin_at(steps, t):
    """A controller's origin at grid time t: what the whole step ending at t left, or the next one started from."""
    step = steps.get(t)
    if step is not None:
        return step[1].origin
    step = steps.get(t + 50)
    return step[0].origin if step is not None else None


def controllers_of_places(whole, rows, shift, t_lo, t_hi):
    votes = collections.defaultdict(collections.Counter)
    for time, place, centre in rows:
        t = (time + shift) // 50 * 50
        if t_lo <= t <= t_hi:
            for controller, steps in whole.items():
                o = origin_at(steps, t)
                if o is not None and math.dist(o, centre) < 0.3:
                    votes[place][controller] += 1
    return {place: counts.most_common(1)[0][0] for place, counts in votes.items()}, votes


def lay(whole, rows, place_of, t_lo, t_hi):
    """For each row: {relation name: distance}, where the trace has the controller's origin."""
    out = []
    for time, place, centre in rows:
        controller = place_of.get(place)
        if controller is None or time % 50 or not t_lo + 150 <= time <= t_hi - 100:
            continue
        found = {}
        for name, offset in RELATIONS:
            o = origin_at(whole[controller], time + offset)
            if o is not None:
                found[name] = max(abs(a - b) for a, b in zip(o, centre))
        if found:
            out.append((time, place, controller, centre, found))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("trace")
    ap.add_argument("worlds")
    ap.add_argument("--out")
    args = ap.parse_args()
    whole = step_file.Trace(args.trace).whole_steps()
    times = sorted({t for steps in whole.values() for t in steps})
    t_lo, t_hi = times[0], times[-1]
    rows = upload_rows(args.worlds)
    print(f"trace: {len(whole)} controllers, whole steps ending {t_lo} .. {t_hi} ms; recording: {len(rows)} alive player blocks")
    place_of, votes = controllers_of_places(whole, rows, -50, t_lo, t_hi)
    print("place -> controller (rows that chose it):", {p: (hex(c), votes[p][c]) for p, c in sorted(place_of.items())})
    laid = lay(whole, rows, place_of, t_lo, t_hi)
    moving = [r for r in laid if len({round(d, 4) for d in r[4].values()}) > 1]
    print(f"{len(laid)} upload rows laid against the trace; {len(moving)} of a body that moved around the world's time")
    for label, group in (("all rows", laid), ("moving bodies", moving)):
        print(f"{label}: rows whose uploaded centre is the trace's origin within the table's rounding (0.0005 m), by relation")
        for name, _ in RELATIONS:
            have = [r for r in group if name in r[4]]
            print(f"  {sum(1 for r in have if r[4][name] <= 0.00051):6d} of {len(have):6d}  {name}")
    step_before = RELATIONS[1][0]
    off = [r for r in laid if r[4].get(step_before, 0.0) > 0.00051]
    buckets = collections.Counter()
    for r in off:
        d = r[4][step_before]
        buckets["< 1 cm" if d < 0.01 else "< 10 cm" if d < 0.1 else "< 1 m" if d < 1 else ">= 1 m"] += 1
    print(f"rows off the one-step-before relation: {len(off)}: {dict(buckets)}; by place: "
          f"{dict(collections.Counter(r[1] for r in off).most_common())}")
    if args.out:
        with open(args.out, "w", newline="\n") as out:
            out.write("time\tplace\tcontroller\tcx\tcy\tcz\t" + "\t".join(name for name, _ in RELATIONS) + "\n")
            for time, place, controller, centre, found in laid:
                out.write(f"{time}\t{place}\t{controller:#x}\t{centre[0]:.3f}\t{centre[1]:.3f}\t{centre[2]:.3f}\t"
                          + "\t".join(f"{found[name]:.6f}" if name in found else "" for name, _ in RELATIONS) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
