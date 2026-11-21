//! Keeping the three filters in one value makes their pipeline order explicit
//! and keeps the emptiness policy out of the orchestration code.

use crate::attrpath::AttrPath;
use crate::prune::EmptyPolicy;

#[derive(Debug, PartialEq, Eq)]
pub struct Filters {
    pub allow_ifd: bool,
    pub parts: Parts,
    pub empty: EmptyPolicy,
}

impl Filters {
    pub fn new(requested: &[AttrPath], allow_ifd: bool, keep_empty: bool) -> Self {
        Self {
            allow_ifd,
            parts: resolve_parts(requested),
            empty: EmptyPolicy::from_keep(keep_empty),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parts {
    pub roots: Vec<AttrPath>,
    pub notices: Vec<(AttrPath, AttrPath)>,
}

/// Pure: callers own printing.
pub fn resolve_parts(raw: &[AttrPath]) -> Parts {
    let mut roots: Vec<AttrPath> = Vec::new();
    let mut notices: Vec<(AttrPath, AttrPath)> = Vec::new();

    for part in raw {
        if roots.contains(part) {
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
    fn new_wraps_the_three_filters() {
        let filters = Filters::new(&[p("a.b"), p("a")], true, true);
        assert!(filters.allow_ifd);
        assert_eq!(filters.parts.roots, vec![p("a")]);
        assert_eq!(filters.parts.notices, vec![(p("a.b"), p("a"))]);
        assert_eq!(filters.empty, EmptyPolicy::Keep);
    }

    #[test]
    fn resolve_dedupes_exact_parts() {
        let resolved = resolve_parts(&[p("a.b"), p("a.b"), p("c")]);
        assert_eq!(resolved.roots, vec![p("a.b"), p("c")]);
        assert!(resolved.notices.is_empty());
    }

    #[test]
    fn resolve_keeps_ancestor_and_reports_swallowed() {
        let resolved = resolve_parts(&[p("a.b.c"), p("a"), p("d.e")]);
        assert_eq!(resolved.roots, vec![p("a"), p("d.e")]);
        assert_eq!(resolved.notices, vec![(p("a.b.c"), p("a"))]);

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
