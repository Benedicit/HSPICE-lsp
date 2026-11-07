//! Element rules: unknown element letters, missing nodes and duplicate element names.

use super::common::repeated_definitions;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::resolve::ElementInfo;
use crate::analysis::symbols::SymbolKind;

/// Runs all element rules.
///
/// # Arguments
/// * `analysis` - semantic analysis
pub fn check(analysis: &Analysis) -> Vec<Diagnostic> {
    analysis
        .elements
        .iter()
        .filter(|info| !info.skipped)
        .filter_map(element_problem)
        .chain(duplicate_elements(analysis))
        .collect()
}

/// Reports an element with an unknown type letter or too few nodes.
///
/// # Arguments
/// * `info` - element with role assignment
fn element_problem(info: &ElementInfo) -> Option<Diagnostic> {
    let name = &info.name;
    let Some(spec) = info.spec else {
        let letter = name.text.chars().next().unwrap_or('?').to_ascii_uppercase();
        return Some(Diagnostic::new(
            name.range,
            Severity::Error,
            "unknown-element",
            format!(
                "`{}` is not an HSPICE element: no element type starts with `{letter}`",
                name.text
            ),
        ));
    };
    let message = if spec.letter == 'X' {
        subckt_instance_problem(info)?
    } else if info.roles.nodes.len() < spec.min_nodes {
        format!(
            "`{}` needs at least {} nodes, found {}",
            name.text,
            spec.min_nodes,
            info.roles.nodes.len()
        )
    } else {
        return None;
    };
    Some(Diagnostic::new(
        name.range,
        Severity::Error,
        "missing-nodes",
        message,
    ))
}

/// Describes what an X instance lacks: nodes or the subcircuit name.
///
/// # Arguments
/// * `info` - X element with role assignment
fn subckt_instance_problem(info: &ElementInfo) -> Option<String> {
    let name = &info.name.text;
    if info.roles.reference.is_none() {
        Some(format!("`{name}` has no subcircuit name"))
    } else if info.roles.nodes.is_empty() {
        Some(format!("`{name}` has no nodes before the subcircuit name"))
    } else {
        None
    }
}

/// Reports elements named twice in one scope, `.alter` section and `.lib` section (outside `.if`).
///
/// # Arguments
/// * `analysis` - semantic analysis
fn duplicate_elements(analysis: &Analysis) -> Vec<Diagnostic> {
    repeated_definitions(analysis, |symbol| symbol.kind == SymbolKind::Element)
        .into_iter()
        .map(|symbol| {
            Diagnostic::new(
                symbol.name.range,
                Severity::Error,
                "duplicate-element",
                format!(
                    "Element `{}` is already defined in this scope",
                    symbol.name.text
                ),
            )
        })
        .collect()
}
