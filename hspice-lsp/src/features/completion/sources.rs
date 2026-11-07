//! Completion candidates for every kind of netlist entity and keyword table.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ls_types::CompletionItemKind;

use super::candidate::{Candidate, escape_snippet};
use crate::analysis::includes::{read_source, resolve_include_path};
use crate::analysis::symbols::{ScopeId, Symbol, SymbolDetail, SymbolKind};
use crate::features::definition_text;
use crate::lang::commands::COMMANDS;
use crate::lang::elements::{ELEMENTS, ElementSpec, SOURCE_FUNCTIONS};
use crate::lang::functions::{FUNCTIONS, SPECIAL_VARIABLES};
use crate::lang::measure::{ANALYSIS_KEYWORDS, ANALYSIS_TYPES, MEASURE_KEYWORDS};
use crate::lang::models::{MODEL_TYPES, ModelParamSpec};
use crate::lang::options::{OPTIONS, OptionValue};
use crate::lang::output_vars::OUTPUT_VARS;
use crate::lang::{KeywordSpec, Status, Usage};
use crate::state::document::Document;
use crate::state::workspace::Workspace;

/// File extensions offered by path completion.
const NETLIST_EXTENSIONS: &[&str] = &[
    "sp", "spice", "inc", "lib", "l", "pm", "mod", "va", "cir", "net",
];

/// What source completion needs to know about the cursor position.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    /// Workspace state.
    pub workspace: &'a Workspace,
    /// Document being edited.
    pub document: &'a Document,
    /// Innermost scope at the cursor.
    pub scope: ScopeId,
}

/// Returns the sort digit of a usage class.
///
/// # Arguments
/// * `usage` - how common the construct is
/// * `status` - support status
fn usage_sort(usage: Usage, status: Status) -> &'static str {
    match (status, usage) {
        (Status::Obsolete, _) => "8",
        (_, Usage::Common) => "0",
        (_, Usage::Occasional) => "1",
        (_, Usage::Rare) => "2",
    }
}

/// Returns the first line of a text.
///
/// # Arguments
/// * `text` - possibly multi-line text
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

/// Builds the Markdown documentation of a keyword with a syntax block.
///
/// # Arguments
/// * `synopsis` - syntax text (may span lines)
/// * `description` - prose description
fn syntax_documentation(synopsis: &str, description: &str) -> String {
    format!("```spice\n{synopsis}\n```\n{description}")
}

/// Returns all dot commands (without foreign ones).
pub fn commands() -> Vec<Candidate> {
    COMMANDS
        .iter()
        .filter(|spec| spec.status != Status::Foreign)
        .map(|spec| {
            let filter = std::iter::once(spec.name)
                .chain(spec.aliases.iter().copied())
                .collect::<Vec<_>>()
                .join(" ");
            let mut documentation = syntax_documentation(spec.synopsis, spec.description);
            if spec.status == Status::Obsolete {
                documentation.push_str("\n\n_Obsolete._");
            }
            if !spec.aliases.is_empty() {
                documentation.push_str(&format!("\n\nAliases: `{}`", spec.aliases.join("`, `")));
            }
            let candidate = Candidate::new(spec.name, CompletionItemKind::KEYWORD)
                .detail(first_line(spec.synopsis))
                .documentation(documentation)
                .filter(filter)
                .sort(usage_sort(spec.usage, spec.status))
                .deprecated(spec.status == Status::Obsolete);
            match spec.snippet {
                Some(snippet) => candidate.snippet(snippet),
                None => candidate,
            }
        })
        .collect()
}

/// Returns the element templates (one per element letter).
pub fn elements() -> Vec<Candidate> {
    ELEMENTS
        .iter()
        .map(|spec| {
            Candidate::new(
                format!("{} \u{2014} {}", spec.letter, spec.name),
                CompletionItemKind::SNIPPET,
            )
            .filter(spec.letter.to_string())
            .detail(spec.synopsis)
            .documentation(syntax_documentation(spec.synopsis, spec.description))
            .snippet(spec.snippet)
        })
        .collect()
}

/// Returns node names visible in a scope (scope nodes, global nodes and ground).
///
/// # Arguments
/// * `env` - cursor environment
/// * `sort` - sort key
pub fn nodes(env: Env, sort: &str) -> Vec<Candidate> {
    let mut seen = HashSet::new();
    let symbols = env.document.analysis.symbols.iter().filter(|symbol| {
        symbol.kind == SymbolKind::Node
            && (symbol.scope == env.scope
                || matches!(symbol.detail, SymbolDetail::Node { global: true, .. }))
    });
    symbols
        .filter(|symbol| seen.insert(symbol.name.key.clone()))
        .map(|symbol| {
            let note = match symbol.detail {
                SymbolDetail::Node { global: true, .. } => "global node".to_string(),
                SymbolDetail::Node {
                    port_index: Some(index),
                    ..
                } => format!("port {}", index + 1),
                _ => "node".to_string(),
            };
            Candidate::new(symbol.name.text.clone(), CompletionItemKind::VARIABLE)
                .detail(note)
                .sort(sort)
        })
        .chain(std::iter::once(
            Candidate::new("0", CompletionItemKind::CONSTANT)
                .detail("ground")
                .sort(format!("{sort}9")),
        ))
        .collect()
}

/// Returns the one-line description of a subcircuit header.
///
/// # Arguments
/// * `symbol` - subcircuit symbol
fn subckt_header(symbol: &Symbol) -> String {
    match &symbol.detail {
        SymbolDetail::Subckt { ports, params, .. } => {
            let ports = ports.iter().map(|port| port.text.as_str());
            let params = params.iter().map(|param| param.name.text.as_str());
            std::iter::once(".subckt")
                .chain(std::iter::once(symbol.name.text.as_str()))
                .chain(ports)
                .chain(params)
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => format!(".subckt {}", symbol.name.text),
    }
}

/// Returns the subcircuits visible in a scope.
///
/// # Arguments
/// * `env` - cursor environment
/// * `typed_ports` - number of nodes already typed (subcircuits with that many ports sort first)
pub fn subckts(env: Env, typed_ports: Option<usize>) -> Vec<Candidate> {
    let analysis = &env.document.analysis;
    analysis
        .visible_symbols(SymbolKind::Subckt, env.scope)
        .into_iter()
        .map(|id| &analysis.symbols[id])
        .map(|symbol| {
            let exact = match &symbol.detail {
                SymbolDetail::Subckt { ports, .. } => Some(ports.len()) == typed_ports,
                _ => false,
            };
            Candidate::new(symbol.name.text.clone(), CompletionItemKind::MODULE)
                .detail(subckt_header(symbol))
                .documentation(format!(
                    "```spice\n{}\n```",
                    first_line(definition_text(env.document, symbol))
                ))
                .sort(if exact { "0" } else { "2" })
        })
        .collect()
}

/// Returns the base name of a binned model name (`nch.1` becomes `nch`).
///
/// # Arguments
/// * `name` - model name as defined
fn base_model_name(name: &str) -> &str {
    name.rsplit_once('.')
        .filter(|(base, bin)| {
            !base.is_empty() && !bin.is_empty() && bin.chars().all(|c| c.is_ascii_digit())
        })
        .map_or(name, |(base, _)| base)
}

/// Returns the models whose type is one of `types`, binned names collapsed.
///
/// # Arguments
/// * `env` - cursor environment
/// * `types` - accepted lowercase model types (empty accepts all)
/// * `sort` - sort key
pub fn models(env: Env, types: &[&str], sort: &str) -> Vec<Candidate> {
    let analysis = &env.document.analysis;
    let mut seen = HashSet::new();
    analysis
        .visible_symbols(SymbolKind::Model, env.scope)
        .into_iter()
        .map(|id| &analysis.symbols[id])
        .filter(|symbol| match &symbol.detail {
            SymbolDetail::Model {
                model_type: Some(kind),
                ..
            } => types.is_empty() || types.contains(&kind.key.as_str()),
            _ => types.is_empty(),
        })
        .filter(|symbol| seen.insert(base_model_name(&symbol.name.key).to_string()))
        .map(|symbol| {
            let base = base_model_name(&symbol.name.text).to_string();
            let bins = analysis.table.bins(&base.to_ascii_lowercase()).len();
            let detail = match bins {
                0 => first_line(definition_text(env.document, symbol)).to_string(),
                count => format!(
                    "{} ({count} bins)",
                    first_line(definition_text(env.document, symbol))
                ),
            };
            Candidate::new(base, CompletionItemKind::CLASS)
                .detail(detail)
                .sort(sort)
        })
        .collect()
}

/// Returns `key=` candidates for the instance parameters of an element.
///
/// # Arguments
/// * `spec` - element type
pub fn element_params(spec: &ElementSpec) -> Vec<Candidate> {
    spec.params
        .iter()
        .map(|param| {
            Candidate::new(format!("{}=", param.name), CompletionItemKind::PROPERTY)
                .documentation(param.description)
                .detail(param.description)
                .sort("6")
        })
        .collect()
}

/// Returns `key=` candidates for header parameters of a subcircuit plus `m=`.
///
/// # Arguments
/// * `env` - cursor environment
/// * `subckt` - lowercase subcircuit name
pub fn subckt_params(env: Env, subckt: &str) -> Vec<Candidate> {
    let analysis = &env.document.analysis;
    let params: Vec<Candidate> = analysis
        .lookup(SymbolKind::Subckt, env.scope, subckt)
        .map(|id| &analysis.symbols[id].detail)
        .into_iter()
        .flat_map(|detail| match detail {
            SymbolDetail::Subckt { params, .. } => params.clone(),
            _ => Vec::new(),
        })
        .map(|param| {
            let default = param.value.as_ref().map_or(String::new(), |_| {
                env.document
                    .text
                    .get(param.range.start..param.range.end)
                    .and_then(|text| text.split_once('='))
                    .map_or(String::new(), |(_, value)| value.trim().to_string())
            });
            Candidate::new(
                format!("{}=", param.name.text),
                CompletionItemKind::PROPERTY,
            )
            .detail(format!("default {default}"))
            .sort("0")
        })
        .collect();
    params
        .into_iter()
        .chain(std::iter::once(
            Candidate::new("m=", CompletionItemKind::PROPERTY)
                .detail("parallel multiplier")
                .sort("1"),
        ))
        .collect()
}

/// Returns the keywords of independent sources (`DC`, `AC` and the transient functions) as snippets.
pub fn source_keywords() -> Vec<Candidate> {
    let functions = SOURCE_FUNCTIONS.iter().map(|spec| {
        let arguments = spec
            .arguments
            .iter()
            .enumerate()
            .map(|(index, name)| format!("${{{}:{name}}}", index + 1))
            .collect::<Vec<_>>()
            .join(" ");
        let signature = format!(
            "{}({})",
            spec.name.to_ascii_uppercase(),
            spec.arguments.join(" ")
        );
        Candidate::new(spec.name.to_ascii_uppercase(), CompletionItemKind::FUNCTION)
            .snippet(format!("{}({arguments})", spec.name.to_ascii_uppercase()))
            .detail(signature)
            .documentation(spec.description)
            .filter(
                std::iter::once(spec.name)
                    .chain(spec.aliases.iter().copied())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .sort("1")
    });
    let levels = [
        Candidate::new("DC", CompletionItemKind::KEYWORD)
            .snippet("DC ${1:0}")
            .detail("DC value")
            .documentation("DC source value (`DC=value` or `DC value`).")
            .sort("0"),
        Candidate::new("AC", CompletionItemKind::KEYWORD)
            .snippet("AC ${1:1} ${2:0}")
            .detail("AC magnitude [phase]")
            .documentation("Small-signal AC magnitude and optional phase in degrees.")
            .sort("0"),
    ];
    levels.into_iter().chain(functions).collect()
}

/// Returns parameters, measures, user functions, built-in functions and special variables.
///
/// # Arguments
/// * `env` - cursor environment
pub fn expression_names(env: Env) -> Vec<Candidate> {
    let analysis = &env.document.analysis;
    let symbols = |kind: SymbolKind| {
        analysis
            .visible_symbols(kind, env.scope)
            .into_iter()
            .map(|id| &analysis.symbols[id])
            .collect::<Vec<_>>()
    };
    let params = symbols(SymbolKind::Param)
        .into_iter()
        .chain(symbols(SymbolKind::Measure))
        .map(|symbol| {
            let text = first_line(definition_text(env.document, symbol))
                .trim()
                .to_string();
            Candidate::new(symbol.name.text.clone(), CompletionItemKind::VARIABLE)
                .detail(if text.is_empty() {
                    "parameter".to_string()
                } else {
                    text.clone()
                })
                .documentation(format!("```spice\n{text}\n```"))
                .sort("0")
        });
    let user_functions = symbols(SymbolKind::Function).into_iter().map(|symbol| {
        let arguments: Vec<&str> = match &symbol.detail {
            SymbolDetail::Function { arguments, .. } => arguments
                .iter()
                .map(|argument| argument.text.as_str())
                .collect(),
            _ => Vec::new(),
        };
        let placeholders = arguments
            .iter()
            .enumerate()
            .map(|(index, name)| format!("${{{}:{}}}", index + 1, escape_snippet(name)))
            .collect::<Vec<_>>()
            .join(", ");
        Candidate::new(symbol.name.text.clone(), CompletionItemKind::FUNCTION)
            .snippet(format!(
                "{}({placeholders})",
                escape_snippet(&symbol.name.text)
            ))
            .detail(format!("{}({})", symbol.name.text, arguments.join(", ")))
            .documentation(format!(
                "```spice\n{}\n```",
                first_line(definition_text(env.document, symbol))
            ))
            .sort("0")
    });
    params
        .chain(user_functions)
        .chain(builtin_functions())
        .chain(special_variables())
        .collect()
}

/// Returns the built-in math functions as snippets.
pub fn builtin_functions() -> Vec<Candidate> {
    FUNCTIONS
        .iter()
        .map(|spec| {
            let placeholders = spec
                .parameters
                .iter()
                .enumerate()
                .map(|(index, name)| format!("${{{}:{name}}}", index + 1))
                .collect::<Vec<_>>()
                .join(", ");
            Candidate::new(spec.name, CompletionItemKind::FUNCTION)
                .snippet(format!("{}({placeholders})", spec.name))
                .detail(spec.signature)
                .documentation(spec.description)
                .filter(
                    std::iter::once(spec.name)
                        .chain(spec.aliases.iter().copied())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
                .sort("3")
        })
        .collect()
}

/// Returns the predefined variables (`time`, `temper`, `hertz`).
pub fn special_variables() -> Vec<Candidate> {
    SPECIAL_VARIABLES
        .iter()
        .filter(|variable| !variable.redefinable)
        .map(|variable| {
            Candidate::new(variable.name, CompletionItemKind::CONSTANT)
                .detail("predefined variable")
                .documentation(variable.description)
                .sort("4")
        })
        .collect()
}

/// Returns `.option` names; `name=` is inserted for options that need a value.
///
/// # Arguments
/// * `prefix_given` - true if the user typed a prefix (otherwise only common options are listed)
pub fn options(prefix_given: bool) -> Vec<Candidate> {
    OPTIONS
        .iter()
        .filter(|spec| spec.status != Status::Foreign)
        .filter(|spec| prefix_given || spec.usage == Usage::Common)
        .map(|spec| {
            let candidate = Candidate::new(spec.name, CompletionItemKind::PROPERTY)
                .detail(spec.default.map_or_else(
                    || "option".to_string(),
                    |default| format!("default {default}"),
                ))
                .documentation(spec.description)
                .filter(
                    std::iter::once(spec.name)
                        .chain(spec.aliases.iter().copied())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
                .sort(usage_sort(spec.usage, spec.status))
                .deprecated(spec.status == Status::Obsolete);
            match spec.value {
                OptionValue::Value => candidate.snippet(format!("{}=${{1}}", spec.name)),
                OptionValue::Flag | OptionValue::FlagOrValue => candidate,
            }
        })
        .collect()
}

/// Returns the `.model` types.
pub fn model_types() -> Vec<Candidate> {
    MODEL_TYPES
        .iter()
        .filter(|spec| spec.status != Status::Foreign)
        .map(|spec| {
            Candidate::new(spec.name, CompletionItemKind::CLASS)
                .detail(spec.description)
                .sort(if spec.status == Status::Obsolete {
                    "8"
                } else {
                    "0"
                })
                .deprecated(spec.status == Status::Obsolete)
        })
        .collect()
}

/// Returns `key=` candidates for model card parameters.
///
/// # Arguments
/// * `params` - tabulated model parameters
/// * `used` - lowercase keys that are already present
pub fn model_params(params: &[ModelParamSpec], used: &HashSet<String>) -> Vec<Candidate> {
    params
        .iter()
        .filter(|param| !used.contains(param.name))
        .map(|param| {
            Candidate::new(format!("{}=", param.name), CompletionItemKind::PROPERTY)
                .detail(param.default.map_or_else(
                    || param.description.to_string(),
                    |default| format!("default {default}"),
                ))
                .documentation(param.description)
                .sort("1")
        })
        .collect()
}

/// Turns a keyword table into candidates (`key=` for keywords that take values).
///
/// # Arguments
/// * `table` - keyword table
/// * `kind` - LSP item kind
/// * `sort` - sort key
pub fn keywords(table: &[KeywordSpec], kind: CompletionItemKind, sort: &str) -> Vec<Candidate> {
    table
        .iter()
        .map(|spec| {
            let label = if spec.takes_value {
                format!("{}=", spec.name)
            } else {
                spec.name.to_string()
            };
            Candidate::new(label, kind)
                .detail(spec.description)
                .documentation(spec.description)
                .filter(
                    std::iter::once(spec.name)
                        .chain(spec.aliases.iter().copied())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
                .sort(sort)
        })
        .collect()
}

/// Returns the keywords of analysis commands (`DEC`, `LIN`, `UIC`, `START=` …).
pub fn analysis_keywords() -> Vec<Candidate> {
    keywords(ANALYSIS_KEYWORDS, CompletionItemKind::KEYWORD, "3")
}

/// Returns the result types of `.print`, `.probe` and `.meas` (`tran`, `ac`, `dc` …).
pub fn analysis_types() -> Vec<Candidate> {
    keywords(ANALYSIS_TYPES, CompletionItemKind::ENUM_MEMBER, "0")
}

/// Returns the `.meas` keywords (`trig`, `targ`, `val=`, `rise=` …).
pub fn measure_keywords() -> Vec<Candidate> {
    keywords(MEASURE_KEYWORDS, CompletionItemKind::KEYWORD, "1")
}

/// Returns output variables such as `v(`, `i(` as call snippets.
pub fn output_variables() -> Vec<Candidate> {
    OUTPUT_VARS
        .iter()
        .map(|spec| {
            Candidate::new(format!("{}(", spec.name), CompletionItemKind::FUNCTION)
                .snippet(format!("{}(${{1}})", spec.name))
                .detail(spec.signature)
                .documentation(spec.description)
                .filter(
                    std::iter::once(spec.name)
                        .chain(spec.aliases.iter().copied())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
                .sort("2")
        })
        .collect()
}

/// Returns element names visible in a scope, optionally restricted to a first letter.
///
/// # Arguments
/// * `env` - cursor environment
/// * `letters` - accepted lowercase first letters (empty accepts all)
/// * `sort` - sort key
pub fn element_names(env: Env, letters: &[char], sort: &str) -> Vec<Candidate> {
    let analysis = &env.document.analysis;
    analysis
        .visible_symbols(SymbolKind::Element, env.scope)
        .into_iter()
        .map(|id| &analysis.symbols[id])
        .filter(|symbol| {
            letters.is_empty()
                || symbol
                    .name
                    .key
                    .chars()
                    .next()
                    .is_some_and(|letter| letters.contains(&letter))
        })
        .map(|symbol| {
            Candidate::new(symbol.name.text.clone(), CompletionItemKind::VARIABLE)
                .detail(
                    first_line(definition_text(env.document, symbol))
                        .trim()
                        .to_string(),
                )
                .sort(sort)
        })
        .collect()
}

/// Returns voltage/current sources and parameters (sweep variables of `.dc`).
///
/// # Arguments
/// * `env` - cursor environment
pub fn sweep_variables(env: Env) -> Vec<Candidate> {
    let analysis = &env.document.analysis;
    let params = analysis
        .visible_symbols(SymbolKind::Param, env.scope)
        .into_iter()
        .map(|id| {
            Candidate::new(
                analysis.symbols[id].name.text.clone(),
                CompletionItemKind::VARIABLE,
            )
            .detail("parameter")
            .sort("1")
        });
    element_names(env, &['v', 'i'], "0")
        .into_iter()
        .chain(params)
        .collect()
}

/// Returns the directories searched for a relative include path.
///
/// # Arguments
/// * `env` - cursor environment
fn base_directory(env: Env) -> Option<PathBuf> {
    env.document
        .directory()
        .map(Path::to_path_buf)
        .or_else(|| env.workspace.root.clone())
}

/// Lists files and directories for the partially typed path.
///
/// # Arguments
/// * `env` - cursor environment
/// * `typed` - the path typed so far (without quote)
///
/// # Returns
/// Candidates and the number of bytes of the last path segment that the edit replaces.
pub fn paths(env: Env, typed: &str) -> (Vec<Candidate>, usize) {
    let separators: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    let (directory_part, name_prefix) = typed
        .rsplit_once(separators)
        .map_or(("", typed), |(head, tail)| (head, tail));
    let base = if typed.starts_with('/') && directory_part.is_empty() {
        Some(PathBuf::from("/"))
    } else {
        base_directory(env).map(|base| base.join(directory_part))
    };
    let entries = base
        .and_then(|directory| std::fs::read_dir(directory).ok())
        .map(|reader| reader.filter_map(Result::ok).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut candidates: Vec<Candidate> = entries
        .iter()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().ok()?.is_dir();
            let listed = is_dir
                || Path::new(&name)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        NETLIST_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
                    });
            (!name.starts_with('.') && listed).then(|| {
                if is_dir {
                    Candidate::new(name.clone(), CompletionItemKind::FOLDER)
                        .insert(format!("{name}/"))
                        .sort("0")
                } else {
                    Candidate::new(name, CompletionItemKind::FILE).sort("1")
                }
            })
        })
        .collect();
    candidates.sort_by(|left, right| (&left.sort, &left.label).cmp(&(&right.sort, &right.label)));
    (candidates, name_prefix.len())
}

/// Returns the section names defined in the library file named by a `.lib` path.
///
/// # Arguments
/// * `env` - cursor environment
/// * `raw_path` - path as written (without quotes)
pub fn lib_sections(env: Env, raw_path: &str) -> Vec<Candidate> {
    let search: Vec<PathBuf> = env
        .workspace
        .root
        .iter()
        .cloned()
        .chain(
            env.workspace
                .settings
                .include_paths
                .iter()
                .map(PathBuf::from),
        )
        .collect();
    let Some(base) = base_directory(env) else {
        return Vec::new();
    };
    let Some(text) =
        resolve_include_path(raw_path, &base, &search).and_then(|path| read_source(&path).ok())
    else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    text.lines()
        .filter_map(section_definition)
        .filter(|name| seen.insert(name.to_ascii_lowercase()))
        .map(|name| {
            Candidate::new(name, CompletionItemKind::MODULE)
                .detail("library section")
                .sort("0")
        })
        .collect()
}

/// Returns the section name if the line is a `.lib name` definition.
///
/// # Arguments
/// * `line` - one line of a library file
fn section_definition(line: &str) -> Option<&str> {
    let mut words = line.split_whitespace();
    let keyword = words.next()?;
    let name = words.next()?;
    (keyword.eq_ignore_ascii_case(".lib") && words.next().is_none()).then_some(name)
}
