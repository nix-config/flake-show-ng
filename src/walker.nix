{
  flakePath ? builtins.getEnv "FSN_FLAKE_PATH",
  partsJson ? builtins.getEnv "FSN_PARTS_JSON",
}:
let
  # `eval_tree` always sets `FSN_PARTS_JSON` (at minimum `[]`), so there is no
  # empty-string branch to guard.
  parts = builtins.fromJSON partsJson;

  outputs = (builtins.getFlake flakePath).outputs or { };

  # Hard recursion guard, same value as the old flake-tree.sh baseline.
  depthCap = 100;

  # Hosts are semantic leaves; only the namespace-to-host level is typed.
  configTypeOf = {
    nixosConfigurations = "nixos";
    homeConfigurations = "home-manager";
    darwinConfigurations = "darwin";
  };

  # Keyed on the top-level namespace; an unrecognised namespace keeps the plain
  # `derivation` label, so the key is omitted there.
  labelByNamespace = {
    packages = "package";
    legacyPackages = "package";
    defaultPackage = "package";
    formatter = "package";
    devShell = "development-environment";
    devShells = "development-environment";
  };

  labelFields =
    ns:
    if builtins.hasAttr ns labelByNamespace then
      {
        label = labelByNamespace.${ns};
      }
    else
      { };

  # `nix flake show` labels these namespaces without inspecting their values;
  # `homeModules` is our extension because upstream show does not know it.
  semanticLeafKind =
    ns: depth:
    if depth == 2 && ns == "nixosModules" then
      "nixos-module"
    else if depth == 1 && ns == "nixosModule" then
      "nixos-module"
    else if depth == 2 && ns == "overlays" then
      "nixpkgs-overlay"
    else if depth == 1 && ns == "overlay" then
      "nixpkgs-overlay"
    else if depth == 2 && ns == "homeModules" then
      "home-module"
    else if depth == 1 && ns == "homeModule" then
      "home-module"
    else
      null;

  leafName = path: builtins.elemAt path (builtins.length path - 1);
  isPrefix =
    pre: xs:
    builtins.length pre <= builtins.length xs
    && builtins.all (i: builtins.elemAt pre i == builtins.elemAt xs i) (
      builtins.genList (i: i) (builtins.length pre)
    );

  # Only `throw`/`assert` are catchable here (Nix 2.34.8: tryEval catches
  # AssertionError only).  Anything else propagates and fails the whole run.
  stringFieldOr =
    fallback: thunk:
    let
      r = builtins.tryEval thunk;
    in
    if r.success && builtins.isString r.value then r.value else fallback;

  scalarKind =
    value:
    let
      t = builtins.typeOf value;
    in
    if t == "lambda" then "function" else t;

  classify =
    path: value:
    let
      name = leafName path;
      depth = builtins.length path;
      ns = builtins.head path;
      isAttrs = builtins.isAttrs value;
      semantic = semanticLeafKind ns depth;
    in
    if semantic != null then
      {
        inherit name;
        inherit path;
        kind = semantic;
      }
    else if depth == 2 && builtins.hasAttr ns configTypeOf then
      {
        inherit name;
        inherit path;
        kind = "config";
        configType = configTypeOf.${ns};
      }
    else if isAttrs && value ? drvPath then
      {
        inherit name;
        inherit path;
        kind = "derivation";
        drvName = stringFieldOr "unknown" (value.name or null);
      }
      // labelFields ns
    else if
      isAttrs
      && ((ns == "defaultApp" && depth >= 1) || (ns == "apps" && depth >= 3))
      && builtins.hasAttr "type" value
      && value.type == "app"
    then
      {
        inherit name;
        inherit path;
        kind = "app";
        description = stringFieldOr null ((value.meta or { }).description or null);
      }
    else if isAttrs && (path == [ "defaultTemplate" ] || (ns == "templates" && depth == 2)) then
      {
        inherit name;
        inherit path;
        kind = "template";
        description = stringFieldOr null (value.description or null);
      }
    else if isAttrs then
      {
        inherit name;
        inherit path;
        kind = "attrset";
      }
    else
      {
        inherit name;
        inherit path;
        kind = scalarKind value;
      };

  # Every requested part that is strictly below `path`.  One shared rule for
  # both `walkChildren`'s pruning set and the leaf/error fallback, so a part
  # pointing below a derivation/error leaf is still reported as unknown
  # (exit 1) instead of being silently swallowed.
  partsBelow =
    path: builtins.filter (p: builtins.length p > builtins.length path && isPrefix path p) parts;

  unknownAt = path: candidates: p: {
    part = p;
    failAt = path;
    # Sibling candidates are single-component paths so Rust can Display them
    # through the same AttrPath path, keeping the notice wording byte-identical.
    candidates = map (c: [ c ]) candidates;
  };

  walkChildren =
    path: value:
    let
      depth = builtins.length path;
      names = builtins.sort (a: b: a < b) (builtins.attrNames value);
      isPartRoot = builtins.any (p: p == path) parts;
      active = if isPartRoot then [ ] else partsBelow path;
      nextSegments = map (p: builtins.elemAt p depth) active;
      missing = builtins.filter (p: !(builtins.elem (builtins.elemAt p depth) names)) active;
      childNames =
        if active == [ ] then names else builtins.filter (n: builtins.elem n nextSegments) names;
      walked = map (n: walk (path ++ [ n ]) (builtins.getAttr n value)) childNames;
    in
    {
      children = map (w: w.node) walked;
      unknown = builtins.concatMap (w: w.unknown) walked ++ builtins.map (unknownAt path names) missing;
    };

  walk =
    path: value:
    let
      depth = builtins.length path;
      probed = builtins.tryEval (classify path value);
      # Parts strictly below this path that cannot be drilled into, either
      # because evaluation failed here or because this is a leaf.
      fallbackUnknown = map (unknownAt path [ ]) (partsBelow path);
    in
    if !probed.success then
      {
        node = {
          name = leafName path;
          path = path;
          kind = "error";
        };
        unknown = fallbackUnknown;
      }
    else
      let
        info = probed.value;
      in
      if info.kind == "attrset" && builtins.isAttrs value && depth < depthCap then
        let
          children = walkChildren path value;
        in
        {
          node = info // {
            inherit (children) children;
          };
          inherit (children) unknown;
        }
      else
        {
          node = info;
          unknown = fallbackUnknown;
        };

  root = walkChildren [ ] outputs;
in
{
  tree = root.children;
  unknownParts = root.unknown;
}
