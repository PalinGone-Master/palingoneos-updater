{
  description = "PalinGoneOS Update";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.${system} = {
        palin-gone-os-updater = pkgs.rustPlatform.buildRustPackage {
          pname = "palin-gone-os-updater";
          version = "1.0.0";
          src = ./.;
          cargoVendorDir = "vendor";
          };

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ 
            pkgs.wayland 
            pkgs.libxkbcommon 
          ];
        };
        default = self.packages.${system}.palin-gone-os-updater;
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          rustc
          cargo
          just
          pkg-config
          gcc
          libxkbcommon.dev
          wayland
        ];

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
          pkgs.wayland
          pkgs.libxkbcommon
        ];
      };
    };
}
