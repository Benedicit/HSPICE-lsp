#!/usr/bin/env bash
# Package the VS Code extension together with a server binary.
# Usage: scripts/package-vscode.sh <vscode-target> <server-binary>   (target: linux-x64, darwin-arm64, win32-x64, …)
# Output: dist/hspice-vscode-<vscode-target>.vsix
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <vscode-target> <server-binary>" >&2
  exit 1
fi

target="$1"
binary="$2"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

rm -rf vscode/server
mkdir -p vscode/server dist
cp "$binary" vscode/server/
chmod +x "vscode/server/$(basename "$binary")"
cp LICENSE vscode/LICENSE

cd vscode
npm ci
npx vsce package --target "$target" --out "../dist/hspice-vscode-${target}.vsix"

echo "Packaged dist/hspice-vscode-${target}.vsix"
