{
  outputs = { self }: {
    emptyTop = { };
    apps = {
      x86_64-linux = { };
    };
    nixosConfigurations = { };
    packages.x86_64-linux = {
      hello = derivation {
        name = "hello";
        system = "x86_64-linux";
        builder = "/bin/sh";
        args = [
          "-c"
          "echo hi > $out"
        ];
      };
    };
    packages.aarch64-linux = { };
    legacyPackages.x86_64-linux = {
      nested.set.a = 1;
      nested.set.pkg = derivation {
        name = "pkg";
        system = "x86_64-linux";
        builder = "/bin/sh";
        args = [
          "-c"
          "echo pkg > $out"
        ];
      };
    };
  };
}
