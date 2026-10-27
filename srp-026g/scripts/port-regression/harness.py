#!/usr/bin/env python3
"""Helpers of run.sh (the port's whole-game regression, README.txt): the
owner's-game check, ports, prefixes, the game folder, the server's
environment, and every comparison and summary. Nothing here starts a game,
a display or a server: run.sh does, and only after its checks."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import sys

GAME_EXE = re.compile(r"survarium.*\.exe$", re.IGNORECASE)
MARKER = ".srp-port-regression"


# ---- the owner's game ---------------------------------------------------------

def process_start(pid: int, proc: Path = Path("/proc")) -> str | None:
    """Field 22 of /proc/<pid>/stat (start time), None when gone or a zombie;
    run.py's rule, so its state.json records compare the same way."""
    try:
        fields = (proc / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
        return None if fields[0] == "Z" else fields[19]
    except (OSError, IndexError):
        return None


def is_game_command(argv: list[str]) -> bool:
    """A game process: its program is a survarium*.exe (Wine shows the
    Windows program as argv[0]), or a wine launcher whose first arguments
    name one. A shell or a python whose arguments merely mention an exe
    (this harness's own command line) is not."""
    if not argv:
        return False
    name = lambda arg: re.split(r"[\\/]", arg)[-1]
    if GAME_EXE.search(name(argv[0])):
        return True
    launcher = name(argv[0]).lstrip(".").lower()
    if launcher.startswith("wine") or launcher in ("start.exe", "explorer.exe"):
        return any(GAME_EXE.search(name(arg)) for arg in argv[1:4])
    return False


def owner_games(proc: Path, ours: str | None, states: list[Path]) -> list[str]:
    """Why the machine is busy: a run local-clients recorded (its server or
    a client still alive), or a game process this harness did not start
    (its WINEPREFIX is not under `ours`)."""
    reasons = []
    for state_file in states:
        try:
            state = json.loads(state_file.read_text())
        except (OSError, ValueError):
            continue
        records = [("server", state.get("server"))]
        records += [(f"client {int(k) + 1}", v) for k, v in (state.get("clients") or {}).items()]
        for label, record in records:
            if record and record.get("pid") and record.get("started") is not None \
                    and process_start(record["pid"], proc) == record["started"]:
                reasons.append(f"{state_file}: the {label} of a local-clients run is alive (pid {record['pid']})")
    for entry in proc.iterdir() if proc.is_dir() else []:
        if not entry.name.isdecimal():
            continue
        try:
            argv = (entry / "cmdline").read_bytes().split(b"\0")
            argv = [a.decode(errors="replace") for a in argv if a]
            if not is_game_command(argv):
                continue
            environ = (entry / "environ").read_bytes().split(b"\0")
        except OSError:
            continue
        prefix = next((e[len(b"WINEPREFIX="):].decode(errors="replace") for e in environ
                       if e.startswith(b"WINEPREFIX=")), "")
        if ours and prefix and (prefix == ours or prefix.startswith(ours.rstrip("/") + "/")):
            continue
        reasons.append(f"pid {entry.name}: a game not started by this harness ({' '.join(argv[:2])[:120]})")
    return reasons


# ---- ports, display, prefixes, the game folder ---------------------------------

def port_free(port: int, udp: bool) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM if udp else socket.SOCK_STREAM) as sock:
        try:
            sock.bind(("127.0.0.1", port))
            return True
        except OSError:
            return False


def free_ports(base: int) -> list[int]:
    """Login TCP, lobby UDP, match UDP, web TCP: four free ports from `base`
    on, in steps of 10 (other sessions hold 2234, 3234, 4234, ...)."""
    for start in range(base, 65000, 10):
        ports = [start, start + 1, start + 2, start + 3]
        if all(port_free(p, udp) for p, udp in zip(ports, (False, True, True, False))):
            return ports
    raise SystemExit("no four free ports")


def free_display(first: int, tmp: Path = Path("/tmp")) -> int:
    for number in range(first, first + 100):
        if not (tmp / ".X11-unix" / f"X{number}").exists() and not (tmp / f".X{number}-lock").exists():
            return number
    raise SystemExit("no free X display number")


def user_cfg(lines: list[str], resolution: str) -> list[str]:
    """run.py's user.cfg rules: the managed keys out, then ours. The
    password goes on the command line, never here (account_digest is
    machine-key XORed)."""
    managed = {"account_name", "account_password", "account_digest", "r_fullscreen", "r_resolution",
               "store_user_password"}
    kept = [line for line in lines if (line.split(maxsplit=1) or [""])[0] not in managed]
    return kept + ["store_user_password on", "r_fullscreen off", f"r_resolution {resolution}"]


def prepare_prefix(template: Path, prefix: Path, resolution: str) -> None:
    if prefix.exists():
        if not (prefix / MARKER).is_file():
            raise SystemExit(f"refusing to use a prefix this harness did not make: {prefix}")
    else:
        if not (template / "system.reg").is_file():
            raise SystemExit(f"the prefix template has no system.reg: {template}")
        staging = prefix.with_name(prefix.name + ".staging")
        if staging.exists():
            shutil.rmtree(staging)
        subprocess.run(["cp", "-a", "--reflink=auto", str(template), str(staging)], check=True)
        (staging / MARKER).write_text("port-regression\n")
        staging.rename(prefix)
    for user in (prefix / "drive_c/users").iterdir():
        if not user.is_dir() or user.name in ("Public", "All Users"):
            continue
        for name in ("Documents", "My Documents"):
            documents = user / name
            if documents.is_symlink():
                documents.unlink()
            game = documents / "Survarium"
            game.mkdir(parents=True, exist_ok=True)
            cfg = game / "user.cfg"
            lines = cfg.read_text().splitlines() if cfg.exists() else []
            cfg.write_text("\n".join(user_cfg(lines, resolution)) + "\n")


def game_documents(prefix: Path) -> list[Path]:
    return sorted(p for p in prefix.glob("drive_c/users/*/Documents/Survarium") if p.is_dir())


def clear_documents(prefix: Path) -> None:
    """A run's outputs out of the prefix, so the next run's are its own."""
    for folder in game_documents(prefix):
        for item in folder.iterdir():
            if item.is_file() and (item.suffix in (".log", ".csv", ".journal")):
                item.unlink()


def game_farm(game: Path, out: Path, exes: dict[str, Path]) -> None:
    """The game folder for this run: every top-level entry of the game a
    symlink (resources.db is 6 GB), binaries/x86 a real folder with the
    DLLs copied and our exes as survarium_<label>.exe. The crash reports
    (bugtrap's zips, beside the exe) so land in our folder."""
    x86 = out / "binaries" / "x86"
    x86.mkdir(parents=True, exist_ok=True)
    for entry in game.iterdir():
        if entry.name == "binaries":
            continue
        link = out / entry.name
        if not link.exists() and not link.is_symlink():
            link.symlink_to(entry)
    for dll in (game / "binaries" / "x86").glob("*.dll"):
        shutil.copy2(dll, x86 / dll.name)
    for label, exe in exes.items():
        shutil.copy2(exe, x86 / f"survarium_{label}.exe")


# ---- the configuration and the server's environment ------------------------------

def read_config(path: Path) -> dict:
    data = json.loads(path.read_text())
    resolve = lambda value: str((path.parent / value).resolve())
    server = data.get("server") or {}
    return {
        "game_dir": resolve(data["game_dir"]),
        "wine": resolve(data["wine"]),
        "wineserver": resolve(data["wineserver"]),
        "prefix_template": resolve(data["prefix_template"]),
        "resolution": data.get("resolution", "1280x720"),
        "client_env": data.get("client_env", {}),
        "server_binary": resolve(server["binary"]) if server.get("binary") else "",
        "server_working_dir": resolve(server["working_dir"]) if server.get("working_dir") else "",
        "server_env": server.get("env", {}),
    }


def server_env(base: dict[str, str], config_env: dict[str, str], work: Path, ports: list[int],
               settings: dict[str, str]) -> dict[str, str]:
    """run.py's start(): the inherited environment (no SRP_* of the shell),
    the configuration's server.env, then the loopback, data and log
    settings, which win; then this match's settings."""
    env = {k: v for k, v in base.items() if not k.startswith("SRP_")}
    env.update(config_env)
    for key in ("SRP_ADMIN_TOKEN", "SRP_ADMIN_PATH", "SRP_ADMIN_IPS"):
        env.pop(key, None)
    env.update(SRP_DATA_DIR=str(work / "data"), SRP_DB="on", SRP_REGISTRATION="open",
               SRP_REGISTRATIONS_PER_DAY="100", SRP_TUTORIAL="on", SRP_LOG_FILES="off",
               SRP_BIND_HOST="127.0.0.1", SRP_PUBLIC_HOST="127.0.0.1", SRP_WEB_BIND="127.0.0.1",
               SRP_LOCAL_PEERS="on", SRP_PACKET_LOG=str(work / "packets"),
               SRP_SNAPSHOT_DIR=str(work / "snapshots"))
    for service, port in zip(("LOGIN", "LOBBY", "MATCH", "WEB"), ports):
        env[f"SRP_{service}_PORT"] = str(port)
        env[f"SRP_{service}_BIND"] = "127.0.0.1"
        env[f"SRP_{service}_PUBLIC_HOST"] = "127.0.0.1"
    env.update({k: v for k, v in settings.items() if v != ""})
    return env


# ---- logs ---------------------------------------------------------------------

SERVER_BAD = {
    "out_of_sync": re.compile(r"is out of sync.*resyncing it"),
    "disagree": re.compile(r"the clients disagree at \d+ ms"),
    "sync_again": re.compile(r"'s sync is done again"),
    "panic": re.compile(r"panicked at|RUST_BACKTRACE"),
    "server_error": re.compile(r"\[ERROR\]"),
}
SERVER_INFO = {
    "matches_started": re.compile(r"gave the initial world"),
    "matches_over": re.compile(r": over at \d+ ms"),
    "deserted": re.compile(r"settled \(Deserted|only bots for \d+ s; it is over|everyone left at"),
}


def scan_server(text: str) -> dict[str, int]:
    counts = {name: 0 for name in list(SERVER_BAD) + list(SERVER_INFO)}
    for line in text.splitlines():
        for name, pattern in {**SERVER_BAD, **SERVER_INFO}.items():
            if pattern.search(line):
                counts[name] += 1
    return counts


WINE_CRASH = re.compile(r"Unhandled exception|Unhandled page fault|EXCEPTION_[A-Z_]+|err:seh:|wine: Assertion")
GAME_FATAL = re.compile(r"<FATAL>|debug::terminate|Replay of other engine version|fatal error", re.IGNORECASE)


def error_kinds(text: str) -> set[str]:
    """The game log's <ERROR>/<FATAL> lines as kinds: the channel and the
    message with times, numbers and hex taken out, so two runs compare."""
    kinds = set()
    for line in text.splitlines():
        match = re.match(r"^(\S+)\s+<(ERROR|FATAL)>\s+\[[^\]]*\]\s*(.*)$", line)
        if match:
            message = re.sub(r"0x[0-9a-fA-F]+|\d+", "#", match.group(3)).strip()
            kinds.add(f"{match.group(1)} {match.group(2)}: {message}")
    return kinds


def scan_client(wine_log: str, game_log: str, crash_reports: int) -> dict[str, object]:
    return {
        "wine_crash_lines": sum(1 for line in wine_log.splitlines() if WINE_CRASH.search(line)),
        "fatal_lines": sum(1 for line in game_log.splitlines() if GAME_FATAL.search(line)),
        "crash_reports": crash_reports,
        "resync_lines": sum(1 for line in game_log.splitlines() if "FULL RESYNC" in line),
    }


# ---- world hashes ----------------------------------------------------------------

DATAGRAM = re.compile(r"^(\S+)\s+(IN|OUT)\s+\S+\s+len=\d+\s+((?:[0-9A-F]{2}\s?)+)$")
MESSAGE = re.compile(r"^\s+msg id=([0-9A-F]{2}) len=(\d+) at=(\d+)")


def packet_hash_reports(text: str) -> list[tuple[str, int, int]]:
    """The 0xDF hash reports a client sent, from one peer's packet log
    (SRP_PACKET_LOG): (arrival time, tick mod 256, CRC) in order. 0xDF is
    `u8 tick, u32 crc` (match.md, the message table)."""
    reports, datagram = [], None
    for line in text.splitlines():
        head = DATAGRAM.match(line)
        if head:
            datagram = (head.group(1), bytes.fromhex(head.group(3))) if head.group(2) == "IN" else None
            continue
        message = MESSAGE.match(line)
        if message and datagram and message.group(1) == "DF" and int(message.group(2)) == 5:
            at = int(message.group(3))
            body = datagram[1][at:at + 5]
            if len(body) == 5:
                reports.append((datagram[0], body[0], int.from_bytes(body[1:5], "little")))
    return reports


def seconds_of(stamp: str) -> float:
    """2026-10-26T12:29:51.109Z -> seconds of the day (a match is never
    long enough for the day to matter more than once; see pair_reports)."""
    clock = stamp.split("T", 1)[1].rstrip("Z")
    h, m, s = clock.split(":")
    return int(h) * 3600 + int(m) * 60 + float(s)


def pair_reports(a: list[tuple[str, int, int]], b: list[tuple[str, int, int]],
                 window: float = 6.0) -> dict[str, object]:
    """Two clients' reports of the same ticks: a tick is mod 256 (12.8 s),
    so a report of `a` pairs with the report of `b` of the same tick whose
    arrival is within `window` seconds (under half the wrap). Returns how
    many paired, how many CRCs differ, and the first difference."""
    gap = lambda t, u: abs(((u - t + 43200) % 86400) - 43200)

    def unique(reports):
        """A report sent again (the same tick within the window) counts once."""
        kept: list[tuple[str, float, int, int]] = []
        last: dict[int, float] = {}
        for stamp, tick, crc in reports:
            t = seconds_of(stamp)
            if tick in last and gap(last[tick], t) <= window:
                continue
            last[tick] = t
            kept.append((stamp, t, tick, crc))
        return kept

    index: dict[int, list[tuple[float, int]]] = {}
    for _, u, tick, crc in unique(b):
        index.setdefault(tick, []).append((u, crc))
    paired = differ = 0
    first = None
    for stamp, t, tick, crc in unique(a):
        candidates = [(gap(t, u), c) for u, c in index.get(tick, [])]
        candidates = [x for x in candidates if x[0] <= window]
        if not candidates:
            continue
        paired += 1
        other = min(candidates)[1]
        if other != crc:
            differ += 1
            if first is None:
                first = f"tick {tick} at {stamp}: {crc:08x} vs {other:08x}"
    return {"paired": paired, "differ": differ, "first": first}


def read_hash_csv(text: str) -> list[tuple[int, int]]:
    """`time_ms,crc` rows (crc hex or decimal); # lines and a header skipped."""
    rows = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = [p.strip() for p in line.split(",")]
        if len(parts) < 2 or not parts[0].isdecimal():
            continue
        crc = int(parts[1], 16) if re.fullmatch(r"(0x)?[0-9a-fA-F]+", parts[1]) and not parts[1].isdecimal() \
            else int(parts[1])
        rows.append((int(parts[0]), crc))
    return rows


def compare_hashes(old: list[tuple[int, int]], new: list[tuple[int, int]]) -> dict[str, object]:
    """Tick by tick: every tick both have must have one CRC; the ticks must
    be the same ones (a run cut short shows as missing ticks)."""
    a, b = dict(old), dict(new)
    common = sorted(set(a) & set(b))
    differ = [t for t in common if a[t] != b[t]]
    return {"common": len(common), "only_old": len(set(a) - set(b)), "only_new": len(set(b) - set(a)),
            "differ": len(differ),
            "first": f"tick {differ[0]} ms: {a[differ[0]]:08x} vs {b[differ[0]]:08x}" if differ else None}


# ---- frame times -------------------------------------------------------------------

def frame_times(text: str, states: set[int]) -> list[int]:
    """frame_us of the frames in `states` from srp_frames.csv (patch 71's
    format 1: # lines, a header line per game start, a row a frame)."""
    columns, values = None, []
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        cells = line.split(",")
        if cells[0] == "frame":
            columns = {name: i for i, name in enumerate(cells)}
            continue
        if not columns:
            continue
        try:
            state = int(cells[columns["state"]] or 0)
            us = int(cells[columns["frame_us"]] or 0)
        except (IndexError, ValueError, KeyError):
            continue
        if state in states:
            values.append(us)
    return values


def percentile(sorted_values: list[int], p: float) -> float:
    """Nearest rank."""
    if not sorted_values:
        return 0.0
    rank = max(1, -(-len(sorted_values) * p // 100))
    return float(sorted_values[int(rank) - 1])


def frame_stats(values: list[int]) -> dict[str, float]:
    s = sorted(values)
    return {"frames": len(s), "p50": percentile(s, 50), "p95": percentile(s, 95),
            "p99": percentile(s, 99), "max": float(s[-1]) if s else 0.0}


def compare_frames(old: dict[str, float], new: dict[str, float], threshold: float,
                   max_threshold: float, slack_us: float = 500.0, least: int = 200) -> tuple[str, str]:
    """PASS unless a percentile of the new exe is above the old one's by
    more than `threshold` (a fraction) and `slack_us`; the longest frame by
    more than `max_threshold`. Too few frames in either: SKIPPED."""
    if old["frames"] < least or new["frames"] < least:
        return "SKIPPED", f"too few frames (old {old['frames']}, new {new['frames']}; need {least})"
    worse = []
    for key, limit in (("p50", threshold), ("p95", threshold), ("p99", threshold), ("max", max_threshold)):
        if new[key] > old[key] * (1 + limit) + slack_us:
            worse.append(f"{key} {old[key] / 1000:.1f}->{new[key] / 1000:.1f} ms")
    text = " ".join(f"{k} {old[k] / 1000:.1f}/{new[k] / 1000:.1f}" for k in ("p50", "p95", "p99", "max"))
    return ("FAIL", "regression: " + ", ".join(worse) + f" ({text} ms old/new)") if worse \
        else ("PASS", f"{text} ms old/new, {old['frames']}/{new['frames']} frames")


# ---- the summary -------------------------------------------------------------------

def summary(rows: list[tuple[str, str, str]]) -> tuple[str, int]:
    """The table and the exit code: 0 only when every row passed (a dry
    run's PLANNED rows count as passed; SKIPPED does not)."""
    width = max([len(r[0]) for r in rows] + [4])
    lines = [f"{'test'.ljust(width)}  status   detail", f"{'-' * width}  -------  ------"]
    lines += [f"{t.ljust(width)}  {s.ljust(7)}  {d}" for t, s, d in rows]
    counts = {}
    for _, status, _ in rows:
        counts[status] = counts.get(status, 0) + 1
    lines.append("")
    lines.append(", ".join(f"{n} {s}" for s, n in sorted(counts.items())) or "no tests")
    ok = bool(rows) and all(s in ("PASS", "PLANNED") for _, s, _ in rows)
    lines.append("RESULT: " + ("PASS" if ok else "FAIL (or skipped: not proven)"))
    return "\n".join(lines), 0 if ok else 1


def read_results(path: Path) -> list[tuple[str, str, str]]:
    rows = []
    if path.exists():
        for line in path.read_text().splitlines():
            parts = line.split("\t", 2)
            if len(parts) == 3:
                rows.append((parts[0], parts[1], parts[2]))
    return rows


# ---- the command line --------------------------------------------------------------

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("busy"); p.add_argument("--ours", default=""); p.add_argument("--state", action="append", default=[])
    p = sub.add_parser("free-ports"); p.add_argument("--base", type=int, default=52234)
    p = sub.add_parser("free-display"); p.add_argument("--first", type=int, default=120)
    p = sub.add_parser("config"); p.add_argument("config", type=Path); p.add_argument("key")
    p = sub.add_parser("prepare-prefix"); p.add_argument("--template", type=Path, required=True)
    p.add_argument("--prefix", type=Path, required=True); p.add_argument("--resolution", default="1280x720")
    p = sub.add_parser("clear-documents"); p.add_argument("prefix", type=Path)
    p = sub.add_parser("documents"); p.add_argument("prefix", type=Path)
    p = sub.add_parser("game-farm"); p.add_argument("--game", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True); p.add_argument("exe", nargs="+", help="label=path")
    p = sub.add_parser("server-env"); p.add_argument("--config", type=Path, required=True)
    p.add_argument("--work", type=Path, required=True); p.add_argument("--ports", required=True)
    p.add_argument("--set", action="append", default=[], help="KEY=VALUE (empty VALUE: leave unset)")
    p = sub.add_parser("password")
    p = sub.add_parser("has-frames"); p.add_argument("exe", type=Path)
    p = sub.add_parser("scan-server"); p.add_argument("log", type=Path)
    p = sub.add_parser("scan-client"); p.add_argument("--wine-log", type=Path, required=True)
    p.add_argument("--game-log", type=Path, nargs="*", default=[]); p.add_argument("--crash-reports", type=int, default=0)
    p = sub.add_parser("error-kinds-new"); p.add_argument("--old", type=Path, nargs="*", default=[])
    p.add_argument("--new", type=Path, nargs="*", default=[])
    p = sub.add_parser("compare-df"); p.add_argument("packets", type=Path)
    p = sub.add_parser("compare-hashes"); p.add_argument("old", type=Path); p.add_argument("new", type=Path)
    p = sub.add_parser("frames-compare"); p.add_argument("old", type=Path); p.add_argument("new", type=Path)
    p.add_argument("--states", default="6"); p.add_argument("--threshold", type=float, default=0.10)
    p.add_argument("--max-threshold", type=float, default=1.0)
    p = sub.add_parser("summary"); p.add_argument("results", type=Path)
    args = parser.parse_args(argv)

    if args.command == "busy":
        reasons = owner_games(Path("/proc"), args.ours or None, [Path(s) for s in args.state])
        print("\n".join(reasons))
        return 1 if reasons else 0
    if args.command == "free-ports":
        print(",".join(map(str, free_ports(args.base))))
    elif args.command == "free-display":
        print(free_display(args.first))
    elif args.command == "config":
        value = read_config(args.config)[args.key]
        if isinstance(value, dict):
            sys.stdout.write("".join(f"{k}={v}\0" for k, v in value.items()))
        else:
            print(value)
    elif args.command == "prepare-prefix":
        prepare_prefix(args.template, args.prefix, args.resolution)
    elif args.command == "clear-documents":
        clear_documents(args.prefix)
    elif args.command == "documents":
        print("\n".join(map(str, game_documents(args.prefix))))
    elif args.command == "game-farm":
        exes = dict(item.split("=", 1) for item in args.exe)
        game_farm(args.game, args.out, {k: Path(v) for k, v in exes.items()})
    elif args.command == "server-env":
        config = read_config(args.config)
        settings = dict(item.split("=", 1) for item in args.set)
        env = server_env(dict(os.environ), config["server_env"], args.work,
                         [int(p) for p in args.ports.split(",")], settings)
        sys.stdout.write("".join(f"{k}={v}\0" for k, v in env.items()))
    elif args.command == "password":
        print("PortReg" + secrets.token_hex(8))
    elif args.command == "has-frames":
        data = args.exe.read_bytes()
        return 0 if b"srp_frames" in data or "srp_frames".encode("utf-16-le") in data else 1
    elif args.command == "scan-server":
        counts = scan_server(args.log.read_text(errors="replace") if args.log.exists() else "")
        print(" ".join(f"{k}={v}" for k, v in counts.items()))
        return 1 if any(counts[k] for k in SERVER_BAD) else 0
    elif args.command == "scan-client":
        wine = args.wine_log.read_text(errors="replace") if args.wine_log.exists() else ""
        game = "".join(p.read_text(errors="replace") for p in args.game_log if p.exists())
        result = scan_client(wine, game, args.crash_reports)
        print(" ".join(f"{k}={v}" for k, v in result.items()))
        return 1 if result["wine_crash_lines"] or result["fatal_lines"] or result["crash_reports"] else 0
    elif args.command == "error-kinds-new":
        read = lambda paths: "".join(p.read_text(errors="replace") for p in paths if p.is_file())
        added = sorted(error_kinds(read(args.new)) - error_kinds(read(args.old)))
        print("\n".join(added))
        return 1 if added else 0
    elif args.command == "compare-df":
        peers = {}
        for log in sorted(args.packets.glob("match-*.log")):
            reports = packet_hash_reports(log.read_text(errors="replace"))
            if reports:
                peers[log.name] = reports
        if len(peers) != 2:
            print(f"peers_with_hashes={len(peers)} (need 2 real clients to compare)")
            return 2
        (na, a), (nb, b) = peers.items()
        result = pair_reports(a, b)
        print(f"paired={result['paired']} differ={result['differ']} first={result['first']} ({na} vs {nb})")
        return 1 if result["differ"] or not result["paired"] else 0
    elif args.command == "compare-hashes":
        result = compare_hashes(read_hash_csv(args.old.read_text()), read_hash_csv(args.new.read_text()))
        print(" ".join(f"{k}={v}" for k, v in result.items()))
        bad = result["differ"] or result["only_old"] or result["only_new"] or not result["common"]
        return 1 if bad else 0
    elif args.command == "frames-compare":
        states = {int(s) for s in args.states.split(",")}
        old = frame_stats(frame_times(args.old.read_text(errors="replace"), states))
        new = frame_stats(frame_times(args.new.read_text(errors="replace"), states))
        status, detail = compare_frames(old, new, args.threshold, args.max_threshold)
        print(f"{status}\t{detail}")
        return {"PASS": 0, "FAIL": 1}.get(status, 2)
    elif args.command == "summary":
        text, code = summary(read_results(args.results))
        print(text)
        return code
    return 0


if __name__ == "__main__":
    sys.exit(main())
