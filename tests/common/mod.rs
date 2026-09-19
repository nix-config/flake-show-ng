//! Shared engine for the `fsn` integration suite (see `tests/integration.rs`).
//!
//! `tests/integration.rs` owns only the env gate and the prerequisite call; the
//! scenario set is data in [`scenarios`], and the one driver below copies each
//! fixture directory into a throwaway temp dir (fsn only acts on its cwd), runs
//! the scenario's setup hook, invokes fsn with the typed argv and applies the
//! scenario's oracle.  All comparisons stay exact: `show.json`/`show.txt` are
//! verified byte-for-byte by sha256 before any run, and the
//! `git-dirty-root-line` root line is compared byte-for-byte against a live
//! `nix flake show`.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use flake_show_ng::model::{ConfigType, Kind, Label, Node};
use serde::Deserialize;
use serde_json::Value;

use self::scenarios::{Check, Format, SCENARIOS, Scenario};

pub mod scenarios;

/// Support directories that hold harness code, not scenarios (excluded from the
/// m1 scenario-set check).
pub const SUPPORT_DIRS: &[&str] = &["common", "render-text"];

/// Namespaces `nix flake show` knows about (subset relevant for the fixtures).
pub const SHOW_ROOTS: &[&str] = &[
    "packages",
    "legacyPackages",
    "checks",
    "devShells",
    "apps",
    "templates",
    "formatter",
    "defaultPackage",
    "defaultApp",
    "nixosConfigurations",
    "nixosModules",
    "overlays",
    "hydraJobs",
];

/// Extension namespaces (fsn only; design §5.5).
pub const EXT_ROOTS: &[&str] = &["homeConfigurations", "darwinConfigurations"];

pub fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// One scenario run: the temp copy of `tests/<fixture>` (cwd for `fsn`), the
/// recorded fixture directory, the fsn binary and the parsed manifest.  `work`
/// is keyed by the row `name` (unique), `fixture` by the shared fixture dir.
pub struct ScenarioContext<'a> {
    pub scenario: &'a Scenario,
    /// Throwaway copy of the fixture; every process runs with this as cwd.
    pub work: &'a Path,
    /// Original `tests/<fixture>`; goldens and manifest hashes are read here.
    pub fixture: &'a Path,
    pub bin: &'a str,
    pub manifest: &'a Manifest,
}

// ---- golden mode (criteria 1-4) ----------------------------------------------

/// The parts of the fsn JSON document the harness reads.  Unknown top-level
/// fields (`schemaVersion`, `lockedRef`, `parts`, ...) are ignored.
#[derive(Deserialize)]
struct FsnDocument {
    #[serde(default)]
    tree: Vec<Node>,
}

/// Criteria 1-4: run `fsn -f json` in the scenario workdir and compare the
/// resulting tree against the recorded `show.json`.  Returns the parsed tree
/// (when it could be produced) plus every problem found; scenarios add their own
/// Gate 4 M1 positive assertions on the returned tree.
pub fn compare_golden(ctx: &ScenarioContext<'_>) -> (Option<Vec<Node>>, Vec<String>) {
    let out = match run_primary(ctx) {
        Ok(out) => out,
        Err(err) => return (None, vec![err]),
    };
    let fsn_exit = exit_code(&out);
    let show = match read_json(&ctx.fixture.join("show.json")) {
        Ok(value) => value,
        Err(err) => return (None, vec![err]),
    };
    // P3: the recorded JSON run must have succeeded (the golden tree is only
    // meaningful when show exited 0).
    let mut problems = expect_manifest_exit(ctx, ExitKind::Json, 0);
    if fsn_exit != 0 {
        problems.push(format!("fsn exited {fsn_exit}; golden show.json succeeded"));
        return (None, problems);
    }
    match serde_json::from_slice::<FsnDocument>(&out.stdout) {
        Ok(document) => {
            problems.extend(compare_against_show(
                fsn_exit,
                &document.tree,
                &show,
                "golden show.json",
            ));
            (Some(document.tree), problems)
        }
        Err(err) => {
            problems.push(format!("fsn stdout is not valid JSON: {err}"));
            (None, problems)
        }
    }
}

/// Hardcoded mode (criterion 5): `fsn -f json` must succeed and parse.
pub fn fsn_json_ok(ctx: &ScenarioContext<'_>) -> Result<Vec<Node>, Vec<String>> {
    let out = match run_primary(ctx) {
        Ok(out) => out,
        Err(err) => return Err(vec![err]),
    };
    let fsn_exit = exit_code(&out);
    if fsn_exit != 0 {
        return Err(vec![format!("fsn exited {fsn_exit}, expected 0")]);
    }
    serde_json::from_slice::<FsnDocument>(&out.stdout)
        .map(|document| document.tree)
        .map_err(|err| vec![format!("fsn stdout is not valid JSON: {err}")])
}

// ---- Gate 4 M1: positive node assertions -------------------------------------

/// One expected field of a semantic node (Gate 4 M1): `(key, Some(want))`
/// requires an exact string match, `(key, None)` requires null/absent.
pub type FieldExpectation = (&'static str, Option<&'static str>);

/// One expected semantic node (Gate 4 M1): `(attrpath, kind, fields)`, checked
/// by [`assert_nodes`] against the typed fsn tree.
pub type NodeExpectation = (&'static str, Kind, &'static [FieldExpectation]);

/// Gate 4 M1: assert semantic nodes by attrpath against the typed fsn tree.
///
/// The show-based comparison silently exempts whole classes of nodes, so every
/// scenario declares the nodes it cannot afford to lose.
pub fn assert_nodes(tree: &[Node], expectations: &[NodeExpectation]) -> Vec<String> {
    let mut nodes: BTreeMap<String, &Node> = BTreeMap::new();
    for node in tree {
        walk_fsn(node, &mut nodes);
    }
    let mut problems = Vec::new();
    for (attrpath, kind, fields) in expectations.iter().copied() {
        match nodes.get(attrpath) {
            None => problems.push(format!(
                "{attrpath}: missing from fsn tree (positive assertion)"
            )),
            Some(node) => {
                if node.kind != kind {
                    problems.push(format!(
                        "{attrpath}: kind={:?}, want {kind:?}",
                        Some(kind_name(node.kind))
                    ));
                }
                for (key, want) in fields.iter().copied() {
                    let got = node_field(node, key);
                    match want {
                        Some(_) if got.as_deref() != want => {
                            problems.push(format!("{attrpath}: {key}={got:?}, want {want:?}"));
                        }
                        None if got.is_some() => {
                            problems.push(format!("{attrpath}: {key}={got:?}, want null/absent"));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    problems
}

/// Typed field access for the M1 assertion tables, exhaustive over the model so
/// a new kind cannot be silently forgotten.
fn node_field(node: &Node, key: &str) -> Option<String> {
    match key {
        "drvName" => node.drv_name.clone(),
        "description" => node.description.clone().flatten(),
        "configType" => node
            .config_type
            .map(|value| config_type_name(value).to_owned()),
        "label" => node.label.map(|value| label_name(value).to_owned()),
        _ => None,
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Attrset => "attrset",
        Kind::Derivation => "derivation",
        Kind::App => "app",
        Kind::Template => "template",
        Kind::Config => "config",
        Kind::Function => "function",
        Kind::String => "string",
        Kind::Int => "int",
        Kind::Float => "float",
        Kind::Bool => "bool",
        Kind::Path => "path",
        Kind::Null => "null",
        Kind::List => "list",
        Kind::Error => "error",
        Kind::Unknown => "unknown",
    }
}

fn config_type_name(config_type: ConfigType) -> &'static str {
    match config_type {
        ConfigType::Nixos => "nixos",
        ConfigType::HomeManager => "home-manager",
        ConfigType::Darwin => "darwin",
    }
}

fn label_name(label: Label) -> &'static str {
    match label {
        Label::Package => "package",
        Label::DevelopmentEnvironment => "development-environment",
    }
}

// ---- manifest ----------------------------------------------------------------

/// The typed `tests/manifest.json`: only the fields the harness reads are
/// modelled; the recording commands and other metadata are ignored by serde.
#[derive(Debug, Default, Deserialize)]
pub struct Manifest {
    #[serde(rename = "nixVersion", default)]
    pub nix_version: String,
    #[serde(default)]
    pub fixtures: BTreeMap<String, Case>,
}

/// One recorded scenario entry.
#[derive(Debug, Default, Deserialize)]
pub struct Case {
    #[serde(default)]
    pub exits: Exits,
    #[serde(rename = "fixtureFiles", default)]
    pub fixture_files: BTreeMap<String, String>,
    #[serde(rename = "goldenFiles", default)]
    pub golden_files: BTreeMap<String, String>,
}

/// Recorded upstream `nix flake show` exit codes.
#[derive(Debug, Default, Deserialize)]
pub struct Exits {
    #[serde(default)]
    pub json: Option<i64>,
    #[serde(default)]
    pub text: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitKind {
    Json,
    Text,
}

impl ExitKind {
    fn as_str(self) -> &'static str {
        match self {
            ExitKind::Json => "json",
            ExitKind::Text => "text",
        }
    }
}

pub fn manifest_exit(manifest: &Manifest, scenario: &str, kind: ExitKind) -> Option<i64> {
    let exits = &manifest.fixtures.get(scenario)?.exits;
    match kind {
        ExitKind::Json => exits.json,
        ExitKind::Text => exits.text,
    }
}

/// The one recorded-exit assertion every hardcoded scenario shares; returns a
/// list so callers append it to their other problems.
pub fn expect_manifest_exit(ctx: &ScenarioContext<'_>, kind: ExitKind, want: i64) -> Vec<String> {
    match manifest_exit(ctx.manifest, ctx.scenario.fixture, kind) {
        Some(got) if got == want => Vec::new(),
        Some(got) => vec![format!(
            "manifest exits.{} is {got}, expected {want}; re-record golden data",
            kind.as_str()
        )],
        None => vec![format!(
            "manifest exits.{} is missing, expected {want}; re-record golden data",
            kind.as_str()
        )],
    }
}

pub fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let bytes = fs::read(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("{} is not a valid manifest: {err}", path.display()))
}

/// Round-trip every `sha256:<hex>` recorded in `manifest.json` against the
/// bytes on disk (`fixtureFiles` + `goldenFiles`).  Editing a golden without
/// re-recording the manifest fails the run during the prerequisite phase.
pub fn verify_manifest_hashes(tests: &Path, manifest: &Manifest) -> Vec<String> {
    let mut problems = Vec::new();
    for (scenario, case) in &manifest.fixtures {
        for files in [&case.fixture_files, &case.golden_files] {
            for (relative, want) in files {
                match file_sha256(&tests.join(scenario).join(relative)) {
                    Ok(got) if got == *want => {}
                    Ok(got) => problems.push(format!(
                        "{scenario}/{relative}: sha256 mismatch (disk={got}, manifest={want}); \
                         golden or fixture edited without re-recording manifest.json"
                    )),
                    Err(err) => problems.push(err),
                }
            }
        }
    }
    problems
}

/// `sha256sum` (coreutils, present in stdenv check phases) instead of a
/// hand-rolled hash.
fn file_sha256(path: &Path) -> Result<String, String> {
    let out = Command::new("sha256sum")
        .arg(path)
        .output()
        .map_err(|err| format!("cannot run sha256sum for {}: {err}", path.display()))?;
    if !out.status.success() {
        return Err(format!("sha256sum failed for {}", path.display()));
    }
    let hex = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned();
    Ok(format!("sha256:{hex}"))
}

// ---- criteria 1-4: attrpath-aligned node comparison --------------------------

pub fn compare_against_show(
    fsn_exit: i32,
    fsn: &[Node],
    show: &Value,
    source: &str,
) -> Vec<String> {
    if fsn_exit != 0 {
        return vec![format!("fsn exited {fsn_exit}; {source} succeeded")];
    }

    let mut problems = Vec::new();
    let mut show_nodes: BTreeMap<String, &Value> = BTreeMap::new();
    walk_show(show, &mut Vec::new(), &mut show_nodes);
    let pruned: BTreeSet<String> = show_nodes
        .iter()
        .filter(|(_, value)| value.as_object().map(|o| o.is_empty()).unwrap_or(false))
        .map(|(key, _)| key.clone())
        .collect();

    let mut fsn_nodes: BTreeMap<String, &Node> = BTreeMap::new();
    for node in fsn {
        walk_fsn(node, &mut fsn_nodes);
    }

    for (attrpath, node) in &fsn_nodes {
        let node = *node;
        let root = attrpath.split('.').next().unwrap_or("");
        match show_nodes.get(attrpath) {
            None => {
                // Exemption classes (asserted, not silenced).
                if node
                    .path
                    .ancestors()
                    .any(|ancestor| pruned.contains(&ancestor.to_string()))
                {
                    continue; // deeper than show's hardcoded white-list
                }
                if EXT_ROOTS.contains(&root) {
                    continue; // fsn extension namespace (home/darwin configs)
                }
                if SHOW_ROOTS.contains(&root) {
                    if empty_output_set(node) {
                        continue; // empty output set: pruned by show
                    }
                    // show physically cannot express these leaves: it warns
                    // ("<attrpath>.name is not a derivation") and omits them,
                    // so fsn rendering them is an expected extension.  Every
                    // semantic kind (derivation/app/template/config) *is*
                    // representable by show; if one is missing here, the
                    // golden/live oracle is stale or fsn invented a node, and
                    // that must FAIL (P2 regression guard, mutation J).
                    if matches!(
                        node.kind,
                        Kind::String
                            | Kind::Int
                            | Kind::Float
                            | Kind::Bool
                            | Kind::Path
                            | Kind::Null
                            | Kind::List
                            | Kind::Function
                    ) {
                        continue; // scalar/list leaf: warn+omit in show
                    }
                }
                problems.push(format!(
                    "{attrpath}: present in fsn but absent from {source}"
                ));
            }
            Some(show_node) => {
                if show_node.as_object().map(|o| o.is_empty()).unwrap_or(false) {
                    continue; // show omitted this leaf (non-deriv/IFD/depth) -> exempt
                }
                match show_node.get("type").and_then(Value::as_str) {
                    Some(show_type) => {
                        if let Some(leaf) = expected_from_show(show_node) {
                            if node.kind != leaf.kind {
                                problems.push(format!(
                                    "{attrpath}: kind mismatch (fsn={:?}, show type={:?})",
                                    Some(kind_name(node.kind)),
                                    Some(show_type)
                                ));
                                continue;
                            }
                            if node.drv_name != leaf.drv_name {
                                problems.push(format!(
                                    "{attrpath}: drvName mismatch (fsn={:?}, show={:?})",
                                    node.drv_name, leaf.drv_name
                                ));
                            }
                            let got_description =
                                node.description.as_ref().and_then(|value| value.as_deref());
                            if got_description != leaf.description.as_deref() {
                                problems.push(format!(
                                    "{attrpath}: description mismatch (fsn={got_description:?}, \
                                     show={:?})",
                                    leaf.description
                                ));
                            }
                            if node.config_type != leaf.config_type {
                                problems.push(format!(
                                    "{attrpath}: configType mismatch (fsn={:?}, show={:?})",
                                    node.config_type, leaf.config_type
                                ));
                            }
                        }
                        // unknown/unhandled show types are exempt (fsn expansion)
                    }
                    None => {
                        if node.kind != Kind::Attrset {
                            problems.push(format!(
                                "{attrpath}: show has an attrset but fsn kind={:?}",
                                Some(kind_name(node.kind))
                            ));
                        }
                    }
                }
            }
        }
    }

    // Reverse direction: every semantic leaf show produced must exist in fsn.
    for (attrpath, show_node) in &show_nodes {
        let show_node = *show_node;
        let Some(kind) = show_node.get("type").and_then(Value::as_str) else {
            continue;
        };
        if kind == "unknown" {
            continue; // fsn expansion may restructure unknown namespaces
        }
        if !fsn_nodes.contains_key(attrpath) {
            problems.push(format!("{attrpath}: show leaf missing from fsn tree"));
        }
    }
    problems
}

/// Flatten `show.json` into `{attrpath: node}`; leaf nodes carry `type`.
pub fn walk_show<'a>(
    node: &'a Value,
    prefix: &mut Vec<String>,
    out: &mut BTreeMap<String, &'a Value>,
) {
    let Some(map) = node.as_object() else {
        return;
    };
    for (name, value) in map {
        prefix.push(name.clone());
        out.insert(prefix.join("."), value);
        let is_empty = value.as_object().map(|o| o.is_empty()).unwrap_or(false);
        if value.is_object() && value.get("type").is_none() && !is_empty {
            walk_show(value, prefix, out);
        }
        prefix.pop();
    }
}

/// Flatten the typed fsn tree into `{attrpath: node}` keyed through
/// [`AttrPath`](flake_show_ng::attrpath::AttrPath)'s display form, so the
/// comparison again aligns against show's dotted attrpaths.
pub fn walk_fsn<'a>(node: &'a Node, out: &mut BTreeMap<String, &'a Node>) {
    // The walker always sets a non-empty `path`, so the display form is the key.
    out.insert(node.path.to_string(), node);
    if let Some(children) = &node.children {
        for child in children {
            walk_fsn(child, out);
        }
    }
}

/// Expected fsn semantic leaf derived from a show JSON node.
#[derive(Debug)]
pub struct ShowLeaf {
    pub kind: Kind,
    pub drv_name: Option<String>,
    pub description: Option<String>,
    pub config_type: Option<ConfigType>,
}

/// §5.3: derive the expected fsn semantic leaf from a show JSON node.
///
/// Returns `None` when the show node carries no comparable semantic
/// (`type:"unknown"`, derivation without `name`, unhandled type).
pub fn expected_from_show(show_node: &Value) -> Option<ShowLeaf> {
    match show_node.get("type").and_then(Value::as_str)? {
        "derivation" => {
            let name = show_node.get("name").and_then(Value::as_str)?;
            Some(ShowLeaf {
                kind: Kind::Derivation,
                drv_name: Some(name.to_owned()),
                description: None,
                config_type: None,
            })
        }
        "app" => Some(ShowLeaf {
            kind: Kind::App,
            drv_name: None,
            description: node_opt_string(show_node, "description"),
            config_type: None,
        }),
        "template" => Some(ShowLeaf {
            kind: Kind::Template,
            drv_name: None,
            description: node_opt_string(show_node, "description"),
            config_type: None,
        }),
        "nixos-configuration" | "nixosConfiguration" => Some(ShowLeaf {
            kind: Kind::Config,
            drv_name: None,
            description: None,
            config_type: Some(ConfigType::Nixos),
        }),
        _ => None,
    }
}

/// JSON `null` and a missing key both normalize to `None` (mirrors the old
/// python driver, where both became `None`).
pub fn node_opt_string(node: &Value, key: &str) -> Option<String> {
    match node.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

/// True when an attrset subtree carries no content (show prunes it entirely).
pub fn empty_output_set(node: &Node) -> bool {
    if node.kind != Kind::Attrset {
        return false;
    }
    node.children
        .as_ref()
        .map(|children| children.iter().all(empty_output_set))
        .unwrap_or(true)
}

pub fn dig<'a>(node: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let mut current = node;
    for key in keys {
        current = current.as_object()?.get(*key)?;
    }
    Some(current)
}

// ---- typed nix version -------------------------------------------------------

/// A parsed `major.minor.patch` version, ordered structurally (not by string).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    /// Parse the first whitespace-separated `X.Y[.Z]` token; `nix --version`
    /// prints `nix (Nix) 2.34.8`.
    pub fn parse(text: &str) -> Option<Version> {
        text.split_whitespace().find_map(|token| {
            let mut parts = token.split('.');
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next()?.parse().ok()?;
            let patch = parts.next().unwrap_or("0").parse().ok()?;
            Some(Version {
                major,
                minor,
                patch,
            })
        })
    }
}

/// The preflight decision in one place: the runtime is accepted when it parses
/// and is not older than the recorded baseline, or when the skip escape hatch
/// is set.  Kept separate from the message so it is unit-testable.
fn version_ok(found: Option<Version>, required: Version, skip: bool) -> bool {
    skip || found.is_some_and(|version| version >= required)
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The rendering baseline, parsed from the crate's single recorded-version
/// constant (`src/nix.rs`), so the tests cannot drift from the implementation.
pub fn required_nix_version() -> Version {
    Version::parse(flake_show_ng::nix::RECORDED_NIX_VERSION)
        .expect("flake_show_ng::nix::RECORDED_NIX_VERSION is a valid version")
}

// ---- process / fs helpers ----------------------------------------------------

pub fn run(bin: &str, args: &[&str], cwd: &Path) -> Result<Output, String> {
    Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|err| {
            format!(
                "cannot run `{bin} {}` in {}: {err}",
                args.join(" "),
                cwd.display()
            )
        })
}

pub fn run_status(bin: &str, args: &[&str], cwd: &Path) -> bool {
    Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

pub fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))
}

pub fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Remove SGR escape sequences (`\x1b\[[0-9;]*m`), like the old python driver.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            let mut probe = chars.clone();
            if probe.next() == Some('[') {
                let mut valid = true;
                loop {
                    match probe.peek().copied() {
                        Some(d) if d.is_ascii_digit() || d == ';' => {
                            probe.next();
                        }
                        Some('m') => {
                            probe.next();
                            break;
                        }
                        _ => {
                            valid = false;
                            break;
                        }
                    }
                }
                if valid {
                    chars = probe;
                    continue;
                }
            }
        }
        out.push(ch);
    }
    out
}

/// Nix prints this warning to stderr whenever the process has no non-loopback
/// network interface (e.g. inside the `nix build` sandbox) and the command
/// declares network use. `fsn` forwards nix's stderr verbatim, so in such an
/// environment the recorded fsn diagnostics are prefixed by this line once per
/// nix invocation. It is environment noise, not part of fsn's own wording.
const NIX_NO_INTERNET_WARNING: &[u8] =
    b"warning: you don't have Internet access; disabling some network-dependent features";

/// Drop whole stderr lines that are exactly the environment-dependent nix
/// warning above, so the recorded-byte oracles compare fsn's own diagnostics no
/// matter whether the host has a network interface.
pub fn strip_nix_env_warnings(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for line in input.split_inclusive(|&b| b == b'\n') {
        let content = line.strip_suffix(b"\n").unwrap_or(line);
        if content != NIX_NO_INTERNET_WARNING {
            out.extend_from_slice(line);
        }
    }
    out
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Unique temp dir under `$TMPDIR`/`/tmp` using only std (no tempfile crate).
pub fn create_temp_dir() -> PathBuf {
    let base = env::temp_dir();
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for attempt in 0..1000u32 {
        let candidate = base.join(format!("fsn-it-{pid}-{nanos}-{attempt}"));
        match fs::create_dir(&candidate) {
            Ok(()) => return candidate,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => continue,
        }
    }
    panic!("cannot create a unique temp dir under {}", base.display());
}

// ---- preflight + scenario runner (driven by `tests/integration.rs`) ----------

/// Everything the scenario runner needs once the prerequisites passed.
pub struct Preflight {
    /// Parsed `tests/manifest.json`.
    pub manifest: Manifest,
    /// fsn binary, resolved from `FSN_BIN` or the cargo-built debug binary.
    pub bin: String,
}

/// Prerequisite phase for the integration root: nix/git availability, manifest
/// load, m1 scenario-set rules (dirs == [`SCENARIOS`] == manifest keys, per-dir
/// `flake.nix`), the golden/fixture sha256 round-trip, and the fsn binary
/// resolution + probe.  Hard failures are never skips: they are printed as
/// `FAIL: ...` lines and abort the test.
pub fn preflight(tests: &Path) -> Preflight {
    let mut hard_failures: Vec<String> = Vec::new();
    let skip_version = env::var("FSN_IT_SKIP_NIX_VERSION").ok().as_deref() == Some("1");
    let required = required_nix_version();

    match Command::new("nix").arg("--version").output() {
        Err(err) => hard_failures.push(format!("FSN_IT=1 but 'nix' is not runnable: {err}")),
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let version = Version::parse(&text);
            // Typed ordering: a newer runtime is accepted, an older one is a
            // hard failure because the goldens are tied to the baseline.
            if !version_ok(version, required, skip_version) {
                hard_failures.push(match version {
                    Some(found) => format!(
                        "golden data is recorded with Nix {required} but found: {found} \
                         (set FSN_IT_SKIP_NIX_VERSION=1 to compare anyway)"
                    ),
                    None => format!(
                        "cannot parse `nix --version` output {text:?} (need >= {required}; \
                         set FSN_IT_SKIP_NIX_VERSION=1 to compare anyway)"
                    ),
                });
            }
        }
    }
    if let Err(err) = Command::new("git").arg("--version").output() {
        hard_failures.push(format!(
            "'git' is required for the git-dirty-root-line scenario: {err}"
        ));
    }

    let manifest = match read_manifest(&tests.join("manifest.json")) {
        Ok(manifest) => manifest,
        Err(err) => {
            hard_failures.push(format!("cannot load tests/manifest.json: {err}"));
            Manifest::default()
        }
    };
    if !skip_version {
        if let Some(recorded) = Version::parse(&manifest.nix_version) {
            if recorded != required {
                hard_failures.push(format!(
                    "manifest nixVersion is {recorded}, expected the recorded requirement \
                     {required}; re-record manifest.json"
                ));
            }
        }
    }

    // m1: scenario set completeness (dirs == unique SCENARIOS fixtures ==
    // unique manifest keys).  Several rows may share one fixture directory.
    let mut expected: Vec<String> = SCENARIOS.iter().map(|s| s.fixture.to_owned()).collect();
    expected.sort();
    expected.dedup();
    let mut actual: Vec<String> = Vec::new();
    match fs::read_dir(tests) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && !SUPPORT_DIRS.contains(&name.as_str())
                {
                    actual.push(name);
                }
            }
        }
        Err(err) => hard_failures.push(format!("cannot read {}: {err}", tests.display())),
    }
    actual.sort();
    if actual != expected {
        hard_failures.push(format!(
            "scenario dir set mismatch: expected {expected:?}, found {actual:?}"
        ));
    }
    // m1 (Phase 2c): every scenario directory is a self-contained fixture; the
    // per-scenario `test.rs` program no longer exists.
    for fixture in &expected {
        if !tests.join(fixture).join("flake.nix").is_file() {
            hard_failures.push(format!("scenario {fixture} has no flake.nix"));
        }
    }
    {
        let mut keys: Vec<&str> = manifest.fixtures.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let want: Vec<&str> = expected.iter().map(String::as_str).collect();
        if keys != want {
            hard_failures.push(format!("manifest fixtures keys mismatch: {keys:?}"));
        }
    }
    // m1 (Phase 5c): golden/fixture sha256 round-trip.  A golden edited without
    // re-recording manifest.json is a hard failure, not a silent pass.
    hard_failures.extend(verify_manifest_hashes(tests, &manifest));

    // ---- fsn binary ---------------------------------------------------------
    let bin = match env::var("FSN_BIN") {
        Ok(value) if !value.is_empty() => {
            if value.contains('/') {
                let path = PathBuf::from(&value);
                let abs = if path.is_absolute() {
                    path
                } else {
                    env::current_dir().unwrap_or_default().join(path)
                };
                if !abs.is_file() {
                    hard_failures.push(format!("fsn binary not found: FSN_BIN={}", abs.display()));
                }
                abs.to_string_lossy().into_owned()
            } else {
                value
            }
        }
        _ => env!("CARGO_BIN_EXE_fsn").to_owned(),
    };
    if hard_failures.is_empty() {
        // Probe once so a missing binary fails as a prerequisite, not per scenario.
        if let Err(err) = Command::new(&bin).arg("--version").output() {
            hard_failures.push(format!("cannot run fsn binary `{bin}`: {err}"));
        }
    }

    if !hard_failures.is_empty() {
        for failure in &hard_failures {
            println!("FAIL: {failure}");
        }
        panic!(
            "integration prerequisites failed ({} problem(s))",
            hard_failures.len()
        );
    }

    Preflight { manifest, bin }
}

// ---- scenario driver ---------------------------------------------------------

/// The typed argv for one invocation: every `-p` value (each its own argv
/// element), then `-f <format>`.
fn args_with_format(scenario: &Scenario, format: Format) -> Vec<&'static str> {
    let mut args = Vec::new();
    for part in scenario.parts {
        args.push("-p");
        args.push(part);
    }
    args.push("-f");
    args.push(format.as_flag());
    args
}

/// The typed argv for a scenario's primary invocation.
fn primary_args(scenario: &Scenario) -> Vec<&'static str> {
    args_with_format(scenario, scenario.format)
}

fn run_primary(ctx: &ScenarioContext<'_>) -> Result<Output, String> {
    let args = primary_args(ctx.scenario);
    run(ctx.bin, &args, ctx.work)
}

/// Apply one scenario's [`Check`] oracle and return its problems.
pub fn evaluate(ctx: &ScenarioContext<'_>) -> Vec<String> {
    match ctx.scenario.check {
        Check::Golden => {
            let (tree, mut problems) = compare_golden(ctx);
            if let Some(tree) = &tree {
                problems.extend(assert_nodes(tree, ctx.scenario.nodes));
            }
            problems
        }
        Check::JsonOnly { manifest_text_exit } => {
            let tree = match fsn_json_ok(ctx) {
                Ok(tree) => tree,
                Err(problems) => return problems,
            };
            let mut problems = assert_nodes(&tree, ctx.scenario.nodes);
            problems.extend(expect_manifest_exit(
                ctx,
                ExitKind::Text,
                manifest_text_exit,
            ));
            problems
        }
        Check::IfdDenied { manifest_text_exit } => evaluate_ifd(ctx, manifest_text_exit),
        Check::LiveShow => evaluate_live_show(ctx),
        Check::TextPart { stdout, stderr } => evaluate_text_part(ctx, stdout, stderr),
        Check::StderrGolden { exit, golden } => evaluate_stderr_golden(ctx, exit, golden),
    }
}

/// Criterion 6: IFD stays disabled, so fsn must fail with a non-empty stderr
/// that names the IFD option, and show is expected to have omitted the node.
fn evaluate_ifd(ctx: &ScenarioContext<'_>, manifest_text_exit: i64) -> Vec<String> {
    let out = match run_primary(ctx) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };
    let fsn_exit = exit_code(&out);
    let mut problems = expect_manifest_exit(ctx, ExitKind::Text, manifest_text_exit);
    if fsn_exit == 0 {
        problems.push("fsn exited 0, expected failure: IFD must be disabled by default".to_owned());
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let lower = stderr.to_lowercase();
        if stderr.trim().is_empty() {
            problems.push("fsn failed but wrote no diagnostics to stderr".to_owned());
        } else if !(lower.contains("ifd")
            || lower.contains("import from derivation")
            || lower.contains("allow-ifd"))
        {
            problems.push(
                "fsn stderr does not mention IFD / import from derivation / --allow-ifd".to_owned(),
            );
        }
    }
    problems
}

/// Criterion 7: turn the fixture copy into a dirty git worktree, then use a
/// live `nix flake show` as the oracle.  The root line is compared byte for
/// byte; the rest of the tree goes through [`compare_against_show`].
fn evaluate_live_show(ctx: &ScenarioContext<'_>) -> Vec<String> {
    for command in ctx.scenario.setup {
        if !run_status("git", command, ctx.work) {
            return vec![format!(
                "git-dirty-root-line worktree setup (`git {}`) failed",
                command.join(" ")
            )];
        }
    }

    let show_text = match run(
        "nix",
        &["flake", "show", "--legacy", "--all-systems", "."],
        ctx.work,
    ) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };
    let show_json = match run(
        "nix",
        &["flake", "show", "--json", "--legacy", "--all-systems", "."],
        ctx.work,
    ) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };
    let fsn_text = match run(ctx.bin, &["-f", Format::Text.as_flag()], ctx.work) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };
    let fsn_json = match run_primary(ctx) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };

    let show_text_exit = exit_code(&show_text);
    if show_text_exit != 0 {
        return vec![format!(
            "live `nix flake show` exited {show_text_exit}; cannot compare"
        )];
    }
    let fsn_text_exit = exit_code(&fsn_text);
    if fsn_text_exit != 0 {
        return vec![format!("fsn -f text exited {fsn_text_exit}, expected 0")];
    }

    let mut problems = Vec::new();
    let show_first = first_line(&show_text.stdout);
    let fsn_first = first_line(&fsn_text.stdout);
    if show_first.trim().is_empty() {
        problems.push("live show stdout is empty".to_owned());
    }
    if fsn_first != show_first {
        problems.push(format!(
            "root line mismatch: fsn={fsn_first:?}, show={show_first:?}"
        ));
    }
    let plain = strip_ansi(&fsn_first);
    if !plain.starts_with("git+file://") {
        problems.push(format!("fsn root line is not git+file://: {plain:?}"));
    }

    let show_json_exit = exit_code(&show_json);
    if show_json_exit != 0 {
        problems.push(format!(
            "live `nix flake show --json` exited {show_json_exit}"
        ));
        return problems;
    }
    let show: Value = match serde_json::from_slice(&show_json.stdout) {
        Ok(value) => value,
        Err(err) => {
            problems.push(format!("live show JSON is invalid: {err}"));
            return problems;
        }
    };
    let hello = dig(&show, &["packages", "x86_64-linux", "hello"]);
    if hello
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        != Some("derivation")
    {
        problems.push(format!(
            "live show JSON is not a valid oracle: packages.x86_64-linux.hello={hello:?}, \
             want type 'derivation'"
        ));
    }
    let empty_top = dig(&show, &["emptyTop"]);
    if empty_top
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        != Some("unknown")
    {
        problems.push(format!(
            "live show JSON is not a valid oracle: emptyTop={empty_top:?}, want type 'unknown'"
        ));
    }

    let fsn_exit = exit_code(&fsn_json);
    match serde_json::from_slice::<FsnDocument>(&fsn_json.stdout) {
        Ok(document) => {
            problems.extend(compare_against_show(
                fsn_exit,
                &document.tree,
                &show,
                "live show output",
            ));
            problems.extend(assert_nodes(&document.tree, ctx.scenario.nodes));
        }
        Err(err) => problems.push(format!("fsn -f json stdout is not valid JSON: {err}")),
    }
    problems
}

// ---- recorded-byte oracles (Phase 3 part coverage) ---------------------------

/// Read a recorded golden from the fixture directory.
fn read_golden_bytes(ctx: &ScenarioContext<'_>, name: &str) -> Result<Vec<u8>, String> {
    let path = ctx.fixture.join(name);
    fs::read(&path).map_err(|err| format!("cannot read golden {}: {err}", path.display()))
}

/// All bytes after the first `\n`: the text root line embeds the temp path and
/// a content hash, so only the body is recordable.
fn body_after_first_line(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|byte| *byte == b'\n') {
        Some(index) => &bytes[index + 1..],
        None => &[],
    }
}

/// A compact report of the first differing byte, for a byte-exact comparison.
fn describe_bytes(label: &str, want: &[u8], got: &[u8]) -> String {
    let index = want
        .iter()
        .zip(got)
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| want.len().min(got.len()));
    let window = |bytes: &[u8]| {
        let start = index.saturating_sub(20);
        let end = (index + 20).min(bytes.len());
        String::from_utf8_lossy(&bytes[start..end]).into_owned()
    };
    format!(
        "{label} mismatch at byte {index}: want {:?}, got {:?} (want {} bytes, got {} bytes)",
        window(want),
        window(got),
        want.len(),
        got.len()
    )
}

/// Text part selection: exact stdout body + exact stderr, then a JSON run with
/// the same parts carries the semantic `nodes` assertions.
fn evaluate_text_part(
    ctx: &ScenarioContext<'_>,
    stdout_golden: &str,
    stderr_golden: &str,
) -> Vec<String> {
    let out = match run_primary(ctx) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };
    let exit = exit_code(&out);
    if exit != 0 {
        return vec![format!("fsn exited {exit}, expected 0")];
    }
    let mut problems = Vec::new();
    if !strip_ansi(&first_line(&out.stdout)).starts_with("path:") {
        problems.push(format!(
            "text root line is not a path ref: {:?}",
            first_line(&out.stdout)
        ));
    }
    match read_golden_bytes(ctx, stdout_golden) {
        Ok(want) => {
            let want_body = body_after_first_line(&want);
            let got_body = body_after_first_line(&out.stdout);
            if want_body != got_body {
                problems.push(describe_bytes("text stdout body", want_body, got_body));
            }
        }
        Err(err) => problems.push(err),
    }
    match read_golden_bytes(ctx, stderr_golden) {
        Ok(want) => {
            let got_stderr = strip_nix_env_warnings(&out.stderr);
            if got_stderr != want {
                problems.push(describe_bytes("stderr", &want, &got_stderr));
            }
        }
        Err(err) => problems.push(err),
    }

    let json_args = args_with_format(ctx.scenario, Format::Json);
    match run(ctx.bin, &json_args, ctx.work) {
        Ok(json_out) if exit_code(&json_out) == 0 => {
            match serde_json::from_slice::<FsnDocument>(&json_out.stdout) {
                Ok(document) => problems.extend(assert_nodes(&document.tree, ctx.scenario.nodes)),
                Err(err) => problems.push(format!("fsn -f json stdout is not valid JSON: {err}")),
            }
        }
        Ok(json_out) => problems.push(format!(
            "fsn -f json exited {}, expected 0",
            exit_code(&json_out)
        )),
        Err(err) => problems.push(err),
    }
    problems
}

/// A failing `-p` run pinned by full stderr bytes, exit code and empty stdout.
fn evaluate_stderr_golden(ctx: &ScenarioContext<'_>, want_exit: i32, golden: &str) -> Vec<String> {
    let out = match run_primary(ctx) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };
    let mut problems = Vec::new();
    let exit = exit_code(&out);
    if exit != want_exit {
        problems.push(format!("fsn exited {exit}, expected {want_exit}"));
    }
    if !out.stdout.is_empty() {
        problems.push(format!(
            "fsn wrote {} byte(s) to stdout, expected empty",
            out.stdout.len()
        ));
    }
    match read_golden_bytes(ctx, golden) {
        Ok(want) => {
            let got_stderr = strip_nix_env_warnings(&out.stderr);
            if got_stderr != want {
                problems.push(describe_bytes("stderr", &want, &got_stderr));
            }
        }
        Err(err) => problems.push(err),
    }
    problems
}

/// Run every scenario in a throwaway copy (fsn only acts on cwd), print the
/// `PASS <name>` / `FAIL <name>: <reason>` lines and the summary, keep the
/// temp dir on failure for inspection, and fail the test if anything failed.
pub fn run_all(tests: &Path, pre: &Preflight) {
    let temp = create_temp_dir();
    let mut results: Vec<(&'static str, Vec<String>)> = Vec::new();
    for scenario in SCENARIOS {
        let work = temp.join("work").join(scenario.name);
        let fixture = tests.join(scenario.fixture);
        let ctx = ScenarioContext {
            scenario,
            work: &work,
            fixture: &fixture,
            bin: &pre.bin,
            manifest: &pre.manifest,
        };
        let problems = match copy_dir_all(&fixture, &work) {
            Ok(()) => evaluate(&ctx),
            Err(err) => vec![format!("cannot copy scenario to temp dir: {err}")],
        };
        results.push((scenario.name, problems));
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    for (name, problems) in &results {
        if problems.is_empty() {
            passed += 1;
            println!("PASS {name}");
        } else {
            failed += 1;
            println!("FAIL {name}: {}", problems.join("; "));
        }
    }
    if failed == 0 {
        let _ = fs::remove_dir_all(&temp);
    } else {
        println!("temp dir kept for inspection: {}", temp.display());
    }
    println!(
        "\n{} scenarios: {} passed, {} failed",
        passed + failed,
        passed,
        failed
    );
    assert_eq!(failed, 0, "integration scenarios failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Evidence that the preflight comparison is a typed ordering: the exact
    /// predicate the preflight uses rejects an older version while the recorded
    /// baseline and anything newer pass (and the skip hatch bypasses it).
    #[test]
    fn nix_version_preflight_uses_typed_ordering() {
        let required = required_nix_version();
        assert_eq!(required.to_string(), "2.34.8");

        let older = Version::parse("nix (Nix) 2.34.7").expect("parse older");
        assert!(older < required, "2.34.7 must order below the baseline");
        assert!(
            !version_ok(Some(older), required, false),
            "an older runtime must be rejected"
        );

        let baseline = Version::parse("nix (Nix) 2.34.8").expect("parse baseline");
        assert!(version_ok(Some(baseline), required, false));

        let newer = Version::parse("nix (Nix) 2.35.0").expect("parse newer");
        assert!(
            version_ok(Some(newer), required, false),
            "a newer runtime must be accepted"
        );

        assert!(
            version_ok(Some(older), required, true),
            "FSN_IT_SKIP_NIX_VERSION=1 bypasses the check"
        );
        assert!(
            !version_ok(None, required, false),
            "an unparseable runtime is rejected"
        );
    }
}
