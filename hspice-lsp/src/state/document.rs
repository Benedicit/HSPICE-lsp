//! An open document: text, parse results, analysis and diagnostics.

use std::path::{Path, PathBuf};

use ls_types::Uri;
use tree_sitter::Tree;

use crate::analysis::Analysis;
use crate::analysis::diagnostics::Diagnostic;
use crate::state::line_index::LineIndex;
use crate::syntax::ast::{Netlist, TextRange};
use crate::syntax::lower::lower;
use crate::syntax::parse::{SyntaxError, mask_ranges, parse_tree, syntax_errors, title_ranges};

/// Parse results of a document.
#[derive(Debug, Clone)]
pub struct SyntaxInfo {
    /// Concrete syntax tree of the (title-masked) text.
    pub tree: Option<Tree>,
    /// Lowered statements.
    pub netlist: Netlist,
    /// Tree-sitter ERROR / MISSING nodes.
    pub errors: Vec<SyntaxError>,
    /// Title lines HSPICE ignores (empty for include files).
    pub title_ranges: Vec<TextRange>,
    /// True if the document is a top-level netlist (its first line is a title).
    pub is_top_level: bool,
}

/// Parses a document, masking title lines of top-level netlists first.
///
/// # Arguments
/// * `text` - document text
/// * `is_top_level` - whether HSPICE treats the first line as a title
pub fn parse_document(text: &str, is_top_level: bool) -> SyntaxInfo {
    let titles = if is_top_level {
        title_ranges(text)
    } else {
        Vec::new()
    };
    let masked = mask_ranges(text, &titles);
    let tree = parse_tree(&masked);
    let netlist = tree
        .as_ref()
        .map(|tree| lower(tree, &masked))
        .unwrap_or_default();
    let errors = tree
        .as_ref()
        .map(|tree| syntax_errors(tree, &masked))
        .unwrap_or_default();
    SyntaxInfo {
        tree,
        netlist,
        errors,
        title_ranges: titles,
        is_top_level,
    }
}

/// An open document with all derived data.
#[derive(Debug, Clone)]
pub struct Document {
    /// Document URI.
    pub uri: Uri,
    /// File-system path, if the URI is a `file:` URI.
    pub path: Option<PathBuf>,
    /// Version reported by the client.
    pub version: i32,
    /// Current text.
    pub text: String,
    /// Line index of `text`.
    pub line_index: LineIndex,
    /// Parse results.
    pub syntax: SyntaxInfo,
    /// Semantic analysis.
    pub analysis: Analysis,
    /// Diagnostics computed from `syntax` and `analysis`.
    pub diagnostics: Vec<Diagnostic>,
}

impl Document {
    /// Returns the directory used to resolve relative includes.
    pub fn directory(&self) -> Option<&Path> {
        self.path.as_deref().and_then(Path::parent)
    }

    /// Returns the source text of a range (empty if out of bounds).
    ///
    /// # Arguments
    /// * `range` - byte range
    pub fn slice(&self, range: TextRange) -> &str {
        self.text.get(range.start..range.end).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_masked_only_for_top_level_files() {
        let text = "Inverter (test\nR1 a b 1k\n.end\n";
        let top = parse_document(text, true);
        assert!(top.errors.is_empty());
        assert_eq!(top.title_ranges.len(), 1);
        let include = parse_document("R1 a b 1k\n", false);
        assert_eq!(include.netlist.statements.len(), 1);
        assert!(include.title_ranges.is_empty());
    }
}
