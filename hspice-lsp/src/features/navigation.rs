//! Go to definition, references, document highlight and rename.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ls_types::{
    DocumentHighlight, DocumentHighlightKind, GotoDefinitionResponse, Location, Position,
    PrepareRenameResponse, Range, TextEdit, Uri, WorkspaceEdit,
};

use super::{symbol_location, to_lsp_range, to_offset};
use crate::analysis::includes::{read_source, summarize};
use crate::analysis::symbols::{Origin, ROOT_SCOPE, ScopeId, Symbol, SymbolId, SymbolKind};
use crate::analysis::{ExternalContext, IncludeResolution, analyze};
use crate::lang::elements::is_ground;
use crate::lang::functions::{find_function, find_special_variable};
use crate::state::document::{Document, parse_document};
use crate::state::line_index::LineIndex;
use crate::state::workspace::Workspace;
use crate::syntax::ast::TextRange;
use crate::syntax::parse::has_end_statement;

/// Symbol kinds whose definitions are visible in other files.
const GLOBAL_KINDS: [SymbolKind; 5] = [
    SymbolKind::Subckt,
    SymbolKind::Model,
    SymbolKind::Param,
    SymbolKind::Function,
    SymbolKind::LibSection,
];

/// The symbol (or group of binned models) under the cursor.
#[derive(Debug, Clone)]
struct Target {
    kind: SymbolKind,
    keys: Vec<String>,
    binned_base: Option<String>,
    cursor: TextRange,
    scope: ScopeId,
    global: bool,
    external: bool,
    home: Option<PathBuf>,
    local_ids: Vec<SymbolId>,
}

/// One occurrence of a target in some file.
#[derive(Debug, Clone)]
struct Occurrence {
    uri: Uri,
    range: Range,
    old: String,
    definition: bool,
}

/// Returns the base name of a binned model such as `nch.12`.
///
/// # Arguments
/// * `key` - lowercase model name
fn bin_base(key: &str) -> Option<String> {
    let (base, bin) = key.rsplit_once('.')?;
    (!base.is_empty() && !bin.is_empty() && bin.chars().all(|c| c.is_ascii_digit()))
        .then(|| base.to_string())
}

/// Returns the file that defines a symbol.
///
/// # Arguments
/// * `document` - document the symbol belongs to (for local symbols)
/// * `symbol` - the symbol
fn defining_path(document: &Document, symbol: &Symbol) -> Option<PathBuf> {
    match &symbol.origin {
        Origin::Local => document.path.clone(),
        Origin::External(file) => Some(file.path.clone()),
    }
}

/// Finds the rename/reference target at an offset, either a symbol or a `.lib` section name.
///
/// # Arguments
/// * `document` - document being queried
/// * `offset` - byte offset of the cursor
fn target_at(document: &Document, offset: usize) -> Option<Target> {
    section_call_target(document, offset).or_else(|| symbol_target(document, offset))
}

/// Builds the target for a section name in a `.lib 'file' section` call.
///
/// # Arguments
/// * `document` - document being queried
/// * `offset` - byte offset of the cursor
fn section_call_target(document: &Document, offset: usize) -> Option<Target> {
    let include = document.analysis.includes.iter().find(|include| {
        include
            .directive
            .section
            .as_ref()
            .is_some_and(|section| section.range.touches(offset))
    })?;
    let section = include.directive.section.as_ref()?;
    let home = include.resolved.clone()?;
    Some(Target {
        kind: SymbolKind::LibSection,
        keys: vec![section.key.clone()],
        binned_base: None,
        cursor: section.range,
        scope: ROOT_SCOPE,
        global: true,
        external: document.path.as_ref() != Some(&home),
        home: Some(home),
        local_ids: Vec::new(),
    })
}

/// Builds the target for the symbol under the cursor, grouping binned models.
///
/// # Arguments
/// * `document` - document being queried
/// * `offset` - byte offset of the cursor
fn symbol_target(document: &Document, offset: usize) -> Option<Target> {
    let analysis = &document.analysis;
    let id = analysis.symbol_at(offset)?;
    let symbol = analysis.symbols.get(id)?;
    let cursor = analysis
        .reference_at(offset)
        .map(|reference| reference.name.range)
        .unwrap_or(symbol.name.range);
    let base = (symbol.kind == SymbolKind::Model)
        .then(|| bin_base(&symbol.name.key))
        .flatten();
    let local_ids = base
        .as_deref()
        .map_or_else(|| vec![id], |base| analysis.table.bins(base).to_vec());
    let keys: Vec<String> = local_ids
        .iter()
        .filter_map(|member| analysis.symbols.get(*member))
        .map(|member| member.name.key.clone())
        .collect();
    Some(Target {
        kind: symbol.kind,
        keys,
        binned_base: base,
        cursor,
        scope: symbol.scope,
        global: symbol.scope == ROOT_SCOPE && GLOBAL_KINDS.contains(&symbol.kind),
        external: !symbol.is_local(),
        home: defining_path(document, symbol),
        local_ids,
    })
}

/// Returns the ids of the symbols in `document` that belong to the target.
///
/// # Arguments
/// * `document` - document to search
/// * `target` - the target
/// * `is_current` - true for the document the request came from
fn matching_ids(document: &Document, target: &Target, is_current: bool) -> Vec<SymbolId> {
    let by_home = target.global && target.home.is_some();
    document
        .analysis
        .symbols
        .iter()
        .enumerate()
        .filter(|(id, symbol)| {
            (is_current && target.local_ids.contains(id))
                || (by_home
                    && symbol.kind == target.kind
                    && symbol.scope == ROOT_SCOPE
                    && target.keys.contains(&symbol.name.key)
                    && defining_path(document, symbol) == target.home)
        })
        .map(|(id, _)| id)
        .collect()
}

/// Returns the section-name ranges of `.lib` calls in `document` that load the target section.
///
/// # Arguments
/// * `document` - document to search
/// * `target` - the target
fn section_call_ranges(document: &Document, target: &Target) -> Vec<TextRange> {
    let Some(home) = target
        .home
        .as_deref()
        .filter(|_| target.kind == SymbolKind::LibSection)
    else {
        return Vec::new();
    };
    document
        .analysis
        .includes
        .iter()
        .filter(|include| include.resolved.as_deref() == Some(home))
        .filter_map(|include| include.directive.section.as_ref())
        .filter(|section| target.keys.contains(&section.key))
        .map(|section| section.range)
        .collect()
}

/// Returns all ranges of the target in an open document, flagged as definition or not.
///
/// # Arguments
/// * `document` - document to search
/// * `target` - the target
/// * `is_current` - true for the document the request came from
fn document_ranges(
    document: &Document,
    target: &Target,
    is_current: bool,
) -> Vec<(TextRange, bool)> {
    let ids = matching_ids(document, target, is_current);
    let definitions: HashSet<TextRange> = ids
        .iter()
        .filter_map(|id| document.analysis.symbols.get(*id))
        .filter(|symbol| symbol.is_local())
        .map(|symbol| symbol.name.range)
        .collect();
    let mut ranges: Vec<TextRange> = ids
        .iter()
        .flat_map(|id| document.analysis.occurrences(*id))
        .chain(section_call_ranges(document, target))
        .collect();
    ranges.sort();
    ranges.dedup();
    ranges
        .into_iter()
        .map(|range| (range, definitions.contains(&range)))
        .collect()
}

/// Collects the occurrences of the target in an open document.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document to search
/// * `target` - the target
/// * `is_current` - true for the document the request came from
fn open_document_occurrences(
    workspace: &Workspace,
    document: &Document,
    target: &Target,
    is_current: bool,
) -> Vec<Occurrence> {
    document_ranges(document, target, is_current)
        .into_iter()
        .map(|(range, definition)| Occurrence {
            uri: document.uri.clone(),
            range: to_lsp_range(workspace, document, range),
            old: document.slice(range).to_string(),
            definition,
        })
        .collect()
}

/// Collects the occurrences of the target in its defining file when that file is not open.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `target` - the target
fn closed_home_occurrences(workspace: &Workspace, target: &Target) -> Vec<Occurrence> {
    let Some(home) = target.home.as_ref().filter(|_| target.global) else {
        return Vec::new();
    };
    if workspace
        .documents()
        .any(|document| document.path.as_ref() == Some(home))
    {
        return Vec::new();
    }
    let (Ok(text), Some(uri)) = (read_source(home), Uri::from_file_path(home)) else {
        return Vec::new();
    };
    let syntax = parse_document(&text, has_end_statement(&text));
    let analysis = analyze(&syntax.netlist, ExternalContext::default());
    let ids: Vec<SymbolId> = analysis
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, symbol)| {
            symbol.is_local()
                && symbol.kind == target.kind
                && symbol.scope == ROOT_SCOPE
                && target.keys.contains(&symbol.name.key)
        })
        .map(|(id, _)| id)
        .collect();
    let definitions: HashSet<TextRange> = ids
        .iter()
        .map(|id| analysis.symbols[*id].name.range)
        .collect();
    let mut ranges: Vec<TextRange> = ids
        .iter()
        .flat_map(|id| analysis.occurrences(*id))
        .collect();
    ranges.sort();
    ranges.dedup();
    let line_index = LineIndex::new(&text);
    ranges
        .into_iter()
        .map(|range| Occurrence {
            uri: uri.clone(),
            range: line_index.range(&text, range, workspace.encoding),
            old: text
                .get(range.start..range.end)
                .unwrap_or_default()
                .to_string(),
            definition: definitions.contains(&range),
        })
        .collect()
}

/// Collects occurrences in the current document, other open documents and the closed defining file.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document the request came from
/// * `target` - the target
fn collect_occurrences(
    workspace: &Workspace,
    document: &Document,
    target: &Target,
) -> Vec<Occurrence> {
    let global = target.global && target.home.is_some();
    let others = workspace
        .documents()
        .filter(|other| other.uri != document.uri)
        .filter(|_| global)
        .flat_map(|other| open_document_occurrences(workspace, other, target, false));
    let mut occurrences: Vec<Occurrence> =
        open_document_occurrences(workspace, document, target, true)
            .into_iter()
            .chain(others)
            .chain(closed_home_occurrences(workspace, target))
            .collect();
    let mut seen = HashSet::new();
    occurrences.retain(|occurrence| {
        seen.insert((
            occurrence.uri.as_str().to_string(),
            occurrence.range.start.line,
            occurrence.range.start.character,
            occurrence.range.end.character,
        ))
    });
    occurrences
}

/// Finds the definition of the symbol (or include path) under the cursor.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
pub fn goto_definition(
    workspace: &Workspace,
    document: &Document,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = to_offset(workspace, document, position);
    include_path_location(document, offset)
        .or_else(|| section_location(workspace, document, offset))
        .or_else(|| symbol_definitions(workspace, document, offset))
}

/// Resolves the path of an include directive under the cursor to the file start.
///
/// # Arguments
/// * `document` - document being edited
/// * `offset` - byte offset of the cursor
fn include_path_location(document: &Document, offset: usize) -> Option<GotoDefinitionResponse> {
    let include = document
        .analysis
        .includes
        .iter()
        .find(|include| include.directive.path.range.touches(offset))?;
    let uri = Uri::from_file_path(include.resolved.as_ref()?)?;
    Some(GotoDefinitionResponse::Scalar(Location {
        uri,
        range: Range::default(),
    }))
}

/// Resolves the section name of a `.lib 'file' section` call to the section definition.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `offset` - byte offset of the cursor
fn section_location(
    workspace: &Workspace,
    document: &Document,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    let include: &IncludeResolution = document.analysis.includes.iter().find(|include| {
        include
            .directive
            .section
            .as_ref()
            .is_some_and(|section| section.range.touches(offset))
    })?;
    let section = include.directive.section.as_ref()?;
    let path = include.resolved.as_ref()?;
    let text = file_text(workspace, path)?;
    let summary = summarize(path, text);
    let symbol = summary
        .symbols
        .iter()
        .find(|symbol| symbol.kind == SymbolKind::LibSection && symbol.name.key == section.key)?;
    let range =
        summary
            .file
            .line_index
            .range(&summary.file.text, symbol.name.range, workspace.encoding);
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: Uri::from_file_path(path)?,
        range,
    }))
}

/// Returns the text of a file, preferring the open editor buffer over the disk.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `path` - file path
fn file_text(workspace: &Workspace, path: &Path) -> Option<String> {
    workspace
        .documents()
        .find(|document| document.path.as_deref() == Some(path))
        .map(|document| document.text.clone())
        .or_else(|| read_source(path).ok())
}

/// Locates the definition(s) of the symbol under the cursor; binned base names give all bins.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `offset` - byte offset of the cursor
fn symbol_definitions(
    workspace: &Workspace,
    document: &Document,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    let analysis = &document.analysis;
    let id = analysis.symbol_at(offset)?;
    let symbol = analysis.symbols.get(id)?;
    let bins: Vec<SymbolId> = analysis
        .reference_at(offset)
        .filter(|reference| {
            reference.kind == SymbolKind::Model && reference.name.key != symbol.name.key
        })
        .map(|reference| analysis.table.bins(&reference.name.key).to_vec())
        .unwrap_or_default();
    let ids = if bins.is_empty() { vec![id] } else { bins };
    let mut locations: Vec<Location> = ids
        .iter()
        .filter_map(|member| analysis.symbols.get(*member))
        .filter_map(|member| symbol_location(workspace, document, member))
        .collect();
    match locations.len() {
        0 => None,
        1 => locations.pop().map(GotoDefinitionResponse::Scalar),
        _ => Some(GotoDefinitionResponse::Array(locations)),
    }
}

/// Finds all references of the symbol under the cursor.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
/// * `include_declaration` - whether the definition itself is included
pub fn references(
    workspace: &Workspace,
    document: &Document,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let target = target_at(document, to_offset(workspace, document, position))?;
    let locations = collect_occurrences(workspace, document, &target)
        .into_iter()
        .filter(|occurrence| include_declaration || !occurrence.definition)
        .map(|occurrence| Location {
            uri: occurrence.uri,
            range: occurrence.range,
        })
        .collect();
    Some(locations)
}

/// Highlights all occurrences of the symbol under the cursor in the document.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
pub fn document_highlight(
    workspace: &Workspace,
    document: &Document,
    position: Position,
) -> Option<Vec<DocumentHighlight>> {
    let target = target_at(document, to_offset(workspace, document, position))?;
    let highlights = document_ranges(document, &target, true)
        .into_iter()
        .map(|(range, definition)| DocumentHighlight {
            range: to_lsp_range(workspace, document, range),
            kind: Some(if definition {
                DocumentHighlightKind::WRITE
            } else {
                DocumentHighlightKind::READ
            }),
        })
        .collect();
    Some(highlights)
}

/// Checks whether the symbol under the cursor can be renamed and returns its range.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
pub fn prepare_rename(
    workspace: &Workspace,
    document: &Document,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let target = target_at(document, to_offset(workspace, document, position))?;
    check_renamable(workspace, &target).ok()?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: to_lsp_range(workspace, document, target.cursor),
        placeholder: document.slice(target.cursor).to_string(),
    })
}

/// Renames the symbol under the cursor.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
/// * `new_name` - new name
///
/// # Returns
/// `Err` with a user-facing message if the rename is not possible.
pub fn rename(
    workspace: &Workspace,
    document: &Document,
    position: Position,
    new_name: &str,
) -> Result<Option<WorkspaceEdit>, String> {
    let offset = to_offset(workspace, document, position);
    let target = target_at(document, offset).ok_or_else(|| no_target_message(document, offset))?;
    check_renamable(workspace, &target)?;
    let name = validate_new_name(document, &target, new_name)?;
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    collect_occurrences(workspace, document, &target)
        .into_iter()
        .for_each(|occurrence| {
            let text = replacement(&target, &occurrence.old, &name);
            changes.entry(occurrence.uri).or_default().push(TextEdit {
                range: occurrence.range,
                new_text: text,
            });
        });
    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    }))
}

/// Checks that the definition of the target may be edited.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `target` - the rename target
fn check_renamable(workspace: &Workspace, target: &Target) -> Result<(), String> {
    if !target.external {
        return Ok(());
    }
    let inside_root = match (&target.home, &workspace.root) {
        (Some(home), Some(root)) => home.starts_with(root),
        _ => false,
    };
    if inside_root {
        return Ok(());
    }
    let location = target.home.as_ref().map_or_else(
        || "another file".to_string(),
        |home| home.display().to_string(),
    );
    Err(format!(
        "Cannot rename: the definition lives outside the workspace ({location})"
    ))
}

/// Explains why nothing can be renamed at an offset.
///
/// # Arguments
/// * `document` - document being edited
/// * `offset` - byte offset of the cursor
fn no_target_message(document: &Document, offset: usize) -> String {
    let word = word_at(&document.text, offset);
    if is_ground(&word) {
        return "Cannot rename the ground node".to_string();
    }
    if find_function(&word).is_some() || find_special_variable(&word).is_some() {
        return format!("Cannot rename built-in `{word}`");
    }
    "Cannot rename: no resolved symbol at this position".to_string()
}

/// Returns the delimiter-free word around an offset.
///
/// # Arguments
/// * `text` - document text
/// * `offset` - byte offset
fn word_at(text: &str, offset: usize) -> String {
    let is_delimiter = |c: char| c.is_whitespace() || ",=()'\"".contains(c);
    let offset = offset.min(text.len());
    let start = text[..offset]
        .rfind(is_delimiter)
        .map_or(0, |index| index + 1);
    let end = text[offset..]
        .find(is_delimiter)
        .map_or(text.len(), |index| offset + index);
    text.get(start..end).unwrap_or_default().to_string()
}

/// Validates the requested new name for the target.
///
/// # Arguments
/// * `document` - document being edited
/// * `target` - the rename target
/// * `new_name` - requested name
///
/// # Returns
/// The accepted name (for binned models without the bin suffix).
fn validate_new_name(
    document: &Document,
    target: &Target,
    new_name: &str,
) -> Result<String, String> {
    let name = new_name.trim();
    let first = name
        .chars()
        .next()
        .ok_or("The new name must not be empty")?;
    if name
        .chars()
        .any(|c| c.is_whitespace() || ",=()'\"".contains(c))
    {
        return Err("The new name must not contain whitespace or any of , = ( ) ' \"".to_string());
    }
    if "*+$.".contains(first) {
        return Err(format!("The new name must not start with `{first}`"));
    }
    if first.is_ascii_digit() && target.kind != SymbolKind::Node {
        return Err("The new name must not start with a digit".to_string());
    }
    if target.kind == SymbolKind::Node && is_ground(name) {
        return Err("Cannot rename a node to a ground name".to_string());
    }
    let old = document.slice(target.cursor);
    let old_first = old.chars().next().unwrap_or(first);
    if target.kind == SymbolKind::Element && !first.eq_ignore_ascii_case(&old_first) {
        return Err(format!(
            "Element names must keep their type letter `{}`",
            old_first.to_ascii_uppercase()
        ));
    }
    let base_name = target
        .binned_base
        .as_ref()
        .map_or(name.to_string(), |_| strip_bin(name));
    check_collision(document, target, &base_name)?;
    Ok(base_name)
}

/// Removes a trailing `.N` bin suffix from a name.
///
/// # Arguments
/// * `name` - model name as typed
fn strip_bin(name: &str) -> String {
    bin_base(&name.to_ascii_lowercase())
        .and_then(|base| name.get(..base.len()))
        .unwrap_or(name)
        .to_string()
}

/// Rejects names that already exist in the same scope.
///
/// # Arguments
/// * `document` - document being edited
/// * `target` - the rename target
/// * `name` - new (base) name
fn check_collision(document: &Document, target: &Target, name: &str) -> Result<(), String> {
    let key = name.to_ascii_lowercase();
    let table = &document.analysis.table;
    let old_keys: Vec<&str> = target.keys.iter().map(String::as_str).collect();
    let exists = match &target.binned_base {
        Some(_) => {
            !table.bins(&key).is_empty()
                && !old_keys
                    .iter()
                    .any(|old| bin_base(old).as_deref() == Some(key.as_str()))
        }
        None => {
            !old_keys.contains(&key.as_str())
                && !table.in_scope(target.kind, target.scope, &key).is_empty()
        }
    };
    if exists {
        Err(format!("`{name}` already exists in this scope"))
    } else {
        Ok(())
    }
}

/// Computes the replacement for one occurrence, keeping bin suffixes of binned models.
///
/// # Arguments
/// * `target` - the rename target
/// * `old` - occurrence text
/// * `name` - new (base) name
fn replacement(target: &Target, old: &str, name: &str) -> String {
    let suffix = target
        .binned_base
        .as_ref()
        .and_then(|base| old.get(base.len()..))
        .unwrap_or_default();
    format!("{name}{suffix}")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::state::line_index::PositionEncoding;
    use std::str::FromStr;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn workspace_with_root(root: Option<PathBuf>) -> Workspace {
        Workspace::new(root, PositionEncoding::Utf16, Settings::default())
    }

    fn open_text(workspace: &mut Workspace, uri: &str, text: &str) -> Uri {
        let uri = Uri::from_str(uri).expect("uri");
        workspace.update(uri.clone(), 1, text.to_string());
        uri
    }

    fn open_fixture(workspace: &mut Workspace, relative: &str) -> Uri {
        let path = fixtures().join(relative);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let uri = Uri::from_file_path(&path).expect("file uri");
        workspace.update(uri.clone(), 1, text);
        uri
    }

    fn position_of(document: &Document, needle: &str, nth: usize) -> Position {
        let offset = document
            .text
            .match_indices(needle)
            .nth(nth)
            .map_or(0, |(index, _)| index);
        document
            .line_index
            .position(&document.text, offset + 1, PositionEncoding::Utf16)
    }

    fn rename_in(
        workspace: &Workspace,
        uri: &Uri,
        needle: &str,
        nth: usize,
        new_name: &str,
    ) -> Result<Option<WorkspaceEdit>, String> {
        let document = workspace.document(uri).expect("document");
        rename(
            workspace,
            document,
            position_of(document, needle, nth),
            new_name,
        )
    }

    fn edit_count(edit: &WorkspaceEdit, uri: &Uri) -> usize {
        edit.changes
            .as_ref()
            .and_then(|changes| changes.get(uri))
            .map_or(0, Vec::len)
    }

    const NETLIST: &str = "t\n.param wn=1u\n.subckt s a b w=2\nR1 a b 'w*wn'\n.ends s\nX1 n 0 s w=wn\nR2 n 0 'wn'\n.end\n";

    #[test]
    fn goto_definition_of_param_reference() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(&mut workspace, "file:///tmp/t.sp", NETLIST);
        let document = workspace.document(&uri).expect("document");
        let response = goto_definition(&workspace, document, position_of(document, "'wn'", 0));
        let Some(GotoDefinitionResponse::Scalar(location)) = response else {
            panic!("expected scalar, got {response:?}");
        };
        assert_eq!(location.range.start.line, 1);
    }

    #[test]
    fn goto_definition_on_ends_name_finds_subckt() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(&mut workspace, "file:///tmp/t.sp", NETLIST);
        let document = workspace.document(&uri).expect("document");
        let at_name = Position::new(4, 6);
        let Some(GotoDefinitionResponse::Scalar(location)) =
            goto_definition(&workspace, document, at_name)
        else {
            panic!("no definition");
        };
        assert_eq!(location.range.start.line, 2);
    }

    #[test]
    fn goto_definition_across_files_and_bins() {
        let mut workspace = workspace_with_root(Some(fixtures()));
        let tb = open_fixture(&mut workspace, "inverter_tb.sp");
        let inc = open_fixture(&mut workspace, "cells/inv.inc");
        let testbench = workspace.document(&tb).expect("tb");
        let Some(GotoDefinitionResponse::Scalar(subckt)) =
            goto_definition(&workspace, testbench, position_of(testbench, " inv wn", 0))
        else {
            panic!("no subckt definition");
        };
        assert!(subckt.uri.as_str().ends_with("cells/inv.inc"));
        let include = workspace.document(&inc).expect("inc");
        let Some(GotoDefinitionResponse::Array(bins)) =
            goto_definition(&workspace, include, position_of(include, " nch ", 0))
        else {
            panic!("expected bins");
        };
        assert_eq!(bins.len(), 2);
        assert!(
            bins.iter()
                .all(|bin| bin.uri.as_str().ends_with("models/ptm45.pm"))
        );
    }

    #[test]
    fn goto_definition_on_include_path_and_section() {
        let mut workspace = workspace_with_root(Some(fixtures()));
        let tb = open_fixture(&mut workspace, "inverter_tb.sp");
        let document = workspace.document(&tb).expect("tb");
        let Some(GotoDefinitionResponse::Scalar(file)) = goto_definition(
            &workspace,
            document,
            position_of(document, "cells/inv.inc", 0),
        ) else {
            panic!("no include target");
        };
        assert!(file.uri.as_str().ends_with("cells/inv.inc"));
        assert_eq!(file.range, Range::default());
        let Some(GotoDefinitionResponse::Scalar(section)) =
            goto_definition(&workspace, document, position_of(document, " tt", 0))
        else {
            panic!("no section target");
        };
        assert!(section.uri.as_str().ends_with("models/tech.lib"));
        assert_eq!(section.range.start.line, 1);
    }

    #[test]
    fn references_and_highlights_in_document() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(&mut workspace, "file:///tmp/t.sp", NETLIST);
        let document = workspace.document(&uri).expect("document");
        let position = position_of(document, "'wn'", 0);
        let all = references(&workspace, document, position, true).unwrap_or_default();
        let without = references(&workspace, document, position, false).unwrap_or_default();
        assert_eq!(all.len(), 4);
        assert_eq!(without.len(), 3);
        let highlights = document_highlight(&workspace, document, position).unwrap_or_default();
        assert_eq!(highlights.len(), 4);
        assert_eq!(
            highlights
                .iter()
                .filter(|h| h.kind == Some(DocumentHighlightKind::WRITE))
                .count(),
            1
        );
    }

    #[test]
    fn references_include_other_open_documents() {
        let mut workspace = workspace_with_root(Some(fixtures()));
        let tb = open_fixture(&mut workspace, "inverter_tb.sp");
        open_fixture(&mut workspace, "cells/inv.inc");
        let document = workspace.document(&tb).expect("tb");
        let found = references(
            &workspace,
            document,
            position_of(document, " inv wn", 0),
            true,
        )
        .unwrap_or_default();
        assert!(
            found
                .iter()
                .any(|location| location.uri.as_str().ends_with("cells/inv.inc")),
            "{found:?}"
        );
        assert!(found.iter().any(|location| location.uri == tb));
    }

    #[test]
    fn rename_param_in_document() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(&mut workspace, "file:///tmp/t.sp", NETLIST);
        let document = workspace.document(&uri).expect("document");
        let prepared = prepare_rename(&workspace, document, position_of(document, "'wn'", 0));
        assert!(
            matches!(prepared, Some(PrepareRenameResponse::RangeWithPlaceholder { ref placeholder, .. }) if placeholder == "wn")
        );
        let edit = rename_in(&workspace, &uri, "'wn'", 0, "width")
            .expect("ok")
            .expect("edit");
        assert_eq!(edit_count(&edit, &uri), 4);
    }

    #[test]
    fn rename_element_keeps_type_letter() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(
            &mut workspace,
            "file:///tmp/t.sp",
            "t\nR1 a 0 1k\nVs a 0 1\n.probe i(vs)\n.end\n",
        );
        assert!(rename_in(&workspace, &uri, "R1", 0, "C9").is_err());
        let edit = rename_in(&workspace, &uri, "Vs a", 0, "Vsup")
            .expect("ok")
            .expect("edit");
        assert_eq!(edit_count(&edit, &uri), 2);
    }

    #[test]
    fn rename_validation_errors() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(&mut workspace, "file:///tmp/t.sp", NETLIST);
        for bad in ["", "a b", "1abc", "a=b", "x(", "'q'"] {
            assert!(
                rename_in(&workspace, &uri, "'wn'", 0, bad).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn rename_to_existing_name_is_rejected() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(
            &mut workspace,
            "file:///tmp/t.sp",
            "t\n.param a=1 b=2\nR1 x 0 'a+b'\n.end\n",
        );
        let message = rename_in(&workspace, &uri, "'a+b'", 0, "b")
            .err()
            .unwrap_or_default();
        assert!(message.contains("already exists"), "{message}");
    }

    #[test]
    fn rename_ground_and_builtin_are_rejected() {
        let mut workspace = workspace_with_root(None);
        let uri = open_text(
            &mut workspace,
            "file:///tmp/t.sp",
            "t\nR1 a 0 'sin(1)'\n.end\n",
        );
        let ground = rename_in(&workspace, &uri, " 0 ", 0, "n");
        assert!(ground.is_err());
        let builtin = rename_in(&workspace, &uri, "sin", 0, "n");
        assert!(builtin.is_err());
    }

    #[test]
    fn rename_binned_models_consistently() {
        let mut workspace = workspace_with_root(Some(fixtures()));
        let inc = open_fixture(&mut workspace, "cells/inv.inc");
        let document = workspace.document(&inc).expect("inc");
        let edit = rename(
            &workspace,
            document,
            position_of(document, " nch ", 0),
            "nmod",
        )
        .expect("ok")
        .expect("edit");
        let changes = edit.changes.expect("changes");
        assert_eq!(changes.get(&inc).map_or(0, Vec::len), 1);
        let pm = changes
            .iter()
            .find(|(uri, _)| uri.as_str().ends_with("ptm45.pm"))
            .map(|(_, edits)| edits);
        let texts: Vec<&str> = pm
            .map(|edits| edits.iter().map(|edit| edit.new_text.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(texts.len(), 2, "{changes:?}");
        assert!(texts.contains(&"nmod.1") && texts.contains(&"nmod.2"));
    }

    #[test]
    fn rename_subckt_edits_external_definition_and_ends() {
        let mut workspace = workspace_with_root(Some(fixtures()));
        let tb = open_fixture(&mut workspace, "inverter_tb.sp");
        let edit = rename_in(&workspace, &tb, " inv wn", 0, "inv2")
            .expect("ok")
            .expect("edit");
        let changes = edit.changes.expect("changes");
        let inc_edits = changes
            .iter()
            .find(|(uri, _)| uri.as_str().ends_with("cells/inv.inc"))
            .map_or(0, |(_, edits)| edits.len());
        assert_eq!(inc_edits, 2);
        assert_eq!(changes.get(&tb).map_or(0, Vec::len), 1);
    }

    #[test]
    fn rename_outside_root_is_rejected() {
        let mut workspace = workspace_with_root(Some(fixtures().join("models")));
        let tb = open_fixture(&mut workspace, "inverter_tb.sp");
        let document = workspace.document(&tb).expect("tb");
        let position = position_of(document, " inv wn", 0);
        let outcome = rename(&workspace, document, position, "inv2");
        let message = outcome.err().unwrap_or_default();
        assert!(message.contains("outside"), "{message}");
        assert!(prepare_rename(&workspace, document, position).is_none());
    }

    #[test]
    fn rename_wn_updates_all_references_in_testbench() {
        let mut workspace = workspace_with_root(Some(fixtures()));
        let tb = open_fixture(&mut workspace, "inverter_tb.sp");
        let edit = rename_in(&workspace, &tb, "wn", 0, "wnn")
            .expect("ok")
            .expect("edit");
        assert!(edit_count(&edit, &tb) >= 3);
    }

    #[test]
    fn replacement_keeps_suffix_only_for_bins() {
        let target = Target {
            kind: SymbolKind::Model,
            keys: vec!["nch.1".into()],
            binned_base: Some("nch".into()),
            cursor: TextRange::default(),
            scope: ROOT_SCOPE,
            global: true,
            external: false,
            home: None,
            local_ids: vec![],
        };
        assert_eq!(replacement(&target, "NCH.12", "new"), "new.12");
        assert_eq!(replacement(&target, "nch", "new"), "new");
        assert_eq!(strip_bin("New.3"), "New");
        assert_eq!(strip_bin("New"), "New");
    }
}
