{
  description = "PalinGoneOS Updater";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.palin-gone-os-updater = pkgs.stdenv.mkDerivation {
          pname = "palin-gone-os-updater";
          version = "1.0.0";

          src = pkgs.fetchurl {
            url = "https://github.com/PalinGone-Master/palingoneos-updater/releases/download/v1.0/palin-gone-os-updater";
           
            hash = "sha256-qAdtPSjSHgIBKyDq99v3VAmmJ3E0Q5Al8oLjaOMwWr8=";
          };

          dontUnpack = true;

          installPhase = ''
            mkdir -p $out/bin
            cp $src $out/bin/palin-gone-os-updater
            chmod +x $out/bin/palin-gone-os-updater
          '';
        };
      }
    );
}
