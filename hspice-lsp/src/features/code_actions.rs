//! Quick fixes for diagnostics.

use std::collections::HashMap;

use ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    NumberOrString, TextEdit, WorkspaceEdit,
};

use super::{is_token_delimiter, to_lsp_range};
use crate::analysis::blocks::{Block, BlockKind};
use crate::analysis::statement_at;
use crate::analysis::symbols::{ROOT_SCOPE, SymbolKind, bin_base};
use crate::lang::Named;
use crate::lang::commands::COMMANDS;
use crate::lang::functions::FUNCTIONS;
use crate::lang::options::OPTIONS;
use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::{CommandKind, StatementKind, TextRange};

/// Maximum edit distance for "Did you mean" suggestions.
const MAX_DISTANCE: usize = 2;

/// Maximum number of "Did you mean" suggestions per diagnostic.
const MAX_SUGGESTIONS: usize = 3;

/// Computes quick fixes for the diagnostics in the requested range.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `params` - code action request parameters
pub fn code_actions(
    workspace: &Workspace,
    document: &Document,
    params: &CodeActionParams,
) -> Option<CodeActionResponse> {
    let actions: Vec<CodeActionOrCommand> = params
        .context
        .diagnostics
        .iter()
        .flat_map(|diagnostic| fixes_for(workspace, document, diagnostic))
        .map(CodeActionOrCommand::CodeAction)
        .collect();
    (!actions.is_empty()).then_some(actions)
}

/// A planned fix: title, edits and whether it is the preferred fix.
struct Fix {
    title: String,
    edits: Vec<TextEdit>,
    preferred: bool,
}

/// Computes all fixes for one diagnostic.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `diagnostic` - diagnostic sent by the client
fn fixes_for(
    workspace: &Workspace,
    document: &Document,
    diagnostic: &ls_types::Diagnostic,
) -> Vec<CodeAction> {
    let Some(NumberOrString::String(code)) = &diagnostic.code else {
        return Vec::new();
    };
    let range =
        document
            .line_index
            .text_range(&document.text, diagnostic.range, workspace.encoding);
    let fixes = plan_fixes(workspace, document, code, range);
    fixes
        .into_iter()
        .map(|fix| CodeAction {
            title: fix.title,
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(HashMap::from([(document.uri.clone(), fix.edits)])),
                ..Default::default()
            }),
            is_preferred: Some(fix.preferred),
            ..Default::default()
        })
        .collect()
}

/// Plans the fixes for a diagnostic code at a range.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `code` - diagnostic code
/// * `range` - diagnostic range as byte offsets
fn plan_fixes(
    workspace: &Workspace,
    document: &Document,
    code: &str,
    range: TextRange,
) -> Vec<Fix> {
    let single = |fix: Option<Fix>| fix.into_iter().collect::<Vec<Fix>>();
    match code {
        "meg-suffix" => single(meg_fix(workspace, document, range)),
        "unclosed-block" => single(close_block_fix(workspace, document, range)),
        "missing-end" => single(add_end_fix(workspace, document)),
        "ends-name-mismatch" => single(ends_name_fix(workspace, document, range)),
        "undefined-param" => define_param_fix(workspace, document, range)
            .into_iter()
            .chain(suggestions(workspace, document, code, range))
            .collect(),
        "unknown-command" | "unknown-option" | "undefined-subckt" | "undefined-model"
        | "undefined-function" => suggestions(workspace, document, code, range),
        _ => Vec::new(),
    }
}

/// Builds an edit that replaces a byte range.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document being edited
/// * `range` - range to replace
/// * `text` - replacement
fn replace_edit(
    workspace: &Workspace,
    document: &Document,
    range: TextRange,
    text: &str,
) -> TextEdit {
    TextEdit {
        range: to_lsp_range(workspace, document, range),
        new_text: text.to_string(),
    }
}

/// Builds an edit that inserts text at a byte offset.
///
/// # Arguments
/// * `workspace` - workspace (for the position encoding)
/// * `document` - document being edited
/// * `offset` - insertion offset
/// * `text` - text to insert
fn insert_edit(workspace: &Workspace, document: &Document, offset: usize, text: &str) -> TextEdit {
    replace_edit(workspace, document, TextRange::new(offset, offset), text)
}

/// Finds the byte index of a milli suffix (`m`/`M`, not `meg`/`mil`) after the number at a token start.
///
/// # Arguments
/// * `token` - delimiter-free token
fn milli_suffix_index(token: &str) -> Option<usize> {
    let bytes = token.as_bytes();
    let sign = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let digits_end = sign
        + bytes[sign..]
            .iter()
            .take_while(|b| b.is_ascii_digit() || **b == b'.')
            .count();
    let has_digits = bytes[sign..digits_end].iter().any(u8::is_ascii_digit);
    let exponent_end = exponent_end(bytes, digits_end);
    let rest = token.get(exponent_end..)?.to_ascii_lowercase();
    (has_digits && rest.starts_with('m') && !rest.starts_with("meg") && !rest.starts_with("mil"))
        .then_some(exponent_end)
}

/// Returns the index after an exponent starting at `start`, or `start` if there is none.
///
/// # Arguments
/// * `bytes` - token bytes
/// * `start` - index right after the mantissa
fn exponent_end(bytes: &[u8], start: usize) -> usize {
    let Some(b'e' | b'E' | b'd' | b'D') = bytes.get(start) else {
        return start;
    };
    let sign = usize::from(matches!(bytes.get(start + 1), Some(b'+' | b'-')));
    let digits = bytes
        .iter()
        .skip(start + 1 + sign)
        .take_while(|b| b.is_ascii_digit())
        .count();
    if digits == 0 {
        start
    } else {
        start + 1 + sign + digits
    }
}

/// Splits text into delimiter-free tokens with their byte offsets.
///
/// # Arguments
/// * `text` - text to split
fn tokens_with_offsets(text: &str) -> Vec<(usize, &str)> {
    text.split(is_token_delimiter)
        .scan(0usize, |offset, token| {
            let start = *offset;
            *offset += token.len() + 1;
            Some((start, token))
        })
        .collect()
}

/// Replaces the milli suffix `M` by `MEG`.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `range` - diagnostic range (the literal or a region containing it)
fn meg_fix(workspace: &Workspace, document: &Document, range: TextRange) -> Option<Fix> {
    let slice = document.slice(range);
    let (start, index) = tokens_with_offsets(slice)
        .into_iter()
        .find_map(|(start, token)| milli_suffix_index(token).map(|index| (start, index)))?;
    let position = range.start + start + index;
    let replacement = if slice[start + index..].starts_with('M') {
        "MEG"
    } else {
        "meg"
    };
    Some(Fix {
        title: "Replace `M` (milli) by `MEG` (mega)".to_string(),
        edits: vec![replace_edit(
            workspace,
            document,
            TextRange::new(position, position + 1),
            replacement,
        )],
        preferred: true,
    })
}

/// Returns the offset of the start of the line containing `offset`.
///
/// # Arguments
/// * `document` - document being edited
/// * `offset` - byte offset
fn line_start_of(document: &Document, offset: usize) -> usize {
    document
        .line_index
        .line_start(document.line_index.line_of(offset))
        .unwrap_or(offset)
}

/// Inserts `.param <name>=0` before the statement that uses an undefined parameter.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `range` - range of the undefined name
fn define_param_fix(workspace: &Workspace, document: &Document, range: TextRange) -> Option<Fix> {
    let (_, name) = word_in(document.slice(range), false)?;
    let netlist = &document.syntax.netlist;
    let structure = &document.analysis.structure;
    let statement = statement_at(netlist, range.start)?;
    let scope = structure.scope_of(statement);
    let anchor = match scope {
        ROOT_SCOPE => statement,
        inner => structure.scopes.get(inner)?.subckt_statement? + 1,
    };
    let anchor_start = netlist.statements.get(anchor)?.range.start;
    let offset = line_start_of(document, anchor_start);
    Some(Fix {
        title: format!("Define parameter `{name}`"),
        edits: vec![insert_edit(
            workspace,
            document,
            offset,
            &format!(".param {name}=0\n"),
        )],
        preferred: false,
    })
}

/// Returns the closing statement text for an unclosed block.
///
/// # Arguments
/// * `block` - the unclosed block
fn closer_text(block: &Block) -> String {
    let named = |keyword: &str| match &block.name {
        Some(name) => format!("{keyword} {}", name.text),
        None => keyword.to_string(),
    };
    let closer = block.kind.keywords().1;
    match block.kind {
        BlockKind::Subckt | BlockKind::LibSection => named(closer),
        _ => closer.to_string(),
    }
}

/// Inserts the matching closer after the last statement of an unclosed block.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `range` - diagnostic range
fn close_block_fix(workspace: &Workspace, document: &Document, range: TextRange) -> Option<Fix> {
    let statements = &document.syntax.netlist.statements;
    let block = document.analysis.structure.blocks.iter().find(|block| {
        block.close.is_none()
            && statements
                .get(block.open)
                .is_some_and(|open| open.range.touches(range.start))
    })?;
    let last = (block.open + 1..statements.len())
        .take_while(|index| !matches!(&statements[*index].kind, StatementKind::Command(command) if matches!(command.kind, CommandKind::End)))
        .filter(|index| !matches!(statements[*index].kind, StatementKind::Comment))
        .last()
        .unwrap_or(block.open);
    let closer = closer_text(block);
    Some(Fix {
        title: format!("Insert `{closer}`"),
        edits: vec![insert_edit(
            workspace,
            document,
            statements.get(last)?.range.end,
            &format!("\n{closer}"),
        )],
        preferred: true,
    })
}

/// Appends `.end` at the end of the file.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
fn add_end_fix(workspace: &Workspace, document: &Document) -> Option<Fix> {
    let text = if document.text.ends_with('\n') || document.text.is_empty() {
        ".end\n"
    } else {
        "\n.end\n"
    };
    Some(Fix {
        title: "Add `.end`".to_string(),
        edits: vec![insert_edit(workspace, document, document.text.len(), text)],
        preferred: true,
    })
}

/// Replaces the name after `.ends` by the name of the subcircuit it closes.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `range` - diagnostic range
fn ends_name_fix(workspace: &Workspace, document: &Document, range: TextRange) -> Option<Fix> {
    let statements = &document.syntax.netlist.statements;
    let index = statement_at(&document.syntax.netlist, range.start)?;
    let StatementKind::Command(command) = &statements.get(index)?.kind else {
        return None;
    };
    let CommandKind::Ends {
        name: Some(written),
    } = &command.kind
    else {
        return None;
    };
    let block = document
        .analysis
        .structure
        .blocks
        .iter()
        .find(|block| block.close == Some(index) && block.kind == BlockKind::Subckt)?;
    let expected = block.name.as_ref()?;
    Some(Fix {
        title: format!("Change to `.ends {}`", expected.text),
        edits: vec![replace_edit(
            workspace,
            document,
            written.range,
            &expected.text,
        )],
        preferred: true,
    })
}

/// Extracts the first name-like word of a diagnostic range.
///
/// # Arguments
/// * `slice` - text of the diagnostic range
/// * `command` - true if the word is a dot command (operators then belong to the name)
///
/// # Returns
/// The byte offset of the word inside `slice` and the word.
fn word_in(slice: &str, command: bool) -> Option<(usize, &str)> {
    let start = slice.len() - slice.trim_start().len();
    let word = slice[start..]
        .split(|c: char| is_token_delimiter(c) || (!command && "*+-/^<>!&|?:".contains(c)))
        .next()
        .filter(|word| !word.is_empty())?;
    Some((start, word))
}

/// Computes the edit distance between two strings.
///
/// # Arguments
/// * `left` - first string
/// * `right` - second string
fn edit_distance(left: &str, right: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (left.chars().collect(), right.chars().collect());
    let rows = (0..=a.len()).fold(Vec::<Vec<usize>>::new(), |mut rows, i| {
        let row: Vec<usize> = (0..=b.len()).fold(Vec::with_capacity(b.len() + 1), |mut row, j| {
            let cost = if i == 0 || j == 0 {
                i.max(j)
            } else {
                let substitution = rows[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]);
                let basic = substitution.min(rows[i - 1][j] + 1).min(row[j - 1] + 1);
                let swapped = i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1];
                if swapped {
                    basic.min(rows[i - 2][j - 2] + 1)
                } else {
                    basic
                }
            };
            row.push(cost);
            row
        });
        rows.push(row);
        rows
    });
    rows.last().and_then(|row| row.last()).copied().unwrap_or(0)
}

/// Lists candidate names for a diagnostic code.
///
/// # Arguments
/// * `document` - document being edited
/// * `code` - diagnostic code
/// * `offset` - offset of the diagnostic (selects the visible scope)
fn candidate_names(document: &Document, code: &str, offset: usize) -> Vec<String> {
    let analysis = &document.analysis;
    let scope = analysis.scope_at(offset);
    let visible = |kind: SymbolKind| -> Vec<String> {
        analysis
            .visible_symbols(kind, scope)
            .into_iter()
            .map(|id| analysis.symbols[id].name.text.clone())
            .collect()
    };
    match code {
        "unknown-command" => COMMANDS
            .iter()
            .flat_map(Named::spellings)
            .map(str::to_string)
            .collect(),
        "unknown-option" => OPTIONS
            .iter()
            .flat_map(Named::spellings)
            .map(str::to_string)
            .collect(),
        "undefined-subckt" => visible(SymbolKind::Subckt),
        "undefined-model" => visible(SymbolKind::Model)
            .into_iter()
            .map(|name| bin_base(&name).map_or_else(|| name.clone(), str::to_string))
            .collect(),
        "undefined-param" => visible(SymbolKind::Param),
        "undefined-function" => visible(SymbolKind::Function)
            .into_iter()
            .chain(FUNCTIONS.iter().map(|function| function.name.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Builds "Did you mean" fixes for a misspelled name.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `code` - diagnostic code
/// * `range` - diagnostic range
fn suggestions(
    workspace: &Workspace,
    document: &Document,
    code: &str,
    range: TextRange,
) -> Vec<Fix> {
    let Some((start, word)) = word_in(document.slice(range), code == "unknown-command") else {
        return Vec::new();
    };
    let word_range = TextRange::new(range.start + start, range.start + start + word.len());
    let lowered = word.to_ascii_lowercase();
    let mut ranked: Vec<(usize, String)> = candidate_names(document, code, range.start)
        .into_iter()
        .map(|name| (edit_distance(&lowered, &name.to_ascii_lowercase()), name))
        .filter(|(distance, name)| {
            (1..=MAX_DISTANCE).contains(distance) || (*distance == 0 && name != word)
        })
        .collect();
    ranked.sort();
    ranked.dedup_by(|a, b| a.1.eq_ignore_ascii_case(&b.1));
    ranked
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, name)| Fix {
            title: format!("Did you mean `{name}`?"),
            edits: vec![replace_edit(workspace, document, word_range, &name)],
            preferred: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::state::line_index::PositionEncoding;
    use ls_types::{CodeActionContext, TextDocumentIdentifier, Uri};
    use std::str::FromStr;

    fn workspace_with(text: &str) -> (Workspace, Uri) {
        let mut workspace = Workspace::new(None, PositionEncoding::Utf16, Settings::default());
        let uri = Uri::from_str("file:///tmp/t.sp").expect("uri");
        workspace.update(uri.clone(), 1, text.to_string());
        (workspace, uri)
    }

    fn request(workspace: &Workspace, uri: &Uri, code: &str, needle: &str) -> Vec<CodeAction> {
        let document = workspace.document(uri).expect("document");
        let start = document.text.find(needle).unwrap_or_default();
        let range = to_lsp_range(
            workspace,
            document,
            TextRange::new(start, start + needle.len()),
        );
        let diagnostic = ls_types::Diagnostic {
            range,
            code: Some(NumberOrString::String(code.to_string())),
            ..Default::default()
        };
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range,
            context: CodeActionContext {
                diagnostics: vec![diagnostic],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        code_actions(workspace, document, &params)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => Some(action),
                CodeActionOrCommand::Command(_) => None,
            })
            .collect()
    }

    fn apply(workspace: &Workspace, uri: &Uri, action: &CodeAction) -> String {
        let document = workspace.document(uri).expect("document");
        let mut edits: Vec<(TextRange, String)> = action
            .edit
            .as_ref()
            .and_then(|edit| edit.changes.as_ref())
            .and_then(|changes| changes.get(uri))
            .map(|edits| {
                edits
                    .iter()
                    .map(|edit| {
                        (
                            document.line_index.text_range(
                                &document.text,
                                edit.range,
                                workspace.encoding,
                            ),
                            edit.new_text.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        edits
            .iter()
            .fold(document.text.clone(), |mut text, (range, new_text)| {
                text.replace_range(range.start..range.end, new_text);
                text
            })
    }

    #[test]
    fn meg_suffix_fix_replaces_m() {
        let (workspace, uri) = workspace_with("t\nR1 a 0 10M\n.end\n");
        for needle in ["10M", "R1 a 0 10M"] {
            let actions = request(&workspace, &uri, "meg-suffix", needle);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].kind, Some(CodeActionKind::QUICKFIX));
            assert_eq!(actions[0].diagnostics.as_ref().map(Vec::len), Some(1));
            assert_eq!(
                apply(&workspace, &uri, &actions[0]),
                "t\nR1 a 0 10MEG\n.end\n"
            );
        }
    }

    #[test]
    fn meg_fix_handles_exponent_and_case() {
        let (workspace, uri) = workspace_with("t\nR1 a 0 1e3m\nR2 a 0 2.5m\n.end\n");
        let actions = request(&workspace, &uri, "meg-suffix", "2.5m");
        assert_eq!(
            apply(&workspace, &uri, &actions[0]),
            "t\nR1 a 0 1e3m\nR2 a 0 2.5meg\n.end\n"
        );
    }

    #[test]
    fn undefined_param_inserts_definition_before_statement() {
        let (workspace, uri) = workspace_with("t\nR1 a 0 1k\nR2 a 0 'rx*2'\n.end\n");
        let actions = request(&workspace, &uri, "undefined-param", "rx");
        assert_eq!(
            apply(&workspace, &uri, &actions[0]),
            "t\nR1 a 0 1k\n.param rx=0\nR2 a 0 'rx*2'\n.end\n"
        );
    }

    #[test]
    fn undefined_param_in_subckt_goes_to_body_start() {
        let text = "t\n.subckt s a b\nR1 a b 1k\nR2 a b 'rx'\n.ends s\n.end\n";
        let (workspace, uri) = workspace_with(text);
        let actions = request(&workspace, &uri, "undefined-param", "rx");
        assert_eq!(
            apply(&workspace, &uri, &actions[0]),
            "t\n.subckt s a b\n.param rx=0\nR1 a b 1k\nR2 a b 'rx'\n.ends s\n.end\n"
        );
    }

    #[test]
    fn unclosed_blocks_get_matching_closer() {
        let cases = [
            (".subckt s a b\nR1 a b 1k\n", ".subckt s a b", ".ends s"),
            (".lib tt\n.param x=1\n", ".lib tt", ".endl tt"),
            (".if (1)\n.param x=1\n", ".if", ".endif"),
            (".data d a\n+ 1\n", ".data", ".enddata"),
        ];
        for (body, needle, closer) in cases {
            let text = format!("t\n{body}.end\n");
            let (workspace, uri) = workspace_with(&text);
            let actions = request(&workspace, &uri, "unclosed-block", needle);
            assert_eq!(actions.len(), 1, "{needle}");
            let expected = format!("t\n{}\n{closer}\n.end\n", body.trim_end());
            assert_eq!(apply(&workspace, &uri, &actions[0]), expected);
        }
    }

    #[test]
    fn missing_end_appends_end() {
        let (workspace, uri) = workspace_with("t\nR1 a 0 1k");
        let actions = request(&workspace, &uri, "missing-end", "R1");
        assert_eq!(apply(&workspace, &uri, &actions[0]), "t\nR1 a 0 1k\n.end\n");
        let (workspace, uri) = workspace_with("t\nR1 a 0 1k\n");
        let actions = request(&workspace, &uri, "missing-end", "R1");
        assert_eq!(apply(&workspace, &uri, &actions[0]), "t\nR1 a 0 1k\n.end\n");
    }

    #[test]
    fn ends_name_mismatch_uses_subckt_name() {
        let (workspace, uri) = workspace_with("t\n.subckt inv a b\nR1 a b 1k\n.ends other\n.end\n");
        let actions = request(&workspace, &uri, "ends-name-mismatch", ".ends other");
        assert_eq!(
            apply(&workspace, &uri, &actions[0]),
            "t\n.subckt inv a b\nR1 a b 1k\n.ends inv\n.end\n"
        );
    }

    #[test]
    fn did_you_mean_for_commands() {
        let (workspace, uri) = workspace_with("t\n.tarn 1n 1u\n.end\n");
        let actions = request(&workspace, &uri, "unknown-command", ".tarn");
        assert!(!actions.is_empty() && actions.len() <= 3);
        assert_eq!(actions[0].title, "Did you mean `.tran`?");
        assert_eq!(
            apply(&workspace, &uri, &actions[0]),
            "t\n.tran 1n 1u\n.end\n"
        );
    }

    #[test]
    fn did_you_mean_for_subckt_model_param_function() {
        let text = "t\n.param width=1u\n.subckt inv a b\nR1 a b 1k\n.ends inv\n.model nch nmos\nX1 a b invv\nM1 a b 0 0 nchh\nR2 a 0 'widht'\nR3 a 0 'sqr(1)'\n.end\n";
        let (workspace, uri) = workspace_with(text);
        let cases = [
            ("undefined-subckt", "invv", "invv", "inv"),
            ("undefined-model", "nchh", "nchh", "nch"),
            ("undefined-param", "widht", "widht", "width"),
            ("undefined-function", "sqr", "sqr", "sqrt"),
        ];
        for (code, needle, old, new) in cases {
            let actions = request(&workspace, &uri, code, needle);
            let suggestion = actions
                .iter()
                .find(|action| action.title.contains(&format!("`{new}`")))
                .unwrap_or_else(|| panic!("{code}: {actions:?}"));
            assert!(apply(&workspace, &uri, suggestion).contains(new));
            assert!(!apply(&workspace, &uri, suggestion).is_empty() && !old.is_empty());
        }
    }

    #[test]
    fn did_you_mean_for_options() {
        let (workspace, uri) = workspace_with("t\n.option runlvll=3\n.end\n");
        let actions = request(&workspace, &uri, "unknown-option", "runlvll");
        assert_eq!(actions[0].title, "Did you mean `runlvl`?");
        assert_eq!(
            apply(&workspace, &uri, &actions[0]),
            "t\n.option runlvl=3\n.end\n"
        );
    }

    #[test]
    fn far_names_get_no_suggestion() {
        let (workspace, uri) = workspace_with("t\n.zzzzzzzz\n.end\n");
        assert!(request(&workspace, &uri, "unknown-command", ".zzzzzzzz").is_empty());
    }

    #[test]
    fn unknown_codes_and_missing_codes_give_nothing() {
        let (workspace, uri) = workspace_with("t\nR1 a 0 1k\n.end\n");
        assert!(request(&workspace, &uri, "floating-node", "R1").is_empty());
    }

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("tran", "tarn"), 1);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
    }
}
