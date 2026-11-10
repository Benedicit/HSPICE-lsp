//! Collects definitions and references of a document and resolves references to definitions.

use std::collections::{HashMap, HashSet};

use super::blocks::{BlockKind, Structure};
use super::roles::{ElementRoles, NameKinds, assign_roles, assignments, is_element_keyword};
use super::symbols::{
    Origin, ParamSource, ROOT_SCOPE, RefContext, Reference, ScopeId, Symbol, SymbolDetail,
    SymbolId, SymbolKind, SymbolTable,
};
use crate::lang::elements::{ElementSpec, find_element, find_source_function, is_ground};
use crate::lang::functions::{find_function, find_special_variable, is_optimization_function};
use crate::lang::measure::{ANALYSIS_KEYWORDS, ANALYSIS_TYPES, MEASURE_KEYWORDS};
use crate::lang::output_vars::{OutputArgKind, find_output_var, output_argument};
use crate::lang::{KeywordSpec, find_named};
use crate::syntax::ast::{
    Arg, AssignKey, Assignment, Call, Command, CommandKind, Element, Expr, ExprKind, Ident,
    Netlist, ParamDef, StatementKind, Value,
};

/// Role information of one element line.
#[derive(Debug, Clone)]
pub struct ElementInfo {
    /// Statement index.
    pub statement: usize,
    /// Element name.
    pub name: Ident,
    /// Scope the element lives in.
    pub scope: ScopeId,
    /// Element type from the first letter.
    pub spec: Option<&'static ElementSpec>,
    /// Role assignment of the positional arguments.
    pub roles: ElementRoles,
    /// True inside `.variation` blocks, where element semantics do not apply.
    pub skipped: bool,
}

/// Definitions, elements and resolved references of a document.
#[derive(Debug, Clone, Default)]
pub struct Resolution {
    /// Local definitions first, then external ones.
    pub symbols: Vec<Symbol>,
    /// Index over `symbols`.
    pub table: SymbolTable,
    /// Element lines with their roles.
    pub elements: Vec<ElementInfo>,
    /// All references in source order of collection.
    pub references: Vec<Reference>,
}

/// Creates a symbol defined in the analysed document (or an external file via `origin`).
///
/// # Arguments
/// * `kind` - symbol kind
/// * `name` - defined name
/// * `scope` - owning scope
/// * `statement` - statement index
/// * `context` - origin, statement range and section shared by all definitions of a statement
/// * `detail` - kind-specific data
fn new_symbol(
    kind: SymbolKind,
    name: &Ident,
    scope: ScopeId,
    statement: usize,
    context: &DefinitionContext,
    detail: SymbolDetail,
) -> Symbol {
    Symbol {
        kind,
        name: name.clone(),
        scope,
        origin: context.origin.clone(),
        definition_range: context.range,
        statement: Some(statement),
        section: context.section.clone(),
        detail,
    }
}

/// Data shared by all definitions of one statement.
struct DefinitionContext {
    origin: Origin,
    range: crate::syntax::ast::TextRange,
    section: Option<String>,
}

/// Collects all definitions except nodes and elements (params, functions, subckts, models, sections, measures, data).
///
/// # Arguments
/// * `netlist` - lowered document
/// * `structure` - block structure of the document
/// * `origin` - where the document lives (local or an external file)
pub fn collect_definitions(
    netlist: &Netlist,
    structure: &Structure,
    origin: &Origin,
) -> Vec<Symbol> {
    netlist
        .statements
        .iter()
        .enumerate()
        .filter(|(index, _)| !structure.is_skipped(*index))
        .flat_map(|(index, statement)| {
            let StatementKind::Command(command) = &statement.kind else {
                return Vec::new();
            };
            let context = DefinitionContext {
                origin: origin.clone(),
                range: statement.range,
                section: structure.statement_section.get(index).cloned().flatten(),
            };
            let scope = structure.scope_of(index);
            command_definitions(command, index, scope, structure, &context)
        })
        .collect()
}

/// Returns the definitions made by one dot command.
///
/// # Arguments
/// * `command` - lowered command
/// * `index` - statement index
/// * `scope` - scope of the statement
/// * `structure` - block structure (for subckt body scopes)
/// * `context` - origin, range and section of the statement
fn command_definitions(
    command: &Command,
    index: usize,
    scope: ScopeId,
    structure: &Structure,
    context: &DefinitionContext,
) -> Vec<Symbol> {
    match &command.kind {
        CommandKind::Param { definitions } => definitions
            .iter()
            .map(|definition| {
                param_symbol(definition, scope, index, context, ParamSource::Statement)
            })
            .collect(),
        CommandKind::Subckt {
            name: Some(name),
            ports,
            params,
        } => {
            let body = structure.subckt_scope(index);
            let subckt = new_symbol(
                SymbolKind::Subckt,
                name,
                scope,
                index,
                context,
                SymbolDetail::Subckt {
                    ports: ports.clone(),
                    params: params.clone(),
                    body,
                },
            );
            let header_params = params.iter().map(|param| {
                param_symbol(
                    param,
                    body.unwrap_or(scope),
                    index,
                    context,
                    ParamSource::SubcktHeader,
                )
            });
            std::iter::once(subckt).chain(header_params).collect()
        }
        CommandKind::Model {
            name: Some(name),
            model_type,
            args,
        } => {
            vec![new_symbol(
                SymbolKind::Model,
                name,
                scope,
                index,
                context,
                SymbolDetail::Model {
                    model_type: model_type.clone(),
                    level: model_level(args),
                },
            )]
        }
        CommandKind::Lib {
            path: None,
            section: Some(section),
        } => {
            vec![new_symbol(
                SymbolKind::LibSection,
                section,
                ROOT_SCOPE,
                index,
                context,
                SymbolDetail::None,
            )]
        }
        CommandKind::Generic { args } => match command.keyword.key.as_str() {
            ".meas" | ".measure" => measure_name(args)
                .map(|name| {
                    vec![new_symbol(
                        SymbolKind::Measure,
                        &name,
                        ROOT_SCOPE,
                        index,
                        context,
                        SymbolDetail::None,
                    )]
                })
                .unwrap_or_default(),
            ".data" => data_definitions(args, index, context),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Creates the symbol for a `.param` definition (a parameter or a user function).
///
/// # Arguments
/// * `definition` - lowered definition
/// * `scope` - owning scope
/// * `index` - statement index
/// * `context` - origin, range and section
/// * `source` - where the parameter comes from
fn param_symbol(
    definition: &ParamDef,
    scope: ScopeId,
    index: usize,
    context: &DefinitionContext,
    source: ParamSource,
) -> Symbol {
    let (kind, detail) = if definition.arguments.is_empty() {
        (
            SymbolKind::Param,
            SymbolDetail::Param {
                value: definition.value.clone(),
                source,
            },
        )
    } else {
        (
            SymbolKind::Function,
            SymbolDetail::Function {
                arguments: definition.arguments.clone(),
            },
        )
    };
    let mut symbol = new_symbol(kind, &definition.name, scope, index, context, detail);
    symbol.definition_range = definition.range;
    symbol
}

/// Extracts a numeric `LEVEL=` value from model arguments (also inside a parenthesized group).
///
/// # Arguments
/// * `args` - model arguments
fn model_level(args: &[Arg]) -> Option<f64> {
    args.iter().find_map(|arg| match arg {
        Arg::Assign(assignment) => match (&assignment.key, &assignment.value) {
            (
                AssignKey::Name(key),
                Some(Value::Expr(Expr {
                    kind: ExprKind::Number(text),
                    ..
                })),
            ) if key.key == "level" => {
                crate::syntax::number::parse_number(text).map(|number| number.value)
            }
            _ => None,
        },
        Arg::Group { args, .. } => model_level(args),
        Arg::Value(_) => None,
    })
}

/// Returns the bare words among the positional arguments of a command.
///
/// # Arguments
/// * `args` - command arguments
fn positional_words(args: &[Arg]) -> Vec<&Ident> {
    args.iter()
        .filter_map(|arg| match arg {
            Arg::Value(Value::Word(word)) => Some(word),
            _ => None,
        })
        .collect()
}

/// Returns the result name of a `.measure` statement (skipping an optional analysis type).
///
/// # Arguments
/// * `args` - `.measure` arguments
fn measure_name(args: &[Arg]) -> Option<Ident> {
    let words = positional_words(args);
    let first = words.first()?;
    let is_analysis = find_named(ANALYSIS_TYPES, &first.key).is_some();
    let name = if is_analysis { words.get(1)? } else { first };
    let starts_with_word = matches!(args.first(), Some(Arg::Value(Value::Word(_))));
    starts_with_word.then(|| (*name).clone())
}

/// Returns the `.data` block symbol and its column parameters.
///
/// # Arguments
/// * `args` - `.data` arguments
/// * `index` - statement index
/// * `context` - origin, range and section
fn data_definitions(args: &[Arg], index: usize, context: &DefinitionContext) -> Vec<Symbol> {
    let words = positional_words(args);
    let Some(name) = words.first() else {
        return Vec::new();
    };
    let inline_columns = args.iter().skip(1).map_while(|arg| match arg {
        Arg::Value(Value::Word(word)) if !matches!(word.key.as_str(), "mer" | "lam") => {
            Some(word.clone())
        }
        _ => None,
    });
    let file_columns = args.iter().filter_map(|arg| match arg {
        Arg::Assign(assignment) => match &assignment.key {
            AssignKey::Name(key) if !matches!(key.key.as_str(), "file" | "out") => {
                Some(key.clone())
            }
            _ => None,
        },
        _ => None,
    });
    let columns: Vec<Ident> = inline_columns.chain(file_columns).collect();
    let column_symbols = columns.iter().map(|column| {
        new_symbol(
            SymbolKind::Param,
            column,
            ROOT_SCOPE,
            index,
            context,
            SymbolDetail::Param {
                value: None,
                source: ParamSource::DataColumn,
            },
        )
    });
    std::iter::once(new_symbol(
        SymbolKind::DataBlock,
        name,
        ROOT_SCOPE,
        index,
        context,
        SymbolDetail::DataBlock {
            columns: columns.clone(),
        },
    ))
    .chain(column_symbols)
    .collect()
}

/// Collects element infos with role assignment, using the definitions known so far for tie-breaking.
///
/// # Arguments
/// * `netlist` - lowered document
/// * `structure` - block structure
/// * `table` - index over the definitions collected so far
fn collect_elements(
    netlist: &Netlist,
    structure: &Structure,
    table: &SymbolTable,
) -> Vec<ElementInfo> {
    let scopes = &structure.scopes;
    netlist
        .statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match &statement.kind {
            StatementKind::Element(element) => Some((index, element)),
            _ => None,
        })
        .map(|(index, element)| {
            let scope = structure.scope_of(index);
            let is_model = |key: &str| {
                table
                    .lookup(scopes, SymbolKind::Model, scope, key)
                    .is_some()
            };
            let is_subckt = |key: &str| {
                table
                    .lookup(scopes, SymbolKind::Subckt, scope, key)
                    .is_some()
            };
            let names = NameKinds {
                is_model: &is_model,
                is_subckt: &is_subckt,
            };
            let spec = find_element(&element.name.text);
            ElementInfo {
                statement: index,
                name: element.name.clone(),
                scope,
                spec,
                roles: assign_roles(element, spec, &names),
                skipped: structure.is_skipped(index),
            }
        })
        .collect()
}

/// Creates element symbols and node symbols (ports, globals, then first use in each scope).
///
/// # Arguments
/// * `elements` - element infos
/// * `netlist` - lowered document
/// * `structure` - block structure
/// * `symbols` - definitions collected so far (subckts provide ports)
fn element_and_node_symbols(
    elements: &[ElementInfo],
    netlist: &Netlist,
    structure: &Structure,
    symbols: &[Symbol],
) -> Vec<Symbol> {
    let local = &DefinitionContext {
        origin: Origin::Local,
        range: Default::default(),
        section: None,
    };
    let with_range = |mut symbol: Symbol, index: usize| {
        symbol.definition_range = netlist
            .statements
            .get(index)
            .map(|statement| statement.range)
            .unwrap_or_default();
        symbol.section = structure.statement_section.get(index).cloned().flatten();
        symbol
    };
    let globals: Vec<Symbol> = netlist
        .statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match &statement.kind {
            StatementKind::Command(Command {
                kind: CommandKind::Global { nodes },
                ..
            }) => Some((index, nodes)),
            _ => None,
        })
        .flat_map(|(index, nodes)| {
            nodes
                .iter()
                .filter(|node| !is_ground(&node.text))
                .map(move |node| {
                    with_range(
                        new_symbol(
                            SymbolKind::Node,
                            node,
                            ROOT_SCOPE,
                            index,
                            local,
                            SymbolDetail::Node {
                                port_index: None,
                                global: true,
                            },
                        ),
                        index,
                    )
                })
        })
        .collect();
    let ports = symbols
        .iter()
        .filter(|symbol| symbol.is_local())
        .filter_map(|symbol| match &symbol.detail {
            SymbolDetail::Subckt {
                ports,
                body: Some(body),
                ..
            } => Some((symbol, ports, *body)),
            _ => None,
        })
        .flat_map(|(symbol, ports, body)| {
            let index = symbol.statement.unwrap_or_default();
            ports
                .iter()
                .enumerate()
                .filter(|(_, port)| !is_ground(&port.text))
                .map(move |(position, port)| {
                    with_range(
                        new_symbol(
                            SymbolKind::Node,
                            port,
                            body,
                            index,
                            local,
                            SymbolDetail::Node {
                                port_index: Some(position),
                                global: false,
                            },
                        ),
                        index,
                    )
                })
        });
    let element_symbols = elements.iter().enumerate().map(|(position, element)| {
        with_range(
            new_symbol(
                SymbolKind::Element,
                &element.name,
                element.scope,
                element.statement,
                local,
                SymbolDetail::Element { element: position },
            ),
            element.statement,
        )
    });
    let declared: Vec<Symbol> = globals.iter().cloned().chain(ports).collect();
    let mut seen: HashSet<(ScopeId, String)> = declared
        .iter()
        .map(|symbol| (symbol.scope, symbol.name.key.clone()))
        .collect();
    let global_keys: HashSet<String> = globals
        .iter()
        .map(|symbol| symbol.name.key.clone())
        .collect();
    let used_nodes: Vec<Symbol> = elements
        .iter()
        .filter(|element| !element.skipped)
        .flat_map(|element| {
            element
                .roles
                .nodes
                .iter()
                .chain(&element.roles.control_nodes)
                .map(move |node| (element, node))
        })
        .filter(|(_, node)| !is_ground(&node.text) && !global_keys.contains(&node.key))
        .filter(|(element, node)| seen.insert((element.scope, node.key.clone())))
        .map(|(element, node)| {
            with_range(
                new_symbol(
                    SymbolKind::Node,
                    node,
                    element.scope,
                    element.statement,
                    local,
                    SymbolDetail::Node {
                        port_index: None,
                        global: false,
                    },
                ),
                element.statement,
            )
        })
        .collect();
    element_symbols.chain(declared).chain(used_nodes).collect()
}

/// Collects definitions and references of a document and resolves them.
///
/// # Arguments
/// * `netlist` - lowered document
/// * `structure` - block structure
/// * `external` - definitions visible from other files
pub fn resolve(netlist: &Netlist, structure: &Structure, external: &[Symbol]) -> Resolution {
    let definitions = collect_definitions(netlist, structure, &Origin::Local);
    let base: Vec<Symbol> = definitions
        .into_iter()
        .chain(external.iter().cloned())
        .collect();
    let base_table = SymbolTable::new(&base);
    let elements = collect_elements(netlist, structure, &base_table);
    let local_count = base.iter().filter(|symbol| symbol.is_local()).count();
    let extra = element_and_node_symbols(&elements, netlist, structure, &base);
    let symbols: Vec<Symbol> = base[..local_count]
        .iter()
        .cloned()
        .chain(extra)
        .chain(base[local_count..].iter().cloned())
        .collect();
    let table = SymbolTable::new(&symbols);
    let collector = Collector {
        structure,
        symbols: &symbols,
        table: &table,
        elements: &elements,
    };
    let references = collector.collect(netlist);
    Resolution {
        symbols,
        table,
        elements,
        references,
    }
}

/// Read-only context for collecting and resolving references.
struct Collector<'a> {
    structure: &'a Structure,
    symbols: &'a [Symbol],
    table: &'a SymbolTable,
    elements: &'a [ElementInfo],
}

/// A reference before resolution.
struct Pending {
    kinds: &'static [SymbolKind],
    name: Ident,
    scope: ScopeId,
    context: RefContext,
}

impl Pending {
    /// Creates a pending reference.
    ///
    /// # Arguments
    /// * `kinds` - kinds to try in order (the first is reported when unresolved)
    /// * `name` - referenced name
    /// * `scope` - scope of the reference
    /// * `context` - syntactic context
    fn new(
        kinds: &'static [SymbolKind],
        name: &Ident,
        scope: ScopeId,
        context: RefContext,
    ) -> Self {
        Self {
            kinds,
            name: name.clone(),
            scope,
            context,
        }
    }
}

const PARAM: &[SymbolKind] = &[SymbolKind::Param];
const PARAM_OR_MEASURE: &[SymbolKind] = &[SymbolKind::Param, SymbolKind::Measure];
const FUNCTION: &[SymbolKind] = &[SymbolKind::Function];
const NODE: &[SymbolKind] = &[SymbolKind::Node];
const ELEMENT: &[SymbolKind] = &[SymbolKind::Element];
const MODEL: &[SymbolKind] = &[SymbolKind::Model];
const MODEL_OR_SUBCKT: &[SymbolKind] = &[SymbolKind::Model, SymbolKind::Subckt];
const SUBCKT: &[SymbolKind] = &[SymbolKind::Subckt];
const SECTION: &[SymbolKind] = &[SymbolKind::LibSection];
const DATA: &[SymbolKind] = &[SymbolKind::DataBlock];
const SOURCE_OR_PARAM: &[SymbolKind] = &[SymbolKind::Element, SymbolKind::Param];

impl Collector<'_> {
    /// Collects and resolves all references of the document.
    ///
    /// # Arguments
    /// * `netlist` - lowered document
    fn collect(&self, netlist: &Netlist) -> Vec<Reference> {
        netlist
            .statements
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.structure.is_skipped(*index))
            .flat_map(|(index, statement)| {
                let scope = self.structure.scope_of(index);
                let pending = match &statement.kind {
                    StatementKind::Element(element) => self.element_refs(index, element),
                    StatementKind::Command(command) => self.command_refs(index, scope, command),
                    StatementKind::Comment | StatementKind::Error => Vec::new(),
                };
                pending
                    .into_iter()
                    .map(move |pending| self.resolve_pending(pending, index))
            })
            .collect()
    }

    /// Resolves a pending reference by trying its kinds in order.
    ///
    /// # Arguments
    /// * `pending` - unresolved reference
    /// * `statement` - statement index
    fn resolve_pending(&self, pending: Pending, statement: usize) -> Reference {
        let found = pending.kinds.iter().find_map(|kind| {
            self.lookup(*kind, pending.scope, &pending.name.key)
                .map(|id| (*kind, id))
        });
        Reference {
            kind: found.map_or(pending.kinds[0], |(kind, _)| kind),
            name: pending.name,
            scope: pending.scope,
            context: pending.context,
            statement,
            target: found.map(|(_, id)| id),
        }
    }

    /// Looks up a name; nodes resolve locally, then as `.global` nodes.
    ///
    /// # Arguments
    /// * `kind` - symbol kind
    /// * `scope` - innermost scope
    /// * `key` - lowercase name
    fn lookup(&self, kind: SymbolKind, scope: ScopeId, key: &str) -> Option<SymbolId> {
        if kind == SymbolKind::Node {
            return self
                .table
                .in_scope(kind, scope, key)
                .first()
                .or_else(|| {
                    self.table
                        .in_scope(kind, ROOT_SCOPE, key)
                        .iter()
                        .find(|id| self.symbols[**id].is_global_node())
                })
                .copied();
        }
        self.table.lookup(&self.structure.scopes, kind, scope, key)
    }

    /// Collects references of an element line.
    ///
    /// # Arguments
    /// * `index` - statement index
    /// * `element` - lowered element
    fn element_refs(&self, index: usize, element: &Element) -> Vec<Pending> {
        let Ok(position) = self
            .elements
            .binary_search_by_key(&index, |info| info.statement)
        else {
            return Vec::new();
        };
        let info = &self.elements[position];
        if info.skipped {
            return Vec::new();
        }
        let scope = info.scope;
        let roles = &info.roles;
        let letter = info.spec.map_or('?', |spec| spec.letter);
        let terminals = roles
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| !is_ground(&node.text))
            .map(|(terminal, node)| {
                Pending::new(
                    NODE,
                    node,
                    scope,
                    RefContext::Terminal {
                        element: position,
                        terminal,
                    },
                )
            });
        let control_nodes = roles
            .control_nodes
            .iter()
            .filter(|node| !is_ground(&node.text))
            .map(|node| {
                Pending::new(
                    NODE,
                    node,
                    scope,
                    RefContext::ControlNode { element: position },
                )
            });
        let reference = roles.reference.iter().map(|name| match letter {
            'X' => Pending::new(
                SUBCKT,
                name,
                scope,
                RefContext::SubcktSlot { element: position },
            ),
            'M' => Pending::new(
                MODEL_OR_SUBCKT,
                name,
                scope,
                RefContext::ModelSlot { element: position },
            ),
            _ => Pending::new(
                MODEL,
                name,
                scope,
                RefContext::ModelSlot { element: position },
            ),
        });
        let controls = roles.controls.iter().map(|name| {
            let context = if letter == 'K' {
                RefContext::CoupledInductor { element: position }
            } else {
                RefContext::ControlSource { element: position }
            };
            Pending::new(ELEMENT, name, scope, context)
        });
        let values = roles
            .values
            .iter()
            .flat_map(|value| self.value_refs(value, info.spec, scope));
        let instance_params = assignments(element).flat_map(|assignment| {
            let key_ref = match (&assignment.key, letter) {
                (AssignKey::Name(key), 'X') => self.instance_param_ref(key, roles, scope, position),
                _ => None,
            };
            let values = assignment.value.iter().chain(&assignment.extra_values);
            key_ref.into_iter().chain(
                values
                    .flat_map(|value| self.value_refs(value, info.spec, scope))
                    .collect::<Vec<_>>(),
            )
        });
        terminals
            .chain(control_nodes)
            .chain(reference)
            .chain(controls)
            .chain(values)
            .chain(instance_params)
            .collect()
    }

    /// Builds the reference for `key=` on an X instance, targeting the subckt's header parameter.
    ///
    /// # Arguments
    /// * `key` - assignment key
    /// * `roles` - roles of the X element
    /// * `scope` - scope of the element
    /// * `position` - element index
    fn instance_param_ref(
        &self,
        key: &Ident,
        roles: &ElementRoles,
        scope: ScopeId,
        position: usize,
    ) -> Option<Pending> {
        let subckt = roles.reference.as_ref()?;
        let subckt_id = self.lookup(SymbolKind::Subckt, scope, &subckt.key)?;
        let symbol = self
            .symbols
            .get(subckt_id)
            .filter(|symbol| symbol.is_local())?;
        let SymbolDetail::Subckt {
            body: Some(body), ..
        } = &symbol.detail
        else {
            return None;
        };
        Some(Pending::new(
            PARAM,
            key,
            *body,
            RefContext::InstanceParam { element: position },
        ))
    }

    /// Collects references inside a positional or assigned element value.
    ///
    /// # Arguments
    /// * `value` - value
    /// * `spec` - element type (for keywords)
    /// * `scope` - scope of the element
    fn value_refs(
        &self,
        value: &Value,
        spec: Option<&ElementSpec>,
        scope: ScopeId,
    ) -> Vec<Pending> {
        match value {
            Value::Word(word)
                if !is_element_keyword(spec, &word.key) && is_plain_name(&word.text) =>
            {
                vec![Pending::new(PARAM, word, scope, RefContext::ValueWord)]
            }
            Value::Expr(expr) => expr_refs(expr, scope, &[]),
            Value::Call(call) => self.call_refs(call, spec, scope),
            _ => Vec::new(),
        }
    }

    /// Collects references inside a call-like value (`pulse(…)`, `str(x)`, `v(out)`).
    ///
    /// # Arguments
    /// * `call` - call value
    /// * `spec` - element type (for keywords)
    /// * `scope` - scope of the statement
    fn call_refs(&self, call: &Call, spec: Option<&ElementSpec>, scope: ScopeId) -> Vec<Pending> {
        let key = call.name.key.as_str();
        if key == "str" {
            return call_words(call)
                .map(|word| Pending::new(PARAM, word, scope, RefContext::ValueWord))
                .collect();
        }
        if find_source_function(key).is_some() {
            return call_values(call)
                .flat_map(|value| self.value_refs(value, spec, scope))
                .collect();
        }
        if find_output_var(key).is_some() {
            return output_refs(call, scope);
        }
        Vec::new()
    }

    /// Collects references of a dot command.
    ///
    /// # Arguments
    /// * `index` - statement index
    /// * `scope` - scope of the statement
    /// * `command` - lowered command
    fn command_refs(&self, index: usize, scope: ScopeId, command: &Command) -> Vec<Pending> {
        match &command.kind {
            CommandKind::Param { definitions } => definitions
                .iter()
                .flat_map(|definition| {
                    let formals: Vec<&str> = definition
                        .arguments
                        .iter()
                        .map(|argument| argument.key.as_str())
                        .collect();
                    definition
                        .value
                        .iter()
                        .flat_map(move |value| expr_refs(value, scope, &formals))
                        .collect::<Vec<_>>()
                })
                .collect(),
            CommandKind::Subckt { ports, params, .. } => {
                let body = self.structure.subckt_scope(index).unwrap_or(scope);
                let port_refs = ports
                    .iter()
                    .filter(|port| !is_ground(&port.text))
                    .map(|port| Pending::new(NODE, port, body, RefContext::Port));
                let default_refs = params.iter().flat_map(|param| {
                    param
                        .value
                        .iter()
                        .flat_map(|value| expr_refs(value, body, &[]))
                        .collect::<Vec<_>>()
                });
                port_refs.chain(default_refs).collect()
            }
            CommandKind::Ends { name: Some(name) } => self.block_end_ref(index, name, SUBCKT),
            CommandKind::Endl { name: Some(name) } => self.block_end_ref(index, name, SECTION),
            CommandKind::Model {
                model_type, args, ..
            } => {
                let is_option_model = model_type.as_ref().is_some_and(|kind| {
                    matches!(kind.key.as_str(), "opt" | "amp" | "plot" | "plotq")
                });
                if is_option_model {
                    return Vec::new();
                }
                assignment_value_refs(args, scope)
            }
            CommandKind::Global { nodes } => nodes
                .iter()
                .filter(|node| !is_ground(&node.text))
                .map(|node| Pending::new(NODE, node, ROOT_SCOPE, RefContext::Global))
                .collect(),
            CommandKind::If { condition } | CommandKind::ElseIf { condition } => condition
                .iter()
                .flat_map(|condition| expr_refs(condition, scope, &[]))
                .collect(),
            CommandKind::Generic { args } => self.generic_refs(&command.keyword.key, args, scope),
            _ => Vec::new(),
        }
    }

    /// Builds the reference of a name after `.ends` / `.endl`, targeting the block it closes.
    ///
    /// # Arguments
    /// * `index` - statement index of the closer
    /// * `name` - name after the closer
    /// * `kinds` - expected kind
    fn block_end_ref(
        &self,
        index: usize,
        name: &Ident,
        kinds: &'static [SymbolKind],
    ) -> Vec<Pending> {
        let opener_scope = self
            .structure
            .blocks
            .iter()
            .find(|block| {
                block.close == Some(index)
                    && matches!(block.kind, BlockKind::Subckt | BlockKind::LibSection)
            })
            .map_or(ROOT_SCOPE, |block| self.structure.scope_of(block.open));
        vec![Pending::new(
            kinds,
            name,
            opener_scope,
            RefContext::BlockEnd,
        )]
    }

    /// Collects references of generic commands (analyses, output requests, measures …).
    ///
    /// # Arguments
    /// * `keyword` - lowercase command keyword
    /// * `args` - command arguments
    /// * `scope` - scope of the statement
    fn generic_refs(&self, keyword: &str, args: &[Arg], scope: ScopeId) -> Vec<Pending> {
        let outputs = args.iter().flat_map(|arg| match arg {
            Arg::Value(Value::Call(call)) => self.call_refs(call, None, scope),
            Arg::Assign(assignment) => match &assignment.key {
                AssignKey::Call(call) => self.call_refs(call, None, scope),
                AssignKey::Name(_) => Vec::new(),
            },
            _ => Vec::new(),
        });
        let specific: Vec<Pending> = match keyword {
            ".dc" | ".tran" | ".ac" => sweep_refs(args, scope, keyword == ".dc"),
            ".noise" | ".tf" | ".pz" => positional_words(args)
                .first()
                .map(|source| vec![Pending::new(ELEMENT, source, scope, RefContext::Sweep)])
                .unwrap_or_default(),
            ".meas" | ".measure" => measure_refs(args, scope),
            ".print" | ".probe" | ".plot" | ".graph" | ".ic" | ".nodeset" | ".dcvolt" | ".four" => {
                assignment_value_refs(args, scope)
            }
            ".fft" => assignment_value_refs(&without_keys(args, FFT_WORD_KEYS), scope),
            ".mosra" => assignment_value_refs(&without_keys(args, MOSRA_WORD_KEYS), scope),
            ".temp" | ".temperature" => args
                .iter()
                .flat_map(|arg| match arg {
                    Arg::Value(Value::Word(word)) if is_plain_name(&word.text) => {
                        vec![Pending::new(PARAM, word, scope, RefContext::ValueWord)]
                    }
                    Arg::Value(Value::Expr(expr)) => expr_refs(expr, scope, &[]),
                    _ => Vec::new(),
                })
                .collect(),
            _ => Vec::new(),
        };
        outputs.chain(specific).collect()
    }
}

/// Returns true for names that can be parameter references (no operators or hierarchy separators).
///
/// # Arguments
/// * `text` - word text
fn is_plain_name(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Iterates over the positional values of a call.
///
/// # Arguments
/// * `call` - call value
fn call_values(call: &Call) -> impl Iterator<Item = &Value> {
    call.args.iter().filter_map(|arg| match arg {
        Arg::Value(value) => Some(value),
        _ => None,
    })
}

/// Iterates over the bare words among a call's arguments.
///
/// # Arguments
/// * `call` - call value
fn call_words(call: &Call) -> impl Iterator<Item = &Ident> {
    call_values(call).filter_map(|value| match value {
        Value::Word(word) => Some(word),
        _ => None,
    })
}

/// Collects node or element references of an output variable such as `v(a,b)` or `i(vdd)`.
///
/// Wildcards and ground are skipped.
///
/// # Arguments
/// * `call` - output-variable call
/// * `scope` - scope of the statement
fn output_refs(call: &Call, scope: ScopeId) -> Vec<Pending> {
    let Some(argument) = output_argument(&call.name.key) else {
        return Vec::new();
    };
    let kinds = match argument {
        OutputArgKind::Nodes => NODE,
        OutputArgKind::Element => ELEMENT,
        OutputArgKind::Expression => {
            return call_values(call)
                .flat_map(|value| match value {
                    Value::Expr(expr) => expr_refs(expr, scope, &[]),
                    _ => Vec::new(),
                })
                .collect();
        }
        OutputArgKind::Other => return Vec::new(),
    };
    call_words(call)
        .filter(|word| !word.text.contains(['*', '?', ':']) && !is_ground(&word.text))
        .map(|word| Pending::new(kinds, word, scope, RefContext::Output))
        .collect()
}

/// `.fft` keys whose values are keywords such as `HANN` or `NORM`, not expressions.
const FFT_WORD_KEYS: &[&str] = &["window", "format"];

/// `.mosra` keys whose values are keywords or instance patterns, not expressions.
const MOSRA_WORD_KEYS: &[&str] = &["mosralife", "aginginst"];

/// Returns the arguments without the assignments to the given keys.
///
/// # Arguments
/// * `args` - command arguments
/// * `keys` - lowercase keys to drop
fn without_keys(args: &[Arg], keys: &[&str]) -> Vec<Arg> {
    args.iter()
        .filter(|arg| match arg {
            Arg::Assign(Assignment {
                key: AssignKey::Name(key),
                ..
            }) => !keys.contains(&key.key.as_str()),
            _ => true,
        })
        .cloned()
        .collect()
}

/// Collects expression references of all assignment values (and nested groups).
///
/// # Arguments
/// * `args` - command arguments
/// * `scope` - scope of the statement
fn assignment_value_refs(args: &[Arg], scope: ScopeId) -> Vec<Pending> {
    args.iter()
        .flat_map(|arg| match arg {
            Arg::Assign(assignment) => assignment
                .value
                .iter()
                .chain(&assignment.extra_values)
                .flat_map(|value| match value {
                    Value::Expr(expr) => expr_refs(expr, scope, &[]),
                    _ => Vec::new(),
                })
                .collect(),
            Arg::Group { args, .. } => assignment_value_refs(args, scope),
            Arg::Value(Value::Expr(expr)) => expr_refs(expr, scope, &[]),
            Arg::Value(_) => Vec::new(),
        })
        .collect()
}

/// Returns true if a word is a keyword in `table`.
///
/// # Arguments
/// * `table` - keyword table
/// * `key` - lowercase word
fn is_keyword(table: &'static [KeywordSpec], key: &str) -> bool {
    find_named(table, key).is_some()
}

/// Collects sweep variables, data references and parameter words of `.dc`, `.tran` and `.ac`.
///
/// # Arguments
/// * `args` - command arguments
/// * `scope` - scope of the statement
/// * `first_is_sweep` - true for `.dc`, whose first word is the sweep variable
fn sweep_refs(args: &[Arg], scope: ScopeId, first_is_sweep: bool) -> Vec<Pending> {
    let (pending, _) = args.iter().enumerate().fold(
        (Vec::new(), false),
        |(mut pending, after_sweep), (index, arg)| {
            match arg {
                Arg::Value(Value::Word(word)) if word.key == "sweep" => return (pending, true),
                Arg::Value(Value::Word(word))
                    if word.key == "temp" || is_keyword(ANALYSIS_KEYWORDS, &word.key) => {}
                Arg::Value(Value::Word(word)) if after_sweep || (first_is_sweep && index == 0) => {
                    pending.push(Pending::new(
                        SOURCE_OR_PARAM,
                        word,
                        scope,
                        RefContext::Sweep,
                    ));
                }
                Arg::Value(Value::Word(word)) if is_plain_name(&word.text) => {
                    pending.push(Pending::new(PARAM, word, scope, RefContext::ValueWord));
                }
                Arg::Value(Value::Expr(expr)) => pending.extend(expr_refs(expr, scope, &[])),
                Arg::Assign(assignment) => match (&assignment.key, &assignment.value) {
                    (
                        AssignKey::Name(key),
                        Some(Value::Expr(Expr {
                            kind: ExprKind::Ident(name),
                            ..
                        })),
                    ) if key.key == "data" => {
                        pending.push(Pending::new(DATA, name, ROOT_SCOPE, RefContext::DataRef));
                    }
                    (AssignKey::Name(key), _)
                        if matches!(
                            key.key.as_str(),
                            "monte" | "optimize" | "results" | "model"
                        ) => {}
                    _ => pending.extend(assignment_value_refs(std::slice::from_ref(arg), scope)),
                },
                _ => {}
            }
            (pending, false)
        },
    );
    pending
}

/// Collects expression references of `.measure` assignments (identifiers may name other measures).
///
/// # Arguments
/// * `args` - `.measure` arguments
/// * `scope` - scope of the statement
fn measure_refs(args: &[Arg], scope: ScopeId) -> Vec<Pending> {
    assignment_value_refs(args, scope)
        .into_iter()
        .filter(|pending| !is_keyword(MEASURE_KEYWORDS, &pending.name.key))
        .map(|pending| Pending {
            kinds: if pending.kinds == PARAM {
                PARAM_OR_MEASURE
            } else {
                pending.kinds
            },
            ..pending
        })
        .collect()
}

/// Collects references inside an expression: parameters, user functions and output variables.
///
/// # Arguments
/// * `expr` - expression
/// * `scope` - scope of the expression
/// * `formals` - formal arguments of the enclosing user function (lowercase), never reported
fn expr_refs(expr: &Expr, scope: ScopeId, formals: &[&str]) -> Vec<Pending> {
    match &expr.kind {
        ExprKind::Ident(name) => {
            let builtin =
                find_special_variable(&name.key).is_some() || formals.contains(&name.key.as_str());
            if builtin || name.key.contains('.') {
                return Vec::new();
            }
            vec![Pending::new(
                PARAM_OR_MEASURE,
                name,
                scope,
                RefContext::Expression,
            )]
        }
        ExprKind::Unary { operand, .. } => expr_refs(operand, scope, formals),
        ExprKind::Binary { left, right, .. } => expr_refs(left, scope, formals)
            .into_iter()
            .chain(expr_refs(right, scope, formals))
            .collect(),
        ExprKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => [condition, then_branch, else_branch]
            .into_iter()
            .flat_map(|part| expr_refs(part, scope, formals))
            .collect(),
        ExprKind::Call { name, args } => call_expr_refs(name, args, scope, formals),
        ExprKind::Number(_) | ExprKind::Error => Vec::new(),
    }
}

/// Collects references of a function call inside an expression.
///
/// # Arguments
/// * `name` - function name
/// * `args` - call arguments
/// * `scope` - scope of the expression
/// * `formals` - formal arguments of the enclosing user function
fn call_expr_refs(name: &Ident, args: &[Expr], scope: ScopeId, formals: &[&str]) -> Vec<Pending> {
    let key = name.key.as_str();
    if matches!(key, "str" | "def" | "val" | "valm" | "valp") {
        return Vec::new();
    }
    if let Some(argument) = output_argument(key) {
        let kinds = match argument {
            OutputArgKind::Nodes => NODE,
            OutputArgKind::Element => ELEMENT,
            OutputArgKind::Expression => {
                return args
                    .iter()
                    .flat_map(|arg| expr_refs(arg, scope, formals))
                    .collect();
            }
            OutputArgKind::Other => return Vec::new(),
        };
        return args
            .iter()
            .filter_map(|arg| match &arg.kind {
                ExprKind::Ident(ident) if !is_ground(&ident.text) => {
                    Some(Pending::new(kinds, ident, scope, RefContext::Output))
                }
                _ => None,
            })
            .collect();
    }
    let builtin = find_function(key).is_some() || is_optimization_function(key);
    let own = (!builtin).then(|| Pending::new(FUNCTION, name, scope, RefContext::Expression));
    own.into_iter()
        .chain(args.iter().flat_map(|arg| expr_refs(arg, scope, formals)))
        .collect()
}

/// Returns, for each node symbol, how many element terminals connect to it.
///
/// # Arguments
/// * `references` - resolved references
pub fn connection_counts(references: &[Reference]) -> HashMap<SymbolId, usize> {
    references
        .iter()
        .filter(|reference| {
            matches!(
                reference.context,
                RefContext::Terminal { .. } | RefContext::ControlNode { .. }
            )
        })
        .filter_map(|reference| reference.target)
        .fold(HashMap::new(), |mut counts, target| {
            *counts.entry(target).or_insert(0) += 1;
            counts
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::blocks::build_structure;
    use crate::syntax::{lower::lower, parse::parse_tree};

    fn resolve_text(text: &str) -> (Netlist, Resolution) {
        let tree = parse_tree(text).expect("parser");
        let netlist = lower(&tree, text);
        let structure = build_structure(&netlist);
        let resolution = resolve(&netlist, &structure, &[]);
        (netlist, resolution)
    }

    fn unresolved(resolution: &Resolution) -> Vec<String> {
        resolution
            .references
            .iter()
            .filter(|reference| reference.target.is_none())
            .map(|reference| reference.name.key.clone())
            .collect()
    }

    #[test]
    fn resolves_params_models_and_subckts() {
        let text = "* t\n.param wn=1u wp='2*wn'\n.model nch.1 nmos level=54\n.subckt inv in out vdd w=1u\nM1 out in 0 0 nch W=w\n.ends inv\nX1 a b vdd inv w=wp\nR1 a b missing\n";
        let (_, resolution) = resolve_text(text);
        assert_eq!(unresolved(&resolution), vec!["missing"]);
    }

    #[test]
    fn nodes_are_scoped_and_ground_is_skipped() {
        let text = "* t\n.global vdd\n.subckt s a\nR1 a n1 1k\nR2 n1 vdd 1k\n.ends\nR3 n1 0 1k\n";
        let (_, resolution) = resolve_text(text);
        let nodes: Vec<(String, ScopeId)> = resolution
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == SymbolKind::Node)
            .map(|symbol| (symbol.name.key.clone(), symbol.scope))
            .collect();
        assert!(nodes.contains(&("vdd".to_string(), 0)));
        assert!(nodes.contains(&("n1".to_string(), 1)));
        assert!(nodes.contains(&("n1".to_string(), 0)));
        assert!(!nodes.iter().any(|(name, _)| name == "0"));
        let vdd_in_subckt = resolution
            .references
            .iter()
            .find(|reference| reference.name.key == "vdd" && reference.scope == 1);
        assert!(
            vdd_in_subckt
                .and_then(|reference| reference.target)
                .is_some()
        );
    }

    #[test]
    fn user_function_formals_and_measures_resolve() {
        let text = "* t\n.param f(x)='x*2' a='f(3)'\n.meas tran t1 trig v(in) val=0.5 rise=1 targ v(out) val=0.5 fall=1\n.meas tran t2 param='t1*2'\nVin in 0 1\nR1 in out 1k\nC1 out 0 1p\n";
        let (_, resolution) = resolve_text(text);
        assert!(
            unresolved(&resolution).is_empty(),
            "{:?}",
            unresolved(&resolution)
        );
    }

    #[test]
    fn sweeps_and_controls_resolve() {
        let text = "* t\nVin in 0 1\nVs a 0 0\nF1 b 0 Vs 2\nL1 a b 1n\nL2 b 0 1n\nK1 L1 L2 0.9\n.dc vin 0 1 0.1\n.noise v(b) vin\n";
        let (_, resolution) = resolve_text(text);
        assert!(
            unresolved(&resolution).is_empty(),
            "{:?}",
            unresolved(&resolution)
        );
    }

    #[test]
    fn counts_connections() {
        let text = "* t\nR1 a b 1k\nR2 b 0 1k\n";
        let (_, resolution) = resolve_text(text);
        let counts = connection_counts(&resolution.references);
        let node_a = resolution
            .symbols
            .iter()
            .position(|symbol| symbol.kind == SymbolKind::Node && symbol.name.key == "a");
        assert_eq!(node_a.and_then(|id| counts.get(&id)), Some(&1));
    }
}
