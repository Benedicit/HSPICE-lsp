//! LSP feature implementations; every feature is a read-only query over the workspace state.

pub mod code_actions;
pub mod completion;
pub mod diagnostics;
pub mod folding;
pub mod hover;
pub mod inlay_hints;
pub mod navigation;
pub mod semantic_tokens;
pub mod signature_help;
pub mod symbols;

use ls_types::{Location, Position, Range, Uri};

use crate::analysis::symbols::{Origin, Symbol};
use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::TextRange;

/// Converts a byte range of a document into an LSP range.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document the range belongs to
/// * `range` - byte range
pub fn to_lsp_range(workspace: &Workspace, document: &Document, range: TextRange) -> Range {
    document
        .line_index
        .range(&document.text, range, workspace.encoding)
}

/// Converts an LSP position into a byte offset of a document.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document the position belongs to
/// * `position` - LSP position
pub fn to_offset(workspace: &Workspace, document: &Document, position: Position) -> usize {
    document
        .line_index
        .offset(&document.text, position, workspace.encoding)
}

/// Returns the location of a byte range inside a symbol's defining file.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - analysed document (used for local symbols)
/// * `symbol` - symbol whose file is used
/// * `range` - byte range inside that file
pub fn location_in_symbol_file(
    workspace: &Workspace,
    document: &Document,
    symbol: &Symbol,
    range: TextRange,
) -> Option<Location> {
    match &symbol.origin {
        Origin::Local => Some(Location {
            uri: document.uri.clone(),
            range: to_lsp_range(workspace, document, range),
        }),
        Origin::External(file) => Some(Location {
            uri: Uri::from_file_path(&file.path)?,
            range: file.line_index.range(&file.text, range, workspace.encoding),
        }),
    }
}

/// Returns the location of a symbol's name.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - analysed document (used for local symbols)
/// * `symbol` - symbol to locate
pub fn symbol_location(
    workspace: &Workspace,
    document: &Document,
    symbol: &Symbol,
) -> Option<Location> {
    location_in_symbol_file(workspace, document, symbol, symbol.name.range)
}

/// Returns the source text of a symbol's whole definition (from its own file).
///
/// # Arguments
/// * `document` - analysed document (used for local symbols)
/// * `symbol` - symbol whose definition text is returned
pub fn definition_text<'a>(document: &'a Document, symbol: &'a Symbol) -> &'a str {
    let range = symbol.definition_range;
    match &symbol.origin {
        Origin::Local => document.slice(range),
        Origin::External(file) => file.text.get(range.start..range.end).unwrap_or_default(),
    }
}
