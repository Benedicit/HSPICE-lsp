//! Block structure rules: unclosed and unmatched blocks, name mismatches, ports, duplicate definitions, missing `.end`.

use super::common::{live_commands, repeated_definitions};
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::blocks::{Block, BlockKind};
use crate::analysis::symbols::{Symbol, SymbolKind};
use crate::lang::commands::{CommandCategory, find_command};
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{CommandKind, Ident, StatementKind, TextRange};

/// Runs all structure rules except the text-based title rule.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    unclosed_blocks(syntax, analysis)
        .into_iter()
        .chain(unmatched_ends(syntax, analysis))
        .chain(ends_name_mismatches(analysis))
        .chain(bad_ports(syntax, analysis))
        .chain(duplicate_definitions(analysis))
        .chain(missing_end(syntax, analysis))
        .collect()
}

/// Returns the opening and closing keywords of a block kind.
///
/// # Arguments
/// * `kind` - block kind
fn keywords_of(kind: BlockKind) -> (&'static str, &'static str) {
    match kind {
        BlockKind::Subckt => (".subckt", ".ends"),
        BlockKind::LibSection => (".lib", ".endl"),
        BlockKind::If => (".if", ".endif"),
        BlockKind::Data => (".data", ".enddata"),
        BlockKind::Protect => (".protect", ".unprotect"),
        BlockKind::Variation => (".variation", ".end_variation"),
        BlockKind::DesignExploration => (".design_exploration", ".end_design_exploration"),
    }
}

/// Reports blocks without a closing statement, on their opener.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn unclosed_blocks(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    analysis
        .structure
        .blocks
        .iter()
        .filter(|block| block.close.is_none())
        .filter_map(|block| {
            let statement = syntax.netlist.statements.get(block.open)?;
            let (opener, closer) = keywords_of(block.kind);
            let named = block.name.as_ref().map_or_else(
                || opener.to_string(),
                |name| format!("{opener} {}", name.text),
            );
            Some(Diagnostic::new(
                statement.range,
                Severity::Error,
                "unclosed-block",
                format!("`{named}` is never closed with `{closer}`"),
            ))
        })
        .collect()
}

/// Reports closers and `.else`/`.elseif` statements without an opener.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn unmatched_ends(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    analysis
        .structure
        .unmatched
        .iter()
        .filter_map(|index| {
            let statement = syntax.netlist.statements.get(*index)?;
            let StatementKind::Command(command) = &statement.kind else {
                return None;
            };
            let opener = match &command.kind {
                CommandKind::Ends { .. } => ".subckt",
                CommandKind::Endl { .. } => ".lib",
                CommandKind::EndIf | CommandKind::Else | CommandKind::ElseIf { .. } => ".if",
                _ => match command.keyword.key.as_str() {
                    ".enddata" => ".data",
                    ".unprot" | ".unprotect" => ".protect",
                    ".end_variation" => ".variation",
                    ".end_design_exploration" => ".design_exploration",
                    _ => "opener",
                },
            };
            Some(Diagnostic::new(
                command.keyword.range,
                Severity::Error,
                "unmatched-end",
                format!("`{}` has no matching `{opener}`", command.keyword.text),
            ))
        })
        .collect()
}

/// Warns when `.ends name` / `.endl name` names a different block than it closes.
///
/// # Arguments
/// * `analysis` - semantic analysis
fn ends_name_mismatches(analysis: &Analysis) -> Vec<Diagnostic> {
    analysis
        .structure
        .blocks
        .iter()
        .filter(|block| matches!(block.kind, BlockKind::Subckt | BlockKind::LibSection))
        .filter_map(mismatch_of)
        .collect()
}

/// Builds the mismatch warning of one block, if its closer names another block.
///
/// # Arguments
/// * `block` - closed block
fn mismatch_of(block: &Block) -> Option<Diagnostic> {
    let (name, close_name) = (block.name.as_ref()?, block.close_name.as_ref()?);
    (name.key != close_name.key).then(|| {
        let (opener, closer) = keywords_of(block.kind);
        Diagnostic::new(
            close_name.range,
            Severity::Warning,
            "ends-name-mismatch",
            format!(
                "`{closer} {}` closes `{opener} {}`",
                close_name.text, name.text
            ),
        )
    })
}

/// Reports the ground node `0` and duplicate names in `.subckt` port lists.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn bad_ports(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_commands(syntax, analysis)
        .filter_map(|(_, command, _)| match &command.kind {
            CommandKind::Subckt { ports, .. } => Some(ports),
            _ => None,
        })
        .flat_map(|ports| {
            ports
                .iter()
                .enumerate()
                .filter_map(|(position, port)| port_problem(ports, position, port))
        })
        .collect()
}

/// Describes why a port is invalid, if it is.
///
/// # Arguments
/// * `ports` - complete port list
/// * `position` - index of the port to check
/// * `port` - the port
fn port_problem(ports: &[Ident], position: usize, port: &Ident) -> Option<Diagnostic> {
    let message = if port.text == "0" {
        "Ground node `0` cannot be a subcircuit port".to_string()
    } else if ports[..position]
        .iter()
        .any(|earlier| earlier.key == port.key)
    {
        format!("Port `{}` is listed more than once", port.text)
    } else {
        return None;
    };
    Some(Diagnostic::new(
        port.range,
        Severity::Error,
        "bad-port",
        message,
    ))
}

/// Warns about subcircuits and models defined twice in the same scope.
///
/// # Arguments
/// * `analysis` - semantic analysis
fn duplicate_definitions(analysis: &Analysis) -> Vec<Diagnostic> {
    repeated_definitions(analysis, |symbol| {
        matches!(symbol.kind, SymbolKind::Subckt | SymbolKind::Model)
    })
    .into_iter()
    .map(duplicate_definition)
    .collect()
}

/// Builds the duplicate warning of one repeated subcircuit or model.
///
/// # Arguments
/// * `symbol` - the repeated definition
fn duplicate_definition(symbol: &Symbol) -> Diagnostic {
    let (code, what) = if symbol.kind == SymbolKind::Subckt {
        ("duplicate-subckt", "Subcircuit")
    } else {
        ("duplicate-model", "Model")
    };
    Diagnostic::new(
        symbol.name.range,
        Severity::Warning,
        code,
        format!(
            "{what} `{}` is already defined in this scope",
            symbol.name.text
        ),
    )
}

/// Warns when a top-level file with analyses has no `.end` card, on its last statement.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
fn missing_end(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let has_analysis = live_commands(syntax, analysis).any(|(_, command, _)| {
        find_command(&command.keyword.text)
            .is_some_and(|spec| spec.category == CommandCategory::Analysis)
    });
    let has_end = live_commands(syntax, analysis)
        .any(|(_, command, _)| matches!(command.kind, CommandKind::End));
    if !syntax.is_top_level || !has_analysis || has_end {
        return Vec::new();
    }
    let last: Option<TextRange> = syntax
        .netlist
        .statements
        .iter()
        .rfind(|statement| !matches!(statement.kind, StatementKind::Comment))
        .map(|statement| statement.range);
    last.map(|range| {
        Diagnostic::new(
            range,
            Severity::Warning,
            "missing-end",
            "The netlist has analyses but no `.end` card",
        )
    })
    .into_iter()
    .collect()
}

/// Warns when the title line looks like a dot command, which HSPICE ignores.
///
/// # Arguments
/// * `syntax` - parse results
/// * `text` - document text
pub fn title_is_statement(syntax: &SyntaxInfo, text: &str) -> Vec<Diagnostic> {
    syntax
        .title_ranges
        .iter()
        .filter(|_| syntax.is_top_level)
        .filter_map(|range| {
            text.get(range.start..range.end)
                .map(|title| (title, *range))
        })
        .filter(|(title, _)| {
            let mut characters = title.trim_start().chars();
            characters.next() == Some('.')
                && characters
                    .next()
                    .is_some_and(|character| character.is_ascii_alphabetic())
        })
        .map(|(title, range)| {
            Diagnostic::new(
                range,
                Severity::Warning,
                "title-is-statement",
                format!(
                    "`{}` is the title line and HSPICE ignores it as a command",
                    title.trim()
                ),
            )
        })
        .collect()
}
