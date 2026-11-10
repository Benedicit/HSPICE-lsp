//! Symbols, scopes, references and the case-insensitive lookup table.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::state::line_index::LineIndex;
use crate::syntax::ast::{Expr, Ident, ParamDef, TextRange};

/// Index into `Analysis::symbols`.
pub type SymbolId = usize;

/// Index into `Structure::scopes`.
pub type ScopeId = usize;

/// The top-level scope of a document.
pub const ROOT_SCOPE: ScopeId = 0;

/// What a symbol (or a reference) names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    /// `.param` value or `.subckt` header parameter.
    Param,
    /// User function `.param f(x)=…`.
    Function,
    /// `.subckt` definition.
    Subckt,
    /// `.model` definition.
    Model,
    /// Element instance such as `M1` or `Vdd`.
    Element,
    /// Circuit node.
    Node,
    /// `.lib` section (entry).
    LibSection,
    /// `.measure` result.
    Measure,
    /// `.data` block.
    DataBlock,
}

/// A file outside the current document that provides definitions.
#[derive(Debug)]
pub struct ExternalFile {
    /// Absolute path.
    pub path: PathBuf,
    /// File contents.
    pub text: String,
    /// Line index of `text`.
    pub line_index: LineIndex,
}

/// Where a symbol is defined.
#[derive(Debug, Clone)]
pub enum Origin {
    /// In the analysed document.
    Local,
    /// In another file (an include, a library or a file that includes this one).
    External(Arc<ExternalFile>),
}

/// Where a `.param` value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamSource {
    /// A `.param` statement.
    Statement,
    /// A default in a `.subckt` header.
    SubcktHeader,
    /// A column of a `.data` block.
    DataColumn,
}

/// Kind-specific data of a symbol.
#[derive(Debug, Clone)]
pub enum SymbolDetail {
    /// No extra data.
    None,
    /// A parameter.
    Param {
        /// Value expression.
        value: Option<Expr>,
        /// Where the parameter is defined.
        source: ParamSource,
    },
    /// A user function.
    Function {
        /// Formal arguments.
        arguments: Vec<Ident>,
    },
    /// A subcircuit.
    Subckt {
        /// Ports in order.
        ports: Vec<Ident>,
        /// Header parameters with defaults.
        params: Vec<ParamDef>,
        /// Scope of the subcircuit body (local definitions only).
        body: Option<ScopeId>,
    },
    /// A model.
    Model {
        /// Model type as written (`nmos`, `npn` …).
        model_type: Option<Ident>,
        /// `LEVEL=` value if it is a plain number.
        level: Option<f64>,
    },
    /// An element instance.
    Element {
        /// Index into `Analysis::elements`.
        element: usize,
    },
    /// A node.
    Node {
        /// Position in the enclosing `.subckt` port list.
        port_index: Option<usize>,
        /// True for `.global` nodes.
        global: bool,
    },
    /// A `.data` block.
    DataBlock {
        /// Column (parameter) names.
        columns: Vec<Ident>,
    },
}

/// A named definition.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// What the symbol names.
    pub kind: SymbolKind,
    /// Name and its location in the defining file.
    pub name: Ident,
    /// Scope that owns the definition (`ROOT_SCOPE` for external symbols).
    pub scope: ScopeId,
    /// Defining file.
    pub origin: Origin,
    /// Range of the whole defining statement or definition.
    pub definition_range: TextRange,
    /// Statement index in the defining document, if local.
    pub statement: Option<usize>,
    /// `.lib` section the definition lives in, if any (lowercase).
    pub section: Option<String>,
    /// Kind-specific data.
    pub detail: SymbolDetail,
}

impl Symbol {
    /// Returns true if the symbol is defined in the analysed document.
    pub fn is_local(&self) -> bool {
        matches!(self.origin, Origin::Local)
    }

    /// Returns true if the symbol is a `.global` node.
    pub fn is_global_node(&self) -> bool {
        matches!(self.detail, SymbolDetail::Node { global: true, .. })
    }
}

/// Returns the base name of a binned model name such as `nch.12`, or `None` without a numeric bin suffix.
///
/// # Arguments
/// * `name` - model name
pub fn bin_base(name: &str) -> Option<&str> {
    let (base, bin) = name.rsplit_once('.')?;
    (!base.is_empty() && !bin.is_empty() && bin.chars().all(|character| character.is_ascii_digit()))
        .then_some(base)
}

/// The syntactic context a reference appears in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefContext {
    /// Identifier or function call inside an expression.
    Expression,
    /// Bare parameter name used as an element value (`Rload a b rval`).
    ValueWord,
    /// Element terminal; `element` indexes `Analysis::elements`, `terminal` the node position.
    Terminal {
        /// Element index.
        element: usize,
        /// Terminal position.
        terminal: usize,
    },
    /// Controlling node pair of an E/G source (counts as a connection).
    ControlNode {
        /// Element index.
        element: usize,
    },
    /// Model name of an element.
    ModelSlot {
        /// Element index.
        element: usize,
    },
    /// Subcircuit name of an X element.
    SubcktSlot {
        /// Element index.
        element: usize,
    },
    /// Controlling voltage source of an F/H element.
    ControlSource {
        /// Element index.
        element: usize,
    },
    /// Inductor coupled by a K element.
    CoupledInductor {
        /// Element index.
        element: usize,
    },
    /// `key=` of an X instance that overrides a subcircuit parameter.
    InstanceParam {
        /// Element index.
        element: usize,
    },
    /// Output variable argument such as `v(out)` or `i(vdd)`.
    Output,
    /// Sweep variable or analysis source (`.dc vin …`, `.noise v(out) vin`).
    Sweep,
    /// Name after `.ends` / `.endl`.
    BlockEnd,
    /// `data=name` reference to a `.data` block.
    DataRef,
    /// Node listed in `.global`.
    Global,
    /// Port in a `.subckt` header.
    Port,
}

/// A use of a name in the analysed document.
#[derive(Debug, Clone)]
pub struct Reference {
    /// What the reference names (for unresolved references: the expected kind).
    pub kind: SymbolKind,
    /// Name as written.
    pub name: Ident,
    /// Scope the reference appears in.
    pub scope: ScopeId,
    /// Syntactic context.
    pub context: RefContext,
    /// Statement index.
    pub statement: usize,
    /// Resolved definition, if found.
    pub target: Option<SymbolId>,
}

/// A lexical scope: the document top level or a `.subckt` body.
#[derive(Debug, Clone, PartialEq)]
pub struct Scope {
    /// Enclosing scope (`None` for the root).
    pub parent: Option<ScopeId>,
    /// Statement index of the `.subckt` that opens this scope.
    pub subckt_statement: Option<usize>,
    /// Range covered by the scope.
    pub range: TextRange,
}

/// Case-insensitive index of symbols by kind, scope and name.
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    by_name: HashMap<(SymbolKind, ScopeId, String), Vec<SymbolId>>,
    binned_models: HashMap<String, Vec<SymbolId>>,
}

impl SymbolTable {
    /// Builds the index for a list of symbols.
    ///
    /// # Arguments
    /// * `symbols` - all symbols of an analysis (local and external)
    pub fn new(symbols: &[Symbol]) -> Self {
        let mut table = SymbolTable::default();
        symbols.iter().enumerate().for_each(|(id, symbol)| {
            let key = (symbol.kind, symbol.scope, symbol.name.key.clone());
            table.by_name.entry(key).or_default().push(id);
            if let Some(base) = binned_base_name(symbol) {
                table.binned_models.entry(base).or_default().push(id);
            }
        });
        table
    }

    /// Returns all symbols of a kind with this name directly in one scope.
    ///
    /// # Arguments
    /// * `kind` - symbol kind
    /// * `scope` - scope to search (no parents)
    /// * `key` - lowercase name
    pub fn in_scope(&self, kind: SymbolKind, scope: ScopeId, key: &str) -> &[SymbolId] {
        self.by_name
            .get(&(kind, scope, key.to_string()))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Finds the closest definition by walking from `scope` up to the root.
    ///
    /// Local definitions win over external ones because they are inserted first.
    ///
    /// # Arguments
    /// * `scopes` - scope tree
    /// * `kind` - symbol kind
    /// * `scope` - innermost scope of the reference
    /// * `key` - lowercase name
    pub fn lookup(
        &self,
        scopes: &[Scope],
        kind: SymbolKind,
        scope: ScopeId,
        key: &str,
    ) -> Option<SymbolId> {
        let found = scope_chain(scopes, scope)
            .find_map(|current| self.in_scope(kind, current, key).first().copied());
        found.or_else(|| self.binned_lookup(kind, key))
    }

    /// Returns all binned models `name.N` for a base model name.
    ///
    /// # Arguments
    /// * `key` - lowercase base name, e.g. `nch`
    pub fn bins(&self, key: &str) -> &[SymbolId] {
        self.binned_models
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Resolves a model reference to its first bin.
    ///
    /// # Arguments
    /// * `kind` - must be `SymbolKind::Model` for a match
    /// * `key` - lowercase base name
    fn binned_lookup(&self, kind: SymbolKind, key: &str) -> Option<SymbolId> {
        (kind == SymbolKind::Model)
            .then(|| self.bins(key).first().copied())
            .flatten()
    }
}

/// Iterates over a scope and all its ancestors, innermost first.
///
/// # Arguments
/// * `scopes` - scope tree
/// * `scope` - starting scope
pub fn scope_chain(scopes: &[Scope], scope: ScopeId) -> impl Iterator<Item = ScopeId> + '_ {
    std::iter::successors(Some(scope), |current| {
        scopes.get(*current).and_then(|entry| entry.parent)
    })
}

/// Returns the base name of a binned model such as `nch.12`, or `None` for other symbols.
///
/// # Arguments
/// * `symbol` - any symbol
fn binned_base_name(symbol: &Symbol) -> Option<String> {
    (symbol.kind == SymbolKind::Model)
        .then(|| bin_base(&symbol.name.key))
        .flatten()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(kind: SymbolKind, name: &str, scope: ScopeId) -> Symbol {
        Symbol {
            kind,
            name: Ident::new(name, TextRange::default()),
            scope,
            origin: Origin::Local,
            definition_range: TextRange::default(),
            statement: None,
            section: None,
            detail: SymbolDetail::None,
        }
    }

    #[test]
    fn lookup_walks_up_the_scope_chain() {
        let scopes = vec![
            Scope {
                parent: None,
                subckt_statement: None,
                range: TextRange::default(),
            },
            Scope {
                parent: Some(0),
                subckt_statement: Some(3),
                range: TextRange::default(),
            },
        ];
        let symbols = vec![
            symbol(SymbolKind::Param, "VDD", 0),
            symbol(SymbolKind::Param, "w", 1),
        ];
        let table = SymbolTable::new(&symbols);
        assert_eq!(table.lookup(&scopes, SymbolKind::Param, 1, "vdd"), Some(0));
        assert_eq!(table.lookup(&scopes, SymbolKind::Param, 1, "w"), Some(1));
        assert_eq!(table.lookup(&scopes, SymbolKind::Param, 0, "w"), None);
    }

    #[test]
    fn binned_models_resolve_by_base_name() {
        let scopes = vec![Scope {
            parent: None,
            subckt_statement: None,
            range: TextRange::default(),
        }];
        let symbols = vec![
            symbol(SymbolKind::Model, "nch.1", 0),
            symbol(SymbolKind::Model, "NCH.2", 0),
        ];
        let table = SymbolTable::new(&symbols);
        assert_eq!(table.lookup(&scopes, SymbolKind::Model, 0, "nch"), Some(0));
        assert_eq!(table.bins("nch").len(), 2);
        assert_eq!(table.lookup(&scopes, SymbolKind::Model, 0, "pch"), None);
    }
}
