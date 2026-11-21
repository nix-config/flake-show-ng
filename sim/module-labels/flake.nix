{
  outputs = { self }: {
    homeModules.bar = { config, ... }: { };
    nixosModules.foo = { config, ... }: { };
    overlays.default = final: prev: { };
    packages.x86_64-linux.hello = derivation {
      name = "hello";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo hi > $out"
      ];
    };
  };
}
