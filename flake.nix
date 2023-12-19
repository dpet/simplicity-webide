{
  description = "Simplicity web IDE";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
  { self
  , nixpkgs
  , flake-utils
  , rust-overlay
  , ...
  }:
  flake-utils.lib.eachSystem [
    "x86_64-linux" "aarch64-linux" "x86_64-darwin"
  ] (system:
    let
      overlays = [
        (import rust-overlay)
      ];
      pkgs = import nixpkgs {
        inherit system overlays;
      };
      rust-min = pkgs.rust-bin.stable.latest.default.override {
        targets = [
          "wasm32-unknown-unknown"
        ];
      };
      leptos-min = [
        rust-min
        pkgs.trunk
      ];
      rust-src = rust-min.override {
        extensions = [
          "rust-src"
        ];
      };
      leptos-dev = [
        rust-src
        pkgs.gdb
        pkgs.trunk
      ];
      wasm-tests = with pkgs; [
        wasm-pack
        wasm-bindgen-cli
        nodejs
      ];
      deploy = pkgs.callPackage ./deploy.nix {
        rust = rust-min;
      };
    in
    {
      devShells = {
        default = pkgs.mkShell.override {
          stdenv = pkgs.clang16Stdenv;
        } {
          buildInputs = leptos-dev ++ wasm-tests;

          CC_wasm32_unknown_unknown = "${pkgs.llvmPackages_16.clang-unwrapped}/bin/clang-16";
          CFLAGS_wasm32_unknown_unknown = "-I ${pkgs.llvmPackages_16.libclang.lib}/lib/clang/16/include/";
          RUST_TOOLCHAIN = "${rust-src}/bin";
          RUST_STDLIB = "${rust-src}/lib/rustlib/src/rust";
          DEBUGGER = "${pkgs.gdb}";
        };
        # Temporary shell until deploy.nix works
        deploy = pkgs.mkShell.override {
          stdenv = pkgs.clang16Stdenv;
        } {
          buildInputs = leptos-min;
          CC_wasm32_unknown_unknown = "${pkgs.llvmPackages_16.clang-unwrapped}/bin/clang-16";
          CFLAGS_wasm32_unknown_unknown = "-I ${pkgs.llvmPackages_16.libclang.lib}/lib/clang/16/include/";
        };
      };
    }
  );
}
