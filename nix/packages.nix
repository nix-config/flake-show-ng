{
  perSystem =
    {
      lib,
      pkgs,
      self',
      ...
    }:
    {
      packages = {
        default = self'.packages.flake-show-ng;
      }
      // builtins.intersectAttrs (lib.genAttrs [
        "flake-show-ng"
      ] (_: null)) pkgs;
    };
}
