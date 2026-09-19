//! Byte-level tests for the text and JSON renderers.
//!
//! Moved out of `src/render/` in Phase 6: the expected text bytes are inlined
//! below as `EXPECTED_*` constants (ANSI escapes verbatim) and checked by the
//! table-driven `renders_text_goldens`, so no expected output is hardcoded in
//! `src/` anymore.  The JSON renderer's expectations are structural and stay
//! as assertions below.
//!
//! Only the public API is used: `flake_show_ng::render::{text, json}` and the
//! `flake_show_ng::model` types (all fields public).  The local `node` /
//! `children` / `drv` / `desc` / `config` helpers keep the tests independent
//! of test-only constructors in `src/`.

use flake_show_ng::attrpath::AttrPath;
use flake_show_ng::model::{ConfigType, Kind, Label, Node, WalkerOutput};
use flake_show_ng::render::{json, text};

// ---- local node builders (public fields only) --------------------------------

fn node(name: &str, kind: Kind, path: &[&str]) -> Node {
    Node {
        name: name.to_owned(),
        kind,
        path: AttrPath::from_components(path.iter().map(|s| (*s).to_owned()).collect()),
        children: None,
        drv_name: None,
        description: None,
        config_type: None,
        label: None,
    }
}

fn children(mut node: Node, children: Vec<Node>) -> Node {
    node.children = Some(children);
    node
}

fn drv(mut node: Node, drv_name: &str) -> Node {
    node.drv_name = Some(drv_name.to_owned());
    node
}

fn desc(mut node: Node, description: Option<&str>) -> Node {
    node.description = Some(description.map(str::to_owned));
    node
}

fn config(mut node: Node, config_type: ConfigType) -> Node {
    node.config_type = Some(config_type);
    node
}

/// The walker-owned derivation label; absent means the plain `derivation` label,
/// which is the default of the `node` builder.
fn label(mut node: Node, label: Label) -> Node {
    node.label = Some(label);
    node
}

// ---- inlined text goldens (ANSI escapes verbatim) ----------------------------

const EXPECTED_ROOT_CONNECTORS_HYDRA: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mlegacyPackages\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m│       └───\u{1b}[0m\u{1b}[1mhello\u{1b}[0m: package 'hello'\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mhydraJobs\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mjobsets\u{1b}[0m\n",
    "\u{1b}[32;1m        └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m            └───\u{1b}[0m\u{1b}[1mjob\u{1b}[0m: derivation 'job'\n",
);

const EXPECTED_DEV_SHELL_LABEL: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mdevShells\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m        └───\u{1b}[0m\u{1b}[1mdefault\u{1b}[0m: development environment 'nix-shell'\n",
);

const EXPECTED_CONFIG_APP_TEMPLATE_ERROR: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mnixosConfigurations\u{1b}[0m\n",
    "\u{1b}[32;1m│   ├───\u{1b}[0m\u{1b}[1mhost\u{1b}[0m: \u{1b}[35;1mNixOS configuration\u{1b}[0m\n",
    "\u{1b}[32;1m│   ├───\u{1b}[0m\u{1b}[1mhm\u{1b}[0m: \u{1b}[35;1mHome Manager configuration\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mdarwin\u{1b}[0m: \u{1b}[35;1mnix-darwin configuration\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mapps\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m│       ├───\u{1b}[0m\u{1b}[1mfoo\u{1b}[0m: app: \u{1b}[1mdoes foo\u{1b}[0m\n",
    "\u{1b}[32;1m│       └───\u{1b}[0m\u{1b}[1mbar\u{1b}[0m: app: \u{1b}[1mno description\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mtemplates\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mtpl\u{1b}[0m: template: \u{1b}[1ma template\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mbroken\u{1b}[0m: \u{1b}[31;1merror\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mscalar\u{1b}[0m\n",
);

const EXPECTED_NON_DERIVATION_LABELS: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mfunctions\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mlist\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mnull\u{1b}[0m\n",
);

const EXPECTED_CHECKS_DEPTH: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mchecks\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m        ├───\u{1b}[0m\u{1b}[1munit\u{1b}[0m: derivation 'unit-check'\n",
    "\u{1b}[32;1m        └───\u{1b}[0m\u{1b}[1mnixos\u{1b}[0m\n",
    "\u{1b}[32;1m            └───\u{1b}[0m\u{1b}[1mdeep\u{1b}[0m: derivation 'deep-check'\n",
);

const EXPECTED_FORMATTER_DEFAULT_PACKAGE: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mformatter\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m: package 'nixfmt'\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mdefaultPackage\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m: package 'default-pkg'\n",
);

const EXPECTED_DEFAULT_APP_TEMPLATE: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mdefaultApp\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m: app: \u{1b}[1mno description\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mdefaultTemplate\u{1b}[0m: template: \u{1b}[1mno description\u{1b}[0m\n",
);

const EXPECTED_OVERLAYS_NIXOS_MODULES: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mnixosModules\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mfoo\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1moverlays\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mbar\u{1b}[0m\n",
);

const EXPECTED_MULTI_PART_FOREST: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mpackages\u{1b}[0m\n",
    "\u{1b}[32;1m│   ├───\u{1b}[0m\u{1b}[1maarch64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m│   │   └───\u{1b}[0m\u{1b}[1mhello\u{1b}[0m: package 'hello-arm'\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m│       └───\u{1b}[0m\u{1b}[1mhello\u{1b}[0m: package 'hello-amd'\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mlegacyPackages\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m│       └───\u{1b}[0m\u{1b}[1mnested\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mfunctions\u{1b}[0m\n",
);

const EXPECTED_DEEP_NESTING: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1ma\u{1b}[0m\n",
    "\u{1b}[32;1m    ├───\u{1b}[0m\u{1b}[1mb1\u{1b}[0m\n",
    "\u{1b}[32;1m    │   ├───\u{1b}[0m\u{1b}[1mc1\u{1b}[0m\n",
    "\u{1b}[32;1m    │   │   ├───\u{1b}[0m\u{1b}[1md1\u{1b}[0m\n",
    "\u{1b}[32;1m    │   │   │   └───\u{1b}[0m\u{1b}[1me1\u{1b}[0m\n",
    "\u{1b}[32;1m    │   │   └───\u{1b}[0m\u{1b}[1md2\u{1b}[0m\n",
    "\u{1b}[32;1m    │   └───\u{1b}[0m\u{1b}[1mc2\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mb2\u{1b}[0m\n",
);

const EXPECTED_ERROR_LAST_CHILD: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mpackages\u{1b}[0m\n",
    "\u{1b}[32;1m    ├───\u{1b}[0m\u{1b}[1mok\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mbroken\u{1b}[0m: \u{1b}[31;1merror\u{1b}[0m\n",
);

const EXPECTED_FALLBACK_LABELS: &str = concat!(
    "\u{1b}[1mpath:/tmp/flake\u{1b}[0m\n",
    "\u{1b}[32;1m├───\u{1b}[0m\u{1b}[1mlegacyPackages\u{1b}[0m\n",
    "\u{1b}[32;1m│   └───\u{1b}[0m\u{1b}[1mx86_64-linux\u{1b}[0m\n",
    "\u{1b}[32;1m│       └───\u{1b}[0m\u{1b}[1mnoname\u{1b}[0m: derivation 'noname'\n",
    "\u{1b}[32;1m└───\u{1b}[0m\u{1b}[1mnixosConfigurations\u{1b}[0m\n",
    "\u{1b}[32;1m    ├───\u{1b}[0m\u{1b}[1mhost\u{1b}[0m: \u{1b}[35;1mNixOS configuration\u{1b}[0m\n",
    "\u{1b}[32;1m    └───\u{1b}[0m\u{1b}[1mhost2\u{1b}[0m\n",
);

fn tree_root_connectors_hydra() -> Vec<Node> {
    let tree = vec![
        children(
            node("legacyPackages", Kind::Attrset, &["legacyPackages"]),
            vec![children(
                node(
                    "x86_64-linux",
                    Kind::Attrset,
                    &["legacyPackages", "x86_64-linux"],
                ),
                vec![label(
                    drv(
                        node(
                            "hello",
                            Kind::Derivation,
                            &["legacyPackages", "x86_64-linux", "hello"],
                        ),
                        "hello",
                    ),
                    Label::Package,
                )],
            )],
        ),
        // §5.3: `hydraJobs` keeps the plain `derivation '<drv.name>'`
        // label at any depth (unlike packages/checks), so the leaf sits
        // below show's three-level white-list.
        children(
            node("hydraJobs", Kind::Attrset, &["hydraJobs"]),
            vec![children(
                node("jobsets", Kind::Attrset, &["hydraJobs", "jobsets"]),
                vec![children(
                    node(
                        "x86_64-linux",
                        Kind::Attrset,
                        &["hydraJobs", "jobsets", "x86_64-linux"],
                    ),
                    vec![drv(
                        node(
                            "job",
                            Kind::Derivation,
                            &["hydraJobs", "jobsets", "x86_64-linux", "job"],
                        ),
                        "job",
                    )],
                )],
            )],
        ),
    ];
    tree
}

fn tree_dev_shell_label() -> Vec<Node> {
    let tree = vec![children(
        node("devShells", Kind::Attrset, &["devShells"]),
        vec![children(
            node(
                "x86_64-linux",
                Kind::Attrset,
                &["devShells", "x86_64-linux"],
            ),
            vec![label(
                drv(
                    node(
                        "default",
                        Kind::Derivation,
                        &["devShells", "x86_64-linux", "default"],
                    ),
                    "nix-shell",
                ),
                Label::DevelopmentEnvironment,
            )],
        )],
    )];
    tree
}

fn tree_config_app_template_error() -> Vec<Node> {
    let tree = vec![
        children(
            node(
                "nixosConfigurations",
                Kind::Attrset,
                &["nixosConfigurations"],
            ),
            vec![
                config(
                    node("host", Kind::Config, &["nixosConfigurations", "host"]),
                    ConfigType::Nixos,
                ),
                config(
                    node("hm", Kind::Config, &["nixosConfigurations", "hm"]),
                    ConfigType::HomeManager,
                ),
                config(
                    node("darwin", Kind::Config, &["nixosConfigurations", "darwin"]),
                    ConfigType::Darwin,
                ),
            ],
        ),
        children(
            node("apps", Kind::Attrset, &["apps"]),
            vec![children(
                node("x86_64-linux", Kind::Attrset, &["apps", "x86_64-linux"]),
                vec![
                    desc(
                        node("foo", Kind::App, &["apps", "x86_64-linux", "foo"]),
                        Some("does foo"),
                    ),
                    desc(
                        node("bar", Kind::App, &["apps", "x86_64-linux", "bar"]),
                        None,
                    ),
                ],
            )],
        ),
        children(
            node("templates", Kind::Attrset, &["templates"]),
            vec![desc(
                node("tpl", Kind::Template, &["templates", "tpl"]),
                Some("a template"),
            )],
        ),
        node("broken", Kind::Error, &["broken"]),
        node("scalar", Kind::String, &["scalar"]),
    ];
    tree
}

fn tree_non_derivation_labels() -> Vec<Node> {
    let tree = vec![
        node("functions", Kind::Function, &["functions"]),
        node("list", Kind::List, &["list"]),
        node("null", Kind::Null, &["null"]),
    ];
    tree
}

// §5.3/§5.5: `checks` keeps the plain derivation label at any depth
// (upstream `nix flake show` only looks three levels deep).  Depth 4 is
// deliberate: the namespace semantics survive beyond show's white-list.
fn tree_checks_depth() -> Vec<Node> {
    let tree = vec![children(
        node("checks", Kind::Attrset, &["checks"]),
        vec![children(
            node("x86_64-linux", Kind::Attrset, &["checks", "x86_64-linux"]),
            vec![
                drv(
                    node(
                        "unit",
                        Kind::Derivation,
                        &["checks", "x86_64-linux", "unit"],
                    ),
                    "unit-check",
                ),
                children(
                    node("nixos", Kind::Attrset, &["checks", "x86_64-linux", "nixos"]),
                    vec![drv(
                        node(
                            "deep",
                            Kind::Derivation,
                            &["checks", "x86_64-linux", "nixos", "deep"],
                        ),
                        "deep-check",
                    )],
                ),
            ],
        )],
    )];
    tree
}

// §5.3: `formatter` and `defaultPackage` derivations both use the
// `package` label (previously only packages/devShells were asserted).
fn tree_formatter_default_package() -> Vec<Node> {
    let tree = vec![
        children(
            node("formatter", Kind::Attrset, &["formatter"]),
            vec![label(
                drv(
                    node(
                        "x86_64-linux",
                        Kind::Derivation,
                        &["formatter", "x86_64-linux"],
                    ),
                    "nixfmt",
                ),
                Label::Package,
            )],
        ),
        children(
            node("defaultPackage", Kind::Attrset, &["defaultPackage"]),
            vec![label(
                drv(
                    node(
                        "x86_64-linux",
                        Kind::Derivation,
                        &["defaultPackage", "x86_64-linux"],
                    ),
                    "default-pkg",
                ),
                Label::Package,
            )],
        ),
    ];
    tree
}

// §5.3: the `defaultApp` / `defaultTemplate` shortcut namespaces; a
// missing description renders the bold `no description` fallback (the
// golden test only covers `defaultApp` *with* a description).
fn tree_default_app_template() -> Vec<Node> {
    let tree = vec![
        children(
            node("defaultApp", Kind::Attrset, &["defaultApp"]),
            vec![desc(
                node("x86_64-linux", Kind::App, &["defaultApp", "x86_64-linux"]),
                None,
            )],
        ),
        desc(
            node("defaultTemplate", Kind::Template, &["defaultTemplate"]),
            None,
        ),
    ];
    tree
}

// §5.5 (Gate 1 decision): non-derivation leaves render as bare name
// nodes.  Upstream `nix flake show` prints `: NixOS module` /
// `: Nixpkgs overlay` for these namespaces; fsn deliberately does not
// (README difference table: tagging does not recurse, whereas a normal attrset does).
fn tree_overlays_nixos_modules() -> Vec<Node> {
    let tree = vec![
        children(
            node("nixosModules", Kind::Attrset, &["nixosModules"]),
            vec![node("foo", Kind::Function, &["nixosModules", "foo"])],
        ),
        children(
            node("overlays", Kind::Attrset, &["overlays"]),
            vec![node("bar", Kind::Function, &["overlays", "bar"])],
        ),
    ];
    tree
}

// §3/§5.1: multiple `--part` subtrees form a flat forest of root nodes;
// every root gets its own connector, and each subtree is indented
// according to its own position (`│   ` when an earlier/later sibling
// exists, `    ` for the last one).
fn tree_multi_part_forest() -> Vec<Node> {
    let tree = vec![
        children(
            node("packages", Kind::Attrset, &["packages"]),
            vec![
                children(
                    node(
                        "aarch64-linux",
                        Kind::Attrset,
                        &["packages", "aarch64-linux"],
                    ),
                    vec![label(
                        drv(
                            node(
                                "hello",
                                Kind::Derivation,
                                &["packages", "aarch64-linux", "hello"],
                            ),
                            "hello-arm",
                        ),
                        Label::Package,
                    )],
                ),
                children(
                    node("x86_64-linux", Kind::Attrset, &["packages", "x86_64-linux"]),
                    vec![label(
                        drv(
                            node(
                                "hello",
                                Kind::Derivation,
                                &["packages", "x86_64-linux", "hello"],
                            ),
                            "hello-amd",
                        ),
                        Label::Package,
                    )],
                ),
            ],
        ),
        children(
            node("legacyPackages", Kind::Attrset, &["legacyPackages"]),
            vec![children(
                node(
                    "x86_64-linux",
                    Kind::Attrset,
                    &["legacyPackages", "x86_64-linux"],
                ),
                vec![node(
                    "nested",
                    Kind::Function,
                    &["legacyPackages", "x86_64-linux", "nested"],
                )],
            )],
        ),
        node("functions", Kind::Function, &["functions"]),
    ];
    tree
}

// §5.1: every `│   `/`    ` combination at depth > 3.  The leaf sits at
// depth 5 under branches that alternate between mid (`├───`) and last
// (`└───`) connectors at each level.
fn tree_deep_nesting() -> Vec<Node> {
    let tree = vec![children(
        node("a", Kind::Attrset, &["a"]),
        vec![
            children(
                node("b1", Kind::Attrset, &["a", "b1"]),
                vec![
                    children(
                        node("c1", Kind::Attrset, &["a", "b1", "c1"]),
                        vec![
                            children(
                                node("d1", Kind::Attrset, &["a", "b1", "c1", "d1"]),
                                vec![node("e1", Kind::String, &["a", "b1", "c1", "d1", "e1"])],
                            ),
                            node("d2", Kind::String, &["a", "b1", "c1", "d2"]),
                        ],
                    ),
                    node("c2", Kind::String, &["a", "b1", "c2"]),
                ],
            ),
            node("b2", Kind::String, &["a", "b2"]),
        ],
    )];
    tree
}

// §5.5: an error leaf as the *last* child (`└───`); the existing
// config/app/error test only covers the `├───` connector for errors.
fn tree_error_last_child() -> Vec<Node> {
    let tree = vec![children(
        node("packages", Kind::Attrset, &["packages"]),
        vec![
            node("ok", Kind::String, &["packages", "ok"]),
            node("broken", Kind::Error, &["packages", "broken"]),
        ],
    )];
    tree
}

// Label fallbacks dropped in Phase 2: the walker always sets `drvName` on
// derivation records, so the old `'unknown'` fallback for a missing name is
// unreachable, and `configType` is now a strict enum that cannot carry a bogus
// value (the old red `unknown` branch was deleted). This case pins the
// surviving valid behavior instead: a derivation without a label renders the
// plain `derivation '<drv.name>'` wording, a valid typed config renders its
// magenta wording, and a config without a type renders no annotation.
fn tree_fallback_labels() -> Vec<Node> {
    let tree = vec![
        children(
            node("legacyPackages", Kind::Attrset, &["legacyPackages"]),
            vec![children(
                node(
                    "x86_64-linux",
                    Kind::Attrset,
                    &["legacyPackages", "x86_64-linux"],
                ),
                vec![drv(
                    node(
                        "noname",
                        Kind::Derivation,
                        &["legacyPackages", "x86_64-linux", "noname"],
                    ),
                    "noname",
                )],
            )],
        ),
        children(
            node(
                "nixosConfigurations",
                Kind::Attrset,
                &["nixosConfigurations"],
            ),
            vec![
                config(
                    node("host", Kind::Config, &["nixosConfigurations", "host"]),
                    ConfigType::Nixos,
                ),
                node("host2", Kind::Config, &["nixosConfigurations", "host2"]),
            ],
        ),
    ];
    tree
}

// ---- table-driven text rendering checks --------------------------------------

/// One table-driven text case: (case name, expected bytes, tree builder); every
/// case's expected output is inlined in this file.
type TextCase = (&'static str, &'static str, fn() -> Vec<Node>);

const TEXT_CASES: &[TextCase] = &[
    (
        "root-connectors-hydra",
        EXPECTED_ROOT_CONNECTORS_HYDRA,
        tree_root_connectors_hydra,
    ),
    (
        "dev-shell-label",
        EXPECTED_DEV_SHELL_LABEL,
        tree_dev_shell_label,
    ),
    (
        "config-app-template-error",
        EXPECTED_CONFIG_APP_TEMPLATE_ERROR,
        tree_config_app_template_error,
    ),
    (
        "non-derivation-labels",
        EXPECTED_NON_DERIVATION_LABELS,
        tree_non_derivation_labels,
    ),
    ("checks-depth", EXPECTED_CHECKS_DEPTH, tree_checks_depth),
    (
        "formatter-default-package",
        EXPECTED_FORMATTER_DEFAULT_PACKAGE,
        tree_formatter_default_package,
    ),
    (
        "default-app-template",
        EXPECTED_DEFAULT_APP_TEMPLATE,
        tree_default_app_template,
    ),
    (
        "overlays-nixos-modules",
        EXPECTED_OVERLAYS_NIXOS_MODULES,
        tree_overlays_nixos_modules,
    ),
    (
        "multi-part-forest",
        EXPECTED_MULTI_PART_FOREST,
        tree_multi_part_forest,
    ),
    ("deep-nesting", EXPECTED_DEEP_NESTING, tree_deep_nesting),
    (
        "error-last-child",
        EXPECTED_ERROR_LAST_CHILD,
        tree_error_last_child,
    ),
    (
        "fallback-labels",
        EXPECTED_FALLBACK_LABELS,
        tree_fallback_labels,
    ),
];

/// Byte-level text rendering checks: each case's failure is printed and all
/// failures are summarized in a single panic.
#[test]
fn renders_text_goldens() {
    let mut failures: Vec<String> = Vec::new();
    for (name, expected, build) in TEXT_CASES {
        let rendered = text::render("path:/tmp/flake", &build());
        if &rendered != expected {
            failures.push(format!(
                "FAIL {name}: expected {expected:?}, rendered {rendered:?}"
            ));
        }
    }
    for failure in &failures {
        println!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} text rendering case(s) failed",
        failures.len()
    );
}

/// Byte-level rendering oracle recorded from `nix flake show
/// --legacy --all-systems path:.` (design §8: use `show.txt`, never a
/// pty capture).  The `apps` fixture has no empty attrsets and no leaves
/// that upstream would prune, so fsn's tree is structurally identical and
/// every tree line must match byte for byte.  Only the root line is skipped:
/// it carries the flake ref (`path:...?lastModified=...` vs fsn's
/// `nix flake metadata` URL), not rendering.
#[test]
fn matches_recorded_golden_capture_for_apps_fixture() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/app-leaves/show.txt"
    ))
    .expect("tests/app-leaves/show.txt must exist");
    let golden_tree: String = golden
        .lines()
        .skip(1)
        .map(|line| format!("{line}\n"))
        .collect();

    let tree = vec![
        children(
            node("apps", Kind::Attrset, &["apps"]),
            vec![children(
                node("x86_64-linux", Kind::Attrset, &["apps", "x86_64-linux"]),
                vec![
                    desc(
                        node("noDesc", Kind::App, &["apps", "x86_64-linux", "noDesc"]),
                        None,
                    ),
                    desc(
                        node("withDesc", Kind::App, &["apps", "x86_64-linux", "withDesc"]),
                        Some("an app with a description"),
                    ),
                ],
            )],
        ),
        children(
            node("defaultApp", Kind::Attrset, &["defaultApp"]),
            vec![desc(
                node("x86_64-linux", Kind::App, &["defaultApp", "x86_64-linux"]),
                Some("the default app"),
            )],
        ),
    ];

    let rendered = text::render("path:/tmp/placeholder", &tree);
    let rendered_tree: String = rendered
        .lines()
        .skip(1)
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(rendered_tree, golden_tree);
}

// ---- JSON renderer (structural assertions) -----------------------------------

#[test]
fn renders_schema_v1_document() {
    let tree = vec![children(
        node("packages", Kind::Attrset, &["packages"]),
        vec![children(
            node("x86_64-linux", Kind::Attrset, &["packages", "x86_64-linux"]),
            vec![label(
                drv(
                    node(
                        "hello",
                        Kind::Derivation,
                        &["packages", "x86_64-linux", "hello"],
                    ),
                    "hello",
                ),
                Label::Package,
            )],
        )],
    )];
    let parts = vec![AttrPath::from_components(vec![
        "packages".to_owned(),
        "x86_64-linux".to_owned(),
    ])];
    let json = json::render("git+file:///repo", &parts, &tree).unwrap();

    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["lockedRef"], "git+file:///repo");
    assert_eq!(value["input"], ".");
    assert_eq!(value["parts"][0], "packages.x86_64-linux");
    let hello = &value["tree"][0]["children"][0]["children"][0];
    assert_eq!(hello["name"], "hello");
    assert_eq!(hello["kind"], "derivation");
    assert_eq!(hello["drvName"], "hello");
    assert_eq!(hello["label"], "package");
    assert_eq!(hello["attrpath"], "packages.x86_64-linux.hello");
    assert_eq!(hello["path"][2], "hello");

    // Field presence rules: attrset has children, derivation has drvName,
    // app/template have a description (possibly null), nothing else.
    assert!(value["tree"][0].get("children").is_some());
    assert!(value["tree"][0].get("drvName").is_none());
    assert!(hello.get("children").is_none());
    assert!(hello.get("description").is_none());
    assert!(hello.get("configType").is_none());
}

#[test]
fn serializes_null_description_and_config_type() {
    let tree = vec![
        desc(node("apps", Kind::App, &["apps", "foo"]), None),
        desc(node("tpl", Kind::Template, &["tpl"]), None),
        config(
            node("host", Kind::Config, &["nixosConfigurations", "host"]),
            ConfigType::Nixos,
        ),
    ];
    let json = json::render("path:/repo", &[], &tree).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value["tree"][0]["description"].is_null());
    assert!(value["tree"][0]["description"].as_str().is_none());
    assert_eq!(value["tree"][1]["description"], serde_json::Value::Null);
    assert_eq!(value["tree"][2]["configType"], "nixos");
    assert_eq!(value["parts"], serde_json::json!([]));
}

#[test]
fn round_trips_through_walker_shape() {
    // The document must be re-parseable with the same node semantics.
    let tree = vec![children(
        node("apps", Kind::Attrset, &["apps"]),
        vec![desc(node("foo", Kind::App, &["apps", "foo"]), Some("hi"))],
    )];
    let json = json::render("path:/repo", &[], &tree).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let parsed: WalkerOutput =
        serde_json::from_value(serde_json::json!({"tree": value["tree"]})).unwrap();
    assert_eq!(parsed.tree, tree);
    assert_eq!(
        parsed.tree[0].children.as_ref().unwrap()[0].description,
        Some(Some("hi".to_owned()))
    );
}

/// Byte-for-byte snapshots of `json::render` output, captured from the current
/// implementation.  They pin JSON schema v1 so an internal refactor cannot
/// silently drift from the wire format.
#[test]
fn json_render_byte_snapshots() {
    struct Case {
        name: &'static str,
        locked_ref: &'static str,
        parts: &'static [&'static str],
        tree: Vec<Node>,
        expected: &'static str,
    }

    let cases: Vec<Case> = vec![
        Case {
            name: "attrset-nested-empty",
            locked_ref: "git+file:///repo",
            parts: &["packages.x86_64-linux"],
            tree: vec![children(
                node("packages", Kind::Attrset, &["packages"]),
                vec![children(
                    node("x86_64-linux", Kind::Attrset, &["packages", "x86_64-linux"]),
                    vec![],
                )],
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"git+file:///repo\",\n  \"input\": \".\",\n  \"parts\": [\n    \"packages.x86_64-linux\"\n  ],\n  \"tree\": [\n    {\n      \"name\": \"packages\",\n      \"kind\": \"attrset\",\n      \"path\": [\n        \"packages\"\n      ],\n      \"attrpath\": \"packages\",\n      \"children\": [\n        {\n          \"name\": \"x86_64-linux\",\n          \"kind\": \"attrset\",\n          \"path\": [\n            \"packages\",\n            \"x86_64-linux\"\n          ],\n          \"attrpath\": \"packages.x86_64-linux\",\n          \"children\": []\n        }\n      ]\n    }\n  ]\n}\n",
        },
        Case {
            name: "derivation-drvname",
            locked_ref: "git+file:///repo",
            parts: &["packages.x86_64-linux"],
            tree: vec![label(
                drv(
                    node(
                        "hello",
                        Kind::Derivation,
                        &["packages", "x86_64-linux", "hello"],
                    ),
                    "hello",
                ),
                Label::Package,
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"git+file:///repo\",\n  \"input\": \".\",\n  \"parts\": [\n    \"packages.x86_64-linux\"\n  ],\n  \"tree\": [\n    {\n      \"name\": \"hello\",\n      \"kind\": \"derivation\",\n      \"path\": [\n        \"packages\",\n        \"x86_64-linux\",\n        \"hello\"\n      ],\n      \"attrpath\": \"packages.x86_64-linux.hello\",\n      \"drvName\": \"hello\",\n      \"label\": \"package\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "derivation-development-environment",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![label(
                drv(
                    node(
                        "default",
                        Kind::Derivation,
                        &["devShells", "x86_64-linux", "default"],
                    ),
                    "nix-shell",
                ),
                Label::DevelopmentEnvironment,
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"default\",\n      \"kind\": \"derivation\",\n      \"path\": [\n        \"devShells\",\n        \"x86_64-linux\",\n        \"default\"\n      ],\n      \"attrpath\": \"devShells.x86_64-linux.default\",\n      \"drvName\": \"nix-shell\",\n      \"label\": \"development-environment\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "app-with-description",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![desc(
                node("foo", Kind::App, &["apps", "x86_64-linux", "foo"]),
                Some("does foo"),
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"foo\",\n      \"kind\": \"app\",\n      \"path\": [\n        \"apps\",\n        \"x86_64-linux\",\n        \"foo\"\n      ],\n      \"attrpath\": \"apps.x86_64-linux.foo\",\n      \"description\": \"does foo\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "app-no-description",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![desc(
                node("bar", Kind::App, &["apps", "x86_64-linux", "bar"]),
                None,
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"bar\",\n      \"kind\": \"app\",\n      \"path\": [\n        \"apps\",\n        \"x86_64-linux\",\n        \"bar\"\n      ],\n      \"attrpath\": \"apps.x86_64-linux.bar\",\n      \"description\": null\n    }\n  ]\n}\n",
        },
        Case {
            name: "template-with-description",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![desc(
                node("tpl", Kind::Template, &["templates", "tpl"]),
                Some("a template"),
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"tpl\",\n      \"kind\": \"template\",\n      \"path\": [\n        \"templates\",\n        \"tpl\"\n      ],\n      \"attrpath\": \"templates.tpl\",\n      \"description\": \"a template\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "config-nixos",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![config(
                node("host", Kind::Config, &["nixosConfigurations", "host"]),
                ConfigType::Nixos,
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"host\",\n      \"kind\": \"config\",\n      \"path\": [\n        \"nixosConfigurations\",\n        \"host\"\n      ],\n      \"attrpath\": \"nixosConfigurations.host\",\n      \"configType\": \"nixos\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "config-home-manager",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![config(
                node("user1", Kind::Config, &["homeConfigurations", "user1"]),
                ConfigType::HomeManager,
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"user1\",\n      \"kind\": \"config\",\n      \"path\": [\n        \"homeConfigurations\",\n        \"user1\"\n      ],\n      \"attrpath\": \"homeConfigurations.user1\",\n      \"configType\": \"home-manager\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "config-darwin",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![config(
                node("mac", Kind::Config, &["darwinConfigurations", "mac"]),
                ConfigType::Darwin,
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"mac\",\n      \"kind\": \"config\",\n      \"path\": [\n        \"darwinConfigurations\",\n        \"mac\"\n      ],\n      \"attrpath\": \"darwinConfigurations.mac\",\n      \"configType\": \"darwin\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "function",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("fn", Kind::Function, &["fn"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"fn\",\n      \"kind\": \"function\",\n      \"path\": [\n        \"fn\"\n      ],\n      \"attrpath\": \"fn\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "string",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("s", Kind::String, &["s"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"s\",\n      \"kind\": \"string\",\n      \"path\": [\n        \"s\"\n      ],\n      \"attrpath\": \"s\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "int",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("i", Kind::Int, &["i"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"i\",\n      \"kind\": \"int\",\n      \"path\": [\n        \"i\"\n      ],\n      \"attrpath\": \"i\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "float",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("f", Kind::Float, &["f"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"f\",\n      \"kind\": \"float\",\n      \"path\": [\n        \"f\"\n      ],\n      \"attrpath\": \"f\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "bool",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("b", Kind::Bool, &["b"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"b\",\n      \"kind\": \"bool\",\n      \"path\": [\n        \"b\"\n      ],\n      \"attrpath\": \"b\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "path",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("p", Kind::Path, &["p"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"p\",\n      \"kind\": \"path\",\n      \"path\": [\n        \"p\"\n      ],\n      \"attrpath\": \"p\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "null",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("n", Kind::Null, &["n"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"n\",\n      \"kind\": \"null\",\n      \"path\": [\n        \"n\"\n      ],\n      \"attrpath\": \"n\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "list",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("l", Kind::List, &["l"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"l\",\n      \"kind\": \"list\",\n      \"path\": [\n        \"l\"\n      ],\n      \"attrpath\": \"l\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "error",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("e", Kind::Error, &["e"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"e\",\n      \"kind\": \"error\",\n      \"path\": [\n        \"e\"\n      ],\n      \"attrpath\": \"e\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "empty-parts",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![desc(node("foo", Kind::App, &["apps", "foo"]), Some("hi"))],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"foo\",\n      \"kind\": \"app\",\n      \"path\": [\n        \"apps\",\n        \"foo\"\n      ],\n      \"attrpath\": \"apps.foo\",\n      \"description\": \"hi\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "multi-part-list",
            locked_ref: "git+file:///repo",
            parts: &["a.b", "c"],
            tree: vec![drv(
                node("job", Kind::Derivation, &["a.b", "c", "job"]),
                "job",
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"git+file:///repo\",\n  \"input\": \".\",\n  \"parts\": [\n    \"a.b\",\n    \"c\"\n  ],\n  \"tree\": [\n    {\n      \"name\": \"job\",\n      \"kind\": \"derivation\",\n      \"path\": [\n        \"a.b\",\n        \"c\",\n        \"job\"\n      ],\n      \"attrpath\": \"a.b.c.job\",\n      \"drvName\": \"job\"\n    }\n  ]\n}\n",
        },
        Case {
            name: "deep-nested",
            locked_ref: "path:/repo",
            parts: &["packages.x86_64-linux.set.leaf"],
            tree: vec![children(
                node("packages", Kind::Attrset, &["packages"]),
                vec![children(
                    node("x86_64-linux", Kind::Attrset, &["packages", "x86_64-linux"]),
                    vec![children(
                        node("set", Kind::Attrset, &["packages", "x86_64-linux", "set"]),
                        vec![node(
                            "leaf",
                            Kind::Int,
                            &["packages", "x86_64-linux", "set", "leaf"],
                        )],
                    )],
                )],
            )],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [\n    \"packages.x86_64-linux.set.leaf\"\n  ],\n  \"tree\": [\n    {\n      \"name\": \"packages\",\n      \"kind\": \"attrset\",\n      \"path\": [\n        \"packages\"\n      ],\n      \"attrpath\": \"packages\",\n      \"children\": [\n        {\n          \"name\": \"x86_64-linux\",\n          \"kind\": \"attrset\",\n          \"path\": [\n            \"packages\",\n            \"x86_64-linux\"\n          ],\n          \"attrpath\": \"packages.x86_64-linux\",\n          \"children\": [\n            {\n              \"name\": \"set\",\n              \"kind\": \"attrset\",\n              \"path\": [\n                \"packages\",\n                \"x86_64-linux\",\n                \"set\"\n              ],\n              \"attrpath\": \"packages.x86_64-linux.set\",\n              \"children\": [\n                {\n                  \"name\": \"leaf\",\n                  \"kind\": \"int\",\n                  \"path\": [\n                    \"packages\",\n                    \"x86_64-linux\",\n                    \"set\",\n                    \"leaf\"\n                  ],\n                  \"attrpath\": \"packages.x86_64-linux.set.leaf\"\n                }\n              ]\n            }\n          ]\n        }\n      ]\n    }\n  ]\n}\n",
        },
        Case {
            name: "unknown-kind",
            locked_ref: "path:/repo",
            parts: &[],
            tree: vec![node("u", Kind::Unknown, &["u"])],
            expected: "{\n  \"schemaVersion\": 1,\n  \"lockedRef\": \"path:/repo\",\n  \"input\": \".\",\n  \"parts\": [],\n  \"tree\": [\n    {\n      \"name\": \"u\",\n      \"kind\": \"unknown\",\n      \"path\": [\n        \"u\"\n      ],\n      \"attrpath\": \"u\"\n    }\n  ]\n}\n",
        },
    ];

    let mut failures: Vec<String> = Vec::new();
    for case in cases {
        let parts: Vec<AttrPath> = case
            .parts
            .iter()
            .map(|s| s.parse::<AttrPath>().unwrap())
            .collect();
        let rendered = json::render(case.locked_ref, &parts, &case.tree).unwrap();
        if rendered != case.expected {
            failures.push(format!(
                "FAIL {}: expected {:?}, rendered {:?}",
                case.name, case.expected, rendered
            ));
        }
    }
    for failure in &failures {
        println!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} JSON byte snapshot(s) failed",
        failures.len()
    );
}
