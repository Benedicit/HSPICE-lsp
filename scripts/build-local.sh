#!/usr/bin/env bash
# Build the server and tree-sitter parser for the current machine.
# Usage: scripts/build-local.sh
# Output: bin/hspice-lsp and parser/hspice.so
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

grammar_dir="tree-sitter-hspice"
parser_src="$grammar_dir/src/parser.c"

if [[ ! -f "$parser_src" || "$grammar_dir/grammar.js" -nt "$parser_src" ]]; then
  echo "Regenerating parser"
  (cd "$grammar_dir" && tree-sitter generate)
fi

echo "Building hspice-lsp"
cargo build --release -p hspice-lsp
mkdir -p bin parser
cp target/release/hspice-lsp bin/hspice-lsp

sources=("$parser_src")
if [[ -f "$grammar_dir/src/scanner.c" ]]; then
  sources+=("$grammar_dir/src/scanner.c")
fi

case "$(uname -s)" in
  Darwin) shared_flags=(-dynamiclib) ;;
  *) shared_flags=(-shared -fPIC) ;;
esac

echo "Compiling parser"
cc -std=c11 -O2 -I "$grammar_dir/src" "${shared_flags[@]}" "${sources[@]}" -o parser/hspice.so

echo "Done: bin/hspice-lsp, parser/hspice.so"
