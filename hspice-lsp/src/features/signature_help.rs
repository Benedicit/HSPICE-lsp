//! Signature help for X instances, element lines, source functions, built-ins and dot commands.

use ls_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, Position,
    SignatureHelp, SignatureInformation,
};

use super::completion::line_context::{Frame, FrameKind, LineScan, scan};
use super::{argument_docs, to_offset};
use crate::analysis::symbols::{Symbol, SymbolDetail, SymbolKind};
use crate::lang::commands::find_command;
use crate::lang::elements::{ElementSpec, find_element, find_source_function};
use crate::lang::functions::find_function;
use crate::lang::output_vars::find_output_var;
use crate::state::document::Document;
use crate::state::workspace::Workspace;

/// Maximum number of candidate subcircuits offered as signatures.
const MAX_CANDIDATES: usize = 5;

/// Computes signature help at a position.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
pub fn signature_help(
    workspace: &Workspace,
    document: &Document,
    position: Position,
) -> Option<SignatureHelp> {
    let offset = to_offset(workspace, document, position);
    if document
        .syntax
        .title_ranges
        .iter()
        .any(|range| range.touches(offset))
    {
        return None;
    }
    let scan = scan(&document.text, offset)?;
    call_signature(document, &scan).or_else(|| line_signature(document, &scan))
}

/// Wraps one signature into a response.
///
/// # Arguments
/// * `signature` - the signature
/// * `active` - active parameter index
fn single(signature: SignatureInformation, active: Option<usize>) -> SignatureHelp {
    SignatureHelp {
        signatures: vec![signature],
        active_signature: Some(0),
        active_parameter: active.and_then(|index| u32::try_from(index).ok()),
    }
}

/// Builds a signature whose parameters are located by searching the label.
///
/// # Arguments
/// * `label` - full signature text
/// * `parameters` - parameter texts in order of appearance
/// * `documentation` - Markdown documentation
fn signature(label: &str, parameters: &[String], documentation: &str) -> SignatureInformation {
    let infos = parameters
        .iter()
        .scan(0usize, |from, parameter| {
            let start = *from + label.get(*from..)?.find(parameter.as_str())?;
            *from = start + parameter.len();
            Some(ParameterInformation {
                label: ParameterLabel::LabelOffsets([
                    u32::try_from(start).ok()?,
                    u32::try_from(start + parameter.len()).ok()?,
                ]),
                documentation: None,
            })
        })
        .collect();
    SignatureInformation {
        label: label.to_string(),
        documentation: (!documentation.is_empty()).then(|| {
            Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: documentation.to_string(),
            })
        }),
        parameters: Some(infos),
        active_parameter: None,
    }
}

/// Attaches a description to each parameter of a signature, in order.
///
/// # Arguments
/// * `info` - signature whose parameters were located by `signature`
/// * `descriptions` - one description per parameter
fn with_parameter_docs(info: SignatureInformation, descriptions: &[&str]) -> SignatureInformation {
    let parameters = info.parameters.map(|parameters| {
        parameters
            .into_iter()
            .zip(descriptions)
            .map(|(parameter, description)| ParameterInformation {
                documentation: Some(Documentation::String((*description).to_string())),
                ..parameter
            })
            .collect()
    });
    SignatureInformation { parameters, ..info }
}

/// Returns the innermost named parenthesis frame at the cursor.
///
/// # Arguments
/// * `scan` - scanned logical line
fn named_call_frame(scan: &LineScan) -> Option<&Frame> {
    scan.frames
        .iter()
        .rev()
        .find(|frame| frame.kind == FrameKind::Paren && !frame.name.is_empty())
}

/// Computes the signature of the function call the cursor is inside.
///
/// # Arguments
/// * `document` - analysed document
/// * `scan` - scanned logical line
fn call_signature(document: &Document, scan: &LineScan) -> Option<SignatureHelp> {
    let frame = named_call_frame(scan)?;
    let first = scan.first_word();
    let source_line = first.starts_with(['v', 'i']) && !first.starts_with('.') && !scan.in_quote();
    let key = frame.name.to_ascii_lowercase();
    let active = Some(frame.argument_index());
    let source = find_source_function(&key)
        .filter(|_| source_line)
        .map(|spec| {
            let arguments: Vec<String> = spec
                .arguments
                .iter()
                .map(|argument| argument.name.to_string())
                .collect();
            let label = format!(
                "{}({})",
                spec.name.to_ascii_uppercase(),
                arguments.join(" ")
            );
            let descriptions: Vec<&str> = spec
                .arguments
                .iter()
                .map(|argument| argument.description)
                .collect();
            with_parameter_docs(
                signature(&label, &arguments, spec.description),
                &descriptions,
            )
        });
    let user = || user_function_signature(document, scan, &key);
    let builtin = || {
        find_function(&key).map(|spec| {
            let parameters: Vec<String> = spec
                .parameters
                .iter()
                .map(|parameter| (*parameter).to_string())
                .collect();
            signature(spec.signature, &parameters, spec.description)
        })
    };
    let output =
        || find_output_var(&key).map(|spec| signature(spec.signature, &[], spec.description));
    source
        .or_else(user)
        .or_else(builtin)
        .or_else(output)
        .map(|info| single(info, active))
}

/// Builds the signature of a user-defined function.
///
/// # Arguments
/// * `document` - analysed document
/// * `scan` - scanned logical line
/// * `key` - lowercase function name
fn user_function_signature(
    document: &Document,
    scan: &LineScan,
    key: &str,
) -> Option<SignatureInformation> {
    let analysis = &document.analysis;
    let scope = analysis.scope_at(scan.cursor);
    let symbol = &analysis.symbols[analysis.lookup(SymbolKind::Function, scope, key)?];
    let SymbolDetail::Function { arguments, .. } = &symbol.detail else {
        return None;
    };
    let names: Vec<String> = arguments
        .iter()
        .map(|argument| argument.text.clone())
        .collect();
    let label = format!("{}({})", symbol.name.text, names.join(", "));
    Some(signature(&label, &names, "User function"))
}

/// Computes the signature for the statement the cursor is in (element line or dot command).
///
/// # Arguments
/// * `document` - analysed document
/// * `scan` - scanned logical line
fn line_signature(document: &Document, scan: &LineScan) -> Option<SignatureHelp> {
    if scan.in_first_word() || scan.in_quote() {
        return None;
    }
    if scan.is_command() {
        return command_signature(scan);
    }
    let spec = find_element(&scan.first_word())?;
    let active = scan.positionals().len();
    if spec.letter == 'X' {
        return subckt_instance_signature(document, scan, active)
            .or_else(|| element_signature(spec, active));
    }
    element_signature(spec, active)
}

/// Returns the synopsis lines of a dot command as signatures.
///
/// # Arguments
/// * `scan` - scanned logical line
fn command_signature(scan: &LineScan) -> Option<SignatureHelp> {
    let spec = find_command(&scan.first_word())?;
    let signatures: Vec<SignatureInformation> = spec
        .synopsis
        .lines()
        .map(|line| {
            let documentation = format!("{}{}", spec.description, argument_docs(spec.arguments));
            signature(line, &[], &documentation)
        })
        .collect();
    Some(SignatureHelp {
        signatures,
        active_signature: Some(0),
        active_parameter: None,
    })
}

/// Builds the generic synopsis signature of an element with one parameter per synopsis word.
///
/// # Arguments
/// * `spec` - element type
/// * `active` - number of positional words before the cursor
fn element_signature(spec: &ElementSpec, active: usize) -> Option<SignatureHelp> {
    let words: Vec<String> = spec
        .synopsis
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect();
    let documentation = format!("{}\n\n{}", spec.name, spec.description);
    Some(single(
        signature(spec.synopsis, &words, &documentation),
        Some(active),
    ))
}

/// Finds the subcircuits an X line may refer to: the resolved one, else those with enough ports.
///
/// # Arguments
/// * `document` - analysed document
/// * `scan` - scanned logical line
/// * `active` - number of positional words before the cursor
fn instance_subckts<'a>(document: &'a Document, scan: &LineScan, active: usize) -> Vec<&'a Symbol> {
    let analysis = &document.analysis;
    let scope = analysis.scope_at(scan.cursor);
    let from_analysis = analysis
        .elements
        .iter()
        .find(|info| {
            document
                .syntax
                .netlist
                .statements
                .get(info.statement)
                .is_some_and(|statement| statement.range.start == scan.line_start)
        })
        .and_then(|info| info.roles.reference.as_ref())
        .and_then(|name| analysis.lookup(SymbolKind::Subckt, scope, &name.key));
    let from_text = || {
        scan.positionals().iter().find_map(|token| {
            analysis.lookup(SymbolKind::Subckt, scope, &token.text.to_ascii_lowercase())
        })
    };
    match from_analysis.or_else(from_text) {
        Some(id) => vec![&analysis.symbols[id]],
        None => analysis
            .visible_symbols(SymbolKind::Subckt, scope)
            .into_iter()
            .map(|id| &analysis.symbols[id])
            .filter(|symbol| matches!(&symbol.detail, SymbolDetail::Subckt { ports, .. } if ports.len() >= active))
            .take(MAX_CANDIDATES)
            .collect(),
    }
}

/// Builds the signature of an X instance for one subcircuit with one parameter per port.
///
/// # Arguments
/// * `symbol` - subcircuit symbol
fn subckt_instance_info(symbol: &Symbol) -> Option<SignatureInformation> {
    let SymbolDetail::Subckt { ports, params, .. } = &symbol.detail else {
        return None;
    };
    let names: Vec<String> = ports.iter().map(|port| port.text.clone()).collect();
    let params_text = if params.is_empty() {
        String::new()
    } else {
        " [params]".to_string()
    };
    let label = format!(
        "X<name> {} {}{params_text}",
        names.join(" "),
        symbol.name.text
    );
    let documentation = if params.is_empty() {
        format!("Instance of subcircuit `{}`", symbol.name.text)
    } else {
        let list = params
            .iter()
            .map(|param| format!("`{}`", param.name.text))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Instance of subcircuit `{}`; parameters: {list}",
            symbol.name.text
        )
    };
    Some(signature(&label, &names, &documentation))
}

/// Builds the signature help of an X instance (one signature per candidate subcircuit).
///
/// # Arguments
/// * `document` - analysed document
/// * `scan` - scanned logical line
/// * `active` - number of positional words before the cursor
fn subckt_instance_signature(
    document: &Document,
    scan: &LineScan,
    active: usize,
) -> Option<SignatureHelp> {
    let signatures: Vec<SignatureInformation> = instance_subckts(document, scan, active)
        .into_iter()
        .filter_map(subckt_instance_info)
        .collect();
    (!signatures.is_empty()).then(|| SignatureHelp {
        signatures,
        active_signature: Some(0),
        active_parameter: u32::try_from(active).ok(),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::super::completion::test_support::{Opened, open, open_in_fixtures};
    use super::*;

    fn help(opened: &Opened) -> Option<SignatureHelp> {
        let document = opened.workspace.document(&opened.uri)?;
        signature_help(&opened.workspace, document, opened.position)
    }

    fn help_of(marked: &str) -> Option<SignatureHelp> {
        help(&open(marked))
    }

    fn label_of(help: &SignatureHelp) -> &str {
        &help.signatures[0].label
    }

    #[test]
    fn x_instance_shows_ports_and_active_node() {
        let marked = "* t\n.subckt inv in out vdd gnd wn=1u\n.ends\nX1 a b |";
        let result = help_of(marked).expect("help");
        assert_eq!(label_of(&result), "X<name> in out vdd gnd inv [params]");
        assert_eq!(
            result.signatures[0].parameters.as_ref().map(Vec::len),
            Some(4)
        );
        assert_eq!(result.active_parameter, Some(2));
    }

    #[test]
    fn x_instance_with_known_subckt_and_cursor_inside() {
        let result = help_of("* t\n.subckt inv in out\n.ends\nX1 a |b inv").expect("help");
        assert_eq!(result.active_parameter, Some(1));
        assert!(label_of(&result).contains("in out inv"));
    }

    #[test]
    fn x_instance_in_fixture() {
        let result =
            help(&open_in_fixtures("* t\n.include 'cells/inv.inc'\nX2 a b |")).expect("help");
        assert_eq!(label_of(&result), "X<name> in out vdd gnd inv [params]");
    }

    #[test]
    fn element_lines_show_synopsis_with_positions() {
        let result = help_of("* t\nM1 d g |").expect("help");
        assert!(label_of(&result).starts_with("Mxxx"));
        assert_eq!(result.active_parameter, Some(2));
        let params = result.signatures[0].parameters.clone().unwrap_or_default();
        assert!(params.len() > 3);
        assert_eq!(result.active_signature, Some(0));
    }

    #[test]
    fn source_functions_track_argument_index() {
        let result = help_of("* t\nVin a 0 pulse(0 1 |").expect("help");
        assert!(label_of(&result).starts_with("PULSE(v1 v2 td"));
        assert_eq!(result.active_parameter, Some(2));
        let first = help_of("* t\nVin a 0 sin(|").expect("help");
        assert_eq!(first.active_parameter, Some(0));
        let typing = help_of("* t\nVin a 0 pulse(0 1|").expect("help");
        assert_eq!(typing.active_parameter, Some(1));
        let params = result.signatures[0].parameters.clone().unwrap_or_default();
        assert_eq!(
            params[2].documentation,
            Some(Documentation::String(
                "Delay before the first edge (default 0)".to_string()
            ))
        );
    }

    #[test]
    fn builtin_and_user_functions_count_commas() {
        let result = help_of("* t\nR1 a b 'max(1, |").expect("help");
        assert!(label_of(&result).starts_with("max("));
        assert_eq!(result.active_parameter, Some(1));
        let nested = help_of("* t\nR1 a b 'max(min(1, 2), |)'").expect("help");
        assert!(label_of(&nested).starts_with("max("));
        assert_eq!(nested.active_parameter, Some(1));
        let user = help_of("* t\n.param f(x,y)='x+y'\nR1 a b 'f(1, |").expect("help");
        assert_eq!(label_of(&user), "f(x, y)");
        assert_eq!(user.active_parameter, Some(1));
        let unquoted = help_of("* t\n.param p=sqrt(|").expect("help");
        assert!(label_of(&unquoted).starts_with("sqrt("));
    }

    #[test]
    fn output_variables_show_their_signature() {
        let result = help_of("* t\n.print tran v(|").expect("help");
        assert_eq!(label_of(&result), "V(n1<,n2>)");
    }

    #[test]
    fn dot_commands_show_every_synopsis_line() {
        let result = help_of("* t\n.ac |").expect("help");
        assert_eq!(result.signatures.len(), 2);
        assert!(result.signatures[0].label.starts_with(".AC DEC"));
        assert_eq!(result.active_parameter, None);
    }

    #[test]
    fn no_help_on_first_word_comments_and_title() {
        assert!(help_of("* t\nM|1 d g s b nch").is_none());
        assert!(help_of("* t\n.tr|an").is_none());
        assert!(help_of("* t\n* M1 d |g").is_none());
        assert!(help_of("Ti|tle\nR1 a b 1k").is_none());
    }
}
