//! Hover documentation for commands, elements, names, numbers, functions and options.

use std::path::Path;

use ls_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use super::{argument_docs, definition_text, to_lsp_range, to_offset};
use crate::analysis::eval::evaluate;
use crate::analysis::resolve::ElementInfo;
use crate::analysis::statement_at;
use crate::analysis::symbols::{
    Origin, ROOT_SCOPE, RefContext, ScopeId, Symbol, SymbolDetail, SymbolId, SymbolKind,
};
use crate::lang::commands::{CommandSpec, find_command};
use crate::lang::elements::{
    ElementSpec, SourceFunctionSpec, find_element, find_source_function, is_ground,
};
use crate::lang::functions::{find_function, find_special_variable};
use crate::lang::measure::{ANALYSIS_TYPES, find_analysis_keyword, find_measure_keyword};
use crate::lang::models::{MODEL_TYPES, find_model_type, model_params_for, mosfet_level_name};
use crate::lang::mosra::find_mosra_param;
use crate::lang::options::{OptionValue, find_option};
use crate::lang::output_vars::find_output_var;
use crate::lang::{Status, find_named};
use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::{
    Arg, AssignKey, Command, CommandKind, Element, Expr, ExprKind, Ident, StatementKind, TextRange,
    Value,
};
use crate::syntax::number::{ParsedNumber, format_engineering, parse_number};

/// Maximum number of lines of a model card shown in a hover.
const MAX_MODEL_LINES: usize = 8;

/// Maximum number of listed connections of a node.
const MAX_CONNECTIONS: usize = 12;

/// Maximum depth of the recursive parameter evaluation.
const MAX_EVAL_DEPTH: usize = 16;

/// Hover text of the ground node.
const GROUND_TEXT: &str = "**Ground** \u{2014} node `0` (`0`, `gnd`, `gnd!`, `ground` and `!gnd` all name the global ground node).";

/// A hoverable token of a statement.
#[derive(Debug, Clone, Copy)]
enum Leaf<'a> {
    /// A numeric literal.
    Number(&'a str, TextRange),
    /// A positional bare word.
    Word(&'a Ident),
    /// The key of an assignment.
    Key(&'a Ident),
    /// The name of a call such as `pulse(` or `v(`.
    Call(&'a Ident),
    /// An identifier inside an expression.
    ExprName(&'a Ident),
}

impl Leaf<'_> {
    /// Returns the source range of the token.
    fn range(&self) -> TextRange {
        match self {
            Leaf::Number(_, range) => *range,
            Leaf::Word(ident) | Leaf::Key(ident) | Leaf::Call(ident) | Leaf::ExprName(ident) => {
                ident.range
            }
        }
    }
}

/// What the hovered statement is.
#[derive(Clone, Copy)]
struct Context<'a> {
    /// The dot command, if the statement is one.
    command: Option<&'static CommandSpec>,
    /// The element type, if the statement is an element line.
    element: Option<&'static ElementSpec>,
    /// Model type of a `.model` statement (lowercase).
    model_type: Option<&'a str>,
}

/// Computes hover contents at a position.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
pub fn hover(workspace: &Workspace, document: &Document, position: Position) -> Option<Hover> {
    let offset = to_offset(workspace, document, position);
    let (range, text) = hover_text(document, offset)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: text,
        }),
        range: Some(to_lsp_range(workspace, document, range)),
    })
}

/// Finds the token under the cursor and renders its documentation.
///
/// # Arguments
/// * `document` - document being edited
/// * `offset` - cursor offset
fn hover_text(document: &Document, offset: usize) -> Option<(TextRange, String)> {
    let index = statement_at(&document.syntax.netlist, offset)?;
    let statement = &document.syntax.netlist.statements[index];
    command_keyword_hover(statement_command(&statement.kind), offset)
        .or_else(|| element_name_hover(document, index, offset))
        .or_else(|| symbol_hover(document, offset))
        .or_else(|| ground_hover(document, index, offset))
        .or_else(|| include_path_hover(document, index, offset))
        .or_else(|| leaf_hover(&statement.kind, offset))
}

/// Returns the command of a statement.
///
/// # Arguments
/// * `kind` - statement kind
fn statement_command(kind: &StatementKind) -> Option<&Command> {
    match kind {
        StatementKind::Command(command) => Some(command),
        _ => None,
    }
}

/// Renders the hover of a dot-command keyword.
///
/// # Arguments
/// * `command` - the statement's command, if any
/// * `offset` - cursor offset
fn command_keyword_hover(command: Option<&Command>, offset: usize) -> Option<(TextRange, String)> {
    let command = command.filter(|command| command.keyword.range.touches(offset))?;
    let spec = find_command(&command.keyword.text)?;
    Some((command.keyword.range, command_text(spec)))
}

/// Renders the documentation of a dot command.
///
/// # Arguments
/// * `spec` - command table entry
fn command_text(spec: &CommandSpec) -> String {
    let notes = match spec.status {
        Status::Obsolete => "\n\n_Obsolete: still accepted by HSPICE, but avoid in new netlists._",
        Status::Foreign => "\n\n_Not HSPICE: this command belongs to another SPICE dialect._",
        Status::Supported => "",
    };
    let aliases = if spec.aliases.is_empty() {
        String::new()
    } else {
        format!("\n\nAliases: `{}`", spec.aliases.join("`, `"))
    };
    format!(
        "```spice\n{}\n```\n{}{}{aliases}{notes}",
        spec.synopsis,
        spec.description,
        argument_docs(spec.arguments)
    )
}

/// Renders the hover of an element name.
///
/// # Arguments
/// * `document` - analysed document
/// * `statement` - statement index
/// * `offset` - cursor offset
fn element_name_hover(
    document: &Document,
    statement: usize,
    offset: usize,
) -> Option<(TextRange, String)> {
    let info = document
        .analysis
        .element_at_statement(statement)
        .filter(|info| info.name.range.touches(offset))?;
    Some((info.name.range, element_text(document, info)))
}

/// Renders the documentation of an element instance with its resolved reference and connections.
///
/// # Arguments
/// * `document` - analysed document
/// * `info` - element line with roles
fn element_text(document: &Document, info: &ElementInfo) -> String {
    let analysis = &document.analysis;
    let Some(spec) = info.spec else {
        return format!("**{}**\n\nUnknown element type.", info.name.text);
    };
    let reference = info.roles.reference.as_ref().and_then(|name| {
        let kind = if spec.letter == 'X' {
            SymbolKind::Subckt
        } else {
            SymbolKind::Model
        };
        analysis
            .lookup(kind, info.scope, &name.key)
            .map(|id| &analysis.symbols[id])
    });
    let reference_text = reference
        .map(|symbol| reference_summary(document, symbol))
        .unwrap_or_default();
    let terminals = terminal_names(spec, reference);
    let table = connection_table(&terminals, &info.roles.nodes);
    format!(
        "**{}** \u{2014} {}\n\n```spice\n{}\n```\n{}{reference_text}{table}",
        info.name.text, spec.name, spec.synopsis, spec.description
    )
}

/// Returns the terminal names of an element (subcircuit ports for X instances).
///
/// # Arguments
/// * `spec` - element type
/// * `reference` - resolved subcircuit or model
fn terminal_names(spec: &ElementSpec, reference: Option<&Symbol>) -> Vec<String> {
    match reference.map(|symbol| &symbol.detail) {
        Some(SymbolDetail::Subckt { ports, .. }) if spec.letter == 'X' => {
            ports.iter().map(|port| port.text.clone()).collect()
        }
        _ => spec
            .terminals
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
    }
}

/// Renders a terminal-to-node table.
///
/// # Arguments
/// * `terminals` - terminal or port names
/// * `nodes` - connected node identifiers in order
fn connection_table(terminals: &[String], nodes: &[Ident]) -> String {
    if nodes.is_empty() {
        return String::new();
    }
    let rows: Vec<String> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let terminal = terminals
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("#{}", index + 1));
            format!("| `{terminal}` | `{}` |", node.text)
        })
        .collect();
    format!("\n\n| terminal | node |\n|---|---|\n{}", rows.join("\n"))
}

/// Summarises the model or subcircuit an element refers to.
///
/// # Arguments
/// * `document` - analysed document
/// * `symbol` - resolved model or subcircuit
fn reference_summary(document: &Document, symbol: &Symbol) -> String {
    let text = definition_text(document, symbol);
    match symbol.kind {
        SymbolKind::Subckt => format!("\n\n```spice\n{}\n```", logical_first_line(text)),
        _ => format!("\n\n```spice\n{}\n```", truncate_lines(text, 3)),
    }
}

/// Renders hover for a resolved reference or a local definition under the cursor.
///
/// # Arguments
/// * `document` - analysed document
/// * `offset` - cursor offset
fn symbol_hover(document: &Document, offset: usize) -> Option<(TextRange, String)> {
    let analysis = &document.analysis;
    let reference = analysis.reference_at(offset);
    let (id, range) = match reference
        .and_then(|reference| reference.target.map(|id| (id, reference.name.range)))
    {
        Some(found) => found,
        None => {
            let id = analysis.definition_at(offset)?;
            (id, analysis.symbols[id].name.range)
        }
    };
    Some((range, symbol_text(document, id)))
}

/// Renders the documentation of a symbol.
///
/// # Arguments
/// * `document` - analysed document
/// * `id` - symbol id
fn symbol_text(document: &Document, id: SymbolId) -> String {
    let symbol = &document.analysis.symbols[id];
    match symbol.kind {
        SymbolKind::Param => param_text(document, id),
        SymbolKind::Function => function_text(document, symbol),
        SymbolKind::Subckt => subckt_text(document, symbol),
        SymbolKind::Model => model_text(document, symbol),
        SymbolKind::Node => node_text(document, id),
        SymbolKind::Element => match symbol.detail {
            SymbolDetail::Element { element } => {
                document.analysis.elements.get(element).map_or_else(
                    || symbol.name.text.clone(),
                    |info| element_text(document, info),
                )
            }
            _ => symbol.name.text.clone(),
        },
        SymbolKind::Measure => format!(
            "**{}** \u{2014} measure result\n\n```spice\n{}\n```\n{}",
            symbol.name.text,
            truncate_lines(definition_text(document, symbol), MAX_MODEL_LINES),
            origin_line(document, symbol)
        ),
        SymbolKind::LibSection => format!(
            "**{}** \u{2014} library section\n\n{}",
            symbol.name.text,
            origin_line(document, symbol)
        ),
        SymbolKind::DataBlock => format!(
            "**{}** \u{2014} `.data` block\n\n{}",
            symbol.name.text,
            origin_line(document, symbol)
        ),
    }
}

/// Returns the file name of a path.
///
/// # Arguments
/// * `path` - file path
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().to_string(),
    )
}

/// Returns the name of the subcircuit that owns a scope.
///
/// # Arguments
/// * `document` - analysed document
/// * `scope` - scope id
fn scope_name(document: &Document, scope: ScopeId) -> Option<String> {
    let statement = document
        .analysis
        .structure
        .scopes
        .get(scope)?
        .subckt_statement?;
    match &document.syntax.netlist.statements.get(statement)?.kind {
        StatementKind::Command(Command {
            kind: CommandKind::Subckt {
                name: Some(name), ..
            },
            ..
        }) => Some(name.text.clone()),
        _ => None,
    }
}

/// Describes where a symbol is defined.
///
/// # Arguments
/// * `document` - analysed document
/// * `symbol` - symbol
fn origin_line(document: &Document, symbol: &Symbol) -> String {
    match &symbol.origin {
        Origin::External(file) => format!("Defined in `{}`", file_name(&file.path)),
        Origin::Local => match scope_name(document, symbol.scope) {
            Some(name) if symbol.scope != ROOT_SCOPE => format!("Local to subcircuit `{name}`"),
            _ => "Defined in this file".to_string(),
        },
    }
}

/// Returns the first logical line of a text (continuation lines joined).
///
/// # Arguments
/// * `text` - statement text
fn logical_first_line(text: &str) -> String {
    let mut lines = text.lines();
    let head = lines.next().unwrap_or_default().trim().to_string();
    lines
        .map(str::trim_start)
        .take_while(|line| line.starts_with('+') || line.starts_with('*'))
        .filter(|line| line.starts_with('+'))
        .fold(head, |joined, line| {
            format!("{joined} {}", line.trim_start_matches('+').trim())
        })
}

/// Keeps the first `count` lines of a text, marking truncation.
///
/// # Arguments
/// * `text` - text to shorten
/// * `count` - maximum number of lines
fn truncate_lines(text: &str, count: usize) -> String {
    let total = text.lines().count();
    let kept: Vec<&str> = text.lines().take(count).collect();
    if total > count {
        format!(
            "{}\n* \u{2026} ({} more lines)",
            kept.join("\n"),
            total - count
        )
    } else {
        kept.join("\n")
    }
}

/// Evaluates a parameter recursively; `visited` guards against cycles.
///
/// # Arguments
/// * `document` - analysed document
/// * `scope` - scope to resolve names from
/// * `key` - lowercase parameter name
/// * `visited` - names being evaluated
fn param_value(document: &Document, scope: ScopeId, key: &str, visited: &[String]) -> Option<f64> {
    if visited.len() >= MAX_EVAL_DEPTH || visited.iter().any(|name| name == key) {
        return None;
    }
    let analysis = &document.analysis;
    let id = analysis.lookup(SymbolKind::Param, scope, key)?;
    let symbol = &analysis.symbols[id];
    let SymbolDetail::Param {
        value: Some(expr), ..
    } = &symbol.detail
    else {
        return None;
    };
    let path: Vec<String> = visited
        .iter()
        .cloned()
        .chain(std::iter::once(key.to_string()))
        .collect();
    evaluate(expr, &|name| {
        param_value(document, symbol.scope, name, &path)
    })
}

/// Renders a plain scientific number such as `1.0e-2`.
///
/// # Arguments
/// * `value` - number to render
fn scientific(value: f64) -> String {
    let text = format!("{value:e}");
    match text.split_once('e') {
        Some((mantissa, exponent)) if !mantissa.contains('.') => format!("{mantissa}.0e{exponent}"),
        _ => text,
    }
}

/// Renders the value line of an evaluated number.
///
/// # Arguments
/// * `value` - evaluated value
fn value_line(value: f64) -> String {
    format!(
        "= `{}` (`{}`)",
        format_engineering(value),
        scientific(value)
    )
}

/// Renders the documentation of a parameter.
///
/// # Arguments
/// * `document` - analysed document
/// * `id` - symbol id
fn param_text(document: &Document, id: SymbolId) -> String {
    let symbol = &document.analysis.symbols[id];
    let definition = definition_text(document, symbol).trim();
    let value = param_value(document, symbol.scope, &symbol.name.key, &[])
        .map_or(String::new(), |value| format!("\n\n{}", value_line(value)));
    format!(
        "```spice\n{definition}\n```{value}\n\n{}",
        origin_line(document, symbol)
    )
}

/// Renders the documentation of a user function.
///
/// # Arguments
/// * `document` - analysed document
/// * `symbol` - function symbol
fn function_text(document: &Document, symbol: &Symbol) -> String {
    let arguments = match &symbol.detail {
        SymbolDetail::Function { arguments, .. } => arguments
            .iter()
            .map(|argument| argument.text.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    };
    format!(
        "**{}({arguments})** \u{2014} user function\n\n```spice\n{}\n```\n{}",
        symbol.name.text,
        definition_text(document, symbol).trim(),
        origin_line(document, symbol)
    )
}

/// Renders the documentation of a subcircuit.
///
/// # Arguments
/// * `document` - analysed document
/// * `symbol` - subcircuit symbol
fn subckt_text(document: &Document, symbol: &Symbol) -> String {
    let header = logical_first_line(definition_text(document, symbol));
    let (ports, params) = match &symbol.detail {
        SymbolDetail::Subckt { ports, params, .. } => (
            ports
                .iter()
                .map(|port| format!("`{}`", port.text))
                .collect::<Vec<_>>()
                .join(", "),
            params
                .iter()
                .map(|param| format!("`{}`", param.name.text))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => (String::new(), String::new()),
    };
    let params_line = if params.is_empty() {
        String::new()
    } else {
        format!("\n\nParameters: {params}")
    };
    format!(
        "```spice\n{header}\n```\nSubcircuit **{}**\n\nPorts: {ports}{params_line}\n\n{}",
        symbol.name.text,
        origin_line(document, symbol)
    )
}

/// Renders the documentation of a model card.
///
/// # Arguments
/// * `document` - analysed document
/// * `symbol` - model symbol
fn model_text(document: &Document, symbol: &Symbol) -> String {
    let card = truncate_lines(definition_text(document, symbol).trim(), MAX_MODEL_LINES);
    let (type_line, level_line) = match &symbol.detail {
        SymbolDetail::Model { model_type, level } => (
            model_type
                .as_ref()
                .and_then(|kind| find_model_type(&kind.key))
                .map_or(String::new(), |spec| {
                    format!("\n\nType: {}", spec.description)
                }),
            level.map_or(String::new(), |level| {
                level_text(model_type.as_ref().map(|kind| kind.key.as_str()), level)
            }),
        ),
        _ => (String::new(), String::new()),
    };
    let base = symbol
        .name
        .key
        .rsplit_once('.')
        .filter(|(_, bin)| bin.chars().all(|c| c.is_ascii_digit()))
        .map_or(symbol.name.key.as_str(), |(base, _)| base);
    let bins = document.analysis.table.bins(base).len();
    let bins_line = if bins > 0 {
        format!("\n\nBinned model: {bins} bins of `{base}`")
    } else {
        String::new()
    };
    format!(
        "```spice\n{card}\n```{type_line}{level_line}{bins_line}\n\n{}",
        origin_line(document, symbol)
    )
}

/// Describes the `LEVEL=` value of a MOSFET model.
///
/// # Arguments
/// * `model_type` - lowercase model type
/// * `level` - `LEVEL=` value
fn level_text(model_type: Option<&str>, level: f64) -> String {
    let is_mos = matches!(model_type, Some("nmos" | "pmos"));
    let name = (is_mos && level >= 0.0 && level.fract() == 0.0)
        .then(|| mosfet_level_name(level as u32))
        .flatten();
    match name {
        Some(name) => format!("\n\nLEVEL={level}: {name}"),
        None => format!("\n\nLEVEL={level}"),
    }
}

/// Renders the documentation of a node.
///
/// # Arguments
/// * `document` - analysed document
/// * `id` - node symbol id
fn node_text(document: &Document, id: SymbolId) -> String {
    let analysis = &document.analysis;
    let symbol = &analysis.symbols[id];
    let scope = scope_name(document, symbol.scope).map_or_else(
        || "top level".to_string(),
        |name| format!("subcircuit `{name}`"),
    );
    let note = match symbol.detail {
        SymbolDetail::Node { global: true, .. } => {
            "\n\nGlobal node (`.global`), visible in every scope.".to_string()
        }
        SymbolDetail::Node {
            port_index: Some(index),
            ..
        } => format!(
            "\n\nPort {} of subcircuit `{}`.",
            index + 1,
            scope_name(document, symbol.scope).unwrap_or_default()
        ),
        _ => String::new(),
    };
    let connections: Vec<String> = analysis
        .references
        .iter()
        .filter(|reference| reference.target == Some(id))
        .filter_map(|reference| connection_label(document, reference.context))
        .collect();
    let shown = connections
        .iter()
        .take(MAX_CONNECTIONS)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let more = connections.len().saturating_sub(MAX_CONNECTIONS);
    let list = match (connections.len(), more) {
        (0, _) => "\n\nNo element connections.".to_string(),
        (count, 0) => format!("\n\n{count} connection(s): {shown}"),
        (count, more) => format!("\n\n{count} connection(s): {shown}, \u{2026} (+{more})"),
    };
    format!(
        "**{}** \u{2014} node in {scope}{note}{list}",
        symbol.name.text
    )
}

/// Labels an element connection of a node (`M1.d`, `X1(in)`).
///
/// # Arguments
/// * `document` - analysed document
/// * `context` - reference context
fn connection_label(document: &Document, context: RefContext) -> Option<String> {
    let analysis = &document.analysis;
    match context {
        RefContext::Terminal { element, terminal } => {
            let info = analysis.elements.get(element)?;
            let reference = info
                .roles
                .reference
                .as_ref()
                .and_then(|name| analysis.lookup(SymbolKind::Subckt, info.scope, &name.key));
            let ports = terminal_names(info.spec?, reference.map(|id| &analysis.symbols[id]));
            let terminal_name = ports
                .get(terminal)
                .cloned()
                .unwrap_or_else(|| format!("#{}", terminal + 1));
            Some(if info.spec?.letter == 'X' {
                format!("`{}({terminal_name})`", info.name.text)
            } else {
                format!("`{}.{terminal_name}`", info.name.text)
            })
        }
        RefContext::ControlNode { element } => analysis
            .elements
            .get(element)
            .map(|info| format!("`{}(ctrl)`", info.name.text)),
        _ => None,
    }
}

/// Renders the hover of a ground node connected to an element.
///
/// # Arguments
/// * `document` - analysed document
/// * `statement` - statement index
/// * `offset` - cursor offset
fn ground_hover(
    document: &Document,
    statement: usize,
    offset: usize,
) -> Option<(TextRange, String)> {
    let info = document.analysis.element_at_statement(statement)?;
    let node = info
        .roles
        .nodes
        .iter()
        .find(|node| node.range.touches(offset) && is_ground(&node.text))?;
    Some((node.range, GROUND_TEXT.to_string()))
}

/// Renders the hover of a resolved include path.
///
/// # Arguments
/// * `document` - analysed document
/// * `statement` - statement index
/// * `offset` - cursor offset
fn include_path_hover(
    document: &Document,
    statement: usize,
    offset: usize,
) -> Option<(TextRange, String)> {
    let resolution = document.analysis.includes.iter().find(|resolution| {
        resolution.directive.statement == statement
            && resolution.directive.path.range.touches(offset)
    })?;
    let text = match &resolution.resolved {
        Some(path) => format!("Resolved to `{}`", path.display()),
        None => "File not found".to_string(),
    };
    Some((resolution.directive.path.range, text))
}

/// Collects the hoverable tokens of one argument.
///
/// # Arguments
/// * `arg` - argument
fn arg_leaves(arg: &Arg) -> Vec<Leaf<'_>> {
    match arg {
        Arg::Value(value) => value_leaves(value),
        Arg::Group { args, .. } => args.iter().flat_map(arg_leaves).collect(),
        Arg::Assign(assignment) => {
            let key = match &assignment.key {
                AssignKey::Name(name) => vec![Leaf::Key(name)],
                AssignKey::Call(call) => std::iter::once(Leaf::Call(&call.name))
                    .chain(call.args.iter().flat_map(arg_leaves))
                    .collect(),
            };
            key.into_iter()
                .chain(assignment.value.iter().flat_map(value_leaves))
                .chain(assignment.extra_values.iter().flat_map(value_leaves))
                .collect()
        }
    }
}

/// Collects the hoverable tokens of one value.
///
/// # Arguments
/// * `value` - positional or assigned value
fn value_leaves(value: &Value) -> Vec<Leaf<'_>> {
    match value {
        Value::Number(number) => vec![Leaf::Number(&number.text, number.range)],
        Value::Word(word) => vec![Leaf::Word(word)],
        Value::Expr(expr) => expr_leaves(expr),
        Value::Str(_) => Vec::new(),
        Value::Call(call) => std::iter::once(Leaf::Call(&call.name))
            .chain(call.args.iter().flat_map(arg_leaves))
            .collect(),
    }
}

/// Collects the hoverable tokens of an expression.
///
/// # Arguments
/// * `expr` - expression
fn expr_leaves(expr: &Expr) -> Vec<Leaf<'_>> {
    match &expr.kind {
        ExprKind::Number(text) => vec![Leaf::Number(text, expr.range)],
        ExprKind::Ident(ident) => vec![Leaf::ExprName(ident)],
        ExprKind::Unary { operand, .. } => expr_leaves(operand),
        ExprKind::Binary { left, right, .. } => expr_leaves(left)
            .into_iter()
            .chain(expr_leaves(right))
            .collect(),
        ExprKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => [condition, then_branch, else_branch]
            .into_iter()
            .flat_map(|branch| expr_leaves(branch))
            .collect(),
        ExprKind::Call { name, args } => std::iter::once(Leaf::Call(name))
            .chain(args.iter().flat_map(expr_leaves))
            .collect(),
        ExprKind::Error => Vec::new(),
    }
}

/// Collects the hoverable tokens of a statement.
///
/// # Arguments
/// * `kind` - statement kind
fn statement_leaves(kind: &StatementKind) -> Vec<Leaf<'_>> {
    match kind {
        StatementKind::Element(Element { args, .. }) => args.iter().flat_map(arg_leaves).collect(),
        StatementKind::Command(command) => command_leaves(&command.kind),
        StatementKind::Comment | StatementKind::Error => Vec::new(),
    }
}

/// Collects the hoverable tokens of a command payload.
///
/// # Arguments
/// * `kind` - command payload
fn command_leaves(kind: &CommandKind) -> Vec<Leaf<'_>> {
    match kind {
        CommandKind::Param { definitions } => definitions
            .iter()
            .filter_map(|def| def.value.as_ref())
            .flat_map(expr_leaves)
            .collect(),
        CommandKind::Subckt { params, .. } => params
            .iter()
            .filter_map(|def| def.value.as_ref())
            .flat_map(expr_leaves)
            .collect(),
        CommandKind::Model {
            model_type, args, ..
        } => model_type
            .iter()
            .map(Leaf::Word)
            .chain(args.iter().flat_map(arg_leaves))
            .collect(),
        CommandKind::Option { args }
        | CommandKind::Generic { args }
        | CommandKind::Hdl { args, .. } => args.iter().flat_map(arg_leaves).collect(),
        CommandKind::If { condition } | CommandKind::ElseIf { condition } => {
            condition.iter().flat_map(expr_leaves).collect()
        }
        _ => Vec::new(),
    }
}

/// Renders the hover of the smallest token under the cursor.
///
/// # Arguments
/// * `kind` - statement under the cursor
/// * `offset` - cursor offset
fn leaf_hover(kind: &StatementKind, offset: usize) -> Option<(TextRange, String)> {
    let command = statement_command(kind).and_then(|command| find_command(&command.keyword.text));
    let element = match kind {
        StatementKind::Element(element) => find_element(&element.name.text),
        _ => None,
    };
    let model_type = match kind {
        StatementKind::Command(Command {
            kind:
                CommandKind::Model {
                    model_type: Some(kind),
                    ..
                },
            ..
        }) => Some(kind.key.as_str()),
        _ => None,
    };
    let context = Context {
        command,
        element,
        model_type,
    };
    let leaf = statement_leaves(kind)
        .into_iter()
        .filter(|leaf| leaf.range().touches(offset))
        .min_by_key(|leaf| leaf.range().len())?;
    leaf_text(context, leaf).map(|text| (leaf.range(), text))
}

/// Renders the documentation of one token.
///
/// # Arguments
/// * `context` - statement context
/// * `leaf` - token
fn leaf_text(context: Context, leaf: Leaf) -> Option<String> {
    match leaf {
        Leaf::Number(text, _) => number_text(text),
        Leaf::Call(name) => call_text(context, name),
        Leaf::ExprName(name) => find_special_variable(&name.key).map(|spec| {
            format!(
                "**{}** \u{2014} predefined variable\n\n{}",
                spec.name, spec.description
            )
        }),
        Leaf::Word(word) => word_text(context, word),
        Leaf::Key(key) => key_text(context, key),
    }
}

/// Returns the English name of a scale-factor suffix.
///
/// # Arguments
/// * `suffix` - canonical suffix
fn scale_name(suffix: &str) -> &str {
    match suffix {
        "t" => "tera",
        "g" => "giga",
        "meg" => "mega",
        "x" => "mega (x)",
        "k" => "kilo",
        "mil" => "mil (25.4e-6)",
        "m" => "milli",
        "u" => "micro",
        "n" => "nano",
        "p" => "pico",
        "f" => "femto",
        "a" => "atto",
        "r" => "ronto",
        other => other,
    }
}

/// Renders the documentation of a numeric literal.
///
/// # Arguments
/// * `text` - literal as written
fn number_text(text: &str) -> Option<String> {
    let parsed = parse_number(text)?;
    let engineering = format_engineering(parsed.value);
    let mut lines = vec![format!(
        "`{text}` = `{engineering}` = `{}`",
        scientific(parsed.value)
    )];
    lines.extend(scale_notes(text, &parsed));
    Some(lines.join("\n\n"))
}

/// Returns notes about the scale factor and ignored unit letters of a literal.
///
/// # Arguments
/// * `text` - literal as written
/// * `parsed` - decoded literal
fn scale_notes(text: &str, parsed: &ParsedNumber) -> Vec<String> {
    let scale_note = parsed.scale.map(|scale| {
        if scale.suffix == "m" {
            let mantissa = text.trim_end_matches(|character: char| character.is_ascii_alphabetic());
            let mega = mantissa.parse::<f64>().map_or(String::new(), |value| {
                format!(" for {}", scientific(value * 1e6))
            });
            format!("`m`/`M` means milli (case-insensitive); use `{mantissa}MEG`{mega} for mega.")
        } else {
            format!(
                "Scale factor `{}` = {} (1e{}).",
                scale.suffix,
                scale_name(scale.suffix),
                scale.multiplier.log10().round()
            )
        }
    });
    let unit_note = (!parsed.unit.is_empty())
        .then(|| format!("Trailing letters `{}` are ignored as a unit.", parsed.unit));
    scale_note.into_iter().chain(unit_note).collect()
}

/// Renders the documentation of a call name (source function, output variable or built-in function).
///
/// # Arguments
/// * `context` - statement context
/// * `name` - call name
fn call_text(context: Context, name: &Ident) -> Option<String> {
    let source = find_source_function(&name.key).map(source_function_text);
    let output = find_output_var(&name.key)
        .or_else(|| {
            find_output_var(
                name.key
                    .trim_end_matches(|character: char| character.is_ascii_digit()),
            )
        })
        .map(|spec| format!("```spice\n{}\n```\n{}", spec.signature, spec.description));
    let function = find_function(&name.key)
        .map(|spec| format!("```spice\n{}\n```\n{}", spec.signature, spec.description));
    let source_first = context
        .element
        .is_some_and(|spec| matches!(spec.letter, 'V' | 'I'));
    if source_first {
        source.or(function).or(output)
    } else {
        output.or(function).or(source)
    }
}

/// Renders the documentation of a transient source function.
///
/// # Arguments
/// * `spec` - source function table entry
fn source_function_text(spec: &SourceFunctionSpec) -> String {
    let names: Vec<&str> = spec
        .arguments
        .iter()
        .map(|argument| argument.name)
        .collect();
    format!(
        "```spice\n{}({})\n```\n{}{}\n\n{} of {} arguments required.",
        spec.name.to_ascii_uppercase(),
        names.join(" "),
        spec.description,
        argument_docs(spec.arguments),
        spec.required,
        spec.arguments.len()
    )
}

/// Renders the documentation of a bare word in its statement context.
///
/// # Arguments
/// * `context` - statement context
/// * `word` - the word
fn word_text(context: Context, word: &Ident) -> Option<String> {
    let command = context.command.map(|spec| spec.name);
    let keyword = match command {
        Some(".option") => find_option(&word.key).map(option_text),
        Some(".measure") => find_measure_keyword(&word.key)
            .map(|spec| {
                format!(
                    "**{}** \u{2014} `.measure` keyword\n\n{}",
                    spec.name, spec.description
                )
            })
            .or_else(|| analysis_type_text(&word.key)),
        Some(".print" | ".probe" | ".plot" | ".graph") => analysis_type_text(&word.key),
        Some(".model") if context.model_type == Some(word.key.as_str()) => MODEL_TYPES
            .iter()
            .find(|spec| spec.name == word.key)
            .map(|spec| {
                format!(
                    "**{}** \u{2014} model type\n\n{}",
                    spec.name, spec.description
                )
            }),
        Some(_) => find_analysis_keyword(&word.key).map(|spec| {
            format!(
                "**{}** \u{2014} analysis keyword\n\n{}",
                spec.name, spec.description
            )
        }),
        None => None,
    };
    keyword.or_else(|| {
        (context.element.is_some() && is_ground(&word.text)).then(|| GROUND_TEXT.to_string())
    })
}

/// Renders the documentation of a result type such as `tran` or `ac`.
///
/// # Arguments
/// * `key` - lowercase word
fn analysis_type_text(key: &str) -> Option<String> {
    find_named(ANALYSIS_TYPES, key).map(|spec| {
        format!(
            "**{}** \u{2014} result type\n\n{}",
            spec.name, spec.description
        )
    })
}

/// Renders the documentation of an `.option` entry.
///
/// # Arguments
/// * `spec` - option table entry
fn option_text(spec: &crate::lang::options::OptionSpec) -> String {
    let form = match spec.value {
        OptionValue::Flag => "flag",
        OptionValue::Value => "value",
        OptionValue::FlagOrValue => "flag or value",
    };
    let default = spec
        .default
        .map_or(String::new(), |default| format!(", default `{default}`"));
    let status = if spec.status == Status::Obsolete {
        "\n\n_Obsolete._"
    } else {
        ""
    };
    format!(
        "**.option {}** ({form}{default})\n\n{}{status}",
        spec.name, spec.description
    )
}

/// Renders the documentation of an assignment key in its statement context.
///
/// # Arguments
/// * `context` - statement context
/// * `key` - assignment key
fn key_text(context: Context, key: &Ident) -> Option<String> {
    let command = context.command.map(|spec| spec.name);
    match command {
        Some(".option") => find_option(&key.key).map(option_text),
        Some(".model") => context.model_type.and_then(|kind| {
            model_params_for(kind)
                .iter()
                .find(|param| param.name == key.key)
                .map(|param| {
                    format!(
                        "**{}** \u{2014} model parameter\n\n{}",
                        param.name, param.description
                    )
                })
        }),
        Some(".mosra") => find_mosra_param(&key.key).map(|spec| {
            format!(
                "**{}** \u{2014} `.MOSRA` parameter\n\n{}",
                spec.name, spec.description
            )
        }),
        Some(".measure") => find_measure_keyword(&key.key).map(|spec| {
            format!(
                "**{}** \u{2014} `.measure` keyword\n\n{}",
                spec.name, spec.description
            )
        }),
        Some(_) => find_analysis_keyword(&key.key).map(|spec| {
            format!(
                "**{}** \u{2014} analysis keyword\n\n{}",
                spec.name, spec.description
            )
        }),
        None => context.element.and_then(|spec| {
            spec.params
                .iter()
                .find(|param| param.name == key.key)
                .map(|param| {
                    format!(
                        "**{}** \u{2014} {} parameter\n\n{}",
                        param.name, spec.name, param.description
                    )
                })
        }),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::super::completion::test_support::{Opened, open, open_in_fixtures};
    use super::*;

    fn hover_at(opened: &Opened) -> Option<Hover> {
        let document = opened.workspace.document(&opened.uri)?;
        hover(&opened.workspace, document, opened.position)
    }

    fn text_of(marked: &str) -> String {
        match hover_at(&open(marked)).map(|hover| hover.contents) {
            Some(HoverContents::Markup(markup)) => markup.value,
            _ => String::new(),
        }
    }

    #[test]
    fn hovers_dot_commands() {
        let text = text_of("* t\n.tr|an 1n 10n");
        assert!(text.contains("```spice") && text.contains(".TRAN") && text.contains("Transient"));
        let hover = hover_at(&open("* t\n.tr|an 1n 10n")).expect("hover");
        assert_eq!(
            hover
                .range
                .map(|range| (range.start.character, range.end.character)),
            Some((0, 5))
        );
    }

    #[test]
    fn hovers_obsolete_and_foreign_commands() {
        use crate::lang::commands::COMMANDS;
        let hover_of = |status: Status| {
            let name = COMMANDS
                .iter()
                .find(|spec| spec.status == status)
                .map(|spec| spec.name)
                .unwrap_or(".tran");
            text_of(&format!("* t\n{name}| x"))
        };
        assert!(hover_of(Status::Obsolete).contains("Obsolete"));
        assert!(hover_of(Status::Foreign).contains("Not HSPICE"));
    }

    #[test]
    fn hovers_element_with_model_and_terminals() {
        let text = text_of("* t\n.model nch nmos level=54\nM|1 out in 0 0 nch w=1u");
        assert!(text.contains("**M1**") && text.contains("MOSFET"));
        assert!(text.contains(".model nch nmos level=54"));
        assert!(text.contains("| `d` | `out` |") && text.contains("| `g` | `in` |"));
    }

    #[test]
    fn hovers_x_instances_with_ports() {
        let text = text_of("* t\n.subckt inv in out\n.ends\nX|1 a b inv");
        assert!(text.contains("| `in` | `a` |") && text.contains("| `out` | `b` |"));
    }

    #[test]
    fn hovers_params_with_values() {
        let text = text_of("* t\n.param wn=200n wp='2*wn'\nR1 a b 'w|p'");
        assert!(
            text.contains("wp='2*wn'") && text.contains("= `400n`"),
            "{text}"
        );
        assert!(text.contains("Defined in this file"));
        let local = text_of("* t\n.subckt s a w=2\nR1 a 0 'w|*3'\n.ends");
        assert!(local.contains("Local to subcircuit `s`"));
    }

    #[test]
    fn param_cycles_do_not_hang() {
        let text = text_of("* t\n.param a='b+1' b='a+1'\nR1 x y 'a|'");
        assert!(text.contains("a='b+1'") && !text.contains("= `"));
    }

    #[test]
    fn hovers_inverter_fixture() {
        let text = std::fs::read_to_string(
            super::super::completion::test_support::fixtures().join("inverter_tb.sp"),
        )
        .expect("fixture");
        let marked = text.replacen("wn=wn", "wn=w|n", 1);
        let opened = open_in_fixtures(&marked);
        let Some(HoverContents::Markup(markup)) = hover_at(&opened).map(|hover| hover.contents)
        else {
            panic!("no hover")
        };
        assert!(markup.value.contains("200n"), "{}", markup.value);
        let marked = text.replacen("X1 in", "X1| in", 1);
        let opened = open_in_fixtures(&marked);
        let Some(HoverContents::Markup(markup)) = hover_at(&opened).map(|hover| hover.contents)
        else {
            panic!("no hover")
        };
        assert!(
            markup.value.contains("| `in` | `in` |")
                && markup.value.contains(".subckt inv in out vdd gnd")
        );
        let marked = text.replacen("inv wn", "in|v wn", 1);
        let opened = open_in_fixtures(&marked);
        let Some(HoverContents::Markup(markup)) = hover_at(&opened).map(|hover| hover.contents)
        else {
            panic!("no hover")
        };
        assert!(markup.value.contains("Ports:") && markup.value.contains("inv.inc"));
    }

    #[test]
    fn hovers_models_with_level_and_bins() {
        let text = text_of(
            "* t\n.model nch.1 nmos level=54 vth0=0.4\n.model nch.2 nmos level=54\nM1 d g s b nc|h w=1u",
        );
        assert!(
            text.contains(".model nch.1 nmos")
                && text.contains("LEVEL=54")
                && text.contains("2 bins"),
            "{text}"
        );
        let long = text_of(
            "* t\n.model big nmos level=1\n+ a=1\n+ b=1\n+ c=1\n+ d=1\n+ e=1\n+ f=1\n+ g=1\n+ h=1\nM1 d g s b bi|g",
        );
        assert!(long.contains("more lines"));
    }

    #[test]
    fn hovers_nodes() {
        let text = text_of("* t\n.global vdd\nM1 out in vdd vdd nch\nR1 o|ut 0 1k");
        assert!(
            text.contains("**out**")
                && text.contains("`M1.d`")
                && text.contains("`R1.n1`")
                && text.contains("2 connection"),
            "{text}"
        );
        let global = text_of("* t\n.global vdd\nM1 out in v|dd vdd nch");
        assert!(global.contains("Global node"));
        let port = text_of("* t\n.subckt s a|\nR1 a b 1k\n.ends");
        assert!(
            port.contains("Port 1") && port.contains("subcircuit `s`"),
            "{port}"
        );
        assert!(text_of("* t\nR1 a 0| 1k").contains("Ground"));
    }

    #[test]
    fn hovers_numbers_with_meg_warning() {
        let text = text_of("* t\nR1 a b 10|M");
        assert!(text.contains("`10M` = `10m` = `1.0e-2`"), "{text}");
        assert!(text.contains("10MEG") && text.contains("1.0e7"));
        let kilo = text_of("* t\nR1 a b 2.|2k");
        assert!(
            kilo.contains("`2.2k` = `2.2k` = `2.2e3`") && kilo.contains("kilo"),
            "{kilo}"
        );
        assert!(text_of("* t\n.param x='3|u*2'").contains("micro"));
    }

    #[test]
    fn hovers_functions_variables_and_outputs() {
        assert!(text_of("* t\n.param x='ma|x(1,2)'").contains("max("));
        assert!(text_of("* t\nR1 a b 'ti|me'").contains("predefined variable"));
        assert!(text_of("* t\n.print tran v|(a)").contains("V(n1<,n2>)"));
        assert!(text_of("* t\nVin a 0 pu|lse(0 1)").contains("PULSE(v1 v2"));
        let user = text_of("* t\n.param f(x)='x*2'\nR1 a b 'f|(3)'");
        assert!(user.contains("user function") && user.contains("f(x)"));
    }

    #[test]
    fn hovers_options_and_keywords() {
        assert!(text_of("* t\n.option po|st").contains("**.option post**"));
        assert!(text_of("* t\n.ac de|c 10 1 1meg").contains("analysis keyword"));
        assert!(text_of("* t\n.meas tran d tri|g v(a) val=1").contains("`.measure` keyword"));
        assert!(text_of("* t\n.meas tr|an d trig v(a) val=1").contains("result type"));
        assert!(text_of("* t\n.model nch nm|os").contains("model type"));
        assert!(text_of("* t\n.model nch nmos le|vel=54").contains("model parameter"));
        assert!(text_of("* t\nR1 a b 1k t|c1=1").contains("First-order temperature coefficient"));
        assert!(text_of("* t\n.mosra relto|taltime=1e8").contains("`.MOSRA` parameter"));
        assert!(text_of("* t\n.option runl|vl=3").contains("speed/accuracy"));
    }

    #[test]
    fn hovers_explain_arguments() {
        assert!(text_of("* t\n.tr|an 1n 10n").contains("- `tstop` \u{2014} End time"));
        assert!(text_of("* t\nVin a 0 pul|se(0 1)").contains("- `pw` \u{2014} Pulse width"));
    }

    #[test]
    fn hovers_include_paths() {
        let opened = open_in_fixtures("* t\n.include 'cells/in|v.inc'");
        let Some(HoverContents::Markup(markup)) = hover_at(&opened).map(|hover| hover.contents)
        else {
            panic!("no hover")
        };
        assert!(markup.value.contains("inv.inc"));
    }

    #[test]
    fn nothing_to_hover_on_blank_space() {
        assert!(hover_at(&open("* t\nR1 a b 1k\n|\n")).is_none());
    }
}
