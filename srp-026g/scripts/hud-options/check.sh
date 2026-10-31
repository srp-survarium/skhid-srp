#!/usr/bin/env bash
# check.sh <main_menu.swf> <hud.swf> [work dir]: the Settings' "Hide hit markers" and "Hide player
# scores" (exe patch 83, docs/notes/hide-hit-markers-scores.md) in Ruffle, on a private CPU-only
# virtual display:
# - SettingsHarness.as: the HUD group (its heading, the two checkboxes) from the exe's answers, the
#   Gameplay tab's two columns (Friends, HUD; Crosshair, Language, Quest progress), a toggle of each
#   pending until Apply, Cancel, the tabs, every dictionary's labels (inside the panel, their glyphs);
# - HudHarness.as: the crosshair's hit marker shown off and hidden on; the Tab list off as today,
#   on names only, each team by clan tag then name (clanless next, bots last; case-insensitive).
# Screenshots into the work dir: settings.png (at rest), settings-checked.png (a toggle, Apply
# enabled), settings-crosshair-open.png, list-scores.png, list-names.png. Exits 0 when both say pass. No game, no server.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
menu=$(readlink -f "$1")
hud=$(readlink -f "$2")
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 3 ]]; then
    work=$(readlink -f "$3")
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi

# Every dictionary's texts of the HUD group and the Language choice (SettingsHarness.check), as JSON.
texts=$("${PYTHON:-python3}" - "$here/../../client-patches" <<'PY'
import json, sys
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
keys = ('st_srp_hide_hit_markers_option', 'st_srp_hide_scores_option', 'st_srp_options_hud', 'st_srp_language_option',
        'st_srp_language_restart', 'st_srp_hide_quests_option', 'st_srp_crosshair_dynamic', 'st_srp_crosshair_static',
        'st_srp_crosshair_hidden', 'st_srp_static_minimap_option')
print(json.dumps([[read_translations(CATALOG / f'{name}.tsv')[k] for k in keys] for name, *_ in LANGUAGES],
                 ensure_ascii=False, separators=(',', ':')))
PY
)

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
for harness in SettingsHarness HudHarness; do
    "$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
        +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
        -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
        -omit-trace-statements=false -default-frame-rate=60 -default-size=1280,720 \
        -output="$work/$harness.swf" "$here/$harness.as" >"$work/$harness.mxmlc.log" 2>&1 \
        || { cat "$work/$harness.mxmlc.log"; exit 1; }
done
cp "$menu" "$work/main_menu.swf"
cp "$hud" "$work/hud.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver nixpkgs#ffmpeg-full --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
ffmpeg=$(grep -- '-ffmpeg-full-.*-bin$' <<<"$paths")/bin/ffmpeg
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
display=:$((90 + RANDOM % 9))
"$xvfb" "$display" -screen 0 1280x720x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 3 ]] || rm -rf "$work"' EXIT
sleep 1

# run <harness>: Ruffle on the harness in the background, its log <harness>.log
run() {
    (
        cd "$work"
        unset WAYLAND_DISPLAY XDG_SESSION_TYPE
        DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
            exec timeout 180 "$ruffle/bin/ruffle" -g vulkan -p high --storage memory --scale no-scale --no-gui \
            --width 1280 --height 720 "-Ptexts=$texts" "$1.swf"
    ) >"$work/$1.log" 2>&1 &
    ruffle_pid=$!
}
wait_for() { for _ in $(seq 1800); do grep -q "$2" "$work/$1.log" && return 0; sleep 0.1; done; return 1; }
shot() { "$ffmpeg" -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x720 -i "$display" -frames:v 1 "$work/$1"; }

run SettingsHarness
wait_for SettingsHarness 'SETTINGS SHOT settings' && shot settings.png
wait_for SettingsHarness 'SETTINGS SHOT checked' && shot settings-checked.png
wait_for SettingsHarness 'SETTINGS SHOT open' && shot settings-crosshair-open.png
kill $ruffle_pid 2>/dev/null || true
run HudHarness
wait_for HudHarness 'HUD SHOT scores' && shot list-scores.png
wait_for HudHarness 'HUD SHOT names' && shot list-names.png
cleanup

for harness in SettingsHarness HudHarness; do
    if ! grep -q 'graphics API vulkan on llvmpipe' "$work/$harness.log"; then
        echo "Ruffle did not run on lavapipe (CPU): see $work/$harness.log" >&2
    fi
done
grep -ho 'SETTINGS .*\|HUD .*' "$work/SettingsHarness.log" "$work/HudHarness.log" || true
! grep -q "RESULT fail" "$work/SettingsHarness.log" "$work/HudHarness.log" \
    && grep -q "SETTINGS RESULT pass" "$work/SettingsHarness.log" && grep -q "HUD RESULT pass" "$work/HudHarness.log"
