#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KOGWISTAR_DIR="$REPO_ROOT/kogwistar"
KOGWISTAR_GIT_URL="https://github.com/humblemat810/kogwistar.git"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command '$1' was not found" >&2
    exit 1
  fi
}

require_cmd git
require_cmd python
python -m pip --version >/dev/null 2>&1

cd "$REPO_ROOT"

echo "Installing the sink package from the repo root..."
python -m pip install -e . --no-deps

if [ ! -d "$KOGWISTAR_DIR" ]; then
  echo "Local ./kogwistar subtree is missing; cloning from GitHub..."
  git clone "$KOGWISTAR_GIT_URL" "$KOGWISTAR_DIR"
fi

if [ ! -f "$KOGWISTAR_DIR/pyproject.toml" ]; then
  echo "error: '$KOGWISTAR_DIR' does not look like a Kogwistar checkout" >&2
  exit 1
fi

echo "Installing local kogwistar subtree editable into the active environment..."
python -m pip install -e "$KOGWISTAR_DIR"

echo "Bootstrap complete."
