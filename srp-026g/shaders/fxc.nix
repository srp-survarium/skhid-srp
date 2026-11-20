# fxc.nix: fxc.exe and its native D3DCompiler_43.dll from Microsoft's June 2010 DirectX SDK, the compiler
# of every shipped 0.26g shader (each blob's RDEF says "Microsoft (R) HLSL Shader Compiler 9.29.952.3111").
# The same derivation as the vostok flake's `dxsdk-shader-compiler`; the installer is fetched from
# Microsoft and pinned by its hash, nothing of the SDK is in this repository.
#   nix build -f srp-026g/shaders/fxc.nix      -> result/bin/{fxc.exe,D3DCompiler_43.dll}
{ pkgs ? import <nixpkgs> { } }:
pkgs.runCommand "dxsdk-shader-compiler" {
  src = pkgs.fetchurl {
    name = "DXSDK_Jun10.exe";
    url = "https://download.microsoft.com/download/A/E/7/AE743F1F-632B-4809-87A9-AA1BB3458E31/DXSDK_Jun10.exe";
    hash = "sha256-cFJx3IO/7lTZuU4ChCbiiNXwcHhLdEbRZPSOz7sqAss=";
  };
  nativeBuildInputs = [ pkgs.cabextract ];
} ''
  set -euo pipefail
  # The installer's payload is one embedded cabinet; the compiler DLL is in a nested redist cab.
  cabextract -q -F 'DXSDK/Utilities/bin/x86/fxc.exe' -d extract "$src"
  cabextract -q -F 'DXSDK/Redist/Jun2010_D3DCompiler_43_x86.cab' -d extract "$src"
  cabextract -q -d cabout "extract/DXSDK/Redist/Jun2010_D3DCompiler_43_x86.cab"
  dll=$(find cabout -iname '*D3DCompiler_43*' -type f | head -1)
  test -n "$dll"
  mkdir -p "$out/bin"
  cp "extract/DXSDK/Utilities/bin/x86/fxc.exe" "$out/bin/fxc.exe"
  cp "$dll" "$out/bin/D3DCompiler_43.dll"
''
