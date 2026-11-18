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
      // lib.filterAttrs (_: v: v != null) {
        inherit (pkgs)
          flake-show-ng
          ;
      };
    };
}
