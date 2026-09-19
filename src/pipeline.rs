//! The ordered orchestration: resolved parts -> walker output -> rendered bytes.

use std::path::Path;

use crate::cli::{self, Args, Format};
use crate::error::{Error, stderr_prefix};
use crate::model::UnknownPart;
use crate::{nix, render};

/// Run the whole pipeline and return the bytes `main` should write to stdout.
pub fn run(args: Args) -> Result<String, Error> {
    let parts = cli::resolve_parts(&args.requested);

    // Ordering constraint: the swallow notices are printed here, immediately
    // after resolving and before the cwd / flake.nix checks, in the resolution
    // report order, so the stderr bytes and order are unchanged.
    for (swallowed, ancestor) in &parts.notices {
        eprintln!(
            "{}part '{swallowed}' is covered by ancestor part '{ancestor}'; not rendered separately",
            stderr_prefix()
        );
    }

    let cwd = std::env::current_dir()?;
    ensure_flake_present(&cwd)?;

    // The walker boundary takes the flake path plus canonical component arrays.
    let flake_ref = cwd.to_string_lossy().into_owned();
    let part_components: Vec<Vec<String>> = parts
        .roots
        .iter()
        .map(|part| part.components().to_vec())
        .collect();

    let locked_ref = nix::flake_metadata(&cwd)?;
    let walker = nix::eval_tree(&cwd, &flake_ref, &part_components, args.allow_ifd)?;

    if !walker.unknown_parts.is_empty() {
        report_unknown_parts(&walker.unknown_parts);
        return Err(Error::Silent);
    }

    match args.format {
        Format::Text => Ok(render::text::render(&locked_ref, &walker.tree)),
        Format::Json => render::json::render(&locked_ref, &parts.roots, &walker.tree),
    }
}

/// `fsn` only acts on the current directory, so a missing `flake.nix` is the
/// first user-facing failure.
fn ensure_flake_present(cwd: &Path) -> Result<(), Error> {
    let flake_nix = cwd.join("flake.nix");
    if flake_nix.is_file() {
        return Ok(());
    }
    Err(Error::msg(format!(
        "current directory is not a flake directory ({} not found); fsn only acts on the current directory",
        flake_nix.display()
    )))
}

/// One wording function for every unknown part, byte-identical to `nix flake show`.
fn report_unknown_parts(unknown_parts: &[UnknownPart]) {
    for unknown in unknown_parts {
        eprintln!("{}unknown part '{}'", stderr_prefix(), unknown.part);
        let at = if unknown.fail_at.is_empty() {
            "flake outputs".to_owned()
        } else {
            format!("'{}'", unknown.fail_at)
        };
        if unknown.candidates.is_empty() {
            eprintln!("  cannot descend past {at} (not an attrset, or an evaluation-failed leaf)");
        } else {
            let names: Vec<String> = unknown.candidates.iter().map(ToString::to_string).collect();
            eprintln!("  sibling candidates under {at}: {}", names.join(", "));
        }
    }
}
