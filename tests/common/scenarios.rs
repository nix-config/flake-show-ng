//! The scenario set for the `fsn` integration suite, entirely as data.
//!
//! This is a module of the `common` harness, not a top-level `tests/*.rs`
//! target, so Cargo does not build it as an extra integration binary.  The one
//! driver (`common::run_all`) copies each row's fixture directory
//! (`tests/<fixture>`), runs the setup hook, invokes `fsn` with the typed argv
//! and applies the oracle named by [`Check`]; the eight former per-scenario
//! `test.rs` programs were byte-identical templates and are now rows below.
//!
//! Several rows may share one fixture directory (`part-unknown-candidates` and
//! `part-unknown-below-leaf` both use `tests/part-unknown`), which is why
//! `fixture` is explicit and distinct from the row `name`.

use flake_show_ng::model::Kind;

use super::NodeExpectation;

/// The typed `-f/--format` argv.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
}

impl Format {
    pub fn as_flag(self) -> &'static str {
        match self {
            Format::Text => "text",
            Format::Json => "json",
        }
    }
}

/// How a scenario decides pass/fail, i.e. where its expected output comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Compare `fsn -f json` against the recorded `show.json` golden; the
    /// recorded `manifest.exits.json` must be 0.
    Golden,
    /// `fsn -f json` must succeed and parse; upstream show aborts on the
    /// fixture, so the recorded `manifest.exits.text` carries the expectation.
    JsonOnly { manifest_text_exit: i64 },
    /// `fsn` must fail with a non-empty, IFD-mentioning stderr (IFD is disabled
    /// by default); upstream show omits IFD, so `manifest.exits.text` is 0.
    IfdDenied { manifest_text_exit: i64 },
    /// The oracle is captured live with `nix flake show` in the same (dirty
    /// git) worktree; the root line is compared byte for byte.
    LiveShow,
    /// Real part selection in text mode: `fsn` must exit 0, its stderr must
    /// equal the `stderr` golden byte for byte (pinning the swallow notice text
    /// and order), and its stdout body (all lines after the root line, whose
    /// temp path is not recordable) must equal the `stdout` golden.  A second
    /// run with the same parts in JSON mode carries the `nodes` assertions.
    TextPart {
        stdout: &'static str,
        stderr: &'static str,
    },
    /// A failing `-p` run: exit code `exit`, empty stdout, and stderr bytes
    /// equal to the `golden` file.  Pins both `report_unknown_parts` branches
    /// (sibling candidates and the below-leaf fallback).
    StderrGolden { exit: i32, golden: &'static str },
}

/// A command run in the workdir before the primary invocation, as its argv.
pub type SetupCommand = &'static [&'static str];

/// One integration scenario, entirely as data.
pub struct Scenario {
    /// Scenario name: unique per row, used by the run report and temp workdir.
    pub name: &'static str,
    /// Fixture directory (`tests/<fixture>`); several rows may share one.
    pub fixture: &'static str,
    /// Typed `-f/--format` argv for the primary invocation.
    pub format: Format,
    /// `-p/--part` values, each passed as its own argv element (clap also
    /// accepts the comma-separated form inside a single value).
    pub parts: &'static [&'static str],
    /// Commands (run as `git <argv>`) in the workdir before the invocation.
    pub setup: &'static [SetupCommand],
    /// Gate 4 M1 positive assertions on the parsed fsn tree.
    pub nodes: &'static [NodeExpectation],
    /// The oracle that decides pass/fail, plus its recorded exit expectation.
    pub check: Check,
}

/// The common row shape: fixture dir == row name, JSON argv, no parts or setup.
const fn scenario(name: &'static str, nodes: &'static [NodeExpectation], check: Check) -> Scenario {
    Scenario {
        name,
        fixture: name,
        format: Format::Json,
        parts: &[],
        setup: &[],
        nodes,
        check,
    }
}

/// A row with an explicit fixture / format / parts, for the Phase 3 gap rows.
const fn part_scenario(
    name: &'static str,
    fixture: &'static str,
    format: Format,
    parts: &'static [&'static str],
    nodes: &'static [NodeExpectation],
    check: Check,
) -> Scenario {
    Scenario {
        name,
        fixture,
        format,
        parts,
        setup: &[],
        nodes,
        check,
    }
}

/// The `git-dirty-root-line` row: the only scenario with a setup hook and a
/// live (non-recorded) oracle.
const fn git_scenario(
    name: &'static str,
    nodes: &'static [NodeExpectation],
    setup: &'static [SetupCommand],
) -> Scenario {
    Scenario {
        name,
        fixture: name,
        format: Format::Json,
        parts: &[],
        setup,
        nodes,
        check: Check::LiveShow,
    }
}

/// `apps`/`defaultApp` leaves with and without `meta.description`.
const APP_LEAVES: &[NodeExpectation] = &[
    (
        "apps.x86_64-linux.withDesc",
        Kind::App,
        &[("description", Some("an app with a description"))],
    ),
    (
        "apps.x86_64-linux.noDesc",
        Kind::App,
        &[("description", None)],
    ),
    (
        "defaultApp.x86_64-linux",
        Kind::App,
        &[("description", Some("the default app"))],
    ),
];

/// Empty sets, `type:"unknown"` top-level attrsets and leaves deeper than show's
/// white-list; `legacyPackages.x86_64-linux.nested.set.a` is an `int`.
const BASELINE_TREE: &[NodeExpectation] = &[
    ("emptyTop", Kind::Attrset, &[]),
    ("apps.x86_64-linux", Kind::Attrset, &[]),
    ("legacyPackages.x86_64-linux.nested.set.a", Kind::Int, &[]),
    (
        "packages.x86_64-linux.hello",
        Kind::Derivation,
        &[("drvName", Some("hello")), ("label", Some("package"))],
    ),
];

/// `nixosConfigurations.host1` aligns with show; `homeConfigurations.user1` is
/// an fsn extension namespace show only reports as `{"type":"unknown"}`.
const CONFIG_LEAVES: &[NodeExpectation] = &[
    (
        "homeConfigurations.user1",
        Kind::Config,
        &[("configType", Some("home-manager"))],
    ),
    (
        "nixosConfigurations.host1",
        Kind::Config,
        &[("configType", Some("nixos"))],
    ),
];

/// The error leaf and its healthy sibling derivation.
const EVAL_ERRORS: &[NodeExpectation] = &[
    (
        "packages.x86_64-linux.fine",
        Kind::Derivation,
        &[("drvName", Some("fine")), ("label", Some("package"))],
    ),
    ("packages.x86_64-linux.broken", Kind::Error, &[]),
];

/// The live fsn tree must still carry the nodes the live show oracle checks.
const GIT_DIRTY_ROOT_LINE: &[NodeExpectation] = &[
    (
        "packages.x86_64-linux.hello",
        Kind::Derivation,
        &[("drvName", Some("hello")), ("label", Some("package"))],
    ),
    ("emptyTop", Kind::Attrset, &[]),
];

/// Attrsets show warns about and omits; fsn expands them and renders the
/// string children.
const NON_DERIVATION_LEAVES: &[NodeExpectation] = &[
    ("packages.x86_64-linux.notdrv", Kind::Attrset, &[]),
    ("packages.x86_64-linux.notdrv.name", Kind::String, &[]),
    ("checks.x86_64-linux.mkCheck.name", Kind::String, &[]),
    ("devShells.x86_64-linux.shell.name", Kind::String, &[]),
];

/// Both template leaves, with an explicit `null` on the upstream-blind one.
const TEMPLATE_LEAVES: &[NodeExpectation] = &[
    (
        "templates.withDesc",
        Kind::Template,
        &[("description", Some("a template with a description"))],
    ),
    ("templates.noDesc", Kind::Template, &[("description", None)]),
];

/// Part selection must still render the selected subtree's semantic leaf.
const PART_SELECTION: &[NodeExpectation] = &[(
    "packages.x86_64-linux.hello",
    Kind::Derivation,
    &[("drvName", Some("hello")), ("label", Some("package"))],
)];

/// Real walker output for the non-`packages` namespace classification table
/// (Gate 1 M6 / Gate 2 F5): `devShell`/`devShells` are development
/// environments, `formatter`/`defaultPackage`/`legacyPackages` are packages,
/// and `checks`/`hydraJobs` keep the plain (absent) label at any depth.
const WALKER_NAMESPACES: &[NodeExpectation] = &[
    (
        "packages.x86_64-linux.pkg",
        Kind::Derivation,
        &[("drvName", Some("pkg")), ("label", Some("package"))],
    ),
    (
        "legacyPackages.x86_64-linux.legacy",
        Kind::Derivation,
        &[("drvName", Some("legacy")), ("label", Some("package"))],
    ),
    (
        "defaultPackage.x86_64-linux",
        Kind::Derivation,
        &[("drvName", Some("dp")), ("label", Some("package"))],
    ),
    (
        "formatter.x86_64-linux",
        Kind::Derivation,
        &[("drvName", Some("fmt")), ("label", Some("package"))],
    ),
    (
        "devShell.x86_64-linux",
        Kind::Derivation,
        &[
            ("drvName", Some("shell-sing")),
            ("label", Some("development-environment")),
        ],
    ),
    (
        "devShells.x86_64-linux.shell",
        Kind::Derivation,
        &[
            ("drvName", Some("shell")),
            ("label", Some("development-environment")),
        ],
    ),
    (
        "checks.x86_64-linux.deep.leaf",
        Kind::Derivation,
        &[("drvName", Some("chk")), ("label", None)],
    ),
    (
        "hydraJobs.x86_64-linux.job",
        Kind::Derivation,
        &[("drvName", Some("job")), ("label", None)],
    ),
    (
        "defaultTemplate",
        Kind::Template,
        &[("description", Some("default template"))],
    ),
    (
        "templates.tmpl",
        Kind::Template,
        &[("description", Some("a template"))],
    ),
    (
        "nixosConfigurations.host1",
        Kind::Config,
        &[("configType", Some("nixos"))],
    ),
    (
        "homeConfigurations.user1",
        Kind::Config,
        &[("configType", Some("home-manager"))],
    ),
    (
        "darwinConfigurations.mac1",
        Kind::Config,
        &[("configType", Some("darwin"))],
    ),
];

/// `git-dirty-root-line`: the fixture copy becomes a dirty worktree so the root
/// line is `git+file://<tmp>` (random path, not recordable).
const GIT_DIRTY_SETUP: &[SetupCommand] = &[
    &["init", "-q"],
    &[
        "-c",
        "user.email=t@t",
        "-c",
        "user.name=t",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "init",
    ],
    &["add", "-N", "flake.nix"],
];

/// Complete scenario set in run order.
pub const SCENARIOS: &[Scenario] = &[
    scenario("app-leaves", APP_LEAVES, Check::Golden),
    scenario("baseline-tree", BASELINE_TREE, Check::Golden),
    scenario("config-leaves", CONFIG_LEAVES, Check::Golden),
    scenario(
        "eval-errors",
        EVAL_ERRORS,
        Check::JsonOnly {
            manifest_text_exit: 1,
        },
    ),
    git_scenario("git-dirty-root-line", GIT_DIRTY_ROOT_LINE, GIT_DIRTY_SETUP),
    scenario(
        "ifd-denied",
        &[],
        Check::IfdDenied {
            manifest_text_exit: 0,
        },
    ),
    scenario(
        "non-derivation-leaves",
        NON_DERIVATION_LEAVES,
        Check::Golden,
    ),
    scenario(
        "template-leaves",
        TEMPLATE_LEAVES,
        Check::JsonOnly {
            manifest_text_exit: 1,
        },
    ),
    part_scenario(
        "part-selection",
        "part-selection",
        Format::Text,
        &["packages.x86_64-linux", "packages", "packages.x86_64-linux"],
        PART_SELECTION,
        Check::TextPart {
            stdout: "fsn.txt",
            stderr: "fsn.stderr",
        },
    ),
    part_scenario(
        "part-unknown-candidates",
        "part-unknown",
        Format::Json,
        &["nope.x"],
        &[],
        Check::StderrGolden {
            exit: 1,
            golden: "candidates.stderr",
        },
    ),
    part_scenario(
        "part-unknown-below-leaf",
        "part-unknown",
        Format::Json,
        &["packages.x86_64-linux.hello.name"],
        &[],
        Check::StderrGolden {
            exit: 1,
            golden: "below-leaf.stderr",
        },
    ),
    scenario("walker-namespaces", WALKER_NAMESPACES, Check::Golden),
];
