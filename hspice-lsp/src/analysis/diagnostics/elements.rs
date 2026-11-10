//! Element rules: unknown letters, missing nodes, duplicate names, shorted terminals, missing values, multipliers.

use super::common::{live_elements, repeated_definitions};
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::eval::evaluate;
use crate::analysis::resolve::ElementInfo;
use crate::analysis::roles::assignments;
use crate::analysis::symbols::SymbolKind;
use crate::lang::elements::is_ground;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{AssignKey, Element, Value};

/// Runs all element rules.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let connection_problems = live_elements(syntax, analysis)
        .filter_map(|(index, element)| {
            analysis
                .element_at_statement(index)
                .map(|info| (info, element))
        })
        .flat_map(|(info, element)| {
            shorted(info)
                .into_iter()
                .chain(missing_value(info, element))
                .chain(bad_multiplier(info, element))
        });
    analysis
        .elements
        .iter()
        .filter(|info| !info.skipped)
        .filter_map(element_problem)
        .chain(duplicate_elements(analysis))
        .chain(connection_problems)
        .collect()
}

/// Warns about a two-terminal element whose terminals are the same node (HSPICE ignores it).
///
/// # Arguments
/// * `info` - element with role assignment
fn shorted(info: &ElementInfo) -> Option<Diagnostic> {
    let two_terminal = info.spec.is_some_and(|spec| "RCLVID".contains(spec.letter));
    let [first, second, ..] = info.roles.nodes.as_slice() else {
        return None;
    };
    let same = first.key == second.key || (is_ground(&first.key) && is_ground(&second.key));
    (two_terminal && same).then(|| {
        Diagnostic::new(
            info.name.range,
            Severity::Warning,
            "shorted-element",
            format!(
                "Both terminals of `{}` are connected to node `{}`",
                info.name.text, first.text
            ),
        )
    })
}

/// Warns about an R, C or L without value, model or parameters; HSPICE substitutes a default.
///
/// # Arguments
/// * `info` - element with role assignment
/// * `element` - the element line
fn missing_value(info: &ElementInfo, element: &Element) -> Option<Diagnostic> {
    let fallback = match info.spec?.letter {
        'R' => "RESMIN",
        'C' => "0",
        'L' => "1e-12",
        _ => return None,
    };
    let roles = &info.roles;
    let bare = roles.nodes.len() >= 2
        && roles.values.is_empty()
        && roles.reference.is_none()
        && assignments(element).next().is_none();
    bare.then(|| {
        Diagnostic::new(
            info.name.range,
            Severity::Warning,
            "missing-value",
            format!("`{}` has no value; HSPICE uses {fallback}", info.name.text),
        )
    })
}

/// Reports a literal multiplier `M=` of zero or less (voltage sources have no multiplier).
///
/// # Arguments
/// * `info` - element with role assignment
/// * `element` - the element line
fn bad_multiplier(info: &ElementInfo, element: &Element) -> Option<Diagnostic> {
    let has_multiplier = info.spec.is_some_and(|spec| spec.letter != 'V');
    assignments(element)
        .filter(|_| has_multiplier)
        .find_map(|assignment| match (&assignment.key, &assignment.value) {
            (AssignKey::Name(key), Some(Value::Expr(expr))) if key.key == "m" => {
                evaluate(expr, &|_| None)
                    .filter(|value| *value <= 0.0)
                    .map(|_| expr.range)
            }
            _ => None,
        })
        .map(|range| {
            Diagnostic::new(
                range,
                Severity::Error,
                "bad-multiplier",
                format!(
                    "Multiplier `M` of `{}` must be greater than 0",
                    info.name.text
                ),
            )
        })
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
