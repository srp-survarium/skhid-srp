#!/usr/bin/env bash
# check.sh <lobby_menu tree> [work dir]: the skills window's right click (the owner, 2026-11-27) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU). The window calls the exe
# (apply_skills; Ruffle has no ExternalInterface): a test copy of the tree is built (flash_tree.py)
# with HarnessExternal.as, which records the calls, in its place, over the original movie.json names
# as tools/checks/movie.sh keeps it (run that first). SkillsTakeBackHarness.as makes the movie's own
# SrvCharWindow on a made-up branch and checks: the confirmed ranks; a confirmed point taken back
# (the points up, Confirm and Cancel lit, nothing sent); Cancel giving it back; Confirm sending the
# lower set and the server's answer drawn; a take-back refused (confirmed and pending) leaving every
# rank as it was, no cascade, the red locks of the skills it holds while the pointer is on it;
# an antiskill opened by its pair's point taken back.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
tree=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The original, as tools/checks/movie.sh keeps it by its sha256.
sha=$(sed -n 's/.*"movie_sha256": *"\([^"]*\)".*/\1/p' "$tree/movie.json")
original=${SKHID_CHECKS_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/skhid-checks}/originals/$sha.swf
[[ -f $original ]] || { echo "check.sh: $original: missing (tools/checks/movie.sh lobby_menu keeps it)" >&2; exit 1; }
# The test copy: the recorder for ExternalInterface. It sits where a tree does (srp-026g/flash/<movie>):
# movie.json's data are the repository's tables, three levels up (the scripts, linked).
copy=$work/repo/srp-026g/flash/lobby_menu
rm -rf "$work/repo" "$work/lobby_menu.built.swf"
mkdir -p "$(dirname "$copy")"
cp -r "$tree" "$copy"
ln -s "$(readlink -f "$tree/../../scripts")" "$work/repo/srp-026g/scripts"
cp "$here/HarnessExternal.as" "$copy/src/HarnessExternal.as"
grep -rlZ "ExternalInterface" "$copy/src" | xargs -0 perl -pi -e 's/^\s*import flash\.external\.ExternalInterface;\n//; s/\bExternalInterface\./HarnessExternal./g'
"${PYTHON:-python3}" "$here/../../../tools/actionscript/flash_tree.py" --tools "$tools" build "$copy" "$original" \
    "$work/lobby_menu.built.swf" >"$work/lobby_menu.build.log" 2>&1 || { cat "$work/lobby_menu.build.log"; exit 1; }

harness_compile "$here/SkillsTakeBackHarness.as" 1280,720
# The native Scaleform placements Ruffle refuses, so the window's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$work/lobby_menu.built.swf" "$work/lobby_menu.swf"
harness_display 1280x720
harness_run SkillsTakeBackHarness.log -p low --width 1280 --height 720 SkillsTakeBackHarness.swf
wait_for SkillsTakeBackHarness.log 'SKILLSTAKEBACK RESULT' || true
harness_stop
harness_report 'SKILLSTAKEBACK .*' SkillsTakeBackHarness.log
grep -q 'SKILLSTAKEBACK RESULT pass' "$work/SkillsTakeBackHarness.log"
