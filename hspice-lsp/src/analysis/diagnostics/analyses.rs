//! Analysis-command rules: required arguments of `.tran`, `.ac`, `.dc`, `.noise`, `.tf` and `.pz`.

use super::common::live_commands;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{Arg, AssignKey, Command, CommandKind, Value};

/// Reports analysis commands that lack required arguments.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    live_commands(syntax, analysis)
        .filter_map(|(_, command, _)| match &command.kind {
            CommandKind::Generic { args } => {
                problem(command, args).map(|message| (command.keyword.range, message))
            }
            _ => None,
        })
        .map(|(range, message)| Diagnostic::new(range, Severity::Error, "analysis-args", message))
        .collect()
}

/// Describes the missing arguments of an analysis command, if any.
///
/// # Arguments
/// * `command` - the command
/// * `args` - its arguments
fn problem(command: &Command, args: &[Arg]) -> Option<String> {
    match command.keyword.key.as_str() {
        ".tran" => tran_problem(args),
        ".ac" => ac_problem(args),
        ".dc" => dc_problem(args),
        ".noise" | ".tf" | ".pz" => source_problem(&command.keyword.key, args),
        _ => None,
    }
}

/// Returns true if an assignment with one of the keys is present.
///
/// # Arguments
/// * `args` - command arguments
/// * `keys` - lowercase keys to look for
fn has_assignment(args: &[Arg], keys: &[&str]) -> bool {
    args.iter().any(|arg| matches!(arg, Arg::Assign(assignment) if matches!(&assignment.key, AssignKey::Name(key) if keys.contains(&key.key.as_str()))))
}

/// Returns the leading positional values up to the first assignment, group or `sweep`/`uic` keyword.
///
/// # Arguments
/// * `args` - command arguments
fn leading_values(args: &[Arg]) -> Vec<&Value> {
    args.iter()
        .map_while(|arg| match arg {
            Arg::Value(Value::Word(word)) if matches!(word.key.as_str(), "sweep" | "uic") => None,
            Arg::Value(value @ (Value::Number(_) | Value::Expr(_) | Value::Word(_))) => Some(value),
            _ => None,
        })
        .collect()
}

/// Returns the lowercase key of a word value.
///
/// # Arguments
/// * `value` - positional value
fn word_key(value: &Value) -> Option<&str> {
    match value {
        Value::Word(word) => Some(word.key.as_str()),
        _ => None,
    }
}

/// `.tran` needs at least `tstep` and `tstop` unless `DATA=` is given.
///
/// # Arguments
/// * `args` - command arguments
fn tran_problem(args: &[Arg]) -> Option<String> {
    (!has_assignment(args, &["data"]) && leading_values(args).len() < 2)
        .then(|| "`.tran` needs at least `tstep` and `tstop`".to_string())
}

/// `.ac` needs `DEC|OCT|LIN|POI np fstart fstop` unless `DATA=` is given.
///
/// # Arguments
/// * `args` - command arguments
fn ac_problem(args: &[Arg]) -> Option<String> {
    if has_assignment(args, &["data"]) {
        return None;
    }
    let values = leading_values(args);
    let Some(kind) = values
        .first()
        .and_then(|value| word_key(value))
        .filter(|key| matches!(*key, "dec" | "oct" | "lin" | "poi"))
    else {
        return Some(
            "`.ac` needs `DEC`, `OCT`, `LIN` or `POI` followed by `np fstart fstop`".to_string(),
        );
    };
    let needed = if kind == "poi" { 2 } else { 3 };
    (values.len() - 1 < needed).then(|| {
        format!(
            "`.ac {}` needs `np fstart fstop`",
            kind.to_ascii_uppercase()
        )
    })
}

/// `.dc` needs `var start stop step` (or `var DEC|OCT|LIN|POI np start stop`) unless `DATA=`/`MONTE=` is given.
///
/// # Arguments
/// * `args` - command arguments
fn dc_problem(args: &[Arg]) -> Option<String> {
    if has_assignment(args, &["data", "monte", "start", "stop", "step"]) {
        return None;
    }
    let values = leading_values(args);
    let Some(Value::Word(_)) = values.first() else {
        return Some("`.dc` needs a sweep variable: `.dc var start stop step`".to_string());
    };
    let rest = &values[1..];
    let needed = match rest.first().and_then(|value| word_key(value)) {
        Some("dec" | "oct" | "lin" | "poi") => 4,
        _ => 3,
    };
    (rest.len() < needed).then(|| "`.dc` needs `var start stop step`".to_string())
}

/// `.noise`, `.tf` and `.pz` need an output variable and a source.
///
/// # Arguments
/// * `keyword` - lowercase command keyword
/// * `args` - command arguments
fn source_problem(keyword: &str, args: &[Arg]) -> Option<String> {
    let positional = args
        .iter()
        .filter(|arg| matches!(arg, Arg::Value(_)))
        .count();
    (positional < 2).then(|| format!("`{keyword}` needs an output variable and a source name"))
}
