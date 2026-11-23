#!/usr/bin/env bash
# pip_lens.sh: compiles pip_lens.hlsl with the game's own compiler (srp-026g/shaders/fxc.sh, the June 2010
# SDK's fxc) into pip_lens.rs, the two shaders' DXBC as byte arrays. Rerun after an edit of the HLSL.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
fxc=$here/../../../../shaders/fxc.sh
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
win() { echo "Z:${1//\//\\}"; }
"$fxc" /nologo /Zpr /O3 /T vs_4_0 /E vs_main /Fo "$(win "$work/vs.dxbc")" "$(win "$here/pip_lens.hlsl")" >/dev/null
"$fxc" /nologo /Zpr /O3 /T ps_4_0 /E ps_main /Fo "$(win "$work/ps.dxbc")" "$(win "$here/pip_lens.hlsl")" >/dev/null
bytes() { od -An -v -tu1 "$1" | tr -s ' \n' ' ' | sed 's/^ //; s/ $//; s/ /, /g'; }
{
  echo "//! Patch 198's lens shaders, compiled from pip_lens.hlsl by pip_lens.sh (do not edit)."
  echo
  echo "pub const VERTEX: [u8; $(stat -c %s "$work/vs.dxbc")] = [$(bytes "$work/vs.dxbc")];"
  echo
  echo "pub const PIXEL: [u8; $(stat -c %s "$work/ps.dxbc")] = [$(bytes "$work/ps.dxbc")];"
} > "$here/pip_lens.rs"
