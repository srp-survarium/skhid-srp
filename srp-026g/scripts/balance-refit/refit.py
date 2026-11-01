#!/usr/bin/env python3
"""Refit the team-balance weights (matchmaking.md section 8) from server logs.

Reads, by match id:
  "Match 0x.. sides: .."       the roster each side started with, and what
                               every player counted for (and why);
  "Match 0x.. late join: .."   who came into the match being played;
  "Match 0x.. at <s> s: .."    each minute: how many of each side are in the
                               match, and who of its roster is out;
  "Match 0x.. settled (.., winner Some(N), ..)"   who won, and how it ended.

and answers:

  1. Which weights would have predicted the winners best (logistic fit)?
  2. How do the weights in force (and any you name) score on the same matches?

A side's value is  P*players + G*gear + L*level + R*trust*rating, the same
sum the server makes (crates/session/src/v026g/balance.rs), with
trust = matches / (matches + trust_matches). The server's logged per-player
numbers are not used, only their parts, so any weights can be tried.

A match that has per-minute lines is fitted on who was in it: each side is
the mean of its rosters over the minutes, so a player who was there for half
of the match is half a player. A match without them (a log from before those
lines) is fitted on its starting roster.

Left out: a match with no winner, a match a whole side left (the server does
not move the balance rating for one either), and a match with fewer than
--min-players in it for most of its time (4: one against one and two against
one say little about how sides should be made, and are the easiest to play
by agreement).

Usage:
  refit.py LOG [LOG ...]                       # files or directories of srp-*.log
  refit.py logs/ --since 2026-10-13T21:31      # only matches made after that UTC time
  refit.py logs/ --weights 12,1,1.5,4 --weights 10,1,2,4
  refit.py logs/ --start-roster                # the sides as the lobby made them
  refit.py logs/ --strict                      # only matches whose roster the log gives in full
  refit.py logs/ --min-players 2               # the smallest matches too

Needs only Python 3. Prints no player names.

Reading the result: with few matches the fit is noisy; the bootstrap range
says how much. Do not change a setting unless the range for it excludes the
value in force AND the log loss of the fit beats the weights in force by a
clear margin over several dozen matches.
"""
import argparse
import math
import random
import re
import sys
from pathlib import Path

# Since 2026-11-01 the parts go on with the K/D at the match's level.
PARTS = r"\(gear (\d+), level (\d+), rating ([+-]\d+\.\d+) of (\d+) matches(?:, [^)]*)?\)"
LINE = re.compile(r"^(\d\d:\d\d:\d\d(?:\.\d+)?)Z ")
SIDES = re.compile(r"Match (0x[0-9a-f]+) sides(?: \(equipment level \d+\))?: team 0 counts -?[\d.]+: (.*) \| team 1 counts -?[\d.]+: (.*)$")
PLAYER = re.compile(r"(?:^|, )(.*?) (-?\d+\.\d+) " + PARTS)
LATE = re.compile(r"Match (0x[0-9a-f]+) late join: (.*); with them team 0 counts ")
# The parts are in the line since 2026-10-14; a line from before has none.
JOINER = re.compile(r"(?:^|, )(.*?) (-?\d+\.\d+)(?: " + PARTS + r")? to team ([01])")
MINUTE = re.compile(
    r"Match (0x[0-9a-f]+) at \d+ s: team 0 counts (-?[\d.]+) \((\d+) in, lead (-?[\d.]+)\), "
    r"team 1 counts (-?[\d.]+) \((\d+) in, lead (-?[\d.]+)\);.*? expected at \d+ %(?:; out: (.*))?$"
)
SETTLED = re.compile(r"Match (0x[0-9a-f]+) settled \((\w+), winner (Some\((\d+)\)|None), (\d+) players\)")

NAMES = ("players", "gear", "level", "rating")
# The weights in force on the players' server since 2026-10-13 21:31 UTC.
IN_FORCE = (10.0, 1.0, 2.0, 4.0)
BEFORE = (12.0, 1.0, 1.5, 4.0)


def parse_side(text):
    return [
        dict(name=n, value=float(v), gear=int(g), level=int(l), rating=float(r), matches=int(m))
        for n, v, g, l, r, m in PLAYER.findall(text)
    ]


def parse_joiners(text):
    """The players of a "late join" line. One without parts has no "gear" key."""
    joiners = []
    for n, v, g, l, r, m, team in JOINER.findall(text):
        p = dict(name=n, value=float(v), team=int(team))
        if g:
            p.update(gear=int(g), level=int(l), rating=float(r), matches=int(m))
        joiners.append(p)
    return joiners


def read_matches(paths, since, until):
    """Yield a dict for each match that has a "sides" and a "settled" line:
    id, sides=[[player..],[player..]], winner (0, 1, 2 for a draw, None),
    reason (how it ended: ModeRule, TimeLimit, Deserted, NoShow),
    settled_players, late (somebody joined it), and events: in the log's
    order ("join", [player..]) and ("minute", dict(count, n, lead, out))."""
    files = []
    for p in paths:
        p = Path(p)
        files += sorted(p.glob("srp-*.log")) if p.is_dir() else [p]
    # Each file once, the days in order: a match's lines are read in order.
    files = sorted({f.resolve() for f in files}, key=lambda f: (f.name, str(f)))
    sides, settled, events = {}, {}, {}
    for f in files:
        day = re.search(r"(\d{4}-\d\d-\d\d)", f.name)
        day = day.group(1) if day else ""
        if (since or until) and not day:
            sys.exit(f"{f.name}: no date in the file name, so --since and --until cannot be applied to it")
        with f.open(encoding="utf-8", errors="replace") as lines:
            for line in lines:
                line = line.rstrip("\n")
                if "Match 0x" not in line:
                    continue
                m = SIDES.search(line)
                if m:
                    t = LINE.match(line)
                    stamp = f"{day}T{t.group(1)}" if t else ""
                    if (since and stamp < since) or (until and stamp > until):
                        continue
                    if m.group(1) not in sides:
                        sides[m.group(1)] = [parse_side(m.group(2)), parse_side(m.group(3))]
                        events[m.group(1)] = []
                    continue
                m = LATE.search(line)
                if m:
                    if m.group(1) in events:
                        events[m.group(1)].append(("join", parse_joiners(m.group(2))))
                    continue
                m = MINUTE.search(line)
                if m:
                    if m.group(1) in events:
                        c0, n0, l0, c1, n1, l1 = m.group(2, 3, 4, 5, 6, 7)
                        events[m.group(1)].append(("minute", dict(
                            count=(float(c0), float(c1)), n=(int(n0), int(n1)),
                            lead=(float(l0), float(l1)), out=m.group(8))))
                    continue
                m = SETTLED.search(line)
                if m:
                    win = int(m.group(4)) if m.group(4) is not None else None
                    settled[m.group(1)] = (win, int(m.group(5)), m.group(2))
    for mid, s in sides.items():
        if mid in settled:
            yield dict(id=mid, sides=s, winner=settled[mid][0], settled_players=settled[mid][1],
                       reason=settled[mid][2],
                       late=any(kind == "join" for kind, _ in events[mid]), events=events[mid])


def players_in(match):
    """How many players the match had: as many as were in it for at least half
    of its per-minute lines, so a match that began one against one and grew is
    as large as it was for most of its time. Its starting roster when the log
    has no such lines."""
    counts = sorted((sum(what["n"]) for kind, what in match["events"] if kind == "minute"), reverse=True)
    if not counts:
        return sum(len(side) for side in match["sides"])
    return counts[(len(counts) - 1) // 2]


def left_out(match, min_players):
    """Why the match is not fitted ("no winner", "deserted", "small"), or None.

    Deserted: a whole side left, and the winner is who stayed; the server
    moves no balance rating for such a match either. Small: fewer than
    `min_players` were in it for most of its time."""
    if match["winner"] not in (0, 1):
        return "no winner"
    if match["reason"] in ("Deserted", "NoShow"):
        return "deserted"
    if players_in(match) < min_players:
        return "small"
    return None


def split_names(text, names):
    """The names a ", "-joined list is made of, or None when it is not made of
    `names`. A name may itself hold ", ", so the list is read against the roster."""
    if not text:
        return []
    for n in names:
        if text == n:
            return [n]
        if n and text.startswith(n + ", "):
            rest = split_names(text[len(n) + 2:], [x for x in names if x != n])
            if rest is not None:
                return [n] + rest
    return None


def rosters(match):
    """Who was in the match at each of its per-minute lines.

    Returns (snapshots, unread): a snapshot is [side 0's players, side 1's];
    `unread` counts the lines the roster could not be told from. A line is
    taken only when it agrees with itself: the players it leaves in are as
    many as it says and count for what it says (less the lead), and the
    parts of each are known."""
    roster = [list(match["sides"][0]), list(match["sides"][1])]
    snapshots, unread = [], 0
    for kind, what in match["events"]:
        if kind == "join":
            for p in what:
                roster[p["team"]].append(p)
            continue
        out = split_names(what["out"], [p["name"] for side in roster for p in side])
        now = []
        for team in (0, 1):
            if out is None:
                break
            side = [p for p in roster[team] if p["name"] not in out]
            # The log rounds each player, the side and the lead to 0.1.
            slack = 0.05 * len(side) + 0.1 + 1e-6
            counted = what["count"][team] - what["lead"][team]
            if (len(side) != what["n"][team] or any("gear" not in p for p in side)
                    or abs(sum(p["value"] for p in side) - counted) > slack):
                break
            now.append(side)
        if len(now) == 2:
            snapshots.append(now)
        else:
            unread += 1
    return snapshots, unread


def explained_by(match, weights, trust_matches, tol=0.08):
    """Does the logged per-player value equal P + G*gear + L*level + R*trust*rating?
    (tol covers the log's rounding: values to 0.1, ratings to 0.01.)"""
    p, g, l, r = weights
    players = [q for side in match["sides"] for q in side]
    players += [q for kind, what in match.get("events", ()) if kind == "join" for q in what if "gear" in q]
    return all(
        abs(p + g * q["gear"] + l * q["level"]
            + r * q["matches"] / (q["matches"] + trust_matches) * q["rating"] - q["value"]) <= tol
        for q in players
    )


def side_features(players, trust_matches):
    """(players, gear, level, trust*rating) summed over a side."""
    n = len(players)
    g = sum(p["gear"] for p in players)
    l = sum(p["level"] for p in players)
    r = sum(p["matches"] / (p["matches"] + trust_matches) * p["rating"] for p in players)
    return (n, g, l, r)


def match_features(match, trust_matches, start_roster=False):
    """What team 0 had more of than team 1, (players, gear, level, rating), and
    how the script knows: dict(minutes, unread, changed).

    minutes  the per-minute lines the sides are the mean of (0: the starting roster)
    unread   the per-minute lines the roster could not be told from
    changed  the roster was not the starting one all through, as far as the log tells
    """
    start = match["sides"]
    snapshots, unread = rosters(match)
    names = lambda sides: [sorted(p["name"] for p in side) for side in sides]
    if snapshots or unread:
        changed = match["late"] or unread > 0 or any(names(s) != names(start) for s in snapshots)
    else:
        changed = match["late"] or match["settled_players"] != len(start[0]) + len(start[1])
    used = [start] if start_roster or not snapshots else snapshots
    diff = [0.0] * 4
    for sides in used:
        a, b = (side_features(side, trust_matches) for side in sides)
        for i in range(4):
            diff[i] += (a[i] - b[i]) / len(used)
    minutes = 0 if start_roster else len(snapshots)
    return diff, dict(minutes=minutes, unread=unread, changed=changed)


def sigmoid(x):
    return 1 / (1 + math.exp(-x)) if x >= 0 else math.exp(x) / (1 + math.exp(x))


def solve(a, b):
    """Gaussian elimination for a small dense system a x = b."""
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for i in range(n):
        piv = max(range(i, n), key=lambda r: abs(m[r][i]))
        m[i], m[piv] = m[piv], m[i]
        if abs(m[i][i]) < 1e-12:
            return None
        for r in range(i + 1, n):
            f = m[r][i] / m[i][i]
            for c in range(i, n + 1):
                m[r][c] -= f * m[i][c]
    x = [0.0] * n
    for i in range(n - 1, -1, -1):
        x[i] = (m[i][n] - sum(m[i][c] * x[c] for c in range(i + 1, n))) / m[i][i]
    return x


def fit_logistic(rows, ridge=0.05):
    """rows: (features diff [k], y in {0,1}). No intercept (sides are symmetric).
    Maximises the log likelihood with a small ridge; Newton's method."""
    k = len(rows[0][0])
    w = [0.0] * k
    for _ in range(60):
        grad = [-ridge * wi for wi in w]
        hess = [[ridge if i == j else 0.0 for j in range(k)] for i in range(k)]
        for x, y in rows:
            p = sigmoid(sum(wi * xi for wi, xi in zip(w, x)))
            for i in range(k):
                grad[i] += (y - p) * x[i]
                for j in range(k):
                    hess[i][j] += p * (1 - p) * x[i] * x[j]
        step = solve(hess, grad)
        if step is None:
            return None
        w = [wi + si for wi, si in zip(w, step)]
        if max(abs(s) for s in step) < 1e-9:
            break
    return w


def log_loss(ps, ys):
    return -sum(math.log(max(min(p, 1 - 1e-12), 1e-12)) if y else math.log(max(min(1 - p, 1 - 1e-12), 1e-12)) for p, y in zip(ps, ys)) / len(ys)


def fit_scale(ds, ys):
    """Best k >= 0 for p = sigmoid(k*d). The slope of the log likelihood falls
    as k grows, so its zero is found by halving. 0 when no k above 0 helps:
    the side ahead by d did not win more often than it lost."""
    def slope(k):
        return sum((y - sigmoid(k * d)) * d for d, y in zip(ds, ys))

    top = max((abs(d) for d in ds), default=0.0)
    if top < 1e-12 or slope(0.0) <= 0:
        return 0.0
    lo, hi = 0.0, 40.0 / top
    if slope(hi) > 0:
        return hi  # called every match right: as sure as the numbers go
    for _ in range(60):
        mid = (lo + hi) / 2
        if slope(mid) > 0:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


def score_weights(feats, ys, w):
    """feats: per match diff vectors (players, gear, level, rating).
    Returns loss, accuracy, scale, wrong: `wrong` when the further these
    weights put a side ahead, the less it won (the best scale is below 0; the
    loss is then a coin's, scale 0)."""
    ds = [sum(wi * xi for wi, xi in zip(w, x)) for x in feats]
    k = fit_scale(ds, ys)
    wrong = sum((y - 0.5) * d for d, y in zip(ds, ys)) < -1e-9
    ps = [sigmoid(k * d) for d in ds]
    decided = [(d, y) for d, y in zip(ds, ys) if abs(d) > 1e-9]
    acc = sum(1 for d, y in decided if (d > 0) == (y == 1)) / len(decided) if decided else float("nan")
    return log_loss(ps, ys), acc, k, wrong


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("logs", nargs="+")
    ap.add_argument("--since", default="", help="UTC, e.g. 2026-10-13T21:31 (matches made at or after)")
    ap.add_argument("--until", default="", help="UTC, matches made at or before")
    ap.add_argument("--trust-matches", type=float, default=5.0, help="SRP_BALANCE_TRUST_MATCHES (5)")
    ap.add_argument("--weights", action="append", default=[], help="P,G,L,R to score, e.g. 12,1,1.5,4 (repeatable)")
    ap.add_argument("--start-roster", action="store_true",
                    help="fit every match on the sides the lobby made, not on who was in it each minute")
    ap.add_argument("--strict", action="store_true",
                    help="leave out the matches whose roster the log does not give in full "
                         "(with --start-roster: the matches whose roster changed)")
    ap.add_argument("--min-players", type=int, default=4,
                    help="leave out the matches with fewer players in them for most of their time "
                         "(4: no one against one, no two against one; 0 keeps every size)")
    ap.add_argument("--bootstrap", type=int, default=300, help="resamples for the ranges (0 = off)")
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()

    ms = list(read_matches(args.logs, args.since, args.until))
    total = len(ms)
    why = [left_out(m, args.min_players) for m in ms]
    ms = [m for m, reason in zip(ms, why) if reason is None]
    dropped = (f"{why.count('no winner')} with no winner 0/1, {why.count('deserted')} left by a whole side, "
               f"{why.count('small')} with fewer than {args.min_players} players in them for most of their time")
    rows = [(m,) + match_features(m, args.trust_matches, args.start_roster) for m in ms]
    if args.strict:
        # In full: every per-minute line read, or none in the log and no sign of a change.
        rows = [(m, x, how) for m, x, how in rows
                if not how["changed"] or (not args.start_roster and how["minutes"] and not how["unread"])]
    if len(rows) < 8:
        sys.exit(f"{len(rows)} usable matches ({total} paired by id; left out: {dropped}): too few to fit.")
    ms = [m for m, _, _ in rows]
    feats = [x for _, x, _ in rows]
    hows = [how for _, _, how in rows]
    ys = [1 if m["winner"] == 0 else 0 for m in ms]

    print(f"{len(ms)} matches used ({total} paired by id; left out: {dropped}"
          f"{'; strict roster' if args.strict else ''}); team 0 won {sum(ys)}.")
    by_minute = sum(1 for how in hows if how["minutes"])
    changed = sum(1 for how in hows if how["changed"])
    if args.start_roster:
        print(f"Rosters: the sides as the lobby made them; {changed} of the matches changed while played.")
    else:
        print(f"Rosters: {by_minute} matches by who was in them each minute, "
              f"{len(ms) - by_minute} by their starting roster (the log has no per-minute lines for them).")
        unread = sum(1 for how in hows if how["unread"])
        if unread:
            print(f"NOTE: in {unread} matches some per-minute lines did not give the roster "
                  f"({sum(how['unread'] for how in hows)} lines: a late joiner the log names without its parts, "
                  "or a line that does not add up); those minutes are left out. --strict leaves the matches out.")
        blind = sum(1 for how in hows if how["changed"] and not how["minutes"])
        if blind:
            print(f"NOTE: {blind} matches changed while played and are fitted on their starting roster all the same "
                  "(no per-minute line of theirs gives the roster). --strict leaves them out.")
    known = [IN_FORCE, BEFORE]
    odd = sum(1 for m in ms if not any(explained_by(m, w, args.trust_matches) for w in known))
    if odd:
        print(f"NOTE: {odd} of {len(ms)} matches were made with weights other than the two known sets "
              "(in force / 12,1,1.5,4) or another trust constant; their parts are still used as logged.")
    print(f"Null model (every match a coin): log loss {math.log(2):.4f}\n")

    # 1. Free fit.
    if len(ms) < 40:
        print(f"WARNING: {len(ms)} matches. The free fit below is not usable yet (4 weights from "
              "so few results swing wildly); read only the scored weights at the end.\n")
    w = fit_logistic([(x, y) for x, y in zip(feats, ys)])
    if w is None or w[1] <= 0:
        print("Free fit: gear has no positive pull in these matches; weights below are not meaningful.")
    if w is not None:
        ds = [sum(wi * xi for wi, xi in zip(w, x)) for x in feats]
        loss = log_loss([sigmoid(d) for d in ds], ys)
        print(f"Free fit (log loss {loss:.4f}, in sample):")
        if w[1] > 0:
            scaled = [wi / w[1] for wi in w]  # gear = 1, the server's unit
            print("  in server units (gear = 1): " + ", ".join(f"{n} {v:+.2f}" for n, v in zip(NAMES, scaled)))
            print("  in force:                   " + ", ".join(f"{n} {v:+.2f}" for n, v in zip(NAMES, IN_FORCE)))
        else:
            print("  raw: " + ", ".join(f"{n} {v:+.3f}" for n, v in zip(NAMES, w)))
        if args.bootstrap and w[1] > 0:
            rnd = random.Random(args.seed)
            boots = [[] for _ in NAMES]
            n = len(feats)
            for _ in range(args.bootstrap):
                idx = [rnd.randrange(n) for _ in range(n)]
                wb = fit_logistic([(feats[i], ys[i]) for i in idx])
                if wb is None or wb[1] <= 0.02:
                    continue
                for j, v in enumerate(wb):
                    boots[j].append(v / wb[1])
            print(f"  90% range over {len(boots[0])} resamples (gear = 1; wide = not enough matches):")
            for nme, vals, cur in zip(NAMES, boots, IN_FORCE):
                if nme == "gear" or len(vals) < 20:
                    continue
                vals.sort()
                lo, hi = vals[int(0.05 * len(vals))], vals[int(0.95 * len(vals)) - 1]
                note = "" if round(lo, 2) <= cur <= round(hi, 2) else "   <- in force is outside"
                print(f"    {nme:8s} {lo:+.2f} .. {hi:+.2f}   (in force {cur:g}){note}")
        print()

    # 2. Named weights. Each gets its own best scale k, so only the *direction* of
    #    the weights is compared, not how confident they are.
    sets = [("in force", IN_FORCE), ("before 2026-10-13 21:31", BEFORE)]
    for s in args.weights:
        vals = tuple(float(v) for v in s.split(","))
        if len(vals) != 4:
            sys.exit("--weights wants four numbers P,G,L,R")
        sets.append(("P,G,L,R = " + s, vals))
    sets += [("gear only", (0.0, 1.0, 0.0, 0.0)), ("level only", (0.0, 0.0, 1.0, 0.0)),
             ("rating only", (0.0, 0.0, 0.0, 1.0)), ("players only", (1.0, 0.0, 0.0, 0.0))]
    print("Weights scored on the same matches (lower log loss is better; accuracy skips ties):")
    print(f"  {'weights':28s} {'log loss':>9s} {'accuracy':>9s}")
    any_wrong = False
    for label, v in sets:
        loss, acc, _, wrong = score_weights(feats, ys, v)
        any_wrong |= wrong
        print(f"  {label:28s} {loss:9.4f} {acc:9.1%}{'   wrong way' if wrong else ''}")
    if any_wrong:
        print("  wrong way: the further these weights put a side ahead, the less it won, so they score as a coin.\n"
              "  For one part alone that is no surprise in matches the server evened out: the side ahead\n"
              "  in one part was made to be behind in another.")
    print("\nA difference under about 0.01 in log loss over fewer than ~60 matches is noise.")
    print("Matches that were made close to even have little to teach: if every split was\n"
          "within a point or two, the fit mostly sees luck. Look at the bootstrap range first.")
    if by_minute and not args.start_roster:
        print("Who is in a match each minute is partly a result of it (players leave a match\n"
              "they are losing), so the fit by minutes counts a player for more than it is worth\n"
              "when the sides are made. Compare with --start-roster.")


if __name__ == "__main__":
    main()
