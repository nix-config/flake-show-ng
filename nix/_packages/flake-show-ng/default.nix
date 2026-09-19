{
  lib,
  git,
  nixVersions,
  rustPlatform,
  writableTmpDirAsHomeHook,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "flake-show-ng";
  version = "0.1.0";

  src = ../../..;

  cargoHash = "sha256-rmXwEKsugvsmCqu4GdHTH6ckQ39wauny/mR1ZU67Pvo=";

  doCheck = true;

  nativeCheckInputs = [
    git
    nixVersions.nix_2_34
    writableTmpDirAsHomeHook
  ];

  preCheck = ''
    export NIX_CONFIG="extra-experimental-features = nix-command flakes"
  '';

  meta = {
    description = "Show the complete tree of a flake's outputs.";
    homepage = "https://github.com/nix-config/flake-show-ng";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ knightfemale ];
    mainProgram = "fsn";
    platforms = [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-linux"
    ];
  };
})
