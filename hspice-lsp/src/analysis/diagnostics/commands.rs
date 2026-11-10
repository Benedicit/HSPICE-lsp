//! Command rules: unknown, foreign and obsolete commands, model types, options and `.mosra` parameters.

use super::common::live_commands;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::eval::evaluate;
use crate::analysis::symbols::{ScopeId, SymbolKind};
use crate::config::DiagnosticSettings;
use crate::lang::Status;
use crate::lang::commands::find_command;
use crate::lang::measure::RESERVED_MEASURE_NAMES;
use crate::lang::models::find_model_type;
use crate::lang::mosra::find_mosra_param;
use crate::lang::options::find_option;
use crate::lang::value_sets::values_of;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{
    Arg, AssignKey, Assignment, Command, CommandKind, Expr, ExprKind, Ident, Value,
};

/// Runs all command-name rules.
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
    let runlvl_off =
        live_commands(syntax, analysis).any(|(_, command, _)| disables_runlvl(command));
    live_commands(syntax, analysis)
        .flat_map(|(index, command, _)| {
            let ignored = (!runlvl_off)
                .then(|| ignored_by_runlvl(command))
                .into_iter()
                .flatten();
            command_status(command)
                .into_iter()
                .chain(model_type_status(command))
                .chain(option_diagnostics(command, settings))
                .chain(mosra_diagnostics(command, settings))
                .chain(value_diagnostics(analysis, index, command, settings))
                .chain(ignored)
                .collect::<Vec<_>>()
        })
        .chain(reserved_measure_names(analysis))
        .collect()
}

/// Reports `.measure` results named `dc`, `ac`, `tran` or `last`, which HSPICE reserves.
///
/// # Arguments
/// * `analysis` - document analysis (local measure symbols)
fn reserved_measure_names(analysis: &Analysis) -> Vec<Diagnostic> {
    analysis
        .symbols
        .iter()
        .filter(|symbol| symbol.is_local() && symbol.kind == SymbolKind::Measure)
        .filter(|symbol| RESERVED_MEASURE_NAMES.contains(&symbol.name.key.as_str()))
        .map(|symbol| {
            Diagnostic::new(
                symbol.name.range,
                Severity::Error,
                "reserved-measure-name",
                format!(
                    "`{}` is reserved and cannot name a `.measure` result",
                    symbol.name.text
                ),
            )
        })
        .collect()
}

/// Reports an unknown, foreign or obsolete command keyword.
///
/// # Arguments
/// * `command` - dot command
fn command_status(command: &Command) -> Option<Diagnostic> {
    let keyword = &command.keyword;
    let text = &keyword.text;
    let Some(spec) = find_command(text) else {
        return Some(Diagnostic::new(
            keyword.range,
            Severity::Warning,
            "unknown-command",
            format!("Unknown command `{text}`"),
        ));
    };
    match spec.status {
        Status::Foreign => Some(Diagnostic::new(
            keyword.range,
            Severity::Warning,
            "foreign-command",
            format!("`{text}` is ngspice/LTspice/PSpice syntax, not HSPICE"),
        )),
        Status::Obsolete => Some(obsolete(keyword, format!("`{text}` is obsolete in HSPICE"))),
        Status::Supported => None,
    }
}

/// Builds an obsolete-construct hint tagged as deprecated.
///
/// # Arguments
/// * `name` - the obsolete name
/// * `message` - hint text
fn obsolete(name: &Ident, message: String) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(name.range, Severity::Hint, "obsolete", message);
    diagnostic.deprecated = true;
    diagnostic
}

/// Reports `.model` types that belong to another dialect or are obsolete.
///
/// # Arguments
/// * `command` - dot command
fn model_type_status(command: &Command) -> Option<Diagnostic> {
    let CommandKind::Model {
        model_type: Some(model_type),
        ..
    } = &command.kind
    else {
        return None;
    };
    let spec = find_model_type(&model_type.key)?;
    match spec.status {
        Status::Foreign => Some(Diagnostic::new(
            model_type.range,
            Severity::Warning,
            "foreign-command",
            format!(
                "Model type `{}` is ngspice/LTspice/PSpice syntax, not HSPICE",
                model_type.text
            ),
        )),
        Status::Obsolete => Some(obsolete(
            model_type,
            format!("Model type `{}` is obsolete in HSPICE", model_type.text),
        )),
        Status::Supported => None,
    }
}

/// Reports unknown and obsolete `.option` names.
///
/// # Arguments
/// * `command` - dot command
/// * `settings` - diagnostic toggles
fn option_diagnostics(command: &Command, settings: &DiagnosticSettings) -> Vec<Diagnostic> {
    let CommandKind::Option { args } = &command.kind else {
        return Vec::new();
    };
    args.iter()
        .filter_map(argument_name)
        .filter_map(|name| match find_option(&name.key) {
            None => settings.unknown_options.then(|| {
                Diagnostic::new(
                    name.range,
                    Severity::Warning,
                    "unknown-option",
                    format!("Unknown option `{}`", name.text),
                )
            }),
            Some(spec) if spec.status == Status::Obsolete => Some(obsolete(
                name,
                format!("Option `{}` is obsolete in HSPICE", name.text),
            )),
            Some(_) => None,
        })
        .collect()
}

/// Reports unknown `.mosra` parameters and a missing `RelTotalTime`.
///
/// # Arguments
/// * `command` - dot command
/// * `settings` - diagnostic toggles
fn mosra_diagnostics(command: &Command, settings: &DiagnosticSettings) -> Vec<Diagnostic> {
    let CommandKind::Generic { args } = &command.kind else {
        return Vec::new();
    };
    if command.keyword.key != ".mosra" {
        return Vec::new();
    }
    let names: Vec<&Ident> = args.iter().filter_map(argument_name).collect();
    let missing_total = (!names.iter().any(|name| name.key == "reltotaltime")).then(|| {
        Diagnostic::new(
            command.keyword.range,
            Severity::Warning,
            "mosra-args",
            "`.MOSRA` requires `RelTotalTime`",
        )
    });
    let unknown = names
        .into_iter()
        .filter(|name| settings.unknown_options && find_mosra_param(&name.key).is_none())
        .map(|name| {
            Diagnostic::new(
                name.range,
                Severity::Warning,
                "unknown-mosra-param",
                format!("Unknown `.MOSRA` parameter `{}`", name.text),
            )
        });
    missing_total.into_iter().chain(unknown).collect()
}

/// Returns the `key=expression` assignments of a command.
///
/// # Arguments
/// * `args` - command arguments
fn named_values(args: &[Arg]) -> impl Iterator<Item = (&Ident, &Expr)> {
    args.iter().filter_map(|arg| match arg {
        Arg::Assign(Assignment {
            key: AssignKey::Name(key),
            value: Some(Value::Expr(expr)),
            ..
        }) => Some((key, expr)),
        _ => None,
    })
}

/// Reports literal `key=value` arguments outside the documented value set.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `index` - statement index of the command
/// * `command` - dot command
/// * `settings` - diagnostic toggles
fn value_diagnostics(
    analysis: &Analysis,
    index: usize,
    command: &Command,
    settings: &DiagnosticSettings,
) -> Vec<Diagnostic> {
    let (CommandKind::Option { args } | CommandKind::Generic { args }) = &command.kind else {
        return Vec::new();
    };
    let Some(spec) = find_command(&command.keyword.key).filter(|_| settings.unknown_options) else {
        return Vec::new();
    };
    let scope = analysis.structure.scope_of(index);
    named_values(args)
        .filter_map(|(key, expr)| {
            let values = values_of(spec.name, &key.key)?;
            is_outside(analysis, scope, values, expr).then(|| {
                Diagnostic::new(
                    expr.range,
                    Severity::Warning,
                    "invalid-value",
                    format!(
                        "`{}` takes one of: {}",
                        key.text,
                        values.join(", ").to_ascii_uppercase()
                    ),
                )
            })
        })
        .collect()
}

/// Returns true if a literal (number or bare word that is no parameter) is not one of `values`.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `scope` - scope of the statement
/// * `values` - documented lowercase values
/// * `expr` - value expression
fn is_outside(analysis: &Analysis, scope: ScopeId, values: &[&str], expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Ident(word) => {
            analysis
                .lookup(SymbolKind::Param, scope, &word.key)
                .is_none()
                && !values.contains(&word.key.as_str())
        }
        _ => evaluate(expr, &|_| None)
            .is_some_and(|number| !values.iter().any(|value| value.parse() == Ok(number))),
    }
}

/// Options that RUNLVL replaces by its own timestep control.
const RUNLVL_IGNORED: &[&str] = &[
    "lvltim", "dvdt", "ft", "fast", "trtol", "absvar", "relvar", "relq", "chgtol", "dvtr", "imin",
    "itl3",
];

/// Returns true for `.option runlvl=0`, which switches RUNLVL off.
///
/// # Arguments
/// * `command` - dot command
fn disables_runlvl(command: &Command) -> bool {
    let CommandKind::Option { args } = &command.kind else {
        return false;
    };
    named_values(args)
        .any(|(key, expr)| key.key == "runlvl" && evaluate(expr, &|_| None) == Some(0.0))
}

/// Hints at step-control options that have no effect while RUNLVL is active (its default).
///
/// # Arguments
/// * `command` - dot command
fn ignored_by_runlvl(command: &Command) -> Vec<Diagnostic> {
    let CommandKind::Option { args } = &command.kind else {
        return Vec::new();
    };
    args.iter()
        .filter_map(argument_name)
        .filter(|name| {
            find_option(&name.key).is_some_and(|spec| RUNLVL_IGNORED.contains(&spec.name))
        })
        .map(|name| {
            let mut diagnostic = Diagnostic::new(
                name.range,
                Severity::Hint,
                "runlvl-ignored-option",
                format!(
                    "`{}` is ignored while RUNLVL is active (default 3); set `RUNLVL=0` to use it",
                    name.text
                ),
            );
            diagnostic.unnecessary = true;
            diagnostic
        })
        .collect()
}

/// Returns the name of an argument: a bare word or the key of an assignment.
///
/// # Arguments
/// * `arg` - `.option` or `.mosra` argument
fn argument_name(arg: &Arg) -> Option<&Ident> {
    match arg {
        Arg::Value(Value::Word(word)) => Some(word),
        Arg::Assign(assignment) => match &assignment.key {
            AssignKey::Name(name) => Some(name),
            AssignKey::Call(_) => None,
        },
        _ => None,
    }
}
