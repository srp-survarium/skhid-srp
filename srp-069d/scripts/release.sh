#!/bin/bash
# Build a Skhid release: srp and world-host in the release profile, packed with the scripts, the example config and the
# guides into dist/skhid-<version>-linux-x86_64.tar.gz, plus a .sha256 file next to it.
#
#   scripts/release.sh [--version V] [--out DIR] [--no-build]
#
#   --version V   the version in the file name (default: the workspace version of Cargo.toml)
#   --out DIR     where the tarball and its .sha256 go (default: dist/ in this folder)
#   --no-build    pack the binaries already built in $CARGO_TARGET_DIR/release (default target/release)
#
# Build machine: Linux x86-64 with rustup (rust-toolchain.toml picks the Rust version) and a C compiler (Debian /
# Ubuntu: build-essential). The binaries link against the build machine's C library: build on an older distribution
# (Debian 12, Ubuntu 22.04) so they run on newer ones, and not in a Nix shell (its dynamic loader is not on other
# systems). The script refuses such binaries and prints the newest glibc symbol they need.
#
# Licence: this tree's LICENSE* files; when the tree is the folder srp-069d of the public repository, that repository's
# LICENSE (../LICENSE). Without a licence nothing is packed.
#
# Before packing it checks that the package holds no game files, no keys, no databases and nothing that
# scripts/check-public.sh reports (private paths, notes, addresses).
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VERSION=""
OUT="$ROOT/dist"
BUILD=1
while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION=${2:?--version needs a value}; shift 2 ;;
    --out) OUT=${2:?--out needs a folder}; shift 2 ;;
    --no-build) BUILD=0; shift ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) echo "unknown option $1 (see --help)"; exit 2 ;;
  esac
done
if [ -z "$VERSION" ]; then
  VERSION=$(tr -d '\r' < "$ROOT/Cargo.toml" | sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' | head -1)
fi
case "$VERSION" in
  ""|*[!A-Za-z0-9.+_-]*) echo "bad version '$VERSION' (letters, digits and . + _ - only)"; exit 2 ;;
esac
[ "$(uname -s)-$(uname -m)" = "Linux-x86_64" ] || { echo "build on Linux x86-64 (this is $(uname -s) $(uname -m))"; exit 2; }

NAME="skhid-$VERSION-linux-x86_64"
TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
problems=0
problem() { echo "PROBLEM  $*"; problems=$((problems + 1)); }

# 1. build (rust-toolchain.toml selects the toolchain; paths in panic messages without the build machine's folders)
if [ "$BUILD" = 1 ]; then
  echo "building srp and world-host (release) ..."
  CARGO_HOME_DIR=${CARGO_HOME:-$HOME/.cargo}
  (cd "$ROOT" && RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$ROOT=. --remap-path-prefix=$CARGO_HOME_DIR=cargo" \
    cargo build --release --locked -p srp -p world-host)
fi
for b in srp world-host; do
  [ -x "$TARGET/release/$b" ] || { echo "no $TARGET/release/$b (build first, or drop --no-build)"; exit 2; }
done

# 2. the binaries run on other Linux systems: the standard dynamic loader; report the newest glibc symbol version
for b in srp world-host; do
  f="$TARGET/release/$b"
  if command -v readelf > /dev/null; then
    interp=$(readelf -l "$f" | sed -n 's/.*program interpreter: \(.*\)]/\1/p')
    [ "$interp" = /lib64/ld-linux-x86-64.so.2 ] || problem "$b uses the dynamic loader '$interp' (a Nix shell?): build on a plain distribution"
  else
    echo "WARNING  readelf missing (binutils): the dynamic loader of $b is not checked"
  fi
  if command -v objdump > /dev/null; then
    glibc=$(objdump -T "$f" | grep -o 'GLIBC_[0-9][0-9.]*' | sort -V | tail -1)
    echo "ok       $b needs ${glibc:-no versioned glibc symbols} ($(du -h "$f" | cut -f1))"
  fi
done
# the game tables are built by `srp setup` on the operator's computer: a binary that carries them is not packed
if grep -q -a -F "$(printf 'dict_id\tcfg_name\tsection\tcategory_id')" "$TARGET/release/srp"; then
  problem "srp carries extracted game tables built in: not for a package"
fi

# 3. stage the package
STAGE_ROOT=$(mktemp -d)
trap 'rm -rf "$STAGE_ROOT"' EXIT
S="$STAGE_ROOT/$NAME"
mkdir -p "$S/config" "$S/scripts/player" "$S/scripts/steam-play" "$S/docs/notes"
install -m 755 "$TARGET/release/srp" "$TARGET/release/world-host" "$S/"
install -m 644 "$ROOT/config/srp.example.toml" "$S/config/"
install -m 755 "$ROOT/scripts/srp-host.sh" "$S/scripts/"
install -m 644 "$ROOT"/scripts/player/*.cmd "$ROOT"/scripts/player/*.ps1 "$S/scripts/player/"
install -m 644 "$ROOT"/scripts/steam-play/*.cmd "$ROOT"/scripts/steam-play/*.ps1 "$S/scripts/steam-play/"
install -m 644 "$ROOT/README.md" "$S/"
install -m 644 "$ROOT/docs/guide-server.md" "$ROOT/docs/guide-player.md" "$S/docs/"
install -m 644 "$ROOT/docs/notes/steam.md" "$ROOT/docs/notes/config.md" "$S/docs/notes/"
# the licence: LICENSE* of this tree, else ../LICENSE when this tree is srp-069d/ of the public repository
lic=0
for f in "$ROOT"/LICENSE*; do
  [ -f "$f" ] && { install -m 644 "$f" "$S/"; lic=1; }
done
REAL=$(cd "$ROOT" && pwd -P)
if [ "$lic" = 0 ] && [ "$(basename "$REAL")" = srp-069d ] && [ -f "$REAL/../LICENSE" ]; then
  install -m 644 "$REAL/../LICENSE" "$S/LICENSE"
  lic=1
fi
[ "$lic" = 1 ] || problem "no licence: neither LICENSE* in $ROOT nor ../LICENSE of the public repository (srp-069d/)"
for f in "$ROOT"/NOTICE* "$ROOT"/COPYING*; do
  [ -f "$f" ] && install -m 644 "$f" "$S/"
done

# 4. nothing that must not be shipped: game files, keys, databases, extracted tables, private paths
bad=$(cd "$S" && find . -type f \( -iname '*.exe' -o -iname '*.dll' -o -iname '*.pem' -o -iname '*.key' -o -iname '*.sqlite*' \
  -o -iname '*.db' -o -iname 'resources.*' -o -iname '*.options' -o -iname '*_069d.json' -o -iname '*_069d.tsv' -o -iname 'srp.toml' \) -print)
[ -z "$bad" ] || problem "files that must not be shipped: $bad"
texts=$(cd "$S" && find . -type f ! -name srp ! -name world-host)
if (cd "$S" && grep -l -E -e '-----BEGIN [A-Z ]*PRIVATE KEY-----' $texts > /dev/null 2>&1); then
  problem "a private key in: $(cd "$S" && grep -l -E -e '-----BEGIN [A-Z ]*PRIVATE KEY-----' $texts | tr '\n' ' ')"
fi
for b in srp world-host; do
  grep -q -a -E -e '-----BEGIN (RSA )?PRIVATE KEY-----' "$S/$b" && problem "$b carries a private key"
done
# the public-tree check over the package texts (a copy of the script outside any git checkout checks a plain folder)
mkdir -p "$STAGE_ROOT/check/scripts"
cp "$ROOT/scripts/check-public.sh" "$STAGE_ROOT/check/scripts/"
found=$(bash "$STAGE_ROOT/check/scripts/check-public.sh" "$S" 2>&1) || problem "check-public findings in the package:
$found"
for b in srp world-host; do
  n=$(grep -c -a -F "$HOME/" "$S/$b" || true)
  [ "$n" = 0 ] || echo "WARNING  $b mentions $HOME/ ($n places): build folder names in its messages"
done
if [ "$problems" != 0 ]; then
  echo "release $NAME: $problems problem(s); nothing written"
  exit 1
fi

# 5. pack (stable order, owner root, file times of the last commit or now) and the checksum
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
when=${SOURCE_DATE_EPOCH:-$(git -C "$ROOT" log -1 --format=%ct 2> /dev/null || date +%s)}
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$when" -C "$STAGE_ROOT" -czf "$OUT/$NAME.tar.gz" "$NAME"
(cd "$OUT" && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
echo "files:"
tar -tzvf "$OUT/$NAME.tar.gz" | awk '{printf "  %10s  %s\n", $3, $6}'
echo "wrote $OUT/$NAME.tar.gz ($(du -h "$OUT/$NAME.tar.gz" | cut -f1)) and $NAME.tar.gz.sha256:"
cat "$OUT/$NAME.tar.gz.sha256"
