{
  perSystem =
    {
      pkgs,
      self',
      ...
    }:
    {
      packages = {
        default = self'.packages.flake-show-ng;

        inherit (pkgs)
          flake-show-ng
          ;
      };
    };
}
