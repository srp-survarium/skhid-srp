#!/usr/bin/env bash
# lights.sh roundtrip <dir of stock blobs> | build <out files dir> [fxc /D ...]
#
# The 0.26g point and spot light accumulators from sm_4_0/*.ps, all 16 permutations each
# (`____A_B___________________C`: A shadow quality 0..3, B SSDO, C shadowed).
#   roundtrip: compile without PCSS and compare with the shipped DXBC (the stock blobs, as
#              `dbgrab <resources.db> get light_accumulator.ps/ <dir>` writes them).
#   build:     compile with /D PCSS=1 (plus any /D given) into db-ready blobs (the stock blob's
#              include records + the new DXBC) under <out>/shaders/sm_4_0/<family>.ps/, for
#              release/build-patch. Needs the stock blobs in $STOCK_BLOBS.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
winpath() { echo "Z:$(echo "$1" | tr / '\\')"; }
mode=$1; dir=$2; shift 2
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
dxbc() { # <blob> -> DXBC payload on stdout
  python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; sys.stdout.buffer.write(d[4+264*n:])' "$1"
}
stock_dir=$([ "$mode" = roundtrip ] && echo "$dir" || echo "${STOCK_BLOBS:?}")
for fam in point_light_accumulator spot_light_accumulator; do
  for a in 0 1 2 3; do for b in 0 1; do for c in 0 1; do
    perm="____${a}_${b}___________________${c}________________________________________"
    stock="$stock_dir/shaders__sm_4_0__$fam.ps__$perm"
    defs=(/D SHADOW_QUALITY=$a /D USE_SSDO=$b /D SHADOWED=$c)
    [ "$mode" = build ] && defs+=(/D PCSS=1 "$@")
    "$here/fxc.sh" /nologo /Zpr /O3 /T ps_4_0 /E main "${defs[@]}" /Fo "$(winpath "$tmp/out.dxbc")" \
      "$(winpath "$here/sm_4_0/$fam.ps")" > "$tmp/log" 2>&1 || { cat "$tmp/log"; exit 1; }
    if [ "$mode" = roundtrip ]; then
      if cmp -s <(dxbc "$stock") "$tmp/out.dxbc"; then echo "$fam $a$b$c byte-identical"; else echo "$fam $a$b$c DIFF"; fi
    else
      mkdir -p "$dir/shaders/sm_4_0/$fam.ps"
      python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; open(sys.argv[3],"wb").write(d[:4+264*n]+open(sys.argv[2],"rb").read())' \
        "$stock" "$tmp/out.dxbc" "$dir/shaders/sm_4_0/$fam.ps/$perm"
    fi
  done; done; done
done
