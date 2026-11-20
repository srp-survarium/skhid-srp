#!/usr/bin/env bash
# check_ui.sh <lobby_menu.swf> <work directory>: weight numbers and native hover tooltip in CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "$2"
harness=$(readlink -f "${3:-$here/WeightHarness.as}")
harness_name=$(basename "$harness" .as)
result_tag=${4:-WEIGHT}
# The game's Scaleform does not join the arrays of an Array.concat with several arguments (each
# further one becomes one element; Ruffle joins them): no lobby source may call it so
# (docs/notes/lobby-source.md, "Scaleform pitfalls").
if grep -rnP --include='*.as' '\.concat\((?:[^(),]|\((?:[^()]|\([^()]*\))*\))*,' "$here/../../flash/lobby_menu/src/survarium/player"; then
    echo "WEIGHT RESULT fail: Array.concat with several arguments (see above)"; exit 1
fi
# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
# The lobby's kill feed weapon icons are the hud's own images (make_lobby_weapon_icons.py --check).
ffdec=$(sed -n 's/.*"ffdec": *"\([^"]*\)".*/\1/p' "$tools")
nix shell --impure --expr 'with import <nixpkgs> {}; python3.withPackages (p: [p.pillow])' -c python3 -I \
    "$here/../kill-feed-icons/make_lobby_weapon_icons.py" --check \
    --patches "${SKHID_PATCHES:-$HOME/.cache/skhid-release/patches}" --ffdec "$ffdec" --java "$java" \
    --hud-bitmaps "$here/../../flash/hud/bitmaps" --lobby "$here/../../flash/lobby_menu" >"$work/kill-icons.log" 2>&1 \
    || { cat "$work/kill-icons.log"; exit 1; }
grep -h '^ok:' "$work/kill-icons.log"
harness_compile "$harness" 960,720 -source-path+="$work"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 960x720
HARNESS_TIMEOUT=60 harness_run ruffle.log -p low --width 960 --height 720 "$harness_name.swf"
wait_for ruffle.log "$result_tag RESULT" 600 || true
harness_stop
harness_report "$result_tag RESULT.*" ruffle.log
grep -q "$result_tag RESULT pass" "$work/ruffle.log"
