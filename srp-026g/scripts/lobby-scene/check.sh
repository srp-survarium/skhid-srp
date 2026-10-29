#!/usr/bin/env bash
# check.sh <rebuilt main_menu.swf> [work dir] [panel.png [window.png]]: the Settings' Lobby choice (exe
# patch 81, docs/notes/lobby-scenes.md) in Ruffle, on a private CPU-only virtual display: the
# selector from the exe's answer, its arrows, the saved pick, the note, the two columns of the
# bottom panel beside the Language choice, the other tabs and the glyphs of every dictionary's
# texts; with a third argument, a screenshot of the bottom panel, with a fourth one of the whole
# Settings window. No game, no server.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
menu=$(readlink -f "$1")
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 2 ]]; then
    work=$(readlink -f "$2")
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi
shot=${3:+$(readlink -f "$3")}
window=${4:+$(readlink -f "$4")}

# Each registered language's six texts (English first), and the Language choice's two, as JSON.
IFS=$'\t' read -r texts language < <("${PYTHON:-python3}" - "$here/../../client-patches" <<'PY'
import json, sys
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
keys = ('st_srp_lobby_scene_option', 'st_srp_lobby_scene_restart', 'st_srp_lobby_day',
        'st_srp_lobby_stk2_blockpost', 'st_srp_lobby_hangar_046', 'st_srp_lobby_camp_069')
texts = [[read_translations(CATALOG / f'{name}.tsv')[k] for k in keys] for name, *_ in LANGUAGES]
english = read_translations(CATALOG / 'english.tsv')
print(json.dumps(texts, ensure_ascii=False, separators=(',', ':')),
      json.dumps([english['st_srp_language_option'], english['st_srp_language_restart']],
                 ensure_ascii=False, separators=(',', ':')), sep='\t')
PY
)

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=1280,720 \
    -output="$work/LobbySceneHarness.swf" "$here/LobbySceneHarness.as" >"$work/LobbySceneHarness.mxmlc.log" 2>&1 \
    || { cat "$work/LobbySceneHarness.mxmlc.log"; exit 1; }
cp "$menu" "$work/main_menu.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver nixpkgs#imagemagick --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths" | head -1)
mesa=$(grep -- '-mesa-' <<<"$paths" | head -1)
xvfb=$(grep -- '-xorg-server-' <<<"$paths" | head -1)/bin/Xvfb
magick=$(grep -- '-imagemagick-' <<<"$paths" | grep -v -- '-dev$' | head -1)/bin/magick
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
display=:$((90 + RANDOM % 9))
"$xvfb" "$display" -screen 0 1280x720x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 2 ]] || rm -rf "$work"' EXIT
sleep 1

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 180 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale --no-gui \
        --width 1280 --height 720 "-Ptexts=$texts" "-Planguage=$language" LobbySceneHarness.swf
) >"$work/LobbySceneHarness.log" 2>&1 &
ruffle_pid=$!
# The harness stops at LOBBY CAPTURE for a while (the panel as the checks leave it), then goes on.
for _ in $(seq 1800); do grep -q 'LOBBY RESULT\|LOBBY CAPTURE' "$work/LobbySceneHarness.log" && break; sleep 0.1; done
if [[ -n $shot ]] && grep -q 'LOBBY CAPTURE' "$work/LobbySceneHarness.log"; then
    read -r x y w h < <(sed -n 's/.*LOBBY SHOT \([0-9-]*\) \([0-9-]*\) \([0-9]*\) \([0-9]*\).*/\1 \2 \3 \4/p' \
        "$work/LobbySceneHarness.log" | tail -1)
    DISPLAY=$display "$magick" import -window root -crop "${w}x${h}+${x}+${y}" +repage "$shot"
    echo "screenshot: $shot"
    if [[ -n $window ]]; then
        read -r x y w h < <(sed -n 's/.*LOBBY WINDOW \([0-9-]*\) \([0-9-]*\) \([0-9]*\) \([0-9]*\).*/\1 \2 \3 \4/p' \
            "$work/LobbySceneHarness.log" | tail -1)
        DISPLAY=$display "$magick" import -window root -crop "${w}x${h}+${x}+${y}" +repage "$window"
        echo "screenshot: $window"
    fi
fi
for _ in $(seq 1800); do grep -q 'LOBBY RESULT' "$work/LobbySceneHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/LobbySceneHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/LobbySceneHarness.log" >&2
fi
grep -ho 'LOBBY .*' "$work/LobbySceneHarness.log" | grep -v 'LOBBY SHOT\|LOBBY WINDOW\|LOBBY CAPTURE' || true
grep -q 'LOBBY RESULT pass' "$work/LobbySceneHarness.log"
