//! Number rules: exponent combined with scale factor, and `M` used where `MEG` is meant.

use super::common::{live_elements, live_statements};
use super::walk::{numeric_literals, values_of};
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::resolve::ElementInfo;
use crate::analysis::roles::assignments;
use crate::config::DiagnosticSettings;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{
    AssignKey, CommandKind, Element, ExprKind, Statement, StatementKind, TextRange, Value,
};
use crate::syntax::number::parse_number;

/// Runs the number rules.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
/// * `settings` - diagnostic toggles
pub fn check(
    syntax: &SyntaxInfo,
    analysis: &Analysis,
    settings: &DiagnosticSettings,
) -> Vec<Diagnostic> {
    let bad = bad_numbers(syntax, analysis);
    let megs = settings
        .meg_suffix
        .then(|| meg_suffixes(syntax, analysis))
        .into_iter()
        .flatten();
    bad.into_iter().chain(megs).collect()
}

/// Reports literals that combine an exponent with a scale factor, such as `1e-6u`.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn bad_numbers(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_statements(syntax, analysis)
        .flat_map(|(_, statement)| numeric_literals(statement))
        .filter(|(text, _)| {
            parse_number(text).is_some_and(|number| number.has_exponent && number.scale.is_some())
        })
        .map(|(text, range)| {
            Diagnostic::new(
                range,
                Severity::Error,
                "bad-number",
                format!("`{text}` combines an exponent with a scale factor; use one or the other"),
            )
        })
        .collect()
}

/// Reports resistor values and `.ac` frequencies written with `M` (milli) that look like mega.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn meg_suffixes(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let resistors = live_elements(syntax, analysis)
        .filter_map(|(index, element)| {
            analysis
                .element_at_statement(index)
                .map(|info| (info, element))
        })
        .filter(|(info, _)| info.spec.is_some_and(|spec| spec.letter == 'R'))
        .flat_map(|(info, element)| resistor_value_literals(info, element));
    let frequencies =
        live_statements(syntax, analysis).flat_map(|(_, statement)| match &statement.kind {
            StatementKind::Command(command)
                if matches!(&command.kind, CommandKind::Generic { .. })
                    && command.keyword.key == ".ac" =>
            {
                ac_frequency_literals(statement)
            }
            _ => Vec::new(),
        });
    resistors
        .chain(frequencies)
        .filter_map(|(text, range)| meg_hint(text, range))
        .collect()
}

/// Returns the literals that give the resistance: the first positional value after the nodes and `R=`.
///
/// # Arguments
/// * `info` - role assignment of the `R` element
/// * `element` - the element line
fn resistor_value_literals<'a>(
    info: &'a ElementInfo,
    element: &'a Element,
) -> Vec<(&'a str, TextRange)> {
    let positional = info.roles.values.first().and_then(|value| match value {
        Value::Number(number) => Some((number.text.as_str(), number.range)),
        _ => None,
    });
    let keyed =
        assignments(element).filter_map(|assignment| match (&assignment.key, &assignment.value) {
            (AssignKey::Name(key), Some(Value::Expr(expr))) if key.key == "r" => match &expr.kind {
                ExprKind::Number(text) => Some((text.as_str(), expr.range)),
                _ => None,
            },
            _ => None,
        });
    positional.into_iter().chain(keyed).collect()
}

/// Returns the frequency literals of a `.ac` command (all numbers after the point count).
///
/// # Arguments
/// * `statement` - the `.ac` statement
fn ac_frequency_literals(statement: &Statement) -> Vec<(&str, TextRange)> {
    let args = super::walk::statement_args(statement);
    values_of(args)
        .into_iter()
        .filter_map(|value| match value {
            Value::Number(number) => Some((number.text.as_str(), number.range)),
            _ => None,
        })
        .skip(1)
        .collect()
}

/// Builds the hint for a literal whose scale factor is exactly `m` and whose mantissa is at least 1.
///
/// # Arguments
/// * `text` - literal as written
/// * `range` - literal location
fn meg_hint(text: &str, range: TextRange) -> Option<Diagnostic> {
    let number = parse_number(text)?;
    let milli = number.scale.is_some_and(|scale| scale.suffix == "m") && !number.has_exponent;
    let mantissa = number.value.abs() / 1e-3;
    (milli && mantissa >= 1.0).then(|| {
        let split = text
            .find(|character: char| character.is_ascii_alphabetic())
            .unwrap_or(text.len());
        Diagnostic::new(
            range,
            Severity::Hint,
            "meg-suffix",
            format!(
                "`{text}` is {} milli ({:e}); use `{}MEG` for {}e6",
                &text[..split],
                number.value,
                &text[..split],
                &text[..split]
            ),
        )
    })
}
