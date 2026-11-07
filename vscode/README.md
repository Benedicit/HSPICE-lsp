# HSPICE for VS Code

Language support for Synopsys HSPICE netlists, powered by the `hspice-lsp` language server:
diagnostics, completion, hover, go to definition, references, rename, symbols, folding, semantic
highlighting, inlay hints, signature help and quick fixes.

Platform packages bundle the server. Files ending in `.sp`/`.spice` open as HSPICE, and `.inc`/`.pm`
files do too when one of their first 100 lines starts with an HSPICE dot command.

Settings live under `hspice.*` (see the Settings UI). `hspice.server.path` points to another server binary.
Run **HSPICE: Restart Language Server** after replacing it.

See the [project README](https://github.com/Benedicit/HSPICE-lsp#readme) for details.

HSPICE is a trademark of Synopsys, Inc. This project is not affiliated with Synopsys.
