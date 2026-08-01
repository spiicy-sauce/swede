//! `swede-semantics` — static validation and basis resolution over the AST.

pub mod basis;
mod diagnostic;
pub mod model;
mod validate;

pub use basis::BasisTable;
pub use diagnostic::{format_line, Diagnostic, Severity};
pub use model::{build as build_graph, Graph, Node, NodeId, NodeKind};

use std::collections::HashSet;
use swede_syntax::*;

/// Validate a parsed file. Syntax errors are surfaced as `E000`.
pub fn validate(lowered: &Lowered) -> Vec<Diagnostic> {
    let mut diags: Vec<Diagnostic> = lowered
        .errors
        .iter()
        .map(|e| Diagnostic::error("E000", format!("syntax: {}", e.message), e.span))
        .collect();

    match &lowered.file {
        Some(File::Recipe(r)) => diags.extend(validate::validate_recipe(r)),
        Some(File::Menu(m)) => diags.extend(validate_menu(m)),
        None => {}
    }

    // stable order: by source position, then code
    diags.sort_by(|a, b| {
        a.span
            .start_byte
            .cmp(&b.span.start_byte)
            .then(a.code.cmp(&b.code))
    });
    diags
}

/// Validate Swede source directly.
pub fn validate_source(source: &str) -> Vec<Diagnostic> {
    validate(&swede_syntax::parse(source))
}

fn validate_menu(m: &Menu) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let aliases: HashSet<&str> = m.entries.iter().map(|e| e.alias.as_str()).collect();
    for entry in &m.entries {
        for link in &entry.links {
            if let Link::By { target, span } | Link::During { target, span } = link {
                // E006: a dotted target's alias must exist in this menu. The
                // node half needs the referenced recipe loaded (deferred).
                if let Some(_node) = &target.member {
                    if !aliases.contains(target.name.as_str()) {
                        diags.push(Diagnostic::error(
                            "E006",
                            format!("menu reference to unknown alias '{}'", target.name),
                            *span,
                        ));
                    }
                } else if !aliases.contains(target.name.as_str()) {
                    diags.push(Diagnostic::error(
                        "E006",
                        format!("menu reference to unknown alias '{}'", target.name),
                        *span,
                    ));
                }
            }
        }
    }
    diags
}
