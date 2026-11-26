use std::path::Path;

use crate::error::{Error, stderr_prefix};
use crate::filter::Filters;
use crate::model::UnknownPart;
use crate::{legacy, nix, render};

pub fn run(filters: Filters, flake: &Path) -> Result<String, Error> {
    // Ordering constraint: the swallow notices are printed here,
    // immediately after resolving and before the flake path checks,
    // in the resolution report order, so the stderr bytes and order are unchanged.
    for (swallowed, ancestor) in &filters.parts.notices {
        eprintln!(
            "{}part '{swallowed}' is covered by ancestor part '{ancestor}'; not rendered separately",
            stderr_prefix()
        );
    }

    ensure_flake_present(flake)?;

    // The walker takes the locked ref plus typed parts.
    // A plain path ref bypasses Nix's flake eval cache and re-evaluates every run.
    let locked_ref = nix::flake_metadata(flake)?;
    let walker = nix::eval_tree(flake, &locked_ref, &filters.parts.roots, filters.allow_ifd)?;

    nix::print_warnings(&walker.warnings);

    if !walker.unknown_parts.is_empty() {
        report_unknown_parts(&walker.unknown_parts);
        return Err(Error::Silent);
    }

    let tree = legacy::clean(walker.tree);
    Ok(render::text(&locked_ref, &filters.empty.apply(tree)))
}

/// A missing `flake.nix` is the first user-facing failure for the selected path.
fn ensure_flake_present(flake: &Path) -> Result<(), Error> {
    let flake_nix = flake.join("flake.nix");
    if flake_nix.is_file() {
        return Ok(());
    }
    Err(Error::msg(format!(
        "'{}' is not a flake directory ({} not found)",
        flake.display(),
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
