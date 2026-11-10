//! Function-call rules: argument counts of built-in and user-defined functions.

use super::common::live_statements;
use super::walk::all_expressions;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::analysis::symbols::{ScopeId, SymbolDetail, SymbolKind};
use crate::lang::functions::{find_function, is_optimization_function};
use crate::lang::output_vars::find_output_var;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{Expr, ExprKind};

/// Reports calls whose argument count is outside what the function accepts.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_statements(syntax, analysis)
        .flat_map(|(index, statement)| {
            let scope = analysis.structure.scope_of(index);
            all_expressions(statement)
                .into_iter()
                .filter_map(move |expr| arity_problem(analysis, scope, expr))
        })
        .collect()
}

/// Builds the arity diagnostic of a call expression, if its argument count is wrong.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `scope` - scope of the expression
/// * `expr` - any expression (non-calls are ignored)
fn arity_problem(analysis: &Analysis, scope: ScopeId, expr: &Expr) -> Option<Diagnostic> {
    let ExprKind::Call { name, args } = &expr.kind else {
        return None;
    };
    if find_output_var(&name.key).is_some() || is_optimization_function(&name.key) {
        return None;
    }
    let got = args.len();
    let (min, max) = expected_arity(analysis, scope, &name.key)?;
    (got < min || got > max).then(|| {
        let expected = if min == max {
            min.to_string()
        } else {
            format!("{min} to {max}")
        };
        let plural = if max == 1 { "argument" } else { "arguments" };
        Diagnostic::new(
            name.range,
            Severity::Error,
            "function-arity",
            format!("`{}` takes {expected} {plural}, got {got}", name.text),
        )
    })
}

/// Returns the accepted argument count range of a built-in or user-defined function.
///
/// # Arguments
/// * `analysis` - semantic analysis
/// * `scope` - scope of the call
/// * `key` - lowercase function name
fn expected_arity(analysis: &Analysis, scope: ScopeId, key: &str) -> Option<(usize, usize)> {
    if let Some(function) = find_function(key) {
        return Some((function.min_args, function.max_args));
    }
    let id = analysis.lookup(SymbolKind::Function, scope, key)?;
    match &analysis.symbols.get(id)?.detail {
        SymbolDetail::Function { arguments, .. } => Some((arguments.len(), arguments.len())),
        _ => None,
    }
}
