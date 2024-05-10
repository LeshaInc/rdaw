{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
  };

  outputs = { self, nixpkgs, utils, fenix }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        rust-toolchain = with fenix.packages.${system};
          combine (with complete; [
            rustc
            rust-src
            cargo
            clippy
            rustfmt
            rust-analyzer
            miri
          ]);
      in {
        devShell = (pkgs.mkShell.override { stdenv = pkgs.clangStdenv; }) rec {
          buildInputs = with pkgs; [
            clangStdenv

            pipewire
            dbus

            pkg-config
            rust-toolchain
            rustPlatform.bindgenHook
          ];

          RUST_SRC_PATH = "${rust-toolchain}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = 1;
        };
      }
    );
}
