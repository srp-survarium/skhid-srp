#!/usr/bin/env bash
# Builds updater_skhid.exe (and the stand-in program of its tests), and puts
# skhid_torrent.dll beside it (torrent-dll/: libtorrent, built by nix from
# srp-026g/flake.nix; SKHID_TORRENT_DLL=<file> takes that one instead).
#   ./build.sh            SKHID_UPDATE_URL=https://host sets another default server
set -euo pipefail
cd "$(dirname "$0")"
# Nix supplies rust-lld inside the sysroot; rustup installations may do the same.
rust_host=$(rustc -vV | sed -n 's/^host: //p')
export PATH="$(rustc --print sysroot)/lib/rustlib/$rust_host/bin:$PATH"
(cd exe && cargo build --locked --release "$@")
(cd tests/fake_exe && cargo build --locked --release "$@")
out=exe/target/i686-win7-windows-msvc/release
dll=${SKHID_TORRENT_DLL:-$(nix build --no-link --print-out-paths ../../srp-026g#skhid-torrent-dll)/skhid_torrent.dll}
install -m 644 "$dll" "$out/skhid_torrent.dll"
ls -l "$out/updater_skhid.exe" "$out/skhid_torrent.dll"
