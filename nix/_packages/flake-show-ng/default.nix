{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "flake-show-ng";
  version = "0.1.0";

  src = ../../..;

  cargoHash = "sha256-BfaHUQKJ6jGP1KmWPnX24JLETG08F2CU/I36VJr2+n0=";

  doCheck = true;

  meta = {
    description = "Show the complete tree of a flake's outputs";
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
