#!/usr/bin/env python3
"""Nineteen stationary native clients for the existing local Wine setup."""
import argparse
import csv
import json
import math
import os
from pathlib import Path
import shutil
import sys
import time

NAMES = [f"Spawn{i}" for i in range(1, 20)]
ROOT = Path(__file__).resolve().parents[3]


def configure(path):
    cfg = json.loads(path.read_text())
    env = cfg.setdefault("server", {}).setdefault("env", {})
    names = [n.strip() for n in env.get("SRP_BOT_ACCOUNTS", "").split(",") if n.strip()]
    existing = {n.lower() for n in names}
    names += [n for n in NAMES if n.lower() not in existing]
    backup = path.with_name(path.name + f".before-spawn-inspection-{time.time_ns()}")
    shutil.copy2(path, backup)
    os.chmod(backup, 0o600)
    env["SRP_BOT_ACCOUNTS"] = ",".join(names)
    path.write_text(json.dumps(cfg, indent=2) + "\n")
    os.chmod(path, 0o600)
    print("Registered Spawn1–Spawn19 as local test clients. Existing accounts preserved.")
    print("Restart the managed local server before running them. Backup:", backup)


def coverage(csv_path, edit_path, match_id=None):
    edits = json.loads(edit_path.read_text())
    points = [(e["where"]["full_name"], e["to"]["position"])
              for e in edits["set_entry"]
              if e["file"] == "projects/level_09/day_2/client_project"
              and e["list"] == "server_objects/respawn_points"]
    if len(points) != 20:
        raise ValueError("London edit list must contain exactly 20 spawn points")
    with csv_path.open() as stream:
        rows = list(csv.DictReader(stream))
    matches = {r["match"] for r in rows}
    if match_id is None:
        if len(matches) != 1:
            raise ValueError("Specify --match when the CSV contains multiple matches")
        match_id = next(iter(matches))
    samples = {}
    for row in rows:
        if row["match"] == match_id and row["alive"] == "1" and row["x"]:
            # srp-worlds writes the physics capsule centre in map coordinates.
            # Compare feet to authoring points (Motion::Body::feet in Rust).
            centre = 0.55 if row.get("crouch") == "1" else 0.9
            feet = (float(row["x"]), float(row["y"]) - centre, float(row["z"]))
            samples.setdefault(int(row["time"]), {})[int(row["place"])] = feet
    full = [(at, players) for at, players in sorted(samples.items()) if len(players) == 20]
    if not full:
        raise ValueError("No snapshot has 20 alive players; keep the game running through the start")
    at, players = full[0]
    occupied = {}
    for place, position in sorted(players.items()):
        distance, name = min((math.dist(position, point), name) for name, point in points)
        if distance <= 1.0:
            occupied.setdefault(name, []).append(place)
        else:
            print(f"Player {place} is {distance:.2f} m from its nearest spawn")
    print(f"Match {match_id}, first complete snapshot {at} ms: {len(occupied)}/20 distinct spawns occupied")
    for name, _ in points:
        if name not in occupied:
            print("MISSING", name)
        elif len(occupied[name]) > 1:
            print("SHARED", name, occupied[name])
    return len(occupied) == 20


def login_port(cfg):
    ports = cfg["ports"]
    # local-clients/run.py stores [login TCP, lobby UDP, match UDP, web TCP].
    # Also accept named ports in hand-written local configurations.
    raw = ports[0] if isinstance(ports, list) and len(ports) == 4 else ports.get("login") if isinstance(ports, dict) else None
    if isinstance(raw, bool) or raw is None or not 1 <= int(raw) <= 65535:
        raise ValueError("local config ports must contain a login port from 1 to 65535")
    return int(raw)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=ROOT / "private/local-clients/config.json")
    parser.add_argument("--bot-bin", type=Path)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("configure", help="register names in local config; restart server afterwards")
    run = sub.add_parser("run", help="join the owner's private 10v10 room without game windows")
    run.add_argument("room", type=int)
    run.add_argument("--pin", type=int, required=True)
    check = sub.add_parser("coverage", help="check London spawns against srp-worlds positions.csv")
    check.add_argument("positions", type=Path)
    check.add_argument("--match")
    check.add_argument("--edits", type=Path, default=ROOT / "srp-026g/scripts/content-edit/london-1.json")
    args = parser.parse_args()
    if args.command == "configure":
        configure(args.config)
    elif args.command == "coverage":
        return 0 if coverage(args.positions, args.edits, args.match) else 1
    else:
        if not 1 <= args.room <= 99999 or not 0 <= args.pin <= 9999:
            parser.error("room must be 1–99999 and PIN 0–9999")
        cfg = json.loads(args.config.read_text())
        env = cfg["server"].get("env", {})
        registered = {n.strip().lower() for n in env.get("SRP_BOT_ACCOUNTS", "").split(",")}
        if not all(n.lower() in registered for n in NAMES):
            parser.error("run configure, then restart the local server first")
        binary = args.bot_bin or Path(cfg["server"]["binary"]).with_name("srp-bot")
        if not binary.is_file():
            parser.error("build srp-bot and provide --bot-bin (or install beside the local srp)")
        game_data = Path(env.get("SRP_GAME_DATA", ""))
        if not game_data.is_file():
            parser.error("local server SRP_GAME_DATA must name its current server-game-data.db")
        child_env = os.environ.copy()
        child_env["SRP_GAME_DATA"] = str(game_data.resolve())
        print("Joining with 19 stationary clients. Wait for 20/20 in your private room, then press Start.", flush=True)
        print("Ctrl+C disconnects the test clients. This does not open game windows.", flush=True)
        argv = [str(binary), "--server", f"127.0.0.1:{login_port(cfg)}",
                "--name", "Spawn1", "--count", "19", "--revision", str(env.get("SRP_MIN_PATCH_REVISION", 35)),
                "--room", str(args.room), "--pin", str(args.pin)]
        os.execve(str(binary), argv, child_env)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError) as error:
        print(f"spawn-inspection: {error}", file=sys.stderr)
        sys.exit(2)
