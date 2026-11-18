# shellcheck shell=bash
# ruffle_harness.sh: what the movies' Ruffle harnesses (scripts/*/check.sh) share, sourced by each:
#   harness_work [dir]                $work: <dir> (made), or a new temporary one removed at exit
#   harness_compile <file.as> <w,h> [mxmlc args]...
#                                     <file>.swf in $work (its log <file>.mxmlc.log; shown on a
#                                     failure), the .as file's folder its source path; Java and Flex of
#                                     the main checkout's private/actionscript/tools.json or $SRP_AS_TOOLS
#   harness_display <w>x<h> [package]...
#                                     a private Xvfb on an unused display ($display: never the owner's
#                                     desktop), Ruffle and lavapipe (CPU Vulkan), and each nixpkgs
#                                     package's programs on PATH (ffmpeg-full, imagemagick, xdotool),
#                                     all of the nixpkgs srp-026g/flake.lock pins
#   harness_run <log> <ruffle args>...
#                                     Ruffle in $work in the background ($ruffle_pid), its output in
#                                     $work/<log>, for $HARNESS_TIMEOUT s at most (default 180)
#   wait_for <log> <pattern> [tenths] until $work/<log> has <pattern> (default 1800 tenths)
#   harness_stop                      Ruffle and the display stopped (at exit too)
#   harness_report <pattern> <log>... the logs' lines matching <pattern>; a note for a log whose
#                                     Ruffle did not run on lavapipe
# $here is the sourcing script's folder.
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
# Ruffle, Mesa and Xvfb from the nixpkgs srp-026g/flake.lock pins: Ruffle 0.7.1 (nixpkgs-unstable,
# 2026-11) never finishes the lobby harnesses; the pinned nightly does in seconds.
nixpkgs=github:NixOS/nixpkgs/$(sed -n '/"nixpkgs": {/,/"rev"/s/.*"rev": "\([0-9a-f]*\)".*/\1/p' \
    "$(dirname "${BASH_SOURCE[0]}")/../flake.lock")
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
work='' temp_work='' display='' xvfb_pid='' ruffle_pid=''

harness_stop() {
    kill $ruffle_pid $xvfb_pid 2>/dev/null || true
    ruffle_pid='' xvfb_pid=''
}
trap 'harness_stop; [[ -z $temp_work ]] || rm -rf "$temp_work"' EXIT

harness_work() {
    if [[ -n ${1:-} ]]; then
        mkdir -p "$1"
        work=$(readlink -f "$1")
    else
        work=$(mktemp -d) temp_work=$work
    fi
}

harness_compile() {
    local as=$1 size=$2 name
    name=$(basename "$as" .as)
    shift 2
    "$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
        +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
        -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" \
        -source-path="$(dirname "$(readlink -f "$as")")" "$@" \
        -omit-trace-statements=false -default-frame-rate=60 -default-size="$size" \
        -output="$work/$name.swf" "$as" >"$work/$name.mxmlc.log" 2>&1 \
        || { cat "$work/$name.mxmlc.log"; exit 1; }
}

harness_display() {
    local size=$1 paths path
    shift
    paths=$(nix build "$nixpkgs#ruffle" "$nixpkgs#mesa" "$nixpkgs#xorg.xorgserver" "${@/#/$nixpkgs#}" --no-link --print-out-paths)
    for path in $paths; do
        if [[ -d $path/bin ]]; then PATH=$path/bin:$PATH; fi
    done
    lvp=$(echo "$(grep -- '-mesa-[0-9.]*$' <<<"$paths")"/share/vulkan/icd.d/lvp_icd.*.json)
    Xvfb -displayfd 3 -screen 0 "${size}x24" -nolisten tcp 3>"$work/display" >"$work/xvfb.log" 2>&1 &
    xvfb_pid=$!
    for _ in $(seq 100); do [[ -s $work/display ]] && break; sleep 0.1; done
    [[ -s $work/display ]] || { cat "$work/xvfb.log"; exit 1; }
    display=:$(cat "$work/display")
}

harness_run() {
    local log=$1
    shift
    (
        cd "$work" || exit
        unset WAYLAND_DISPLAY XDG_SESSION_TYPE
        DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
            exec timeout "${HARNESS_TIMEOUT:-180}" ruffle -g vulkan --storage memory --scale no-scale "$@"
    ) >"$work/$log" 2>&1 &
    ruffle_pid=$!
}

wait_for() {
    for _ in $(seq "${3:-1800}"); do grep -q -- "$2" "$work/$1" && return 0; sleep 0.1; done
    return 1
}

harness_report() {
    local pattern=$1 log
    shift
    for log in "$@"; do
        grep -q 'graphics API vulkan on llvmpipe' "$work/$log" ||
            echo "Ruffle did not run on lavapipe (CPU): see $work/$log" >&2
    done
    (cd "$work" && grep -ho -- "$pattern" "$@") || true
}
