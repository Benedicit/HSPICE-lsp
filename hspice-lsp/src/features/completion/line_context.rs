//! Text-based scan of the logical netlist line before the cursor (robust against half-typed lines).

use crate::lang::commands::find_command;
use crate::lang::elements::find_source_function;
use crate::lang::functions::find_function;

/// A completed word of a line with its absolute byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    /// Word text (trimmed).
    pub text: String,
    /// First byte offset in the document.
    pub start: usize,
    /// One past the last byte offset in the document.
    pub end: usize,
}

/// What opened a scan frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// The logical line itself.
    Root,
    /// An unclosed `(`.
    Paren,
    /// An unclosed quote of the given character.
    Quote(char),
}

/// One nesting level (line, parenthesis or quote) at the cursor.
#[derive(Debug, Clone)]
pub struct Frame {
    /// What opened the frame.
    pub kind: FrameKind,
    /// Call name in front of a parenthesis (empty for bare groups).
    pub name: String,
    /// Absolute offset of the opening character.
    pub open: usize,
    /// True if arguments are separated by commas only (expression calls).
    pub comma_mode: bool,
    /// Completed words (or comma separated arguments) inside the frame.
    pub tokens: Vec<Tok>,
    /// Number of commas seen inside the frame.
    pub commas: usize,
    /// Start offset of the word that is still open.
    pub word_start: Option<usize>,
}

impl Frame {
    /// Creates an empty frame.
    ///
    /// # Arguments
    /// * `kind` - what opened the frame
    /// * `name` - call name in front of the parenthesis
    /// * `open` - absolute offset of the opening character
    /// * `comma_mode` - whether only commas separate arguments
    fn new(kind: FrameKind, name: &str, open: usize, comma_mode: bool) -> Self {
        Self {
            kind,
            name: name.to_string(),
            open,
            comma_mode,
            tokens: Vec::new(),
            commas: 0,
            word_start: None,
        }
    }

    /// Returns the index of the argument the cursor is in.
    pub fn argument_index(&self) -> usize {
        if self.comma_mode {
            self.commas
        } else {
            self.tokens.len()
        }
    }
}

/// The scanned logical line up to the cursor.
#[derive(Debug, Clone)]
pub struct LineScan {
    /// Absolute offset of the logical line start.
    pub line_start: usize,
    /// Absolute cursor offset.
    pub cursor: usize,
    /// Line text up to the cursor with comments and continuations blanked (same byte length as the source).
    pub text: String,
    /// Frame stack at the cursor; index 0 is the line.
    pub frames: Vec<Frame>,
}

impl LineScan {
    /// Returns the innermost frame at the cursor.
    pub fn innermost(&self) -> &Frame {
        self.frames.last().unwrap_or(&self.frames[0])
    }

    /// Returns the root (line) frame.
    pub fn root(&self) -> &Frame {
        &self.frames[0]
    }

    /// Returns true if the cursor is inside a quote.
    pub fn in_quote(&self) -> bool {
        self.frames
            .iter()
            .any(|frame| matches!(frame.kind, FrameKind::Quote(_)))
    }

    /// Returns the text of an absolute range of the scanned line.
    ///
    /// # Arguments
    /// * `start` - first byte offset
    /// * `end` - one past the last byte offset
    pub fn slice(&self, start: usize, end: usize) -> &str {
        self.text
            .get(start.saturating_sub(self.line_start)..end.saturating_sub(self.line_start))
            .unwrap_or_default()
    }

    /// Returns the first word of the line (possibly still being typed), lowercase.
    pub fn first_word(&self) -> String {
        first_word(self.root(), &self.text, self.line_start)
    }

    /// Returns the start offset of the word being typed in the innermost frame.
    pub fn word_start(&self) -> usize {
        self.innermost().word_start.unwrap_or(self.cursor)
    }

    /// Returns the word being typed in the innermost frame.
    pub fn current_word(&self) -> &str {
        self.slice(self.word_start(), self.cursor)
    }

    /// Returns the line's completed words after the first one that are not assignments.
    pub fn positionals(&self) -> Vec<&Tok> {
        self.root()
            .tokens
            .iter()
            .skip(1)
            .filter(|token| !token.text.contains('='))
            .collect()
    }

    /// Returns true if the line is a dot command.
    pub fn is_command(&self) -> bool {
        self.first_word().starts_with('.')
    }

    /// Returns true if the cursor is still inside the first word of the line.
    pub fn in_first_word(&self) -> bool {
        self.frames.len() == 1 && self.root().tokens.is_empty()
    }
}

/// Returns the lowercase first word of a line, which may still be open.
///
/// # Arguments
/// * `root` - root frame of the line
/// * `text` - scanned text
/// * `base` - absolute offset of `text[0]`
fn first_word(root: &Frame, text: &str, base: usize) -> String {
    root.tokens
        .first()
        .map(|token| token.text.as_str())
        .or_else(|| {
            root.word_start.map(|start| {
                text.get(start.saturating_sub(base)..)
                    .unwrap_or_default()
                    .split('(')
                    .next()
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Returns true if the line keyword starts a line whose values are expressions.
///
/// # Arguments
/// * `first` - lowercase first word of the line
pub fn is_expression_line(first: &str) -> bool {
    find_command(first).is_some_and(|command| matches!(command.name, ".param" | ".if" | ".elseif"))
}

/// Returns true if the first word names an independent source element.
///
/// # Arguments
/// * `first` - lowercase first word of the line
pub fn is_source_element(first: &str) -> bool {
    first.starts_with(['v', 'i'])
}

/// Returns the start offset of the logical line that contains `offset`.
///
/// # Arguments
/// * `text` - document text
/// * `offset` - byte offset
pub fn logical_line_start(text: &str, offset: usize) -> usize {
    let mut start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    while start > 0 && is_continuation(line_at(text, start)) {
        start = previous_statement_line(text, start);
    }
    start
}

/// Returns the line starting at `start` (without the newline).
///
/// # Arguments
/// * `text` - document text
/// * `start` - byte offset of a line start
fn line_at(text: &str, start: usize) -> &str {
    let rest = &text[start..];
    rest.split('\n').next().unwrap_or_default()
}

/// Returns true if a line starts with a `+` continuation marker.
///
/// # Arguments
/// * `line` - line text
fn is_continuation(line: &str) -> bool {
    line.trim_start().starts_with('+')
}

/// Returns true if a line is a full-line comment.
///
/// # Arguments
/// * `line` - line text
fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with('*')
}

/// Returns the start of the closest earlier line that is not a comment line.
///
/// # Arguments
/// * `text` - document text
/// * `start` - byte offset of a line start greater than zero
fn previous_statement_line(text: &str, start: usize) -> usize {
    let previous = text[..start - 1].rfind('\n').map_or(0, |index| index + 1);
    if previous > 0 && is_comment_line(line_at(text, previous)) {
        previous_statement_line(text, previous)
    } else {
        previous
    }
}

/// Replaces every character with spaces of the same byte length.
///
/// # Arguments
/// * `text` - text to blank out
fn blank(text: &str) -> String {
    text.chars()
        .map(|character| " ".repeat(character.len_utf8()))
        .collect()
}

/// Blanks a `$` comment (starting at whitespace or line start) and returns the cleaned line.
///
/// # Arguments
/// * `line` - one physical line (no newline)
///
/// # Returns
/// The cleaned line and whether a comment was found.
fn strip_inline_comment(line: &str) -> (String, bool) {
    let comment = line
        .char_indices()
        .find(|(index, character)| {
            *character == '$'
                && line[..*index]
                    .chars()
                    .next_back()
                    .is_none_or(char::is_whitespace)
        })
        .map(|(index, _)| index);
    match comment {
        Some(index) => (format!("{}{}", &line[..index], blank(&line[index..])), true),
        None => (line.to_string(), false),
    }
}

/// Cleans one physical line: comment lines and `$` comments blanked, `+` replaced by a space.
///
/// # Arguments
/// * `line` - one physical line (no newline)
///
/// # Returns
/// The cleaned line and whether the line end lies inside a comment.
fn clean_line(line: &str) -> (String, bool) {
    if is_comment_line(line) {
        return (blank(line), true);
    }
    let (stripped, commented) = strip_inline_comment(line);
    let cleaned = match stripped
        .find('+')
        .filter(|index| stripped[..*index].trim().is_empty())
    {
        Some(index) => format!("{} {}", &stripped[..index], &stripped[index + 1..]),
        None => stripped,
    };
    (cleaned, commented)
}

/// Builds the cleaned text of the logical line from its start to the cursor.
///
/// # Arguments
/// * `text` - document text
/// * `start` - logical line start
/// * `cursor` - cursor offset
///
/// # Returns
/// `None` if the cursor lies inside a comment.
fn cleaned_prefix(text: &str, start: usize, cursor: usize) -> Option<String> {
    let lines: Vec<&str> = text[start..cursor].split('\n').collect();
    let cleaned: Vec<(String, bool)> = lines
        .iter()
        .map(|line| match line.strip_suffix('\r') {
            Some(content) => {
                let (cleaned, commented) = clean_line(content);
                (cleaned + " ", commented)
            }
            None => clean_line(line),
        })
        .collect();
    let last_commented = cleaned.last().is_some_and(|(_, commented)| *commented);
    let comment_line_start = lines.last().is_some_and(|line| is_comment_line(line));
    (!last_commented && !comment_line_start).then(|| {
        cleaned
            .iter()
            .map(|(line, _)| line.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// Returns the identifier characters directly in front of `index`.
///
/// # Arguments
/// * `text` - scanned text
/// * `index` - index of a `(`
fn trailing_identifier(text: &str, index: usize) -> &str {
    &text[identifier_start(text, index)..index]
}

/// Scans the logical line around the cursor.
///
/// # Arguments
/// * `text` - document text
/// * `cursor` - cursor byte offset
///
/// # Returns
/// `None` if the cursor is inside a comment.
pub fn scan(text: &str, cursor: usize) -> Option<LineScan> {
    let cursor = cursor.min(text.len());
    let line_start = logical_line_start(text, cursor);
    let cleaned = cleaned_prefix(text, line_start, cursor)?;
    let frames = cleaned.char_indices().fold(
        vec![Frame::new(FrameKind::Root, "", line_start, false)],
        |frames, (index, character)| step(frames, &cleaned, line_start, index, character),
    );
    Some(LineScan {
        line_start,
        cursor,
        text: cleaned,
        frames,
    })
}

/// Opens the word of the innermost frame at `at` if none is open.
///
/// # Arguments
/// * `frames` - frame stack
/// * `at` - absolute offset
fn open_word(frames: &mut [Frame], at: usize) {
    if let Some(top) = frames.last_mut() {
        top.word_start.get_or_insert(at);
    }
}

/// Closes the open word of the innermost frame, recording it as a token.
///
/// # Arguments
/// * `frames` - frame stack
/// * `text` - scanned text
/// * `base` - absolute offset of `text[0]`
/// * `end` - absolute end offset of the word
fn close_word(frames: &mut [Frame], text: &str, base: usize, end: usize) {
    let Some(top) = frames.last_mut() else { return };
    let Some(start) = top.word_start.take() else {
        return;
    };
    let word = text
        .get(start - base..end - base)
        .unwrap_or_default()
        .trim();
    if !word.is_empty() {
        top.tokens.push(Tok {
            text: word.to_string(),
            start,
            end,
        });
    }
}

/// Returns true if whitespace at `index` belongs to a `key = value` assignment.
///
/// # Arguments
/// * `text` - scanned text
/// * `index` - byte index of the whitespace
fn glues_assignment(text: &str, index: usize) -> bool {
    text[..index].trim_end().ends_with('=') || text[index..].trim_start().starts_with('=')
}

/// Decides whether a new parenthesis frame separates arguments by commas only.
///
/// # Arguments
/// * `frames` - frame stack before the new frame
/// * `name` - call name in front of the parenthesis
/// * `text` - scanned text
/// * `base` - absolute offset of `text[0]`
fn opens_comma_mode(frames: &[Frame], name: &str, text: &str, base: usize) -> bool {
    let first = first_word(&frames[0], text, base);
    let in_quote = frames
        .iter()
        .any(|frame| matches!(frame.kind, FrameKind::Quote(_)));
    let source_call = is_source_element(&first) && find_source_function(name).is_some();
    in_quote
        || is_expression_line(&first)
        || (!name.is_empty() && find_function(name).is_some() && !source_call)
}

/// Advances the frame stack by one character.
///
/// # Arguments
/// * `frames` - frame stack
/// * `text` - scanned text
/// * `base` - absolute offset of `text[0]`
/// * `index` - byte index of the character
/// * `character` - the character
fn step(
    mut frames: Vec<Frame>,
    text: &str,
    base: usize,
    index: usize,
    character: char,
) -> Vec<Frame> {
    let at = base + index;
    if let Some(level) = frames
        .iter()
        .rposition(|frame| frame.kind == FrameKind::Quote(character))
    {
        frames.truncate(level);
        return frames;
    }
    let in_bare_quote = matches!(
        frames.last(),
        Some(Frame {
            kind: FrameKind::Quote(_),
            ..
        })
    );
    if in_bare_quote && !matches!(character, '(' | ')') {
        return frames;
    }
    match character {
        '\'' | '"' => {
            open_word(&mut frames, at);
            frames.push(Frame::new(FrameKind::Quote(character), "", at, false));
        }
        '(' => {
            open_word(&mut frames, at);
            let name = trailing_identifier(text, index);
            let comma_mode = opens_comma_mode(&frames, name, text, base);
            frames.push(Frame::new(FrameKind::Paren, name, at, comma_mode));
        }
        ')' => {
            if frames
                .last()
                .is_some_and(|frame| frame.kind == FrameKind::Paren)
            {
                frames.pop();
            }
        }
        ',' => {
            close_word(&mut frames, text, base, at);
            if let Some(top) = frames.last_mut() {
                top.commas += 1;
            }
        }
        _ if character.is_whitespace() => {
            let comma_mode = frames.last().is_some_and(|frame| frame.comma_mode);
            if !comma_mode && !glues_assignment(text, index) {
                close_word(&mut frames, text, base, at);
            }
        }
        _ => open_word(&mut frames, at),
    }
    frames
}

/// Returns the start of the identifier characters directly before `offset`.
///
/// # Arguments
/// * `text` - document text
/// * `offset` - cursor offset
pub fn identifier_start(text: &str, offset: usize) -> usize {
    offset
        - text[..offset]
            .chars()
            .rev()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '#')
            })
            .map(char::len_utf8)
            .sum::<usize>()
}

/// Returns the start of the node/element name characters directly before `offset`.
///
/// # Arguments
/// * `text` - document text
/// * `offset` - cursor offset
pub fn name_start(text: &str, offset: usize) -> usize {
    offset
        - text[..offset]
            .chars()
            .rev()
            .take_while(|character| {
                !character.is_whitespace() && !"()',\"=*/+^<>?:&|".contains(*character)
            })
            .map(char::len_utf8)
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_at_bar(text: &str) -> LineScan {
        let offset = text.find('|').unwrap_or_default();
        let clean = text.replacen('|', "", 1);
        scan(&clean, offset).expect("scan")
    }

    #[test]
    fn counts_positionals_across_continuations() {
        let result = scan_at_bar("* t\nM1 d g\n+ s b |");
        assert_eq!(result.first_word(), "m1");
        assert_eq!(result.positionals().len(), 4);
    }

    #[test]
    fn glues_spaced_assignments() {
        let result = scan_at_bar("R1 a b r = 1k |");
        assert_eq!(result.root().tokens.len(), 4);
        assert!(result.positionals().len() == 2);
    }

    #[test]
    fn tracks_open_calls_and_quotes() {
        let result = scan_at_bar("Vin a 0 pulse(0 1 |");
        assert_eq!(result.innermost().name, "pulse");
        assert_eq!(result.innermost().argument_index(), 2);
        let quoted = scan_at_bar("R1 a b 'max(w, 2*|");
        assert!(quoted.in_quote());
        assert_eq!(quoted.frames[1].kind, FrameKind::Quote('\''));
        assert_eq!(quoted.frames.len(), 3);
        assert_eq!(quoted.innermost().argument_index(), 1);
    }

    #[test]
    fn crlf_keeps_offsets_aligned() {
        let text = "* t\r\nM1 d g\r\n+ s b n";
        let result = scan(text, text.len()).expect("scan");
        assert_eq!(result.text.len(), text.len() - result.line_start);
        assert_eq!(result.current_word(), "n");
        assert_eq!(result.positionals().len(), 4);
    }

    #[test]
    fn comments_stop_the_scan() {
        assert!(scan("* t\n* note x", 12).is_none());
        assert!(scan("* t\nR1 a b $ note", 17).is_none());
    }
}
