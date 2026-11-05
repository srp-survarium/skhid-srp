#!/usr/bin/env bash
# hold.sh <count> [config]: under the game lock, start the local server + <count> clients
# (run.py), then hold until $S/release exists; then stop them.
S=${CR_WORK:?CR_WORK: a scratch folder with env.sh (DISPLAY, PATH, PULSE_SINK)}
export CFG=${2:-/home/sheep/Projects/survarium/skhid-srp/private/local-clients-repro/config.json}
. $(dirname $0)/lib.sh
rm -f $S/release
echo "lock taken $(date +%T)"
runpy restart --count $1 3>&- 2>&1 || runpy start --count $1 3>&- 2>&1
until [ -e $S/release ]; do sleep 2; done
runpy stop 3>&- 2>&1
echo "released $(date +%T)"
