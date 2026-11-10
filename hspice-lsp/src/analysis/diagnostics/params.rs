//! Parameter rules: reserved names, duplicates, unused parameters, unquoted expressions, output signals
//! in parameters and `.measure` keywords used as parameters.

use std::collections::HashSet;

use super::common::{live_commands, repeated_definitions};
use super::walk::flatten_expression;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::symbols::{ParamSource, ScopeId, SymbolDetail, SymbolKind};
use crate::config::DiagnosticSettings;
use crate::lang::commands::find_command;
use crate::lang::functions::{find_function, find_special_variable};
use crate::lang::measure::find_measure_keyword;
use crate::lang::output_vars::{OutputArgKind, output_argument};
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{
    Arg, AssignKey, Assignment, Command, CommandKind, Expr, ExprKind, Ident, ParamDef, Value,
};
use crate::syntax::number::parse_number;

/// Runs all parameter rules except the text-based quoting rule.
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
    reserved_params(syntax, analysis)
        .into_iter()
        .chain(duplicate_params(analysis))
        .chain(output_signal_params(syntax, analysis))
        .chain(measure_keyword_params(syntax, analysis))
        .chain(
            settings
                .unused_params
                .then(|| unused_params(syntax, analysis))
                .into_iter()
                .flatten(),
        )
        .collect()
}

/// Returns true for `.measure` / `.meas`.
///
/// # Arguments
/// * `command` - dot command
fn is_measure(command: &Command) -> bool {
    find_command(&command.keyword.key).is_some_and(|spec| spec.name == ".measure")
}

/// Returns the name of the first node or element output variable called in an expression.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `scope` - scope of the statement (user functions may shadow output variables)
/// * `expr` - expression to search
fn output_signal<'a>(analysis: &Analysis, scope: ScopeId, expr: &'a Expr) -> Option<&'a Ident> {
    flatten_expression(expr)
        .into_iter()
        .find_map(|part| match &part.kind {
            ExprKind::Call { name, .. } => Some(name),
            _ => None,
        })
        .filter(|name| {
            matches!(
                output_argument(&name.key),
                Some(OutputArgKind::Nodes | OutputArgKind::Element)
            ) && find_function(&name.key).is_none()
                && analysis
                    .lookup(SymbolKind::Function, scope, &name.key)
                    .is_none()
        })
}

/// Warns about `.param` values and `.measure ... PARAM=` equations that contain `v()` or `i()`;
/// HSPICE reports that such results may be incorrect.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn output_signal_params(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_commands(syntax, analysis)
        .flat_map(|(index, command, _)| {
            let subjects: Vec<(String, &Expr)> = match &command.kind {
                CommandKind::Param { definitions } => definitions
                    .iter()
                    .filter(|definition| definition.arguments.is_empty())
                    .filter_map(|definition| {
                        let subject = format!("Parameter `{}`", definition.name.text);
                        definition.value.as_ref().map(|value| (subject, value))
                    })
                    .collect(),
                CommandKind::Generic { args } if is_measure(command) => args
                    .iter()
                    .filter_map(|arg| match arg {
                        Arg::Assign(Assignment {
                            key: AssignKey::Name(key),
                            value: Some(Value::Expr(expr)),
                            ..
                        }) if key.key == "param" => {
                            Some(("The `.measure` equation".to_string(), expr))
                        }
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let scope = analysis.structure.scope_of(index);
            subjects.into_iter().filter_map(move |(subject, expr)| {
                output_signal(analysis, scope, expr).map(|call| {
                    Diagnostic::new(
                        call.range,
                        Severity::Warning,
                        "output-signal-param",
                        format!(
                            "{subject} depends on the output signal `{}()`; HSPICE warns that the result may be incorrect",
                            call.text
                        ),
                    )
                })
            })
        })
        .collect()
}

/// Returns the `.measure` keywords written where a value is expected: `to=td` or `TO td`.
///
/// # Arguments
/// * `args` - `.measure` arguments
fn keywords_as_values(args: &[Arg]) -> Vec<&Ident> {
    let assigned = args.iter().filter_map(|arg| match arg {
        Arg::Assign(Assignment {
            value:
                Some(Value::Expr(Expr {
                    kind: ExprKind::Ident(word),
                    ..
                })),
            ..
        }) => Some(word).filter(|word| !matches!(word.key.as_str(), "last" | "trig")),
        _ => None,
    });
    let positional = args.windows(2).filter_map(|pair| match pair {
        [Arg::Value(Value::Word(key)), Arg::Value(Value::Word(word))] => {
            find_measure_keyword(&key.key)
                .filter(|spec| spec.takes_value)
                .map(|_| word)
        }
        _ => None,
    });
    assigned
        .chain(positional)
        .filter(|word| find_measure_keyword(&word.key).is_some())
        .collect()
}

/// Warns when a parameter named like a `.measure` keyword is used as a value in `.measure`;
/// the strict parser reads the keyword, not the parameter.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn measure_keyword_params(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_commands(syntax, analysis)
        .filter(|(_, command, _)| is_measure(command))
        .flat_map(|(index, command, _)| {
            let scope = analysis.structure.scope_of(index);
            let args: &[Arg] = match &command.kind {
                CommandKind::Generic { args } => args,
                _ => &[],
            };
            keywords_as_values(args)
                .into_iter()
                .filter(move |word| {
                    analysis
                        .lookup(SymbolKind::Param, scope, &word.key)
                        .is_some()
                })
                .map(|word| {
                    Diagnostic::new(
                        word.range,
                        Severity::Warning,
                        "measure-keyword-param",
                        format!(
                            "`{0}` is a `.measure` keyword, so HSPICE does not read it as parameter `{0}` here; rename the parameter",
                            word.text
                        ),
                    )
                })
        })
        .collect()
}

/// Returns all definitions of `.param` statements and `.subckt` headers, with their names.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn definitions<'a>(
    syntax: &'a SyntaxInfo,
    analysis: &'a Analysis,
) -> impl Iterator<Item = &'a ParamDef> + 'a {
    live_commands(syntax, analysis).flat_map(|(_, command, _)| match &command.kind {
        CommandKind::Param { definitions } => definitions.as_slice(),
        CommandKind::Subckt { params, .. } => params.as_slice(),
        _ => &[],
    })
}

/// Reports definitions of predefined variables that HSPICE does not allow to redefine.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn reserved_params(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    definitions(syntax, analysis)
        .filter(|definition| definition.arguments.is_empty())
        .filter(|definition| {
            find_special_variable(&definition.name.key)
                .is_some_and(|variable| !variable.redefinable)
        })
        .map(|definition| {
            Diagnostic::new(
                definition.name.range,
                Severity::Error,
                "reserved-param",
                format!(
                    "`{}` is a predefined HSPICE variable and cannot be redefined",
                    definition.name.text
                ),
            )
        })
        .collect()
}

/// Hints at parameters and functions defined twice (the last definition wins).
///
/// # Arguments
/// * `analysis` - semantic analysis
fn duplicate_params(analysis: &Analysis) -> Vec<Diagnostic> {
    let is_statement_definition = |symbol: &crate::analysis::symbols::Symbol| match &symbol.detail {
        SymbolDetail::Param { source, .. } => *source == ParamSource::Statement,
        SymbolDetail::Function { .. } => true,
        _ => false,
    };
    repeated_definitions(analysis, is_statement_definition)
        .into_iter()
        .map(|symbol| {
            Diagnostic::new(
                symbol.name.range,
                Severity::Hint,
                "duplicate-param",
                format!(
                    "`{}` is defined again; the last definition wins",
                    symbol.name.text
                ),
            )
        })
        .collect()
}

/// Hints at `.param` definitions that nothing in this document uses (top-level files with full context only).
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn unused_params(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    if !syntax.is_top_level || !analysis.context_complete {
        return Vec::new();
    }
    let used_ids: HashSet<usize> = analysis
        .references
        .iter()
        .filter_map(|reference| reference.target)
        .collect();
    let key_of = |id: usize| {
        analysis
            .symbols
            .get(id)
            .map(|symbol| (symbol.scope, symbol.name.key.clone()))
    };
    let used: HashSet<(usize, String)> = used_ids.into_iter().filter_map(key_of).collect();
    let data_columns: HashSet<&str> = analysis
        .symbols
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.detail,
                SymbolDetail::Param {
                    source: ParamSource::DataColumn,
                    ..
                }
            )
        })
        .map(|symbol| symbol.name.key.as_str())
        .collect();
    analysis
        .symbols
        .iter()
        .filter(|symbol| symbol.is_local() && symbol.kind == SymbolKind::Param)
        .filter(|symbol| {
            matches!(
                symbol.detail,
                SymbolDetail::Param {
                    source: ParamSource::Statement,
                    ..
                }
            )
        })
        .filter(|symbol| {
            let key = symbol.name.key.as_str();
            !used.contains(&(symbol.scope, key.to_string()))
                && !data_columns.contains(key)
                && !analysis
                    .external_words
                    .iter()
                    .any(|words| words.contains(key))
        })
        .map(|symbol| {
            let mut diagnostic = Diagnostic::new(
                symbol.name.range,
                Severity::Hint,
                "unused-param",
                format!("Parameter `{}` is never used", symbol.name.text),
            );
            diagnostic.unnecessary = true;
            diagnostic
        })
        .collect()
}

/// Warns about unquoted `.param` values that start with `-` or contain whitespace (plain numbers are fine).
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
/// * `text` - document text
pub fn unquoted_expressions(
    syntax: &SyntaxInfo,
    analysis: &Analysis,
    text: &str,
) -> Vec<Diagnostic> {
    live_commands(syntax, analysis)
        .filter_map(|(_, command, _)| match &command.kind {
            CommandKind::Param { definitions } => Some(definitions),
            _ => None,
        })
        .flatten()
        .filter(|definition| !definition.quoted && definition.value.is_some())
        .filter_map(|definition| {
            let source = text.get(definition.range.start..definition.range.end)?;
            let value = source.split_once('=')?.1.trim();
            let risky = (value.starts_with('-') || value.contains(char::is_whitespace))
                && parse_number(value).is_none();
            risky.then(|| {
                Diagnostic::new(
                    definition.range,
                    Severity::Warning,
                    "unquoted-expression",
                    format!(
                        "Quote the expression of `{}`: `{}='{value}'`",
                        definition.name.text, definition.name.text
                    ),
                )
            })
        })
        .collect()
}
