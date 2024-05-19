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
            mold

            dbus
            libxkbcommon
            pipewire
            vulkan-loader
            vulkan-validation-layers
            wayland

            pkg-config
            rust-toolchain
            rustPlatform.bindgenHook
          ];

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
          VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d/";
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.llvmPackages.clangUseLLVM}/bin/clang";
          CARGO_ENCODED_RUSTFLAGS = "-Clink-arg=-fuse-ld=${pkgs.mold}/bin/mold";
          RUST_SRC_PATH = "${rust-toolchain}/lib/rustlib/src/rust/library";
          RUST_LOG = "warn,rdaw=trace";
          RUST_BACKTRACE = 1;
        };
      }
    );
}
