//! `.MOSRA` combination rules: sweeps, `.DC`/`.AC`, a missing `.TRAN` and MOSRA models in `.ALTER` sections.

use super::common::live_commands;
use super::{Diagnostic, Severity};
use crate::analysis::Analysis;
use crate::state::document::SyntaxInfo;
use crate::syntax::ast::{Arg, AssignKey, Command, CommandKind, Ident, Value};
use crate::syntax::number::parse_number;

/// Reports constructs that a `.MOSRA` run does not support.
///
/// # Arguments
/// * `syntax` - parse results
/// * `analysis` - semantic analysis
pub fn check(syntax: &SyntaxInfo, analysis: &Analysis) -> Vec<Diagnostic> {
    let commands: Vec<(usize, &Command)> = live_commands(syntax, analysis)
        .map(|(index, command, _)| (index, command))
        .collect();
    let stress_runs: Vec<&Command> = commands
        .iter()
        .map(|(_, command)| *command)
        .filter(|command| is_stress_run(command))
        .collect();
    let has_tran = commands
        .iter()
        .any(|(_, command)| command.keyword.key == ".tran");
    let analyses = commands
        .iter()
        .filter(|_| !stress_runs.is_empty())
        .flat_map(|(_, command)| sweep(command).into_iter().chain(dc_ac(command)));
    let missing_tran = stress_runs
        .iter()
        .filter(|_| syntax.is_top_level && !has_tran)
        .map(|command| {
            Diagnostic::new(
                command.keyword.range,
                Severity::Warning,
                "mosra-without-tran",
                "`.MOSRA` computes the device stress during a `.TRAN` analysis, but the netlist has none",
            )
        });
    analyses
        .chain(missing_tran)
        .chain(models_in_alter(analysis, &commands))
        .collect()
}

/// Returns true for a `.mosra` command that simulates the stress phase (`SIMMODE` absent or a literal other than 1).
///
/// # Arguments
/// * `command` - dot command
fn is_stress_run(command: &Command) -> bool {
    let CommandKind::Generic { args } = &command.kind else {
        return false;
    };
    command.keyword.key == ".mosra"
        && args
            .iter()
            .find_map(|arg| match arg {
                Arg::Assign(assignment)
                    if matches!(&assignment.key, AssignKey::Name(key) if key.key == "simmode") =>
                {
                    Some(&assignment.value)
                }
                _ => None,
            })
            .is_none_or(|value| match value {
                Some(Value::Number(number)) => parse_number(&number.text)
                    .is_some_and(|parsed| (parsed.value - 1.0).abs() > f64::EPSILON),
                _ => false,
            })
}

/// Returns the `SWEEP` keyword or `MONTE=` key of an argument.
///
/// # Arguments
/// * `arg` - analysis argument
fn sweep_marker(arg: &Arg) -> Option<&Ident> {
    match arg {
        Arg::Value(Value::Word(word)) if word.key == "sweep" => Some(word),
        Arg::Assign(assignment) => match &assignment.key {
            AssignKey::Name(key) if key.key == "monte" => Some(key),
            _ => None,
        },
        _ => None,
    }
}

/// Reports the first sweep of a `.tran`, `.dc` or `.ac` command, which aborts a `.MOSRA` run.
///
/// # Arguments
/// * `command` - dot command
fn sweep(command: &Command) -> Option<Diagnostic> {
    let CommandKind::Generic { args } = &command.kind else {
        return None;
    };
    matches!(command.keyword.key.as_str(), ".tran" | ".dc" | ".ac")
        .then(|| args.iter().find_map(sweep_marker))
        .flatten()
        .map(|marker| {
            Diagnostic::new(
                marker.range,
                Severity::Error,
                "mosra-sweep",
                format!(
                    "HSPICE aborts when `{}` is combined with `.MOSRA`; vary the value in `.ALTER` blocks instead (sweeps are only supported with `SIMMODE=1`)",
                    marker.text.to_ascii_uppercase()
                ),
            )
        })
}

/// Reports a `.dc` or `.ac` analysis, which `.MOSRA` supports only in post-stress simulations.
///
/// # Arguments
/// * `command` - dot command
fn dc_ac(command: &Command) -> Option<Diagnostic> {
    matches!(command.keyword.key.as_str(), ".dc" | ".ac").then(|| {
        Diagnostic::new(
            command.keyword.range,
            Severity::Warning,
            "mosra-dc-ac",
            format!(
                "`.MOSRA` supports `{}` only in post-stress simulations (`SIMMODE=1`)",
                command.keyword.text.to_ascii_uppercase()
            ),
        )
    })
}

/// Reports `.model … mosra` cards and `.appendmodel` commands inside `.alter` sections.
///
/// # Arguments
/// * `analysis` - analysis with the `.alter` section of each statement
/// * `commands` - live commands with their statement indices
fn models_in_alter(analysis: &Analysis, commands: &[(usize, &Command)]) -> Vec<Diagnostic> {
    commands
        .iter()
        .filter(|(index, _)| {
            analysis
                .structure
                .statement_alter
                .get(*index)
                .is_some_and(|alter| *alter > 0)
        })
        .filter_map(|(_, command)| match &command.kind {
            CommandKind::Model {
                model_type: Some(model_type),
                ..
            } if model_type.key == "mosra" => Some(model_type),
            CommandKind::Generic { .. } if command.keyword.key == ".appendmodel" => {
                Some(&command.keyword)
            }
            _ => None,
        })
        .map(|name| {
            Diagnostic::new(
                name.range,
                Severity::Error,
                "mosra-model-in-alter",
                "The MOSRA model cannot be changed in `.ALTER` sections",
            )
        })
        .collect()
}
