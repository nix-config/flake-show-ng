{
  description = "Show the complete tree of a flake's outputs.";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      lib = nixpkgs.lib;

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];

      exposedSystems = builtins.filter (system: builtins.elem system lib.systems.flakeExposed) systems;

      forAllSystems = f: lib.genAttrs exposedSystems (system: f system nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        system: pkgs: rec {
          flake-show-ng = pkgs.callPackage (pkgs.rustPlatform.buildRustPackage {
            pname = "flake-show-ng";
            version = "0.1.0";

            src = ./.;

            cargoHash = "sha256-S5wbuGb8Z5LmT8El/1cFcZCWukYYrnhMxlmCJwQyXJw=";

            doCheck = true;

            nativeCheckInputs = with pkgs; [
              git
              nixVersions.nix_2_34
              writableTmpDirAsHomeHook
            ];

            preCheck = ''
              export NIX_CONFIG="extra-experimental-features = nix-command flakes"
            '';

            meta = with pkgs.lib; {
              description = "Show the complete tree of a flake's outputs.";
              license = licenses.mit;
              mainProgram = "fsn";
              platforms = platforms.unix;
            };
          }) { };

          default = flake-show-ng;
        }
      );

      apps = forAllSystems (
        system: _pkgs: {
          fsn = {
            type = "app";
            program = lib.getExe self.packages.${system}.flake-show-ng;
          };

          default = {
            type = "app";
            program = lib.getExe self.packages.${system}.flake-show-ng;
          };
        }
      );

      devShells = forAllSystems (
        system: pkgs: {
          default = pkgs.mkShell {
            packages = with pkgs; [
              # Justfile
              just-lsp
              # Nix
              nix-output-monitor
              nixd
              nixfmt
              nixfmt-tree
              # Rust
              cargo
              clippy
              rustc
              rustfmt
            ];
          };
        }
      );
    };
}
