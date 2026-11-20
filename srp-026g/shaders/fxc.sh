#!/usr/bin/env bash
# fxc.sh <fxc arguments...>: the June 2010 DirectX SDK's fxc.exe, with its native D3DCompiler_43.dll beside
# it (Wine's builtin d3dcompiler is another compiler), under Wine in a prefix of its own. Paths given to fxc
# are Windows paths (Z:\...). DXSDK_SHADER_COMPILER: the directory holding fxc.exe and the DLL; unset, it
# is built from fxc.nix. FXC_WINEPREFIX: the prefix (default ~/.cache/skhid-fxc-prefix).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
if [ -z "${DXSDK_SHADER_COMPILER:-}" ]; then
  DXSDK_SHADER_COMPILER=$(nix build -f "$here/fxc.nix" --no-link --print-out-paths)/bin
fi
export WINEPREFIX=${FXC_WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/skhid-fxc-prefix}
export WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml=;d3dcompiler_43=n"
unset WAYLAND_DISPLAY DISPLAY
exec wine "$DXSDK_SHADER_COMPILER/fxc.exe" "$@"
