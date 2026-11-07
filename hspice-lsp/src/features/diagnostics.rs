//! Conversion of analysis diagnostics into LSP diagnostics.

use ls_types::{DiagnosticSeverity, DiagnosticTag, NumberOrString};

use super::to_lsp_range;
use crate::analysis::diagnostics::{Diagnostic, Severity};
use crate::state::document::Document;
use crate::state::workspace::Workspace;

/// Converts all diagnostics of a document for `textDocument/publishDiagnostics`.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document whose diagnostics are converted
pub fn to_lsp_diagnostics(workspace: &Workspace, document: &Document) -> Vec<ls_types::Diagnostic> {
    document
        .diagnostics
        .iter()
        .map(|diagnostic| to_lsp_diagnostic(workspace, document, diagnostic))
        .collect()
}

/// Converts one diagnostic.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document the diagnostic belongs to
/// * `diagnostic` - analysis diagnostic
fn to_lsp_diagnostic(
    workspace: &Workspace,
    document: &Document,
    diagnostic: &Diagnostic,
) -> ls_types::Diagnostic {
    let severity = match diagnostic.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Information => DiagnosticSeverity::INFORMATION,
        Severity::Hint => DiagnosticSeverity::HINT,
    };
    let tags: Vec<DiagnosticTag> = [
        diagnostic.unnecessary.then_some(DiagnosticTag::UNNECESSARY),
        diagnostic.deprecated.then_some(DiagnosticTag::DEPRECATED),
    ]
    .into_iter()
    .flatten()
    .collect();
    ls_types::Diagnostic {
        range: to_lsp_range(workspace, document, diagnostic.range),
        severity: Some(severity),
        code: Some(NumberOrString::String(diagnostic.code.to_string())),
        source: Some("hspice".to_string()),
        message: diagnostic.message.clone(),
        tags: (!tags.is_empty()).then_some(tags),
        ..Default::default()
    }
}
