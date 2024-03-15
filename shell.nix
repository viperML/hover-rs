with import <nixpkgs> {};
mkShell {
  packages = [
    cargo
    rustc
    rust-analyzer
    rustfmt
    man-pages
    man-pages-posix
  ];
  env.RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
