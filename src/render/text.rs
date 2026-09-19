//! Byte-faithful tree renderer (ANSI is unconditional, matching `nix flake show`).

use crate::model::{ConfigType, Kind, Label, Node};

const RESET: &str = "\u{1b}[0m";
const BOLD: &str = "\u{1b}[1m";
const GREEN_BOLD: &str = "\u{1b}[32;1m";
const MAGENTA_BOLD: &str = "\u{1b}[35;1m";
const RED_BOLD: &str = "\u{1b}[31;1m";

const CONNECTOR_MID: &str = "├───";
const CONNECTOR_LAST: &str = "└───";
const INDENT_MID: &str = "│   ";
const INDENT_LAST: &str = "    ";

/// Render the whole tree.  ANSI colors are unconditional (no TTY check).
pub fn render(locked_ref: &str, tree: &[Node]) -> String {
    let mut out = String::new();
    out.push_str(BOLD);
    out.push_str(locked_ref);
    out.push_str(RESET);
    out.push('\n');
    render_nodes(&mut out, tree, "");
    out
}

fn render_nodes(out: &mut String, nodes: &[Node], prefix: &str) {
    for (index, node) in nodes.iter().enumerate() {
        let last = index + 1 == nodes.len();
        let connector = if last { CONNECTOR_LAST } else { CONNECTOR_MID };

        out.push_str(GREEN_BOLD);
        out.push_str(prefix);
        out.push_str(connector);
        out.push_str(RESET);
        out.push_str(BOLD);
        out.push_str(&node.name);
        out.push_str(RESET);
        out.push_str(&annotation(node));
        out.push('\n');

        if let Some(children) = &node.children {
            let child_prefix = if last {
                format!("{prefix}{INDENT_LAST}")
            } else {
                format!("{prefix}{INDENT_MID}")
            };
            render_nodes(out, children, &child_prefix);
        }
    }
}

fn annotation(node: &Node) -> String {
    match node.kind {
        // The walker always sets `drvName` on derivation nodes, so there is no
        // defensive "unknown" fallback.
        Kind::Derivation => match node.drv_name.as_deref() {
            Some(drv) => format!(": {} '{drv}'", label_wording(node.label)),
            None => String::new(),
        },
        Kind::App => format!(": app: {BOLD}{}{RESET}", description_text(node)),
        Kind::Template => format!(": template: {BOLD}{}{RESET}", description_text(node)),
        // `configType` is a closed enum, so unrecognized values cannot reach here.
        Kind::Config => match node.config_type {
            Some(config_type) => format!(": {MAGENTA_BOLD}{}{RESET}", config_wording(config_type)),
            None => String::new(),
        },
        Kind::Error => format!(": {RED_BOLD}error{RESET}"),
        _ => String::new(),
    }
}

fn description_text(node: &Node) -> String {
    match node.description.as_ref().and_then(|d| d.as_deref()) {
        Some(description) => description.to_owned(),
        None => "no description".to_owned(),
    }
}

/// The walker-owned derivation label; absent means the plain `derivation` label.
fn label_wording(label: Option<Label>) -> &'static str {
    match label {
        Some(Label::Package) => "package",
        Some(Label::DevelopmentEnvironment) => "development environment",
        None => "derivation",
    }
}

fn config_wording(config_type: ConfigType) -> &'static str {
    match config_type {
        ConfigType::Nixos => "NixOS configuration",
        ConfigType::HomeManager => "Home Manager configuration",
        ConfigType::Darwin => "nix-darwin configuration",
    }
}
