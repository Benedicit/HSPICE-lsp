# hspice.nvim

A language server and Neovim plugin for HSPICE netlists. The server (`hspice-lsp`) is written in Rust on top of a tree-sitter grammar. The plugin wires it into Neovim's native LSP client.

## Disclaimer

This LSP is mainly vibe-coded without much human supervision, because writing this LSP would require deep knowledge of HSPICE's AST, functions etc. and time to implement and test everything, which I simply don't have.

Everything is based solely upon publicly available manuals/reference sheets online, which are possibly outdated, contain deprecated features or syntax and lack modern features.

## Features

- **Diagnostics**:
  - syntax errors
  - unclosed or unmatched `.subckt/.lib/.if/.data/.prot` blocks
  - undefined parameters, functions, subcircuits, models, sources and inductors
  - wrong port counts on `X` instances, and model-type mismatches
  - missing nodes, duplicate elements, subckts, models and params
  - built-in function arity, analysis arguments (`.tran`, `.ac`, `.dc`, `.noise`, `.tf`, `.pz`)
  - include files or library sections that are not found
  - floating nodes and missing ground
  - unknown, obsolete or foreign-dialect commands and options
  - `10M` vs `10MEG`, unused parameters
- **Completion**: dot commands and element templates (snippets), subckts, models matching the element type, nodes, parameters, functions, instance/model parameters, source functions, `.option` names, analysis and `.measure` keywords, output variables, include paths and `.lib` sections.
- **Hover**: command synopses, element terminal mapping, parameter values (evaluated), subckt and model cards, node connections, number values (`10M` = 1e-2).
- **Navigation**: go to definition (including into `.include`/`.lib` files and all model bins), find references, document highlight, rename.
- **Structure**: document and workspace symbols, folding ranges.
- **Highlighting**: semantic tokens on top of tree-sitter highlighting.
- **Editing help**: signature help, inlay hints (subckt port and terminal names, source-function arguments), quick fixes (`MEG`, add `.param`, insert closers and `.end`, "did you mean …").

The grammar and the semantics follow the HSPICE manuals; see `docs/hspice-reference.md`. `docs/architecture.md` describes the server internals.

## Requirements

- Neovim 0.11 or newer (developed against 0.12)
- Prebuilt packages from CI (Linux x86_64, macOS arm64, Windows x86_64), or a Rust toolchain plus a C compiler for local builds

## Installation

### Local build (macOS or Linux with Rust)

```sh
scripts/build-local.sh
```

This builds `bin/hspice-lsp` and `parser/hspice.so`. If `tree-sitter-hspice/grammar.js` is newer than `src/parser.c`, the parser is regenerated first, which needs the `tree-sitter` CLI.

lazy.nvim spec:

```lua
{ dir = "/path/to/HSPICE_lsp", lazy = false, opts = {} }
```

### Prebuilt packages

Every push to `main` runs the CI pipeline (`.github/workflows/ci.yml`). It builds the plugin for Linux, macOS and Windows and uploads `hspice.nvim-<platform>` artifacts. Download one from the workflow run page (or with `gh run download -n hspice.nvim-linux-x86_64`), then copy and unpack it

## Configuration

`opts` is passed to `require("hspice").setup(opts)`:

```lua
opts = {
  inlay_hints = true,   -- enable inlay hints on attach
  lsp_folding = true,   -- use LSP folding ranges (window-local foldmethod=expr)
  cmd = nil,            -- override the server command, e.g. { "/path/to/hspice-lsp" }
  settings = {
    includePaths = {},
    diagnostics = { floatingNodes = true, unusedParams = true, unknownOptions = true, megSuffix = true },
    inlayHints = { ports = true, sourceArgs = true },
    workspaceScan = { enabled = true, maxFiles = 500 },
    maxIncludeFileSizeMb = 64,
  },
}
```

Settings are sent as `initializationOptions` and as `settings.hspice` in `workspace/didChangeConfiguration`.

| Setting | Default | Meaning |
| --- | --- | --- |
| `includePaths` | `{}` | Extra directories searched for `.include` and `.lib` files |
| `diagnostics.floatingNodes` | `true` | Report nodes connected to a single terminal |
| `diagnostics.unusedParams` | `true` | Report unused `.param` definitions |
| `diagnostics.unknownOptions` | `true` | Report unknown `.option` names |
| `diagnostics.megSuffix` | `true` | Report suspicious `m`/`meg` suffix use |
| `inlayHints.ports` | `true` | Show port names on subcircuit instances |
| `inlayHints.sourceArgs` | `true` | Show argument names on source instances |
| `workspaceScan.enabled` | `true` | Index other netlists in the workspace |
| `workspaceScan.maxFiles` | `500` | Maximum number of files scanned |
| `maxIncludeFileSizeMb` | `64` | Skip included files larger than this |

The plugin creates no keymaps. Use your own `LspAttach` mappings.

## File types

Neovim already maps `.sp` and `.spice` to the filetype `spice`. The plugin also detects `.inc` and `.pm` files as `spice` when one of the first 100 lines starts with a known HSPICE dot command (`.subckt`, `.model`, `.param`, `.include`, `.lib`, …). Otherwise Neovim's built-in detection applies, so Perl modules and C/Pascal include files keep their filetypes. The tree-sitter parser `hspice` is registered for the `spice` filetype.

## Troubleshooting

- `:checkhealth hspice` checks the binary, parser, queries and filetype registration.
- `:checkhealth vim.lsp` (or `:LspInfo`) shows active clients and their configuration.
- `:LspLog` opens the client log.
- If the binary is missing, rerun `scripts/build-local.sh` or reinstall the tarball.

## Architecture

- `tree-sitter-hspice/`: tree-sitter grammar for HSPICE.
- `hspice-lsp/`: the Rust language server. It uses `#![forbid(unsafe_code)]`. The only `unsafe` in the workspace is the two-line generated FFI declaration in `tree-sitter-hspice/bindings/rust/lib.rs`, which tree-sitter requires.
- `plugin/`, `lsp/`, `lua/`, `ftplugin/`, `queries/`: the Neovim plugin (filetype detection, native LSP config, health check, highlight queries).
- `scripts/`: local build, packaging (used by CI) and the headless smoke test.
- `.github/workflows/ci.yml`: the CI pipeline. Every push and pull request runs the tests: generated-parser check, grammar corpus, rustfmt, clippy, unit and end-to-end tests, and the Neovim smoke test. Pushes to `main` additionally build Linux, macOS and Windows packages in parallel.

## Development

```sh
cargo test
(cd tree-sitter-hspice && tree-sitter test)
XDG_STATE_HOME=$TMPDIR XDG_DATA_HOME=$TMPDIR XDG_CACHE_HOME=$TMPDIR \
  nvim --headless --clean -u NONE -l scripts/nvim-smoke.lua
```

The smoke test needs `bin/hspice-lsp` and `parser/hspice.so` from `scripts/build-local.sh`. It starts the server on `hspice-lsp/tests/fixtures/inverter_tb.sp` and checks attach, diagnostics, completion, hover and go-to-definition. Pass another fixture path as an argument to override the default.
