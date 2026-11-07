//! Command rules: unknown, foreign and obsolete commands, model types and options.

use super::common::live_commands;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::symbols::SymbolKind;
use crate::config::DiagnosticSettings;
use crate::lang::Status;
use crate::lang::commands::find_command;
use crate::lang::measure::RESERVED_MEASURE_NAMES;
use crate::lang::models::find_model_type;
use crate::lang::options::find_option;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{Arg, AssignKey, Command, CommandKind, Ident};

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
    live_commands(syntax, analysis)
        .flat_map(|(_, command, _)| {
            command_status(command)
                .into_iter()
                .chain(model_type_status(command))
                .chain(option_diagnostics(command, settings))
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
        .filter_map(option_name)
        .filter_map(|name| match find_option(&name.key) {
            None => settings.unknown_options.then(|| {
                Diagnostic::new(
                    name.range,
                    Severity::Hint,
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

/// Returns the option name of an argument: a bare word or the key of an assignment.
///
/// # Arguments
/// * `arg` - `.option` argument
fn option_name(arg: &Arg) -> Option<&Ident> {
    match arg {
        Arg::Value(crate::syntax::ast::Value::Word(word)) => Some(word),
        Arg::Assign(assignment) => match &assignment.key {
            AssignKey::Name(name) => Some(name),
            AssignKey::Call(_) => None,
        },
        _ => None,
    }
}
