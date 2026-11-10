//! Transient source function rules of V and I elements: argument count, negative times, PWL time order.

use super::common::live_elements;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::eval::evaluate;
use crate::analysis::roles::{SourceCall, source_calls};
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{Arg, TextRange, Value};
use crate::syntax::number::parse_number;

/// Runs the source function rules.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_elements(syntax, analysis)
        .filter(|(index, _)| {
            analysis
                .element_at_statement(*index)
                .and_then(|info| info.spec)
                .is_some_and(|spec| matches!(spec.letter, 'V' | 'I'))
        })
        .flat_map(|(_, element)| source_calls(element))
        .flat_map(|call| {
            too_few_arguments(&call)
                .into_iter()
                .chain(negative_times(&call))
                .chain(decreasing_times(&call))
        })
        .collect()
}

/// Returns the leading positional values with their literal value, if constant.
///
/// # Arguments
/// * `call` - source function
fn literals(call: &SourceCall) -> Vec<(TextRange, Option<f64>)> {
    call.args
        .iter()
        .map_while(|arg| match arg {
            Arg::Value(Value::Number(number)) => Some((
                number.range,
                parse_number(&number.text).map(|parsed| parsed.value),
            )),
            Arg::Value(Value::Expr(expr)) => Some((expr.range, evaluate(expr, &|_| None))),
            Arg::Value(Value::Word(word)) if call.parenthesised => Some((word.range, None)),
            _ => None,
        })
        .collect()
}

/// Warns about a parenthesised source function with fewer arguments than required.
///
/// # Arguments
/// * `call` - source function
fn too_few_arguments(call: &SourceCall) -> Option<Diagnostic> {
    let found = call.args.len();
    (call.parenthesised && found < call.function.required).then(|| {
        Diagnostic::new(
            call.name.range,
            Severity::Warning,
            "source-args",
            format!(
                "`{}` needs at least {} arguments, found {found}",
                call.function.signature(),
                call.function.required
            ),
        )
    })
}

/// Warns about negative literal times and frequencies, which HSPICE resets to a default.
///
/// # Arguments
/// * `call` - source function
fn negative_times(call: &SourceCall) -> Vec<Diagnostic> {
    let checked: &[usize] = match call.function.name {
        "pulse" => &[3, 4],
        "sin" => &[2, 4],
        "exp" => &[2, 3, 4, 5],
        _ => &[],
    };
    literals(call)
        .into_iter()
        .zip(call.function.arguments)
        .enumerate()
        .filter(|(index, ((_, value), _))| {
            checked.contains(index) && value.is_some_and(|number| number < 0.0)
        })
        .map(|(_, ((range, _), argument))| {
            Diagnostic::new(
                range,
                Severity::Warning,
                "source-args",
                format!(
                    "`{}` of `{}` cannot be negative",
                    argument.name,
                    call.function.name.to_ascii_uppercase()
                ),
            )
        })
        .collect()
}

/// Reports literal PWL time points that are smaller than their predecessor.
///
/// # Arguments
/// * `call` - source function
fn decreasing_times(call: &SourceCall) -> Vec<Diagnostic> {
    let first_time = match call.function.name {
        "pwl" | "pwlz" => 0,
        "pl" => 1,
        _ => return Vec::new(),
    };
    let times: Vec<(TextRange, Option<f64>)> = literals(call)
        .into_iter()
        .skip(first_time)
        .step_by(2)
        .collect();
    times
        .windows(2)
        .filter_map(|pair| match pair {
            [(_, Some(earlier)), (range, Some(later))] if later < earlier => Some(*range),
            _ => None,
        })
        .map(|range| {
            Diagnostic::new(
                range,
                Severity::Error,
                "source-args",
                "Time points of a piecewise linear source must increase",
            )
        })
        .collect()
}
