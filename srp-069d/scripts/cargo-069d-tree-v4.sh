#!/bin/bash
# Build / test any copy of the 0.69d workspace (the repo or a git worktree) inside WSL / Linux with the nix toolchain.
# No machine-specific paths: everything comes from the script's own location or the environment.
#
#   SRP_TREE       tree to build (default: the repo this script lives in)
#   SRP_NAME       short tag; the tree is copied to ~/srp069d/src-$SRP_NAME and built in ~/srp069d/target-$SRP_NAME
#                  (one target per tree: a shared target mixed the path crates of different trees)  (default: main)
#   SRP_FLAKE_DIR  directory holding flake.nix / flake.lock (and rust-toolchain.toml) of the toolchain
#                  (default: the repo itself)
#   SRP_SLOT_WRAPPER  optional command that takes a compile slot and runs its arguments (a shared slot script);
#                  unset = 0.69d builds run one at a time under the lock ~/srp069d/build.lock
#   CARGO_BUILD_JOBS  default 4
#
# Usage (from Windows Git Bash):
#   MSYS_NO_PATHCONV=1 wsl -d Ubuntu -- env SRP_TREE=/mnt/<drive>/.../worktree SRP_NAME=items \
#       bash /mnt/<drive>/.../srp-069d/scripts/cargo-069d-tree-v4.sh test -p lobby-server -- --test-threads=2
set -u
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SRP_TREE=${SRP_TREE:-$HERE}
SRP_NAME=${SRP_NAME:-main}
FLAKE_SRC=${SRP_FLAKE_DIR:-$HERE}
export PATH=/nix/var/nix/profiles/default/bin:$HOME/.nix-profile/bin:$PATH
SRC=$HOME/srp069d/src-$SRP_NAME
FLAKE=$HOME/srp069d/flake
mkdir -p "$SRC" "$FLAKE"
[ -f "$FLAKE_SRC/flake.nix" ] || { echo "no flake.nix in $FLAKE_SRC (set SRP_FLAKE_DIR)"; exit 2; }
cd "$SRP_TREE" || exit 2
tar --exclude=./target --exclude=./.git --exclude=./.claude -cf - . | tar -xf - -C "$SRC"
cp -u "$FLAKE_SRC/flake.nix" "$FLAKE_SRC/flake.lock" "$FLAKE/" 2>/dev/null
[ -f "$FLAKE_SRC/rust-toolchain.toml" ] && cp -u "$FLAKE_SRC/rust-toolchain.toml" "$FLAKE/"
cd "$SRC"
export CARGO_TARGET_DIR="$HOME/srp069d/target-$SRP_NAME"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
if [ -n "${SRP_SLOT_WRAPPER:-}" ]; then
  $SRP_SLOT_WRAPPER nix develop "$FLAKE" -c cargo "$@"
else
  echo "waiting for the 0.69d build lock ($(date +%T))"
  flock "$HOME/srp069d/build.lock" nix develop "$FLAKE" -c cargo "$@"
fi
rc=$?
cmp -s "$SRC/Cargo.lock" "$SRP_TREE/Cargo.lock" 2>/dev/null || cp "$SRC/Cargo.lock" "$SRP_TREE/Cargo.lock" 2>/dev/null || echo "note: Cargo.lock stays in $SRC"
exit $rc
