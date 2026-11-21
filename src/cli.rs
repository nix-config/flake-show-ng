use std::path::PathBuf;

use clap::Parser;

use crate::attrpath::AttrPath;
use crate::filter::Filters;

#[derive(Parser, Debug)]
#[command(
    name = "fsn",
    version,
    disable_version_flag = true,
    about = "Show the complete tree of the current directory's flake outputs",
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
    #[arg(short = 'i', long = "allow-ifd")]
    pub allow_ifd: bool,

    /// Keep parts whose subtree is empty
    #[arg(short = 'e', long = "empty")]
    pub empty: bool,

    /// Flake project directory
    #[arg(short = 'f', long = "flake", value_name = "PATH", default_value = ".")]
    pub flake: PathBuf,

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
  fsn -f ../other-flake                  # complete tree of another flake directory
  fsn -p packages.x86_64-linux           # one subtree
  fsn -p packages -p legacyPackages      # multiple parts, rendered as one forest
  fsn -p packages,legacyPackages         # same, comma-separated
  fsn -e                                 # keep parts whose subtree is empty
  fsn -i                                 # allow import-from-derivation

Runtime dependency: nix >= 2.34.8.";

impl Cli {
    pub fn into_filters(self) -> Filters {
        Filters::new(&self.parts, self.allow_ifd, self.empty)
    }
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
        let cli = Cli::try_parse_from(["fsn", "-i"]).unwrap();
        assert!(cli.allow_ifd);
        let cli = Cli::try_parse_from(["fsn", "--allow-ifd"]).unwrap();
        assert!(cli.allow_ifd);
    }

    #[test]
    fn rejects_removed_all_flag() {
        assert!(Cli::try_parse_from(["fsn", "-a"]).is_err());
        assert!(Cli::try_parse_from(["fsn", "--all"]).is_err());
    }

    #[test]
    fn parses_empty() {
        let cli = Cli::try_parse_from(["fsn", "-e"]).unwrap();
        assert!(cli.empty);
        let cli = Cli::try_parse_from(["fsn", "--empty"]).unwrap();
        assert!(cli.empty);
    }

    #[test]
    fn parses_flake_path() {
        let cli = Cli::try_parse_from(["fsn"]).unwrap();
        assert_eq!(cli.flake, PathBuf::from("."));
        let cli = Cli::try_parse_from(["fsn", "-f", "../other"]).unwrap();
        assert_eq!(cli.flake, PathBuf::from("../other"));
        let cli = Cli::try_parse_from(["fsn", "--flake", "/tmp/flake"]).unwrap();
        assert_eq!(cli.flake, PathBuf::from("/tmp/flake"));
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
    fn into_filters_carries_the_three_layers() {
        let cli = Cli::try_parse_from(["fsn", "-p", "a.b", "-e"]).unwrap();
        let filters = cli.into_filters();
        assert!(!filters.allow_ifd);
        assert_eq!(filters.parts.roots, vec![p("a.b")]);
        assert_eq!(filters.empty, crate::prune::EmptyPolicy::Keep);
    }
}
