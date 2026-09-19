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

      # `x86_64-darwin` stays listed for forward compatibility, but the pinned
      # nixpkgs (26.11) no longer supports it: touching
      # `legacyPackages.x86_64-darwin` throws
      #   "Nixpkgs 26.11 has dropped support for x86_64-darwin."
      # so only expose the systems the pinned nixpkgs actually provides. If
      # upstream (or a future pin) restores x86_64-darwin, it reappears here
      # automatically through lib.systems.flakeExposed.
      exposedSystems = builtins.filter (system: builtins.elem system lib.systems.flakeExposed) systems;

      # Hand-rolled forAllSystems: no flake-utils input.
      forAllSystems = f: lib.genAttrs exposedSystems (system: f system nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        system: pkgs: rec {
          flake-show-ng = pkgs.rustPlatform.buildRustPackage {
            pname = "flake-show-ng";
            version = "0.1.0";
            src = ./.;
            cargoHash = "sha256-S5wbuGb8Z5LmT8El/1cFcZCWukYYrnhMxlmCJwQyXJw=";
            # The integration suite (tests/integration.rs) drives the freshly
            # built binary against the recorded scenario flakes; it needs a
            # `nix` binary (pinned to the version the goldens were recorded
            # with) and `git` (one scenario creates a dirty git worktree).
            # These are check-only inputs: they never end up in the runtime
            # closure. `doCheck = false` is the standard escape hatch.
            nativeCheckInputs = with pkgs; [
              git
              nixVersions.nix_2_34
            ];
            preCheck = ''
              export NIX_CONFIG="experimental-features = nix-command flakes"
              export HOME=$TMPDIR
              export FSN_IT=1
            '';
            # cargo-check-hook prepends `--`, so this becomes the harness flag
            # `--nocapture`: the integration suite's PASS/scenario summary lines
            # show up in the build log (`nix log`) instead of being swallowed.
            checkFlags = [ "--nocapture" ];
            meta = with pkgs.lib; {
              description = "Show the complete tree of a flake's outputs.";
              license = licenses.mit;
              mainProgram = "fsn";
              platforms = platforms.unix;
            };
          };
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
