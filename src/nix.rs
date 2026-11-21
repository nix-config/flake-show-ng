use std::io::{IsTerminal, Write};
use std::path::Path;
use std::process::Command;

use crate::attrpath::AttrPath;
use crate::error::Error;
use crate::model::WalkerOutput;

const WALKER_SRC: &str = include_str!("walker.nix");

/// Pins the version used to record the rendering goldens.
pub const RECORDED_NIX_VERSION: &str = "2.34.8";

/// Always passed explicitly so a user nix.conf cannot silently re-enable IFD.
pub const IFD_OPTION: &str = "allow-import-from-derivation";

pub const IFD_HINT: &str = "hint: evaluation requires import from derivation (IFD), which is disabled by default; retry with --allow-ifd (may trigger builds)";

pub const NIX_MISSING_MSG: &str =
    "cannot run nix (runtime dependency nix >= 2.34.8; make sure it is on PATH)";

fn exit_code(status: std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => code.to_string(),
        None => "signal".to_owned(),
    }
}

// Nix disables coloring when its stderr is a pipe, but fsn always colors its
// own output; restore the warning style when our stderr is a terminal.
fn style_warnings(stderr: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(stderr.len());
    for line in stderr.split_inclusive(|&b| b == b'\n') {
        if let Some(rest) = line.strip_prefix(b"warning:") {
            out.extend_from_slice(b"\x1b[35;1mwarning:\x1b[0m");
            out.extend_from_slice(rest);
        } else {
            out.extend_from_slice(line);
        }
    }
    out
}

fn forward_stderr(stderr: &[u8]) {
    let styled = std::io::stderr()
        .is_terminal()
        .then(|| style_warnings(stderr));
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(styled.as_deref().unwrap_or(stderr));
    let _ = err.flush();
}

fn run_nix(mut cmd: Command) -> Result<std::process::Output, Error> {
    cmd.output()
        .map_err(|e| Error::msg(format!("{NIX_MISSING_MSG}: {e}")))
}

/// A string contract with Nix's own error text: if upstream rewords it, this
/// detection and its test must change. The exit code alone drives the error
/// path; this only decides whether to add the hint.
fn stderr_mentions_ifd(stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr).contains(IFD_OPTION)
}

fn eval_args(expr: &str, allow_ifd: bool) -> Vec<String> {
    vec![
        "eval".to_owned(),
        "--impure".to_owned(),
        "--no-write-lock-file".to_owned(),
        "--json".to_owned(),
        // `flake_metadata` already forwards the dirty-tree warning; the walker
        // resolves the same ref and would otherwise print it a second time.
        "--option".to_owned(),
        "warn-dirty".to_owned(),
        "false".to_owned(),
        "--option".to_owned(),
        IFD_OPTION.to_owned(),
        if allow_ifd { "true" } else { "false" }.to_owned(),
        "--expr".to_owned(),
        expr.to_owned(),
    ]
}

/// `.url` is byte-identical to the root line of `nix flake show`.
fn locked_ref_from_metadata(metadata: &serde_json::Value) -> Result<String, Error> {
    metadata
        .get("url")
        .and_then(|u| u.as_str())
        .map(str::to_owned)
        .ok_or_else(|| Error::msg("nix flake metadata --json output lacks .url"))
}

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

pub fn eval_tree(
    cwd: &Path,
    flake_ref: &str,
    parts: &[AttrPath],
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
    use super::{IFD_OPTION, eval_args, locked_ref_from_metadata, style_warnings};

    #[test]
    fn style_warnings_colors_only_warning_prefixes() {
        let input = b"warning: dirty\nplain\nwarning: two";
        assert_eq!(
            style_warnings(input),
            b"\x1b[35;1mwarning:\x1b[0m dirty\nplain\n\x1b[35;1mwarning:\x1b[0m two".to_vec()
        );
    }

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
            args.iter().filter(|a| *a == IFD_OPTION).count(),
            1,
            "only one allow-import-from-derivation option may appear"
        );
        assert_eq!(
            args.windows(3)
                .filter(|w| w[0] == "--option" && w[1] == "warn-dirty")
                .count(),
            1,
            "the walker must not repeat the dirty-tree warning already forwarded by flake_metadata"
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
