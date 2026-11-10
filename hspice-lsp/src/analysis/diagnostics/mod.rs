//! Diagnostics derived from the syntax tree and the analysis; each rule family lives in its own file.

mod analyses;
mod commands;
mod common;
mod elements;
mod functions;
mod mosra_conflicts;
mod nodes;
mod numbers;
mod params;
mod references;
mod source_functions;
mod structure;
mod walk;

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod rule_tests;

use crate::analysis::{Analysis, IncludeError};
use crate::config::DiagnosticSettings;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::TextRange;

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Definitely wrong; HSPICE will reject or misinterpret it.
    Error,
    /// Probably wrong.
    Warning,
    /// Noteworthy.
    Information,
    /// Style or likely-mistake hint.
    Hint,
}

/// One diagnostic with a stable code (used by tests and quick fixes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Location.
    pub range: TextRange,
    /// Severity.
    pub severity: Severity,
    /// Stable kebab-case code such as `undefined-param`.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// Rendered faded (unused code).
    pub unnecessary: bool,
    /// Rendered struck through (obsolete construct).
    pub deprecated: bool,
}

impl Diagnostic {
    /// Creates a diagnostic without tags.
    ///
    /// # Arguments
    /// * `range` - location
    /// * `severity` - severity
    /// * `code` - stable code
    /// * `message` - human-readable message
    pub fn new(
        range: TextRange,
        severity: Severity,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            range,
            severity,
            code,
            message: message.into(),
            unnecessary: false,
            deprecated: false,
        }
    }
}

/// Computes all diagnostics of a document and sorts them by position.
///
/// # Arguments
/// * `syntax` - parse results of the document
/// * `analysis` - semantic analysis of the document
/// * `settings` - diagnostic toggles
/// * `text` - document text (needed by the title-line and unquoted-expression rules)
pub fn compute(
    syntax: &SyntaxInfo,
    analysis: &Analysis,
    settings: &DiagnosticSettings,
    text: &str,
) -> Vec<Diagnostic> {
    let text_rules = structure::title_is_statement(syntax, text)
        .into_iter()
        .chain(params::unquoted_expressions(syntax, analysis, text));
    let floating = settings
        .floating_nodes
        .then(|| nodes::floating_nodes(syntax, analysis))
        .into_iter()
        .flatten();
    let mut diagnostics: Vec<Diagnostic> = syntax_diagnostics(syntax, analysis)
        .into_iter()
        .chain(include_diagnostics(analysis))
        .chain(structure::check(syntax, analysis))
        .chain(references::check(syntax, analysis))
        .chain(elements::check(syntax, analysis))
        .chain(functions::check(syntax, analysis))
        .chain(source_functions::check(syntax, analysis))
        .chain(nodes::no_ground(syntax, analysis))
        .chain(nodes::undefined_output_targets(syntax, analysis))
        .chain(nodes::duplicate_initial_conditions(syntax, analysis))
        .chain(floating)
        .chain(commands::check(syntax, analysis, settings))
        .chain(analyses::check(syntax, analysis))
        .chain(mosra_conflicts::check(syntax, analysis))
        .chain(params::check(syntax, analysis, settings))
        .chain(numbers::check(syntax, analysis, settings))
        .chain(text_rules)
        .collect();
    diagnostics.sort_by_key(|diagnostic| (diagnostic.range.start, diagnostic.severity));
    diagnostics
}

/// Reports tree-sitter syntax errors, except inside opaque regions (variation blocks, encrypted text).
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - analysis (for opaque statements)
fn syntax_diagnostics(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    syntax
        .errors
        .iter()
        .filter(|error| {
            crate::analysis::statement_at(&syntax.netlist, error.range.start).is_none_or(|index| {
                !analysis
                    .structure
                    .opaque
                    .get(index)
                    .copied()
                    .unwrap_or(false)
            })
        })
        .map(|error| {
            Diagnostic::new(
                error.range,
                Severity::Error,
                "syntax-error",
                error.message.clone(),
            )
        })
        .collect()
}

/// Reports include directives that could not be followed.
///
/// # Arguments
/// * `analysis` - analysis with include resolutions
fn include_diagnostics(analysis: &Analysis) -> Vec<Diagnostic> {
    analysis
        .includes
        .iter()
        .filter_map(|include| {
            let path = &include.directive.path;
            let (code, message) = match include.error? {
                IncludeError::NotFound => (
                    "include-not-found",
                    format!("Cannot find file `{}`", path.text),
                ),
                IncludeError::SectionNotFound => {
                    let section = include
                        .directive
                        .section
                        .as_ref()
                        .map_or("", |section| section.text.as_str());
                    (
                        "lib-section-not-found",
                        format!("Library `{}` has no section `{section}`", path.text),
                    )
                }
                IncludeError::Cycle => {
                    ("include-cycle", format!("`{}` includes itself", path.text))
                }
                IncludeError::TooLarge => (
                    "include-too-large",
                    format!(
                        "`{}` exceeds the configured size limit and was skipped",
                        path.text
                    ),
                ),
            };
            let range = match (include.error, &include.directive.section) {
                (Some(IncludeError::SectionNotFound), Some(section)) => section.range,
                _ => path.range,
            };
            let severity = if code == "include-too-large" {
                Severity::Information
            } else {
                Severity::Error
            };
            Some(Diagnostic::new(range, severity, code, message))
        })
        .collect()
}
