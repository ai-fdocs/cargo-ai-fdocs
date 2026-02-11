#!/usr/bin/env bash
set -euo pipefail

FDOCS_DIR="${1:-fdocs}"

if [[ ! -e "$FDOCS_DIR" ]]; then
  echo "nothing to clean: '$FDOCS_DIR' does not exist"
  exit 0
fi

if [[ ! -d "$FDOCS_DIR" ]]; then
  echo "error: '$FDOCS_DIR' exists but is not a directory" >&2
  exit 1
fi

rm -rf -- "$FDOCS_DIR"
echo "removed '$FDOCS_DIR'"
