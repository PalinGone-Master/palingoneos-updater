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
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "accesskit-0.22.0" = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
              "atomicwrites-0.4.2" = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
	      "build_helpers-0.14.0" = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
            };
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
