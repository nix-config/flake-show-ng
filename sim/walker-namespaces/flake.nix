{
  outputs = { self }: {
    packages.x86_64-linux.pkg = derivation {
      name = "pkg";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo pkg > $out"
      ];
    };
    legacyPackages.x86_64-linux.legacy = derivation {
      name = "legacy";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo legacy > $out"
      ];
    };
    defaultPackage.x86_64-linux = derivation {
      name = "dp";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo dp > $out"
      ];
    };
    formatter.x86_64-linux = derivation {
      name = "fmt";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo fmt > $out"
      ];
    };
    devShell.x86_64-linux = derivation {
      name = "shell-sing";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo shell > $out"
      ];
    };
    devShells.x86_64-linux.shell = derivation {
      name = "shell";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo shell > $out"
      ];
    };
    checks.x86_64-linux.deep.leaf = derivation {
      name = "chk";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo chk > $out"
      ];
    };
    hydraJobs.x86_64-linux.job = derivation {
      name = "job";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo job > $out"
      ];
    };
    defaultTemplate = {
      description = "default template";
      path = ./tmpl;
    };
    templates.tmpl = {
      description = "a template";
      path = ./tmpl;
    };
    nixosConfigurations.host1 = {
      config = {
        system.stateVersion = "25.05";
      };
    };
    homeConfigurations.user1 = {
      config = {
        home.stateVersion = "25.05";
      };
    };
    darwinConfigurations.mac1 = {
      config = {
        system.stateVersion = "25.05";
      };
    };
  };
}
