//! The scenario set for the `fsn` integration suite, entirely as data.
//!
//! This is a module of the `common` harness, not a top-level `tests/*.rs`
//! target, so Cargo does not build it as an extra integration binary.  The one
//! driver (`common::run_all`) copies each row's fixture directory
//! (`tests/<fixture>`), runs the setup hook, invokes `fsn` with the typed argv
//! and applies the oracle named by [`Check`]; the former per-scenario
//! `test.rs` programs were byte-identical templates and are now rows below.
//!
//! Several rows may share one fixture directory (`part-unknown-candidates` and
//! `part-unknown-below-leaf` both use `tests/part-unknown`), which is why
//! `fixture` is explicit and distinct from the row `name`.

use flake_show_ng::model::Kind;

/// How a scenario decides pass/fail, i.e. where its expected output comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Run `fsn` with only its `-p` args, require exit 0, require the
    /// ANSI-stripped first stdout line to start with `path:` and contain
    /// `narHash=`, and compare every byte after the first `\n` against the
    /// `stdout` golden.  The root line embeds the temp path, so only the body
    /// is recordable.  `walker` re-derives the walker output and pins the kind
    /// of each named node, covering bare leaves the text golden cannot type.
    TextGolden {
        stdout: &'static str,
        walker: &'static [(&'static str, Kind)],
    },
    /// [`Check::TextGolden`] plus stderr compared byte-for-byte after
    /// [`strip_nix_env_warnings`](super::strip_nix_env_warnings), pinning the
    /// swallow notice text and order (for `part-selection`).
    TextGoldenStderr {
        stdout: &'static str,
        stderr: &'static str,
    },
    /// Independent end-to-end oracle: the fsn body is compared against a
    /// separately recorded `nix flake show` capture (`show`), ANSI-stripped
    /// byte-for-byte (for `app-leaves`).
    TextMatchesShow { show: &'static str },
    /// `fsn` must fail with a non-empty, IFD-mentioning stderr (IFD is disabled
    /// by default); the recorded `manifest.exits.text` carries the upstream
    /// show expectation.
    IfdDenied { manifest_text_exit: i64 },
    /// The root line is captured live with `nix flake show --legacy
    /// --all-systems` in the same (dirty git) worktree and compared byte for
    /// byte; the body is compared against the `stdout` golden (first line
    /// skipped, since it embeds the random temp path).
    LiveShow { stdout: &'static str },
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
    /// `-p/--part` values, each passed as its own argv element (clap also
    /// accepts the comma-separated form inside a single value).
    pub parts: &'static [&'static str],
    /// Commands (run as `git <argv>`) in the workdir before the invocation.
    pub setup: &'static [SetupCommand],
    /// The oracle that decides pass/fail.
    pub check: Check,
}

/// The common row shape: fixture dir == row name, no parts or setup.
const fn scenario(name: &'static str, check: Check) -> Scenario {
    Scenario {
        name,
        fixture: name,
        parts: &[],
        setup: &[],
        check,
    }
}

/// A row with an explicit fixture and `-p` args, for the rows that share a
/// fixture directory or select parts.
const fn part_scenario(
    name: &'static str,
    fixture: &'static str,
    parts: &'static [&'static str],
    check: Check,
) -> Scenario {
    Scenario {
        name,
        fixture,
        parts,
        setup: &[],
        check,
    }
}

/// The `git-dirty-root-line` row: the only scenario with a setup hook and a
/// live (non-recorded) oracle.
const fn git_scenario(name: &'static str, setup: &'static [SetupCommand]) -> Scenario {
    Scenario {
        name,
        fixture: name,
        parts: &[],
        setup,
        check: Check::LiveShow { stdout: "fsn.txt" },
    }
}

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
    scenario("app-leaves", Check::TextMatchesShow { show: "show.txt" }),
    scenario(
        "baseline-tree",
        Check::TextGolden {
            stdout: "fsn.txt",
            walker: &[
                ("legacyPackages.x86_64-linux.nested.set.a", Kind::Int),
                ("packages.x86_64-linux.hello", Kind::Derivation),
            ],
        },
    ),
    scenario(
        "config-leaves",
        Check::TextGolden {
            stdout: "fsn.txt",
            walker: &[],
        },
    ),
    scenario(
        "eval-errors",
        Check::TextGolden {
            stdout: "fsn.txt",
            walker: &[],
        },
    ),
    git_scenario("git-dirty-root-line", GIT_DIRTY_SETUP),
    scenario(
        "ifd-denied",
        Check::IfdDenied {
            manifest_text_exit: 0,
        },
    ),
    scenario(
        "non-derivation-leaves",
        Check::TextGolden {
            stdout: "fsn.txt",
            walker: &[
                ("packages.x86_64-linux.notdrv", Kind::Attrset),
                ("packages.x86_64-linux.notdrv.name", Kind::String),
                ("checks.x86_64-linux.mkCheck.name", Kind::String),
                ("devShells.x86_64-linux.shell.name", Kind::String),
            ],
        },
    ),
    scenario(
        "template-leaves",
        Check::TextGolden {
            stdout: "fsn.txt",
            walker: &[],
        },
    ),
    part_scenario(
        "part-selection",
        "part-selection",
        &["packages.x86_64-linux", "packages", "packages.x86_64-linux"],
        Check::TextGoldenStderr {
            stdout: "fsn.txt",
            stderr: "fsn.stderr",
        },
    ),
    part_scenario(
        "part-unknown-candidates",
        "part-unknown",
        &["nope.x"],
        Check::StderrGolden {
            exit: 1,
            golden: "candidates.stderr",
        },
    ),
    part_scenario(
        "part-unknown-below-leaf",
        "part-unknown",
        &["packages.x86_64-linux.hello.name"],
        Check::StderrGolden {
            exit: 1,
            golden: "below-leaf.stderr",
        },
    ),
    scenario(
        "walker-namespaces",
        Check::TextGolden {
            stdout: "fsn.txt",
            walker: &[],
        },
    ),
];
