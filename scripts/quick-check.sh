#!/usr/bin/env bash
# Run only the check steps the changed files need (tests/tags.toml), and log
# the time in notes/check-times.md. ./scripts/check.sh stays the check before
# a handoff, a deploy or a publish. scripts/quick_check.py says more.
#
#   ./scripts/quick-check.sh [BASE] [--dry-run]
#   ./scripts/quick-check.sh --full
set -euo pipefail
cd "$(dirname "$0")/.."
PY="${BUILDER_PYTHON:-}"
if [ -z "$PY" ]; then
  if [ -x .venv/bin/python ]; then PY=.venv/bin/python; else PY=python3; fi
fi
exec "$PY" scripts/quick_check.py "$@"
