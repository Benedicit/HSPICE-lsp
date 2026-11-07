//! Traversal helpers that collect values, expressions and numeric literals of statements.

use crate::syntax::ast::{
    Arg, AssignKey, Call, CommandKind, Expr, ExprKind, Statement, StatementKind, TextRange, Value,
};

/// Returns the argument list of an element or of a command that carries free-form arguments.
///
/// # Arguments
/// * `statement` - statement to inspect
pub fn statement_args(statement: &Statement) -> &[Arg] {
    match &statement.kind {
        StatementKind::Element(element) => &element.args,
        StatementKind::Command(command) => match &command.kind {
            CommandKind::Generic { args }
            | CommandKind::Option { args }
            | CommandKind::Model { args, .. }
            | CommandKind::Hdl { args, .. } => args,
            _ => &[],
        },
        _ => &[],
    }
}

/// Flattens arguments into all values, descending into calls, groups and assignments.
///
/// # Arguments
/// * `args` - arguments to flatten
pub fn values_of(args: &[Arg]) -> Vec<&Value> {
    args.iter()
        .flat_map(|arg| match arg {
            Arg::Value(value) => value_with_children(value),
            Arg::Assign(assignment) => {
                let key_values = match &assignment.key {
                    AssignKey::Call(call) => values_of(&call.args),
                    AssignKey::Name(_) => Vec::new(),
                };
                key_values
                    .into_iter()
                    .chain(
                        assignment
                            .value
                            .iter()
                            .chain(&assignment.extra_values)
                            .flat_map(value_with_children),
                    )
                    .collect()
            }
            Arg::Group { args, .. } => values_of(args),
        })
        .collect()
}

/// Returns a value followed by the values nested in it (call arguments).
///
/// # Arguments
/// * `value` - value to expand
fn value_with_children(value: &Value) -> Vec<&Value> {
    let children = match value {
        Value::Call(Call { args, .. }) => values_of(args),
        _ => Vec::new(),
    };
    std::iter::once(value).chain(children).collect()
}

/// Returns the root expressions of a statement (expression values, parameter values, conditions).
///
/// # Arguments
/// * `statement` - statement to inspect
pub fn root_expressions(statement: &Statement) -> Vec<&Expr> {
    let from_args =
        values_of(statement_args(statement))
            .into_iter()
            .filter_map(|value| match value {
                Value::Expr(expr) => Some(expr),
                _ => None,
            });
    let from_command: Vec<&Expr> = match &statement.kind {
        StatementKind::Command(command) => match &command.kind {
            CommandKind::Param { definitions } => definitions
                .iter()
                .filter_map(|definition| definition.value.as_ref())
                .collect(),
            CommandKind::Subckt { params, .. } => params
                .iter()
                .filter_map(|definition| definition.value.as_ref())
                .collect(),
            CommandKind::If { condition } | CommandKind::ElseIf { condition } => {
                condition.iter().collect()
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    from_args.chain(from_command).collect()
}

/// Returns an expression followed by all its sub-expressions in pre-order.
///
/// # Arguments
/// * `expr` - root expression
pub fn flatten_expression(expr: &Expr) -> Vec<&Expr> {
    let children: Vec<&Expr> = match &expr.kind {
        ExprKind::Unary { operand, .. } => flatten_expression(operand),
        ExprKind::Binary { left, right, .. } => flatten_expression(left)
            .into_iter()
            .chain(flatten_expression(right))
            .collect(),
        ExprKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => [condition, then_branch, else_branch]
            .into_iter()
            .flat_map(|part| flatten_expression(part))
            .collect(),
        ExprKind::Call { args, .. } => args.iter().flat_map(flatten_expression).collect(),
        ExprKind::Number(_) | ExprKind::Ident(_) | ExprKind::Error => Vec::new(),
    };
    std::iter::once(expr).chain(children).collect()
}

/// Returns all expressions of a statement including nested ones.
///
/// # Arguments
/// * `statement` - statement to inspect
pub fn all_expressions(statement: &Statement) -> Vec<&Expr> {
    root_expressions(statement)
        .into_iter()
        .flat_map(flatten_expression)
        .collect()
}

/// Returns all numeric literals (text and range) of a statement, in positional values and expressions.
///
/// # Arguments
/// * `statement` - statement to inspect
pub fn numeric_literals(statement: &Statement) -> Vec<(&str, TextRange)> {
    let positional =
        values_of(statement_args(statement))
            .into_iter()
            .filter_map(|value| match value {
                Value::Number(number) => Some((number.text.as_str(), number.range)),
                _ => None,
            });
    let in_expressions =
        all_expressions(statement)
            .into_iter()
            .filter_map(|expr| match &expr.kind {
                ExprKind::Number(text) => Some((text.as_str(), expr.range)),
                _ => None,
            });
    positional.chain(in_expressions).collect()
}
