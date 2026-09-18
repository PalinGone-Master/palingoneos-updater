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
        packages.palin-gone-os-updater = pkgs.rustPlatform.buildRustPackage {
          pname = "palin-gone-os-updater";
          version = "1.1.0";
          src = ./.;

          # Mettez un faux hash temporaire, Nix vous donnera le bon au premier essai
          cargoHash = "";
        };
      }
    );
}
