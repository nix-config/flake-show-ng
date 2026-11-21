//! ANSI is unconditional (no TTY check) to stay byte-identical to `nix flake show`.

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

pub fn text(locked_ref: &str, tree: &[Node]) -> String {
    let mut out = String::new();
    out.push_str(BOLD);
    out.push_str(locked_ref);
    out.push_str(RESET);
    out.push('\n');
    let mut prefix = String::new();
    render_nodes(&mut out, tree, &mut prefix);
    out
}

fn render_nodes(out: &mut String, nodes: &[Node], prefix: &mut String) {
    for (index, node) in nodes.iter().enumerate() {
        let last = index + 1 == nodes.len();

        out.push_str(GREEN_BOLD);
        out.push_str(prefix);
        out.push_str(if last { CONNECTOR_LAST } else { CONNECTOR_MID });
        out.push_str(RESET);
        out.push_str(BOLD);
        out.push_str(&node.name);
        out.push_str(RESET);
        annotation(out, node);
        out.push('\n');

        if let Some(children) = &node.children {
            let len = prefix.len();
            prefix.push_str(if last { INDENT_LAST } else { INDENT_MID });
            render_nodes(out, children, prefix);
            prefix.truncate(len);
        }
    }
}

fn annotation(out: &mut String, node: &Node) {
    match node.kind {
        // The walker always sets `drvName` on derivation nodes, so there is no
        // defensive "unknown" fallback.
        Kind::Derivation => {
            if let Some(drv) = node.drv_name.as_deref() {
                out.push_str(": ");
                out.push_str(label_wording(node.label));
                out.push_str(" '");
                out.push_str(drv);
                out.push('\'');
            }
        }
        Kind::App => {
            out.push_str(": app: ");
            styled(out, BOLD, description(node));
        }
        Kind::Template => {
            out.push_str(": template: ");
            styled(out, BOLD, description(node));
        }
        // `configType` is a closed enum, so unrecognized values cannot reach here.
        Kind::Config => {
            if let Some(config_type) = node.config_type {
                out.push_str(": ");
                styled(out, MAGENTA_BOLD, config_wording(config_type));
            }
        }
        Kind::NixosModule => {
            out.push_str(": ");
            styled(out, MAGENTA_BOLD, "NixOS module");
        }
        Kind::NixpkgsOverlay => {
            out.push_str(": ");
            styled(out, MAGENTA_BOLD, "Nixpkgs overlay");
        }
        Kind::HomeModule => {
            out.push_str(": ");
            styled(out, MAGENTA_BOLD, "Home module");
        }
        Kind::Error => {
            out.push_str(": ");
            styled(out, RED_BOLD, "error");
        }
        _ => {}
    }
}

fn styled(out: &mut String, style: &str, text: &str) {
    out.push_str(style);
    out.push_str(text);
    out.push_str(RESET);
}

fn description(node: &Node) -> &str {
    node.description
        .as_ref()
        .and_then(|d| d.as_deref())
        .unwrap_or("no description")
}

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
