use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::error::Error;
use crate::model::WalkerOutput;

/// The traversal strategy, embedded in the binary.
const WALKER_SRC: &str = include_str!("walker.nix");

/// Nix version this crate is recorded against (the rendering baseline).
pub const RECORDED_NIX_VERSION: &str = "2.34.8";

/// Nix option that gates import-from-derivation. Always passed explicitly, so
/// `--allow-ifd` can override a user nix.conf and the default cannot be
/// silently re-enabled by one.
pub const IFD_OPTION: &str = "allow-import-from-derivation";

/// Actionable hint printed when evaluation fails and IFD was not allowed.
pub const IFD_HINT: &str = "hint: evaluation requires import from derivation (IFD), which is disabled by default; retry with --allow-ifd (may trigger builds)";

/// Prefix of the error when the `nix` binary cannot be spawned.
pub const NIX_MISSING_MSG: &str =
    "cannot run nix (runtime dependency nix >= 2.34.8; make sure it is on PATH)";

fn exit_code(status: std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => code.to_string(),
        None => "signal".to_owned(),
    }
}

fn forward_stderr(stderr: &[u8]) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(stderr);
    let _ = err.flush();
}

fn run_nix(mut cmd: Command) -> Result<std::process::Output, Error> {
    cmd.output()
        .map_err(|e| Error::msg(format!("{NIX_MISSING_MSG}: {e}")))
}

/// Heuristic for "IFD is disabled" failures.
///
/// It matches the literal option name `allow-import-from-derivation` in nix's
/// stderr, which is what Nix 2.34.8 prints
/// (`error: cannot build '...' during evaluation because the option
/// 'allow-import-from-derivation' is disabled`). This is a string contract with
/// the upstream error message: if upstream rewords it, this detection (and the
/// integration test assertion on the hint) must be updated. It is only consulted
/// when the eval failed and IFD was not explicitly allowed; the exit code alone
/// already drives the error path.
fn stderr_mentions_ifd(stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr).contains(IFD_OPTION)
}

/// Build the argv for `nix eval` (kept separate so it is unit-testable without nix).
fn eval_args(expr: &str, allow_ifd: bool) -> Vec<String> {
    vec![
        "eval".to_owned(),
        "--impure".to_owned(),
        "--no-write-lock-file".to_owned(),
        "--json".to_owned(),
        "--option".to_owned(),
        IFD_OPTION.to_owned(),
        if allow_ifd { "true" } else { "false" }.to_owned(),
        "--expr".to_owned(),
        expr.to_owned(),
    ]
}

/// Parse the locked ref from `nix flake metadata --json` output.
///
/// The `.url` field is byte-identical to the root line of `nix flake show`.
/// Kept pure so it is unit-testable without nix.
fn locked_ref_from_metadata(metadata: &serde_json::Value) -> Result<String, Error> {
    metadata
        .get("url")
        .and_then(|u| u.as_str())
        .map(str::to_owned)
        .ok_or_else(|| Error::msg("nix flake metadata --json output lacks .url"))
}

/// `nix flake metadata --json --no-write-lock-file`, returning `.url`.
pub fn flake_metadata(cwd: &Path) -> Result<String, Error> {
    let mut cmd = Command::new("nix");
    cmd.args(["flake", "metadata", "--json", "--no-write-lock-file"])
        .current_dir(cwd);
    let out = run_nix(cmd)?;
    forward_stderr(&out.stderr);
    if !out.status.success() {
        return Err(Error::msg(format!(
            "nix flake metadata failed (exit code {}); the current directory is not a usable flake",
            exit_code(out.status)
        )));
    }
    let value: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    locked_ref_from_metadata(&value)
}

/// Run the embedded walker with `nix eval` and parse its JSON output.
///
/// `parts` are canonical component arrays and travel as `FSN_PARTS_JSON`
/// (e.g. `[["a","b"],["c"]]`); the walker only matches prefixes.
pub fn eval_tree(
    cwd: &Path,
    flake_ref: &str,
    parts: &[Vec<String>],
    allow_ifd: bool,
) -> Result<WalkerOutput, Error> {
    let parts_json = serde_json::to_string(parts)?;
    let expr = format!("({WALKER_SRC}) {{ }}");

    let mut cmd = Command::new("nix");
    cmd.args(eval_args(&expr, allow_ifd));
    cmd.current_dir(cwd)
        .env("FSN_FLAKE_PATH", flake_ref)
        .env("FSN_PARTS_JSON", parts_json);

    let out = run_nix(cmd)?;
    forward_stderr(&out.stderr);
    if !out.status.success() {
        if !allow_ifd && stderr_mentions_ifd(&out.stderr) {
            eprintln!("{IFD_HINT}");
        }
        return Err(Error::msg(format!(
            "nix eval failed (exit code {})",
            exit_code(out.status)
        )));
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

#[cfg(test)]
mod tests {
    use super::{eval_args, locked_ref_from_metadata};

    /// Value of `--option allow-import-from-derivation <value>` in an argv.
    fn ifd_option(args: &[String]) -> Option<String> {
        args.windows(3)
            .find(|w| w[0] == "--option" && w[1] == "allow-import-from-derivation")
            .map(|w| w[2].clone())
    }

    #[test]
    fn eval_args_pass_ifd_option_false_by_default() {
        let args = eval_args("(walker) { }", false);
        assert_eq!(
            ifd_option(&args).as_deref(),
            Some("false"),
            "the default must explicitly pass --option allow-import-from-derivation false instead of relying on nix.conf"
        );
        assert!(args.iter().any(|a| a == "--impure"));
        assert!(args.iter().any(|a| a == "--no-write-lock-file"));
        assert!(args.iter().any(|a| a == "--json"));
        assert_eq!(args.first().map(String::as_str), Some("eval"));
        assert_eq!(args.last().map(String::as_str), Some("(walker) { }"));
    }

    #[test]
    fn eval_args_pass_ifd_option_true_with_allow_ifd() {
        let args = eval_args("(walker) { }", true);
        assert_eq!(
            ifd_option(&args).as_deref(),
            Some("true"),
            "--allow-ifd must explicitly pass true to override a disabled nix.conf"
        );
        assert_eq!(
            args.iter().filter(|a| *a == "--option").count(),
            1,
            "only one --option may appear"
        );
    }

    #[test]
    fn locked_ref_from_metadata_extracts_url() {
        let metadata = serde_json::json!({ "url": "git+file:///abs/path?submodules=1" });
        assert_eq!(
            locked_ref_from_metadata(&metadata).unwrap(),
            "git+file:///abs/path?submodules=1"
        );
    }

    #[test]
    fn locked_ref_from_metadata_rejects_missing_or_non_string_url() {
        let missing = serde_json::json!({ "path": "/tmp/x" });
        assert!(locked_ref_from_metadata(&missing).is_err());
        let non_string = serde_json::json!({ "url": 42 });
        assert!(locked_ref_from_metadata(&non_string).is_err());
    }
}
