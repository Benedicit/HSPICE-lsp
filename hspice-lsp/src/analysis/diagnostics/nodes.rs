//! Node rules: floating nodes and missing ground connection.

use std::collections::HashSet;

use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::symbols::{ROOT_SCOPE, RefContext, ScopeId, SymbolDetail, SymbolKind};
use crate::lang::elements::is_ground;
use crate::state::document::SyntaxInfo;

/// Warns about nodes connected to exactly one element terminal that are neither ports nor global.
///
/// Elements of included files are not analysed, so scopes where they may add connections are
/// skipped: scopes with an unresolved include, subckt bodies with any include (PEX `.pxi` files)
/// and the top level of include files (connected by the including netlist).
///
/// # Arguments
/// * `syntax` - parse results (for the top-level flag)
/// * `analysis` - semantic analysis
pub fn floating_nodes(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let counts = analysis.connection_counts();
    let unchecked_scopes: HashSet<ScopeId> = analysis
        .includes
        .iter()
        .filter_map(|include| {
            let scope = analysis
                .structure
                .statement_scope
                .get(include.directive.statement)
                .copied()?;
            (include.error.is_some() || scope != ROOT_SCOPE).then_some(scope)
        })
        .chain((!syntax.is_top_level).then_some(ROOT_SCOPE))
        .collect();
    analysis
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, symbol)| symbol.is_local() && symbol.kind == SymbolKind::Node)
        .filter(|(_, symbol)| !unchecked_scopes.contains(&symbol.scope))
        .filter(|(_, symbol)| {
            matches!(
                symbol.detail,
                SymbolDetail::Node {
                    port_index: None,
                    global: false
                }
            )
        })
        .filter(|(id, _)| counts.get(id) == Some(&1))
        .filter_map(|(id, symbol)| {
            let connection = analysis.references.iter().find(|reference| {
                reference.target == Some(id)
                    && matches!(
                        reference.context,
                        RefContext::Terminal { .. } | RefContext::ControlNode { .. }
                    )
            })?;
            Some(Diagnostic::new(
                connection.name.range,
                Severity::Warning,
                "floating-node",
                format!(
                    "Node `{}` is connected to only one terminal",
                    symbol.name.text
                ),
            ))
        })
        .collect()
}

/// Warns when a top-level netlist has elements but no terminal on a ground node.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn no_ground(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let live: Vec<_> = analysis
        .elements
        .iter()
        .filter(|info| !info.skipped)
        .collect();
    let has_ground = live.iter().any(|info| {
        info.roles
            .nodes
            .iter()
            .chain(&info.roles.control_nodes)
            .any(|node| is_ground(&node.text))
    });
    let first = live
        .iter()
        .find(|info| info.scope == crate::analysis::symbols::ROOT_SCOPE);
    match first {
        Some(info) if syntax.is_top_level && !has_ground => vec![Diagnostic::new(
            info.name.range,
            Severity::Warning,
            "no-ground",
            "No element is connected to ground (`0`, `gnd`, `gnd!`, `ground` or `!gnd`)",
        )],
        _ => Vec::new(),
    }
}
