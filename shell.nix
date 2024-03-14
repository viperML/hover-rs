with import <nixpkgs> {};
mkShell {
  packages = [
    cargo
    rustc
    rust-analyzer
    rustfmt
  ];
  env.RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
