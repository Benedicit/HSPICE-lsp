//! Include directives, include-path resolution and definition summaries of included files.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use super::blocks::build_structure;
use super::resolve::collect_definitions;
use super::symbols::{ExternalFile, Origin, ROOT_SCOPE, Symbol};
use crate::lang::commands::find_command;
use crate::state::line_index::LineIndex;
use crate::syntax::ast::{CommandKind, Ident, Netlist, PathArg, StatementKind, TextRange};
use crate::syntax::lower::lower;
use crate::syntax::parse::{
    has_end_statement, mask_ranges, parse_tree, parse_tree_in_ranges, title_ranges,
};

/// Files larger than this are parsed in definition-only mode.
pub const LARGE_FILE_BYTES: usize = 8 * 1024 * 1024;

/// How a directive pulls in another file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeKind {
    /// `.include` — the whole file outside `.lib` sections.
    Include,
    /// `.lib 'file' section` — one library section.
    Lib,
}

/// An `.include` or `.lib` call found in a file.
#[derive(Debug, Clone, PartialEq)]
pub struct IncludeDirective {
    /// Statement index in the containing file.
    pub statement: usize,
    /// Directive kind.
    pub kind: IncludeKind,
    /// Path as written (without quotes).
    pub path: PathArg,
    /// Section requested by a `.lib` call.
    pub section: Option<Ident>,
    /// `.lib` section of the containing file that holds the directive (lowercase).
    pub enclosing_section: Option<String>,
}

/// Definitions and include directives of a file that is not open in the editor.
#[derive(Debug, Clone)]
pub struct FileSummary {
    /// The file itself (path, text and line index).
    pub file: Arc<ExternalFile>,
    /// Top-level definitions, tagged with their `.lib` section.
    pub symbols: Vec<Symbol>,
    /// Include directives in source order.
    pub includes: Vec<IncludeDirective>,
    /// Lowercase identifier words of the whole file (tells which parameters it reads).
    pub words: Arc<HashSet<String>>,
}

impl FileSummary {
    /// Returns the definitions visible through an `.include` (`None`) or a `.lib` call of `section`.
    ///
    /// # Arguments
    /// * `section` - requested lowercase section, or `None` for a plain include
    pub fn symbols_in(&self, section: Option<&str>) -> impl Iterator<Item = &Symbol> {
        self.symbols
            .iter()
            .filter(move |symbol| symbol.section.as_deref() == section)
    }

    /// Returns the include directives that are active for `.include` (`None`) or a `.lib` call of `section`.
    ///
    /// # Arguments
    /// * `section` - requested lowercase section, or `None` for a plain include
    pub fn includes_in(&self, section: Option<&str>) -> impl Iterator<Item = &IncludeDirective> {
        self.includes
            .iter()
            .filter(move |directive| directive.enclosing_section.as_deref() == section)
    }

    /// Returns true if the file defines a `.lib` section with this lowercase name.
    ///
    /// # Arguments
    /// * `section` - lowercase section name
    pub fn has_section(&self, section: &str) -> bool {
        self.symbols.iter().any(|symbol| {
            symbol.kind == super::symbols::SymbolKind::LibSection && symbol.name.key == section
        })
    }
}

/// Extracts all `.include` and `.lib` call directives of a lowered file.
///
/// # Arguments
/// * `netlist` - lowered file
/// * `sections` - enclosing `.lib` section of each statement
pub fn include_directives(netlist: &Netlist, sections: &[Option<String>]) -> Vec<IncludeDirective> {
    netlist
        .statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            let StatementKind::Command(command) = &statement.kind else {
                return None;
            };
            let (kind, path, section) = match &command.kind {
                CommandKind::Include { path: Some(path) } => {
                    (IncludeKind::Include, path.clone(), None)
                }
                CommandKind::Lib {
                    path: Some(path),
                    section,
                } => (IncludeKind::Lib, path.clone(), section.clone()),
                _ => return None,
            };
            Some(IncludeDirective {
                statement: index,
                kind,
                path,
                section,
                enclosing_section: sections.get(index).cloned().flatten(),
            })
        })
        .collect()
}

/// Builds the definition summary of a file read from disk.
///
/// # Arguments
/// * `path` - absolute file path
/// * `text` - file contents
pub fn summarize(path: &Path, text: String) -> FileSummary {
    let tree = if text.len() > LARGE_FILE_BYTES {
        parse_tree_in_ranges(&text, &definition_ranges(&text))
    } else if has_end_statement(&text) {
        parse_tree(&mask_ranges(&text, &title_ranges(&text)))
    } else {
        parse_tree(&text)
    };
    let netlist = tree.map(|tree| lower(&tree, &text)).unwrap_or_default();
    let structure = build_structure(&netlist);
    let file = Arc::new(ExternalFile {
        path: path.to_path_buf(),
        line_index: LineIndex::new(&text),
        text,
    });
    let symbols = collect_definitions(&netlist, &structure, &Origin::External(Arc::clone(&file)))
        .into_iter()
        .filter(|symbol| symbol.scope == ROOT_SCOPE)
        .map(without_body_scope)
        .collect();
    let includes = include_directives(&netlist, &structure.statement_section);
    let words = Arc::new(identifier_words(&file.text));
    FileSummary {
        file,
        symbols,
        includes,
        words,
    }
}

/// Returns the lowercase identifier-like words of a text.
///
/// # Arguments
/// * `text` - file contents
fn identifier_words(text: &str) -> HashSet<String> {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '#'))
    })
    .filter(|word| word.starts_with(|first: char| first.is_ascii_alphabetic() || first == '_'))
    .map(str::to_ascii_lowercase)
    .collect()
}

/// Drops the body scope of external subckts, since scope ids are only meaningful inside their own file.
///
/// # Arguments
/// * `symbol` - external symbol
fn without_body_scope(mut symbol: Symbol) -> Symbol {
    if let super::symbols::SymbolDetail::Subckt { body, .. } = &mut symbol.detail {
        *body = None;
    }
    symbol
}

/// Returns the byte ranges of definition statements for definition-only parsing of huge files.
///
/// `.model` statements contribute only `.model name type` plus the line break; other definition
/// commands contribute their whole logical line including `+` continuation lines.
///
/// # Arguments
/// * `text` - file contents
pub fn definition_ranges(text: &str) -> Vec<TextRange> {
    const FULL_LINE: &[&str] = &[
        ".subckt", ".ends", ".param", ".lib", ".endl", ".include", ".global", ".end",
    ];
    let lines: Vec<(usize, &str)> = text
        .split_inclusive('\n')
        .scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .collect();
    lines
        .iter()
        .enumerate()
        .flat_map(|(index, (start, line))| {
            let keyword = line
                .split_whitespace()
                .next()
                .and_then(find_command)
                .map_or("", |command| command.name);
            if keyword == ".model" {
                return model_header_ranges(*start, line);
            }
            if !FULL_LINE.contains(&keyword) {
                return Vec::new();
            }
            let continuation_end = lines[index + 1..]
                .iter()
                .take_while(|(_, next)| next.trim_start().starts_with('+'))
                .last()
                .map_or(start + line.len(), |(next_start, next)| {
                    next_start + next.len()
                });
            vec![TextRange::new(*start, continuation_end)]
        })
        .collect()
}

/// Returns the ranges of `.model name type` and the line break of a model line.
///
/// # Arguments
/// * `start` - offset of the line
/// * `line` - line text including its line break
fn model_header_ranges(start: usize, line: &str) -> Vec<TextRange> {
    let header_end = line
        .char_indices()
        .scan((false, 0), |(in_token, tokens), (offset, character)| {
            let is_token = !character.is_whitespace() && character != '(';
            if is_token && !*in_token {
                *tokens += 1;
            }
            *in_token = is_token;
            Some((offset + character.len_utf8(), *tokens, is_token))
        })
        .take_while(|(_, tokens, _)| *tokens <= 3)
        .filter(|(_, _, is_token)| *is_token)
        .last()
        .map_or(0, |(end, _, _)| end);
    let line_break = line.trim_end_matches(['\n', '\r']).len();
    [
        TextRange::new(start, start + header_end),
        TextRange::new(start + line_break, start + line.len()),
    ]
    .into_iter()
    .filter(|range| !range.is_empty())
    .collect()
}

/// Expands `$VAR`, `${VAR}` and a leading `~` in an include path.
///
/// # Arguments
/// * `raw` - path as written
/// * `env` - environment lookup (`std::env::var` in production)
pub fn expand_path(raw: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let with_home = match raw.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => env("HOME")
            .or_else(|| env("USERPROFILE"))
            .map_or_else(|| raw.to_string(), |home| format!("{home}{rest}")),
        _ => raw.to_string(),
    };
    expand_variables(&with_home, env)
}

/// Replaces `$NAME` and `${NAME}` with environment values; unknown variables are kept verbatim.
///
/// # Arguments
/// * `text` - path text
/// * `env` - environment lookup
fn expand_variables(text: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let Some(dollar) = text.find('$') else {
        return text.to_string();
    };
    let (before, after) = text.split_at(dollar);
    let rest = &after[1..];
    let (name, remainder) = match rest.strip_prefix('{') {
        Some(braced) => match braced.find('}') {
            Some(close) => (&braced[..close], &braced[close + 1..]),
            None => ("", rest),
        },
        None => {
            let end = rest
                .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .unwrap_or(rest.len());
            (&rest[..end], &rest[end..])
        }
    };
    let replacement = (!name.is_empty())
        .then(|| env(name))
        .flatten()
        .unwrap_or_else(|| after[..after.len() - remainder.len()].to_string());
    format!("{before}{replacement}{}", expand_variables(remainder, env))
}

/// Resolves an include path: relative to the including file, then the search directories.
///
/// # Arguments
/// * `raw` - path as written
/// * `base_dir` - directory of the including file
/// * `search_dirs` - workspace root, `.option search` and `includePaths` directories in order
pub fn resolve_include_path(
    raw: &str,
    base_dir: &Path,
    search_dirs: &[PathBuf],
) -> Option<PathBuf> {
    let expanded = PathBuf::from(expand_path(raw, &|name| std::env::var(name).ok()));
    if expanded.is_absolute() {
        let path = normalize(&expanded);
        return path.is_file().then_some(path);
    }
    std::iter::once(base_dir)
        .chain(search_dirs.iter().map(PathBuf::as_path))
        .map(|directory| normalize(&directory.join(&expanded)))
        .find(|candidate| candidate.is_file())
}

/// Removes `.` and folds `..` components without touching the file system.
///
/// # Arguments
/// * `path` - path to normalise
fn normalize(path: &Path) -> PathBuf {
    path.components()
        .fold(PathBuf::new(), |mut normalized, component| {
            match component {
                Component::CurDir => {}
                Component::ParentDir
                    if matches!(
                        normalized.components().next_back(),
                        Some(Component::Normal(_))
                    ) =>
                {
                    normalized.pop();
                }
                _ => normalized.push(component),
            }
            normalized
        })
}

/// Reads a netlist from disk, replacing invalid UTF-8 and dropping a leading byte-order mark.
///
/// # Arguments
/// * `path` - file path
pub fn read_source(path: &Path) -> std::io::Result<String> {
    let text = String::from_utf8_lossy(&std::fs::read(path)?).into_owned();
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::symbols::SymbolKind;

    fn env(name: &str) -> Option<String> {
        match name {
            "PDK" => Some("/eda/pdk".into()),
            "HOME" => Some("/home/me".into()),
            _ => None,
        }
    }

    #[test]
    fn expands_environment_variables() {
        assert_eq!(expand_path("$PDK/models.lib", &env), "/eda/pdk/models.lib");
        assert_eq!(expand_path("${PDK}/x/${PDK}", &env), "/eda/pdk/x//eda/pdk");
        assert_eq!(expand_path("~/lib.sp", &env), "/home/me/lib.sp");
        assert_eq!(expand_path("$UNKNOWN/a", &env), "$UNKNOWN/a");
        let windows = |name: &str| (name == "USERPROFILE").then(|| "C:\\Users\\me".to_string());
        assert_eq!(expand_path("~\\lib.sp", &windows), "C:\\Users\\me\\lib.sp");
    }

    #[test]
    fn normalizes_parent_components() {
        assert_eq!(
            normalize(Path::new("/p/tb/../cells/./inv.inc")),
            PathBuf::from("/p/cells/inv.inc")
        );
        assert_eq!(normalize(Path::new("../a/../b")), PathBuf::from("../b"));
    }

    #[test]
    fn read_source_drops_byte_order_mark() {
        let path = std::env::temp_dir().join("hspice-lsp-bom-test.inc");
        std::fs::write(&path, "\u{feff}.subckt a b\n").unwrap_or_default();
        assert_eq!(read_source(&path).ok().as_deref(), Some(".subckt a b\n"));
        std::fs::remove_file(&path).unwrap_or_default();
    }

    #[test]
    fn summary_words_are_lowercase_identifiers() {
        let summary = summarize(
            Path::new("/tmp/t.lib"),
            "* c\n.model n nmos vth0='0.4+Dvth'\n".to_string(),
        );
        assert!(summary.words.contains("dvth"));
        assert!(!summary.words.contains("0.4"));
    }

    #[test]
    fn summary_tags_sections() {
        let text = "* lib\n.lib tt\n.param vth=0.4\n.model nch nmos level=54\n.endl tt\n.lib ff\n.inc 'x.inc'\n.endl ff\n".to_string();
        let summary = summarize(Path::new("/tmp/t.lib"), text);
        let in_tt: Vec<&str> = summary
            .symbols_in(Some("tt"))
            .map(|symbol| symbol.name.key.as_str())
            .collect();
        assert_eq!(in_tt, vec!["vth", "nch"]);
        assert!(summary.has_section("ff"));
        assert_eq!(summary.includes_in(Some("ff")).count(), 1);
        assert!(
            summary
                .symbols
                .iter()
                .any(|symbol| symbol.kind == SymbolKind::LibSection)
        );
    }

    #[test]
    fn definition_ranges_keep_model_headers_only() {
        let text = "* big\n.model nch.1 nmos (level=54\n+ vth0=0.4)\nR1 a b 1\n.subckt s a\n+ b\n";
        let ranges = definition_ranges(text);
        let pieces: Vec<&str> = ranges
            .iter()
            .map(|range| &text[range.start..range.end])
            .collect();
        assert_eq!(
            pieces,
            vec![".model nch.1 nmos", "\n", ".subckt s a\n+ b\n"]
        );
        assert_eq!(definition_ranges(".PARA a=1\n.EOM\n").len(), 2);
    }
}
