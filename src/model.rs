//! Wire types for the walker JSON and the user JSON schema v1.
//!
//! `Node` deserialization accepts the walker shape; its serialization is
//! hand-written so the user-facing key order and the `skip_serializing_if`
//! presence semantics stay frozen.

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::attrpath::AttrPath;

/// Node kinds of the flake-show-ng JSON schema v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
/// `attrpath`). Serialization is manual: `attrpath` is computed here and
/// `label` is appended after the other kind-specific fields.
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

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Upper bound on the field count; `serialize_struct` treats it as a hint.
        let mut state = serializer.serialize_struct("Node", 8)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("attrpath", &self.path.to_string())?;
        if let Some(children) = &self.children {
            state.serialize_field("children", children)?;
        }
        if let Some(drv_name) = &self.drv_name {
            state.serialize_field("drvName", drv_name)?;
        }
        if let Some(description) = &self.description {
            state.serialize_field("description", description)?;
        }
        if let Some(config_type) = &self.config_type {
            state.serialize_field("configType", config_type)?;
        }
        if let Some(label) = &self.label {
            state.serialize_field("label", label)?;
        }
        state.end()
    }
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

/// Top level JSON document, schema v1.
#[derive(Debug, Serialize)]
pub struct FlakeTree<'a> {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "lockedRef")]
    pub locked_ref: &'a str,
    pub input: &'a str,
    pub parts: Vec<String>,
    pub tree: &'a [Node],
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn p(s: &str) -> AttrPath {
        AttrPath::from_str(s).unwrap()
    }

    #[test]
    fn kind_roundtrips() {
        // Wire names are pinned explicitly so the schema v1 contract cannot drift
        // from `rename_all = "lowercase"`.
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
            assert_eq!(serde_json::to_string(&kind).unwrap(), json);
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

    #[test]
    fn serializes_node_fields_in_frozen_order_and_appends_label_last() {
        let node = Node {
            name: "hello".to_owned(),
            kind: Kind::Derivation,
            path: p("packages.x86_64-linux.hello"),
            children: None,
            drv_name: Some("hello".to_owned()),
            description: None,
            config_type: None,
            label: Some(Label::Package),
        };
        let json = serde_json::to_string(&node).unwrap();
        assert_eq!(
            json,
            "{\"name\":\"hello\",\"kind\":\"derivation\",\
             \"path\":[\"packages\",\"x86_64-linux\",\"hello\"],\
             \"attrpath\":\"packages.x86_64-linux.hello\",\
             \"drvName\":\"hello\",\"label\":\"package\"}"
        );

        let plain = Node {
            label: None,
            ..node
        };
        let json = serde_json::to_string(&plain).unwrap();
        assert_eq!(
            json,
            "{\"name\":\"hello\",\"kind\":\"derivation\",\
             \"path\":[\"packages\",\"x86_64-linux\",\"hello\"],\
             \"attrpath\":\"packages.x86_64-linux.hello\",\
             \"drvName\":\"hello\"}"
        );
    }
}
