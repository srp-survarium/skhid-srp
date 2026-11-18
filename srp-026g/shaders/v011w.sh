#!/usr/bin/env bash
# v011w.sh check <0.11e resources.db>
# v011w.sh build <0.26g stock resources.db> <out files dir>
#
# 0.11e's atmosphere programs from v011w/ (vostok's 0.10b sources with 0.11e's changes, README.md):
#   check: compile the four for 0.11e and compare with 0.11e's shipped DXBC; all four must be
#          byte-identical (container, checksum and all). Exits 1 otherwise.
#   build: compile atmospheric_scattering_on_geometry_add.ps for 0.26g (/D TARGET_026G=1: 0.26g's
#          static_globals and table order) into a db-ready blob (0.26g's stock blob's include records
#          + the new DXBC) under <out>/shaders/sm_4_0/, the file of the pack v011e-radar-fog.
# The shipped blobs are read with dbgrab (tools/re-kit/common/dbgrab; DBGRAB overrides). Needs Wine,
# python3 and fxc (fxc.sh).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
dbgrab=${DBGRAB:-$repo/tools/re-kit/common/dbgrab/target/release/dbgrab}
[ -x "$dbgrab" ] || { echo "no dbgrab at $dbgrab: cargo build --release in tools/re-kit/common/dbgrab, or set DBGRAB" >&2; exit 2; }
winpath() { echo "Z:$(echo "$1" | tr / '\\')"; }
mode=${1:-}; db=${2:-}
[ -f "$db" ] || { sed -n 2,13p "$0"; exit 2; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
dxbc() { python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; sys.stdout.buffer.write(d[4+264*n:])' "$1"; }
compile() {  # compile <program> <out.dxbc> [fxc /D ...]
  local program=$1 out=$2; shift 2
  "$here/fxc.sh" /nologo /Zpr /O3 /T ps_4_0 /E main "$@" /Fo "$(winpath "$out")" "$(winpath "$here/v011w/$program.ps")" \
    > "$tmp/log" 2>&1 || { cat "$tmp/log" >&2; exit 1; }
}
shipped() {  # shipped <program>: the one blob of its family in $db (0.11e's and 0.26g's keys differ in length)
  rm -rf "$tmp/grab"; "$dbgrab" "$db" get "$1.ps/" "$tmp/grab" > /dev/null
  set -- "$tmp/grab"/*; [ $# = 1 ] || { echo "$# blobs for $1" >&2; exit 1; }; echo "$1"
}
case $mode in
check)
  bad=0
  for program in atmosphere make_mie_rayleigh_texture atmospheric_scattering_on_geometry_mul atmospheric_scattering_on_geometry_add; do
    compile $program "$tmp/out.dxbc"
    if cmp -s <(dxbc "$(shipped $program)") "$tmp/out.dxbc"; then echo "$program byte-identical"
    else echo "$program DIFF"; bad=$((bad + 1)); fi
  done
  [ $bad = 0 ] ;;
build)
  out=${3:?the out files dir}
  program=atmospheric_scattering_on_geometry_add
  compile $program "$tmp/out.dxbc" /D TARGET_026G=1
  stock=$(shipped $program)
  name=$(basename "$stock" | sed 's/^shaders__sm_4_0__//; s/__/\//')
  mkdir -p "$out/shaders/sm_4_0/$(dirname "$name")"
  python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; open(sys.argv[3],"wb").write(d[:4+264*n]+open(sys.argv[2],"rb").read())' \
    "$stock" "$tmp/out.dxbc" "$out/shaders/sm_4_0/$name"
  echo "$out/shaders/sm_4_0/$name" ;;
*) sed -n 2,13p "$0"; exit 2 ;;
esac
