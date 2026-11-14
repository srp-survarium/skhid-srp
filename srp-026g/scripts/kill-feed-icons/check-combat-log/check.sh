#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the hud movie's kill feed (SrvCombatLogList) fed every kind of row
# the exe sends, in Ruffle (CPU only: lavapipe on a virtual X display, never the GPU), by
# KillFeedHarness.as. Exits 0 when every row is added and the feed keeps a later row; prints the
# harness's lines. A work dir given is kept (the harness, Ruffle's log). Java and Flex from
# private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
movie=$(readlink -f "$1")
. "$here/../../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/KillFeedHarness.as" 400,400
cp "$movie" "$work/hud.swf"
harness_display 640x480
# The harness quits Ruffle when it is done.
harness_run ruffle.log -p low --width 400 --height 400 -Pmovie=hud.swf KillFeedHarness.swf
wait "$ruffle_pid" || true
harness_stop
harness_report 'KF .*' ruffle.log
grep -q 'KF RESULT pass' "$work/ruffle.log"
