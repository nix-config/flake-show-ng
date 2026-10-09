//! Wire types for the walker JSON.
//!
//! `Node` deserialization accepts the walker shape; the walker JSON is internal
//! input, so these types intentionally implement no serialization.

use serde::{Deserialize, Deserializer};

use crate::attrpath::AttrPath;

/// Node kinds of the internal walker wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Attrset,
    Derivation,
    App,
    Template,
    Config,
    Function,
    String,
    Int,
    Float,
    Bool,
    Path,
    Null,
    List,
    Error,
    /// Deserialization-only: not part of the schema and never produced by the
    /// walker. Unrecognized names fall back here so a stray value renders as a
    /// bare name node instead of failing the whole run.
    #[serde(other)]
    Unknown,
}

/// The derivation annotation owned by the walker (fed by its top-level
/// namespace table). Plain `derivation` nodes omit the field entirely.
///
/// Deliberately closed: the walker's namespace table is the only producer, so
/// an unlisted value is a protocol violation and deserialization is meant to
/// fail.  (Contrast `Kind::Unknown`, whose `#[serde(other)]` exists only for
/// forward-compatible deserialization.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Label {
    Package,
    DevelopmentEnvironment,
}

/// The flavor of a `config` node, mapped to wording by the text renderer.
///
/// Deliberately closed: the walker's config table is the only producer, so an
/// unlisted value is a protocol violation and deserialization is meant to fail
/// rather than degrade silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigType {
    Nixos,
    HomeManager,
    Darwin,
}

fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

/// A node of the output tree.
///
/// Deserialized from the walker shape (`path` as a component array, no
/// `attrpath`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Node {
    pub name: String,
    pub kind: Kind,
    /// Lossless path from the flake outputs root.
    pub path: AttrPath,
    #[serde(default)]
    pub children: Option<Vec<Node>>,
    #[serde(rename = "drvName", default)]
    pub drv_name: Option<String>,
    /// `Some(None)` means "explicit JSON null" (app/template without description).
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(rename = "configType", default)]
    pub config_type: Option<ConfigType>,
    #[serde(default)]
    pub label: Option<Label>,
}

/// Raw walker output (`nix eval --json` of `src/walker.nix`).
#[derive(Debug, Default, Deserialize)]
pub struct WalkerOutput {
    #[serde(default)]
    pub tree: Vec<Node>,
    #[serde(rename = "unknownParts", default)]
    pub unknown_parts: Vec<UnknownPart>,
}

#[derive(Debug, Deserialize)]
pub struct UnknownPart {
    /// Component array, e.g. `["a","b"]`.
    pub part: AttrPath,
    #[serde(rename = "failAt", default)]
    pub fail_at: AttrPath,
    /// Sibling candidates, each a component array.
    #[serde(default)]
    pub candidates: Vec<AttrPath>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn p(s: &str) -> AttrPath {
        AttrPath::from_str(s).unwrap()
    }

    #[test]
    fn kind_deserializes() {
        // Wire names are pinned explicitly so the deserialization contract
        // cannot drift from `rename_all = "lowercase"`.
        for (kind, name) in [
            (Kind::Attrset, "attrset"),
            (Kind::Derivation, "derivation"),
            (Kind::App, "app"),
            (Kind::Template, "template"),
            (Kind::Config, "config"),
            (Kind::Function, "function"),
            (Kind::String, "string"),
            (Kind::Int, "int"),
            (Kind::Float, "float"),
            (Kind::Bool, "bool"),
            (Kind::Path, "path"),
            (Kind::Null, "null"),
            (Kind::List, "list"),
            (Kind::Error, "error"),
            (Kind::Unknown, "unknown"),
        ] {
            let json = format!("\"{name}\"");
            assert_eq!(serde_json::from_str::<Kind>(&json).unwrap(), kind);
        }
        assert_eq!(
            serde_json::from_str::<Kind>("\"lambda\"").unwrap(),
            Kind::Unknown
        );
    }

    #[test]
    fn parses_walker_output_and_keeps_null_description() {
        let raw = r#"{
            "tree": [
                {
                    "name": "apps",
                    "kind": "attrset",
                    "path": ["apps"],
                    "children": [
                        {"name": "foo", "kind": "app", "path": ["apps", "foo"], "description": null}
                    ]
                }
            ],
            "unknownParts": [
                {"part": ["nope","x"], "failAt": ["nope"], "candidates": [["a"], ["b"]]}
            ]
        }"#;
        let parsed: WalkerOutput = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.tree.len(), 1);
        assert_eq!(parsed.tree[0].path, p("apps"));
        let child = &parsed.tree[0].children.as_ref().unwrap()[0];
        assert_eq!(child.kind, Kind::App);
        assert_eq!(child.description, Some(None));
        assert_eq!(parsed.unknown_parts.len(), 1);
        assert_eq!(parsed.unknown_parts[0].part, p("nope.x"));
        assert_eq!(parsed.unknown_parts[0].fail_at, p("nope"));
        assert_eq!(parsed.unknown_parts[0].candidates, vec![p("a"), p("b")]);
    }
}
