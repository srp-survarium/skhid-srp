#!/usr/bin/env bash
# run.sh <DXVK's x32 folder> [work dir] [DXVK_CONFIG]: the proof of exe patch 83's DXVK load
# (docs/notes/dxvk-option.md) under Wine, no window: loadtest.c preloads the five
# DLLs from <work>\dxvk\ by their full paths (dxgi, d3d11, d3d10core, d3d9, d3d8),
# then asks for each plain name as the game's delayed imports do, and calls a
# delay-loaded D3D11CreateDevice. Prints "RESULT pass" when every plain name is
# the preloaded module, one module of each name, and the delayed call took DXVK's.
# With a DXVK_CONFIG value it first sets the variable in its own process
# (SetEnvironmentVariableA, as patch 83 does with the Vulkan options) and prints
# what DXVK's log says it took (its "Found config env" and "Effective configuration").
# WINEPREFIX as set (its Documents are not touched).
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
dlls=$(readlink -f "$1")
work=${2:-$(mktemp -d)}
mkdir -p "$work/dxvk"
cp "$dlls"/{dxgi,d3d11,d3d10core,d3d9,d3d8}.dll "$work/dxvk/"
cp "$here/loadtest.c" "$here/d3d11.def" "$work/"
cd "$work"
if [[ $# -ge 3 ]]; then printf '#define CONFIG "%s"\n' "$3" >config.h; else : >config.h; fi
nix-shell -E 'with import <nixpkgs> {}; pkgsCross.mingw32.mkShell {}' --run \
    'i686-w64-mingw32-dlltool -k --input-def d3d11.def --output-delaylib libd3d11_delay.a --dllname d3d11.dll &&
     i686-w64-mingw32-gcc -O1 -static -DDELAYED -include config.h -o loadtest.exe loadtest.c libd3d11_delay.a -lpsapi'
unset DISPLAY WAYLAND_DISPLAY DXVK_CONFIG DXVK_CONFIG_FILE DXVK_HUD
WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml=;winemenubuilder.exe=d" wine loadtest.exe 2>dxvk.log | grep -v '^$'
if [[ $# -ge 3 ]]; then
    sed -n 's/^info: *//p' dxvk.log | sed -n '/^Found config env/,/^Vulkan:/p' | grep -v '^Vulkan:'
fi
