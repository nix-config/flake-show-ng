{
  config,
  ...
}:
{
  flake.overlays.default = final: prev: {
    flake-show-ng = final.callPackage ./_packages/flake-show-ng { };
  };

  perSystem =
    {
      inputs',
      ...
    }:
    {
      _module.args.pkgs = inputs'.nixpkgs.legacyPackages.extend config.flake.overlays.default;
    };
}
