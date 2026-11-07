//! Parameter rules: reserved names, duplicates, unused parameters and unquoted expressions.

use std::collections::HashSet;

use super::common::{live_commands, repeated_definitions};
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::symbols::{ParamSource, SymbolDetail, SymbolKind};
use crate::config::DiagnosticSettings;
use crate::lang::functions::find_special_variable;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{CommandKind, ParamDef};
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
        .chain(
            settings
                .unused_params
                .then(|| unused_params(syntax, analysis))
                .into_iter()
                .flatten(),
        )
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
            !used.contains(&(symbol.scope, symbol.name.key.clone()))
                && !data_columns.contains(symbol.name.key.as_str())
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
