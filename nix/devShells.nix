{
  perSystem =
    {
      pkgs,
      ...
    }:
    {
      devShells.default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          clippy
          flake-show-ng
          just-lsp
          nixd
          nixfmt
          nixfmt-tree
          nix-output-monitor
          rust-analyzer
          rustc
          rustfmt
        ];
      };
    };
}
