//! Open documents, the include-summary cache and cross-file context resolution.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ls_types::Uri;

use crate::analysis::diagnostics::compute;
use crate::analysis::includes::{
    FileSummary, IncludeDirective, IncludeKind, expand_path, include_directives,
    resolve_include_path, summarize,
};
use crate::analysis::symbols::Symbol;
use crate::analysis::{ExternalContext, IncludeError, IncludeResolution, analyze};
use crate::config::Settings;
use crate::state::document::{Document, SyntaxInfo, parse_document};
use crate::state::line_index::{LineIndex, PositionEncoding};
use crate::syntax::ast::{Arg, AssignKey, CommandKind, Netlist, StatementKind, Value};
use crate::syntax::parse::has_end_statement;

/// Maximum include nesting depth.
const MAX_INCLUDE_DEPTH: usize = 16;

/// Maximum directory depth of the workspace scan.
const MAX_SCAN_DEPTH: usize = 4;

/// File extensions of netlists that may include other files.
const NETLIST_EXTENSIONS: &[&str] = &["sp", "spice", "cir", "inc"];

/// File extensions whose first line HSPICE treats as a title when run directly.
const TOP_LEVEL_EXTENSIONS: &[&str] = &["sp", "spice", "cir"];

/// Identifies the content a cached summary was built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stamp {
    /// File on disk with modification time and length.
    Disk(Option<SystemTime>, u64),
    /// Open document with its version.
    Open(i32),
}

/// A cached file summary.
#[derive(Debug, Clone)]
struct CacheEntry {
    stamp: Stamp,
    summary: Arc<FileSummary>,
}

/// Definitions collected by following include directives.
#[derive(Debug, Default)]
struct Closure {
    symbols: Vec<Symbol>,
    visited: HashSet<(PathBuf, Option<String>)>,
    complete: bool,
}

/// All server-side state that features read from.
#[derive(Debug)]
pub struct Workspace {
    /// Workspace root directory, if the client sent one.
    pub root: Option<PathBuf>,
    /// Negotiated position encoding.
    pub encoding: PositionEncoding,
    /// Current settings.
    pub settings: Settings,
    documents: HashMap<String, Document>,
    cache: HashMap<PathBuf, CacheEntry>,
    dependencies: HashMap<String, HashSet<PathBuf>>,
    scanned_netlists: Option<Vec<PathBuf>>,
    includer_index: Option<HashMap<PathBuf, Vec<PathBuf>>>,
}

impl Workspace {
    /// Creates an empty workspace.
    ///
    /// # Arguments
    /// * `root` - workspace root directory
    /// * `encoding` - negotiated position encoding
    /// * `settings` - initial settings
    pub fn new(root: Option<PathBuf>, encoding: PositionEncoding, settings: Settings) -> Self {
        Self {
            root,
            encoding,
            settings,
            documents: HashMap::new(),
            cache: HashMap::new(),
            dependencies: HashMap::new(),
            scanned_netlists: None,
            includer_index: None,
        }
    }

    /// Returns an open document.
    ///
    /// # Arguments
    /// * `uri` - document URI
    pub fn document(&self, uri: &Uri) -> Option<&Document> {
        self.documents.get(uri.as_str())
    }

    /// Iterates over all open documents.
    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.documents.values()
    }

    /// Opens or replaces a document and re-analyses it and its open dependents.
    ///
    /// # Arguments
    /// * `uri` - document URI
    /// * `version` - client version
    /// * `text` - full text
    ///
    /// # Returns
    /// URIs whose diagnostics changed.
    pub fn update(&mut self, uri: Uri, version: i32, text: String) -> Vec<Uri> {
        let path = uri.to_file_path().map(|path| path.into_owned());
        if self.document(&uri).is_none() {
            self.includer_index = None;
        }
        if let Some(path) = &path {
            self.cache.remove(path);
        }
        let document = self.build_document(uri.clone(), path.clone(), version, text);
        self.documents.insert(uri.as_str().to_string(), document);
        std::iter::once(uri)
            .chain(self.reanalyze_dependents(path.as_deref()))
            .collect()
    }

    /// Closes a document; it stays on disk and in the include cache.
    ///
    /// # Arguments
    /// * `uri` - document URI
    ///
    /// # Returns
    /// URIs of dependents whose diagnostics changed.
    pub fn close(&mut self, uri: &Uri) -> Vec<Uri> {
        let removed = self.documents.remove(uri.as_str());
        self.dependencies.remove(uri.as_str());
        self.includer_index = None;
        let path = removed.and_then(|document| document.path);
        if let Some(path) = &path {
            self.cache.remove(path);
        }
        self.reanalyze_dependents(path.as_deref())
    }

    /// Invalidates cached data of a saved file and re-analyses its dependents.
    ///
    /// # Arguments
    /// * `uri` - saved document URI
    ///
    /// # Returns
    /// URIs whose diagnostics changed.
    pub fn saved(&mut self, uri: &Uri) -> Vec<Uri> {
        self.includer_index = None;
        let path = uri.to_file_path().map(|path| path.into_owned());
        if let Some(path) = &path {
            self.cache.remove(path);
        }
        let own = self
            .document(uri)
            .map(|document| (document.version, document.text.clone()));
        match own {
            Some((version, text)) => self.update(uri.clone(), version, text),
            None => self.reanalyze_dependents(path.as_deref()),
        }
    }

    /// Replaces the settings and re-analyses every open document.
    ///
    /// # Arguments
    /// * `settings` - new settings
    ///
    /// # Returns
    /// URIs of all open documents.
    pub fn set_settings(&mut self, settings: Settings) -> Vec<Uri> {
        self.settings = settings;
        self.cache.clear();
        self.scanned_netlists = None;
        self.includer_index = None;
        self.reanalyze_all()
    }

    /// Re-analyses all open documents.
    ///
    /// # Returns
    /// URIs of all open documents.
    pub fn reanalyze_all(&mut self) -> Vec<Uri> {
        let keys: Vec<String> = self.documents.keys().cloned().collect();
        keys.iter().filter_map(|key| self.reanalyze(key)).collect()
    }

    /// Re-analyses open documents that depend on `path`.
    ///
    /// # Arguments
    /// * `path` - changed file
    fn reanalyze_dependents(&mut self, path: Option<&Path>) -> Vec<Uri> {
        let Some(path) = path else {
            return Vec::new();
        };
        let dependents: Vec<String> = self
            .dependencies
            .iter()
            .filter(|(key, paths)| {
                paths.contains(path)
                    && self
                        .documents
                        .get(*key)
                        .and_then(|document| document.path.as_deref())
                        != Some(path)
            })
            .map(|(key, _)| key.clone())
            .collect();
        dependents
            .iter()
            .filter_map(|key| self.reanalyze(key))
            .collect()
    }

    /// Rebuilds one open document from its current text.
    ///
    /// # Arguments
    /// * `key` - document URI string
    fn reanalyze(&mut self, key: &str) -> Option<Uri> {
        let document = self.documents.get(key)?;
        let (uri, path, version, text) = (
            document.uri.clone(),
            document.path.clone(),
            document.version,
            document.text.clone(),
        );
        let rebuilt = self.build_document(uri.clone(), path, version, text);
        self.documents.insert(key.to_string(), rebuilt);
        Some(uri)
    }

    /// Parses, analyses and diagnoses a document.
    ///
    /// # Arguments
    /// * `uri` - document URI
    /// * `path` - file path, if any
    /// * `version` - client version
    /// * `text` - full text
    fn build_document(
        &mut self,
        uri: Uri,
        path: Option<PathBuf>,
        version: i32,
        text: String,
    ) -> Document {
        let is_top_level = self.is_top_level(path.as_deref(), &text);
        let syntax = parse_document(&text, is_top_level);
        let (external, dependencies) = self.external_context(path.as_deref(), &syntax);
        self.dependencies
            .insert(uri.as_str().to_string(), dependencies);
        let analysis = analyze(&syntax.netlist, external);
        let diagnostics = compute(&syntax, &analysis, &self.settings.diagnostics, &text);
        Document {
            uri,
            path,
            version,
            line_index: LineIndex::new(&text),
            text,
            syntax,
            analysis,
            diagnostics,
        }
    }

    /// Decides whether HSPICE treats a document as a top-level netlist (first line = title).
    ///
    /// # Arguments
    /// * `path` - file path, if any
    /// * `text` - document text
    fn is_top_level(&mut self, path: Option<&Path>, text: &str) -> bool {
        if has_end_statement(text) {
            return true;
        }
        let Some(path) = path else {
            return false;
        };
        let netlist_extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                TOP_LEVEL_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
            });
        netlist_extension && self.includers_of(path).is_empty()
    }

    /// Collects definitions from includes (and, for include files, from their includers).
    ///
    /// # Arguments
    /// * `path` - path of the analysed document
    /// * `syntax` - parse results of the analysed document
    ///
    /// # Returns
    /// The external context and the set of files the document depends on.
    fn external_context(
        &mut self,
        path: Option<&Path>,
        syntax: &SyntaxInfo,
    ) -> (ExternalContext, HashSet<PathBuf>) {
        let base_dir = path
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| self.root.clone())
            .unwrap_or_default();
        let search_dirs = self.search_dirs(&syntax.netlist, &base_dir);
        let sections = crate::analysis::blocks::build_structure(&syntax.netlist).statement_section;
        let directives = include_directives(&syntax.netlist, &sections);
        let mut closure = Closure {
            complete: true,
            ..Closure::default()
        };
        if let Some(path) = path {
            closure.visited.insert((path.to_path_buf(), None));
        }
        let includes: Vec<IncludeResolution> = directives
            .into_iter()
            .map(|directive| {
                self.follow_own_directive(directive, path, &base_dir, &search_dirs, &mut closure)
            })
            .collect();
        if let Some(path) = path.filter(|_| !syntax.is_top_level) {
            let found = self.add_includer_context(path, &search_dirs, &mut closure);
            closure.complete &= found;
        }
        let dependencies = closure
            .visited
            .iter()
            .map(|(visited, _)| visited.clone())
            .collect();
        let context = ExternalContext {
            symbols: closure.symbols,
            includes,
            complete: closure.complete,
        };
        (context, dependencies)
    }

    /// Follows one include directive of the analysed document and records the outcome.
    ///
    /// # Arguments
    /// * `directive` - the directive
    /// * `own_path` - path of the analysed document (for cycle detection)
    /// * `base_dir` - directory of the analysed document
    /// * `search_dirs` - search directories
    /// * `closure` - accumulated definitions
    fn follow_own_directive(
        &mut self,
        directive: IncludeDirective,
        own_path: Option<&Path>,
        base_dir: &Path,
        search_dirs: &[PathBuf],
        closure: &mut Closure,
    ) -> IncludeResolution {
        let Some(resolved) = resolve_include_path(&directive.path.text, base_dir, search_dirs)
        else {
            closure.complete = false;
            return IncludeResolution {
                directive,
                resolved: None,
                error: Some(IncludeError::NotFound),
            };
        };
        if own_path.is_some_and(|own| same_file(own, &resolved)) {
            return IncludeResolution {
                directive,
                resolved: Some(resolved),
                error: Some(IncludeError::Cycle),
            };
        }
        let summary = match self.summary(&resolved) {
            Ok(summary) => summary,
            Err(error) => {
                closure.complete = false;
                return IncludeResolution {
                    directive,
                    resolved: Some(resolved),
                    error: Some(error),
                };
            }
        };
        let section = directive
            .section
            .as_ref()
            .map(|section| section.key.clone());
        if directive.kind == IncludeKind::Lib
            && section
                .as_deref()
                .is_some_and(|section| !summary.has_section(section))
        {
            return IncludeResolution {
                directive,
                resolved: Some(resolved),
                error: Some(IncludeError::SectionNotFound),
            };
        }
        self.collect_closure(&summary, section.as_deref(), search_dirs, 1, closure);
        IncludeResolution {
            directive,
            resolved: Some(resolved),
            error: None,
        }
    }

    /// Adds the definitions of a summary (one section) and of everything it includes.
    ///
    /// # Arguments
    /// * `summary` - file summary
    /// * `section` - `.lib` section, or `None` for a plain include
    /// * `search_dirs` - search directories
    /// * `depth` - current nesting depth
    /// * `closure` - accumulated definitions
    fn collect_closure(
        &mut self,
        summary: &Arc<FileSummary>,
        section: Option<&str>,
        search_dirs: &[PathBuf],
        depth: usize,
        closure: &mut Closure,
    ) {
        let key = (summary.file.path.clone(), section.map(str::to_string));
        if !closure.visited.insert(key) {
            return;
        }
        if depth > MAX_INCLUDE_DEPTH {
            closure.complete = false;
            return;
        }
        closure.symbols.extend(summary.symbols_in(section).cloned());
        let base_dir = summary
            .file
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let nested: Vec<IncludeDirective> = summary.includes_in(section).cloned().collect();
        nested.iter().for_each(|directive| {
            let resolved = resolve_include_path(&directive.path.text, &base_dir, search_dirs);
            match resolved.map(|path| self.summary(&path)) {
                Some(Ok(nested_summary)) => {
                    let nested_section = directive
                        .section
                        .as_ref()
                        .map(|section| section.key.clone());
                    self.collect_closure(
                        &nested_summary,
                        nested_section.as_deref(),
                        search_dirs,
                        depth + 1,
                        closure,
                    );
                }
                _ => closure.complete = false,
            }
        });
    }

    /// Adds definitions of netlists that include `path` (transitively), so include files see their context.
    ///
    /// # Arguments
    /// * `path` - include file being analysed
    /// * `search_dirs` - search directories
    /// * `closure` - accumulated definitions
    ///
    /// # Returns
    /// True if at least one includer was found.
    fn add_includer_context(
        &mut self,
        path: &Path,
        search_dirs: &[PathBuf],
        closure: &mut Closure,
    ) -> bool {
        let mut pending = vec![path.to_path_buf()];
        let mut seen: HashSet<PathBuf> = HashSet::from([path.to_path_buf()]);
        let mut found = false;
        while let Some(current) = pending.pop() {
            let includers: Vec<PathBuf> = self
                .includers_of(&current)
                .into_iter()
                .filter(|includer| seen.insert(includer.clone()))
                .collect();
            includers.iter().for_each(|includer| {
                if let Ok(summary) = self.summary(includer) {
                    found = true;
                    self.collect_closure(&summary, None, search_dirs, 1, closure);
                }
            });
            pending.extend(includers);
        }
        found
    }

    /// Returns files that directly include `path` (open documents and scanned workspace netlists).
    ///
    /// # Arguments
    /// * `path` - included file
    fn includers_of(&mut self, path: &Path) -> Vec<PathBuf> {
        if self.includer_index.is_none() {
            let index = self.build_includer_index();
            self.includer_index = Some(index);
        }
        self.includer_index
            .as_ref()
            .and_then(|index| index.get(&canonical(path)))
            .cloned()
            .unwrap_or_default()
    }

    /// Builds the map from included file to the files that include it.
    fn build_includer_index(&mut self) -> HashMap<PathBuf, Vec<PathBuf>> {
        let open: Vec<PathBuf> = self
            .documents
            .values()
            .filter_map(|document| document.path.clone())
            .collect();
        let scanned = self.scanned_netlists().to_vec();
        let candidates: Vec<PathBuf> = open
            .into_iter()
            .chain(scanned)
            .collect::<HashSet<PathBuf>>()
            .into_iter()
            .collect();
        let root_dirs: Vec<PathBuf> = self
            .root
            .iter()
            .cloned()
            .chain(self.configured_include_dirs())
            .collect();
        candidates
            .iter()
            .fold(HashMap::new(), |mut index, candidate| {
                let Ok(summary) = self.summary(candidate) else {
                    return index;
                };
                let base_dir = candidate
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default();
                summary
                    .includes
                    .iter()
                    .filter_map(|directive| {
                        resolve_include_path(&directive.path.text, &base_dir, &root_dirs)
                    })
                    .for_each(|included| {
                        index
                            .entry(canonical(&included))
                            .or_insert_with(Vec::new)
                            .push(candidate.clone())
                    });
                index
            })
    }

    /// Returns the netlists found by the (lazy, bounded) workspace scan.
    fn scanned_netlists(&mut self) -> &[PathBuf] {
        if self.scanned_netlists.is_none() {
            let files = match (&self.root, self.settings.workspace_scan.enabled) {
                (Some(root), true) => scan_netlists(root, self.settings.workspace_scan.max_files),
                _ => Vec::new(),
            };
            self.scanned_netlists = Some(files);
        }
        self.scanned_netlists.as_deref().unwrap_or_default()
    }

    /// Returns the summary of a file, preferring the text of an open document.
    ///
    /// # Arguments
    /// * `path` - absolute file path
    fn summary(&mut self, path: &Path) -> Result<Arc<FileSummary>, IncludeError> {
        let open = self
            .documents
            .values()
            .find(|document| {
                document
                    .path
                    .as_deref()
                    .is_some_and(|open_path| same_file(open_path, path))
            })
            .map(|document| (Stamp::Open(document.version), document.text.clone()));
        let (stamp, text) = match open {
            Some((stamp, text)) => (stamp, Some(text)),
            None => {
                let metadata = std::fs::metadata(path).map_err(|_| IncludeError::NotFound)?;
                if metadata.len()
                    > self
                        .settings
                        .max_include_file_size_mb
                        .saturating_mul(1024 * 1024)
                {
                    return Err(IncludeError::TooLarge);
                }
                (Stamp::Disk(metadata.modified().ok(), metadata.len()), None)
            }
        };
        if let Some(entry) = self.cache.get(path).filter(|entry| entry.stamp == stamp) {
            return Ok(Arc::clone(&entry.summary));
        }
        let text = match text {
            Some(text) => text,
            None => {
                let bytes = std::fs::read(path).map_err(|_| IncludeError::NotFound)?;
                String::from_utf8_lossy(&bytes).into_owned()
            }
        };
        let summary = Arc::new(summarize(path, text));
        self.cache.insert(
            path.to_path_buf(),
            CacheEntry {
                stamp,
                summary: Arc::clone(&summary),
            },
        );
        Ok(summary)
    }

    /// Returns the include search directories: workspace root, `.option search=` and `includePaths`.
    ///
    /// # Arguments
    /// * `netlist` - analysed document (for `.option search`)
    /// * `base_dir` - directory of the analysed document
    fn search_dirs(&self, netlist: &Netlist, base_dir: &Path) -> Vec<PathBuf> {
        let option_dirs = option_search_dirs(netlist).into_iter().map(|dir| {
            let expanded = PathBuf::from(expand_path(&dir, &|name| std::env::var(name).ok()));
            if expanded.is_absolute() {
                expanded
            } else {
                base_dir.join(expanded)
            }
        });
        self.root
            .iter()
            .cloned()
            .chain(option_dirs)
            .chain(self.configured_include_dirs())
            .collect()
    }

    /// Returns the `includePaths` setting as absolute directories (relative ones are taken from the root).
    fn configured_include_dirs(&self) -> Vec<PathBuf> {
        self.settings
            .include_paths
            .iter()
            .map(|dir| PathBuf::from(expand_path(dir, &|name| std::env::var(name).ok())))
            .map(|dir| match (&self.root, dir.is_absolute()) {
                (Some(root), false) => root.join(dir),
                _ => dir,
            })
            .collect()
    }
}

/// Returns the directories given by `.option search='dir'` in a netlist.
///
/// # Arguments
/// * `netlist` - lowered document
fn option_search_dirs(netlist: &Netlist) -> Vec<String> {
    netlist
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            StatementKind::Command(command) => match &command.kind {
                CommandKind::Option { args } => Some(args),
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .filter_map(|arg| match arg {
            Arg::Assign(assignment) => match (&assignment.key, &assignment.value) {
                (AssignKey::Name(key), Some(Value::Str(path))) if key.key == "search" => {
                    Some(path.text.clone())
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Returns the canonical form of a path, or the path itself if it cannot be canonicalised.
///
/// # Arguments
/// * `path` - any path
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Returns true if two paths name the same file.
///
/// # Arguments
/// * `left` - first path
/// * `right` - second path
fn same_file(left: &Path, right: &Path) -> bool {
    left == right || canonical(left) == canonical(right)
}

/// Finds netlist files below `root` (hidden directories skipped), up to `max_files`.
///
/// # Arguments
/// * `root` - workspace root
/// * `max_files` - maximum number of files to return
pub fn scan_netlists(root: &Path, max_files: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((directory, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if hidden {
                continue;
            }
            if path.is_dir() && depth < MAX_SCAN_DEPTH {
                pending.push((path, depth + 1));
            } else if is_netlist_file(&path) {
                found.push(path);
                if found.len() >= max_files {
                    return found;
                }
            }
        }
    }
    found
}

/// Returns true if a path has a netlist extension.
///
/// # Arguments
/// * `path` - file path
fn is_netlist_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            NETLIST_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(relative)
    }

    fn open_fixture(workspace: &mut Workspace, relative: &str) -> Uri {
        let path = fixture(relative);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let uri = Uri::from_file_path(&path).expect("file uri");
        workspace.update(uri.clone(), 1, text);
        uri
    }

    #[test]
    fn testbench_sees_library_section_and_included_subckt() {
        let mut workspace = Workspace::new(
            Some(fixture("")),
            PositionEncoding::Utf16,
            Settings::default(),
        );
        let uri = open_fixture(&mut workspace, "inverter_tb.sp");
        let document = workspace.document(&uri).expect("document");
        assert!(document.syntax.is_top_level);
        assert!(
            document
                .analysis
                .includes
                .iter()
                .all(|include| include.error.is_none()),
            "{:?}",
            document.analysis.includes
        );
        let unresolved: Vec<&str> = document
            .analysis
            .references
            .iter()
            .filter(|reference| reference.target.is_none())
            .map(|reference| reference.name.text.as_str())
            .collect();
        assert!(unresolved.is_empty(), "unresolved: {unresolved:?}");
    }

    #[test]
    fn include_file_sees_models_through_its_includer() {
        let mut workspace = Workspace::new(
            Some(fixture("")),
            PositionEncoding::Utf16,
            Settings::default(),
        );
        let uri = open_fixture(&mut workspace, "cells/inv.inc");
        let document = workspace.document(&uri).expect("document");
        assert!(!document.syntax.is_top_level);
        assert!(document.analysis.context_complete);
        let models = document
            .analysis
            .references
            .iter()
            .filter(|reference| reference.kind == crate::analysis::symbols::SymbolKind::Model);
        assert!(models.clone().count() >= 2);
        assert!(
            models
                .into_iter()
                .all(|reference| reference.target.is_some())
        );
    }

    #[test]
    fn scan_finds_fixture_netlists() {
        let files = scan_netlists(&fixture(""), 100);
        assert!(files.iter().any(|path| path.ends_with("inverter_tb.sp")));
        assert!(files.iter().any(|path| path.ends_with("cells/inv.inc")));
    }
}
