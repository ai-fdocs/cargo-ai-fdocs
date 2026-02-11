#!/usr/bin/env bash
set -euo pipefail

if [[ -f "Cargo.toml" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo is not installed" >&2
    exit 1
  fi

  if [[ ! -f "ai-fdocs.toml" ]]; then
    echo "ai-fdocs.toml not found; generating with: cargo ai-fdocs init"
    cargo ai-fdocs init
  fi

  cargo ai-fdocs sync
  exit 0
fi

if [[ -f "package.json" ]]; then
  if [[ ! -f "npn/dist/cli.js" ]]; then
    echo "building npm ai-fdocs CLI (npn/dist/cli.js)..."
    (cd npn && npm run build)
  fi

  if [[ ! -f "ai-fdocs.toml" ]]; then
    echo "ai-fdocs.toml not found; generating with npm ai-fdocs init"
    node npn/dist/cli.js init --overwrite
  fi

  node npn/dist/cli.js sync
  exit 0
fi

echo "error: neither Cargo.toml nor package.json found in current directory" >&2
exit 1
