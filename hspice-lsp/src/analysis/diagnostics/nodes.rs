//! Node rules: floating nodes, missing ground connection and undefined output-variable names.

use std::collections::HashSet;

use super::common::{inside_if, live_commands};
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::hierarchy::{PathLookup, lookup_path};
use crate::analysis::symbols::{
    ROOT_SCOPE, RefContext, Reference, ScopeId, SymbolDetail, SymbolKind,
};
use crate::lang::commands::find_command;
use crate::lang::elements::is_ground;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{Arg, AssignKey, Assignment, CommandKind, Value};

/// Returns the scopes whose nodes and elements may be incomplete.
///
/// Elements of included files are not analysed, so these scopes are skipped: scopes with an
/// unresolved include, subckt bodies with any include (PEX `.pxi` files), scopes with encrypted or
/// `.variation` statements and the top level of include files (connected by the including netlist).
///
/// # Arguments
/// * `syntax` - parse results (for the top-level flag)
/// * `analysis` - semantic analysis
fn unchecked_scopes(syntax: &SyntaxInfo, analysis: &Analysis) -> HashSet<ScopeId> {
    let structure = &analysis.structure;
    let include_scopes = analysis.includes.iter().filter_map(|include| {
        let scope = structure.scope_of(include.directive.statement);
        (include.error.is_some() || scope != ROOT_SCOPE).then_some(scope)
    });
    let opaque_scopes = structure
        .opaque
        .iter()
        .zip(&structure.statement_scope)
        .filter(|(opaque, _)| **opaque)
        .map(|(_, scope)| *scope);
    include_scopes
        .chain(opaque_scopes)
        .chain((!syntax.is_top_level).then_some(ROOT_SCOPE))
        .collect()
}

/// Warns about nodes connected to exactly one element terminal that are neither ports nor global.
///
/// # Arguments
/// * `syntax` - parse results (for the top-level flag)
/// * `analysis` - semantic analysis
pub fn floating_nodes(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let counts = analysis.connection_counts();
    let unchecked_scopes = unchecked_scopes(syntax, analysis);
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

/// Warns about nodes and elements named in output variables (`v(out)`, `i(r1)`, `v(x1.out)`) that do not exist.
///
/// # Arguments
/// * `syntax` - parse results (for the top-level flag)
/// * `analysis` - semantic analysis
pub fn undefined_output_targets(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let unchecked = unchecked_scopes(syntax, analysis);
    analysis
        .references
        .iter()
        .filter(|reference| reference.context == RefContext::Output && reference.target.is_none())
        .filter(|reference| matches!(reference.kind, SymbolKind::Node | SymbolKind::Element))
        .filter(|reference| !named_in_other_files(analysis, &reference.name.key))
        .filter(|reference| is_missing(analysis, reference, &unchecked))
        .map(|reference| {
            let (code, label) = match reference.kind {
                SymbolKind::Node => ("undefined-node", "Node"),
                _ => ("undefined-element", "Element"),
            };
            Diagnostic::new(
                reference.name.range,
                Severity::Warning,
                code,
                format!("{label} `{}` is not defined", reference.name.text),
            )
        })
        .collect()
}

/// Returns true if any segment of a (possibly dotted) name appears in an included file.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `key` - lowercase name
fn named_in_other_files(analysis: &Analysis, key: &str) -> bool {
    std::iter::once(key).chain(key.split('.')).any(|word| {
        analysis
            .external_words
            .iter()
            .any(|words| words.contains(word))
    })
}

/// Returns true if an unresolved output reference names something that is known not to exist.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `reference` - unresolved output reference
/// * `unchecked` - scopes whose contents may be incomplete
fn is_missing(analysis: &Analysis, reference: &Reference, unchecked: &HashSet<ScopeId>) -> bool {
    let key = &reference.name.key;
    if key.contains('.') {
        lookup_path(analysis, reference.scope, key, reference.kind, unchecked)
            == PathLookup::Missing
    } else {
        !unchecked.contains(&reference.scope)
    }
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

/// Warns about nodes initialised more than once by `.ic` or by `.nodeset`; HSPICE keeps the last value.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn duplicate_initial_conditions(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let structure = &analysis.structure;
    let mut seen = HashSet::new();
    live_commands(syntax, analysis)
        .filter(|(index, _, _)| !inside_if(analysis, *index))
        .filter_map(|(index, command, _)| {
            let name = find_command(&command.keyword.key)?.name;
            let CommandKind::Generic { args } = &command.kind else {
                return None;
            };
            let alter = structure.statement_alter.get(index).copied();
            matches!(name, ".ic" | ".nodeset").then_some((
                name,
                structure.scope_of(index),
                alter,
                args,
            ))
        })
        .flat_map(|(name, scope, alter, args)| {
            args.iter().filter_map(move |arg| match arg {
                Arg::Assign(Assignment {
                    key: AssignKey::Call(call),
                    ..
                }) if call.name.key == "v" => {
                    let nodes: Vec<String> = call
                        .args
                        .iter()
                        .filter_map(|node| match node {
                            Arg::Value(Value::Word(word)) => Some(word.key.clone()),
                            Arg::Value(Value::Number(number)) => Some(number.text.clone()),
                            _ => None,
                        })
                        .collect();
                    Some(((name, scope, alter, nodes), call))
                }
                _ => None,
            })
        })
        .filter(|(key, _)| !seen.insert(key.clone()))
        .map(|((name, _, _, _), call)| {
            Diagnostic::new(
                call.range,
                Severity::Warning,
                "duplicate-ic",
                format!("This node already has an `{name}` value; HSPICE keeps the last one"),
            )
        })
        .collect()
}
