#!/usr/bin/env bash
# Run in nix develop ./srp-026g, like client/bugtrap/build.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
rust_host=$(rustc -vV | sed -n 's/^host: //p')
voice_rust_sysroot=$(rustc --print sysroot)
export PATH="$voice_rust_sysroot/lib/rustlib/$rust_host/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target-voice}"
export CARGO_TARGET_I686_WIN7_WINDOWS_MSVC_LINKER=rust-lld
# Static GCC Opus objects have no MSVC SafeSEH annotations. They contain no
# SEH handlers; Rust uses aborting panics, as the game's other DLLs do.
export RUSTFLAGS='-C target-feature=+crt-static -C link-arg=/Brepro -C link-arg=/DEBUG:NONE -C link-arg=/IGNORE:4099 -C link-arg=/SUBSYSTEM:WINDOWS,6.1 -C link-arg=/SAFESEH:NO -C panic=abort -C opt-level=s -C embed-bitcode=yes -C codegen-units=1'
cargo rustc --locked --release -p skhid-voice --lib --crate-type cdylib --target i686-win7-windows-msvc \
  -Z build-std=std,panic_abort -Z trim-paths --config 'profile.release.trim-paths="all"' "$@" -- -C lto=fat
python3 client-voice/check_abi.py "$CARGO_TARGET_DIR/i686-win7-windows-msvc/release/skhid_voice.dll"
