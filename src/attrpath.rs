//! `AttrPath`: the one typed representation of flake output paths.
//!
//! The wire format carries component arrays (`["a","b"]`); the display format
//! is the dotted attrpath (`a.b`), produced only by [`Display`].

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A validated sequence of attribute names, e.g. `legacyPackages.x86_64-linux`.
///
/// On the wire it is a component array (`["a","b"]`); the dotted form exists
/// only through [`Display`](std::fmt::Display).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttrPath(Vec<String>);

impl AttrPath {
    /// Build from already-validated components.
    ///
    /// Construction / test-facing API: production paths arrive through
    /// `FromStr` (CLI) or `Deserialize` (wire).
    pub fn from_components(components: Vec<String>) -> Self {
        AttrPath(components)
    }

    /// The components, without separators.
    pub fn components(&self) -> &[String] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// True when `self` is a prefix of (or equal to) `other`.
    pub fn is_prefix_of(&self, other: &AttrPath) -> bool {
        self.0.len() <= other.0.len() && other.0.starts_with(&self.0)
    }

    /// All proper ancestors, outermost first (`a` then `a.b` for `a.b.c`).
    pub fn ancestors(&self) -> impl Iterator<Item = AttrPath> + '_ {
        (1..self.0.len()).map(|len| AttrPath(self.0[..len].to_vec()))
    }
}

impl fmt::Display for AttrPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("."))
    }
}

impl FromStr for AttrPath {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err("attrpath must not be empty".to_owned());
        }
        let components: Vec<String> = s.split('.').map(str::to_owned).collect();
        if components.iter().any(String::is_empty) {
            return Err(format!("attrpath '{s}' has an empty component"));
        }
        Ok(AttrPath(components))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> AttrPath {
        AttrPath::from_str(s).unwrap()
    }

    #[test]
    fn parses_dotted_attrpaths() {
        assert_eq!(
            p("legacyPackages.x86_64-linux").components(),
            &["legacyPackages".to_owned(), "x86_64-linux".to_owned()]
        );
    }

    #[test]
    fn rejects_empty_and_empty_components() {
        assert_eq!(
            AttrPath::from_str("").unwrap_err(),
            "attrpath must not be empty"
        );
        assert_eq!(
            AttrPath::from_str("a..b").unwrap_err(),
            "attrpath 'a..b' has an empty component"
        );
        assert_eq!(
            AttrPath::from_str(".a").unwrap_err(),
            "attrpath '.a' has an empty component"
        );
        assert_eq!(
            AttrPath::from_str("a.").unwrap_err(),
            "attrpath 'a.' has an empty component"
        );
    }

    #[test]
    fn display_joins_components() {
        assert_eq!(
            p("legacyPackages.x86_64-linux").to_string(),
            "legacyPackages.x86_64-linux"
        );
    }

    #[test]
    fn is_prefix_of_matches_equal_and_descendants_only() {
        assert!(p("a").is_prefix_of(&p("a")));
        assert!(p("a").is_prefix_of(&p("a.b")));
        assert!(!p("a.b").is_prefix_of(&p("a")));
        assert!(!p("a.b").is_prefix_of(&p("a.c")));
        assert!(!p("ab").is_prefix_of(&p("a.b")));
    }

    #[test]
    fn ancestors_are_outermost_first() {
        assert_eq!(
            p("a.b.c").ancestors().collect::<Vec<_>>(),
            vec![p("a"), p("a.b")]
        );
        assert_eq!(p("a").ancestors().count(), 0);
    }
}
