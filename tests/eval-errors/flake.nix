{
  outputs = { self }: {
    packages.x86_64-linux = {
      fine = derivation {
        name = "fine";
        system = "x86_64-linux";
        builder = "/bin/sh";
        args = [
          "-c"
          "echo hi > $out"
        ];
      };
      broken = throw "boom from fixture";
    };
  };
}
