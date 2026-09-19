use clap::Parser;

use crate::attrpath::AttrPath;

/// fsn - show the complete tree of a flake's outputs.
#[derive(Parser, Debug)]
#[command(
    name = "fsn",
    version,
    disable_version_flag = true,
    about = "Show the complete tree of the current directory's flake outputs (attrpaths and semantic leaves fully expanded)",
    after_help = HELP_TAIL
)]
pub struct Cli {
    /// Only show the subtree at the given attrpath; repeatable, and comma-separated values are accepted
    #[arg(
        short = 'p',
        long = "part",
        value_name = "ATTRPATH",
        value_delimiter = ','
    )]
    pub parts: Vec<AttrPath>,

    /// Allow import-from-derivation (IFD; may trigger builds)
    #[arg(short = 'a', long = "allow-ifd")]
    pub allow_ifd: bool,

    /// Print version and exit
    // Required by clap-derive to define `-v` (`ArgAction::Version` prints and
    // exits during parsing), so it is never read as a value.
    #[allow(dead_code)]
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    pub version: Option<bool>,
}

const HELP_TAIL: &str = "\
Examples:
  fsn                                    # complete tree of the current directory's flake
  fsn -p packages.x86_64-linux           # one subtree
  fsn -p packages -p legacyPackages      # multiple parts, rendered as one forest
  fsn -p packages,legacyPackages         # same, comma-separated
  fsn -a                                 # allow import-from-derivation

Runtime dependency: nix >= 2.34.8; fsn only acts on the current directory and does not accept a flake path.";

/// Fully resolved command line: everything `pipeline::run` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub allow_ifd: bool,
    pub requested: Vec<AttrPath>,
}

impl Cli {
    pub fn into_args(self) -> Args {
        Args {
            allow_ifd: self.allow_ifd,
            requested: self.parts,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parts {
    /// Effective parts, in request order, without exact duplicates and without
    /// any part that is covered by another (kept) part.
    pub roots: Vec<AttrPath>,
    /// (swallowed, ancestor) pairs in report order; the caller prints them.
    pub notices: Vec<(AttrPath, AttrPath)>,
}

/// Apply the frozen multi-part rules:
/// request order, exact dedup, ancestor wins over descendant.  Pure: it
/// computes the swallow notices and leaves printing to the caller.
pub fn resolve_parts(raw: &[AttrPath]) -> Parts {
    let mut roots: Vec<AttrPath> = Vec::new();
    let mut notices: Vec<(AttrPath, AttrPath)> = Vec::new();

    for part in raw {
        if roots.contains(part) {
            // Exact duplicate: silently dropped.
            continue;
        }
        if let Some(ancestor) = roots.iter().find(|p| p.is_prefix_of(part)) {
            notices.push((part.clone(), ancestor.clone()));
            continue;
        }
        let covered: Vec<AttrPath> = roots
            .iter()
            .filter(|p| part.is_prefix_of(p))
            .cloned()
            .collect();
        for descendant in covered {
            notices.push((descendant, part.clone()));
        }
        roots.retain(|p| !part.is_prefix_of(p));
        roots.push(part.clone());
    }

    Parts { roots, notices }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn p(s: &str) -> AttrPath {
        AttrPath::from_str(s).unwrap()
    }

    #[test]
    fn parses_repeated_and_comma_separated_parts() {
        let cli = Cli::try_parse_from(["fsn", "-p", "a.b,c", "-p", "d"]).unwrap();
        assert_eq!(cli.parts, vec![p("a.b"), p("c"), p("d")]);
        assert!(!cli.allow_ifd);
    }

    #[test]
    fn parses_allow_ifd() {
        let cli = Cli::try_parse_from(["fsn", "-a"]).unwrap();
        assert!(cli.allow_ifd);
        let cli = Cli::try_parse_from(["fsn", "--allow-ifd"]).unwrap();
        assert!(cli.allow_ifd);
    }

    #[test]
    fn version_flag_is_lowercase_v() {
        let err = Cli::try_parse_from(["fsn", "-v"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        let err = Cli::try_parse_from(["fsn", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn rejects_uppercase_v() {
        assert!(Cli::try_parse_from(["fsn", "-V"]).is_err());
    }

    #[test]
    fn rejects_empty_part() {
        assert!(Cli::try_parse_from(["fsn", "-p", ""]).is_err());
        assert!(Cli::try_parse_from(["fsn", "-p", "a,,b"]).is_err());
    }

    #[test]
    fn rejects_empty_component() {
        assert!(Cli::try_parse_from(["fsn", "-p", "a..b"]).is_err());
        assert!(Cli::try_parse_from(["fsn", "-p", ".a"]).is_err());
        assert!(Cli::try_parse_from(["fsn", "-p", "a."]).is_err());
    }

    #[test]
    fn into_args_carries_requested_parts() {
        let cli = Cli::try_parse_from(["fsn", "-p", "a.b"]).unwrap();
        let args = cli.into_args();
        assert!(!args.allow_ifd);
        assert_eq!(args.requested, vec![p("a.b")]);
    }

    #[test]
    fn resolve_dedupes_exact_parts() {
        let resolved = resolve_parts(&[p("a.b"), p("a.b"), p("c")]);
        assert_eq!(resolved.roots, vec![p("a.b"), p("c")]);
        assert!(resolved.notices.is_empty());
    }

    #[test]
    fn resolve_keeps_ancestor_and_reports_swallowed() {
        // descendant first, ancestor later
        let resolved = resolve_parts(&[p("a.b.c"), p("a"), p("d.e")]);
        assert_eq!(resolved.roots, vec![p("a"), p("d.e")]);
        assert_eq!(resolved.notices, vec![(p("a.b.c"), p("a"))]);

        // ancestor first, descendant later
        let resolved = resolve_parts(&[p("a"), p("a.b"), p("a.b.c")]);
        assert_eq!(resolved.roots, vec![p("a")]);
        assert_eq!(
            resolved.notices,
            vec![(p("a.b"), p("a")), (p("a.b.c"), p("a"))]
        );
    }

    #[test]
    fn resolve_siblings_keep_request_order() {
        let resolved = resolve_parts(&[p("packages"), p("legacyPackages.x86_64-linux")]);
        assert_eq!(
            resolved.roots,
            vec![p("packages"), p("legacyPackages.x86_64-linux")]
        );
        assert!(resolved.notices.is_empty());
    }

    #[test]
    fn resolve_pins_the_repeated_descendant_notice_order() {
        // The exact shape captured in `swallow-pinned.stderr`.
        let resolved = resolve_parts(&[p("a.b"), p("a"), p("a.b")]);
        assert_eq!(resolved.roots, vec![p("a")]);
        assert_eq!(
            resolved.notices,
            vec![(p("a.b"), p("a")), (p("a.b"), p("a"))]
        );
    }
}
