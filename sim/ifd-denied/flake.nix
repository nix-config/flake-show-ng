{
  outputs = { self }: {
    packages.x86_64-linux = {
      ifd = builtins.readFile (derivation {
        name = "ifd-file";
        system = "x86_64-linux";
        builder = "/bin/sh";
        args = [
          "-c"
          "echo hello > $out"
        ];
      });
    };
  };
}
