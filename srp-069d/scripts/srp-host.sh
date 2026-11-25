#!/bin/bash
# Start / stop the 0.69d host (Linux or WSL2): srp = login + lobby + match server, plus the world host daemon that srp
# starts itself from the config (and stops with it). Everything comes from the config file (config/srp.example.toml).
#
#   scripts/srp-host.sh setup  [CONFIG] [--game DIR] [--address IP]
#                                         check this computer, write the config (if missing), the database and the
#                                         login key, print the players' launch line (docs/guide-server.md)
#   scripts/srp-host.sh start  [CONFIG]   start in the background; srp's log -> <log_dir>/srp.log
#   scripts/srp-host.sh run    [CONFIG]   run in the foreground (log to the terminal and <log_dir>/srp.log); on WSL2
#                                         keep this call attached: a WSL instance with no attached call stops
#   scripts/srp-host.sh stop   [CONFIG]   stop srp (the world host it started stops with it)
#   scripts/srp-host.sh status [CONFIG]
#   scripts/srp-host.sh log    [CONFIG]   follow srp.log
#   scripts/srp-host.sh check  [CONFIG]   check the config and the files it names
#   scripts/srp-host.sh selftest [CONFIG] start a world host of its own, build one start world and play a few seconds
#                                         of a bot match: OK or the failing part
#   scripts/srp-host.sh account|ban|unban|comments ...
#                                         the srp admin commands on ./srp.toml (or $SRP_CONFIG), e.g.
#                                         scripts/srp-host.sh account add <name> <password>
#
# CONFIG defaults to ./srp.toml. SRP_BIN = the srp binary (default: srp in this script's target/release (built from
# source), else srp next to the scripts folder (a release package), else srp on PATH). Logs: <log_dir>/srp.log,
# <log_dir>/world-host.log, <log_dir>/world-host/ (the daemon's per-match logs).
set -u
cmd=${1:-}
CFG=${2:-srp.toml}
case "$CFG" in --*) CFG=srp.toml ;; esac
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
if [ -n "${SRP_BIN:-}" ]; then BIN=$SRP_BIN
elif [ -x "$HERE/target/release/srp" ]; then BIN=$HERE/target/release/srp
elif [ -x "$HERE/srp" ]; then BIN=$HERE/srp
else BIN=$(command -v srp || true); fi
[ -n "$BIN" ] && [ -x "$BIN" ] || { echo "no srp binary (build: cargo build --release -p srp -p world-host; or set SRP_BIN)"; exit 2; }
case "$cmd" in account|ban|unban|comments) exec "$BIN" --config "${SRP_CONFIG:-srp.toml}" "$@" ;; esac
if [ "$cmd" = setup ]; then shift; case "${1:-}" in ""|--*) ;; *) shift ;; esac; exec "$BIN" --config "$CFG" setup "$@"; fi
[ -f "$CFG" ] || { echo "no config $CFG (run: scripts/srp-host.sh setup $CFG --game <game folder>)"; exit 2; }
[ "$cmd" = selftest ] && exec "$BIN" --config "$CFG" selftest
CFG=$(cd "$(dirname "$CFG")" && pwd)/$(basename "$CFG")
# the folders the config names (srp check prints them)
CHECK=$("$BIN" --config "$CFG" check 2>&1)
LOG=$(printf '%s\n' "$CHECK" | sed -n 's/^logs  *//p')
RUN=$(printf '%s\n' "$CHECK" | sed -n 's/^run  *//p')
[ -n "$LOG" ] && [ -n "$RUN" ] || { printf '%s\n' "$CHECK"; exit 2; }
mkdir -p "$LOG" "$RUN"
PID=$RUN/srp.pid
running() { [ -f "$PID" ] && kill -0 "$(cat "$PID")" 2>/dev/null; }
case "$cmd" in
  start)
    running && { echo "already running (pid $(cat "$PID"))"; exit 0; }
    setsid nohup "$BIN" --config "$CFG" run >> "$LOG/srp.log" 2>&1 < /dev/null &
    echo $! > "$PID"
    sleep 1
    running || { echo "srp exited at once:"; tail -20 "$LOG/srp.log"; exit 1; }
    echo "started srp (pid $(cat "$PID")); log $LOG/srp.log"
    ;;
  run)
    running && { echo "already running (pid $(cat "$PID"))"; exit 0; }
    echo $$ > "$PID"
    exec "$BIN" --config "$CFG" run > >(tee -a "$LOG/srp.log") 2>&1
    ;;
  stop)
    running || { echo "not running"; rm -f "$PID"; exit 0; }
    p=$(cat "$PID")
    kill "$p"
    for _ in $(seq 1 50); do kill -0 "$p" 2>/dev/null || break; sleep 0.2; done
    kill -0 "$p" 2>/dev/null && { echo "srp (pid $p) did not stop; kill -9 $p"; exit 1; }
    rm -f "$PID"
    echo "stopped srp (pid $p)"
    ;;
  status)
    if running; then echo "running (pid $(cat "$PID")), log $LOG/srp.log"; else echo "not running"; fi
    ;;
  log)
    tail -n 50 -F "$LOG/srp.log"
    ;;
  check)
    printf '%s\n' "$CHECK"
    ;;
  *)
    sed -n '2,23p' "$0"
    exit 2
    ;;
esac
