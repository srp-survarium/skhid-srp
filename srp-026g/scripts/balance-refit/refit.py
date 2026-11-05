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
  refit.py LOG [LOG ...]                       # files or directories of lobby.log*, srp.log* (and old srp-*.log)
  refit.py logs/ --since 2026-10-13T21:31      # only matches made after that UTC time
  refit.py logs/ --weights 12,1,1.5,4 --weights 10,1,2,4
  refit.py logs/ --start-roster                # the sides as the lobby made them
  refit.py logs/ --strict                      # only matches whose roster the log gives in full
  refit.py logs/ --min-players 2               # the smallest matches too
  refit.py logs/ --db dbs/ [--clan TAG] [--show REGEX]   # test the strength rule (kd-balance.md)

Needs only Python 3. Prints no player names (but those --show and --clan ask for).

Reading the result: with few matches the fit is noisy; the bootstrap range
says how much. Do not change a setting unless the range for it excludes the
value in force AND the log loss of the fit beats the weights in force by a
clear margin over several dozen matches.
"""
import argparse
import json
import math
import random
import re
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path

# Since 2026-11-01 the parts go on with the K/D at the match's level (the
# last group: "at 6: kd 1.25 of 40/31", and since the kills a minute
# ", kpm 0.90 of 27/30 min" and ", squad" for a squad's member).
PARTS = r"\(gear (\d+), level (\d+), rating ([+-]\d+\.\d+) of (\d+) matches(?:, ([^)]*))?\)"
KD_AT = re.compile(r"at (\d+): kd (\d+\.\d+) of (\d+)/(\d+)")
# The server files (lobby.log, srp.log) date each line; the day files of
# before 2026-11-05 (srp-YYYY-MM-DD.log) have the date in their name.
LINE = re.compile(r"^(?:(\d{4}-\d\d-\d\d)T)?(\d\d:\d\d:\d\d(?:\.\d+)?)Z ")
# A server file and its rotated ones: lobby.log, lobby.log.1 (older), ...
ROTATED = re.compile(r"(.+\.log)(?:\.(\d+))?")
SIDES = re.compile(r"Match (0x[0-9a-f]+) sides(?: \(equipment level (\d+)\))?: team 0 counts -?[\d.]+: (.*) \| team 1 counts -?[\d.]+: (.*)$")
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


def at_level(extra):
    """The parts after the rating (since 2026-11-01): {"kd": K/D at the match's
    level, "squad": counted as a squad's member} when they give the K/D."""
    m = KD_AT.search(extra)
    if m is None:
        return {}
    return dict(kd=float(m.group(2)), squad="squad" in extra.split(", "))


def parse_side(text):
    return [
        dict(name=n, value=float(v), gear=int(g), level=int(l), rating=float(r), matches=int(m), **at_level(x))
        for n, v, g, l, r, m, x in PLAYER.findall(text)
    ]


def parse_joiners(text):
    """The players of a "late join" line. One without parts has no "gear" key."""
    joiners = []
    for n, v, g, l, r, m, x, team in JOINER.findall(text):
        p = dict(name=n, value=float(v), team=int(team))
        if g:
            p.update(gear=int(g), level=int(l), rating=float(r), matches=int(m), **at_level(x))
        joiners.append(p)
    return joiners


def age(f):
    """Oldest first, so a match's lines are read in order: the day files by
    their date, then each server's files from the last rotated one to its
    current one."""
    m = ROTATED.fullmatch(f.name)
    if m is None or re.search(r"\d{4}-\d\d-\d\d", f.name):
        return (0, f.name, 0, str(f))
    return (1, m.group(1), -int(m.group(2) or 0), str(f))


def read_matches(paths, since, until):
    """Yield a dict for each match that has a "sides" and a "settled" line:
    id, sides=[[player..],[player..]], winner (0, 1, 2 for a draw, None),
    reason (how it ended: ModeRule, TimeLimit, Deserted, NoShow),
    settled_players, late (somebody joined it), and events: in the log's
    order ("join", [player..]) and ("minute", dict(count, n, lead, out))."""
    files = []
    for p in paths:
        p = Path(p)
        files += [*p.glob("srp-*.log"), *p.glob("srp.log*"), *p.glob("lobby.log*")] if p.is_dir() else [p]
    files = sorted({f.resolve() for f in files}, key=age)
    sides, settled, events = {}, {}, {}
    for f in files:
        day = re.search(r"(\d{4}-\d\d-\d\d)", f.name)
        day = day.group(1) if day else ""
        with f.open(encoding="utf-8", errors="replace") as lines:
            for line in lines:
                line = line.rstrip("\n")
                if "Match 0x" not in line:
                    continue
                t = LINE.match(line)
                stamp = f"{t.group(1) or day}T{t.group(2)}" if t else ""
                m = SIDES.search(line)
                if m:
                    if (since or until) and not stamp[:1].isdigit():
                        sys.exit(f"{f.name}: a line without a date, so --since and --until cannot be applied to it")
                    if (since and stamp < since) or (until and stamp > until):
                        continue
                    if m.group(1) not in sides:
                        level = int(m.group(2)) if m.group(2) else None
                        sides[m.group(1)] = ([parse_side(m.group(3)), parse_side(m.group(4))], level, stamp)
                        events[m.group(1)] = []
                    continue
                m = LATE.search(line)
                if m:
                    if m.group(1) in events:
                        joiners = parse_joiners(m.group(2))
                        for p in joiners:
                            p["at"] = stamp
                        events[m.group(1)].append(("join", joiners))
                    continue
                m = MINUTE.search(line)
                if m:
                    if m.group(1) in events:
                        c0, n0, l0, c1, n1, l1 = m.group(2, 3, 4, 5, 6, 7)
                        events[m.group(1)].append(("minute", dict(
                            count=(float(c0), float(c1)), n=(int(n0), int(n1)),
                            lead=(float(l0), float(l1)), out=m.group(8), at=stamp)))
                    continue
                m = SETTLED.search(line)
                if m:
                    win = int(m.group(4)) if m.group(4) is not None else None
                    settled[m.group(1)] = (win, int(m.group(5)), m.group(2), stamp)
    for mid, (s, level, stamp) in sides.items():
        if mid in settled:
            yield dict(id=mid, sides=s, winner=settled[mid][0], settled_players=settled[mid][1],
                       reason=settled[mid][2], level=level, at=stamp, settled_at=settled[mid][3],
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


# ---------------------------------------------------------------------------
# The strength back-test (--db): docs/notes/kd-balance.md. A player counts for
# its K/D at the match's level times its kills a minute against the server's
# average; the sides keep the strongest apart and a late joiner goes to the
# weaker side. The numbers are those of crates/session/src/v026g/level_stats.rs.
PRIOR_DEATHS = 10
PRIOR_MINUTES = 15
AVERAGE_KPM = 0.83
KPM_FACTOR_MOST = 1.5
STRONG = 2.5
SQUAD_FACTOR = 1.3
PLAYER_KD = 1.0  # SRP_BALANCE_PLAYER_KD


def read_reports(paths):
    """The match reports kept in srp databases (their g26_results rows; any
    number of copies of the database, a report in several read once), by
    match id as the log writes it: how a match against players (no bots, no
    PIN) ended, its winner, and by name each player's team, kills, deaths and
    seconds in it."""
    files = []
    for p in map(Path, paths):
        files += p.glob("*.db") if p.is_dir() else [p]
    reports, accounts = {}, {}
    # Oldest first, so the newest database names each account.
    for f in sorted(files, key=lambda f: f.stat().st_mtime):
        con = sqlite3.connect(f"{f.resolve().as_uri()}?mode=ro", uri=True)
        try:
            rows = con.execute("SELECT key, body FROM g26_results WHERE key LIKE 'report:%'").fetchall()
            for account, name, tag in con.execute("SELECT account_id, name, clan_tag FROM g26_accounts"):
                accounts[account] = (name, tag)
        finally:
            con.close()
        for key, body in rows:
            mid = f"{int(key[len('report:'):]):#x}"
            if mid in reports:
                continue
            r = json.loads(body)
            stats = r["stats"]
            by_place = {row["player_id"]: row for row in stats["rows"]}
            bots = r.get("vs_bots") or any(row["clan_tag"] == "BOTS" for row in by_place.values())
            players = {}
            for p in r["players"]:
                row = by_place.get(p["player_id"])
                seconds = ((p.get("full") or {}).get("play_seconds") or [0])[0]
                if row is None or not p["account_id"] or seconds <= 0:
                    continue
                players[row["name"]] = dict(account=p["account_id"], team=row["team"], kills=row["kills"],
                                            deaths=row["deaths"], seconds=seconds)
            reports[mid] = None if bots or r.get("private") else dict(
                end=r["end"], winner=stats["winner_team"], players=players)
    return {mid: r for mid, r in reports.items() if r is not None}, accounts


class Totals:
    """Each account's kills, deaths and minutes over the reports, and the
    same without one match (so a match is never predicted from itself). A
    name of the log is the account that had it in that match's report, else
    the one named so now (or last in a report)."""

    def __init__(self, reports, accounts):
        self.reports = reports
        self.sums, self.names, self.clans = {}, {}, {}
        for r in reports.values():
            for name, p in r["players"].items():
                s = self.sums.setdefault(p["account"], [0, 0, 0.0])
                s[0] += p["kills"]
                s[1] += p["deaths"]
                s[2] += p["seconds"] / 60
                self.names.setdefault(name, p["account"])
        for account, (name, tag) in accounts.items():
            self.names[name] = account
            self.clans[account] = tag

    def account(self, name, match=None):
        own = self.reports.get(match, {}).get("players", {}).get(name) if match else None
        return own["account"] if own else self.names.get(name)

    def of(self, name, without=None):
        """(kills, deaths, minutes) of the account named so, without the match."""
        k, d, minutes = self.sums.get(self.account(name, without), (0, 0, 0.0))
        own = self.reports.get(without, {}).get("players", {}).get(name) if without else None
        if own:
            k, d, minutes = k - own["kills"], d - own["deaths"], minutes - own["seconds"] / 60
        return k, d, max(minutes, 0.0)


def damped_kd(kills, deaths, prior=1.0):
    return (kills + PRIOR_DEATHS * prior) / (deaths + PRIOR_DEATHS)


def kpm_factor(kills, minutes, prior_minutes=None, most=None):
    """The kills a minute against the server's average, damped toward it, at
    most KPM_FACTOR_MOST."""
    pm = PRIOR_MINUTES if prior_minutes is None else prior_minutes
    most = KPM_FACTOR_MOST if most is None else most
    return min((kills + pm * AVERAGE_KPM) / (minutes + pm) / AVERAGE_KPM, most)


def rivals(players):
    """The parties that are never on one side (level_stats::rivals): the two
    strongest players' when they are of two parties (and not tied with a
    third), and a squad that has a strong player with every other party that
    has one. `players`: (party, own strength). A set of frozensets."""
    order = sorted(players, key=lambda p: -p[1])
    pairs = set()
    if len(order) >= 2 and (len(order) == 2 or order[1][1] > order[2][1]) and order[0][0] != order[1][0]:
        pairs.add(frozenset((order[0][0], order[1][0])))
    size = {}
    for party, _ in players:
        size[party] = size.get(party, 0) + 1
    strong = {party for party, s in players if s >= STRONG}
    for q in strong:
        if size[q] >= 2:
            pairs |= {frozenset((q, r)) for r in strong - {q}}
    return pairs


def too_strong(members):
    """A squad that plays no match (level_stats::too_strong): two or more strong."""
    return len(members) >= 2 and sum(1 for s in members if s >= STRONG) >= 2


def stamp_seconds(stamp):
    return datetime.fromisoformat(stamp).replace(tzinfo=timezone.utc).timestamp() if stamp[:1].isdigit() else None


def report_backtest(reports, totals, min_players, prior_minutes=None):
    """Per decided match of the reports: side 0 less side 1, each player by
    its share of the match (its time against the longest anybody's), for the
    K/D alone and for K/D x kpm, both of the player's other matches. Returns
    (kd diffs, kd x kpm diffs, winners)."""
    a, b, ys = [], [], []
    for mid, r in reports.items():
        if r["winner"] not in (0, 1) or r["end"] in ("Deserted", "NoShow") or not r["players"]:
            continue
        longest = max(p["seconds"] for p in r["players"].values())
        if sum(1 for p in r["players"].values() if 2 * p["seconds"] >= longest) < min_players:
            continue
        da = db = 0.0
        for name, p in r["players"].items():
            k, d, minutes = totals.of(name, without=mid)
            kd = damped_kd(k, d)
            sign = (1 if p["team"] == 0 else -1) * p["seconds"] / longest
            da += sign * (PLAYER_KD + kd)
            db += sign * (PLAYER_KD + kd * kpm_factor(k, minutes, prior_minutes))
        a.append(da)
        b.append(db)
        ys.append(1 if r["winner"] == 0 else 0)
    return a, b, ys


def scored(ds, ys):
    k = fit_scale(ds, ys)
    return log_loss([sigmoid(k * d) for d in ds], ys)


def loss_range(a, b, ys, rounds, seed):
    """90 % range of loss(b) - loss(a) over resampled matches."""
    rnd = random.Random(seed)
    n = len(ys)
    diffs = []
    for _ in range(rounds):
        idx = [rnd.randrange(n) for _ in range(n)]
        diffs.append(scored([b[i] for i in idx], [ys[i] for i in idx])
                     - scored([a[i] for i in idx], [ys[i] for i in idx]))
    diffs.sort()
    return (diffs[int(0.05 * rounds)], diffs[int(0.95 * rounds) - 1]) if diffs else (float("nan"),) * 2


def is_squad(p):
    """Whether the log counted the player as a squad's member: the line says
    so (", squad"), or, in lines of before, its count is 1.3 times its K/D."""
    if p.get("squad"):
        return True
    if "kd" not in p or p["kd"] < 0.05:
        return False
    own = p["value"] - PLAYER_KD
    return abs(own - SQUAD_FACTOR * p["kd"]) + 0.006 < abs(own - p["kd"])


class Strengths:
    """Each player of a logged match as the new rule counts it: its logged K/D
    at the match's level times its kpm factor of the reports (without that
    match), 1.3 times for a squad's member; and its party (a squad: its
    members counted as such on one side)."""

    def __init__(self, totals, mid):
        self.totals, self.mid = totals, mid

    def own(self, p):
        k, _, minutes = self.totals.of(p["name"], without=self.mid)
        return p.get("kd", 1.0) * kpm_factor(k, minutes)

    def counted(self, p):
        return self.own(p) * (SQUAD_FACTOR if is_squad(p) else 1.0)

    @staticmethod
    def party(p, team):
        return ("squad", team) if is_squad(p) else ("solo", p["name"])


def strength_backtest(args):
    reports, accounts = read_reports(args.db)
    totals = Totals(reports, accounts)
    kills = sum(s[0] for s in totals.sums.values())
    minutes = sum(s[2] for s in totals.sums.values())
    print(f"Reports: {len(reports)} matches against players in the databases, {len(totals.sums)} players.")
    if not minutes:
        sys.exit("No minutes in the reports: nothing to test.")
    print(f"Kills a minute: {kills} kills in {minutes:.0f} minutes, {kills / minutes:.3f} "
          f"(AVERAGE_KPM in force here {AVERAGE_KPM}).")
    # The prior: how far players' rates are apart against the noise of a minute
    # (kills as a Poisson count: a rate's variance is its mean over the minutes).
    rated = [(s[0], s[2]) for s in totals.sums.values() if s[2] >= 1]
    if rated:
        mean = sum(k for k, _ in rated) / sum(m for _, m in rated)
        between = sum(m * (k / m - mean) ** 2 for k, m in rated) / sum(m for _, m in rated) \
            - mean * len(rated) / sum(m for _, m in rated)
        if between > 0:
            print(f"  players' rates apart: variance {between:.3f}; a prior of mean / variance = "
                  f"{mean / between:.1f} minutes (Poisson noise only, so a floor)")
    regular = sorted(s[0] / s[2] for s in totals.sums.values() if s[2] >= 30)
    if regular:
        q = lambda f: regular[int(f * (len(regular) - 1))]
        print(f"  of {len(regular)} players of 30 minutes or more: median {q(0.5):.2f}, 90th {q(0.9):.2f}, "
              f"99th {q(0.99):.2f}")

    # 1. Which predicts the winners: K/D alone or K/D x kpm.
    a, b, ys = report_backtest(reports, totals, args.min_players)
    if len(ys) >= 8:
        la, lb = scored(a, ys), scored(b, ys)
        lo, hi = loss_range(a, b, ys, args.bootstrap, args.seed)
        print(f"\nWinners of {len(ys)} decided matches of {args.min_players} players or more (each player by its "
              "other matches, by its share of the match):")
        print(f"  K/D alone       log loss {la:.4f}")
        print(f"  K/D x kpm       log loss {lb:.4f}   (difference {lb - la:+.4f}, 90% range {lo:+.4f} .. {hi:+.4f})")
        for pm in (5, 15, 30, 60):
            print(f"    prior {pm:3d} min: {scored(report_backtest(reports, totals, args.min_players, pm)[1], ys):.4f}")
        print(f"  a coin          log loss {math.log(2):.4f}")

    # The logged matches since the K/D rule (their "sides" line names the level).
    ms = [m for m in read_matches(args.logs, args.since, args.until)
          if m["level"] is not None and m["sides"][0] and m["sides"][1]]
    print(f"\nLogged matches against players since the K/D rule: {len(ms)}.")
    if not ms:
        return
    strong_seen, best = [], {}
    changed = dict(top=0, squad=0, any=0)
    refused = {}
    rows_in_force, rows_new, rows_y = [], [], []
    for m in ms:
        st = Strengths(totals, m["id"])
        players = [(Strengths.party(p, team), st.own(p), team, p) for team in (0, 1) for p in m["sides"][team]]
        strong_seen += [own for _, own, _, _ in players]
        for _, own, _, p in players:
            best[p["name"]] = max(best.get(p["name"], 0.0), own)
        pairs = rivals([(party, own) for party, own, _, _ in players])
        side_of = {party: team for party, _, team, _ in players}
        bad = [pair for pair in pairs if len({side_of[x] for x in pair}) == 1]
        top = sorted(players, key=lambda x: -x[1])
        top_bad = len(top) >= 2 and top[0][0] != top[1][0] and frozenset((top[0][0], top[1][0])) in bad
        changed["top"] += bool(top_bad)
        changed["squad"] += any(pair for pair in bad if pair != frozenset((top[0][0], top[1][0])))
        changed["any"] += bool(bad)
        for team in (0, 1):
            squad = [(p["name"], own) for party, own, t, p in players if t == team and party[0] == "squad"]
            if too_strong([own for _, own in squad]):
                refused.setdefault(tuple(sorted(name for name, _ in squad)), []).append(m["id"])
        # The winners by who was in each minute: as logged, and as the new rule counts.
        if left_out(m, args.min_players) is None:
            snapshots, _ = rosters(m)
            if snapshots:
                old = sum(sum(p["value"] for p in s[0]) - sum(p["value"] for p in s[1]) for s in snapshots)
                new = sum(sum(PLAYER_KD + st.counted(p) for p in s[0])
                          - sum(PLAYER_KD + st.counted(p) for p in s[1]) for s in snapshots)
                rows_in_force.append(old / len(snapshots))
                rows_new.append(new / len(snapshots))
                rows_y.append(1 if m["winner"] == 0 else 0)
    n = len(strong_seen)
    print(f"  players at the start: {n}; strong (>= {STRONG}) {sum(s >= STRONG for s in strong_seen)} "
          f"({sum(s >= STRONG for s in strong_seen) / n:.0%}); at other cuts: "
          + ", ".join(f">= {c} {sum(s >= c for s in strong_seen) / n:.0%}" for c in (1.5, 2.5, 3.0, 4.0)))
    print(f"  players: {len(best)}; strong in a match or more {sum(s >= STRONG for s in best.values())} "
          f"({sum(s >= STRONG for s in best.values()) / len(best):.0%}); at other cuts: "
          + ", ".join(f">= {c} {sum(s >= c for s in best.values()) / len(best):.0%}" for c in (1.5, 2.5, 3.0, 4.0)))
    print(f"  sides the new rule would have changed: {changed['any']} of {len(ms)} "
          f"(the two strongest together {changed['top']}, a strong player beside a squad with one "
          f"{changed['squad']})")
    print(f"  squads too strong to play (two strong or more): {len(refused)} squads, in "
          f"{sum(len(v) for v in refused.values())} matches")
    if len(rows_y) >= 8:
        la, lb = scored(rows_in_force, rows_y), scored(rows_new, rows_y)
        lo, hi = loss_range(rows_in_force, rows_new, rows_y, args.bootstrap, args.seed)
        print(f"  winners of {len(rows_y)} of them by who was in each minute: in force {la:.4f}, "
              f"new {lb:.4f} (difference {lb - la:+.4f}, 90% range {lo:+.4f} .. {hi:+.4f})")

    # 2. The late joiners, put on the weaker side only.
    late_backtest(ms, totals, args.room_size)

    # 3. Named players, on request only.
    if args.show or args.clan:
        named_players(ms, totals, reports, args)


def late_backtest(ms, totals, room_size):
    """Each logged late join again by the new rule: the weaker side now, with a
    seat (the room's size) and no rival there; else it waits, until a later
    minute's sides let it in (the sides as they really went on)."""
    at_once = other_side = waited = to_end = 0
    waits = []
    for m in ms:
        st = Strengths(totals, m["id"])
        roster = [list(m["sides"][0]), list(m["sides"][1])]
        out, lead = [], (0.0, 0.0)
        events = m["events"]
        for i, (kind, what) in enumerate(events):
            if kind == "minute":
                out = split_names(what["out"], [p["name"] for side in roster for p in side]) or []
                lead = what["lead"]
                continue
            joiners = what
            team = joiners[0]["team"]
            mate = len(joiners) == 1 and is_squad(joiners[0])

            def allowed(roster, out, lead):
                now = [[p for p in side if p["name"] not in out] for side in roster]
                worth = [sum(PLAYER_KD + st.counted(p) for p in now[t]) + lead[t] for t in (0, 1)]
                weaker = 1 if worth[1] < worth[0] else 0
                if len(now[weaker]) + len(joiners) > room_size or (mate and weaker != team):
                    return None
                party = ("join", joiners[0]["name"]) if not mate else ("squad", team)
                players = [(Strengths.party(p, t), st.own(p), t) for t in (0, 1) for p in now[t]]
                players += [(party, st.own(p), weaker) for p in joiners]
                for pair in rivals([(x, s) for x, s, _ in players]):
                    if party in pair:
                        (other,) = pair - {party}
                        if any(x == other and t == weaker for x, _, t in players):
                            return None
                return weaker

            side = allowed(roster, out, lead)
            if side is not None:
                at_once += 1
                other_side += side != team
            else:
                waited += 1
                start = stamp_seconds(joiners[0]["at"])
                later_roster = [list(s) for s in roster]
                found = None
                for kind2, what2 in events[i + 1:]:
                    if kind2 == "join":
                        for p in what2:
                            later_roster[p["team"]].append(p)
                        continue
                    names = [p["name"] for s in later_roster for p in s]
                    o = split_names(what2["out"], names) or []
                    if allowed(later_roster, o, what2["lead"]) is not None:
                        found = stamp_seconds(what2["at"])
                        break
                if found is not None and start is not None:
                    waits.append(found - start)
                else:
                    to_end += 1
            for p in joiners:
                roster[p["team"]].append(p)
    total = at_once + waited
    print(f"\nLate joins: {total}; by the new rule {at_once} in at once ({other_side} of them on the other side "
          f"than they went), {waited} would have waited ({len(waits)} let in later, {to_end} to the match's end)")
    if waits:
        waits.sort()
        print(f"  waits: median {waits[len(waits) // 2] / 60:.1f} min, longest {waits[-1] / 60:.1f} min")


def named_players(ms, totals, reports, args):
    """Where the players named by --show / --clan land: K/D and kpm, the
    strength, its rank among the players of 30 minutes or more, and their
    logged K/D at each level."""
    pattern = re.compile(args.show) if args.show else None
    named = {account: name for name, account in totals.names.items()}
    # Each account's latest logged K/D at each level.
    at_levels = {}
    for m in ms:
        for side in m["sides"]:
            for p in side:
                if "kd" in p:
                    at_levels.setdefault(totals.account(p["name"], m["id"]), {})[m["level"]] = p["kd"]

    def of(account):
        k, d, minutes = totals.sums[account]
        return k, d, minutes, damped_kd(k, d), kpm_factor(k, minutes)

    ranked = sorted((a for a, s in totals.sums.items() if s[2] >= 30), key=lambda a: -of(a)[3] * of(a)[4])
    rank = {a: i + 1 for i, a in enumerate(ranked)}
    print(f"\nNamed players (rank of {len(ranked)} with 30 minutes or more; K/D and kpm of the reports, "
          f"strength = K/D x kpm factor (factor at most {KPM_FACTOR_MOST}); at L: the logged K/D there x the factor):")
    shown = [a for a in totals.sums if (pattern and pattern.search(named.get(a, "")))
             or (args.clan and totals.clans.get(a) == args.clan)]
    for account in sorted(shown, key=lambda a: -of(a)[3] * of(a)[4]):
        k, d, minutes, kd, f = of(account)
        levels = ", ".join(f"{lv}: {at:.2f} -> {at * f:.2f}{' strong' if at * f >= STRONG else ''}"
                           for lv, at in sorted(at_levels.get(account, {}).items()))
        print(f"  {named.get(account, account)} [{totals.clans.get(account, '')}]: K/D {kd:.2f} ({k}/{d}), "
              f"kpm {k / max(minutes, 1e-9):.2f} in {minutes:.0f} min, factor {f:.2f}; strength {kd * f:.2f}"
              f"{' STRONG' if kd * f >= STRONG else ''}, rank {rank.get(account, '-')}; at L {levels or '-'}")


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
    ap.add_argument("--db", action="append", default=[],
                    help="srp databases (copies of srp.db, or a directory of them): test the strength rule "
                         "(kd-balance.md) on their match reports and the logs instead of fitting the weights")
    ap.add_argument("--room-size", type=int, default=8, help="with --db: a side's most players (8)")
    ap.add_argument("--show", default="", help="with --db: print the players whose name matches this regex")
    ap.add_argument("--clan", default="", help="with --db: print the players of this clan tag")
    args = ap.parse_args()
    if args.db:
        strength_backtest(args)
        return

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
