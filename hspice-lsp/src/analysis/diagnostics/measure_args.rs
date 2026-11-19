//! `.measure` argument rules: constant values where an output variable is measured, `FROM` without `TO`.

use super::common::{is_measure, live_commands};
use super::walk::flatten_expression;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::lang::measure::find_measure_keyword;
use crate::lang::output_vars::find_output_var;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{
    Arg, AssignKey, Assignment, CommandKind, Expr, ExprKind, Ident, TextRange, Value,
};

/// `.measure` functions whose next argument is the measured output variable.
const SIGNAL_FUNCTIONS: &[&str] = &[
    "trig", "targ", "find", "avg", "rms", "min", "max", "pp", "em_avg", "integ", "deriv",
];

/// Runs the `.measure` argument rules.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_commands(syntax, analysis)
        .filter(|(_, command, _)| is_measure(command))
        .filter_map(|(_, command, _)| match &command.kind {
            CommandKind::Generic { args } => Some(args),
            _ => None,
        })
        .flat_map(|args| {
            scalar_subjects(args)
                .into_iter()
                .chain(from_without_to(args))
        })
        .collect()
}

/// Warns about measure functions such as `INTEG 5` or `AVG vdd` that measure a constant value.
///
/// # Arguments
/// * `args` - `.measure` arguments
fn scalar_subjects(args: &[Arg]) -> Vec<Diagnostic> {
    args.windows(2)
        .filter_map(|pair| match pair {
            [Arg::Value(Value::Word(function)), Arg::Value(subject)] => {
                find_measure_keyword(&function.key)
                    .filter(|spec| SIGNAL_FUNCTIONS.contains(&spec.name))
                    .and_then(|_| scalar_range(subject))
                    .map(|range| (function, range))
            }
            _ => None,
        })
        .map(|(function, range)| {
            Diagnostic::new(
                range,
                Severity::Warning,
                "measure-scalar-subject",
                format!(
                    "`{}` expects an output variable such as `v(node)` or `i(element)`, not a constant value",
                    function.text.to_ascii_uppercase()
                ),
            )
        })
        .collect()
}

/// Returns the range of a value that is a constant: a number, a parameter word or an expression without output variables.
///
/// # Arguments
/// * `value` - measured value
fn scalar_range(value: &Value) -> Option<TextRange> {
    match value {
        Value::Number(number) => Some(number.range),
        Value::Word(word)
            if find_measure_keyword(&word.key).is_none()
                && find_output_var(&word.key).is_none() =>
        {
            Some(word.range)
        }
        Value::Expr(expr) if is_constant(expr) => Some(expr.range),
        _ => None,
    }
}

/// Returns true if an expression calls no output variable and parsed without errors.
///
/// # Arguments
/// * `expr` - expression
fn is_constant(expr: &Expr) -> bool {
    flatten_expression(expr)
        .iter()
        .all(|part| match &part.kind {
            ExprKind::Call { name, .. } => find_output_var(&name.key).is_none(),
            ExprKind::Error => false,
            _ => true,
        })
}

/// Warns about a `FROM` window start without a `TO` window end.
///
/// # Arguments
/// * `args` - `.measure` arguments
fn from_without_to(args: &[Arg]) -> Option<Diagnostic> {
    let keys: Vec<&Ident> = args
        .iter()
        .filter_map(|arg| match arg {
            Arg::Assign(Assignment {
                key: AssignKey::Name(key),
                ..
            })
            | Arg::Value(Value::Word(key)) => Some(key),
            _ => None,
        })
        .collect();
    let from = keys.iter().find(|key| key.key == "from")?;
    (!keys.iter().any(|key| key.key == "to")).then(|| {
        Diagnostic::new(
            from.range,
            Severity::Warning,
            "measure-from-without-to",
            "`FROM` is given without `TO`; set `TO` to close the measurement window",
        )
    })
}
