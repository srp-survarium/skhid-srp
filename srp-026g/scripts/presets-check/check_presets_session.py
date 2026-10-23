"""Did each lobby client of a test session get its kit presets list?

USAGE

    python check_presets_session.py <packet log folder> [--within <s>] [--since <time>]

    <packet log folder>  what SRP_PACKET_LOG was set to (one file a
                         connection, crates/udp-transport packet_log.rs;
                         the local game's runtime/packets).
    --within <s>         how soon after the connection's first datagram the
                         list must have come (default 10).
    --since <time>       only connections that began at or after it (UTC,
                         ISO 8601, e.g. 2026-10-23T00:49): a session's own,
                         not older ones of a build without presets.

The Character panel's title names the active preset only once the lobby
movie has the list (docs/notes/presets.md, lobby.md 2.16.4): the movie
asks with a `3B` ranking request at 2 090 000 000 and the server answers
with `91` rows, the first one's nickname `P1;L;...`. A movie whose first
ask is lost (sent while the game had not installed its handler yet) shows
"Character" and takes no press: 2026-10-23, both builds of #314.

For each lobby connection it prints when the first list ask went out, when
the first list came back and when the first choice (2 090 000 011..15)
went out, and FAILS (exit 1) a connection where no list came within
--within seconds, or where a choice went out before any list was asked
(the old dropdown's case: it worked without the list). Connections shorter
than --within are skipped. Only reads files.
"""

import datetime
import os
import re
import sys

RAW = re.compile(r"^(\S+) (IN |OUT) \S+ len=\d+((?: [0-9A-F*]{2})*)\s*$")

# 2 090 000 000 little-endian, after the 3B id and its 2-byte field.
LIST = "80 DE 92 7C"
ASK = re.compile(r"\b3B [0-9A-F]{2} [0-9A-F]{2} " + LIST)
CHOOSE = re.compile(r"\b3B [0-9A-F]{2} [0-9A-F]{2} 8[B-F] DE 92 7C")
# "P1;L;" in a 91 row's nickname.
ANSWER = "50 31 3B 4C 3B"


def when(stamp):
    return datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00"))


def connection(path):
    first = last = asked = answered = chose = None
    with open(path, encoding="utf-8", errors="replace") as lines:
        for line in lines:
            m = RAW.match(line)
            if not m:
                continue
            at, direction, data = when(m.group(1)), m.group(2).strip(), m.group(3)
            first = first or at
            last = at
            if direction == "IN":
                if asked is None and ASK.search(data):
                    asked = at
                if chose is None and CHOOSE.search(data):
                    chose = at
            elif answered is None and ANSWER in data:
                answered = at
    return first, last, asked, answered, chose


def main(argv):
    if not argv or argv[0].startswith("-"):
        print(__doc__)
        return 2
    folder, within, since_time = argv[0], 10.0, None
    if "--within" in argv:
        within = float(argv[argv.index("--within") + 1])
    if "--since" in argv:
        since_time = when(argv[argv.index("--since") + 1])
        if since_time.tzinfo is None:
            since_time = since_time.replace(tzinfo=datetime.timezone.utc)
    failed = checked = 0
    for name in sorted(os.listdir(folder)):
        if not name.startswith("lobby-"):
            continue
        first, last, asked, answered, chose = connection(os.path.join(folder, name))
        if first is None or (last - first).total_seconds() < within:
            continue
        if since_time is not None and first < since_time:
            continue
        checked += 1

        def since(t):
            return "never" if t is None else "%.1f s" % (t - first).total_seconds()

        problems = []
        if answered is None or (answered - first).total_seconds() > within:
            problems.append("no list within %g s" % within)
        if chose is not None and (asked is None or chose < asked):
            problems.append("a choice before any list was asked")
        failed += bool(problems)
        print(
            "%s %s: asked %s, list %s, first choice %s%s"
            % ("FAIL" if problems else "ok  ", name, since(asked), since(answered), since(chose),
               "; " + ", ".join(problems) if problems else "")
        )
    print("%d lobby connection(s) checked, %d failed" % (checked, failed))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
