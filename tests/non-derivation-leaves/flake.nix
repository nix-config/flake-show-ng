{
  outputs = { self }: {
    packages.x86_64-linux.notdrv = {
      name = "x";
    };
    checks.x86_64-linux.mkCheck = {
      name = "y";
    };
    devShells.x86_64-linux.shell = {
      name = "z";
    };
  };
}
