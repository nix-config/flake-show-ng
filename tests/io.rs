//! Every file in `sim/<name>/` is copied into a throwaway flake directory as
//! sample input, except the reserved `case.json`, `stdout`, `stderr`, and
//! `show`.  `case.json` carries the invocation parameters and non-file
//! expectations; every key is optional and unknown keys are rejected.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use flake_show_ng::model::{Kind, Node};
use serde::Deserialize;

const TESTED_BIN: &str = env!("CARGO_BIN_EXE_fsn");

const RESERVED: &[&str] = &["case.json", "stdout", "stderr", "show"];

struct Sample {
    name: String,
    dir: PathBuf,
}

fn sim_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("sim")
}

fn discover(sim: &Path) -> Result<Vec<Sample>, String> {
    let entries = fs::read_dir(sim)
        .map_err(|err| format!("cannot read sample root {}: {err}", sim.display()))?;
    let mut samples = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("cannot read {}: {err}", sim.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("cannot stat {}: {err}", entry.path().display()))?;
        if !file_type.is_dir() {
            continue;
        }
        samples.push(Sample {
            name: entry.file_name().to_string_lossy().into_owned(),
            dir: entry.path(),
        });
    }
    samples.sort_by(|a, b| a.name.cmp(&b.name));
    if samples.is_empty() {
        return Err(format!("no sample directories under {}", sim.display()));
    }
    Ok(samples)
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Case {
    args: Vec<String>,
    exit: i32,
    kinds: BTreeMap<String, Kind>,
    stderr_contains: Vec<String>,
    setup: Vec<String>,
    git_ref: bool,
}

fn read_case(sample: &Sample) -> Result<Case, String> {
    let path = sample.dir.join("case.json");
    let bytes =
        fs::read(&path).map_err(|err| format!("{}: cannot read case.json: {err}", sample.name))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("{}: invalid case.json: {err}", sample.name))
}

fn read_output(sample: &Sample, name: &str) -> Result<Option<Vec<u8>>, String> {
    match fs::read(sample.dir.join(name)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("{}: cannot read {name}: {err}", sample.name)),
    }
}

struct Expected {
    case: Case,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    show: Option<Vec<u8>>,
}

fn read_expected(sample: &Sample) -> Result<Expected, String> {
    Ok(Expected {
        case: read_case(sample)?,
        stdout: read_output(sample, "stdout")?,
        stderr: read_output(sample, "stderr")?,
        show: read_output(sample, "show")?,
    })
}

fn copy_inputs(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        if RESERVED
            .iter()
            .any(|reserved| entry.file_name() == OsStr::new(reserved))
        {
            continue;
        }
        let target = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_inputs(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// No tempfile dependency: pick a unique directory under `env::temp_dir()`.
fn create_temp_dir() -> PathBuf {
    let base = env::temp_dir();
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for attempt in 0..1000u32 {
        let candidate = base.join(format!("fsn-sim-{pid}-{nanos}-{attempt}"));
        match fs::create_dir(&candidate) {
            Ok(()) => return candidate,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => continue,
        }
    }
    panic!("cannot create a unique temp dir under {}", base.display());
}

fn run(bin: &str, args: &[&str], cwd: &Path) -> Result<Output, String> {
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

fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The root line embeds the temp path and a content hash, so only the body is
/// recordable.
fn body_after_first_line(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|byte| *byte == b'\n') {
        Some(index) => &bytes[index + 1..],
        None => &[],
    }
}

/// `nix flake show` colors its output unconditionally when stdout is not a tty.
fn strip_ansi(input: &str) -> String {
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

/// Nix prints this warning whenever the host has no non-loopback interface
/// (e.g. the sandbox) and fsn forwards nix's stderr, so the wording of fsn's own
/// diagnostics must be compared without it.
const NIX_NO_INTERNET_WARNING: &[u8] =
    b"warning: you don't have Internet access; disabling some network-dependent features";

fn strip_nix_env_warnings(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for line in input.split_inclusive(|&b| b == b'\n') {
        let content = line.strip_suffix(b"\n").unwrap_or(line);
        if content != NIX_NO_INTERNET_WARNING {
            out.extend_from_slice(line);
        }
    }
    out
}

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

/// The text rendering does not spell out int-vs-string-vs-attrset, so `kinds`
/// re-runs the real walker and checks the typed result.
fn evaluate_kinds(work: &Path, wants: &BTreeMap<String, Kind>) -> Vec<String> {
    if wants.is_empty() {
        return Vec::new();
    }
    let locked_ref = match flake_show_ng::nix::flake_metadata(work) {
        Ok(reference) => reference,
        Err(err) => return vec![format!("kinds: nix flake metadata failed: {err}")],
    };
    let output = match flake_show_ng::nix::eval_tree(work, &locked_ref, &[], false) {
        Ok(output) => output,
        Err(err) => return vec![format!("kinds: nix eval failed: {err}")],
    };
    let mut by_path: BTreeMap<String, Kind> = BTreeMap::new();
    collect_kinds(&output.tree, &mut by_path);
    let mut problems = Vec::new();
    for (path, want) in wants {
        match by_path.get(path) {
            Some(got) if got == want => {}
            Some(got) => {
                problems.push(format!("kinds: {path} has kind {got:?}, expected {want:?}"))
            }
            None => problems.push(format!(
                "kinds: {path} is absent from the walker tree, expected {want:?}"
            )),
        }
    }
    problems
}

fn collect_kinds(nodes: &[Node], out: &mut BTreeMap<String, Kind>) {
    for node in nodes {
        out.insert(node.path.to_string(), node.kind);
        if let Some(children) = &node.children {
            collect_kinds(children, out);
        }
    }
}

fn evaluate(sample: &Sample, expected: &Expected, bin: &str, work: &Path) -> Vec<String> {
    if let Err(err) = copy_inputs(&sample.dir, work) {
        return vec![format!("cannot copy sample to temp dir: {err}")];
    }

    for command in &expected.case.setup {
        match Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(work)
            .output()
        {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                return vec![format!(
                    "setup command failed (`{command}`): exit {}",
                    exit_code(&out)
                )];
            }
            Err(err) => return vec![format!("cannot run setup command `{command}`: {err}")],
        }
    }

    // A dirty worktree root line contains the random temp path, so it cannot be
    // recorded; capture it from upstream in the same worktree instead.
    let live_show = if expected.case.git_ref {
        match run(
            "nix",
            &["flake", "show", "--legacy", "--all-systems", "."],
            work,
        ) {
            Ok(out) if out.status.success() => Some(first_line(&out.stdout)),
            Ok(out) => {
                return vec![format!(
                    "live `nix flake show` exited {}; cannot compare",
                    exit_code(&out)
                )];
            }
            Err(err) => return vec![err],
        }
    } else {
        None
    };

    let args: Vec<&str> = expected.case.args.iter().map(String::as_str).collect();
    let out = match run(bin, &args, work) {
        Ok(out) => out,
        Err(err) => return vec![err],
    };

    let mut problems = Vec::new();
    let got_exit = exit_code(&out);
    if got_exit != expected.case.exit {
        problems.push(format!(
            "fsn exited {got_exit}, expected {}",
            expected.case.exit
        ));
    }

    if let Some(show_first) = live_show {
        let fsn_first = first_line(&out.stdout);
        if fsn_first != show_first {
            problems.push(format!(
                "root line mismatch: fsn={fsn_first:?}, show={show_first:?}"
            ));
        }
        let plain = strip_ansi(&fsn_first);
        if !plain.starts_with("git+file://") {
            problems.push(format!("fsn root line is not git+file://: {plain:?}"));
        }
    } else if (expected.stdout.is_some() || expected.show.is_some()) && got_exit == 0 {
        let first = strip_ansi(&first_line(&out.stdout));
        if !first.starts_with("path:") || !first.contains("narHash=") {
            problems.push(format!(
                "root line is not a path ref with narHash=: {:?}",
                first_line(&out.stdout)
            ));
        }
    }

    if let Some(want) = &expected.stdout {
        let want_body = body_after_first_line(want);
        let got_body = body_after_first_line(&out.stdout);
        if want_body != got_body {
            problems.push(describe_bytes("stdout body", want_body, got_body));
        }
    }

    if let Some(want) = &expected.show {
        let want_body = strip_ansi(&String::from_utf8_lossy(body_after_first_line(want)));
        let got_body = strip_ansi(&String::from_utf8_lossy(body_after_first_line(&out.stdout)));
        if want_body != got_body {
            problems.push(describe_bytes(
                "show body",
                want_body.as_bytes(),
                got_body.as_bytes(),
            ));
        }
    }

    if let Some(want) = &expected.stderr {
        let got = strip_nix_env_warnings(&out.stderr);
        if &got != want {
            problems.push(describe_bytes("stderr", want, &got));
        }
        if expected.stdout.is_none() && expected.show.is_none() && !out.stdout.is_empty() {
            problems.push(format!(
                "fsn wrote {} byte(s) to stdout, expected empty",
                out.stdout.len()
            ));
        }
    } else if expected.case.exit != 0 && out.stderr.iter().all(u8::is_ascii_whitespace) {
        problems.push("fsn failed but wrote no diagnostics to stderr".to_owned());
    }

    if !expected.case.stderr_contains.is_empty() {
        let text = String::from_utf8_lossy(&out.stderr).to_lowercase();
        let hit = expected
            .case
            .stderr_contains
            .iter()
            .any(|needle| text.contains(&needle.to_lowercase()));
        if !hit {
            problems.push(format!(
                "stderr does not mention any of {:?}",
                expected.case.stderr_contains
            ));
        }
    }

    problems.extend(evaluate_kinds(work, &expected.case.kinds));
    problems
}

fn preflight(samples: &[Sample]) -> Vec<String> {
    let mut problems = Vec::new();
    if let Err(err) = Command::new("nix").arg("--version").output() {
        problems.push(format!("sim samples require 'nix' on PATH: {err}"));
    }
    let mut needs_git = false;
    for sample in samples {
        if !sample.dir.join("flake.nix").is_file() {
            problems.push(format!("sample {} has no flake.nix", sample.name));
        }
        match read_case(sample) {
            Ok(case) => needs_git |= !case.setup.is_empty() || case.git_ref,
            Err(err) => problems.push(err),
        }
    }
    // Only git-based samples need git; keep it out of the general
    // prerequisite so the suite still runs on hosts without git.
    if needs_git && let Err(err) = Command::new("git").arg("--version").output() {
        problems.push(format!("'git' is required by a sim sample: {err}"));
    }
    if problems.is_empty()
        && let Err(err) = Command::new(TESTED_BIN).arg("--version").output()
    {
        problems.push(format!("cannot run fsn binary `{TESTED_BIN}`: {err}"));
    }
    problems
}

#[test]
fn flake_flag_uses_the_given_directory() {
    if Command::new("nix").arg("--version").output().is_err() {
        return;
    }

    let sample_dir = sim_dir().join("baseline-tree");
    let sample = Sample {
        name: "baseline-tree".to_owned(),
        dir: sample_dir,
    };
    let expected = read_expected(&sample).unwrap();
    let want = expected.stdout.expect("baseline-tree has a stdout golden");

    let temp = create_temp_dir();
    let work = temp.join("flake");
    copy_inputs(&sample.dir, &work).unwrap();

    let out = Command::new(TESTED_BIN)
        .arg("-f")
        .arg(&work)
        .current_dir(&temp)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&temp);

    assert_eq!(
        exit_code(&out),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        body_after_first_line(&want),
        body_after_first_line(&out.stdout)
    );
    let first = strip_ansi(&first_line(&out.stdout));
    assert!(first.starts_with("path:") && first.contains("narHash="));
}

#[test]
fn sim_samples() {
    let sim = sim_dir();
    let samples = match discover(&sim) {
        Ok(samples) => samples,
        Err(err) => {
            println!("FAIL: {err}");
            panic!("cannot load the sim sample set");
        }
    };
    let problems = preflight(&samples);
    if !problems.is_empty() {
        for problem in &problems {
            println!("FAIL: {problem}");
        }
        panic!("sim prerequisites failed ({} problem(s))", problems.len());
    }

    let temp = create_temp_dir();
    let mut results: Vec<(&str, Vec<String>)> = Vec::new();
    for sample in &samples {
        let work = temp.join("work").join(&sample.name);
        let entry = match read_expected(sample) {
            Ok(expected) => evaluate(sample, &expected, TESTED_BIN, &work),
            Err(err) => vec![err],
        };
        results.push((&sample.name, entry));
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
        "\n{} samples: {} passed, {} failed",
        passed + failed,
        passed,
        failed
    );
    assert_eq!(failed, 0, "sim samples failed");
}
