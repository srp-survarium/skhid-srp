#!/usr/bin/env bash
# Builds the programs the players' ZIP ships (BugTrap.dll, updater_skhid.exe)
# twice, from copies of client/ in two folders of different names, and checks
# that both builds are the same files, byte for byte; and skhid_torrent.dll
# (updater/torrent-dll, a nix build) and its libtorrent: built again (`nix build --rebuild`),
# which fails when the bytes differ. Run it in the toolchain of
# srp-026g/flake.nix:
#   nix develop ./srp-026g -c client/check-reproducible.sh
set -euo pipefail
client=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

outputs=(
    bugtrap/target/i686-pc-windows-msvc/release/BugTrap.dll
    updater/exe/target/i686-pc-windows-msvc/release/updater_skhid.exe
)

# The DLL and its static libtorrent: nix's builds, then once more, compared.
dll=$(nix build --no-link --print-out-paths "$client/../srp-026g#skhid-torrent-dll")
status=0
for part in skhid-torrent-dll.libtorrent skhid-torrent-dll; do
    if nix build --no-link --rebuild "$client/../srp-026g#$part" 2>"$work/rebuild.log"; then
        echo "same      $part"
    else
        echo "DIFFERENT $part (nix build --rebuild):"
        tail -n 5 "$work/rebuild.log"
        status=1
    fi
done
export SKHID_TORRENT_DLL=$dll/skhid_torrent.dll

build() {
    local dir=$1
    mkdir -p "$dir"
    tar -C "$client" --exclude=target -cf - . | tar -C "$dir" -xf -
    (cd "$dir/bugtrap" && ./build.sh >"$dir.log" 2>&1)
    (cd "$dir/updater" && bash build.sh >>"$dir.log" 2>&1)
}

build "$work/a"
build "$work/another-folder"
for output in "${outputs[@]}"; do
    a=$(sha256sum <"$work/a/$output" | cut -d' ' -f1)
    b=$(sha256sum <"$work/another-folder/$output" | cut -d' ' -f1)
    if [[ $a == "$b" ]]; then
        echo "same      $a  $output"
    else
        echo "DIFFERENT $a $b  $output"
        status=1
    fi
done
exit $status
