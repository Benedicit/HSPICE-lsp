//! Folding ranges for blocks, `.alter` sections, comment runs (opt-in) and `+` continued statements (opt-in).

use std::collections::BTreeMap;

use ls_types::{FoldingRange, FoldingRangeKind};

use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::{CommandKind, StatementKind};

/// A fold as `(start_line, end_line, kind)`.
type Fold = (u32, u32, Option<FoldingRangeKind>);

/// Computes folding ranges of a document.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document to fold
pub fn folding_ranges(workspace: &Workspace, document: &Document) -> Option<Vec<FoldingRange>> {
    let comments = workspace
        .settings
        .folding
        .comments
        .then(|| comment_folds(document))
        .into_iter()
        .flatten();
    let continuations = workspace
        .settings
        .folding
        .continuations
        .then(|| statement_folds(document))
        .into_iter()
        .flatten();
    let folds = block_folds(document)
        .chain(alter_folds(document))
        .chain(comments)
        .chain(continuations);
    let by_start: BTreeMap<u32, Fold> = folds.fold(BTreeMap::new(), |mut map, fold| {
        let keep_existing = map
            .get(&fold.0)
            .is_some_and(|existing| existing.1 >= fold.1);
        if !keep_existing {
            map.insert(fold.0, fold);
        }
        map
    });
    Some(
        by_start
            .into_values()
            .map(|(start_line, end_line, kind)| FoldingRange {
                start_line,
                start_character: None,
                end_line,
                end_character: None,
                kind,
                collapsed_text: None,
            })
            .collect(),
    )
}

/// Returns the line of a byte offset.
///
/// # Arguments
/// * `document` - document the offset belongs to
/// * `offset` - byte offset
fn line_of(document: &Document, offset: usize) -> u32 {
    document.line_index.line_of(offset) as u32
}

/// Folds of block constructs; the closing statement stays visible.
///
/// # Arguments
/// * `document` - document to fold
fn block_folds(document: &Document) -> impl Iterator<Item = Fold> + '_ {
    let statements = &document.syntax.netlist.statements;
    document
        .analysis
        .structure
        .blocks
        .iter()
        .filter_map(|block| {
            let start = line_of(document, statements.get(block.open)?.range.start);
            let end = match block.close.and_then(|close| statements.get(close)) {
                Some(closer) => line_of(document, closer.range.start).checked_sub(1)?,
                None => line_of(document, block.range.end),
            };
            (end > start).then_some((start, end, Some(FoldingRangeKind::Region)))
        })
}

/// Returns true for `.alter` statements.
///
/// # Arguments
/// * `kind` - statement kind
fn is_alter(kind: &StatementKind) -> bool {
    matches!(kind, StatementKind::Command(command) if matches!(command.kind, CommandKind::Alter { .. }))
}

/// Returns true for statements that end an `.alter` section (`.alter`, `.end`).
///
/// # Arguments
/// * `kind` - statement kind
fn ends_section(kind: &StatementKind) -> bool {
    is_alter(kind)
        || matches!(kind, StatementKind::Command(command) if matches!(command.kind, CommandKind::End))
}

/// Folds of `.alter` sections up to the next `.alter` or `.end`.
///
/// # Arguments
/// * `document` - document to fold
fn alter_folds(document: &Document) -> impl Iterator<Item = Fold> + '_ {
    let statements = &document.syntax.netlist.statements;
    statements
        .iter()
        .enumerate()
        .filter(|(_, statement)| is_alter(&statement.kind))
        .filter_map(move |(index, statement)| {
            let start = line_of(document, statement.range.start);
            let end = match statements[index + 1..]
                .iter()
                .find(|next| ends_section(&next.kind))
            {
                Some(next) => line_of(document, next.range.start).checked_sub(1)?,
                None => line_of(document, statements.last()?.range.end),
            };
            (end > start).then_some((start, end, Some(FoldingRangeKind::Region)))
        })
}

/// Folds of runs of at least two consecutive comment lines (including the leading header).
///
/// # Arguments
/// * `document` - document to fold
fn comment_folds(document: &Document) -> impl Iterator<Item = Fold> + '_ {
    let lines: Vec<u32> = document
        .syntax
        .netlist
        .statements
        .iter()
        .filter(|statement| matches!(statement.kind, StatementKind::Comment))
        .map(|statement| line_of(document, statement.range.start))
        .collect();
    let runs = lines
        .iter()
        .fold(Vec::<(u32, u32)>::new(), |mut runs, line| {
            match runs.last_mut() {
                Some(run) if run.1 + 1 == *line => run.1 = *line,
                _ => runs.push((*line, *line)),
            }
            runs
        });
    runs.into_iter()
        .filter(|(start, end)| end > start)
        .map(|(start, end)| (start, end, Some(FoldingRangeKind::Comment)))
}

/// Folds of statements spanning several lines through `+` continuations.
///
/// # Arguments
/// * `document` - document to fold
fn statement_folds(document: &Document) -> impl Iterator<Item = Fold> + '_ {
    document
        .syntax
        .netlist
        .statements
        .iter()
        .filter_map(|statement| {
            let start = line_of(document, statement.range.start);
            let end = line_of(document, statement.range.end);
            (end > start).then_some((start, end, Some(FoldingRangeKind::Region)))
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::str::FromStr;

    use ls_types::Uri;

    use super::*;
    use crate::config::Settings;
    use crate::state::line_index::PositionEncoding;

    fn folds_of(text: &str) -> Vec<(u32, u32, Option<FoldingRangeKind>)> {
        folds_with(text, Settings::default())
    }

    fn folds_with(text: &str, settings: Settings) -> Vec<(u32, u32, Option<FoldingRangeKind>)> {
        let mut workspace = Workspace::new(None, PositionEncoding::Utf16, settings);
        let Ok(uri) = Uri::from_str("file:///tmp/t.sp") else {
            return Vec::new();
        };
        workspace.update(uri.clone(), 1, text.to_string());
        let Some(document) = workspace.document(&uri) else {
            return Vec::new();
        };
        folding_ranges(&workspace, document)
            .unwrap_or_default()
            .into_iter()
            .map(|fold| (fold.start_line, fold.end_line, fold.kind))
            .collect()
    }

    #[test]
    fn subckt_block_leaves_closer_visible() {
        let folds = folds_of("t\n.subckt a x\nR1 x 0 1\nR2 x 0 2\n.ends\n.end\n");
        assert!(folds.contains(&(1, 3, Some(FoldingRangeKind::Region))));
    }

    #[test]
    fn comment_runs_and_header_fold_when_enabled() {
        let mut settings = Settings::default();
        settings.folding.comments = true;
        let folds = folds_with(
            "t\n* one\n* two\n* three\nR1 a b 1\n* lonely\nR2 a b 1\n",
            settings,
        );
        assert!(folds.contains(&(1, 3, Some(FoldingRangeKind::Comment))));
        assert!(!folds.iter().any(|fold| fold.0 == 5));
    }

    #[test]
    fn comment_runs_do_not_fold_by_default() {
        let folds = folds_of("t\n* one\n* two\n* three\nR1 a b 1\n");
        assert!(
            !folds
                .iter()
                .any(|fold| fold.2 == Some(FoldingRangeKind::Comment))
        );
    }

    #[test]
    fn continued_statements_fold_when_enabled() {
        let mut settings = Settings::default();
        settings.folding.continuations = true;
        let folds = folds_with(
            "t\n.model nch nmos\n+ vth0=0.4\n+ u0=300\nR1 a b 1\n",
            settings,
        );
        assert!(folds.contains(&(1, 3, Some(FoldingRangeKind::Region))));
    }

    #[test]
    fn continued_statements_do_not_fold_by_default() {
        let folds = folds_of("t\n.model nch nmos\n+ vth0=0.4\n+ u0=300\nR1 a b 1\n");
        assert!(folds.is_empty());
    }

    #[test]
    fn alter_sections_fold_to_the_next_alter() {
        let folds = folds_of(
            "t\nR1 a b 1\n.alter first\nR1 a b 2\nR2 a b 2\n.alter second\nR1 a b 3\n.end\n",
        );
        assert!(folds.contains(&(2, 4, Some(FoldingRangeKind::Region))));
        assert!(folds.contains(&(5, 6, Some(FoldingRangeKind::Region))));
    }

    #[test]
    fn fixture_include_folds_its_subckt() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut workspace = Workspace::new(
            Some(root.clone()),
            PositionEncoding::Utf16,
            Settings::default(),
        );
        let path = root.join("cells/inv.inc");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let Some(uri) = Uri::from_file_path(&path) else {
            panic!("uri");
        };
        workspace.update(uri.clone(), 1, text);
        let Some(document) = workspace.document(&uri) else {
            panic!("document");
        };
        let folds = folding_ranges(&workspace, document).unwrap_or_default();
        assert!(
            folds
                .iter()
                .any(|fold| fold.start_line == 1 && fold.end_line == 3)
        );
    }
}
