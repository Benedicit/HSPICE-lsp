# hspice-lsp architecture

```
text ──► syntax::parse (tree-sitter, title lines masked) ──► syntax::lower (CST → typed AST)
     ──► analysis::analyze (blocks → definitions → element roles → nodes → references)
     ──► analysis::diagnostics::compute ──► features::* (pure queries) ──► LSP responses
```

The server is a single-threaded `lsp-server` loop (`server/mod.rs`). Text sync is *Full*: every
change re-parses and re-analyses the document (milliseconds for netlists) and re-analyses open
documents that depend on it. Everything below `server/` is safe, synchronous and side-effect free
except `state/workspace.rs` (file reads for includes, workspace scan).

Clients: the Neovim plugin (`lsp/hspice.lua`) and the VS Code extension (`vscode/src/extension.ts`)
start the same binary over stdio. Only `file:` URIs have a path (`state::workspace::file_path`).

## Crates and modules (`hspice-lsp/src`)
| Module | Purpose |
|---|---|
| `syntax/ast.rs` | Typed AST: `Netlist { statements }`, `Statement { kind, range }`, `StatementKind::{Comment, Element, Command, Error}`, `Element { name, args }`, `Command { keyword, kind: CommandKind }`, `Arg::{Value, Assign, Group}`, `Value::{Number, Word, Expr, Str, Call}`, `Expr`/`ExprKind`, `Ident { text, key (lowercase), range }`, `TextRange { start, end }` (byte offsets; `touches(offset)` = cursor semantics). |
| `syntax/parse.rs` | `parse_tree`, `parse_tree_in_ranges`, `title_ranges`, `mask_ranges`, `has_end_statement`, `syntax_errors`. |
| `syntax/lower.rs` | `lower(tree, text) -> Netlist`, `lower_expr`. |
| `syntax/node_kinds.rs` | Node-kind / field-name constants of the grammar (use these, never string literals). |
| `syntax/number.rs` | `parse_number("1meg")`, `format_engineering(1e6) == "1meg"`. |
| `analysis/blocks.rs` | `Structure { blocks, scopes, statement_scope, statement_section, statement_alter, skipped, opaque, unmatched }`; `Block { kind, open, close, name, close_name, range }`. |
| `analysis/symbols.rs` | `Symbol { kind, name, scope, origin, definition_range, statement, section, detail }`, `SymbolKind`, `SymbolDetail`, `Origin::{Local, External(Arc<ExternalFile { path, text, line_index }>)}`, `Reference { kind, name, scope, context: RefContext, statement, target }`, `Scope`, `SymbolTable`, `ROOT_SCOPE`. |
| `analysis/roles.rs` | `ElementRoles { nodes, control_nodes, reference, controls, values }` (which positional args are nodes / model / subckt / controlling sources). |
| `analysis/resolve.rs` | `ElementInfo { statement, name, scope, spec, roles, skipped }`; definition & reference collection. |
| `analysis/includes.rs` | `FileSummary`, `IncludeDirective`, path expansion/resolution. |
| `analysis/hierarchy.rs` | `lookup_path(analysis, scope, "x1.x2.out", kind, unchecked) -> PathLookup::{Found, Missing, Unknown}` through local subcircuits. |
| `analysis/mod.rs` | `Analysis { structure, symbols, table, elements, references, includes, context_complete }` with `lookup`, `scope_at`, `reference_at`, `definition_at`, `symbol_at`, `occurrences`, `visible_symbols`, `element_at_statement`, `connection_counts`; free fn `statement_at(netlist, offset)`. |
| `analysis/diagnostics/` | `Diagnostic { range, severity, code, message, unnecessary, deprecated }`, `compute(syntax, analysis, settings)`. |
| `analysis/eval.rs` | `evaluate(expr, lookup)` constant folding for hover. |
| `state/document.rs` | `Document { uri, path, version, text, line_index, syntax: SyntaxInfo { tree, netlist, errors, title_ranges, is_top_level }, analysis, diagnostics }`. The tree was parsed from the title-masked text; byte offsets are identical to `text`. |
| `state/line_index.rs` | `LineIndex` byte ↔ LSP position (UTF-8 / UTF-16). |
| `state/workspace.rs` | `Workspace { root, encoding, settings, … }` with `document(uri)`, `documents()`, `update`, `close`, `saved`, `set_settings`. |
| `features/mod.rs` | Shared helpers: `to_lsp_range`, `to_offset`, `symbol_location`, `location_in_symbol_file`, `definition_text`, `argument_docs`. |
| `features/*.rs` | One file per LSP feature; signature `fn feature(workspace: &Workspace, document: &Document, …) -> Option<LspType>`. |
| `lang/*.rs` | Static HSPICE tables: `find_command`, `find_element`, `find_source_function`, `is_ground`, `find_function`, `find_special_variable`, `is_optimization_function`, `find_model_type`, `mosfet_level_name`, `model_params_for`, `find_option`, `find_mosra_param`, `find_measure_keyword`, `find_analysis_keyword`, `find_output_var`, `output_argument`, tables `COMMANDS`, `ELEMENTS`, `OPTIONS`, `ANALYSIS_TYPES`, … |
| `config.rs` | `Settings { include_paths, diagnostics, inlay_hints, workspace_scan, folding, max_include_file_size_mb }`. |

## Semantics in brief
- Names are case-insensitive: compare `Ident::key`.
- Scopes: `ROOT_SCOPE` (0) plus one per `.subckt`. A `.subckt` statement belongs to its parent
  scope; its ports and header params belong to the body scope (`SymbolDetail::Subckt::body`).
- Node symbols: ports, `.global` nodes (root, `global: true`), then first use per scope. Ground
  (`0 gnd gnd! ground !gnd`) has no symbol and no references.
- Every occurrence of a node (ports, terminals, `.global`, `v(node)`) is a `Reference`; terminal
  references have `RefContext::Terminal { element, terminal }` where `element` indexes
  `Analysis::elements`.
- External symbols (from includes / `.lib` sections / including netlists) come after local ones
  and have `Origin::External`; their `name.range` refers to *their* file. Include paths are
  normalised lexically (no `..`); disk reads go through `includes::read_source` (BOM stripped).
- `Analysis::context_complete == false` means some definitions may be missing (unresolved include,
  include file without a known includer): report "undefined …" more leniently.
- `Structure::skipped[i]` statements (variation blocks, encrypted `.prot` regions, unparsable lines)
  have no semantics; `opaque[i]` ones also get no syntax errors.

## Conventions
Safe Rust (`unsafe_code = "forbid"`), edition 2024, rustfmt defaults, clippy clean, no
`unwrap`/`expect` outside tests, functional style (iterator chains, pure functions), every
function documented with a 1–2 sentence summary plus a rustdoc `# Arguments` list.
