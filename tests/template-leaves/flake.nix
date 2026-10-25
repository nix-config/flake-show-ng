{
  outputs = { self }: {
    templates = {
      withDesc = {
        description = "a template with a description";
        path = ./tmpl;
      };
      noDesc = {
        path = ./tmpl;
      };
    };
  };
}
