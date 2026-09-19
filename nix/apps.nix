{
  perSystem =
    {
      lib,
      self',
      ...
    }:
    {
      apps = {
        default = self'.apps.flake-show-ng;

        flake-show-ng = {
          type = "app";
          program = lib.getExe self'.packages.flake-show-ng;
        };
      };
    };
}
