//! Tree-sitter parsing, title-line masking and syntax-error extraction.

use tree_sitter::{Language, Node, Parser, Tree};

use super::ast::TextRange;

/// A syntax error reported by tree-sitter (an ERROR or MISSING node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    /// Location of the erroneous text.
    pub range: TextRange,
    /// Human-readable description.
    pub message: String,
}

/// Returns the HSPICE tree-sitter language.
pub fn language() -> Language {
    tree_sitter_hspice::LANGUAGE.into()
}

/// Parses netlist text into a concrete syntax tree.
///
/// # Arguments
/// * `text` - netlist source (title lines already masked if needed)
///
/// # Returns
/// `None` only if the parser could not be created, which indicates an ABI mismatch.
pub fn parse_tree(text: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language()).ok()?;
    parser.parse(text, None)
}

/// Parses only the given byte ranges of the text, used for large include files.
///
/// # Arguments
/// * `text` - full file contents
/// * `ranges` - byte ranges to parse; everything else is skipped
pub fn parse_tree_in_ranges(text: &str, ranges: &[TextRange]) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language()).ok()?;
    let included: Vec<tree_sitter::Range> = ranges
        .iter()
        .map(|range| tree_sitter::Range {
            start_byte: range.start,
            end_byte: range.end,
            start_point: point_at(text, range.start),
            end_point: point_at(text, range.end),
        })
        .collect();
    parser.set_included_ranges(&included).ok()?;
    parser.parse(text, None)
}

/// Computes the tree-sitter point (row, byte column) of a byte offset.
///
/// # Arguments
/// * `text` - document text
/// * `offset` - byte offset
fn point_at(text: &str, offset: usize) -> tree_sitter::Point {
    let before = text.get(..offset).unwrap_or(text);
    let row = before.bytes().filter(|byte| *byte == b'\n').count();
    let column = before
        .rfind('\n')
        .map_or(offset, |newline| offset - newline - 1);
    tree_sitter::Point { row, column }
}

/// Iterates over lines as `(start offset, line text without line ending)`.
///
/// # Arguments
/// * `text` - document text
fn lines_with_offsets(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_inclusive('\n').scan(0, |offset, line| {
        let start = *offset;
        *offset += line.len();
        Some((start, line.trim_end_matches(['\n', '\r'])))
    })
}

/// Returns true if a line is an `.end` statement (not `.ends`, `.endl`, `.enddata` …).
///
/// # Arguments
/// * `line` - one line of text
fn is_end_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(keyword) = trimmed.get(..4) else {
        return false;
    };
    let rest = &trimmed[4..];
    keyword.eq_ignore_ascii_case(".end") && rest.chars().next().is_none_or(char::is_whitespace)
}

/// Returns true if the text contains an `.end` statement, which marks a top-level netlist.
///
/// # Arguments
/// * `text` - document text
pub fn has_end_statement(text: &str) -> bool {
    lines_with_offsets(text).any(|(_, line)| is_end_line(line))
}

/// Returns the ranges HSPICE treats as title lines: line 1 and the first non-blank line after each `.end`.
///
/// # Arguments
/// * `text` - text of a top-level netlist
pub fn title_ranges(text: &str) -> Vec<TextRange> {
    let lines: Vec<(usize, &str)> = lines_with_offsets(text).collect();
    let first_title = lines
        .first()
        .map(|(start, line)| TextRange::new(*start, start + line.len()));
    let titles_after_end = lines.iter().enumerate().filter_map(|(index, (_, line))| {
        if !is_end_line(line) {
            return None;
        }
        lines[index + 1..]
            .iter()
            .find(|(_, next)| !next.trim().is_empty())
            .map(|(start, next)| TextRange::new(*start, start + next.len()))
    });
    first_title
        .into_iter()
        .chain(titles_after_end)
        .filter(|range| !range.is_empty())
        .collect()
}

/// Replaces every byte inside the given ranges with a space, keeping all byte offsets valid.
///
/// # Arguments
/// * `text` - document text
/// * `ranges` - ranges to blank out (must not contain line breaks)
pub fn mask_ranges(text: &str, ranges: &[TextRange]) -> String {
    text.char_indices()
        .flat_map(|(offset, character)| {
            let masked = ranges
                .iter()
                .any(|range| range.start <= offset && offset < range.end);
            let replacement = if masked { ' ' } else { character };
            let repeat = if masked { character.len_utf8() } else { 1 };
            std::iter::repeat_n(replacement, repeat)
        })
        .collect()
}

/// Collects all ERROR and MISSING nodes of a tree as syntax errors.
///
/// # Arguments
/// * `tree` - parsed tree
/// * `text` - text the tree was parsed from
pub fn syntax_errors(tree: &Tree, text: &str) -> Vec<SyntaxError> {
    let mut errors = Vec::new();
    collect_errors(tree.root_node(), text, &mut errors);
    errors
}

/// Recursively appends the syntax errors below `node`, skipping subtrees without errors.
///
/// # Arguments
/// * `node` - subtree root
/// * `text` - source text
/// * `errors` - output list
fn collect_errors(node: Node, text: &str, errors: &mut Vec<SyntaxError>) {
    if !node.has_error() {
        return;
    }
    let range = TextRange::new(node.start_byte(), node.end_byte());
    if node.is_missing() {
        errors.push(SyntaxError {
            range,
            message: format!("Syntax error: missing `{}`", node.kind()),
        });
        return;
    }
    if node.is_error() {
        let snippet: String = text
            .get(range.start..range.end)
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(40)
            .collect();
        errors.push(SyntaxError {
            range,
            message: format!("Syntax error: unexpected `{}`", snippet.trim()),
        });
        return;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .for_each(|child| collect_errors(child, text, errors));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_end_statement_but_not_ends() {
        assert!(has_end_statement("* t\nR1 a b 1\n.END\n"));
        assert!(has_end_statement(".end comment"));
        assert!(!has_end_statement(
            ".subckt a b\n.ends a\n.endl\n.enddata\n"
        ));
    }

    #[test]
    fn title_is_first_line_and_line_after_end() {
        let text = "My title\nR1 a b 1k\n.end\n\nSecond deck\nR2 a b 1k\n.end\n";
        let titles = title_ranges(text);
        let titled: Vec<&str> = titles
            .iter()
            .map(|range| &text[range.start..range.end])
            .collect();
        assert_eq!(titled, vec!["My title", "Second deck"]);
    }

    #[test]
    fn masking_keeps_offsets() {
        let text = "Tïtle\nR1 a b 1k\n";
        let masked = mask_ranges(text, &title_ranges(text));
        assert_eq!(masked.len(), text.len());
        assert!(masked.starts_with("      \nR1"));
    }

    #[test]
    fn parses_and_reports_errors() {
        let text = "R1 a b 1k\n+ +\n";
        let tree = parse_tree("R1 a b 1k\n").expect("parser");
        assert!(syntax_errors(&tree, "R1 a b 1k\n").is_empty());
        let broken = parse_tree(text).expect("parser");
        assert!(!syntax_errors(&broken, text).is_empty());
    }

    #[test]
    fn ranged_parse_skips_other_text() {
        let text = "garbage (((\n.model n1 nmos level=1\n";
        let start = text.find(".model").unwrap_or_default();
        let tree =
            parse_tree_in_ranges(text, &[TextRange::new(start, text.len())]).expect("parser");
        assert!(!tree.root_node().has_error());
    }
}
