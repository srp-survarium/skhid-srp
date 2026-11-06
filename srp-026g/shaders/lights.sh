#!/usr/bin/env bash
# lights.sh check <resources.db>
# lights.sh build <resources.db> <out files dir> [--pcss] [fxc /D NAME=VALUE ...]
#
# The 0.26g point and spot light accumulators from sm_4_0/*.ps, all 16 permutations of each:
# `____A_B___________________C`, A = shadow quality (r_shadow_quality 0..3), B = SSDO, C = shadowed.
#   check: compile the stock build and compare with the shipped DXBC in <resources.db>. Expected:
#          12 of 16 byte-identical per shader, and the four with B = 1 and C = 1 differing only in
#          where fxc schedules one `mov r4.w, l(1.0)` (README.md). Exits 1 on anything else.
#   build: compile into db-ready blobs (the shipped blob's include records + the new DXBC) under
#          <out>/shaders/sm_4_0/<family>.ps/<permutation>, the files of a resources patch. --pcss
#          builds the soft-shadow variant (sm_4_0/pcss.h); further /D set its options.
# The shipped blobs are read with dbgrab (tools/re-kit/common/dbgrab; DBGRAB overrides). Needs Wine,
# python3 and fxc (fxc.sh).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
dbgrab=${DBGRAB:-$repo/tools/re-kit/common/dbgrab/target/release/dbgrab}
[ -x "$dbgrab" ] || { echo "no dbgrab at $dbgrab: cargo build --release in tools/re-kit/common/dbgrab (needs tools/vostok-resources-db), or set DBGRAB" >&2; exit 2; }
winpath() { echo "Z:$(echo "$1" | tr / '\\')"; }
mode=${1:-}; db=${2:-}
[ -f "$db" ] || { sed -n 2,16p "$0"; exit 2; }
shift 2
out=; pcss=0
if [ "$mode" = build ]; then out=$1; shift; fi
[ "${1:-}" = --pcss ] && { pcss=1; shift; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
"$dbgrab" "$db" get light_accumulator.ps/ "$tmp/stock" > /dev/null
dxbc() { python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; sys.stdout.buffer.write(d[4+264*n:])' "$1"; }
count=0; bad=0
for fam in point_light_accumulator spot_light_accumulator; do
  for a in 0 1 2 3; do for b in 0 1; do for c in 0 1; do
    perm="____${a}_${b}___________________${c}________________________________________"
    stock="$tmp/stock/shaders__sm_4_0__$fam.ps__$perm"
    defs=(/D SHADOW_QUALITY=$a /D USE_SSDO=$b /D SHADOWED=$c)
    [ $pcss = 1 ] && defs+=(/D PCSS=1 "$@")
    "$here/fxc.sh" /nologo /Zpr /O3 /T ps_4_0 /E main "${defs[@]}" /Fo "$(winpath "$tmp/out.dxbc")" \
      "$(winpath "$here/sm_4_0/$fam.ps")" > "$tmp/log" 2>&1 || { cat "$tmp/log" >&2; exit 1; }
    if [ "$mode" = check ]; then
      if cmp -s <(dxbc "$stock") "$tmp/out.dxbc"; then r=byte-identical; count=$((count + 1))
      elif [ $b = 1 ] && [ $c = 1 ] && [ "$(stat -c %s "$tmp/out.dxbc")" = "$(dxbc "$stock" | wc -c)" ]; then r="same size (the one-mov scheduling difference)"
      else r=DIFF; bad=$((bad + 1)); fi
      echo "$fam $a$b$c $r"
    else
      mkdir -p "$out/shaders/sm_4_0/$fam.ps"
      python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; open(sys.argv[3],"wb").write(d[:4+264*n]+open(sys.argv[2],"rb").read())' \
        "$stock" "$tmp/out.dxbc" "$out/shaders/sm_4_0/$fam.ps/$perm"
    fi
  done; done; done
done
if [ "$mode" = check ]; then
  echo "$count of 32 byte-identical, $bad unexpected"
  [ $count = 24 ] && [ $bad = 0 ]
else
  echo "32 blobs in $out/shaders/sm_4_0 ($([ $pcss = 1 ] && echo PCSS || echo stock))"
fi
