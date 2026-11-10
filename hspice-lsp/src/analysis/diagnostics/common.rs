//! Helpers shared by the diagnostic rule families.

use std::collections::HashSet;

use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::blocks::BlockKind;
use crate::analysis::symbols::Symbol;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{Command, Element, Statement, StatementKind, TextRange};

/// Suffix appended to "undefined" messages when definitions of other files may be missing.
const MISSING_DEFINITIONS_NOTE: &str = " (definitions from other files may be missing)";

/// Iterates over all statements that have semantics, with their indices.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - analysis with the block structure
pub fn live_statements<'a>(
    syntax: &'a SyntaxInfo,
    analysis: &'a Analysis,
) -> impl Iterator<Item = (usize, &'a Statement)> + 'a {
    syntax
        .netlist
        .statements
        .iter()
        .enumerate()
        .filter(move |(index, _)| !analysis.structure.is_skipped(*index))
}

/// Iterates over all live dot commands with their statement index and range.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - analysis with the block structure
pub fn live_commands<'a>(
    syntax: &'a SyntaxInfo,
    analysis: &'a Analysis,
) -> impl Iterator<Item = (usize, &'a Command, TextRange)> + 'a {
    live_statements(syntax, analysis).filter_map(|(index, statement)| match &statement.kind {
        StatementKind::Command(command) => Some((index, command, statement.range)),
        _ => None,
    })
}

/// Iterates over all live element lines with their statement index.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - analysis with the block structure
pub fn live_elements<'a>(
    syntax: &'a SyntaxInfo,
    analysis: &'a Analysis,
) -> impl Iterator<Item = (usize, &'a Element)> + 'a {
    live_statements(syntax, analysis).filter_map(|(index, statement)| match &statement.kind {
        StatementKind::Element(element) => Some((index, element)),
        _ => None,
    })
}

/// Returns true if the statement lies between an `.if` and its `.endif`.
///
/// # Arguments
/// * `analysis` - analysis with the block structure
/// * `statement` - statement index
pub fn inside_if(analysis: &Analysis, statement: usize) -> bool {
    analysis.structure.blocks.iter().any(|block| {
        block.kind == BlockKind::If
            && block.open < statement
            && block.close.is_none_or(|close| statement < close)
    })
}

/// Builds an "undefined …" diagnostic: an error, or a warning when other files may hold the definition.
///
/// # Arguments
/// * `analysis` - analysis (for `context_complete`)
/// * `range` - location
/// * `code` - stable code
/// * `message` - message without the incompleteness note
pub fn lenient(
    analysis: &Analysis,
    range: TextRange,
    code: &'static str,
    message: String,
) -> Diagnostic {
    if analysis.context_complete {
        Diagnostic::new(range, Severity::Error, code, message)
    } else {
        Diagnostic::new(
            range,
            Severity::Warning,
            code,
            format!("{message}{MISSING_DEFINITIONS_NOTE}"),
        )
    }
}

/// Returns the later definitions of local symbols that repeat an earlier one.
///
/// Definitions count as repeated when kind, scope, name, `.alter` section and `.lib` section
/// agree; definitions inside `.if` branches are ignored.
///
/// # Arguments
/// * `analysis` - analysis with the symbols
/// * `keep` - selects the symbols to consider
pub fn repeated_definitions(analysis: &Analysis, keep: impl Fn(&Symbol) -> bool) -> Vec<&Symbol> {
    let mut seen = HashSet::new();
    analysis
        .symbols
        .iter()
        .filter(|symbol| symbol.is_local() && keep(symbol))
        .filter_map(|symbol| symbol.statement.map(|statement| (symbol, statement)))
        .filter(|(_, statement)| !inside_if(analysis, *statement))
        .filter(|(symbol, statement)| {
            let alter = analysis
                .structure
                .statement_alter
                .get(*statement)
                .copied()
                .unwrap_or_default();
            let key = (
                symbol.kind,
                symbol.scope,
                symbol.name.key.clone(),
                alter,
                symbol.section.clone(),
            );
            !seen.insert(key)
        })
        .map(|(symbol, _)| symbol)
        .collect()
}
