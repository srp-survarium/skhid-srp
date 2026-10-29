#!/usr/bin/env bash
# shots.sh <lobby_menu.swf> <out dir> [<width>x<height> ...]: screenshots of a private room's window
# as the lobby movie draws it (RoomsShots.as) in Ruffle (CPU only: lavapipe on its own virtual X
# display, never the GPU), at the game's resolutions (default 1920x1080 and 1280x720): 1, 3, 10 and
# 20 members, as the room's owner and as a member, and the owner's room playing its match. One PNG
# a case, <out>/<w>x<h>-<case>.png. Java and Flex from private/actionscript/tools.json, or
# SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
out=$(readlink -f "$2")
shift 2
sizes=("$@")
[[ ${#sizes[@]} -gt 0 ]] || sizes=(1920x1080 1280x720)
mkdir -p "$out"
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
work=$(mktemp -d)

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work" english

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" -source-path+="$work" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=1280,720 -default-background-color=0x101418 \
    -output="$work/RoomsShots.swf" "$here/RoomsShots.as" >"$work/RoomsShots.mxmlc.log" 2>&1 \
    || { cat "$work/RoomsShots.mxmlc.log"; exit 1; }
cp "$lobby" "$work/lobby_menu.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver nixpkgs#xorg.xwd nixpkgs#imagemagick \
    --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
xwd=$(grep -- '-xwd-' <<<"$paths")/bin/xwd
magick=$(grep -- '-imagemagick-' <<<"$paths" | head -1)/bin/magick
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json

# case: <name> <members> <owner> <teams> <phase>
cases=(
    "owner-1 1 1 0 0"
    "member-1-of-3 3 0 010 0"
    "owner-3 3 1 010 0"
    "owner-10 10 1 0111111111 0"
    "member-10 10 0 0101010101 0"
    "owner-20 20 1 01010101010101010101 0"
    "member-20 20 0 01010101010101010101 0"
    "owner-10-playing 10 1 0101010101 2"
)
for size in "${sizes[@]}"; do
    w=${size%x*}
    h=${size#*x}
    display=:$((90 + RANDOM % 9))
    "$xvfb" "$display" -screen 0 "${w}x$((h + 200))x24" -nolisten tcp >"$work/xvfb.log" 2>&1 &
    xvfb_pid=$!
    sleep 1
    for case in "${cases[@]}"; do
        read -r name members owner teams phase <<<"$case"
        log=$work/$size-$name.log
        (
            cd "$work"
            unset WAYLAND_DISPLAY XDG_SESSION_TYPE
            DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
                exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
                --width "$w" --height "$h" -Pmovie=lobby_menu.swf -Pmembers="$members" -Powner="$owner" \
                -Pteams="$teams" -Pphase="$phase" RoomsShots.swf
        ) >"$log" 2>&1 &
        ruffle_pid=$!
        for _ in $(seq 900); do grep -q 'SHOT ' "$log" && break; sleep 0.1; done
        sleep 1
        if grep -q 'SHOT ready' "$log"; then
            DISPLAY=$display "$xwd" -root -silent >"$work/shot.xwd"
            # The stage only: under Ruffle's menu bar (22 px).
            "$magick" "$work/shot.xwd" -crop "${w}x${h}+0+22" +repage "$out/$size-$name.png"
            echo "$out/$size-$name.png: $(grep -o 'SHOT ready.*' "$log")"
        else
            echo "$size $name: no shot, see $log" >&2
        fi
        kill "$ruffle_pid" 2>/dev/null || true
        wait "$ruffle_pid" 2>/dev/null || true
    done
    kill "$xvfb_pid" 2>/dev/null || true
    wait "$xvfb_pid" 2>/dev/null || true
done
