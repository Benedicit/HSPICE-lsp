//! Document and workspace symbols.

use ls_types::{
    DocumentSymbol, DocumentSymbolResponse, Location, SymbolInformation,
    SymbolKind as LspSymbolKind, WorkspaceSymbolResponse,
};

use super::to_lsp_range;
use crate::analysis::blocks::BlockKind;
use crate::analysis::symbols::{ParamSource, Symbol, SymbolDetail, SymbolKind};
use crate::lang::commands::{CommandCategory, find_command};
use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::{CommandKind, StatementKind, TextRange};

/// An outline entry before conversion to LSP types.
struct Item {
    name: String,
    detail: Option<String>,
    kind: LspSymbolKind,
    range: TextRange,
    selection: TextRange,
}

/// Returns the hierarchical outline of a document.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document to outline
pub fn document_symbols(
    workspace: &Workspace,
    document: &Document,
) -> Option<DocumentSymbolResponse> {
    let mut items: Vec<Item> = symbol_items(document)
        .chain(analysis_items(document))
        .collect();
    items.sort_by_key(|item| (item.range.start, std::cmp::Reverse(item.range.end)));
    Some(DocumentSymbolResponse::Nested(nest(
        workspace, document, &items,
    )))
}

/// Builds the symbol tree from items sorted by start and descending end.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document the items belong to
/// * `items` - sorted items; an item contains the directly following items inside its range
fn nest(workspace: &Workspace, document: &Document, items: &[Item]) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    let mut index = 0;
    while let Some(item) = items.get(index) {
        let end = items[index + 1..]
            .iter()
            .position(|next| !item.range.contains_range(next.range))
            .map_or(items.len(), |offset| index + 1 + offset);
        let children = nest(workspace, document, &items[index + 1..end]);
        symbols.push(document_symbol(workspace, document, item, children));
        index = end;
    }
    symbols
}

/// Converts one item into a `DocumentSymbol`.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document the item belongs to
/// * `item` - outline item
/// * `children` - nested symbols
#[allow(deprecated)]
fn document_symbol(
    workspace: &Workspace,
    document: &Document,
    item: &Item,
    children: Vec<DocumentSymbol>,
) -> DocumentSymbol {
    DocumentSymbol {
        name: item.name.clone(),
        detail: item.detail.clone(),
        kind: item.kind,
        tags: None,
        deprecated: None,
        range: to_lsp_range(workspace, document, item.range),
        selection_range: to_lsp_range(workspace, document, item.selection),
        children: (!children.is_empty()).then_some(children),
    }
}

/// Returns the LSP kind of a symbol, or `None` if it does not appear in the outline.
///
/// # Arguments
/// * `symbol` - local symbol
fn outline_kind(symbol: &Symbol) -> Option<LspSymbolKind> {
    match (symbol.kind, &symbol.detail) {
        (SymbolKind::Subckt, _) => Some(LspSymbolKind::CLASS),
        (SymbolKind::Model, _) => Some(LspSymbolKind::STRUCT),
        (SymbolKind::Function, _) => Some(LspSymbolKind::FUNCTION),
        (SymbolKind::LibSection, _) => Some(LspSymbolKind::NAMESPACE),
        (
            SymbolKind::Param,
            SymbolDetail::Param {
                source: ParamSource::SubcktHeader,
                ..
            },
        ) => Some(LspSymbolKind::VARIABLE),
        (
            SymbolKind::Param,
            SymbolDetail::Param {
                source: ParamSource::DataColumn,
                ..
            },
        ) => None,
        (SymbolKind::Param, _) => Some(LspSymbolKind::CONSTANT),
        (SymbolKind::Element, _) => Some(LspSymbolKind::OBJECT),
        (SymbolKind::Measure, _) => Some(LspSymbolKind::FIELD),
        (SymbolKind::DataBlock, _) => Some(LspSymbolKind::ARRAY),
        (SymbolKind::Node, _) => None,
    }
}

/// Returns a short detail string for a symbol.
///
/// # Arguments
/// * `document` - document the symbol belongs to
/// * `symbol` - local symbol
fn symbol_detail(document: &Document, symbol: &Symbol) -> Option<String> {
    match &symbol.detail {
        SymbolDetail::Subckt { ports, .. } => {
            let names: Vec<&str> = ports.iter().map(|port| port.text.as_str()).collect();
            Some(names.join(" "))
        }
        SymbolDetail::Model { model_type, level } => {
            let kind = model_type.as_ref().map(|kind| kind.text.clone());
            let level = level.map(|level| format!("level={level}"));
            let parts: Vec<String> = kind.into_iter().chain(level).collect();
            Some(parts.join(" "))
        }
        SymbolDetail::Param {
            value: Some(value), ..
        } => Some(document.slice(value.range).to_string()),
        SymbolDetail::Function { arguments, .. } => {
            let names: Vec<&str> = arguments
                .iter()
                .map(|argument| argument.text.as_str())
                .collect();
            Some(format!("({})", names.join(", ")))
        }
        SymbolDetail::Element { element } => document.analysis.elements.get(*element).map(|info| {
            let kind = info.spec.map_or("element", |spec| spec.name);
            match &info.roles.reference {
                Some(reference) => format!("{kind} {}", reference.text),
                None => kind.to_string(),
            }
        }),
        SymbolDetail::DataBlock { columns } => {
            let names: Vec<&str> = columns.iter().map(|column| column.text.as_str()).collect();
            Some(names.join(" "))
        }
        _ => None,
    }
    .filter(|detail| !detail.is_empty())
}

/// Returns the full range of a symbol: its block for subcircuits and sections, else its definition.
///
/// # Arguments
/// * `document` - document the symbol belongs to
/// * `symbol` - local symbol
fn full_range(document: &Document, symbol: &Symbol) -> TextRange {
    let block_kind = match symbol.kind {
        SymbolKind::Subckt => Some(BlockKind::Subckt),
        SymbolKind::LibSection => Some(BlockKind::LibSection),
        _ => None,
    };
    let block = block_kind.and_then(|kind| {
        document
            .analysis
            .structure
            .blocks
            .iter()
            .find(|block| block.kind == kind && Some(block.open) == symbol.statement)
    });
    block
        .map_or(symbol.definition_range, |block| block.range)
        .cover(symbol.name.range)
}

/// Returns outline items for the document's local definitions.
///
/// # Arguments
/// * `document` - analysed document
fn symbol_items(document: &Document) -> impl Iterator<Item = Item> + '_ {
    document
        .analysis
        .symbols
        .iter()
        .filter(|symbol| symbol.is_local() && !symbol.name.range.is_empty())
        .filter_map(|symbol| {
            Some(Item {
                name: symbol.name.text.clone(),
                detail: symbol_detail(document, symbol),
                kind: outline_kind(symbol)?,
                range: full_range(document, symbol),
                selection: symbol.name.range,
            })
        })
}

/// Returns outline items for analysis commands (`.tran`, `.ac` …).
///
/// # Arguments
/// * `document` - analysed document
fn analysis_items(document: &Document) -> impl Iterator<Item = Item> + '_ {
    document
        .syntax
        .netlist
        .statements
        .iter()
        .filter_map(|statement| {
            let StatementKind::Command(command) = &statement.kind else {
                return None;
            };
            let is_analysis = matches!(command.kind, CommandKind::Generic { .. })
                && find_command(&command.keyword.text)
                    .is_some_and(|spec| spec.category == CommandCategory::Analysis)
                && command.keyword.key != ".temp";
            if !is_analysis {
                return None;
            }
            let text = document.slice(statement.range);
            let arguments = text
                .get(command.keyword.range.end - statement.range.start..)
                .unwrap_or_default();
            let detail: String = arguments.split_whitespace().collect::<Vec<_>>().join(" ");
            Some(Item {
                name: command.keyword.text.clone(),
                detail: (!detail.is_empty()).then_some(detail),
                kind: LspSymbolKind::EVENT,
                range: statement.range,
                selection: command.keyword.range,
            })
        })
}

/// Returns symbols of all open documents matching a query.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `query` - case-insensitive substring filter
pub fn workspace_symbols(workspace: &Workspace, query: &str) -> Option<WorkspaceSymbolResponse> {
    let needle = query.to_lowercase();
    let mut found: Vec<SymbolInformation> = workspace
        .documents()
        .flat_map(|document| {
            let needle = needle.clone();
            document
                .analysis
                .symbols
                .iter()
                .filter(|symbol| symbol.is_local())
                .filter(|symbol| workspace_kind(symbol).is_some())
                .filter(move |symbol| symbol.name.key.contains(&needle))
                .filter_map(move |symbol| symbol_information(workspace, document, symbol))
        })
        .collect();
    found.sort_by_key(|entry| entry.name.to_lowercase());
    Some(WorkspaceSymbolResponse::Flat(found))
}

/// Returns the LSP kind of symbols listed in workspace search (definitions only).
///
/// # Arguments
/// * `symbol` - local symbol
fn workspace_kind(symbol: &Symbol) -> Option<LspSymbolKind> {
    let is_data_column = matches!(
        symbol.detail,
        SymbolDetail::Param {
            source: ParamSource::DataColumn,
            ..
        }
    );
    match symbol.kind {
        SymbolKind::Subckt | SymbolKind::Model | SymbolKind::LibSection | SymbolKind::Function => {
            outline_kind(symbol)
        }
        SymbolKind::Param if !is_data_column => outline_kind(symbol),
        _ => None,
    }
}

/// Builds the flat workspace symbol entry for a symbol.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document that defines the symbol
/// * `symbol` - local symbol
#[allow(deprecated)]
fn symbol_information(
    workspace: &Workspace,
    document: &Document,
    symbol: &Symbol,
) -> Option<SymbolInformation> {
    let container = document
        .analysis
        .structure
        .scopes
        .get(symbol.scope)
        .and_then(|scope| scope.subckt_statement)
        .and_then(|statement| {
            document
                .analysis
                .symbols
                .iter()
                .find(|candidate| candidate.statement == Some(statement))
        })
        .map(|subckt| subckt.name.text.clone());
    Some(SymbolInformation {
        name: symbol.name.text.clone(),
        kind: workspace_kind(symbol)?,
        tags: None,
        deprecated: None,
        location: Location {
            uri: document.uri.clone(),
            range: to_lsp_range(workspace, document, symbol.name.range),
        },
        container_name: container,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::str::FromStr;

    use ls_types::Uri;

    use super::*;
    use crate::config::Settings;
    use crate::state::line_index::PositionEncoding;

    const NETLIST: &str = "t\n.param w=1u\n.subckt inv in out w2=2\nM1 out in 0 0 nch W=w2\n.ends\n.model nch nmos level=54\n.tran 1n 10n\n.meas tran d trig v(in) val=0.5 targ v(out) val=0.5\nX1 a b inv\n";

    fn open(text: &str) -> (Workspace, Uri) {
        let mut workspace = Workspace::new(None, PositionEncoding::Utf16, Settings::default());
        let uri = Uri::from_str("file:///tmp/t.sp").unwrap_or_else(|_| panic!("uri"));
        workspace.update(uri.clone(), 1, text.to_string());
        (workspace, uri)
    }

    fn outline(text: &str) -> Vec<DocumentSymbol> {
        let (workspace, uri) = open(text);
        let Some(document) = workspace.document(&uri) else {
            panic!("document");
        };
        match document_symbols(&workspace, document) {
            Some(DocumentSymbolResponse::Nested(symbols)) => symbols,
            _ => Vec::new(),
        }
    }

    fn names(symbols: &[DocumentSymbol]) -> Vec<&str> {
        symbols.iter().map(|symbol| symbol.name.as_str()).collect()
    }

    #[test]
    fn subckt_contains_its_elements_and_parameters() {
        let symbols = outline(NETLIST);
        let Some(subckt) = symbols.iter().find(|symbol| symbol.name == "inv") else {
            panic!("subckt missing: {:?}", names(&symbols));
        };
        assert_eq!(subckt.kind, LspSymbolKind::CLASS);
        assert_eq!(subckt.detail.as_deref(), Some("in out"));
        let children = subckt.children.clone().unwrap_or_default();
        assert!(names(&children).contains(&"M1"));
        assert!(names(&children).contains(&"w2"));
        assert!(!names(&symbols).contains(&"M1"));
    }

    #[test]
    fn top_level_kinds() {
        let symbols = outline(NETLIST);
        let kind = |name: &str| symbols.iter().find(|s| s.name == name).map(|s| s.kind);
        assert_eq!(kind("w"), Some(LspSymbolKind::CONSTANT));
        assert_eq!(kind("nch"), Some(LspSymbolKind::STRUCT));
        assert_eq!(kind(".tran"), Some(LspSymbolKind::EVENT));
        assert_eq!(kind("d"), Some(LspSymbolKind::FIELD));
        assert_eq!(kind("X1"), Some(LspSymbolKind::OBJECT));
        let model = symbols.iter().find(|s| s.name == "nch");
        assert_eq!(
            model.and_then(|s| s.detail.as_deref()),
            Some("nmos level=54")
        );
    }

    #[test]
    fn workspace_symbols_filter_by_substring() {
        let (workspace, _) = open(NETLIST);
        let Some(WorkspaceSymbolResponse::Flat(found)) = workspace_symbols(&workspace, "NC") else {
            panic!("flat response expected");
        };
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "nch");
        let Some(WorkspaceSymbolResponse::Flat(all)) = workspace_symbols(&workspace, "") else {
            panic!("flat response expected");
        };
        let all_names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
        assert!(
            all_names.contains(&"inv") && all_names.contains(&"w") && !all_names.contains(&"M1")
        );
        assert!(
            all.iter()
                .any(|s| s.name == "w2" && s.container_name.as_deref() == Some("inv"))
        );
    }

    #[test]
    fn fixture_outline_has_subckt() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut workspace = Workspace::new(
            Some(root.clone()),
            PositionEncoding::Utf16,
            Settings::default(),
        );
        let path = root.join("cells/inv.inc");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let Some(uri) = Uri::from_file_path(&path) else {
            panic!("uri");
        };
        workspace.update(uri.clone(), 1, text);
        let Some(document) = workspace.document(&uri) else {
            panic!("document");
        };
        let Some(DocumentSymbolResponse::Nested(symbols)) = document_symbols(&workspace, document)
        else {
            panic!("nested response expected");
        };
        assert_eq!(names(&symbols), vec!["inv"]);
        assert_eq!(symbols[0].children.as_ref().map(Vec::len), Some(4));
    }
}
