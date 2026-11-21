{
  outputs = { self }: {
    legacyPackages.aarch64-darwin = { };
    legacyPackages.x86_64-linux = {
      callPackage = x: x;
      newScope = x: x;
      nested.good = derivation {
        name = "good";
        system = "x86_64-linux";
        builder = "/bin/sh";
        args = [
          "-c"
          "echo good > $out"
        ];
      };
      override.__functionArgs = {
        lib = true;
      };
      packages = { };
    };
  };
}
