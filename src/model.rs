//! Walker JSON is internal input, so these types deliberately implement no serialization.

use serde::{Deserialize, Deserializer};

use crate::attrpath::AttrPath;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Attrset,
    Derivation,
    App,
    Template,
    Config,
    NixosModule,
    NixpkgsOverlay,
    HomeModule,
    Function,
    String,
    Int,
    Float,
    Bool,
    Path,
    Null,
    List,
    Error,
    /// Forward-compatible fallback: a stray wire value renders as a bare name
    /// node instead of failing the whole run.
    #[serde(other)]
    Unknown,
}

/// Closed on purpose: only the walker's namespace table produces this, so an
/// unlisted value is a protocol violation. `Kind::Unknown` is the opposite case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Label {
    Package,
    DevelopmentEnvironment,
}

/// Closed on purpose: only the walker's config table produces this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigType {
    Nixos,
    HomeManager,
    Darwin,
}

// Explicit JSON null must survive as `Some(None)`; a plain `Option` would
// collapse it into a missing field.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Node {
    pub name: String,
    pub kind: Kind,
    pub path: AttrPath,
    #[serde(default)]
    pub children: Option<Vec<Node>>,
    #[serde(rename = "drvName", default)]
    pub drv_name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(rename = "configType", default)]
    pub config_type: Option<ConfigType>,
    #[serde(default)]
    pub label: Option<Label>,
}

#[derive(Debug, Default, Deserialize)]
pub struct WalkerOutput {
    #[serde(default)]
    pub tree: Vec<Node>,
    #[serde(rename = "unknownParts", default)]
    pub unknown_parts: Vec<UnknownPart>,
}

#[derive(Debug, Deserialize)]
pub struct UnknownPart {
    pub part: AttrPath,
    #[serde(rename = "failAt", default)]
    pub fail_at: AttrPath,
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
        // Pin wire names so a `rename_all` change cannot silently alter the contract.
        for (kind, name) in [
            (Kind::Attrset, "attrset"),
            (Kind::Derivation, "derivation"),
            (Kind::App, "app"),
            (Kind::Template, "template"),
            (Kind::Config, "config"),
            (Kind::NixosModule, "nixos-module"),
            (Kind::NixpkgsOverlay, "nixpkgs-overlay"),
            (Kind::HomeModule, "home-module"),
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
