{
  description = "SRP — Survarium Restoration Project: server-side reimplementation (Rust)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";

      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };

      # Nightly toolchain — the workspace uses `edition = "2024"` and several
      # nightly `#![feature(...)]` gates (generic_atomic, iter_intersperse,
      # slice_split_once), so stable will not build it.
      rust = pkgs.rust-bin.nightly.latest.default.override {
        extensions = [ "rust-src" "rustfmt" "clippy" "rust-analyzer" ];
        # The Rust exe patches (client-patches/rust) are tested and linted for
        # the game's i686, never the host's x86_64 (rust_patches.md).
        targets = [ "i686-unknown-linux-gnu" ];
      };

      # The players' Windows programs (client/bugtrap, client/updater) are
      # built with Microsoft's C runtime, linked in statically (libcmt,
      # libvcruntime, libucrt), as a Rust program built on Windows with
      # `+crt-static` is. These are those libraries and the import libraries
      # Rust's standard library names, 32-bit, from the Visual Studio 2022
      # build tools and the Windows 11 SDK, fetched from Microsoft by `xwin`
      # at pinned versions (its --accept-license: Microsoft's terms for the
      # build tools). The output hash pins the bytes: a different download
      # fails the build. Nothing of the SDK's headers or tools is kept.
      msvcLibs = pkgs.stdenvNoCC.mkDerivation {
        name = "msvc-crt-14.44.17.14-sdk-10.0.26100-x86-libs";
        nativeBuildInputs = [ pkgs.xwin ];
        SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
        outputHashMode = "recursive";
        outputHashAlgo = "sha256";
        outputHash = "sha256-F4ErAyoePkNwxGseI36VfaoPnFfvfje0Ldnekrzlg/8=";
        # (This file has CRLF line ends; bash takes none.)
        buildCommand = builtins.replaceStrings [ "\r" ] [ "" ] (''
          xwin --accept-license --log-level warn --http-retry 3 --timeout 600s \
            --arch x86 --variant desktop \
            --crt-version 14.44.17.14 --sdk-version 10.0.26100 \
            --cache-dir "$TMPDIR/cache" splat --output "$TMPDIR/splat"
          mkdir -p "$out/lib/x86"
          # Each library with the other spellings of its name xwin links to it.
          for lib in libcmt libvcruntime oldnames libucrt \
                     kernel32 ntdll userenv ws2_32 dbghelp advapi32 uuid; do
            for dir in crt/lib/x86 sdk/lib/ucrt/x86 sdk/lib/um/x86; do
              find "$TMPDIR/splat/$dir" -maxdepth 1 -iname "$lib.lib" \
                -exec cp -P -t "$out/lib/x86" {} +
            done
          done
        '');
      };

      # NASM 2.16.03 for the exe patches' arena (client-patches/link_026g.py
      # links it from source at every build, the release's too): NASM 3
      # assembles `jmp near` to a label as a two-byte short jump, so the
      # arena's bytes, locked in arena_026g.lock.json, come from 2.16.
      nasm216 = pkgs.nasm.overrideAttrs (old: {
        version = "2.16.03";
        src = pkgs.fetchurl {
          url = "https://www.nasm.us/pub/nasm/releasebuilds/2.16.03/nasm-2.16.03.tar.xz";
          hash = "sha256-FBKhx2C70F2wJrbA0WV6/9ZjHNCmPN229zzG1KphYUg=";
        };
        patches = [ ];
      });

      # skhid_voice.dll (client-voice/src, built by client-voice/build.sh): in-match
      # voice chat, beside survarium.exe. libopus as the DLL has it: floating
      # point (opus 1.6.1's fixed-point build for x86 does not link: its
      # SSE4.1 SILK functions are missing), no custom modes, a static
      # library; the same build for the loopback test on Linux.
      opusForVoice = opus: opus.overrideAttrs (old: {
        mesonFlags = (old.mesonFlags or [ ]) ++ [ "-Dcustom-modes=false" ];
      });
      mingw32 = pkgs.pkgsCross.mingw32;
      # This file's CRLF line ends, out of the shell snippets.
      noCR = builtins.replaceStrings [ "\r" ] [ "" ];
      voiceOpusWin = (opusForVoice mingw32.libopus).overrideAttrs (old: {
        mesonFlags = old.mesonFlags ++ [ "-Ddefault_library=static" ];
        NIX_CFLAGS_COMPILE = (old.NIX_CFLAGS_COMPILE or "") + " -mstackrealign -D__USE_MINGW_ANSI_STDIO=0 -D_CRTIMP=";
      });
      # GCC's COFF objects use the same C ABI as Rust's MSVC target. Keep
      # Opus static, align calls from the game's four-byte stack, use the
      # standard C stdio names, and strip its automatic DLL export directives.
      voiceOpusMsvc = pkgs.stdenvNoCC.mkDerivation {
        name = "skhid-voice-opus-msvc";
        nativeBuildInputs = [ pkgs.llvm mingw32.stdenv.cc ];
        buildCommand = noCR ''
          mkdir -p $out/lib
          llvm-objcopy -R .drectve ${voiceOpusWin}/lib/libopus.a $out/lib/opus.lib
          cp $(${mingw32.stdenv.cc.targetPrefix}gcc -print-libgcc-file-name) $out/lib/gcc.lib
        '';
      };

      # Its smoke test for Windows or Wine (client-voice/smoke_voice.c).
      skhidVoiceSmoke = mingw32.stdenv.mkDerivation {
        pname = "skhid-voice-smoke";
        version = "1";
        src = ./client-voice;
        buildInputs = [ mingw32.windows.mcfgthreads ];
        dontConfigure = true;
        buildPhase = noCR ''
          $CC -std=c99 -O2 -Wall -Wno-cast-function-type -o smoke_voice.exe smoke_voice.c \
            -static -static-libgcc -Wl,--as-needed -lws2_32
        '';
        installPhase = noCR ''
          mkdir -p $out
          cp smoke_voice.exe $out/
        '';
      };

      # The portable Rust core has no registry dependencies. This hermetic
      # check runs the same tests as cargo test -p skhid-voice, plus builds
      # the WAV listening tool; the real relay test lives in match-server.
      voiceLoopback = pkgs.stdenv.mkDerivation {
        name = "skhid-voice-loopback";
        src = ./client-voice;
        nativeBuildInputs = [ rust ];
        buildInputs = [ (opusForVoice pkgs.libopus) ];
        dontConfigure = true;
        buildPhase = noCR ''
          rustc --edition=2024 --test src/lib.rs -l opus -o test_voice
          ./test_voice >test_voice.log
          cat test_voice.log
          rustc --edition=2024 --crate-name skhid_voice --crate-type rlib src/lib.rs -l opus -o libskhid_voice.rlib
          rustc --edition=2024 src/bin/radio_demo.rs --extern skhid_voice=libskhid_voice.rlib -L . -l opus -o radio_demo
        '';
        installPhase = noCR ''
          mkdir -p $out
          cp test_voice.log radio_demo $out/
        '';
      };
    in
    {
      packages.${system} = {
        voice-opus = voiceOpusMsvc;
        skhid-voice-smoke = skhidVoiceSmoke;
      };

      checks.${system}.voice-loopback = voiceLoopback;

      devShells.${system}.default = pkgs.mkShell {
        name = "srp";

        nativeBuildInputs = [
          pkgs.pkg-config
        ];

        buildInputs = [
          rust
          pkgs.openssl
          pkgs.libopus
          # Used by scripts/dev-bootstrap.sh to run the servers in tmux windows.
          pkgs.tmux
          # llvm-rc (the Windows programs' version and icon resources) and
          # llvm-readobj (client/bugtrap/tests/check_abi.py).
          pkgs.llvm
          # The exe patches (client-patches/*.py; release/build-zip runs
          # them here): Python, and for the arena gcc (-m32) and GNU ld, the
          # shell's own, and NASM 2.16 (above).
          pkgs.python3
          nasm216
        ];

        # The `openssl` crate defaults to vendoring (compiling OpenSSL from
        # source). Use the system (Nix) OpenSSL via pkg-config instead so the
        # dev shell stays hermetic and fast.
        env = {
          OPENSSL_NO_VENDOR = "1";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.libopus.dev}/lib/pkgconfig";
          # Where client/bugtrap and client/updater link the C runtime from.
          SKHID_MSVC_LIB = "${msvcLibs}/lib/x86";
          VOICE_OPUS_LIB_DIR = "${voiceOpusMsvc}/lib";
          # `cargo test --target i686-unknown-linux-gnu` in client-patches/rust
          # links its test binaries against 32-bit glibc with this.
          CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.pkgsi686Linux.stdenv.cc}/bin/cc";
        };
      };
    };
}
