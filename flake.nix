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
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "palin-gone-os-updater";
          version = "1.0.0";
          src = ./.;

          # L'unique et suprême méthode sans hachages
          cargoVendorDir = "vendor";

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.wayland pkgs.libxkbcommon ];
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy ];
        };
      }
    );
}
