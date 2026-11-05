#!/usr/bin/env bash
# lobby_menu.sh <out.swf>: this checkout's lobby movie (srp-026g/flash/lobby_menu, flash_tree.py
# build) over the original its movie.json names. The original is kept by its sha256 in
# ~/.cache/skhid-checks/originals; the first time it is taken out of its patch in the release cache
# ($SKHID_PATCHES, default ~/.cache/skhid-release/patches; release/build-content fills it) with
# srp-game-data ($SRP_GAME_DATA_BIN, else built in the dev shell).
# Java and Flex from the main checkout's private/actionscript/tools.json, or $SRP_AS_TOOLS.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
main=$(git -C "$root" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
tree=$root/srp-026g/flash/lobby_menu
field() { sed -n "s/.*\"$1\": *\"\([^\"]*\)\".*/\1/p" "$tree/movie.json"; }
sha=$(field movie_sha256)
cache=${SKHID_CHECKS_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/skhid-checks}/originals
original=$cache/$sha.swf
if [[ ! -f $original ]]; then
    archive=${SKHID_PATCHES:-$HOME/.cache/skhid-release/patches}/$(field archive)
    [[ -f $archive ]] || { echo "lobby_menu.sh: $archive: missing (release/build-content downloads the listed patches)" >&2; exit 1; }
    bin=${SRP_GAME_DATA_BIN:-}
    if [[ -z $bin ]]; then
        (cd "$root" && nice -n 10 nix develop ./srp-026g -c bash -c \
            'cd srp-026g && cargo build --quiet --release -p resources-db --bin srp-game-data') >&2
        bin=$root/srp-026g/target/release/srp-game-data
    fi
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    field movie >"$work/paths"
    "$bin" get --db "$archive" --out "$work/out" --paths "$work/paths"
    got=$work/out/$(field movie)
    [[ $(sha256sum "$got" | cut -d' ' -f1) == "$sha" ]] || { echo "lobby_menu.sh: $(field movie) of $archive is not $sha" >&2; exit 1; }
    mkdir -p "$cache"
    cp "$got" "$original.$$" && mv "$original.$$" "$original"
fi
"${PYTHON:-python3}" "$root/tools/actionscript/flash_tree.py" --tools "$tools" build "$tree" "$original" "$1"
