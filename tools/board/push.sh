#!/usr/bin/env bash
# Publish the board's data file: copy it to the host, check it there with the installed binary,
# then swap it in with one rename. The running board picks it up on the next page load.
#
#   tools/board/push.sh [FILE]     (default: <main checkout>/private/board/board.json)
#
# BOARD_HOST overrides the host (default root@2.28.121.133).
set -euo pipefail

host=${BOARD_HOST:-root@2.28.121.133}
dir=/opt/skhid-board

if [[ $# -ge 1 ]]; then
  src=$1
else
  # private/ lives in the main checkout only, also when this runs from a worktree.
  common=$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)
  src=$(dirname "$common")/private/board/board.json
fi
[[ -f $src ]] || { echo "push.sh: no data file at $src" >&2; exit 1; }

tmp=$dir/.board.json.upload
scp -q "$src" "$host:$tmp"
ssh "$host" "set -e
  if ! $dir/skhid-board --check $tmp; then rm -f $tmp; exit 1; fi
  chmod 644 $tmp
  mv -f $tmp $dir/board.json"
echo "push.sh: published $src"
