//! Inlay hints: port/terminal names before element nodes and argument names in source functions.

use ls_types::{InlayHint, InlayHintKind, InlayHintLabel, Range};

use super::to_offset;
use crate::analysis::roles::source_calls;
use crate::analysis::symbols::{SymbolDetail, SymbolKind};
use crate::lang::elements::ElementSpec;
use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::{Arg, Element, Ident, StatementKind, Value};

/// A hint candidate: byte offset, label text and the text it annotates.
struct Candidate<'a> {
    offset: usize,
    label: String,
    annotated: &'a str,
}

/// Computes inlay hints inside a range.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being displayed
/// * `range` - visible range
pub fn inlay_hints(
    workspace: &Workspace,
    document: &Document,
    range: Range,
) -> Option<Vec<InlayHint>> {
    let start = to_offset(workspace, document, range.start);
    let end = to_offset(workspace, document, range.end);
    let settings = workspace.settings.inlay_hints;
    let hints = document
        .analysis
        .elements
        .iter()
        .filter(|info| !info.skipped)
        .filter_map(|info| {
            let StatementKind::Element(element) =
                &document.syntax.netlist.statements.get(info.statement)?.kind
            else {
                return None;
            };
            let terminals = if settings.ports {
                terminal_candidates(document, info, terminal_names(document, info))
            } else {
                Vec::new()
            };
            let sources = if settings.source_args {
                source_candidates(document, element)
            } else {
                Vec::new()
            };
            Some(terminals.into_iter().chain(sources))
        })
        .flatten()
        .filter(|candidate| start <= candidate.offset && candidate.offset <= end)
        .filter(|candidate| {
            !candidate
                .label
                .trim_end_matches(':')
                .eq_ignore_ascii_case(candidate.annotated)
        })
        .map(|candidate| InlayHint {
            position: document.line_index.position(
                &document.text,
                candidate.offset,
                workspace.encoding,
            ),
            label: InlayHintLabel::String(candidate.label),
            kind: Some(InlayHintKind::PARAMETER),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: Some(true),
            data: None,
        })
        .collect();
    Some(hints)
}

/// Returns the terminal names an element's nodes are labelled with.
///
/// # Arguments
/// * `document` - analysed document
/// * `info` - element line with its roles
fn terminal_names(
    document: &Document,
    info: &crate::analysis::resolve::ElementInfo,
) -> Vec<String> {
    match info.spec {
        Some(spec) if spec.letter == 'X' => subckt_ports(document, info),
        Some(spec) if labels_terminals(spec) => spec
            .terminals
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns true for element types whose terminal labels are worth showing.
///
/// # Arguments
/// * `spec` - element type
fn labels_terminals(spec: &ElementSpec) -> bool {
    !spec.terminals.is_empty() && !matches!(spec.letter, 'R' | 'C' | 'L' | 'V' | 'I')
}

/// Returns the port names of the subcircuit an X element instantiates.
///
/// # Arguments
/// * `document` - analysed document
/// * `info` - element line with its roles
fn subckt_ports(document: &Document, info: &crate::analysis::resolve::ElementInfo) -> Vec<String> {
    let analysis = &document.analysis;
    info.roles
        .reference
        .as_ref()
        .and_then(|reference| analysis.lookup(SymbolKind::Subckt, info.scope, &reference.key))
        .and_then(|id| analysis.symbols.get(id))
        .map(|symbol| match &symbol.detail {
            SymbolDetail::Subckt { ports, .. } => {
                ports.iter().map(|port| port.text.clone()).collect()
            }
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

/// Pairs element nodes with their terminal names.
///
/// # Arguments
/// * `document` - analysed document
/// * `info` - element line with its roles
/// * `names` - terminal names in order
fn terminal_candidates<'a>(
    document: &'a Document,
    info: &crate::analysis::resolve::ElementInfo,
    names: Vec<String>,
) -> Vec<Candidate<'a>> {
    info.roles
        .nodes
        .iter()
        .zip(names)
        .map(|(node, name)| candidate(document, node, format!("{name}:")))
        .collect()
}

/// Builds a candidate for an identifier.
///
/// # Arguments
/// * `document` - document the identifier belongs to
/// * `node` - annotated identifier
/// * `label` - hint text
fn candidate<'a>(document: &'a Document, node: &Ident, label: String) -> Candidate<'a> {
    Candidate {
        offset: node.range.start,
        label,
        annotated: document.slice(node.range),
    }
}

/// Returns argument-name candidates for the source functions of an element.
///
/// # Arguments
/// * `document` - analysed document
/// * `element` - lowered element line
fn source_candidates<'a>(document: &'a Document, element: &Element) -> Vec<Candidate<'a>> {
    source_calls(element)
        .into_iter()
        .flat_map(|call| {
            call.args
                .iter()
                .map_while(plain_value)
                .zip(call.function.arguments)
                .map(|(value, argument)| Candidate {
                    offset: value.range().start,
                    label: format!("{}:", argument.name),
                    annotated: document.slice(value.range()),
                })
        })
        .collect()
}

/// Returns the value of a positional numeric or expression argument.
///
/// # Arguments
/// * `arg` - argument
fn plain_value(arg: &Arg) -> Option<&Value> {
    match arg {
        Arg::Value(value @ (Value::Number(_) | Value::Expr(_))) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ls_types::{Position, Uri};

    use super::*;
    use crate::config::Settings;
    use crate::state::line_index::PositionEncoding;

    fn everything() -> Range {
        Range::new(Position::new(0, 0), Position::new(1000, 0))
    }

    fn labels(workspace: &Workspace, uri: &Uri, range: Range) -> Vec<String> {
        let Some(document) = workspace.document(uri) else {
            return Vec::new();
        };
        inlay_hints(workspace, document, range)
            .unwrap_or_default()
            .into_iter()
            .map(|hint| match hint.label {
                InlayHintLabel::String(text) => text,
                InlayHintLabel::LabelParts(_) => String::new(),
            })
            .collect()
    }

    fn open(text: &str, settings: Settings) -> (Workspace, Uri) {
        let mut workspace = Workspace::new(None, PositionEncoding::Utf16, settings);
        let uri = Uri::from_str("file:///tmp/t.sp").unwrap_or_else(|_| panic!("uri"));
        workspace.update(uri.clone(), 1, text.to_string());
        (workspace, uri)
    }

    #[test]
    fn fixture_instance_gets_port_names() {
        let root = crate::test_support::fixtures();
        let mut workspace = Workspace::new(
            Some(root.clone()),
            PositionEncoding::Utf16,
            Settings::default(),
        );
        let path = root.join("inverter_tb.sp");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let Some(uri) = Uri::from_file_path(&path) else {
            panic!("uri");
        };
        workspace.update(uri.clone(), 1, text);
        let all = labels(&workspace, &uri, everything());
        // `in`, `out` and `vdd` equal their port names and are skipped; ground `0` gets `gnd:`.
        assert!(all.contains(&"gnd:".to_string()));
        assert!(
            !all.iter()
                .any(|label| matches!(label.as_str(), "in:" | "out:" | "vdd:"))
        );
    }

    #[test]
    fn mos_terminals_and_source_args() {
        let (workspace, uri) = open(
            "t\nM1 dd gg ss bb nch\nVin in 0 PULSE(0 1 0 1n)\n",
            Settings::default(),
        );
        let all = labels(&workspace, &uri, everything());
        assert_eq!(
            all,
            vec!["d:", "g:", "s:", "b:", "v1:", "v2:", "td:", "tr:"]
        );
    }

    #[test]
    fn hints_equal_to_the_node_are_skipped() {
        let (workspace, uri) = open("t\nM1 d g s b nch\n", Settings::default());
        assert!(labels(&workspace, &uri, everything()).is_empty());
    }

    #[test]
    fn only_requested_range_and_settings_apply() {
        let (workspace, uri) = open(
            "t\nM1 dd gg ss bb nch\nM2 dd gg ss bb nch\n",
            Settings::default(),
        );
        let second = Range::new(Position::new(2, 0), Position::new(3, 0));
        assert_eq!(labels(&workspace, &uri, second).len(), 4);
        let mut settings = Settings::default();
        settings.inlay_hints.ports = false;
        let (workspace, uri) = open("t\nM1 dd gg ss bb nch\n", settings);
        assert!(labels(&workspace, &uri, everything()).is_empty());
    }
}
