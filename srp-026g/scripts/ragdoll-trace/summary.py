#!/usr/bin/env python3
"""Summarise a ragdoll trace (patch 76's opt-in srp_ragdoll.csv, a build
made with `patch_026g.py ... --ragdoll-trace`; docs/notes/ragdoll.md).

    python summary.py <srp_ragdoll.csv> [--calc <calc.js>] [--top N]

Prints:
- the pushes per ammo: each living hit's push (N s) and speed, grouped by
  the ammo config's mass (and its muzzle speed, when the trace has it:
  464f's lines); `--calc` (the damage calculator's calc.js, which holds
  every ammo config's mass and muzzle speed) names them;
- the repeat groups: one bullet recorded more than once on one victim in
  one step (the same time and place: its segments through the body), their
  sizes and the worst;
- the top deaths by knock (the whole body's m/s), with the hits before each;
- the spike lines (464f): every applied push or part speed past the
  trace's threshold or at the cap, with its inputs.

The lines (columns past the first ones are optional: older traces lack them):
    hit,time,place,mass,speed,push[,muzzle,segment,bullet,shooter]
    death,time,place,cause,hits,push,knock,mass,speed,legs,waist,neck,arms
    spike,time,place,what,mass,speed,muzzle,segment,hits,part,kit,push,part speed,capped
"""
import collections
import json
import re
import sys

PARTS = ("pelvis", "torso", "head", "left thigh", "left shin", "right thigh", "right shin", "left upper arm",
         "left forearm", "right upper arm", "right forearm")
CAUSES = {0: "none", 1: "body", 2: "head", 3: "blast"}
WINDOW = 0.15          # the solver's hits window before a death, s (shot.rs Recent)


def pct(values, p):
    values = sorted(values)
    if not values:
        return float("nan")
    k = (len(values) - 1) * p / 100
    lo = int(k)
    hi = min(lo + 1, len(values) - 1)
    return values[lo] + (values[hi] - values[lo]) * (k - lo)


def numbers(fields):
    out = []
    for f in fields:
        try:
            out.append(float(f))
        except ValueError:
            out.append(float("nan"))
    return out


def read(path):
    hits, deaths, spikes = [], [], []
    for line in open(path, encoding="ascii", errors="replace"):
        parts = line.strip().split(",")
        if parts[0] == "hit" and len(parts) >= 6:
            v = numbers(parts[1:])
            hit = {"time": v[0], "place": int(v[1]), "mass": v[2], "speed": v[3], "push": v[4]}
            if len(v) >= 9:
                hit.update(muzzle=v[5], segment=int(v[6]), bullet=int(v[7]), shooter=int(v[8]))
            hits.append(hit)
        elif parts[0] == "death" and len(parts) >= 13:
            v = numbers(parts[1:])
            deaths.append({"time": v[0], "place": int(v[1]), "cause": int(v[2]), "hits": int(v[3]), "push": v[4],
                           "knock": v[5], "mass": v[6], "speed": v[7], "strengths": v[8:12]})
        elif parts[0] == "spike" and len(parts) >= 14:
            v = numbers(parts[1:3] + parts[4:])
            spikes.append({"time": v[0], "place": int(v[1]), "what": parts[3], "mass": v[2], "speed": v[3],
                           "muzzle": v[4], "segment": int(v[5]), "hits": int(v[6]), "part": int(v[7]), "kit": v[8],
                           "push": v[9], "part_speed": v[10], "capped": int(v[11])})
    return hits, deaths, spikes


def ammo_names(calc):
    """{(mass, muzzle speed): [names]} from the damage calculator's calc.js."""
    text = open(calc, encoding="utf-8").read()
    at = text.find('"ammo":{')
    if at < 0:
        return {}
    start = at + len('"ammo":')
    depth = 0
    for end in range(start, len(text)):
        depth += {"{": 1, "}": -1}.get(text[end], 0)
        if depth == 0:
            break
    names = collections.defaultdict(list)
    for key, a in json.loads(text[start:end + 1]).items():
        if "mass" in a and "muzzle_speed" in a:
            names[(round(a["mass"], 4), round(a["muzzle_speed"]))].append(a.get("name") or key)
    return names


def main():
    args = sys.argv[1:]
    calc = args[args.index("--calc") + 1] if "--calc" in args else None
    top = int(args[args.index("--top") + 1]) if "--top" in args else 10
    path = [a for a in args if not a.startswith("--") and a not in (calc, str(top))][0]
    hits, deaths, spikes = read(path)
    names = ammo_names(calc) if calc else {}
    print(f"{path}: {len(hits)} hits, {len(deaths)} deaths, {len(spikes)} spikes")

    print("\n== pushes per ammo (N s; speed m/s at the hit)")
    groups = collections.defaultdict(list)
    for h in hits:
        groups[(h["mass"], h.get("muzzle"))].append(h)
    print(f"  {'mass':>6} {'muzzle':>6} {'hits':>5} {'push p10':>8} {'p50':>6} {'p90':>6} {'max':>6} "
          f"{'speed p50':>9}  ammo")
    for (mass, muzzle), group in sorted(groups.items(), key=lambda x: (x[0][0], x[0][1] or 0)):
        push = [h["push"] for h in group]
        speed = [h["speed"] for h in group]
        named = ", ".join(names.get((round(mass, 4), round(muzzle)), [])) if muzzle is not None else \
            ", ".join(sorted({n for (m, _), ns in names.items() if m == round(mass, 4) for n in ns}))
        print(f"  {mass:6.3f} {muzzle if muzzle is not None else '-':>6} {len(group):5d} {pct(push, 10):8.2f} "
              f"{pct(push, 50):6.2f} {pct(push, 90):6.2f} {max(push):6.2f} {pct(speed, 50):9.1f}  {named}")

    print("\n== repeat groups (one bullet, one victim, one step)")
    if hits and "bullet" in hits[0]:
        key = lambda h: (h["time"], h["place"], h.get("shooter"), h["bullet"])
    else:
        # no bullet id (464d's lines): the same step, place and ammo, the
        # speed never rising (one bullet's successive segments)
        key = lambda h: (h["time"], h["place"], h["mass"])
    repeats = collections.defaultdict(list)
    for h in hits:
        repeats[key(h)].append(h)
    sizes = collections.Counter(len(g) for g in repeats.values())
    print("  size: groups  " + "  ".join(f"{n}: {c}" for n, c in sorted(sizes.items())))
    worst = sorted(repeats.values(), key=len, reverse=True)[:5]
    for g in worst:
        if len(g) > 1:
            print(f"  t {g[0]['time']:.3f} place {g[0]['place']} mass {g[0]['mass']}: {len(g)} lines, speeds "
                  + " -> ".join(f"{h['speed']:.1f}" for h in g[:8]) + (" ..." if len(g) > 8 else ""))

    print(f"\n== top {top} deaths by knock (m/s)")
    by_place = collections.defaultdict(list)
    for h in hits:
        by_place[h["place"]].append(h)
    for d in sorted(deaths, key=lambda d: d["knock"], reverse=True)[:top]:
        before = [h for h in by_place[d["place"]] if d["time"] - 1.0 <= h["time"] <= d["time"]]
        masses = collections.Counter(h["mass"] for h in before if d["time"] - h["time"] <= WINDOW + 0.05)
        print(f"  t {d['time']:.3f} place {d['place']:2d} {CAUSES.get(d['cause'], d['cause']):5s} hits {d['hits']} "
              f"push {d['push']:6.2f} knock {d['knock']:.2f} mass {d['mass']:.1f} speed {d['speed']:.2f}; the "
              f"hits just before by mass: " + ", ".join(f"{m}: {c}" for m, c in sorted(masses.items())))
    knocks = [d["knock"] for d in deaths]
    if knocks:
        print(f"  knock p50 {pct(knocks, 50):.2f} p90 {pct(knocks, 90):.2f} max {max(knocks):.2f}")

    if spikes:
        print("\n== spikes")
        for s in sorted(spikes, key=lambda s: s["part_speed"], reverse=True)[:top]:
            part = PARTS[s["part"]] if 0 <= s["part"] < len(PARTS) else s["part"]
            print(f"  t {s['time']:.3f} place {s['place']:2d} {s['what']:5s} mass {s['mass']} speed {s['speed']:.1f} "
                  f"muzzle {s['muzzle']:.0f} segment {s['segment']} hits {s['hits']} {part} kit {s['kit']:.1f} kg: "
                  f"push {s['push']:.2f} N s, part {s['part_speed']:.2f} m/s{' (capped)' if s['capped'] else ''}")
        print("  by ammo: " + ", ".join(f"{m}: {c}" for m, c in
                                        sorted(collections.Counter(s["mass"] for s in spikes).items())))


if __name__ == "__main__":
    main()
