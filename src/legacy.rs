//! `legacyPackages` is an untyped package-set escape hatch, so helper attrs from
//! `makeScope` would otherwise drown the real packages.

use crate::model::{Kind, Node};

pub fn clean(tree: Vec<Node>) -> Vec<Node> {
    tree.into_iter().filter_map(clean_top).collect()
}

fn clean_top(mut node: Node) -> Option<Node> {
    if !is_legacy(&node) {
        return Some(node);
    }
    clean_node(&mut node).then_some(node)
}

fn is_legacy(node: &Node) -> bool {
    node.path
        .components()
        .first()
        .is_some_and(|component| component == "legacyPackages")
}

fn clean_node(node: &mut Node) -> bool {
    if matches!(node.kind, Kind::Derivation | Kind::Error) {
        return true;
    }

    let Some(children) = node.children.as_mut() else {
        return false;
    };

    // A natively empty system must survive for `--empty`; a container that only
    // became empty because its helpers were removed must not.
    let originally_empty = children.is_empty();
    children.retain_mut(clean_node);
    !children.is_empty() || originally_empty
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrpath::AttrPath;

    fn node(name: &str, kind: Kind, path: &[&str], children: Option<Vec<Node>>) -> Node {
        Node {
            name: name.to_owned(),
            kind,
            path: AttrPath::from_components(path.iter().map(|s| (*s).to_owned()).collect()),
            children,
            drv_name: None,
            description: None,
            config_type: None,
            label: None,
        }
    }

    fn names(nodes: &[Node]) -> Vec<&str> {
        nodes.iter().map(|node| node.name.as_str()).collect()
    }

    #[test]
    fn keeps_derivations_and_their_ancestors() {
        let tree = vec![node(
            "legacyPackages",
            Kind::Attrset,
            &["legacyPackages"],
            Some(vec![node(
                "x86_64-linux",
                Kind::Attrset,
                &["legacyPackages", "x86_64-linux"],
                Some(vec![node(
                    "nested",
                    Kind::Attrset,
                    &["legacyPackages", "x86_64-linux", "nested"],
                    Some(vec![node(
                        "pkg",
                        Kind::Derivation,
                        &["legacyPackages", "x86_64-linux", "nested", "pkg"],
                        None,
                    )]),
                )]),
            )]),
        )];

        let cleaned = clean(tree);
        let x86 = &cleaned[0].children.as_ref().unwrap()[0];
        let nested = &x86.children.as_ref().unwrap()[0];
        assert_eq!(names(nested.children.as_ref().unwrap()), vec!["pkg"]);
    }

    #[test]
    fn drops_helpers_and_filtered_empty_containers() {
        let tree = vec![node(
            "legacyPackages",
            Kind::Attrset,
            &["legacyPackages"],
            Some(vec![node(
                "x86_64-linux",
                Kind::Attrset,
                &["legacyPackages", "x86_64-linux"],
                Some(vec![
                    node(
                        "callPackage",
                        Kind::Function,
                        &["legacyPackages", "x86_64-linux", "callPackage"],
                        None,
                    ),
                    node(
                        "__functionArgs",
                        Kind::Attrset,
                        &["legacyPackages", "x86_64-linux", "__functionArgs"],
                        Some(vec![node(
                            "lib",
                            Kind::String,
                            &["legacyPackages", "x86_64-linux", "__functionArgs", "lib"],
                            None,
                        )]),
                    ),
                    node(
                        "packages",
                        Kind::Attrset,
                        &["legacyPackages", "x86_64-linux", "packages"],
                        Some(vec![]),
                    ),
                    node(
                        "real",
                        Kind::Derivation,
                        &["legacyPackages", "x86_64-linux", "real"],
                        None,
                    ),
                ]),
            )]),
        )];

        let cleaned = clean(tree);
        let x86 = &cleaned[0].children.as_ref().unwrap()[0];
        assert_eq!(
            names(x86.children.as_ref().unwrap()),
            vec!["packages", "real"]
        );
    }

    #[test]
    fn keeps_error_leaves() {
        let tree = vec![node(
            "legacyPackages",
            Kind::Attrset,
            &["legacyPackages"],
            Some(vec![node(
                "broken",
                Kind::Error,
                &["legacyPackages", "broken"],
                None,
            )]),
        )];

        let cleaned = clean(tree);
        assert_eq!(names(cleaned[0].children.as_ref().unwrap()), vec!["broken"]);
    }

    #[test]
    fn leaves_non_legacy_subtrees_untouched() {
        let tree = vec![node(
            "packages",
            Kind::Attrset,
            &["packages"],
            Some(vec![node("fn", Kind::Function, &["packages", "fn"], None)]),
        )];

        let cleaned = clean(tree);
        assert_eq!(names(cleaned[0].children.as_ref().unwrap()), vec!["fn"]);
    }
}
