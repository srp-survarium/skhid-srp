# skhid_torrent.dll (skt.h over libtorrent, statically linked, 32-bit
# Windows 10+) and skhid-dht (the same API on the host, for
# release/publish-dht): `nix build ./srp-026g#skhid-torrent-dll` and
# `#skhid-dht`. Hashing is Windows' own (CNG): no OpenSSL, so no HTTPS
# trackers and no encryption. The same sources make the same bytes.
{ pkgs }:
let
  mingw = pkgs.pkgsCross.mingw32;
  libtorrent = mingw.stdenv.mkDerivation {
    pname = "libtorrent-win10-static";
    version = "2.0.12";
    src = pkgs.libtorrent-rasterbar-2_0_x.src;
    nativeBuildInputs = [ pkgs.buildPackages.cmake pkgs.buildPackages.ninja ];
    buildInputs = [ mingw.windows.mcfgthreads ];
    postPatch = "";
    cmakeFlags = [
      "-DBUILD_SHARED_LIBS=OFF"
      "-DCMAKE_BUILD_TYPE=MinSizeRel"
      "-Ddeprecated-functions=OFF"
      "-Dencryption=OFF"
      "-Di2p=OFF"
      "-Dlogging=OFF"
      "-Dmutable-torrents=OFF"
      "-Dstreaming=OFF"
      "-Ddht=ON"
      "-DCMAKE_DISABLE_FIND_PACKAGE_OpenSSL=ON"
      "-DCMAKE_DISABLE_FIND_PACKAGE_GnuTLS=ON"
      "-DCMAKE_DISABLE_FIND_PACKAGE_LibGcrypt=ON"
      "-DBoost_DIR=${pkgs.boost.dev}/lib/cmake/Boost-1.89.0"
    ];
    env.NIX_CFLAGS_COMPILE = "-ffunction-sections -fdata-sections";
    postInstall = "rm -r $out/lib/pkgconfig";
  };
  # The defines libtorrent was built with: its headers must see the same.
  defines = "-DBOOST_ASIO_ENABLE_CANCELIO -DBOOST_ASIO_NO_DEPRECATED -DTORRENT_NO_DEPRECATE"
    + " -DTORRENT_DISABLE_ENCRYPTION -DTORRENT_USE_I2P=0 -DTORRENT_DISABLE_LOGGING"
    + " -DTORRENT_DISABLE_MUTABLE_TORRENTS -DTORRENT_DISABLE_STREAMING";
  # The DLL's own sources only: a change to the tool does not rebuild it.
  sourcesOf = files: pkgs.lib.fileset.toSource {
    root = ./.;
    fileset = pkgs.lib.fileset.unions files;
  };
in
{
  dll = mingw.stdenv.mkDerivation {
    pname = "skhid-torrent-dll";
    # For client/check-reproducible.sh, which builds it again too.
    passthru = { inherit libtorrent; };
    version = "2.0.12";
    src = sourcesOf [ ./skt.h ./skt.cpp ];
    buildInputs = [ mingw.windows.mcfgthreads ];
    dontConfigure = true;
    buildPhase = ''
      $CXX -std=c++17 -Os -DNDEBUG -D_WIN32_WINNT=0x0A00 -DWINVER=0x0A00 -DWIN32_LEAN_AND_MEAN \
        ${defines} -ffunction-sections -fdata-sections \
        -I${libtorrent}/include -I${pkgs.boost.dev}/include -c skt.cpp -o skt.o
      $CXX -shared -o skhid_torrent.dll skt.o ${libtorrent}/lib/libtorrent-rasterbar.a \
        -static -static-libgcc -static-libstdc++ -Wl,--gc-sections -s \
        -Wl,--major-os-version,10 -Wl,--minor-os-version,0 \
        -Wl,--major-subsystem-version,10 -Wl,--minor-subsystem-version,0 \
        -lbcrypt -lmswsock -lws2_32 -liphlpapi -lcrypt32
    '';
    installPhase = ''
      mkdir -p $out
      cp skhid_torrent.dll $out/
    '';
  };
  dht = pkgs.stdenv.mkDerivation {
    pname = "skhid-dht";
    version = "2.0.12";
    src = sourcesOf [ ./skt.h ./skt.cpp ./skhid_dht.c ];
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.libtorrent-rasterbar-2_0_x pkgs.boost pkgs.openssl ];
    dontConfigure = true;
    buildPhase = ''
      $CXX -std=c++17 -O2 $(pkg-config --cflags libtorrent-rasterbar) -c skt.cpp -o skt.o
      $CC -O2 -c skhid_dht.c -o skhid_dht.o
      $CXX -o skhid-dht skhid_dht.o skt.o $(pkg-config --libs libtorrent-rasterbar)
    '';
    installPhase = ''
      mkdir -p $out/bin
      cp skhid-dht $out/bin/
    '';
  };
}
