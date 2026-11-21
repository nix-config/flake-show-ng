{
  # Non-empty config namespaces: the host/user level is a semantic leaf for
  # both `nix flake show` (nixos-configuration) and fsn (kind:"config" +
  # configType). `homeConfigurations` is an fsn extension namespace: show
  # reports the whole namespace as {"type":"unknown"} and does not recurse.
  outputs = { self }: {
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
  };
}
