//! Lookup of hierarchical names such as `x1.x2.out` through local subcircuit definitions.

use std::collections::HashSet;

use super::Analysis;
use super::symbols::{ROOT_SCOPE, RefContext, ScopeId, SymbolDetail, SymbolId, SymbolKind};

/// Outcome of looking up a hierarchical name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathLookup {
    /// Every segment exists.
    Found,
    /// A segment is not defined where it should be.
    Missing,
    /// The path leads into a subcircuit whose contents are not known (external, unresolved, unchecked).
    Unknown,
}

/// Looks up a hierarchical name: instance segments, then a node or element in the last subcircuit.
///
/// # Arguments
/// * `analysis` - document analysis
/// * `scope` - scope of the statement that uses the name
/// * `path` - lowercase dotted name, e.g. `x1.x2.out`
/// * `kind` - kind of the last segment (`Node` or `Element`)
/// * `unchecked` - scopes whose contents may be incomplete
pub fn lookup_path(
    analysis: &Analysis,
    scope: ScopeId,
    path: &str,
    kind: SymbolKind,
    unchecked: &HashSet<ScopeId>,
) -> PathLookup {
    let segments: Vec<&str> = path.split('.').collect();
    let Some((last, instances)) = segments.split_last() else {
        return PathLookup::Unknown;
    };
    let body = instances
        .iter()
        .enumerate()
        .try_fold(scope, |current, (depth, instance)| {
            if unchecked.contains(&current) {
                return Err(PathLookup::Unknown);
            }
            let element = if depth == 0 {
                analysis.lookup(SymbolKind::Element, current, instance)
            } else {
                analysis
                    .table
                    .in_scope(SymbolKind::Element, current, instance)
                    .first()
                    .copied()
            };
            let element = element.ok_or(PathLookup::Missing)?;
            instance_body(analysis, element).ok_or(PathLookup::Unknown)
        });
    match body {
        Err(outcome) => outcome,
        Ok(body) if unchecked.contains(&body) => PathLookup::Unknown,
        Ok(body) if is_defined_in(analysis, kind, body, last) => PathLookup::Found,
        Ok(_) => PathLookup::Missing,
    }
}

/// Returns the body scope of the local subcircuit an X instance refers to.
///
/// # Arguments
/// * `analysis` - document analysis
/// * `element` - element symbol of the instance
fn instance_body(analysis: &Analysis, element: SymbolId) -> Option<ScopeId> {
    let SymbolDetail::Element { element: index } = analysis.symbols.get(element)?.detail else {
        return None;
    };
    let subckt = analysis
        .references
        .iter()
        .find(|reference| reference.context == RefContext::SubcktSlot { element: index })?
        .target?;
    match analysis.symbols.get(subckt)?.detail {
        SymbolDetail::Subckt { body, .. } => body,
        _ => None,
    }
}

/// Returns true if a node or element is defined directly in a scope (nodes also as `.global`).
///
/// # Arguments
/// * `analysis` - document analysis
/// * `kind` - `Node` or `Element`
/// * `scope` - subcircuit body scope
/// * `key` - lowercase name
fn is_defined_in(analysis: &Analysis, kind: SymbolKind, scope: ScopeId, key: &str) -> bool {
    let global = || {
        analysis
            .table
            .in_scope(SymbolKind::Node, ROOT_SCOPE, key)
            .iter()
            .any(|id| {
                matches!(
                    analysis.symbols[*id].detail,
                    SymbolDetail::Node { global: true, .. }
                )
            })
    };
    !analysis.table.in_scope(kind, scope, key).is_empty() || (kind == SymbolKind::Node && global())
}
