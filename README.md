# hspice.nvim

A language server for HSPICE netlists, with a Neovim plugin and a VS Code extension. The server (`hspice-lsp`) is written in Rust on top of a tree-sitter grammar. The Neovim plugin wires it into Neovim's native LSP client; the VS Code extension bundles it.

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
  - nodes and elements in output variables that do not exist (`.print v(out)`, `i(R1)`, `v(x1.out)`)
  - unknown, obsolete or foreign-dialect commands and options, unknown `.MOSRA` parameters
  - `.MOSRA` combined with `SWEEP`/`MONTE`, `.DC`/`.AC` (both need `SIMMODE=1`) or no `.TRAN`, and MOSRA models changed in `.ALTER`
  - `10M` vs `10MEG`, unused parameters, capacitors of 1 F or inductors of 0.1 H and more
  - values outside the documented set (`.option method=`, `.MOSRA simmode=`, `.fft window=` ...), options that RUNLVL ignores
  - shorted two-terminal elements, R/C/L without value, multipliers `M<=0`
  - source functions: too few arguments, negative times, PWL time points that go backwards
  - `data=` without `.data` block, nodes initialised twice by `.ic`/`.nodeset`, `.print`/`.probe`/`.ic` without arguments
  - output signals `v()`/`i()` in `.param` or `.measure PARAM=`, `.measure` keywords used as parameter names, `.elseif`/`.else` after `.else`
- **Completion**: dot commands and element templates (snippets, HSPICE keywords in uppercase), subckts, models matching the element type, nodes, parameters, functions, instance/model parameters, source functions, `.option` names, analysis and `.measure` keywords, output variables, include paths and `.lib` sections, documented values after `key=`, `.data` names after `data=`, the block name after `.ends`/`.endl`, and the arguments of `.mosraprint`, `.appendmodel` and `.mosra_subckt_pin_volt`.
- **Hover**: command synopses with argument descriptions, element terminal mapping, parameter values (evaluated), subckt and model cards, node connections, number values (`10M` = 1e-2).
- **Navigation**: go to definition (including into `.include`/`.lib` files and all model bins), find references, document highlight, rename.
- **Structure**: document and workspace symbols, folding ranges (comment runs only with `folding.comments`, `+` continued statements only with `folding.continuations`).
- **Highlighting**: semantic tokens on top of tree-sitter (Neovim) or a basic TextMate grammar (VS Code).
- **Editing help**: signature help, inlay hints (subckt port and terminal names, source-function arguments), quick fixes (`MEG`, add `.param`, insert closers and `.end`, "did you mean …").

The grammar and the semantics follow the HSPICE manuals; see `docs/hspice-reference.md`. `docs/architecture.md` describes the server internals.

## Requirements

- Neovim 0.11 or newer (developed against 0.12), or VS Code 1.91 or newer
- Prebuilt packages (Linux x86_64/aarch64, macOS arm64, Windows x86_64/arm64), or a Rust toolchain plus a C compiler for local builds

## Installation

### Prebuilt packages

Every push to `main` runs the CI pipeline (`.github/workflows/ci.yml`). It builds the Neovim plugin (`hspice.nvim-<platform>`) and the VS Code extension (`hspice-vscode-<target>`) for Linux, macOS and Windows. Publishing a GitHub release attaches these files to the release. Download them from the release page or from the workflow run (`gh run download -n hspice.nvim-linux-x86_64`).

For Neovim, unpack the archive anywhere and point lazy.nvim at the extracted `hspice.nvim` folder:

```lua
{ dir = "/path/to/hspice.nvim", lazy = false, opts = {} }
```

### Local build (macOS or Linux with Rust)

```sh
scripts/build-local.sh
```

This builds `bin/hspice-lsp` and `parser/hspice.so`. If `tree-sitter-hspice/grammar.js` is newer than `src/parser.c`, the parser is regenerated first, which needs the `tree-sitter` CLI. Then use the checkout as the plugin directory: `{ dir = "/path/to/HSPICE_lsp", lazy = false, opts = {} }`.

### Windows

Neovim: extract `hspice.nvim-windows-x86_64.zip` (`hspice.nvim-windows-arm64.zip` on ARM devices), e.g. to `%LOCALAPPDATA%\nvim-data\hspice.nvim`, and use forward slashes in the lazy.nvim spec:

```lua
{ dir = "C:/Users/<you>/AppData/Local/nvim-data/hspice.nvim", lazy = false, opts = {} }
```

VS Code: install `hspice-vscode-win32-x64.vsix` (`hspice-vscode-win32-arm64.vsix` on ARM devices, see below).

To build locally, install the Rust MSVC toolchain, the Visual Studio C++ Build Tools and the tree-sitter CLI (`npm install -g tree-sitter-cli@0.27.1`), then run in PowerShell from the checkout:

```powershell
cargo build --release -p hspice-lsp
New-Item -ItemType Directory -Force bin, parser | Out-Null
Copy-Item target\release\hspice-lsp.exe bin\
tree-sitter build --output parser\hspice.dll tree-sitter-hspice
```

Include paths may use `\` or `/`. A leading `~` expands to `%USERPROFILE%` when `HOME` is not set.

### VS Code

Install the package for your platform (`hspice-vscode-linux-x64.vsix`, `-linux-arm64`, `-darwin-arm64`, `-win32-x64` or `-win32-arm64`) with **Extensions: Install from VSIX…** or `code --install-extension hspice-vscode-<target>.vsix`. It bundles the server.

To package it yourself (Node.js 22+, Git Bash on Windows), build the server and run e.g. `scripts/package-vscode.sh darwin-arm64 bin/hspice-lsp`. The `.vsix` ends up in `dist/`.

## Configuration

Neovim: `opts` is passed to `require("hspice").setup(opts)`:

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
    folding = { comments = false, continuations = false },
    maxIncludeFileSizeMb = 64,
  },
}
```

VS Code: the same settings live under `hspice.` (e.g. `"hspice.includePaths": ["/eda/pdk"]`). In addition, `hspice.server.path` replaces the bundled server and `hspice.trace.server` logs the LSP traffic.

Settings are sent as `initializationOptions` and as `settings.hspice` in `workspace/didChangeConfiguration`.

| Setting | Default | Meaning |
| --- | --- | --- |
| `includePaths` | `{}` | Extra directories searched for `.include` and `.lib` files |
| `diagnostics.floatingNodes` | `true` | Report nodes connected to a single terminal |
| `diagnostics.unusedParams` | `true` | Report unused `.param` definitions |
| `diagnostics.unknownOptions` | `true` | Report unknown `.option` names, `.MOSRA` parameters and undocumented values |
| `diagnostics.megSuffix` | `true` | Report suspicious `m`/`meg` suffix use |
| `inlayHints.ports` | `true` | Show port names on subcircuit instances |
| `inlayHints.sourceArgs` | `true` | Show argument names on source instances |
| `folding.comments` | `false` | Fold runs of consecutive comment lines |
| `folding.continuations` | `false` | Fold statements continued over several lines with `+` |
| `workspaceScan.enabled` | `true` | Index other netlists in the workspace |
| `workspaceScan.maxFiles` | `500` | Maximum number of files scanned |
| `maxIncludeFileSizeMb` | `64` | Skip included files larger than this |

The Neovim plugin creates no keymaps. Use your own `LspAttach` mappings.

## File types

Neovim already maps `.sp` and `.spice` to the filetype `spice`. The plugin also detects `.inc` and `.pm` files as `spice` when one of the first 100 lines starts with a known HSPICE dot command (`.subckt`, `.model`, `.param`, `.include`, `.lib`, …). Otherwise Neovim's built-in detection applies, so Perl modules and C/Pascal include files keep their filetypes. The tree-sitter parser `hspice` is registered for the `spice` filetype.

VS Code opens `.sp` and `.spice` as HSPICE and applies the same `.inc`/`.pm` check once per file, so choosing another language mode afterwards sticks.

## Troubleshooting

- `:checkhealth hspice` checks the binary, parser, queries and filetype registration.
- `:checkhealth vim.lsp` (or `:LspInfo`) shows active clients and their configuration.
- `:LspLog` opens the client log.
- If the binary is missing, rerun `scripts/build-local.sh` or reinstall the package.
- VS Code: the **HSPICE** output channel shows the server log, and **HSPICE: Restart Language Server** restarts the server.

## Architecture

- `tree-sitter-hspice/`: tree-sitter grammar for HSPICE.
- `hspice-lsp/`: the Rust language server. It uses `#![forbid(unsafe_code)]`. The only `unsafe` in the workspace is the two-line generated FFI declaration in `tree-sitter-hspice/bindings/rust/lib.rs`, which tree-sitter requires.
- `plugin/`, `lsp/`, `lua/`, `ftplugin/`, `queries/`: the Neovim plugin (filetype detection, native LSP config, health check, highlight queries).
- `vscode/`: the VS Code extension (language client, `.inc`/`.pm` detection, TextMate grammar, smoke test).
- `scripts/`: local build, packaging (used by CI) and the headless Neovim smoke test.
- `.github/workflows/ci.yml`: the CI pipeline. Every push and pull request runs the tests: generated-parser check, grammar corpus, rustfmt, clippy, unit and end-to-end tests, and the Neovim and VS Code smoke tests. Pushes to `main` additionally build the Linux, macOS and Windows packages in parallel.
- `.github/workflows/release.yml`: publishing a GitHub release attaches the packages from the CI run of the released commit. That commit must have been pushed to `main`, and CI artifacts expire after 90 days.

## Development

```sh
cargo test
(cd tree-sitter-hspice && tree-sitter test)
XDG_STATE_HOME=$TMPDIR XDG_DATA_HOME=$TMPDIR XDG_CACHE_HOME=$TMPDIR \
  nvim --headless --clean -u NONE -l scripts/nvim-smoke.lua
(cd vscode && npm ci && npm run check && mkdir -p server && cp ../bin/hspice-lsp server/ && npm test)
```

The Neovim smoke test needs `bin/hspice-lsp` and `parser/hspice.so` from `scripts/build-local.sh`. It starts the server on `hspice-lsp/tests/fixtures/inverter_tb.sp` and checks attach, diagnostics, completion, hover and go-to-definition. Pass another fixture path as an argument to override the default.

The VS Code smoke test (`vscode/test/`) downloads VS Code, or uses the one in `VSCODE_EXECUTABLE`, and checks the same features plus `.inc`/`.pm` detection.

## License

MIT, see `LICENSE`. HSPICE is a trademark of Synopsys, Inc. This project is not affiliated with Synopsys.
