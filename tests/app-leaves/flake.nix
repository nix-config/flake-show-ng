{
  outputs = { self }: {
    apps.x86_64-linux = {
      withDesc = {
        type = "app";
        program = "/bin/true";
        meta.description = "an app with a description";
      };
      noDesc = {
        type = "app";
        program = "/bin/true";
      };
    };
    defaultApp.x86_64-linux = {
      type = "app";
      program = "/bin/true";
      meta.description = "the default app";
    };
  };
}
