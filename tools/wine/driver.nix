# Build only the Unix X11 driver; use with Wine 11.8, never another Wine ABI.
{ pkgs }:
let wine = pkgs.wineWow64Packages.staging;
in assert wine.version == "11.8";
wine.overrideAttrs (old: {
  pname = "wine-x11-keyboard-test";
  patches = old.patches ++ [ ./x11-base-layout.patch ];
  buildPhase = ''
    runHook preBuild
    make -j$NIX_BUILD_CORES dlls/winex11.drv/winex11.so
    runHook postBuild
  '';
  installPhase = ''
    mkdir -p $out/lib/wine/x86_64-unix
    cp dlls/winex11.drv/winex11.so $out/lib/wine/x86_64-unix/winex11.so
  '';
  postInstall = "";
})
