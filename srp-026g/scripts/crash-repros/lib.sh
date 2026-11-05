# source me: helpers for driving local clients headless.
#   CR_WORK   a scratch folder with env.sh: DISPLAY (an Xorg dummy at 60 Hz with
#             openbox), PATH with xdotool and ImageMagick, PULSE_SINK (a null sink,
#             so no sound reaches the owner's speakers)
#   CR_REPO   the main checkout (its private/ holds the configs); default: this repo
#   CFG       the run.py config (private/local-clients-repro/config.json)
S=${CR_WORK:?CR_WORK: a scratch folder with env.sh}
. $S/env.sh
R=${CR_REPO:-$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)}
CFG=${CFG:-$R/private/local-clients-repro/config.json}
CD=$(dirname $CFG)
G=$(nix shell nixpkgs#python3 -c python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['game_dir'])" "$CFG")/binaries/x86
WEB_PORT=$(nix shell nixpkgs#python3 -c python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['ports'][3])" "$CFG")
runpy() { (cd $R && nix shell nixpkgs#python3 -c python3 srp-026g/scripts/local-clients/run.py --config $CFG "$@"); }
admin() { curl -s -X POST http://127.0.0.1:$WEB_PORT/admin-local --data-urlencode token=local-demo-token "$@" | sed 's/<[^>]*>/\n/g' | grep -E ': given |No such|Wrong token|has not signed' | head -3; }
wins() { xdotool search --name "Survarium v0.26g" 2>/dev/null; }
# W must be set to a window id
focus() { xdotool windowraise $W 2>/dev/null; xdotool windowactivate --sync $W 2>/dev/null; xdotool windowfocus --sync $W 2>/dev/null; sleep 0.2; }
click() { focus; xdotool mousemove --sync $1 $2; sleep 0.3; xdotool mousedown ${3:-1}; sleep 0.12; xdotool mouseup ${3:-1}; sleep ${4:-1}; }
rmove() { focus; xdotool mousemove_relative -- -5000 -5000; sleep 0.2; xdotool mousemove_relative -- $1 $2; sleep 0.3; }
rclick() { rmove $1 $2; xdotool mousedown ${3:-1}; sleep 0.12; xdotool mouseup ${3:-1}; sleep ${4:-1}; }
look() { focus; xdotool mousemove_relative -- $1 $2; sleep 0.3; }
key() { focus; xdotool keydown $1; sleep ${2:-0.1}; xdotool keyup $1; sleep 0.4; }
shotw() { magick import -display $DISPLAY -window root "$S/shots/$1.png"; echo "$S/shots/$1.png"; }
gamelog() { ls $CD/runtime/client-$1/drive_c/users/sheep/Documents/Survarium/Survarium*.log; }
