#!/usr/bin/env bash
# probes.sh check <resources.db>
# probes.sh build <resources.db> <out dir>
#
# The 0.26g box environment probe program and patch 202's tier programs, from
# sm_4_0/environment_probe_lighting.ps: the 8 permutations of ENV_PROBE_GEOMETRY_TYPE 1, LOD_INDEX 0
# (GLOBAL_LIGHTING_QUALITY 0/3 x CONFIG_TDIFFUSE x CONFIG_TALPHABLENDED_DIFFUSE).
#   check: compile the stock program (TIER 0) and compare with the shipped DXBC. Expected: 6 of 8
#          byte-identical; A = 0, C = 0 (two) the same size, one mul's operands placed otherwise.
#   build: the three tier families' DXBC under <out>/<family>/<permutation>.dxbc (what
#          resource-porter's lighting pack takes as its environment_probe_tier_* programs).
# Needs Wine, python3, dbgrab (DBGRAB) and fxc (fxc.sh), as lights.sh.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
dbgrab=${DBGRAB:-$repo/tools/re-kit/common/dbgrab/target/release/dbgrab}
[ -x "$dbgrab" ] || { echo "no dbgrab at $dbgrab (see lights.sh)" >&2; exit 2; }
winpath() { echo "Z:$(echo "$1" | tr / '\\')"; }
mode=${1:-}; db=${2:-}
[ -f "$db" ] || { sed -n 2,13p "$0"; exit 2; }
out=${3:-}
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
"$dbgrab" "$db" get environment_probe_lighting.ps/ "$tmp/stock" > /dev/null
dxbc() { python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; sys.stdout.buffer.write(d[4+264*n:])' "$1"; }
count=0; bad=0
for a in 0 3; do for b in 0 1; do for c in 0 1; do
  perm="___${a}_${b}____________________0${c}___0_________1_________________________"
  stock="$tmp/stock/shaders__sm_4_0__environment_probe_lighting.ps__$perm"
  [ -f "$stock" ] || { echo "no stock $perm" >&2; exit 1; }
  defs=(/D LIGHTING_QUALITY=$a /D AFFECT_DIFFUSE=$b /D BLEND=$c)
  if [ "$mode" = check ]; then
    "$here/fxc.sh" /nologo /Zpr /O3 /T ps_4_0 /E main "${defs[@]}" /Fo "$(winpath "$tmp/out.dxbc")" \
      "$(winpath "$here/sm_4_0/environment_probe_lighting.ps")" > "$tmp/log" 2>&1 || { cat "$tmp/log" >&2; exit 1; }
    if cmp -s <(dxbc "$stock") "$tmp/out.dxbc"; then r=byte-identical; count=$((count + 1))
    elif [ $a = 0 ] && [ $c = 0 ] && [ "$(stat -c %s "$tmp/out.dxbc")" = "$(dxbc "$stock" | wc -c)" ]; then r="same size (the one mul's operands)"
    else r=DIFF; bad=$((bad + 1)); fi
    echo "$a$b$c $r"
  else
    for t in 1:weight 2:damp 3:light; do
      mkdir -p "$out/environment_probe_tier_${t#*:}"
      "$here/fxc.sh" /nologo /Zpr /O3 /T ps_4_0 /E main "${defs[@]}" /D TIER=${t%%:*} \
        /Fo "$(winpath "$out/environment_probe_tier_${t#*:}/$perm.dxbc")" \
        "$(winpath "$here/sm_4_0/environment_probe_lighting.ps")" > "$tmp/log" 2>&1 || { cat "$tmp/log" >&2; exit 1; }
    done
  fi
done; done; done
if [ "$mode" = check ]; then
  echo "$count of 8 byte-identical, $bad unexpected"
  [ $count = 6 ] && [ $bad = 0 ]
else
  echo "24 programs in $out"
fi
