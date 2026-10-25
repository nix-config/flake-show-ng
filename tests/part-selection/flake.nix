{
  outputs = { self }: {
    apps.x86_64-linux.app1 = {
      type = "app";
      program = "/bin/true";
      meta.description = "an app";
    };
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
