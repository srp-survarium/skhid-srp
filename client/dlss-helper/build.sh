#!/usr/bin/env bash
# Builds dlss_helper.exe (x64) in `nix develop ./srp-026g`:
#   target/x86_64-pc-windows-msvc/release/dlss_helper.exe
set -euo pipefail
cd "$(dirname "$0")"
# Nix supplies rust-lld inside the sysroot; rustup installations may do the same.
rust_host=$(rustc -vV | sed -n 's/^host: //p')
export PATH="$(rustc --print sysroot)/lib/rustlib/$rust_host/bin:$PATH"
cargo build --locked --release "$@"
ls -l target/x86_64-pc-windows-msvc/release/dlss_helper.exe
