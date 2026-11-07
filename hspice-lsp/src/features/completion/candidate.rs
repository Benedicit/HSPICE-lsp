//! Completion candidates and their conversion into LSP completion items.

use ls_types::{
    CompletionItem, CompletionItemKind, CompletionItemTag, CompletionTextEdit, Documentation,
    InsertTextFormat, MarkupContent, MarkupKind, Range, TextEdit,
};

/// One completion candidate before the replacement range is known.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Text shown in the menu.
    pub label: String,
    /// LSP item kind.
    pub kind: CompletionItemKind,
    /// Short detail line.
    pub detail: Option<String>,
    /// Markdown documentation.
    pub documentation: Option<String>,
    /// Text inserted in place of the typed prefix.
    pub insert: String,
    /// True if `insert` is a snippet.
    pub snippet: bool,
    /// Text the client matches the typed prefix against (defaults to the label).
    pub filter: Option<String>,
    /// Sort key.
    pub sort: String,
    /// True for obsolete entries.
    pub deprecated: bool,
}

impl Candidate {
    /// Creates a plain-text candidate that inserts its label.
    ///
    /// # Arguments
    /// * `label` - menu text, also the inserted text
    /// * `kind` - LSP item kind
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        let label = label.into();
        Self {
            insert: label.clone(),
            label,
            kind,
            detail: None,
            documentation: None,
            snippet: false,
            filter: None,
            sort: "5".to_string(),
            deprecated: false,
        }
    }

    /// Sets the detail line.
    ///
    /// # Arguments
    /// * `detail` - detail text
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Sets the Markdown documentation.
    ///
    /// # Arguments
    /// * `documentation` - Markdown text
    pub fn documentation(mut self, documentation: impl Into<String>) -> Self {
        self.documentation = Some(documentation.into());
        self
    }

    /// Sets a plain-text insertion that differs from the label.
    ///
    /// # Arguments
    /// * `insert` - inserted text
    pub fn insert(mut self, insert: impl Into<String>) -> Self {
        self.insert = insert.into();
        self.snippet = false;
        self
    }

    /// Sets a snippet insertion.
    ///
    /// # Arguments
    /// * `snippet` - LSP snippet text
    pub fn snippet(mut self, snippet: impl Into<String>) -> Self {
        self.insert = snippet.into();
        self.snippet = true;
        self
    }

    /// Sets the filter text.
    ///
    /// # Arguments
    /// * `filter` - text matched against the typed prefix
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Sets the sort key (compared as text).
    ///
    /// # Arguments
    /// * `sort` - sort key
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.sort = sort.into();
        self
    }

    /// Marks the candidate as obsolete.
    ///
    /// # Arguments
    /// * `deprecated` - whether the entry is obsolete
    pub fn deprecated(mut self, deprecated: bool) -> Self {
        self.deprecated = deprecated;
        self
    }

    /// Returns true if the typed prefix matches the start of the candidate (case-insensitive, any space-separated word of the filter text).
    ///
    /// # Arguments
    /// * `prefix` - typed prefix
    pub fn matches(&self, prefix: &str) -> bool {
        let target = self
            .filter
            .as_deref()
            .unwrap_or(&self.label)
            .to_ascii_lowercase();
        let typed = prefix.to_ascii_lowercase();
        target
            .split_whitespace()
            .any(|word| word.starts_with(&typed))
    }

    /// Converts the candidate into an LSP item that replaces `range`.
    ///
    /// # Arguments
    /// * `range` - range of the typed prefix
    pub fn into_item(self, range: Range) -> CompletionItem {
        let sort_text = format!("{}{}", self.sort, self.label.to_ascii_lowercase());
        CompletionItem {
            filter_text: self.filter,
            sort_text: Some(sort_text),
            kind: Some(self.kind),
            detail: self.detail,
            documentation: self.documentation.map(|value| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value,
                })
            }),
            insert_text_format: self.snippet.then_some(InsertTextFormat::SNIPPET),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: self.insert,
            })),
            deprecated: self.deprecated.then_some(true),
            tags: self.deprecated.then(|| vec![CompletionItemTag::DEPRECATED]),
            label: self.label,
            ..CompletionItem::default()
        }
    }
}

/// Escapes characters that are special inside LSP snippets.
///
/// # Arguments
/// * `text` - literal text
pub fn escape_snippet(text: &str) -> String {
    text.chars()
        .flat_map(|character| match character {
            '$' | '}' | '\\' => vec!['\\', character],
            _ => vec![character],
        })
        .collect()
}
