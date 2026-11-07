//! Semantic analysis: block structure, symbols, element roles, references and diagnostics.

pub mod blocks;
pub mod diagnostics;
pub mod eval;
pub mod includes;
pub mod resolve;
pub mod roles;
pub mod symbols;

use std::collections::HashMap;
use std::path::PathBuf;

use blocks::{Structure, build_structure};
use includes::IncludeDirective;
use resolve::{ElementInfo, connection_counts, resolve};
use symbols::{
    ROOT_SCOPE, Reference, ScopeId, Symbol, SymbolId, SymbolKind, SymbolTable, scope_chain,
};

use crate::syntax::ast::{Netlist, TextRange};

/// Why an include directive could not be followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeError {
    /// No file at the path (relative to the including file or any search directory).
    NotFound,
    /// The file exists but has no `.lib` section with the requested name.
    SectionNotFound,
    /// The file includes itself (directly or indirectly).
    Cycle,
    /// The file exceeds the configured size limit.
    TooLarge,
}

/// The outcome of following one include directive of the analysed document.
#[derive(Debug, Clone, PartialEq)]
pub struct IncludeResolution {
    /// The directive.
    pub directive: IncludeDirective,
    /// Absolute path of the included file, if found.
    pub resolved: Option<PathBuf>,
    /// Failure reason, if any.
    pub error: Option<IncludeError>,
}

/// Information from other files that the analysis needs.
#[derive(Debug, Clone, Default)]
pub struct ExternalContext {
    /// Definitions visible through includes, libraries and including files.
    pub symbols: Vec<Symbol>,
    /// One resolution per include directive of the document.
    pub includes: Vec<IncludeResolution>,
    /// False if some definitions may be missing (unresolved includes, include file without known includer).
    pub complete: bool,
}

/// Result of analysing one document.
#[derive(Debug, Clone, Default)]
pub struct Analysis {
    /// Block structure and scopes.
    pub structure: Structure,
    /// Local symbols first, then external ones.
    pub symbols: Vec<Symbol>,
    /// Index over `symbols`.
    pub table: SymbolTable,
    /// Element lines with role assignment.
    pub elements: Vec<ElementInfo>,
    /// All references with their resolved targets.
    pub references: Vec<Reference>,
    /// Include directives and how they resolved.
    pub includes: Vec<IncludeResolution>,
    /// False if definitions from other files may be missing.
    pub context_complete: bool,
}

impl Default for Structure {
    fn default() -> Self {
        build_structure(&Netlist::default())
    }
}

/// Analyses a lowered document together with the definitions of other files.
///
/// # Arguments
/// * `netlist` - lowered document
/// * `external` - definitions and include results from other files
pub fn analyze(netlist: &Netlist, external: ExternalContext) -> Analysis {
    let structure = build_structure(netlist);
    let resolution = resolve(netlist, &structure, &external.symbols);
    Analysis {
        structure,
        symbols: resolution.symbols,
        table: resolution.table,
        elements: resolution.elements,
        references: resolution.references,
        includes: external.includes,
        context_complete: external.complete,
    }
}

impl Analysis {
    /// Finds the closest definition of a name visible from `scope`.
    ///
    /// # Arguments
    /// * `kind` - symbol kind
    /// * `scope` - innermost scope
    /// * `key` - lowercase name
    pub fn lookup(&self, kind: SymbolKind, scope: ScopeId, key: &str) -> Option<SymbolId> {
        self.table.lookup(&self.structure.scopes, kind, scope, key)
    }

    /// Returns the innermost scope whose range contains `offset`.
    ///
    /// # Arguments
    /// * `offset` - byte offset
    pub fn scope_at(&self, offset: usize) -> ScopeId {
        self.structure
            .scopes
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, scope)| scope.range.touches(offset))
            .max_by_key(|(_, scope)| scope.range.start)
            .map_or(ROOT_SCOPE, |(id, _)| id)
    }

    /// Returns the reference whose name touches `offset`.
    ///
    /// # Arguments
    /// * `offset` - byte offset of the cursor
    pub fn reference_at(&self, offset: usize) -> Option<&Reference> {
        self.references
            .iter()
            .find(|reference| reference.name.range.touches(offset))
    }

    /// Returns the local symbol whose defining name touches `offset`.
    ///
    /// # Arguments
    /// * `offset` - byte offset of the cursor
    pub fn definition_at(&self, offset: usize) -> Option<SymbolId> {
        self.symbols
            .iter()
            .position(|symbol| symbol.is_local() && symbol.name.range.touches(offset))
    }

    /// Returns the symbol under the cursor: a resolved reference or a local definition.
    ///
    /// # Arguments
    /// * `offset` - byte offset of the cursor
    pub fn symbol_at(&self, offset: usize) -> Option<SymbolId> {
        self.reference_at(offset)
            .and_then(|reference| reference.target)
            .or_else(|| self.definition_at(offset))
    }

    /// Returns the ranges of all occurrences of a symbol in the analysed document (definition first).
    ///
    /// # Arguments
    /// * `id` - symbol id
    pub fn occurrences(&self, id: SymbolId) -> Vec<TextRange> {
        let definition = self
            .symbols
            .get(id)
            .filter(|symbol| symbol.is_local())
            .map(|symbol| symbol.name.range);
        let mut ranges: Vec<TextRange> = definition
            .into_iter()
            .chain(
                self.references
                    .iter()
                    .filter(|reference| reference.target == Some(id))
                    .map(|reference| reference.name.range),
            )
            .collect();
        ranges.sort();
        ranges.dedup();
        ranges
    }

    /// Returns the symbols of a kind visible from `scope`, innermost first, one per name.
    ///
    /// # Arguments
    /// * `kind` - symbol kind
    /// * `scope` - innermost scope
    pub fn visible_symbols(&self, kind: SymbolKind, scope: ScopeId) -> Vec<SymbolId> {
        let chain: Vec<ScopeId> = scope_chain(&self.structure.scopes, scope).collect();
        let mut seen = std::collections::HashSet::new();
        chain
            .iter()
            .flat_map(|current| {
                self.symbols
                    .iter()
                    .enumerate()
                    .filter(move |(_, symbol)| symbol.kind == kind && symbol.scope == *current)
                    .map(|(id, _)| id)
            })
            .filter(|id| seen.insert(self.symbols[*id].name.key.clone()))
            .collect()
    }

    /// Returns the element info of a statement, if it is an element line.
    ///
    /// # Arguments
    /// * `statement` - statement index
    pub fn element_at_statement(&self, statement: usize) -> Option<&ElementInfo> {
        self.elements
            .iter()
            .find(|element| element.statement == statement)
    }

    /// Returns, per node symbol, the number of element terminals connected to it.
    pub fn connection_counts(&self) -> HashMap<SymbolId, usize> {
        connection_counts(&self.references)
    }
}

/// Returns the index of the statement whose range touches `offset`.
///
/// # Arguments
/// * `netlist` - lowered document
/// * `offset` - byte offset of the cursor
pub fn statement_at(netlist: &Netlist, offset: usize) -> Option<usize> {
    netlist
        .statements
        .iter()
        .position(|statement| statement.range.touches(offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{lower::lower, parse::parse_tree};

    fn analyze_text(text: &str) -> Analysis {
        let tree = parse_tree(text).expect("parser");
        analyze(&lower(&tree, text), ExternalContext::default())
    }

    #[test]
    fn symbol_at_finds_definitions_and_references() {
        let text = "* t\n.param wn=1u\nR1 a b 'wn*2'\n";
        let analysis = analyze_text(text);
        let definition = text.find("wn=").unwrap_or_default();
        let usage = text.rfind("wn").unwrap_or_default();
        let id = analysis.symbol_at(definition);
        assert!(id.is_some());
        assert_eq!(analysis.symbol_at(usage), id);
        assert_eq!(analysis.occurrences(id.unwrap_or_default()).len(), 2);
    }

    #[test]
    fn scope_at_returns_subckt_scope() {
        let text = "* t\n.subckt s a\nR1 a b 1k\n.ends\nR2 a b 1k\n";
        let analysis = analyze_text(text);
        assert_eq!(analysis.scope_at(text.find("R1").unwrap_or_default()), 1);
        assert_eq!(
            analysis.scope_at(text.find("R2").unwrap_or_default()),
            ROOT_SCOPE
        );
    }

    #[test]
    fn visible_symbols_prefers_inner_scope() {
        let text = "* t\n.param w=1\n.subckt s a w=2\nR1 a 0 'w'\n.ends\n";
        let analysis = analyze_text(text);
        let visible = analysis.visible_symbols(SymbolKind::Param, 1);
        assert_eq!(visible.len(), 1);
        assert_eq!(analysis.symbols[visible[0]].scope, 1);
    }
}
