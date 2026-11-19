#!/usr/bin/env bash
# v011w.sh check <0.11e resources.db>
# v011w.sh build <0.26g stock resources.db> <out files dir> [--twin] [program ...] | --post-twin
#
# 0.11e's atmosphere programs from v011w/ (vostok's 0.10b sources with 0.11e's changes, README.md):
#   check: compile the four atmosphere programs and every permutation of the post blend for 0.11e and
#          compare with 0.11e's shipped DXBC; all must be byte-identical (container, checksum and all).
#   build: compile each program (default atmospheric_scattering_on_geometry_add, the file of the pack
#          v011e-radar-fog) for 0.26g (/D TARGET_026G=1: 0.26g's static_globals and table order) into a
#          db-ready blob (0.26g's stock blob's include records + the new DXBC) under <out>/shaders/sm_4_0/;
#          --twin: as the family <program>_v011w (the twins exe patch 162 compiles), not the program's own;
#          --post-twin: complex_post_process_blend_v011w.ps (0.11e's post on 0.26g's inputs) into the family
#          complex_post_process_blend_v011w under the plain key (shaders/masks names no define of a family it
#          does not list, so the engine asks for that one; the program reads none of the post's defines).
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
  for program in atmosphere make_mie_rayleigh_texture atmospheric_scattering_on_geometry_mul atmospheric_scattering_on_geometry_add skylight; do
    compile $program "$tmp/out.dxbc"
    if cmp -s <(dxbc "$(shipped $program)") "$tmp/out.dxbc"; then echo "$program byte-identical"
    else echo "$program DIFF"; bad=$((bad + 1)); fi
  done
  # complex_post_process_blend: every shipped permutation; 0.11e's key is 4 globals then 64 configuration
  # defines (fill_shader_configuration_macros 0x732E30): [1] the post quality, [22] bokeh dof, [23] bokeh
  # image, [40] image grain
  rm -rf "$tmp/post"; "$dbgrab" "$db" get complex_post_process_blend.ps/ "$tmp/post" > /dev/null
  same=0; count=0
  for blob in "$tmp/post"/*; do
    key=${blob##*.ps__}; defs=(/D GLOBAL_POST_PROCESS_QUALITY=${key:1:1})
    [ "${key:22:1}" != _ ] && defs+=(/D CONFIG_USE_BOKEH_DOF=${key:22:1})
    [ "${key:23:1}" != _ ] && defs+=(/D CONFIG_USE_BOKEH_IMAGE=${key:23:1})
    [ "${key:40:1}" != _ ] && defs+=(/D CONFIG_USE_IMAGE_GRAIN=${key:40:1})
    compile complex_post_process_blend "$tmp/out.dxbc" "${defs[@]}"
    count=$((count + 1))
    if cmp -s <(dxbc "$blob") "$tmp/out.dxbc"; then same=$((same + 1)); else echo "complex_post_process_blend $key DIFF"; fi
  done
  echo "complex_post_process_blend: $same of $count permutations byte-identical"
  [ $bad = 0 ] && [ $same = $count ] && [ $count -gt 0 ] ;;
build)
  out=${3:?the out files dir}; shift 3
  if [ "${1:-}" = --post-twin ]; then
    "$here/fxc.sh" /nologo /Zpr /O3 /T ps_4_0 /E main /D TARGET_026G=1 /Fo "$(winpath "$tmp/post.dxbc")" \
      "$(winpath "$here/v011w/complex_post_process_blend_v011w.ps")" > "$tmp/log" 2>&1 || { cat "$tmp/log" >&2; exit 1; }
    rm -rf "$tmp/post"; "$dbgrab" "$db" get complex_post_process_blend.ps/ "$tmp/post" > /dev/null
    mkdir -p "$out/shaders/sm_4_0/complex_post_process_blend_v011w.ps"
    set -- "$tmp/post"/*
    stock=$1; key=${stock##*.ps__}; plain=$(printf '%*s' ${#key} '' | tr ' ' _)
    dest="$out/shaders/sm_4_0/complex_post_process_blend_v011w.ps/$plain"
    python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; open(sys.argv[3],"wb").write(d[:4+264*n]+open(sys.argv[2],"rb").read())' \
      "$stock" "$tmp/post.dxbc" "$dest"
    echo "$dest"
    exit 0
  fi
  suffix=; [ "${1:-}" = --twin ] && { suffix=_v011w; shift; }
  for program in "${@:-atmospheric_scattering_on_geometry_add}"; do
    compile $program "$tmp/out.dxbc" /D TARGET_026G=1
    stock=$(shipped $program)
    name=$(basename "$stock" | sed "s/^shaders__sm_4_0__//; s/\.ps__/$suffix.ps\//")
    mkdir -p "$out/shaders/sm_4_0/$(dirname "$name")"
    python3 -c 'import sys,struct; d=open(sys.argv[1],"rb").read(); n=struct.unpack_from("<I",d)[0]; open(sys.argv[3],"wb").write(d[:4+264*n]+open(sys.argv[2],"rb").read())' \
      "$stock" "$tmp/out.dxbc" "$out/shaders/sm_4_0/$name"
    echo "$out/shaders/sm_4_0/$name"
  done ;;
*) sed -n 2,13p "$0"; exit 2 ;;
esac
