//! Reference rules: undefined parameters, functions, subcircuits, models, sources, inductors; model types; ports.

use super::common::{lenient, live_elements};
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::resolve::ElementInfo;
use crate::analysis::roles::assignments;
use crate::analysis::symbols::{RefContext, Reference, Symbol, SymbolDetail, SymbolKind};
use crate::lang::elements::ReferenceKind;
use crate::lang::models::find_model_type;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{AssignKey, StatementKind};

/// Runs all reference rules.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    analysis
        .references
        .iter()
        .filter_map(|reference| check_reference(syntax, analysis, reference))
        .chain(port_counts(analysis))
        .chain(unknown_instance_params(syntax, analysis))
        .collect()
}

/// Builds the diagnostic of one reference, if it is unresolved or has the wrong model type.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
/// * `reference` - the reference
fn check_reference(
    syntax: &SyntaxInfo,
    analysis: &Analysis,
    reference: &Reference,
) -> Option<Diagnostic> {
    let name = &reference.name;
    let range = name.range;
    let Some(target) = reference.target else {
        return check_unresolved(syntax, analysis, reference);
    };
    match reference.context {
        RefContext::ModelSlot { element } => {
            model_type_mismatch(analysis, reference, element, analysis.symbols.get(target)?)
        }
        _ => None,
    }
    .map(|message| Diagnostic::new(range, Severity::Error, "model-type-mismatch", message))
}

/// Builds the diagnostic of an unresolved reference.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
/// * `reference` - unresolved reference
fn check_unresolved(
    syntax: &SyntaxInfo,
    analysis: &Analysis,
    reference: &Reference,
) -> Option<Diagnostic> {
    let name = &reference.name;
    let range = name.range;
    let text = &name.text;
    match (reference.context, reference.kind) {
        (RefContext::Expression | RefContext::ValueWord | RefContext::Sweep, SymbolKind::Param) => {
            (!resolves_as_other_kind(analysis, reference)).then(|| {
                lenient(
                    analysis,
                    range,
                    "undefined-param",
                    format!("Parameter `{text}` is not defined"),
                )
            })
        }
        (RefContext::Expression, SymbolKind::Function) => Some(lenient(
            analysis,
            range,
            "undefined-function",
            format!("Function `{text}` is not defined"),
        )),
        (RefContext::SubcktSlot { .. }, _) => Some(lenient(
            analysis,
            range,
            "undefined-subckt",
            format!("Subcircuit `{text}` is not defined"),
        )),
        (RefContext::ModelSlot { .. }, _) => Some(lenient(
            analysis,
            range,
            "undefined-model",
            format!("Model `{text}` is not defined"),
        )),
        (RefContext::ControlSource { .. }, _) => Some(Diagnostic::new(
            range,
            Severity::Error,
            "undefined-source",
            format!("Controlling source `{text}` is not defined"),
        )),
        (RefContext::CoupledInductor { .. }, _) => Some(Diagnostic::new(
            range,
            Severity::Error,
            "undefined-inductor",
            format!("Coupled inductor `{text}` is not defined"),
        )),
        (RefContext::Sweep, SymbolKind::Element) => Some(Diagnostic::new(
            range,
            Severity::Error,
            "undefined-source",
            sweep_message(syntax, reference),
        )),
        _ => None,
    }
}

/// Returns true if a parameter-like name is defined as an element, data block, measure, model or subcircuit.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `reference` - unresolved parameter reference
fn resolves_as_other_kind(analysis: &Analysis, reference: &Reference) -> bool {
    [
        SymbolKind::Element,
        SymbolKind::DataBlock,
        SymbolKind::Measure,
        SymbolKind::Model,
        SymbolKind::Subckt,
    ]
    .into_iter()
    .any(|kind| {
        analysis
            .lookup(kind, reference.scope, &reference.name.key)
            .is_some()
    })
}

/// Words the message of an unresolved sweep or analysis source after the command it belongs to.
///
/// # Arguments
/// * `syntax` - parse results
/// * `reference` - unresolved sweep reference
fn sweep_message(syntax: &SyntaxInfo, reference: &Reference) -> String {
    let keyword = match syntax
        .netlist
        .statements
        .get(reference.statement)
        .map(|statement| &statement.kind)
    {
        Some(StatementKind::Command(command)) => command.keyword.key.as_str(),
        _ => "",
    };
    let text = &reference.name.text;
    if keyword == ".dc" {
        format!("`{text}` is neither a source, a parameter nor `temp`")
    } else {
        format!("Source `{text}` is not defined")
    }
}

/// Describes a model whose type does not fit the element that uses it.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `reference` - the model reference
/// * `element` - index of the using element
/// * `model` - the resolved symbol
fn model_type_mismatch(
    analysis: &Analysis,
    reference: &Reference,
    element: usize,
    model: &Symbol,
) -> Option<String> {
    let info = analysis.elements.get(element)?;
    let SymbolDetail::Model {
        model_type: Some(model_type),
        ..
    } = &model.detail
    else {
        return None;
    };
    let spec = info.spec?;
    let (ReferenceKind::Model(allowed) | ReferenceKind::OptionalModel(allowed)) = spec.reference
    else {
        return None;
    };
    let type_spec = find_model_type(&model_type.key)?;
    let accepted = allowed.contains(&model_type.key.as_str())
        || type_spec.element_letters.contains(&spec.letter);
    (!accepted).then(|| {
        format!(
            "Model `{}` has type `{}`, but {} `{}` needs {}",
            reference.name.text,
            model_type.text,
            spec.name.to_ascii_lowercase(),
            info.name.text,
            allowed.join(" or ")
        )
    })
}

/// Returns the subcircuit symbol an X element instantiates, with its ports and the element info.
///
/// # Arguments
/// * `analysis` - semantic analysis
fn instantiations(analysis: &Analysis) -> impl Iterator<Item = (&ElementInfo, &Symbol)> {
    analysis.references.iter().filter_map(|reference| {
        let RefContext::SubcktSlot { element } = reference.context else {
            return None;
        };
        let symbol = analysis.symbols.get(reference.target?)?;
        Some((analysis.elements.get(element)?, symbol))
    })
}

/// Reports X instances whose node count differs from the port count of the subcircuit.
///
/// # Arguments
/// * `analysis` - semantic analysis
fn port_counts(analysis: &Analysis) -> Vec<Diagnostic> {
    instantiations(analysis)
        .filter(|(info, _)| !info.skipped)
        .filter_map(|(info, symbol)| {
            let SymbolDetail::Subckt { ports, .. } = &symbol.detail else {
                return None;
            };
            let got = info.roles.nodes.len();
            (got != ports.len() && got > 0).then(|| {
                let names: Vec<&str> = ports.iter().map(|port| port.text.as_str()).collect();
                let plural = if ports.len() == 1 { "port" } else { "ports" };
                Diagnostic::new(
                    info.name.range,
                    Severity::Error,
                    "port-count",
                    format!(
                        "`{}` expects {} {plural} ({}), got {got}",
                        symbol.name.text,
                        ports.len(),
                        names.join(" ")
                    ),
                )
            })
        })
        .collect()
}

/// Warns about `key=` overrides on X instances that the subcircuit does not declare.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn unknown_instance_params(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let elements: Vec<_> = live_elements(syntax, analysis).collect();
    instantiations(analysis)
        .filter(|(info, _)| !info.skipped)
        .flat_map(|(info, symbol)| {
            let element = elements
                .binary_search_by_key(&info.statement, |(index, _)| *index)
                .ok()
                .map(|position| elements[position].1);
            element.into_iter().flat_map(move |element| {
                assignments(element).filter_map(move |assignment| {
                    let AssignKey::Name(key) = &assignment.key else {
                        return None;
                    };
                    (!accepts_instance_param(analysis, symbol, &key.key)).then(|| {
                        Diagnostic::new(
                            key.range,
                            Severity::Warning,
                            "unknown-instance-param",
                            format!(
                                "`{}` is not a parameter of subcircuit `{}`{}",
                                key.text,
                                symbol.name.text,
                                declared_list(symbol)
                            ),
                        )
                    })
                })
            })
        })
        .collect()
}

/// Returns true if an instance may set `key` on the subcircuit.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `subckt` - subcircuit symbol
/// * `key` - lowercase parameter name
fn accepts_instance_param(analysis: &Analysis, subckt: &Symbol, key: &str) -> bool {
    let SymbolDetail::Subckt { params, body, .. } = &subckt.detail else {
        return true;
    };
    matches!(key, "m" | "s")
        || params.iter().any(|param| param.name.key == key)
        || body.is_some_and(|body| {
            !analysis
                .table
                .in_scope(SymbolKind::Param, body, key)
                .is_empty()
        })
}

/// Formats the declared parameters of a subcircuit for a message suffix.
///
/// # Arguments
/// * `subckt` - subcircuit symbol
fn declared_list(subckt: &Symbol) -> String {
    match &subckt.detail {
        SymbolDetail::Subckt { params, .. } if !params.is_empty() => {
            let names: Vec<&str> = params
                .iter()
                .map(|param| param.name.text.as_str())
                .collect();
            format!(" (declared: {})", names.join(", "))
        }
        _ => " (it declares none)".to_string(),
    }
}
