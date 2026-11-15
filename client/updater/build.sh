#!/usr/bin/env bash
# Builds updater_skhid.exe (and the stand-in program of its tests).
#   ./build.sh            SKHID_UPDATE_URL=https://host sets another default server
set -euo pipefail
cd "$(dirname "$0")"
# Nix supplies rust-lld inside the sysroot; rustup installations may do the same.
rust_host=$(rustc -vV | sed -n 's/^host: //p')
export PATH="$(rustc --print sysroot)/lib/rustlib/$rust_host/bin:$PATH"
(cd exe && cargo build --locked --release "$@")
(cd tests/fake_exe && cargo build --locked --release "$@")
ls -l exe/target/i686-pc-windows-msvc/release/updater_skhid.exe
