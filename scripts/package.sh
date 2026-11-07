#!/usr/bin/env bash
# Stage the Neovim plugin together with a server binary and a compiled tree-sitter parser.
# Usage: scripts/package.sh <name> <server-binary> <parser-library>
# Output: dist/hspice.nvim-<name>.tar.gz, or dist/hspice.nvim-<name>.zip for Windows binaries (*.exe)
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <name> <server-binary> <parser-library>" >&2
  exit 1
fi

name="$1"
binary="$2"
parser="$3"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

stage_dir="dist/hspice.nvim"
rm -rf "$stage_dir"
mkdir -p "$stage_dir/bin" "$stage_dir/parser"

for item in plugin lsp lua ftplugin queries README.md LICENSE; do
  cp -R "$item" "$stage_dir/"
done
cp "$binary" "$stage_dir/bin/"
cp "$parser" "$stage_dir/parser/"
chmod +x "$stage_dir/bin/$(basename "$binary")"

if [[ "$binary" == *.exe ]]; then
  archive="hspice.nvim-${name}.zip"
  (cd dist && 7z a -tzip "$archive" hspice.nvim > /dev/null)
else
  archive="hspice.nvim-${name}.tar.gz"
  tar -czf "dist/$archive" -C dist hspice.nvim
fi

echo "Packaged dist/$archive"
