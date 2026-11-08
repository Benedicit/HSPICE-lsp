//! Context-aware completion (commands, elements, names, keywords, options, paths).

mod candidate;
pub(crate) mod line_context;
mod sources;
#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) mod test_support;

use std::collections::HashSet;

use ls_types::{CompletionResponse, Position};

use self::candidate::Candidate;
use self::line_context::{FrameKind, LineScan, identifier_start, name_start};
use self::sources::Env;
use super::{to_lsp_range, to_offset};
use crate::analysis::symbols::SymbolKind;
use crate::lang::commands::find_command;
use crate::lang::elements::{ElementSpec, ReferenceKind, find_element, find_source_function};
use crate::lang::functions::find_function;
use crate::lang::models::model_params_for;
use crate::lang::output_vars::{OutputArgKind, OutputVarSpec, find_output_var};
use crate::state::document::Document;
use crate::state::workspace::Workspace;
use crate::syntax::ast::{Command, CommandKind, StatementKind, TextRange};

/// Maximum number of items returned for one request.
const MAX_ITEMS: usize = 400;

/// Candidates plus the byte offset where the replaced prefix starts.
struct Proposal {
    /// Candidates before prefix filtering.
    candidates: Vec<Candidate>,
    /// Start of the replaced prefix.
    start: usize,
}

impl Proposal {
    /// Creates a proposal.
    ///
    /// # Arguments
    /// * `candidates` - candidates before prefix filtering
    /// * `start` - start of the replaced prefix
    fn new(candidates: Vec<Candidate>, start: usize) -> Self {
        Self { candidates, start }
    }
}

/// Computes completion items at a position.
///
/// # Arguments
/// * `workspace` - workspace state
/// * `document` - document being edited
/// * `position` - cursor position
pub fn completion(
    workspace: &Workspace,
    document: &Document,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = to_offset(workspace, document, position);
    if document
        .syntax
        .title_ranges
        .iter()
        .any(|range| range.touches(offset))
    {
        return None;
    }
    let scan = line_context::scan(&document.text, offset)?;
    let env = Env {
        workspace,
        document,
        scope: scope_at_cursor(document, &scan),
    };
    let proposal = propose(env, &scan)?;
    let prefix = document.text.get(proposal.start..offset)?;
    let range = to_lsp_range(workspace, document, TextRange::new(proposal.start, offset));
    let mut candidates: Vec<Candidate> = proposal
        .candidates
        .into_iter()
        .filter(|candidate| candidate.matches(prefix))
        .collect();
    candidates.sort_by(|left, right| (&left.sort, &left.label).cmp(&(&right.sort, &right.label)));
    let items: Vec<_> = candidates
        .into_iter()
        .take(MAX_ITEMS)
        .map(|candidate| candidate.into_item(range))
        .collect();
    (!items.is_empty()).then_some(CompletionResponse::Array(items))
}

/// Returns the scope the cursor is in, using the logical line or the closest preceding statement.
///
/// # Arguments
/// * `document` - analysed document
/// * `scan` - scanned logical line
fn scope_at_cursor(document: &Document, scan: &LineScan) -> usize {
    let structure = &document.analysis.structure;
    let statements = &document.syntax.netlist.statements;
    let on_line = statements
        .iter()
        .position(|statement| statement.range.touches(scan.line_start));
    let preceding = statements
        .iter()
        .rposition(|statement| statement.range.end <= scan.line_start);
    let scope_after = |index: usize| match &statements[index].kind {
        StatementKind::Command(Command {
            kind: CommandKind::Ends { .. },
            ..
        }) => document.analysis.scope_at(scan.cursor),
        StatementKind::Command(Command {
            kind: CommandKind::Subckt { .. },
            ..
        }) => structure
            .scopes
            .iter()
            .position(|scope| scope.subckt_statement == Some(index))
            .unwrap_or_default(),
        _ => structure
            .statement_scope
            .get(index)
            .copied()
            .unwrap_or_default(),
    };
    on_line
        .map(|index| {
            structure
                .statement_scope
                .get(index)
                .copied()
                .unwrap_or_default()
        })
        .or_else(|| preceding.map(scope_after))
        .unwrap_or_else(|| document.analysis.scope_at(scan.cursor))
}

/// Chooses the candidates for the frame the cursor is in.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn propose(env: Env, scan: &LineScan) -> Option<Proposal> {
    match scan.innermost().kind {
        FrameKind::Quote(_) => quote_proposal(env, scan),
        FrameKind::Paren => call_proposal(env, scan),
        FrameKind::Root => root_proposal(env, scan),
    }
}

/// Returns true if a lowercase first word starts a command that takes file paths.
///
/// # Arguments
/// * `first` - lowercase first word
fn is_path_command(first: &str) -> bool {
    find_command(first).is_some_and(|command| matches!(command.name, ".include" | ".lib" | ".hdl"))
}

/// Proposes expression names (parameters, functions, variables) at the identifier before the cursor.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn expression_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    let start = identifier_start(&env.document.text, scan.cursor);
    let starts_with_digit = env.document.text[start..scan.cursor]
        .starts_with(|character: char| character.is_ascii_digit());
    (!starts_with_digit).then(|| Proposal::new(sources::expression_names(env), start))
}

/// Proposes paths or section names inside a quote of an include-like command.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn quoted_path_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    let open = scan.innermost().open;
    let typed = scan.slice(open + 1, scan.cursor);
    let command = find_command(&scan.first_word())?;
    if command.name == ".lib" && !scan.root().tokens.is_empty() && scan.root().tokens.len() >= 2 {
        return Some(Proposal::new(
            sources::lib_sections(env, &unquote(&scan.root().tokens[1].text)),
            open + 1,
        ));
    }
    let (candidates, replaced) = sources::paths(env, typed);
    Some(Proposal::new(candidates, scan.cursor - replaced))
}

/// Proposes candidates inside an open quote.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn quote_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    if scan.frames.len() == 2 && is_path_command(&scan.first_word()) {
        return quoted_path_proposal(env, scan);
    }
    expression_proposal(env, scan)
}

/// Looks up an output variable by name, accepting a trailing terminal number (`i1`, `v2`).
///
/// # Arguments
/// * `name` - call name as typed
fn output_var(name: &str) -> Option<&'static OutputVarSpec> {
    find_output_var(name).or_else(|| {
        find_output_var(name.trim_end_matches(|character: char| character.is_ascii_digit()))
    })
}

/// Proposes candidates inside an open parenthesis.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn call_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    let frame = scan.innermost();
    let first = scan.first_word();
    let is_vi_line = first.starts_with(['v', 'i']);
    let name_start_offset = name_start(&env.document.text, scan.cursor).max(frame.open + 1);
    if find_source_function(&frame.name).is_some() && is_vi_line && !scan.in_quote() {
        return expression_proposal(env, scan);
    }
    match output_var(&frame.name).map(|spec| spec.argument) {
        Some(OutputArgKind::Nodes) => {
            Some(Proposal::new(sources::nodes(env, "0"), name_start_offset))
        }
        Some(OutputArgKind::Element) => Some(Proposal::new(
            sources::element_names(env, &[], "0"),
            name_start_offset,
        )),
        Some(OutputArgKind::Expression | OutputArgKind::Other) => expression_proposal(env, scan),
        None => unnamed_call_proposal(env, scan, &first),
    }
}

/// Proposes candidates in a parenthesis that is not an output variable call.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
/// * `first` - lowercase first word of the line
fn unnamed_call_proposal(env: Env, scan: &LineScan, first: &str) -> Option<Proposal> {
    let frame = scan.innermost();
    let user_function = env
        .document
        .analysis
        .lookup(
            SymbolKind::Function,
            env.scope,
            &frame.name.to_ascii_lowercase(),
        )
        .is_some();
    if first == ".model" && frame.name.is_empty() {
        return model_param_proposal(scan);
    }
    let expression = scan.in_quote()
        || user_function
        || find_function(&frame.name).is_some()
        || matches!(
            first,
            ".param" | ".if" | ".elseif" | ".parameter" | ".parameters"
        );
    expression.then(|| expression_proposal(env, scan)).flatten()
}

/// Proposes `key=` completions for a model card.
///
/// # Arguments
/// * `scan` - scanned logical line
fn model_param_proposal(scan: &LineScan) -> Option<Proposal> {
    let model_type = scan.positionals().get(1).map(|token| token.text.clone())?;
    let used: HashSet<String> = scan
        .text
        .split(|character: char| character.is_whitespace() || character == '(' || character == ')')
        .filter_map(|word| {
            word.split_once('=')
                .map(|(key, _)| key.to_ascii_lowercase())
        })
        .collect();
    Some(Proposal::new(
        sources::model_params(model_params_for(&model_type), &used),
        scan.word_start(),
    ))
}

/// Removes surrounding quotes from a word.
///
/// # Arguments
/// * `text` - word as typed
fn unquote(text: &str) -> String {
    text.trim_matches(|character| character == '\'' || character == '"')
        .to_string()
}

/// Proposes candidates for the line level (first word, positional arguments, assignments).
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn root_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    if scan.in_first_word() {
        return line_start_proposal(scan);
    }
    if scan.is_command() {
        command_proposal(env, scan)
    } else {
        element_proposal(env, scan)
    }
}

/// Proposes commands and element templates for the first word of a line.
///
/// # Arguments
/// * `scan` - scanned logical line
fn line_start_proposal(scan: &LineScan) -> Option<Proposal> {
    let word = scan.current_word();
    let candidates = match word.chars().next() {
        Some('.') => sources::commands(),
        Some(_) => sources::elements(),
        None => sources::commands()
            .into_iter()
            .chain(sources::elements())
            .collect(),
    };
    Some(Proposal::new(candidates, scan.word_start()))
}

/// Returns true if the value after `=` of this command is an expression.
///
/// # Arguments
/// * `command` - canonical command name
fn takes_expression_values(command: &str) -> bool {
    matches!(
        command,
        ".param"
            | ".subckt"
            | ".measure"
            | ".if"
            | ".elseif"
            | ".ic"
            | ".nodeset"
            | ".global"
            | ".mosra"
    )
}

/// Returns true if every one of the tokens looks like a number or expression.
///
/// # Arguments
/// * `tokens` - words to test
fn all_numeric(tokens: &[&line_context::Tok]) -> bool {
    tokens.iter().all(|token| {
        token
            .text
            .starts_with(|c: char| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | '\'' | '"'))
    })
}

/// Proposes candidates for the arguments of a dot command.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn command_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    let command = find_command(&scan.first_word())?.name;
    let current = scan.current_word();
    let start = scan.word_start();
    let positionals = scan.positionals();
    let n = positionals.len();
    if current.contains('=') {
        return takes_expression_values(command)
            .then(|| expression_proposal(env, scan))
            .flatten();
    }
    let candidates = match command {
        ".option" => sources::options(!current.is_empty()),
        ".mosra" => sources::mosra_params(),
        ".model" => match n {
            0 => return None,
            1 => sources::model_types(),
            _ => return model_param_proposal(scan),
        },
        ".include" | ".hdl" if n == 0 => return unquoted_path_proposal(env, scan),
        ".lib" if n == 0 => return unquoted_path_proposal(env, scan),
        ".lib" if n == 1 => sources::lib_sections(env, &unquote(&positionals[0].text)),
        ".print" | ".probe" | ".plot" | ".graph" if n == 0 => sources::analysis_types()
            .into_iter()
            .chain(sources::output_variables())
            .collect(),
        ".print" | ".probe" | ".plot" | ".graph" | ".ic" | ".nodeset" | ".dcvolt" => {
            sources::output_variables()
        }
        ".measure" => measure_candidates(env, n),
        ".dc" => dc_candidates(env, &positionals),
        ".noise" | ".tf" | ".pz" | ".disto" if n == 0 => sources::output_variables(),
        ".noise" | ".tf" | ".pz" if n == 1 => sources::element_names(env, &['v', 'i'], "0"),
        ".tran" | ".ac" | ".op" | ".noise" | ".sens" | ".four" | ".fft" | ".tf" | ".pz"
        | ".disto" => sources::analysis_keywords(),
        ".if" | ".elseif" => return expression_proposal(env, scan),
        _ => return None,
    };
    Some(Proposal::new(candidates, start))
}

/// Returns the candidates of the `.measure` argument at a position.
///
/// # Arguments
/// * `env` - cursor environment
/// * `n` - number of positional words before the cursor
fn measure_candidates(env: Env, n: usize) -> Vec<Candidate> {
    match n {
        0 => sources::analysis_types(),
        1 => Vec::new(),
        _ => {
            let names = sources::expression_names(env)
                .into_iter()
                .filter(|candidate| candidate.sort == "0");
            sources::measure_keywords()
                .into_iter()
                .chain(sources::output_variables())
                .chain(names)
                .collect()
        }
    }
}

/// Returns the candidates of the `.dc` argument at a position.
///
/// # Arguments
/// * `env` - cursor environment
/// * `positionals` - positional words before the cursor
fn dc_candidates(env: Env, positionals: &[&line_context::Tok]) -> Vec<Candidate> {
    let n = positionals.len();
    let after_sweep = positionals
        .last()
        .is_some_and(|token| token.text.eq_ignore_ascii_case("sweep"));
    let next_variable = n >= 4 && n.is_multiple_of(4) && all_numeric(&positionals[n - 3..]);
    let variables = if n == 0 || after_sweep || next_variable {
        sources::sweep_variables(env)
    } else {
        Vec::new()
    };
    variables
        .into_iter()
        .chain(sources::analysis_keywords())
        .collect()
}

/// Proposes paths for an unquoted include argument.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn unquoted_path_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    let (candidates, replaced) = sources::paths(env, scan.current_word());
    Some(Proposal::new(candidates, scan.cursor - replaced))
}

/// Proposes candidates for the arguments of an element line.
///
/// # Arguments
/// * `env` - cursor environment
/// * `scan` - scanned logical line
fn element_proposal(env: Env, scan: &LineScan) -> Option<Proposal> {
    let first = scan.root().tokens.first()?.text.clone();
    let spec = find_element(&first)?;
    if scan.current_word().contains('=') {
        return expression_proposal(env, scan);
    }
    let positionals = scan.positionals();
    let candidates = if spec.letter == 'X' {
        subckt_instance_candidates(env, &positionals)
    } else {
        device_candidates(env, spec, &positionals)
    };
    Some(Proposal::new(candidates, scan.word_start()))
}

/// Returns the candidates of an X instance argument.
///
/// # Arguments
/// * `env` - cursor environment
/// * `positionals` - positional words after the element name before the cursor
fn subckt_instance_candidates(env: Env, positionals: &[&line_context::Tok]) -> Vec<Candidate> {
    let analysis = &env.document.analysis;
    let subckt = positionals.iter().find(|token| {
        analysis
            .lookup(
                SymbolKind::Subckt,
                env.scope,
                &token.text.to_ascii_lowercase(),
            )
            .is_some()
    });
    match subckt {
        Some(token) => sources::subckt_params(env, &token.text.to_ascii_lowercase()),
        None => sources::nodes(env, "1")
            .into_iter()
            .chain(sources::subckts(env, Some(positionals.len())))
            .collect(),
    }
}

/// Returns the model types an element refers to, if its reference is a model.
///
/// # Arguments
/// * `spec` - element type
fn model_types_of(spec: &ElementSpec) -> Option<&'static [&'static str]> {
    match spec.reference {
        ReferenceKind::Model(types) | ReferenceKind::OptionalModel(types) => Some(types),
        _ => None,
    }
}

/// Returns the candidates of a non-X element argument.
///
/// # Arguments
/// * `env` - cursor environment
/// * `spec` - element type
/// * `positionals` - positional words after the element name before the cursor
fn device_candidates(
    env: Env,
    spec: &ElementSpec,
    positionals: &[&line_context::Tok],
) -> Vec<Candidate> {
    let n = positionals.len();
    let analysis = &env.document.analysis;
    let node_slots = spec.max_nodes.unwrap_or(if spec.terminals.is_empty() {
        usize::MAX
    } else {
        spec.terminals.len()
    });
    let node_open = n < node_slots;
    let model_found = positionals.iter().any(|token| {
        analysis
            .lookup(
                SymbolKind::Model,
                env.scope,
                &token.text.to_ascii_lowercase(),
            )
            .is_some()
    });
    let nodes = if node_open {
        sources::nodes(env, "1")
    } else {
        Vec::new()
    };
    let models = model_types_of(spec)
        .filter(|_| n >= spec.min_nodes && !model_found)
        .map(|types| sources::models(env, types, if node_open { "2" } else { "0" }))
        .unwrap_or_default();
    let keys =
        if n >= spec.min_nodes && (!node_open || model_found || n >= spec.terminals.len().max(1)) {
            sources::element_params(spec)
        } else {
            Vec::new()
        };
    let sources_keywords = if matches!(spec.letter, 'V' | 'I') && n >= 2 {
        sources::source_keywords()
    } else {
        Vec::new()
    };
    let references = match spec.reference {
        ReferenceKind::ControllingSources if n == 2 => sources::element_names(env, &['v'], "0"),
        ReferenceKind::Inductors => sources::element_names(env, &['l'], "0"),
        _ => Vec::new(),
    };
    nodes
        .into_iter()
        .chain(models)
        .chain(sources_keywords)
        .chain(references)
        .chain(keys)
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use ls_types::{CompletionItem, CompletionItemKind, CompletionTextEdit};

    use super::test_support::{Opened, open, open_in_fixtures};
    use super::*;

    fn complete(opened: &Opened) -> Vec<CompletionItem> {
        let document = opened.workspace.document(&opened.uri).expect("document");
        match completion(&opened.workspace, document, opened.position) {
            Some(CompletionResponse::Array(items)) => items,
            _ => Vec::new(),
        }
    }

    fn labels(marked: &str) -> Vec<String> {
        complete(&open(marked))
            .into_iter()
            .map(|item| item.label)
            .collect()
    }

    fn has(labels: &[String], wanted: &str) -> bool {
        labels.iter().any(|label| label == wanted)
    }

    fn edit_text(item: &CompletionItem) -> (String, u32) {
        match &item.text_edit {
            Some(CompletionTextEdit::Edit(edit)) => {
                (edit.new_text.clone(), edit.range.start.character)
            }
            _ => (String::new(), 0),
        }
    }

    #[test]
    fn completes_dot_commands_with_leading_dot() {
        let items = complete(&open("* t\n.tr|"));
        let tran = items
            .iter()
            .find(|item| item.label == ".TRAN")
            .expect(".TRAN");
        assert_eq!(
            edit_text(tran),
            (".TRAN ${1:tstep} ${2:tstop}".to_string(), 0)
        );
        assert!(tran.insert_text_format.is_some());
        assert!(items.iter().all(|item| item.label.starts_with(".TR")));
    }

    #[test]
    fn commands_are_ranked_and_foreign_ones_skipped() {
        let items = complete(&open("* t\n|"));
        assert!(items.iter().any(|item| item.label == ".TRAN"));
        assert!(
            items
                .iter()
                .any(|item| item.label.starts_with("M \u{2014}"))
        );
        let first = items.first().expect("first");
        assert!(
            first
                .sort_text
                .as_deref()
                .is_some_and(|sort| sort.starts_with('0'))
        );
        let measure = items
            .iter()
            .find(|item| item.label == ".MEASURE")
            .expect(".MEASURE");
        assert!(
            measure
                .filter_text
                .as_deref()
                .is_some_and(|text| text.contains(".meas"))
        );
    }

    #[test]
    fn completes_element_templates() {
        let items = complete(&open("* t\nM|"));
        assert_eq!(items.len(), 1);
        assert!(items[0].label.starts_with("M \u{2014}"));
        assert!(edit_text(&items[0]).0.starts_with("M${1:1}"));
    }

    #[test]
    fn completes_x_nodes_and_subckts() {
        let text = ".subckt inv in out vdd gnd wn=1u\n.ends\nX1 a b |\n";
        let found = labels(&format!("* t\n{text}").replace("X1 a b |\n", "X1 a b |"));
        assert!(has(&found, "inv"));
        assert!(has(&found, "a"));
        assert!(has(&found, "0"));
    }

    #[test]
    fn completes_on_crlf_continuation_lines() {
        let opened = open("* t\r\n.subckt inv in out\r\n.ends\r\nX1 a\r\n+ b i|");
        let items = complete(&opened);
        let inv = items.iter().find(|item| item.label == "inv").expect("inv");
        match &inv.text_edit {
            Some(CompletionTextEdit::Edit(edit)) => {
                assert_eq!(edit.range.start.line, opened.position.line);
                assert_eq!(edit.range.start.character, opened.position.character - 1);
            }
            _ => panic!("expected a text edit"),
        }
    }

    #[test]
    fn completes_x_instance_params_after_subckt_name() {
        let found = labels("* t\n.subckt inv in out wn=1u wp=2u\n.ends\nX1 a b inv |");
        assert!(has(&found, "wn="));
        assert!(has(&found, "wp="));
        assert!(has(&found, "M="));
        assert!(!has(&found, "inv"));
    }

    #[test]
    fn completes_models_for_mosfets_with_bins_collapsed() {
        let text = "* t\n.model nch.1 nmos level=54\n.model nch.2 nmos level=54\n.model pch pmos\n.model dd d\nM1 d g s b |";
        let found = labels(text);
        assert!(has(&found, "nch"));
        assert!(has(&found, "pch"));
        assert!(!has(&found, "nch.1"));
        assert!(!has(&found, "dd"));
        assert!(has(&found, "W="));
    }

    #[test]
    fn mosfet_third_node_offers_nodes_and_models() {
        let found = labels("* t\n.model nch nmos\nR1 x y 1k\nM1 d g |");
        assert!(has(&found, "x"));
        assert!(!has(&found, "nch"));
        let found = labels("* t\n.model nch nmos\nR1 x y 1k\nM1 d g s |");
        assert!(has(&found, "x"));
        assert!(has(&found, "nch"));
    }

    #[test]
    fn completes_instance_keys() {
        let found = labels("* t\nR1 a b 1k t|");
        assert!(has(&found, "TC1="));
        assert!(!has(&found, "W="));
    }

    #[test]
    fn completes_source_keywords_after_nodes() {
        let items = complete(&open("* t\nVin in 0 pu|"));
        let pulse = items
            .iter()
            .find(|item| item.label == "PULSE")
            .expect("pulse");
        let (text, _) = edit_text(pulse);
        assert!(text.starts_with("PULSE(${1:v1} ${2:v2} ${3:td}"));
        let found = labels("* t\nVin in 0 |");
        assert!(has(&found, "DC") && has(&found, "SIN") && has(&found, "PWL"));
    }

    #[test]
    fn completes_params_inside_quotes_and_values() {
        let found = labels("* t\n.param wn=1u\nR1 a b '2*w|'");
        assert!(has(&found, "wn"));
        let found = labels("* t\nR1 a b '2*|'");
        assert!(has(&found, "max") && has(&found, "time"));
        let found = labels("* t\n.param wn=1u wp=2*|");
        assert!(has(&found, "wn") && has(&found, "time") && has(&found, "sqrt"));
        let found = labels("* t\n.param wn=1u\nR1 a b r=w|");
        assert!(has(&found, "wn"));
    }

    #[test]
    fn typing_inside_quote_replaces_only_the_identifier() {
        let items = complete(&open("* t\n.param wn=1u\nR1 a b '2*w|"));
        let wn = items.iter().find(|item| item.label == "wn").expect("wn");
        assert_eq!(edit_text(wn), ("wn".to_string(), 10));
    }

    #[test]
    fn completes_user_functions_with_formals() {
        let items = complete(&open("* t\n.param f(x,y)=x+y\nR1 a b 'f|'"));
        let function = items.iter().find(|item| item.label == "f").expect("f");
        assert_eq!(edit_text(function).0, "f(${1:x}, ${2:y})");
    }

    #[test]
    fn completes_if_conditions() {
        let found = labels("* t\n.param vdd=1\n.if (vd|");
        assert!(has(&found, "vdd"));
    }

    #[test]
    fn completes_options() {
        let items = complete(&open("* t\n.option po|"));
        assert!(items.iter().any(|item| item.label == "POST"));
        let many = complete(&open("* t\n.option |"));
        assert!(!many.is_empty() && many.len() < 120);
        let accuracy = complete(&open("* t\n.option numd|"));
        let numdgt = accuracy
            .iter()
            .find(|item| item.label == "NUMDGT")
            .expect("NUMDGT");
        assert_eq!(edit_text(numdgt).0, "NUMDGT=${1}");
        let renamed = complete(&open("* t\n.option run_l|"));
        assert!(renamed.iter().any(|item| item.label == "RUNLVL"));
    }

    #[test]
    fn completes_model_types_and_params() {
        let found = labels("* t\n.model nch n|");
        assert!(has(&found, "NMOS") && !has(&found, "PMOS"));
        let found = labels("* t\n.model nch nmos lev|");
        assert!(has(&found, "LEVEL="));
        let found = labels("* t\n.model nch nmos (level=54 vt|");
        assert!(has(&found, "VTO=") || has(&found, "VTH0="));
        let found = labels("* t\n.model nch nmos level=1 |");
        assert!(!has(&found, "LEVEL="));
    }

    #[test]
    fn completes_analysis_keywords_and_sweep_variables() {
        let found = labels("* t\n.ac |");
        assert!(has(&found, "DEC") && has(&found, "LIN"));
        let found = labels("* t\n.tran 1n 10n |");
        assert!(has(&found, "UIC") && has(&found, "START=") && has(&found, "SWEEP"));
        let found = labels("* t\nVin a 0 dc 1\n.param p=1\n.dc |");
        assert!(has(&found, "Vin") && has(&found, "p"));
        let found = labels("* t\nVin a 0 dc 1\nVx b 0 dc 1\n.dc Vin 0 1 0.1 |");
        assert!(has(&found, "Vx"));
    }

    #[test]
    fn completes_print_and_measure() {
        let found = labels("* t\n.print |");
        assert!(has(&found, "TRAN") && has(&found, "V("));
        let found = labels("* t\nR1 a b 1k\n.print tran v(|");
        assert!(has(&found, "a") && has(&found, "b"));
        let found = labels("* t\nVdd a 0 1\nR1 a b 1k\n.probe tran i(|");
        assert!(has(&found, "Vdd") && has(&found, "R1"));
        let found = labels("* t\n.meas |");
        assert!(has(&found, "TRAN"));
        let found = labels("* t\n.param lvl=1\n.meas tran d trig v(a) |");
        assert!(has(&found, "VAL=") && has(&found, "TARG") && has(&found, "lvl"));
        let found = labels("* t\nR1 a b 1k\n.meas tran d trig v(|)");
        assert!(has(&found, "a"));
    }

    #[test]
    fn nodes_of_other_scopes_are_hidden() {
        let found = labels(
            "* t\n.global vdd\n.subckt s p\nR1 p inner 1k\n.ends\nR2 top x 1k\n.print tran v(|",
        );
        assert!(has(&found, "top") && has(&found, "vdd"));
        assert!(!has(&found, "inner"));
        let found = labels("* t\n.global vdd\n.subckt s p\nR1 p inner 1k\nR2 inner |");
        assert!(
            has(&found, "vdd") && has(&found, "inner") && !has(&found, "top"),
            "{found:?}"
        );
    }

    #[test]
    fn completes_include_paths_and_sections() {
        let items = complete(&open_in_fixtures("* t\n.include 'ce|"));
        assert!(
            items
                .iter()
                .any(|item| item.label == "cells" && item.kind == Some(CompletionItemKind::FOLDER))
        );
        let items = complete(&open_in_fixtures("* t\n.include 'cells/|"));
        let inv = items
            .iter()
            .find(|item| item.label == "inv.inc")
            .expect("inv.inc");
        assert_eq!(inv.kind, Some(CompletionItemKind::FILE));
        assert_eq!(edit_text(inv).1, 16);
        let items = complete(&open_in_fixtures("* t\n.lib 'models/tech.lib' |"));
        let names: Vec<_> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(names, vec!["ff", "ss", "tt"]);
        let items = complete(&open_in_fixtures("* t\n.lib 'models/tech.lib' f|"));
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn works_on_the_inverter_fixture() {
        let text = std::fs::read_to_string(test_support::fixtures().join("inverter_tb.sp"))
            .expect("fixture");
        let marked = text.replace(".end", "X2 a b vdd 0 |\n.end");
        let items = complete(&open_in_fixtures(&marked));
        assert!(
            items.iter().any(|item| item.label == "inv"),
            "{:?}",
            items.iter().map(|i| &i.label).collect::<Vec<_>>()
        );
        let inv = items.iter().find(|item| item.label == "inv").expect("inv");
        assert_eq!(
            inv.detail.as_deref(),
            Some(".subckt inv in out vdd gnd wn wp")
        );
        assert_eq!(inv.sort_text.as_deref().map(|text| &text[..1]), Some("0"));
    }

    #[test]
    fn no_completion_in_title_or_comments() {
        assert!(labels("Titl|e\nR1 a b 1k").is_empty());
        assert!(labels("* t\n* comm|ent").is_empty());
        assert!(labels("* t\nR1 a b 1k $ note|").is_empty());
    }

    #[test]
    fn blank_lines_inside_unclosed_subckts_use_the_subckt_scope() {
        let found = labels("* t\n.subckt s p\nR1 p inner 1k\nR2 inner |\n");
        assert!(has(&found, "p") && has(&found, "inner"));
        let found = labels("* t\n.subckt s p\nR1 p inner 1k\n.ends\nR2 top |\n");
        assert!(has(&found, "top") && !has(&found, "inner"));
    }

    #[test]
    fn continuation_lines_join_the_statement() {
        let found = labels("* t\n.model nch nmos\nM1 d g\n+ s b |");
        assert!(has(&found, "nch"));
    }
}
