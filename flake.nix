{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = {
    self,
    nixpkgs,
  }: {
    packages.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.callPackage ./package.nix {};

    legacyPackages.x86_64-linux = import nixpkgs {
      system = "x86_64-linux";
      overlays = [(final: prev: {hover-rs = final.callPackage ./package.nix {};})];
    };

    devShells.x86_64-linux.default = with nixpkgs.legacyPackages.x86_64-linux;
      mkShell {
        packages = [
          cargo
          rustc
          rust-analyzer
          rustfmt
          man-pages
          man-pages-posix
          clippy
          bubblewrap
          strace
        ];
        env.RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
      };
  };
}
