#!/usr/bin/env bash
# fxc.sh <fxc args...>: the June 2010 DirectX SDK fxc.exe (with its native D3DCompiler_43.dll), the
# compiler of every shipped 0.26g shader, under Wine in its own prefix. Paths given to fxc must be
# Windows paths (Z:\...); `winpath` below converts. DXSDK_SHADER_COMPILER: the directory with
# fxc.exe and D3DCompiler_43.dll (vostok flake package `dxsdk-shader-compiler`).
: "${DXSDK_SHADER_COMPILER:?set it to the dxsdk-shader-compiler bin directory (vostok flake)}"
export WINEPREFIX=${FXC_WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/skhid-fxc-prefix}
export WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml=;d3dcompiler_43=n"
unset WAYLAND_DISPLAY DISPLAY
exec wine "$DXSDK_SHADER_COMPILER/fxc.exe" "$@"
