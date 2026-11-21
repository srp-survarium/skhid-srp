#!/usr/bin/env bash
# Run inside nix develop ./srp-026g. Produce a Linux server bundle that carries
# its exact libc/loader, so the relay host need not install Nix or match its libc.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root/srp-026g"
cargo build --locked --release -p udp-transport --bin srp-relay
binary=$PWD/target/release/srp-relay
out=$PWD/target/relay-dist
mkdir -p "$out/lib"
install -m 755 "$binary" "$out/srp-relay"
loader=$(readelf -l "$binary" | sed -n 's/.*Requesting program interpreter: \(.*\)]/\1/p')
[[ -f $loader ]] || { echo 'ELF loader not found' >&2; exit 1; }
install -m 755 "$loader" "$out/lib/ld-linux-x86-64.so.2"
while read -r library; do
    [[ -f $library ]] || { echo "missing dependency: $library" >&2; exit 1; }
    install -m 755 "$library" "$out/lib/$(basename "$library")"
done < <(ldd "$binary" | awk '$2 == "=>" {print $3}')
install -m 755 "$root/srp-026g/deploy/relay/run" "$out/run"
(cd "$out" && sha256sum srp-relay run lib/* > SHA256SUMS)
printf '%s\n' "$out"
