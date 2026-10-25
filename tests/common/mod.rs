//! Shared engine for the `fsn` integration suite (see `tests/integration.rs`).
//!
//! `tests/integration.rs` owns only the env gate and the prerequisite call; the
//! scenario set is data in [`scenarios`], and the one driver below copies each
//! fixture directory into a throwaway temp dir (fsn only acts on its cwd), runs
//! the scenario's setup hook, invokes fsn with the typed argv and applies the
//! scenario's oracle.  All comparisons stay exact: the recorded `fsn.txt` /
//! `fsn.stderr` bytes are verified by sha256 before any run, and the
//! `git-dirty-root-line` root line is compared byte-for-byte against a live
//! `nix flake show`.

use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use flake_show_ng::model::{Kind, Node};
use serde::Deserialize;

use self::scenarios::{Check, SCENARIOS, Scenario};

pub mod scenarios;

/// Support directories that hold harness code, not scenarios (excluded from the
/// m1 scenario-set check).
pub const SUPPORT_DIRS: &[&str] = &["common", "render-text"];

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

/// Recorded upstream `nix flake show` text exit code.
#[derive(Debug, Default, Deserialize)]
pub struct Exits {
    #[serde(default)]
    pub text: Option<i64>,
}

fn manifest_text_exit(manifest: &Manifest, scenario: &str) -> Option<i64> {
    manifest.fixtures.get(scenario)?.exits.text
}

/// The recorded-exit assertion used only by [`evaluate_ifd`], which needs the
/// upstream `nix flake show` exit code as a second witness that the fixture
/// really exercises an IFD denial.
pub fn expect_manifest_text_exit(ctx: &ScenarioContext<'_>, want: i64) -> Vec<String> {
    match manifest_text_exit(ctx.manifest, ctx.scenario.fixture) {
        Some(got) if got == want => Vec::new(),
        Some(got) => vec![format!(
            "manifest exits.text is {got}, expected {want}; re-record golden data"
        )],
        None => vec![format!(
            "manifest exits.text is missing, expected {want}; re-record golden data"
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

/// The golden files a check reads from its fixture directory.  Used by the
/// preflight to prove every referenced golden is hash-pinned in the manifest,
/// so a new oracle cannot silently bypass the sha256 round-trip.
fn referenced_goldens(check: &Check) -> Vec<&'static str> {
    match check {
        Check::TextGolden { stdout, .. } => vec![stdout],
        Check::TextGoldenStderr { stdout, stderr } => vec![stdout, stderr],
        Check::TextMatchesShow { show } => vec![show],
        Check::LiveShow { stdout } => vec![stdout],
        Check::StderrGolden { golden, .. } => vec![golden],
        Check::IfdDenied { .. } => Vec::new(),
    }
}

/// Prerequisite phase for the integration root: nix/git availability, manifest
/// load, m1 scenario-set rules (dirs == [`SCENARIOS`] == manifest keys, per-dir
/// `flake.nix`), the golden/fixture sha256 round-trip, and the fsn binary
/// resolution + probe.  Hard failures are never skips: they are printed as
/// `FAIL: ...` lines and abort the test.
pub fn preflight(tests: &Path) -> Preflight {
    let mut hard_failures: Vec<String> = Vec::new();
    let skip_version = env::var("FSN_SKIP_NIX_VERSION").ok().as_deref() == Some("1");
    let required = required_nix_version();

    match Command::new("nix").arg("--version").output() {
        Err(err) => hard_failures.push(format!("integration tests require 'nix' on PATH: {err}")),
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let version = Version::parse(&text);
            // Typed ordering: a newer runtime is accepted, an older one is a
            // hard failure because the goldens are tied to the baseline.
            if !version_ok(version, required, skip_version) {
                hard_failures.push(match version {
                    Some(found) => format!(
                        "golden data is recorded with Nix {required} but found: {found} \
                         (set FSN_SKIP_NIX_VERSION=1 to compare anyway)"
                    ),
                    None => format!(
                        "cannot parse `nix --version` output {text:?} (need >= {required}; \
                         set FSN_SKIP_NIX_VERSION=1 to compare anyway)"
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
    if !skip_version
        && let Some(recorded) = Version::parse(&manifest.nix_version)
        && recorded != required
    {
        hard_failures.push(format!(
            "manifest nixVersion is {recorded}, expected the recorded requirement \
             {required}; re-record manifest.json"
        ));
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
    // Every golden a check reads must be a hash-pinned key of its fixture, or
    // the sha256 round-trip below would not cover it.
    for scenario in SCENARIOS {
        let Some(case) = manifest.fixtures.get(scenario.fixture) else {
            continue;
        };
        for golden in referenced_goldens(&scenario.check) {
            if !case.golden_files.contains_key(golden) {
                hard_failures.push(format!(
                    "{}/{}: golden referenced by {} is not hash-pinned in manifest.json",
                    scenario.fixture, golden, scenario.name
                ));
            }
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

/// The typed argv for a scenario's primary invocation: every `-p` value (each
/// its own argv element).  A row that selects no parts runs fsn bare.
fn primary_args(scenario: &Scenario) -> Vec<&'static str> {
    let mut args = Vec::new();
    for part in scenario.parts {
        args.push("-p");
        args.push(part);
    }
    args
}

fn run_primary(ctx: &ScenarioContext<'_>) -> Result<Output, String> {
    let args = primary_args(ctx.scenario);
    run(ctx.bin, &args, ctx.work)
}

/// Apply one scenario's [`Check`] oracle and return its problems.
pub fn evaluate(ctx: &ScenarioContext<'_>) -> Vec<String> {
    match ctx.scenario.check {
        Check::TextGolden { stdout, walker } => evaluate_text_golden(ctx, stdout, None, walker),
        Check::TextGoldenStderr { stdout, stderr } => {
            evaluate_text_golden(ctx, stdout, Some(stderr), &[])
        }
        Check::TextMatchesShow { show } => evaluate_text_matches_show(ctx, show),
        Check::IfdDenied { manifest_text_exit } => evaluate_ifd(ctx, manifest_text_exit),
        Check::LiveShow { stdout } => evaluate_live_show(ctx, stdout),
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
    let mut problems = expect_manifest_text_exit(ctx, manifest_text_exit);
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
/// live `nix flake show` (text) as the root-line oracle and the recorded
/// `stdout` golden as the body oracle.  Both the live and recorded root lines
/// embed a random temp path, so the root line is compared live while the body
/// is compared byte-for-byte against the golden.
fn evaluate_live_show(ctx: &ScenarioContext<'_>, stdout_golden: &str) -> Vec<String> {
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
    let fsn_text = match run_primary(ctx) {
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
        return vec![format!("fsn exited {fsn_text_exit}, expected 0")];
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
    match read_golden_bytes(ctx, stdout_golden) {
        Ok(want) => {
            let want_body = body_after_first_line(&want);
            let got_body = body_after_first_line(&fsn_text.stdout);
            if want_body != got_body {
                problems.push(describe_bytes("git-dirty text body", want_body, got_body));
            }
        }
        Err(err) => problems.push(err),
    }
    problems
}

// ---- recorded-byte oracles ---------------------------------------------------

/// Read a recorded golden from the fixture directory.
fn read_golden_bytes(ctx: &ScenarioContext<'_>, name: &str) -> Result<Vec<u8>, String> {
    let path = ctx.fixture.join(name);
    fs::read(&path).map_err(|err| format!("cannot read golden {}: {err}", path.display()))
}

/// All bytes after the first `\n`: the text root line embeds the temp path, so
/// only the body is recordable.
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

/// Text goldens: exit 0, root line starts with `path:` and contains
/// `narHash=`, body (all bytes after the first `\n`) equals the golden, and
/// optionally exact stderr after [`strip_nix_env_warnings`].  `walker` adds
/// typed walker-kind assertions when the text golden cannot type bare leaves.
fn evaluate_text_golden(
    ctx: &ScenarioContext<'_>,
    stdout_golden: &str,
    stderr_golden: Option<&str>,
    walker: &[(&str, Kind)],
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
    let first = strip_ansi(&first_line(&out.stdout));
    if !first.starts_with("path:") {
        problems.push(format!(
            "text root line is not a path ref: {:?}",
            first_line(&out.stdout)
        ));
    } else if !first.contains("narHash=") {
        problems.push(format!(
            "text root line lacks narHash=: {:?}",
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
    if let Some(stderr_golden) = stderr_golden {
        match read_golden_bytes(ctx, stderr_golden) {
            Ok(want) => {
                let got_stderr = strip_nix_env_warnings(&out.stderr);
                if got_stderr != want {
                    problems.push(describe_bytes("stderr", &want, &got_stderr));
                }
            }
            Err(err) => problems.push(err),
        }
    }
    problems.extend(evaluate_walker_kinds(ctx, walker));
    problems
}

/// Independent end-to-end oracle: the fsn text body must match a separately
/// recorded `nix flake show` capture, ANSI-stripped byte-for-byte.  This keeps a
/// refactor in the text renderer from being blessed by re-recording its own
/// golden.
fn evaluate_text_matches_show(ctx: &ScenarioContext<'_>, show_golden: &str) -> Vec<String> {
    let out = match run_primary(ctx) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };
    let exit = exit_code(&out);
    if exit != 0 {
        return vec![format!("fsn exited {exit}, expected 0")];
    }
    let mut problems = Vec::new();
    let first = strip_ansi(&first_line(&out.stdout));
    if !first.starts_with("path:") {
        problems.push(format!(
            "text root line is not a path ref: {:?}",
            first_line(&out.stdout)
        ));
    } else if !first.contains("narHash=") {
        problems.push(format!(
            "text root line lacks narHash=: {:?}",
            first_line(&out.stdout)
        ));
    }
    match read_golden_bytes(ctx, show_golden) {
        Ok(want) => {
            let want_body = strip_ansi(&String::from_utf8_lossy(body_after_first_line(&want)));
            let got_body = strip_ansi(&String::from_utf8_lossy(body_after_first_line(&out.stdout)));
            if want_body != got_body {
                problems.push(describe_bytes(
                    "show.txt-vs-fsn body",
                    want_body.as_bytes(),
                    got_body.as_bytes(),
                ));
            }
        }
        Err(err) => problems.push(err),
    }
    problems
}

/// Re-derive the walker output for the workdir and assert each named node's
/// `Kind`.  The walker JSON is fsn's internal input, so this pins leaves whose
/// kind (int/string/attrset) the text rendering does not spell out.
fn evaluate_walker_kinds(ctx: &ScenarioContext<'_>, wants: &[(&str, Kind)]) -> Vec<String> {
    if wants.is_empty() {
        return Vec::new();
    }
    let locked_ref = match flake_show_ng::nix::flake_metadata(ctx.work) {
        Ok(reference) => reference,
        Err(err) => return vec![format!("walker coverage: nix flake metadata failed: {err}")],
    };
    let output = match flake_show_ng::nix::eval_tree(ctx.work, &locked_ref, &[], false) {
        Ok(output) => output,
        Err(err) => return vec![format!("walker coverage: nix eval failed: {err}")],
    };
    let mut by_path: BTreeMap<String, Kind> = BTreeMap::new();
    collect_kinds(&output.tree, &mut by_path);
    let mut problems = Vec::new();
    for (path, want) in wants {
        match by_path.get(*path) {
            Some(got) if got == want => {}
            Some(got) => problems.push(format!(
                "walker coverage: {path} has kind {got:?}, expected {want:?}"
            )),
            None => problems.push(format!(
                "walker coverage: {path} is absent from the walker tree, expected {want:?}"
            )),
        }
    }
    problems
}

/// Flatten the node tree keyed by the node's full attrpath.
fn collect_kinds(nodes: &[Node], out: &mut BTreeMap<String, Kind>) {
    for node in nodes {
        out.insert(node.path.to_string(), node.kind);
        if let Some(children) = &node.children {
            collect_kinds(children, out);
        }
    }
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
            "FSN_SKIP_NIX_VERSION=1 bypasses the check"
        );
        assert!(
            !version_ok(None, required, false),
            "an unparseable runtime is rejected"
        );
    }
}
