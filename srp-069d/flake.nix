{
  description = "SRP 0.69d: Survarium 0.69d server (Rust)";

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

      # Nightly toolchain for development. Stable Rust builds the workspace
      # too (edition 2024, no `#![feature(...)]` gates): releases use the
      # stable version pinned in rust-toolchain.toml, which rustup reads.
      rust = pkgs.rust-bin.nightly.latest.default.override {
        extensions = [ "rust-src" "rustfmt" "clippy" "rust-analyzer" ];
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        name = "srp-069d";

        nativeBuildInputs = [
          pkgs.pkg-config
        ];

        buildInputs = [
          rust
          pkgs.openssl
          # Terminal multiplexer: the server and its logs side by side in one shell.
          pkgs.tmux
        ];

        # The `openssl` crate defaults to vendoring (compiling OpenSSL from
        # source). Use the system (Nix) OpenSSL via pkg-config instead so the
        # dev shell stays hermetic and fast.
        env = {
          OPENSSL_NO_VENDOR = "1";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };
      };
    };
}
