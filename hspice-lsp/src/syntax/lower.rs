//! Lowering of the tree-sitter CST into the typed AST in `ast.rs`.

use tree_sitter::{Node, Tree};

use super::ast::{
    Arg, AssignKey, Assignment, BinaryOp, Call, Command, CommandKind, Element, Expr, ExprKind,
    Ident, Netlist, NumberLit, ParamDef, PathArg, Statement, StatementKind, TextRange, UnaryOp,
    Value,
};
use super::node_kinds as kinds;

/// Lowers a parsed tree into a flat list of statements.
///
/// # Arguments
/// * `tree` - tree produced by `parse::parse_tree`
/// * `text` - text the tree was parsed from
pub fn lower(tree: &Tree, text: &str) -> Netlist {
    let root = tree.root_node();
    let mut cursor = root.walk();
    let statements = root
        .named_children(&mut cursor)
        .filter(|node| !node.is_extra() || node.is_error())
        .map(|node| lower_statement(node, text))
        .collect();
    Netlist { statements }
}

/// Returns the byte range of a node.
///
/// # Arguments
/// * `node` - any CST node
fn range_of(node: Node) -> TextRange {
    TextRange::new(node.start_byte(), node.end_byte())
}

/// Returns the source text of a node (empty if the range is invalid).
///
/// # Arguments
/// * `node` - any CST node
/// * `text` - source text
fn text_of<'a>(node: Node, text: &'a str) -> &'a str {
    text.get(node.start_byte()..node.end_byte())
        .unwrap_or_default()
}

/// Builds an identifier from a node's text.
///
/// # Arguments
/// * `node` - name-like CST node
/// * `text` - source text
fn ident_of(node: Node, text: &str) -> Ident {
    Ident::new(text_of(node, text), range_of(node))
}

/// Builds an identifier from a field child, if present.
///
/// # Arguments
/// * `node` - parent node
/// * `field` - field name
/// * `text` - source text
fn field_ident(node: Node, field: &str, text: &str) -> Option<Ident> {
    node.child_by_field_name(field)
        .map(|child| name_of(child, text))
}

/// Builds an identifier for a name that may be quoted (strings lose their quotes).
///
/// # Arguments
/// * `node` - word, number or string node
/// * `text` - source text
fn name_of(node: Node, text: &str) -> Ident {
    if node.kind() == kinds::STRING {
        let path = path_of(node, text);
        return Ident::new(&path.text, path.range);
    }
    ident_of(node, text)
}

/// Converts a `string` or `path_word` node into a path argument without quotes.
///
/// # Arguments
/// * `node` - path node
/// * `text` - source text
fn path_of(node: Node, text: &str) -> PathArg {
    let raw = text_of(node, text);
    let quoted = node.kind() == kinds::STRING && raw.len() >= 2;
    let (inner, start) = if quoted {
        (&raw[1..raw.len() - 1], node.start_byte() + 1)
    } else {
        (raw, node.start_byte())
    };
    PathArg {
        text: inner.to_string(),
        range: TextRange::new(start, start + inner.len()),
    }
}

/// Returns the children of a node that are not attached to any field and are not extras.
///
/// # Arguments
/// * `node` - parent node
fn unfielded_children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    let mut children = Vec::new();
    if !cursor.goto_first_child() {
        return children;
    }
    loop {
        let child = cursor.node();
        if cursor.field_name().is_none() && !child.is_extra() {
            children.push(child);
        }
        if !cursor.goto_next_sibling() {
            return children;
        }
    }
}

/// Lowers one top-level statement node.
///
/// # Arguments
/// * `node` - statement node
/// * `text` - source text
fn lower_statement(node: Node, text: &str) -> Statement {
    let kind = match node.kind() {
        kinds::COMMENT => StatementKind::Comment,
        kinds::ELEMENT => lower_element(node, text),
        _ => lower_command(node, text)
            .map(StatementKind::Command)
            .unwrap_or(StatementKind::Error),
    };
    Statement {
        kind,
        range: range_of(node),
    }
}

/// Lowers an element line.
///
/// # Arguments
/// * `node` - `element` node
/// * `text` - source text
fn lower_element(node: Node, text: &str) -> StatementKind {
    let Some(name) = field_ident(node, kinds::FIELD_NAME, text) else {
        return StatementKind::Error;
    };
    let args = lower_args(unfielded_children(node), text);
    StatementKind::Element(Element { name, args })
}

/// Lowers a dot-command node; returns `None` for ERROR nodes and unknown kinds.
///
/// # Arguments
/// * `node` - command node
/// * `text` - source text
fn lower_command(node: Node, text: &str) -> Option<Command> {
    let keyword = field_ident(node, kinds::FIELD_NAME, text)?;
    let path = || {
        node.child_by_field_name(kinds::FIELD_PATH)
            .map(|child| path_of(child, text))
    };
    let args = || lower_args(unfielded_children(node), text);
    let condition = || {
        node.child_by_field_name(kinds::FIELD_CONDITION)
            .map(|child| lower_expr(child, text))
    };
    let free_text = || {
        node.child_by_field_name(kinds::FIELD_TEXT)
            .map(|child| text_of(child, text).trim().to_string())
    };
    let kind = match node.kind() {
        kinds::INCLUDE_COMMAND => CommandKind::Include { path: path() },
        kinds::LIB_COMMAND => CommandKind::Lib {
            path: path(),
            section: field_ident(node, kinds::FIELD_SECTION, text),
        },
        kinds::PARAM_COMMAND => CommandKind::Param {
            definitions: lower_param_definitions(node, text),
        },
        kinds::MODEL_COMMAND => CommandKind::Model {
            name: field_ident(node, kinds::FIELD_MODEL, text),
            model_type: field_ident(node, kinds::FIELD_TYPE, text),
            args: args(),
        },
        kinds::SUBCKT_COMMAND => lower_subckt(node, text),
        kinds::ENDS_COMMAND => CommandKind::Ends {
            name: field_ident(node, kinds::FIELD_SUBCKT, text),
        },
        kinds::ENDL_COMMAND => CommandKind::Endl {
            name: field_ident(node, kinds::FIELD_SECTION, text),
        },
        kinds::END_COMMAND => CommandKind::End,
        kinds::GLOBAL_COMMAND => CommandKind::Global {
            nodes: field_idents(node, kinds::FIELD_NODE, text),
        },
        kinds::OPTION_COMMAND => CommandKind::Option { args: args() },
        kinds::IF_COMMAND => CommandKind::If {
            condition: condition(),
        },
        kinds::ELSEIF_COMMAND => CommandKind::ElseIf {
            condition: condition(),
        },
        kinds::ELSE_COMMAND => CommandKind::Else,
        kinds::ENDIF_COMMAND => CommandKind::EndIf,
        kinds::TITLE_COMMAND => CommandKind::Title { text: free_text() },
        kinds::ALTER_COMMAND => CommandKind::Alter { text: free_text() },
        kinds::HDL_COMMAND => CommandKind::Hdl {
            path: path(),
            args: args(),
        },
        kinds::GENERIC_COMMAND => CommandKind::Generic { args: args() },
        _ => return None,
    };
    Some(Command { keyword, kind })
}

/// Collects identifiers of all children attached to a field.
///
/// # Arguments
/// * `node` - parent node
/// * `field` - field name
/// * `text` - source text
fn field_idents(node: Node, field: &str, text: &str) -> Vec<Ident> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor)
        .map(|child| name_of(child, text))
        .collect()
}

/// Lowers a `.subckt` header into name, ports and default parameters.
///
/// # Arguments
/// * `node` - `subckt_command` node
/// * `text` - source text
fn lower_subckt(node: Node, text: &str) -> CommandKind {
    let mut cursor = node.walk();
    let params = node
        .children_by_field_name(kinds::FIELD_PARAMETER, &mut cursor)
        .filter_map(|assignment| lower_header_param(assignment, text))
        .collect();
    CommandKind::Subckt {
        name: field_ident(node, kinds::FIELD_SUBCKT, text),
        ports: field_idents(node, kinds::FIELD_PORT, text),
        params,
    }
}

/// Converts a `.subckt` header assignment into a parameter definition.
///
/// # Arguments
/// * `node` - `assignment` node
/// * `text` - source text
fn lower_header_param(node: Node, text: &str) -> Option<ParamDef> {
    let key = node.child_by_field_name(kinds::FIELD_KEY)?;
    let value = node.child_by_field_name(kinds::FIELD_VALUE);
    Some(ParamDef {
        name: ident_of(key, text),
        arguments: Vec::new(),
        quoted: value.is_some_and(|value| value.kind() == kinds::QUOTED_EXPRESSION),
        value: value.map(|value| lower_expr(value, text)),
        range: range_of(node),
    })
}

/// Lowers all `param_definition` children of a `.param` statement.
///
/// # Arguments
/// * `node` - `param_command` node
/// * `text` - source text
fn lower_param_definitions(node: Node, text: &str) -> Vec<ParamDef> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == kinds::PARAM_DEFINITION)
        .filter_map(|definition| {
            let name = field_ident(definition, kinds::FIELD_NAME, text)?;
            let value = definition.child_by_field_name(kinds::FIELD_VALUE);
            let arguments = definition
                .child_by_field_name(kinds::FIELD_PARAMETERS)
                .map(|list| {
                    let mut list_cursor = list.walk();
                    list.named_children(&mut list_cursor)
                        .filter(|child| child.kind() == kinds::IDENTIFIER)
                        .map(|child| ident_of(child, text))
                        .collect()
                })
                .unwrap_or_default();
            Some(ParamDef {
                name,
                arguments,
                quoted: value.is_some_and(|value| value.kind() == kinds::QUOTED_EXPRESSION),
                value: value.map(|value| lower_expr(value, text)),
                range: range_of(definition),
            })
        })
        .collect()
}

/// Lowers argument nodes, merging comma lists such as `AC=1,90` into the preceding assignment.
///
/// # Arguments
/// * `nodes` - argument nodes in source order (commas included, other anonymous tokens ignored)
/// * `text` - source text
fn lower_args(nodes: Vec<Node>, text: &str) -> Vec<Arg> {
    let (args, _) = nodes.into_iter().fold(
        (Vec::<Arg>::new(), false),
        |(mut args, after_comma), node| {
            if node.kind() == "," {
                return (args, true);
            }
            let Some(arg) = lower_arg(node, text) else {
                return (args, false);
            };
            let mergeable = matches!(arg, Arg::Value(Value::Number(_) | Value::Expr(_)));
            match (after_comma && mergeable, args.last_mut(), arg) {
                (true, Some(Arg::Assign(assignment)), Arg::Value(value)) => {
                    assignment.range = assignment.range.cover(value.range());
                    assignment.extra_values.push(value);
                }
                (_, _, arg) => args.push(arg),
            }
            (args, false)
        },
    );
    args
}

/// Lowers one argument node; returns `None` for punctuation and ERROR nodes.
///
/// # Arguments
/// * `node` - argument node
/// * `text` - source text
fn lower_arg(node: Node, text: &str) -> Option<Arg> {
    match node.kind() {
        kinds::ASSIGNMENT => lower_assignment(node, text).map(Arg::Assign),
        kinds::FILE_ASSIGNMENT => {
            let key = node.child_by_field_name(kinds::FIELD_KEY)?;
            let value = node.child_by_field_name(kinds::FIELD_VALUE);
            Some(Arg::Assign(Assignment {
                key: AssignKey::Name(ident_of(key, text)),
                value: value.map(|value| Value::Str(path_of(value, text))),
                extra_values: Vec::new(),
                range: range_of(node),
            }))
        }
        kinds::PARENTHESIZED => Some(Arg::Group {
            args: lower_args(unfielded_children(node), text),
            range: range_of(node),
        }),
        _ => lower_value(node, text).map(Arg::Value),
    }
}

/// Lowers a `key=value` assignment.
///
/// # Arguments
/// * `node` - `assignment` node
/// * `text` - source text
fn lower_assignment(node: Node, text: &str) -> Option<Assignment> {
    let key_node = node.child_by_field_name(kinds::FIELD_KEY)?;
    let key = if key_node.kind() == kinds::CALL {
        AssignKey::Call(lower_call(key_node, text)?)
    } else {
        AssignKey::Name(ident_of(key_node, text))
    };
    let mut values = node
        .child_by_field_name(kinds::FIELD_VALUE)
        .map(list_items)
        .unwrap_or_default()
        .into_iter()
        .map(|item| Value::Expr(lower_expr(item, text)));
    Some(Assignment {
        key,
        value: values.next(),
        extra_values: values.collect(),
        range: range_of(node),
    })
}

/// Returns the items of a `value_list` (without its function name), or the node itself.
///
/// # Arguments
/// * `value` - assignment value node
fn list_items(value: Node) -> Vec<Node> {
    if value.kind() != kinds::VALUE_LIST {
        return vec![value];
    }
    let function = value.child_by_field_name(kinds::FIELD_FUNCTION);
    let mut cursor = value.walk();
    value
        .named_children(&mut cursor)
        .filter(|child| !child.is_extra() && Some(*child) != function)
        .collect()
}

/// Lowers a positional value node.
///
/// # Arguments
/// * `node` - number, word, quoted expression, call or path node
/// * `text` - source text
fn lower_value(node: Node, text: &str) -> Option<Value> {
    match node.kind() {
        kinds::NUMBER => Some(Value::Number(NumberLit {
            text: text_of(node, text).split_whitespace().collect(),
            range: range_of(node),
        })),
        kinds::WORD => Some(Value::Word(ident_of(node, text))),
        kinds::QUOTED_EXPRESSION => Some(Value::Expr(lower_expr(node, text))),
        kinds::CALL => lower_call(node, text).map(Value::Call),
        kinds::STRING | kinds::PATH_WORD => Some(Value::Str(path_of(node, text))),
        _ => None,
    }
}

/// Lowers a call-like argument such as `pulse(0 1 …)` or `v(out)`.
///
/// # Arguments
/// * `node` - `call` node
/// * `text` - source text
fn lower_call(node: Node, text: &str) -> Option<Call> {
    let name = field_ident(node, kinds::FIELD_FUNCTION, text)?;
    let mut cursor = node.walk();
    let arguments: Vec<Node> = node
        .children(&mut cursor)
        .filter(|child| !child.is_extra())
        .filter(|child| child.kind() != kinds::WORD || range_of(*child) != name.range)
        .filter(|child| child.is_named() || child.kind() == ",")
        .collect();
    Some(Call {
        name,
        args: lower_args(arguments, text),
        range: range_of(node),
    })
}

/// Lowers an expression node (quotes and parentheses are unwrapped).
///
/// # Arguments
/// * `node` - expression node
/// * `text` - source text
pub fn lower_expr(node: Node, text: &str) -> Expr {
    let range = range_of(node);
    let child = |field: &str| {
        node.child_by_field_name(field)
            .map(|child| Box::new(lower_expr(child, text)))
    };
    let kind = match node.kind() {
        kinds::NUMBER => ExprKind::Number(text_of(node, text).to_string()),
        kinds::IDENTIFIER => ExprKind::Ident(ident_of(node, text)),
        kinds::QUOTED_EXPRESSION | kinds::PARENTHESIZED_EXPRESSION => {
            let mut cursor = node.walk();
            let inner = node
                .named_children(&mut cursor)
                .find(|inner| !inner.is_extra());
            return inner.map_or(
                Expr {
                    kind: ExprKind::Error,
                    range,
                },
                |inner| lower_expr(inner, text),
            );
        }
        kinds::UNARY_EXPRESSION => match (
            operator_text(node, text).as_str(),
            child(kinds::FIELD_OPERAND),
        ) {
            ("-", Some(operand)) => ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            },
            ("+", Some(operand)) => ExprKind::Unary {
                op: UnaryOp::Plus,
                operand,
            },
            ("!", Some(operand)) => ExprKind::Unary {
                op: UnaryOp::Not,
                operand,
            },
            _ => ExprKind::Error,
        },
        kinds::BINARY_EXPRESSION => {
            match (
                binary_op(&operator_text(node, text)),
                child(kinds::FIELD_LEFT),
                child(kinds::FIELD_RIGHT),
            ) {
                (Some(op), Some(left), Some(right)) => ExprKind::Binary { op, left, right },
                _ => ExprKind::Error,
            }
        }
        kinds::TERNARY_EXPRESSION => match (
            child(kinds::FIELD_CONDITION),
            child(kinds::FIELD_CONSEQUENCE),
            child(kinds::FIELD_ALTERNATIVE),
        ) {
            (Some(condition), Some(then_branch), Some(else_branch)) => ExprKind::Ternary {
                condition,
                then_branch,
                else_branch,
            },
            _ => ExprKind::Error,
        },
        kinds::CALL_EXPRESSION => match field_ident(node, kinds::FIELD_FUNCTION, text) {
            Some(name) => {
                let mut cursor = node.walk();
                let args = node
                    .children_by_field_name(kinds::FIELD_ARGUMENT, &mut cursor)
                    .map(|argument| lower_expr(argument, text))
                    .collect();
                ExprKind::Call { name, args }
            }
            None => ExprKind::Error,
        },
        _ => ExprKind::Error,
    };
    Expr { kind, range }
}

/// Returns the operator token text of a unary or binary expression.
///
/// # Arguments
/// * `node` - expression node with an `operator` field
/// * `text` - source text
fn operator_text(node: Node, text: &str) -> String {
    node.child_by_field_name(kinds::FIELD_OPERATOR)
        .map(|operator| text_of(operator, text).to_string())
        .unwrap_or_default()
}

/// Maps an operator token to its binary operator.
///
/// # Arguments
/// * `operator` - operator text
fn binary_op(operator: &str) -> Option<BinaryOp> {
    let op = match operator {
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Sub,
        "*" => BinaryOp::Mul,
        "/" => BinaryOp::Div,
        "**" | "^" => BinaryOp::Pow,
        "==" => BinaryOp::Eq,
        "!=" => BinaryOp::Ne,
        "<" => BinaryOp::Lt,
        "<=" => BinaryOp::Le,
        ">" => BinaryOp::Gt,
        ">=" => BinaryOp::Ge,
        "&&" => BinaryOp::And,
        "||" => BinaryOp::Or,
        _ => return None,
    };
    Some(op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parse::parse_tree;

    fn lower_text(text: &str) -> Netlist {
        let tree = parse_tree(text).expect("parser");
        lower(&tree, text)
    }

    fn only_statement(text: &str) -> StatementKind {
        let netlist = lower_text(text);
        assert_eq!(netlist.statements.len(), 1, "{netlist:#?}");
        netlist.statements[0].kind.clone()
    }

    #[test]
    fn value_lists_become_extra_values() {
        let StatementKind::Command(command) =
            only_statement(".tran 1n 1u sweep monte=list(10 20:30)\n")
        else {
            panic!("expected command");
        };
        let CommandKind::Generic { args } = &command.kind else {
            panic!("expected generic command");
        };
        let Some(Arg::Assign(monte)) = args.last() else {
            panic!("expected assignment");
        };
        assert!(monte.value.is_some());
        assert_eq!(monte.extra_values.len(), 2);
    }

    #[test]
    fn lowers_element_with_continuation_and_assignments() {
        let StatementKind::Element(element) =
            only_statement("M1 d g s b nch W=1u\n+ L='2*lmin' AC=1,90\n")
        else {
            panic!("expected element");
        };
        assert_eq!(element.name.key, "m1");
        assert_eq!(element.args.len(), 8);
        let Arg::Assign(ac) = &element.args[7] else {
            panic!("expected assignment");
        };
        assert_eq!(ac.extra_values.len(), 1);
    }

    #[test]
    fn lowers_param_definitions_and_functions() {
        let StatementKind::Command(command) = only_statement(".param a=1 b='a*2' f(x,y)='x+y'\n")
        else {
            panic!("expected command");
        };
        let CommandKind::Param { definitions } = command.kind else {
            panic!("expected .param");
        };
        let names: Vec<&str> = definitions
            .iter()
            .map(|definition| definition.name.text.as_str())
            .collect();
        assert_eq!(names, vec!["a", "b", "f"]);
        assert!(definitions[1].quoted);
        assert_eq!(definitions[2].arguments.len(), 2);
        assert!(matches!(
            definitions[1].value.as_ref().map(|value| &value.kind),
            Some(ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            })
        ));
    }

    #[test]
    fn lowers_subckt_header() {
        let StatementKind::Command(command) = only_statement(".SUBCKT inv in out vdd wn=1u\n")
        else {
            panic!("expected command");
        };
        let CommandKind::Subckt {
            name,
            ports,
            params,
        } = command.kind
        else {
            panic!("expected .subckt");
        };
        assert_eq!(name.map(|name| name.key), Some("inv".to_string()));
        assert_eq!(ports.len(), 3);
        assert_eq!(params[0].name.key, "wn");
    }

    #[test]
    fn lowers_lib_call_and_definition() {
        let netlist = lower_text(".lib 'models/x.lib' tt\n.lib ff\n");
        let kinds: Vec<CommandKind> = netlist
            .statements
            .into_iter()
            .filter_map(|statement| match statement.kind {
                StatementKind::Command(command) => Some(command.kind),
                _ => None,
            })
            .collect();
        assert!(
            matches!(&kinds[0], CommandKind::Lib { path: Some(path), section: Some(section) }
            if path.text == "models/x.lib" && section.key == "tt")
        );
        assert!(
            matches!(&kinds[1], CommandKind::Lib { path: None, section: Some(section) } if section.key == "ff")
        );
    }

    #[test]
    fn lowers_output_variable_assignment_key() {
        let StatementKind::Command(command) = only_statement(".ic v(out)=0\n") else {
            panic!("expected command");
        };
        let CommandKind::Generic { args } = command.kind else {
            panic!("expected generic command");
        };
        assert!(
            matches!(&args[0], Arg::Assign(Assignment { key: AssignKey::Call(call), .. }) if call.name.key == "v")
        );
    }

    #[test]
    fn lowers_call_arguments() {
        let StatementKind::Element(element) = only_statement("Vin in 0 PULSE(0 1.8 0 1n)\n") else {
            panic!("expected element");
        };
        let Arg::Value(Value::Call(call)) = &element.args[2] else {
            panic!("expected call");
        };
        assert_eq!(call.name.key, "pulse");
        assert_eq!(call.args.len(), 4);
    }

    #[test]
    fn error_lines_become_error_statements() {
        let netlist = lower_text("+ 1k\nR1 a b 1k\n");
        assert!(
            netlist
                .statements
                .iter()
                .any(|statement| matches!(statement.kind, StatementKind::Element(_)))
        );
    }
}
