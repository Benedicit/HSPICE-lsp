//! Conversion between byte offsets and LSP positions.

use crate::syntax::ast::TextRange;
use ls_types::{Position, Range};

/// Position encoding negotiated with the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionEncoding {
    /// Columns count UTF-8 bytes.
    Utf8,
    /// Columns count UTF-16 code units.
    Utf16,
}

/// Precomputed line start offsets of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    text_len: usize,
}

/// Returns the width of a character in the given encoding.
///
/// # Arguments
/// * `character` - the character
/// * `encoding` - position encoding
fn width(character: char, encoding: PositionEncoding) -> usize {
    match encoding {
        PositionEncoding::Utf8 => character.len_utf8(),
        PositionEncoding::Utf16 => character.len_utf16(),
    }
}

impl LineIndex {
    /// Builds the index by scanning `text` for line breaks.
    ///
    /// # Arguments
    /// * `text` - the full document text
    pub fn new(text: &str) -> Self {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        Self {
            line_starts,
            text_len: text.len(),
        }
    }

    /// Converts a byte offset to an LSP position.
    ///
    /// # Arguments
    /// * `text` - the document text the index was built from
    /// * `offset` - byte offset; clamped to the text length and moved left to a char boundary
    /// * `encoding` - position encoding of the column
    pub fn position(&self, text: &str, offset: usize, encoding: PositionEncoding) -> Position {
        let offset = (0..=offset.min(self.text_len))
            .rev()
            .find(|&candidate| text.is_char_boundary(candidate))
            .unwrap_or(0);
        let line = self.line_of(offset);
        let line_start = self.line_starts.get(line).copied().unwrap_or(0);
        let column: usize = text[line_start..offset]
            .chars()
            .map(|character| width(character, encoding))
            .sum();
        Position::new(line as u32, column as u32)
    }

    /// Converts an LSP position to a byte offset.
    ///
    /// # Arguments
    /// * `text` - the document text the index was built from
    /// * `position` - LSP position
    /// * `encoding` - position encoding of the column
    ///
    /// # Returns
    /// The byte offset; a line past the end gives the text length and a column past the line end
    /// gives the end of the line content.
    pub fn offset(&self, text: &str, position: Position, encoding: PositionEncoding) -> usize {
        let Some(line_start) = self.line_start(position.line as usize) else {
            return self.text_len;
        };
        let content_end = self.content_end(text, position.line as usize);
        let target = position.character as usize;
        let advanced = text[line_start..content_end]
            .chars()
            .scan(0usize, |columns, character| {
                (*columns < target).then(|| {
                    *columns += width(character, encoding);
                    character.len_utf8()
                })
            })
            .sum::<usize>();
        line_start + advanced
    }

    /// Converts a byte range to an LSP range.
    ///
    /// # Arguments
    /// * `text` - the document text the index was built from
    /// * `range` - byte range
    /// * `encoding` - position encoding of the columns
    pub fn range(&self, text: &str, range: TextRange, encoding: PositionEncoding) -> Range {
        Range::new(
            self.position(text, range.start, encoding),
            self.position(text, range.end, encoding),
        )
    }

    /// Converts an LSP range to a byte range.
    ///
    /// # Arguments
    /// * `text` - the document text the index was built from
    /// * `range` - LSP range
    /// * `encoding` - position encoding of the columns
    pub fn text_range(&self, text: &str, range: Range, encoding: PositionEncoding) -> TextRange {
        TextRange::new(
            self.offset(text, range.start, encoding),
            self.offset(text, range.end, encoding),
        )
    }

    /// Returns the zero-based line containing `offset` (clamped to the last line).
    ///
    /// # Arguments
    /// * `offset` - byte offset
    pub fn line_of(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    }

    /// Returns the byte offset where `line` starts.
    ///
    /// # Arguments
    /// * `line` - zero-based line number
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.line_starts.get(line).copied()
    }

    /// Returns the end of the content of `line`, before `\r\n` or `\n`.
    ///
    /// # Arguments
    /// * `text` - the document text
    /// * `line` - zero-based line number
    fn content_end(&self, text: &str, line: usize) -> usize {
        let raw_end = self
            .line_starts
            .get(line + 1)
            .map_or(self.text_len, |next| next - 1);
        let has_carriage_return = line + 1 < self.line_starts.len()
            && raw_end > 0
            && text.as_bytes().get(raw_end - 1) == Some(&b'\r');
        raw_end - usize::from(has_carriage_return)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PositionEncoding::{Utf8, Utf16};

    fn pos(line: u32, character: u32) -> Position {
        Position::new(line, character)
    }

    #[test]
    fn ascii_round_trip() {
        let text = "abc\ndef\n";
        let index = LineIndex::new(text);
        for offset in 0..=text.len() {
            let position = index.position(text, offset, Utf16);
            assert_eq!(index.offset(text, position, Utf16), offset);
        }
        assert_eq!(index.position(text, 5, Utf16), pos(1, 1));
    }

    #[test]
    fn empty_text() {
        let index = LineIndex::new("");
        assert_eq!(index.line_starts.len(), 1);
        assert_eq!(index.position("", 0, Utf16), pos(0, 0));
        assert_eq!(index.position("", 10, Utf16), pos(0, 0));
        assert_eq!(index.offset("", pos(3, 3), Utf16), 0);
        assert_eq!(index.offset("", pos(0, 3), Utf16), 0);
    }

    #[test]
    fn trailing_newline_adds_empty_line() {
        let text = "a\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_starts.len(), 2);
        assert_eq!(index.position(text, 2, Utf8), pos(1, 0));
        assert_eq!(index.offset(text, pos(1, 0), Utf8), 2);
        assert_eq!(index.offset(text, pos(2, 0), Utf8), 2);
        assert_eq!(index.line_start(1), Some(2));
        assert_eq!(index.line_start(2), None);
    }

    #[test]
    fn utf16_versus_utf8_columns() {
        let text = "R1 a b 1µ Ω 😀 x";
        let index = LineIndex::new(text);
        let end = text.len();
        assert_eq!(index.position(text, end, Utf8), pos(0, end as u32));
        let units = text.encode_utf16().count() as u32;
        assert_eq!(index.position(text, end, Utf16), pos(0, units));
        assert!(units < end as u32);
        let emoji_offset = text.find('😀').unwrap_or(0);
        let emoji_column = index.position(text, emoji_offset, Utf16).character;
        assert_eq!(
            index.offset(text, pos(0, emoji_column + 2), Utf16),
            emoji_offset + 4
        );
        assert_eq!(
            index.offset(text, pos(0, emoji_column), Utf16),
            emoji_offset
        );
    }

    #[test]
    fn offset_inside_char_moves_left() {
        let text = "aµb";
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, 2, Utf8), pos(0, 1));
        assert_eq!(index.position(text, 3, Utf8), pos(0, 3));
        assert_eq!(index.position(text, 100, Utf16), pos(0, 3));
    }

    #[test]
    fn crlf_lines() {
        let text = "ab\r\ncd\r\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_starts.len(), 3);
        assert_eq!(index.position(text, 4, Utf8), pos(1, 0));
        assert_eq!(index.offset(text, pos(0, 99), Utf8), 2);
        assert_eq!(index.offset(text, pos(1, 99), Utf8), 6);
        assert_eq!(index.offset(text, pos(1, 1), Utf8), 5);
    }

    #[test]
    fn column_beyond_line_stops_before_newline() {
        let text = "abc\ndef";
        let index = LineIndex::new(text);
        assert_eq!(index.offset(text, pos(0, 50), Utf16), 3);
        assert_eq!(index.offset(text, pos(1, 50), Utf16), 7);
        assert_eq!(index.offset(text, pos(5, 0), Utf16), 7);
    }

    #[test]
    fn line_lookup() {
        let text = "a\nbb\nccc";
        let index = LineIndex::new(text);
        assert_eq!(index.line_of(0), 0);
        assert_eq!(index.line_of(1), 0);
        assert_eq!(index.line_of(2), 1);
        assert_eq!(index.line_of(5), 2);
        assert_eq!(index.line_of(999), 2);
        assert_eq!(index.line_start(2), Some(5));
    }

    #[test]
    fn range_conversion_round_trip() {
        let text = "* Ω comment\nR1 a b 1k\n";
        let index = LineIndex::new(text);
        let start = text.find("R1").unwrap_or(0);
        let range = TextRange::new(start, start + 2);
        for encoding in [Utf8, Utf16] {
            let lsp = index.range(text, range, encoding);
            assert_eq!(lsp, Range::new(pos(1, 0), pos(1, 2)));
            assert_eq!(index.text_range(text, lsp, encoding), range);
        }
        let omega = index.range(text, TextRange::new(2, 4), Utf8);
        assert_eq!(omega, Range::new(pos(0, 2), pos(0, 4)));
        let omega16 = index.range(text, TextRange::new(2, 4), Utf16);
        assert_eq!(omega16, Range::new(pos(0, 2), pos(0, 3)));
    }
}
