//! JSON schema v1 document assembly.

use crate::attrpath::AttrPath;
use crate::error::Error;
use crate::model::{FlakeTree, Node};

/// `input` is always the current directory, which is the flake root.
pub const INPUT_ROOT: &str = ".";

/// Serialize schema v1: `schemaVersion`/`lockedRef`/`input`/`parts`/`tree`.
pub fn render(locked_ref: &str, parts: &[AttrPath], tree: &[Node]) -> Result<String, Error> {
    let document = FlakeTree {
        schema_version: 1,
        locked_ref,
        input: INPUT_ROOT,
        parts: parts.iter().map(ToString::to_string).collect(),
        tree,
    };
    let mut out = serde_json::to_string_pretty(&document)?;
    out.push('\n');
    Ok(out)
}
