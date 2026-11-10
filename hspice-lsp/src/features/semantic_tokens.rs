//! Semantic tokens refining the tree-sitter highlighting with resolved symbol kinds.
//!
//! Only standard token types are used so that colorschemes need no special support. Tokens are
//! collected with a priority per byte range (syntax < symbol resolution < lexical fixes), sorted,
//! stripped of overlaps and delta-encoded in the negotiated position encoding.

use std::collections::{HashMap, HashSet};

use ls_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
    SemanticTokensResult,
};
use tree_sitter::Node;

use crate::analysis::symbols::{RefContext, Symbol, SymbolId, SymbolKind};
use crate::lang::commands::find_command;
use crate::lang::elements::{find_source_function, is_ground};
use crate::lang::functions::{find_function, find_special_variable, is_optimization_function};
use crate::lang::measure::{ANALYSIS_TYPES, find_analysis_keyword, find_measure_keyword};
use crate::lang::options::find_option;
use crate::lang::output_vars::{OutputArgKind, find_output_var};
use crate::lang::{Status, find_named};
use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::{
    Arg, AssignKey, Call, Command, CommandKind, Element, Expr, ExprKind, Ident, ParamDef,
    StatementKind, TextRange, Value,
};
use crate::syntax::node_kinds::{COMMENT, INLINE_COMMENT, LINE_CONTINUATION};

/// Token types in legend order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Namespace,
    Type,
    Class,
    EnumMember,
    Parameter,
    Variable,
    Property,
    Function,
    Keyword,
    Comment,
    String,
    Number,
}

/// All token kinds in the order they appear in the legend.
const KINDS: [TokenKind; 12] = [
    TokenKind::Namespace,
    TokenKind::Type,
    TokenKind::Class,
    TokenKind::EnumMember,
    TokenKind::Parameter,
    TokenKind::Variable,
    TokenKind::Property,
    TokenKind::Function,
    TokenKind::Keyword,
    TokenKind::Comment,
    TokenKind::String,
    TokenKind::Number,
];

/// Modifier bit: defining occurrence (declaration).
const DECLARATION: u32 = 1 << 0;
/// Modifier bit: defining occurrence (definition).
const DEFINITION: u32 = 1 << 1;
/// Modifier bit: value cannot be reassigned.
const READONLY: u32 = 1 << 2;
/// Modifier bit: provided by HSPICE itself.
const DEFAULT_LIBRARY: u32 = 1 << 3;
/// Modifier bit: obsolete construct.
const DEPRECATED: u32 = 1 << 4;
/// Both defining-occurrence bits.
const DEFINING: u32 = DECLARATION | DEFINITION;

/// Priority of purely syntactic tokens.
const SYNTAX: u8 = 0;
/// Priority of tokens derived from symbol resolution.
const SYMBOL: u8 = 1;
/// Priority of lexical facts that must win over resolution (keywords, built-ins, ground).
const LEXICAL: u8 = 2;

impl TokenKind {
    /// Returns the standard LSP token type of the kind.
    fn lsp_type(self) -> SemanticTokenType {
        match self {
            TokenKind::Namespace => SemanticTokenType::NAMESPACE,
            TokenKind::Type => SemanticTokenType::TYPE,
            TokenKind::Class => SemanticTokenType::CLASS,
            TokenKind::EnumMember => SemanticTokenType::ENUM_MEMBER,
            TokenKind::Parameter => SemanticTokenType::PARAMETER,
            TokenKind::Variable => SemanticTokenType::VARIABLE,
            TokenKind::Property => SemanticTokenType::PROPERTY,
            TokenKind::Function => SemanticTokenType::FUNCTION,
            TokenKind::Keyword => SemanticTokenType::KEYWORD,
            TokenKind::Comment => SemanticTokenType::COMMENT,
            TokenKind::String => SemanticTokenType::STRING,
            TokenKind::Number => SemanticTokenType::NUMBER,
        }
    }

    /// Returns the legend index of the kind.
    fn index(self) -> u32 {
        KINDS
            .iter()
            .position(|kind| *kind == self)
            .unwrap_or_default() as u32
    }
}

/// A token before position conversion.
#[derive(Debug, Clone, Copy)]
struct RawToken {
    kind: TokenKind,
    modifiers: u32,
    priority: u8,
}

/// Collects tokens by byte range, keeping the highest priority per range.
#[derive(Default)]
struct Sink {
    tokens: HashMap<TextRange, RawToken>,
}

impl Sink {
    /// Adds a token unless a higher-priority token already covers the same range.
    ///
    /// # Arguments
    /// * `range` - byte range of the token
    /// * `kind` - token type
    /// * `modifiers` - modifier bit set
    /// * `priority` - conflict resolution priority
    fn push(&mut self, range: TextRange, kind: TokenKind, modifiers: u32, priority: u8) {
        if range.is_empty() {
            return;
        }
        let token = RawToken {
            kind,
            modifiers,
            priority,
        };
        let replace = self
            .tokens
            .get(&range)
            .is_none_or(|existing| existing.priority <= priority);
        if replace {
            self.tokens.insert(range, token);
        }
    }
}

/// Which bare words count as keywords in the current context.
#[derive(Clone, Copy)]
enum Keywords<'a> {
    /// No bare word is a keyword.
    None,
    /// Words listed by the element type.
    List(&'a [&'static str]),
    /// Analysis, sweep and measure keywords of dot commands.
    Command,
}

impl Keywords<'_> {
    /// Returns true if the lowercase word is a keyword in this context.
    ///
    /// # Arguments
    /// * `key` - lowercase word
    fn contains(&self, key: &str) -> bool {
        match self {
            Keywords::None => false,
            Keywords::List(list) => list.contains(&key),
            Keywords::Command => {
                find_analysis_keyword(key).is_some()
                    || find_measure_keyword(key).is_some()
                    || find_named(ANALYSIS_TYPES, key).is_some()
            }
        }
    }
}

/// Returns the token types and modifiers advertised to the client.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: KINDS.iter().map(|kind| kind.lsp_type()).collect(),
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::DEFINITION,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::DEFAULT_LIBRARY,
            SemanticTokenModifier::DEPRECATED,
        ],
    }
}

/// Computes semantic tokens for a whole document.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document to tokenize
pub fn semantic_tokens(workspace: &Workspace, document: &Document) -> Option<SemanticTokensResult> {
    let mut sink = Sink::default();
    collect_comments(document, &mut sink);
    collect_statements(document, &mut sink);
    collect_resolution(document, &mut sink);
    let data = encode(workspace, document, sink);
    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    }))
}

/// Sorts, de-overlaps and delta-encodes collected tokens.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document the tokens belong to
/// * `sink` - collected tokens
fn encode(workspace: &Workspace, document: &Document, sink: Sink) -> Vec<SemanticToken> {
    let mut ranges: Vec<(TextRange, RawToken)> = sink.tokens.into_iter().collect();
    ranges.sort_by_key(|(range, _)| *range);
    let mut last_end = 0;
    let kept: Vec<(TextRange, RawToken)> = ranges
        .into_iter()
        .filter(|(range, _)| {
            let keep = range.start >= last_end;
            if keep {
                last_end = range.end;
            }
            keep
        })
        .collect();
    let positioned = kept.iter().filter_map(|(range, token)| {
        let start = document
            .line_index
            .position(&document.text, range.start, workspace.encoding);
        let end = document
            .line_index
            .position(&document.text, range.end, workspace.encoding);
        (start.line == end.line && end.character > start.character).then_some((
            start.line,
            start.character,
            end.character - start.character,
            *token,
        ))
    });
    let mut previous = (0, 0);
    positioned
        .map(|(line, column, length, token)| {
            let delta_line = line - previous.0;
            let delta_start = if delta_line == 0 {
                column - previous.1
            } else {
                column
            };
            previous = (line, column);
            SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type: token.kind.index(),
                token_modifiers_bitset: token.modifiers,
            }
        })
        .collect()
}

/// Splits a byte range at line breaks and drops empty pieces and carriage returns.
///
/// # Arguments
/// * `text` - document text
/// * `range` - range to split
fn line_pieces(text: &str, range: TextRange) -> Vec<TextRange> {
    let slice = text.get(range.start..range.end).unwrap_or_default();
    let mut offset = range.start;
    slice
        .split_inclusive('\n')
        .filter_map(|line| {
            let start = offset;
            offset += line.len();
            let content = line.trim_end_matches(['\n', '\r']);
            (!content.is_empty()).then(|| TextRange::new(start, start + content.len()))
        })
        .collect()
}

/// Returns the comment lines hidden inside a `+` continuation token.
///
/// # Arguments
/// * `text` - document text
/// * `range` - range of the `line_continuation` node
fn continuation_comments(text: &str, range: TextRange) -> Vec<TextRange> {
    line_pieces(text, range)
        .into_iter()
        .filter_map(|piece| {
            let line = text.get(piece.start..piece.end).unwrap_or_default();
            let trimmed = line.trim_start();
            trimmed
                .starts_with(['*', '$'])
                .then(|| TextRange::new(piece.end - trimmed.len(), piece.end))
        })
        .collect()
}

/// Returns all comment ranges below a CST node.
///
/// # Arguments
/// * `node` - CST node
/// * `text` - document text
fn comment_ranges(node: Node, text: &str) -> Vec<TextRange> {
    let range = TextRange::new(node.start_byte(), node.end_byte());
    match node.kind() {
        COMMENT | INLINE_COMMENT => vec![range],
        LINE_CONTINUATION => continuation_comments(text, range),
        _ => {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .flat_map(|child| comment_ranges(child, text))
                .collect()
        }
    }
}

/// Adds comment tokens: comment lines, inline comments, hidden continuation comments and titles.
///
/// # Arguments
/// * `document` - document to scan
/// * `sink` - token collector
fn collect_comments(document: &Document, sink: &mut Sink) {
    let from_tree = document
        .syntax
        .tree
        .as_ref()
        .map(|tree| comment_ranges(tree.root_node(), &document.text))
        .unwrap_or_default();
    let titles = document
        .syntax
        .title_ranges
        .iter()
        .flat_map(|range| line_pieces(&document.text, *range));
    from_tree
        .into_iter()
        .chain(titles)
        .for_each(|range| sink.push(range, TokenKind::Comment, 0, LEXICAL));
}

/// Adds syntax-derived tokens for every non-skipped statement.
///
/// # Arguments
/// * `document` - document to scan
/// * `sink` - token collector
fn collect_statements(document: &Document, sink: &mut Sink) {
    let analysis = &document.analysis;
    document
        .syntax
        .netlist
        .statements
        .iter()
        .enumerate()
        .filter(|(index, _)| !analysis.structure.is_skipped(*index))
        .for_each(|(index, statement)| match &statement.kind {
            StatementKind::Command(command) => command_tokens(command, sink),
            StatementKind::Element(element) => {
                let excluded = analysis
                    .element_at_statement(index)
                    .map(|info| role_ranges(&info.roles))
                    .unwrap_or_default();
                element_tokens(element, &excluded, sink);
                if let Some(info) = analysis.element_at_statement(index) {
                    info.roles
                        .nodes
                        .iter()
                        .chain(&info.roles.control_nodes)
                        .filter(|node| is_ground(&node.text))
                        .for_each(|node| ground(node.range, sink));
                }
            }
            StatementKind::Comment | StatementKind::Error => {}
        });
}

/// Returns the ranges of an element's nodes and reference (never keywords).
///
/// # Arguments
/// * `roles` - role assignment of the element
fn role_ranges(roles: &crate::analysis::roles::ElementRoles) -> HashSet<TextRange> {
    roles
        .nodes
        .iter()
        .chain(&roles.control_nodes)
        .chain(&roles.reference)
        .chain(&roles.controls)
        .map(|ident| ident.range)
        .collect()
}

/// Adds a ground-node token.
///
/// # Arguments
/// * `range` - range of the ground name
/// * `sink` - token collector
fn ground(range: TextRange, sink: &mut Sink) {
    sink.push(range, TokenKind::Variable, DEFAULT_LIBRARY, LEXICAL);
}

/// Adds tokens of an element line (the name itself comes from symbol resolution).
///
/// # Arguments
/// * `element` - lowered element
/// * `excluded` - ranges that are nodes or references, not keywords
/// * `sink` - token collector
fn element_tokens(element: &Element, excluded: &HashSet<TextRange>, sink: &mut Sink) {
    let keywords = crate::lang::elements::find_element(&element.name.text)
        .map_or(Keywords::None, |spec| Keywords::List(spec.keywords));
    sink.push(element.name.range, TokenKind::EnumMember, 0, SYNTAX);
    element
        .args
        .iter()
        .for_each(|arg| walk_arg(arg, keywords, excluded, sink));
}

/// Adds tokens of a dot command.
///
/// # Arguments
/// * `command` - lowered command
/// * `sink` - token collector
fn command_tokens(command: &Command, sink: &mut Sink) {
    let deprecated = find_command(&command.keyword.text)
        .filter(|spec| spec.status == Status::Obsolete)
        .map_or(0, |_| DEPRECATED);
    sink.push(
        command.keyword.range,
        TokenKind::Keyword,
        deprecated,
        LEXICAL,
    );
    let none = HashSet::new();
    match &command.kind {
        CommandKind::Include { path } | CommandKind::Hdl { path, .. } => {
            path_token(path.as_ref(), sink)
        }
        CommandKind::Lib { path, section } => {
            path_token(path.as_ref(), sink);
            let modifiers = if path.is_none() { DEFINING } else { 0 };
            section
                .iter()
                .for_each(|name| sink.push(name.range, TokenKind::Namespace, modifiers, SYNTAX));
        }
        CommandKind::Param { definitions } => definitions
            .iter()
            .for_each(|definition| param_tokens(definition, sink)),
        CommandKind::Model {
            name,
            model_type,
            args,
        } => {
            name.iter()
                .for_each(|name| sink.push(name.range, TokenKind::Type, DEFINING, SYNTAX));
            model_type
                .iter()
                .for_each(|kind| sink.push(kind.range, TokenKind::Type, DEFAULT_LIBRARY, SYNTAX));
            args.iter()
                .for_each(|arg| walk_arg(arg, Keywords::None, &none, sink));
        }
        CommandKind::Subckt {
            name,
            ports,
            params,
        } => {
            name.iter()
                .for_each(|name| sink.push(name.range, TokenKind::Class, DEFINING, SYNTAX));
            ports
                .iter()
                .for_each(|port| sink.push(port.range, TokenKind::Variable, DECLARATION, SYNTAX));
            params
                .iter()
                .for_each(|definition| param_tokens(definition, sink));
        }
        CommandKind::Ends { name } => name
            .iter()
            .for_each(|name| sink.push(name.range, TokenKind::Class, 0, SYNTAX)),
        CommandKind::Endl { name } => name
            .iter()
            .for_each(|name| sink.push(name.range, TokenKind::Namespace, 0, SYNTAX)),
        CommandKind::Global { nodes } => nodes
            .iter()
            .for_each(|node| sink.push(node.range, TokenKind::Variable, READONLY, SYNTAX)),
        CommandKind::Option { args } => args.iter().for_each(|arg| option_tokens(arg, sink)),
        CommandKind::If { condition } | CommandKind::ElseIf { condition } => {
            condition.iter().for_each(|expr| walk_expr(expr, sink))
        }
        CommandKind::Generic { args } => args
            .iter()
            .for_each(|arg| walk_arg(arg, Keywords::Command, &none, sink)),
        CommandKind::End
        | CommandKind::Else
        | CommandKind::EndIf
        | CommandKind::Title { .. }
        | CommandKind::Alter { .. } => {}
    }
}

/// Adds a string token for a file path.
///
/// # Arguments
/// * `path` - path argument, if any
/// * `sink` - token collector
fn path_token(path: Option<&crate::syntax::ast::PathArg>, sink: &mut Sink) {
    path.iter()
        .for_each(|path| sink.push(path.range, TokenKind::String, 0, SYNTAX));
}

/// Adds tokens of one `.param` / header parameter definition.
///
/// # Arguments
/// * `definition` - parameter or user-function definition
/// * `sink` - token collector
fn param_tokens(definition: &ParamDef, sink: &mut Sink) {
    let kind = if definition.arguments.is_empty() {
        TokenKind::Parameter
    } else {
        TokenKind::Function
    };
    sink.push(definition.name.range, kind, DEFINING, SYNTAX);
    definition
        .arguments
        .iter()
        .for_each(|argument| sink.push(argument.range, TokenKind::Parameter, DECLARATION, SYNTAX));
    definition
        .value
        .iter()
        .for_each(|value| walk_expr(value, sink));
}

/// Adds tokens of one `.option` argument.
///
/// # Arguments
/// * `arg` - option argument
/// * `sink` - token collector
fn option_tokens(arg: &Arg, sink: &mut Sink) {
    let none = HashSet::new();
    let option_name = |name: &Ident, sink: &mut Sink| {
        let deprecated = find_option(&name.key)
            .filter(|spec| spec.status == Status::Obsolete)
            .map_or(0, |_| DEPRECATED);
        sink.push(name.range, TokenKind::Property, deprecated, LEXICAL);
    };
    match arg {
        Arg::Value(Value::Word(name)) => option_name(name, sink),
        Arg::Assign(assignment) => {
            if let AssignKey::Name(name) = &assignment.key {
                option_name(name, sink);
            }
            assignment
                .value
                .iter()
                .chain(&assignment.extra_values)
                .for_each(|value| walk_value(value, Keywords::None, &none, sink));
        }
        other => walk_arg(other, Keywords::None, &none, sink),
    }
}

/// Adds tokens of an argument.
///
/// # Arguments
/// * `arg` - argument to walk
/// * `keywords` - bare words that are keywords here
/// * `excluded` - ranges that are never keywords
/// * `sink` - token collector
fn walk_arg(arg: &Arg, keywords: Keywords, excluded: &HashSet<TextRange>, sink: &mut Sink) {
    match arg {
        Arg::Value(value) => walk_value(value, keywords, excluded, sink),
        Arg::Assign(assignment) => {
            match &assignment.key {
                AssignKey::Name(name) => sink.push(name.range, TokenKind::Property, 0, LEXICAL),
                AssignKey::Call(call) => walk_call(call, sink),
            }
            assignment
                .value
                .iter()
                .chain(&assignment.extra_values)
                .for_each(|value| walk_value(value, Keywords::None, excluded, sink));
        }
        Arg::Group { args, .. } => args
            .iter()
            .for_each(|inner| walk_arg(inner, keywords, excluded, sink)),
    }
}

/// Adds tokens of a value.
///
/// # Arguments
/// * `value` - value to walk
/// * `keywords` - bare words that are keywords here
/// * `excluded` - ranges that are never keywords
/// * `sink` - token collector
fn walk_value(value: &Value, keywords: Keywords, excluded: &HashSet<TextRange>, sink: &mut Sink) {
    match value {
        Value::Number(number) => sink.push(number.range, TokenKind::Number, 0, SYNTAX),
        Value::Word(word) => {
            if !excluded.contains(&word.range) && keywords.contains(&word.key) {
                sink.push(word.range, TokenKind::Keyword, 0, LEXICAL);
            }
        }
        Value::Str(path) => sink.push(path.range, TokenKind::String, 0, SYNTAX),
        Value::Expr(expr) => walk_expr(expr, sink),
        Value::Call(call) => {
            if keywords.contains(&call.name.key) && !is_builtin_call(&call.name.key) {
                sink.push(call.name.range, TokenKind::Keyword, 0, LEXICAL);
                call.args
                    .iter()
                    .for_each(|arg| walk_arg(arg, Keywords::None, excluded, sink));
            } else {
                walk_call(call, sink);
            }
        }
    }
}

/// Returns true for functions, output variables and source functions HSPICE provides.
///
/// # Arguments
/// * `key` - lowercase call name
fn is_builtin_call(key: &str) -> bool {
    find_function(key).is_some()
        || find_output_var(key).is_some()
        || find_source_function(key).is_some()
        || is_optimization_function(key)
}

/// Adds the name token of a call and walks its arguments.
///
/// # Arguments
/// * `call` - call-like argument
/// * `sink` - token collector
fn walk_call(call: &Call, sink: &mut Sink) {
    call_name(&call.name, sink);
    let node_args =
        find_output_var(&call.name.key).is_some_and(|spec| spec.argument == OutputArgKind::Nodes);
    let none = HashSet::new();
    call.args.iter().for_each(|arg| match arg {
        Arg::Value(Value::Word(word)) if node_args && is_ground(&word.text) => {
            ground(word.range, sink)
        }
        Arg::Value(Value::Number(number)) if node_args && is_ground(&number.text) => {
            ground(number.range, sink)
        }
        other => walk_arg(other, Keywords::None, &none, sink),
    });
}

/// Adds the token of a call name: built-ins are library functions, others plain functions.
///
/// # Arguments
/// * `name` - function name
/// * `sink` - token collector
fn call_name(name: &Ident, sink: &mut Sink) {
    if is_builtin_call(&name.key) {
        sink.push(name.range, TokenKind::Function, DEFAULT_LIBRARY, LEXICAL);
    } else {
        sink.push(name.range, TokenKind::Function, 0, SYNTAX);
    }
}

/// Adds tokens of an expression.
///
/// # Arguments
/// * `expr` - expression to walk
/// * `sink` - token collector
fn walk_expr(expr: &Expr, sink: &mut Sink) {
    match &expr.kind {
        ExprKind::Number(_) => sink.push(expr.range, TokenKind::Number, 0, SYNTAX),
        ExprKind::Ident(ident) => {
            if find_special_variable(&ident.key).is_some_and(|variable| !variable.redefinable) {
                sink.push(
                    ident.range,
                    TokenKind::Variable,
                    READONLY | DEFAULT_LIBRARY,
                    LEXICAL,
                );
            }
        }
        ExprKind::Unary { operand, .. } => walk_expr(operand, sink),
        ExprKind::Binary { left, right, .. } => {
            walk_expr(left, sink);
            walk_expr(right, sink);
        }
        ExprKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            [condition, then_branch, else_branch]
                .into_iter()
                .for_each(|branch| walk_expr(branch, sink));
        }
        ExprKind::Call { name, args } => {
            call_name(name, sink);
            let node_args = find_output_var(&name.key)
                .is_some_and(|spec| spec.argument == OutputArgKind::Nodes);
            args.iter().for_each(|arg| match &arg.kind {
                ExprKind::Number(text) if node_args && is_ground(text) => ground(arg.range, sink),
                ExprKind::Ident(ident) if node_args && is_ground(&ident.text) => {
                    ground(ident.range, sink)
                }
                _ => walk_expr(arg, sink),
            });
        }
        ExprKind::Error => {}
    }
}

/// Returns the token kind and extra modifiers for a symbol kind.
///
/// # Arguments
/// * `kind` - symbol or reference kind
fn symbol_token(kind: SymbolKind) -> (TokenKind, u32) {
    match kind {
        SymbolKind::Param => (TokenKind::Parameter, 0),
        SymbolKind::Function => (TokenKind::Function, 0),
        SymbolKind::Subckt => (TokenKind::Class, 0),
        SymbolKind::Model => (TokenKind::Type, 0),
        SymbolKind::Element => (TokenKind::EnumMember, 0),
        SymbolKind::Node => (TokenKind::Variable, 0),
        SymbolKind::LibSection => (TokenKind::Namespace, 0),
        SymbolKind::Measure => (TokenKind::Variable, READONLY),
        SymbolKind::DataBlock => (TokenKind::Variable, 0),
    }
}

/// Returns true if the symbol is a `.global` node.
///
/// # Arguments
/// * `document` - analysed document
/// * `id` - symbol id
fn is_global_node(document: &Document, id: SymbolId) -> bool {
    document
        .analysis
        .symbols
        .get(id)
        .is_some_and(Symbol::is_global_node)
}

/// Adds tokens from definitions and resolved references.
///
/// # Arguments
/// * `document` - analysed document
/// * `sink` - token collector
fn collect_resolution(document: &Document, sink: &mut Sink) {
    let analysis = &document.analysis;
    let skipped = |statement: Option<usize>| {
        statement.is_some_and(|index| analysis.structure.is_skipped(index))
    };
    analysis
        .references
        .iter()
        .filter(|reference| !matches!(reference.context, RefContext::InstanceParam { .. }))
        .filter(|reference| !skipped(Some(reference.statement)))
        .for_each(|reference| {
            let (kind, base) = symbol_token(reference.kind);
            let global = reference
                .target
                .is_some_and(|id| is_global_node(document, id));
            let modifiers = base | if global { READONLY } else { 0 };
            sink.push(reference.name.range, kind, modifiers, SYMBOL);
        });
    analysis
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, symbol)| symbol.is_local() && !skipped(symbol.statement))
        .for_each(|(id, symbol)| {
            let (kind, base) = symbol_token(symbol.kind);
            let global = if is_global_node(document, id) {
                READONLY
            } else {
                0
            };
            sink.push(symbol.name.range, kind, base | global | DEFINING, SYMBOL);
        });
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ls_types::Uri;

    use super::*;
    use crate::config::Settings;
    use crate::state::line_index::PositionEncoding;

    /// Decoded token: (line, column, length, type name, modifier bits).
    type Decoded = (u32, u32, u32, String, u32);

    fn decode(workspace: &Workspace, uri: &Uri) -> Vec<Decoded> {
        let Some(document) = workspace.document(uri) else {
            return Vec::new();
        };
        let Some(SemanticTokensResult::Tokens(tokens)) = semantic_tokens(workspace, document)
        else {
            return Vec::new();
        };
        let types = legend().token_types;
        let mut line = 0;
        let mut column = 0;
        tokens
            .data
            .iter()
            .map(|token| {
                line += token.delta_line;
                column = if token.delta_line == 0 {
                    column + token.delta_start
                } else {
                    token.delta_start
                };
                let name = types[token.token_type as usize].as_str().to_string();
                (
                    line,
                    column,
                    token.length,
                    name,
                    token.token_modifiers_bitset,
                )
            })
            .collect()
    }

    fn tokens_of(text: &str) -> Vec<Decoded> {
        let mut workspace = Workspace::new(None, PositionEncoding::Utf16, Settings::default());
        let Ok(uri) = Uri::from_str("file:///tmp/t.sp") else {
            return Vec::new();
        };
        workspace.update(uri.clone(), 1, text.to_string());
        decode(&workspace, &uri)
    }

    fn has(tokens: &[Decoded], line: u32, column: u32, length: u32, name: &str) -> bool {
        tokens
            .iter()
            .any(|t| t.0 == line && t.1 == column && t.2 == length && t.3 == name)
    }

    #[test]
    fn legend_uses_standard_types() {
        let legend = legend();
        assert!(legend.token_types.contains(&SemanticTokenType::KEYWORD));
        assert_eq!(legend.token_modifiers.len(), 5);
    }

    #[test]
    fn small_netlist_decodes_to_expected_tokens() {
        let text = "My title\n.param w=1u\nR1 a 0 'w'\n.tran 1n 10n uic\n";
        let tokens = tokens_of(text);
        assert!(has(&tokens, 0, 0, 8, "comment"));
        assert!(has(&tokens, 1, 0, 6, "keyword"));
        assert!(has(&tokens, 1, 7, 1, "parameter"));
        assert!(has(&tokens, 1, 9, 2, "number"));
        assert!(has(&tokens, 2, 0, 2, "enumMember"));
        assert!(has(&tokens, 2, 3, 1, "variable"));
        assert!(
            tokens
                .iter()
                .any(|t| t.0 == 2 && t.1 == 5 && t.3 == "variable" && t.4 & DEFAULT_LIBRARY != 0)
        );
        assert!(has(&tokens, 2, 8, 1, "parameter"));
        assert!(has(&tokens, 3, 13, 3, "keyword"));
    }

    #[test]
    fn definitions_carry_declaration_modifiers() {
        let tokens = tokens_of("t\n.param w=1\n");
        assert!(
            tokens
                .iter()
                .any(|t| t.0 == 1 && t.1 == 7 && t.4 & DEFINING == DEFINING)
        );
    }

    #[test]
    fn hidden_continuation_comments_are_comments() {
        let tokens = tokens_of("t\nR1 a b\n* hidden\n+ 1k\n");
        assert!(has(&tokens, 2, 0, 8, "comment"));
    }

    #[test]
    fn builtins_and_keys_are_classified() {
        let tokens = tokens_of("t\nVin in 0 PULSE(0 1 0 1n 1n 5n 10n)\nM1 d g s b nch W=1u\n");
        assert!(
            tokens
                .iter()
                .any(|t| t.0 == 1 && t.1 == 9 && t.3 == "function" && t.4 & DEFAULT_LIBRARY != 0)
        );
        assert!(has(&tokens, 2, 15, 1, "property"));
    }

    #[test]
    fn tokens_are_sorted_and_use_utf16_lengths() {
        let tokens = tokens_of("t\n* \u{1F600} x\n.param w=1\n");
        assert!(has(&tokens, 1, 0, 6, "comment"));
        assert!(has(&tokens, 2, 0, 6, "keyword"));
    }
}
