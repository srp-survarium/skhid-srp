#!/usr/bin/env bash
# The exe patches' self-test (srp-026g/client-patches/selftest_026g.py, its Linux part) on the
# original exe, exactly as release.sh runs it at a release (release/lib.sh run_selftest: the
# checkout's dev shell, nixpkgs' Python unicorn, every core, about 5 min), so a PR that breaks it
# fails at the PR. Its builds go to $CHECK_WORK/scratch (removed when it passes).
set -euo pipefail
root=$(git rev-parse --show-toplevel)
main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
[[ -f $exe ]] || { echo "selftest: no original 0.26g survarium.exe at $exe (SKHID_ORIGINAL_EXE)" >&2; exit 1; }
work=${CHECK_WORK:-$(mktemp -d)}
# The real self-test, never release/test.sh's stand-in.
unset SKHID_SELFTEST_BIN
# shellcheck source=../../release/lib.sh
. "$root/release/lib.sh"
status=0
run_selftest "$root" "$exe" "$work/selftest.log" "$work/scratch" || status=$?
cat "$work/selftest.log"
if ((status == 0)); then rm -rf "$work/scratch"; fi
exit "$status"
