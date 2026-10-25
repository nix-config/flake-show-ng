//! Recursive pruning of empty subtree nodes.

use crate::model::Node;

/// Drop every subtree node whose recursively pruned children list is empty.
///
/// Leaf nodes (`children == None`) are always kept, so only attrsets that
/// bottom out in nothing disappear; empty top-level nodes are dropped too.
pub fn prune_empty(tree: Vec<Node>) -> Vec<Node> {
    tree.into_iter()
        .filter_map(|mut node| match node.children.take() {
            Some(children) => {
                let kept = prune_empty(children);
                if kept.is_empty() {
                    None
                } else {
                    node.children = Some(kept);
                    Some(node)
                }
            }
            None => Some(node),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::prune_empty;
    use crate::attrpath::AttrPath;
    use crate::model::{Kind, Node};

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

    #[test]
    fn drops_empty_top_level_attrset() {
        let tree = vec![node("empty", Kind::Attrset, &["empty"], Some(vec![]))];
        assert!(prune_empty(tree).is_empty());
    }

    #[test]
    fn keeps_leaf_nodes() {
        let tree = vec![
            node("fn", Kind::Function, &["fn"], None),
            node("s", Kind::String, &["s"], None),
        ];
        assert_eq!(prune_empty(tree).len(), 2);
    }

    #[test]
    fn keeps_namespace_and_prunes_only_empty_architecture() {
        let tree = vec![node(
            "packages",
            Kind::Attrset,
            &["packages"],
            Some(vec![
                node(
                    "aarch64-linux",
                    Kind::Attrset,
                    &["packages", "aarch64-linux"],
                    Some(vec![]),
                ),
                node(
                    "x86_64-linux",
                    Kind::Attrset,
                    &["packages", "x86_64-linux"],
                    Some(vec![node(
                        "hello",
                        Kind::Derivation,
                        &["packages", "x86_64-linux", "hello"],
                        None,
                    )]),
                ),
            ]),
        )];

        let pruned = prune_empty(tree);
        assert_eq!(pruned.len(), 1);
        let archs = pruned[0].children.as_ref().unwrap();
        assert_eq!(archs.len(), 1);
        assert_eq!(archs[0].name, "x86_64-linux");
    }

    #[test]
    fn drops_namespace_whose_architectures_are_all_empty() {
        let tree = vec![node(
            "apps",
            Kind::Attrset,
            &["apps"],
            Some(vec![
                node(
                    "aarch64-linux",
                    Kind::Attrset,
                    &["apps", "aarch64-linux"],
                    Some(vec![]),
                ),
                node(
                    "x86_64-linux",
                    Kind::Attrset,
                    &["apps", "x86_64-linux"],
                    Some(vec![]),
                ),
            ]),
        )];
        assert!(prune_empty(tree).is_empty());
    }

    #[test]
    fn keeps_attrset_with_non_empty_child() {
        let tree = vec![node(
            "packages",
            Kind::Attrset,
            &["packages"],
            Some(vec![node(
                "hello",
                Kind::Derivation,
                &["packages", "hello"],
                None,
            )]),
        )];
        assert_eq!(prune_empty(tree).len(), 1);
    }

    #[test]
    fn removes_deep_empty_attrset_but_keeps_siblings() {
        let tree = vec![node(
            "packages",
            Kind::Attrset,
            &["packages"],
            Some(vec![
                node("empty", Kind::Attrset, &["packages", "empty"], Some(vec![])),
                node("hello", Kind::Derivation, &["packages", "hello"], None),
            ]),
        )];

        let pruned = prune_empty(tree);
        let children = pruned[0].children.as_ref().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "hello");
    }
}
