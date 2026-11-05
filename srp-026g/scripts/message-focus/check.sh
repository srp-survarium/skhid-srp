#!/usr/bin/env bash
# check.sh <login_menu tree> <its original .swf> <main_menu tree> <its original .swf> [work dir]: closing a
# message gives the focus back (srp-026g/flash/*/src/survarium/controls/Message.as), in Ruffle (CPU
# only: lavapipe on its own virtual X display, never the GPU). Ruffle gives a loaded movie no stage
# while its root is made, so the login movie runs as Ruffle's own: a copy of the tree is built
# (flash_tree.py) with the model of GFx's focus manager (scaleform/gfx/FocusManager.as here) in
# place of its stub and MessageFocusHarness.as started at the end of LoginMenu's constructor, and
# the main menu's with the model too; in both, ExternalInterface (the exe; Ruffle has none) is
# HarnessExternal.as, which records the calls. It
# runs the login movie's own LoginMenu and the main menu's Message:
# Esc, Esc, Enter and the close button, Cancel, Enter sign in; Tab after a close goes round the
# form's fields; a main-menu message's close gives the focus back.
# The movie is built without trace(): the driver ends with an uncaught error whose message is its
# lines, which Ruffle logs. Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
login_tree=$(readlink -f "$1")
login_original=$(readlink -f "$2")
main_tree=$(readlink -f "$3")
main_original=$(readlink -f "$4")
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 5 ]]; then
    work=$(readlink -f "$5")
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi

# The test copies: the model for the stub, the recorder for ExternalInterface; the login root starts the driver.
copy() {
    rm -rf "$work/$1" "$work/$1.built.swf"
    cp -r "$2" "$work/$1"
    cp "$here/scaleform/gfx/FocusManager.as" "$work/$1/src/scaleform/gfx/FocusManager.as"
    cp "$here/HarnessExternal.as" "$work/$1/src/HarnessExternal.as"
    grep -rlZ "ExternalInterface" "$work/$1/src" | xargs -0 perl -pi -e 's/^\s*import flash\.external\.ExternalInterface;\n//; s/\bExternalInterface\./HarnessExternal./g'
}
build() {
    "${PYTHON:-python3}" "$here/../../../tools/actionscript/flash_tree.py" --tools "$tools" build "$work/$1" "$2" \
        "$work/$1.built.swf" >"$work/$1.build.log" 2>&1 || { cat "$work/$1.build.log"; exit 1; }
    # the native Scaleform placements Ruffle refuses, so the movie's named children exist
    "${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$work/$1.built.swf" "$work/$1.swf"
}
copy login_menu "$login_tree"
cp "$here/MessageFocusHarness.as" "$work/login_menu/src/MessageFocusHarness.as"
perl -0pi -e 's/(\n         SrpLoginHint\.apply\(this\);\n)/$1         MessageFocusHarness.start(this);\n/' "$work/login_menu/src/LoginMenu.as"
grep -q "MessageFocusHarness.start" "$work/login_menu/src/LoginMenu.as" || { echo "LoginMenu.as: no place to start the harness" >&2; exit 1; }
build login_menu "$login_original"
copy main_menu "$main_tree"
build main_menu "$main_original"


paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
# Xvfb chooses an unused display without touching the owner's desktop.
"$xvfb" -displayfd 3 -screen 0 1280x720x24 -nolisten tcp 3>"$work/display" >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 5 ]] || rm -rf "$work"' EXIT
for _ in $(seq 100); do [[ -s $work/display ]] && break; sleep 0.1; done
[[ -s $work/display ]] || { cat "$work/xvfb.log"; exit 1; }

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=:$(cat display) VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 1280 --height 720 login_menu.swf
) >"$work/MessageFocusHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1200); do grep -q 'FOCUS RESULT' "$work/MessageFocusHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/MessageFocusHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/MessageFocusHarness.log" >&2
fi
grep -ho 'FOCUS .*' "$work/MessageFocusHarness.log" || true
grep -q 'FOCUS RESULT pass' "$work/MessageFocusHarness.log"
