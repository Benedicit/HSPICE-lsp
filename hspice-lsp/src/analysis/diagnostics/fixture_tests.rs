//! Fixture tests: annotated and clean netlists opened through a workspace.

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use ls_types::Uri;

use super::{Diagnostic, Severity};
use crate::config::Settings;
use crate::state::line_index::PositionEncoding;
use crate::state::workspace::Workspace;

/// Returns the absolute path of a fixture file.
///
/// # Arguments
/// * `relative` - path below `tests/fixtures`
fn fixture_path(relative: &str) -> PathBuf {
    crate::test_support::fixtures().join(relative)
}

/// Opens a fixture in a fresh workspace and returns its text and diagnostics.
///
/// # Arguments
/// * `relative` - path below `tests/fixtures`
fn open_fixture(relative: &str) -> (String, Vec<Diagnostic>) {
    let path = fixture_path(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut workspace = Workspace::new(
        Some(fixture_path("")),
        PositionEncoding::Utf16,
        Settings::default(),
    );
    let Some(uri) = Uri::from_file_path(&path) else {
        return (text, Vec::new());
    };
    workspace.update(uri.clone(), 1, text.clone());
    let diagnostics = workspace
        .document(&uri)
        .map(|document| document.diagnostics.clone())
        .unwrap_or_default();
    (text, diagnostics)
}

/// Returns the zero-based line of a byte offset.
///
/// # Arguments
/// * `text` - document text
/// * `offset` - byte offset
fn line_of(text: &str, offset: usize) -> usize {
    text.get(..offset).unwrap_or_default().matches('\n').count()
}

/// Parses the codes after `marker` on a line, separated by commas.
///
/// # Arguments
/// * `line` - source line
/// * `marker` - annotation marker such as `$ expect:`
fn codes_after(line: &str, marker: &str) -> Option<Vec<String>> {
    let (before, rest) = line.split_once(marker)?;
    if marker.starts_with('$') && before.trim_start().starts_with('*') {
        return None;
    }
    let is_code = |code: &&str| {
        !code.is_empty()
            && code
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '-')
    };
    Some(
        rest.split(',')
            .map(str::trim)
            .filter(is_code)
            .map(str::to_string)
            .collect(),
    )
}

/// Returns the `$ expect:` annotations as (line, code) pairs.
///
/// # Arguments
/// * `text` - fixture text
fn line_expectations(text: &str) -> BTreeSet<(usize, String)> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| codes_after(line, "$ expect:").map(|codes| (index, codes)))
        .flat_map(|(index, codes)| codes.into_iter().map(move |code| (index, code)))
        .collect()
}

/// Returns the codes of all `expect-anywhere:` annotations.
///
/// # Arguments
/// * `text` - fixture text
fn anywhere_expectations(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|line| codes_after(line, "expect-anywhere:"))
        .flatten()
        .collect()
}

/// Formats diagnostics for failure messages.
///
/// # Arguments
/// * `text` - document text
/// * `diagnostics` - diagnostics to print
fn describe(text: &str, diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "  line {} {:?} {}: {}",
                line_of(text, diagnostic.range.start) + 1,
                diagnostic.severity,
                diagnostic.code,
                diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns true for severities the fixtures are checked against.
///
/// # Arguments
/// * `diagnostic` - diagnostic to classify
fn is_checked(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity != Severity::Information
}

#[test]
fn errors_fixture_produces_exactly_the_annotated_diagnostics() {
    let (text, diagnostics) = open_fixture("errors.sp");
    let expected = line_expectations(&text);
    let anywhere = anywhere_expectations(&text);
    let annotated_lines: HashSet<usize> = expected.iter().map(|(line, _)| *line).collect();
    let actual: BTreeSet<(usize, String)> = diagnostics
        .iter()
        .filter(|diagnostic| is_checked(diagnostic))
        .map(|diagnostic| {
            (
                line_of(&text, diagnostic.range.start),
                diagnostic.code.to_string(),
            )
        })
        .collect();
    let unexpected: Vec<&(usize, String)> = actual
        .iter()
        .filter(|(line, code)| {
            !expected.contains(&(*line, code.clone()))
                && !(anywhere.contains(code) && !annotated_lines.contains(line))
        })
        .collect();
    let missing: Vec<&(usize, String)> = expected
        .iter()
        .filter(|entry| !actual.contains(*entry))
        .collect();
    let codes: HashSet<String> = actual.iter().map(|(_, code)| code.clone()).collect();
    let missing_anywhere: Vec<&String> = anywhere
        .iter()
        .filter(|code| !codes.contains(*code))
        .collect();
    let report = describe(&text, &diagnostics);
    assert!(
        unexpected.is_empty(),
        "unexpected (line0, code): {unexpected:?}\nall:\n{report}"
    );
    assert!(
        missing.is_empty(),
        "missing (line0, code): {missing:?}\nall:\n{report}"
    );
    assert!(
        missing_anywhere.is_empty(),
        "missing anywhere: {missing_anywhere:?}\nall:\n{report}"
    );
}

#[test]
fn missing_end_fixture_reports_the_annotated_codes() {
    let (text, diagnostics) = open_fixture("missing_end.sp");
    let codes: HashSet<String> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect();
    let missing: Vec<String> = anywhere_expectations(&text)
        .into_iter()
        .filter(|code| !codes.contains(code))
        .collect();
    assert!(
        missing.is_empty(),
        "missing: {missing:?}\nall:\n{}",
        describe(&text, &diagnostics)
    );
}

#[test]
fn clean_fixtures_have_no_errors_or_warnings() {
    let files = [
        "inverter_tb.sp",
        "ring_osc.sp",
        "cells/inv.inc",
        "models/tech.lib",
        "models/ptm45.pm",
        "encrypted.inc",
    ];
    let failures: Vec<String> = files
        .iter()
        .filter_map(|file| {
            let (text, diagnostics) = open_fixture(file);
            let serious: Vec<Diagnostic> = diagnostics
                .into_iter()
                .filter(|diagnostic| diagnostic.severity <= Severity::Warning)
                .collect();
            (!serious.is_empty()).then(|| format!("{file}:\n{}", describe(&text, &serious)))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn clean_fixture_hints_are_justified() {
    let (text, diagnostics) = open_fixture("ring_osc.sp");
    let hints: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Hint)
        .collect();
    let unjustified: Vec<&&Diagnostic> = hints
        .iter()
        .filter(|diagnostic| diagnostic.code != "unused-param")
        .collect();
    assert!(
        unjustified.is_empty(),
        "{}",
        describe(
            &text,
            &hints.iter().map(|d| (*d).clone()).collect::<Vec<_>>()
        )
    );
}

#[test]
fn output_names_are_not_checked_where_included_files_may_define_them() {
    let codes = |text: &str| -> Vec<&'static str> {
        let mut workspace = Workspace::new(
            Some(fixture_path("")),
            PositionEncoding::Utf16,
            Settings::default(),
        );
        let Some(uri) = Uri::from_file_path(fixture_path("inline_tb.sp")) else {
            return Vec::new();
        };
        workspace.update(uri.clone(), 1, text.to_string());
        workspace
            .document(&uri)
            .map(|document| document.diagnostics.iter().map(|d| d.code).collect())
            .unwrap_or_default()
    };
    let base = "tb\nV1 a 0 1\nR1 a 0 1k\n";
    assert!(codes(&format!("{base}.print v(nosuch)\n")).contains(&"undefined-node"));
    let unresolved = codes(&format!("{base}.include 'missing.sp'\n.print v(nosuch)\n"));
    assert!(!unresolved.contains(&"undefined-node"));
    let resolved = codes(&format!(
        "{base}.include 'cells/inv.inc'\n.print v(vdd) v(nosuch)\n"
    ));
    assert_eq!(
        resolved
            .iter()
            .filter(|code| **code == "undefined-node")
            .count(),
        1
    );
}
