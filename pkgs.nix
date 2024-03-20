import <nixpkgs> {
  overlays = [ (final: prev: { hover-rs = final.callPackage ./package.nix { }; }) ];
}
