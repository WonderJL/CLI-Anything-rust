#!/usr/bin/env bash
# Local CI-equivalent gate for CLI-Anything-rust.
# Mirrors the per-phase definition-of-done: build + clippy + fmt + supply-chain.
#
# Usage: scripts/check.sh
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
step() {
  local name="$1"; shift
  echo "── $name"
  if "$@"; then
    echo "   ✅ pass"
  else
    echo "   ❌ FAIL ($name)"
    fail=1
  fi
}

step "build"  cargo build --workspace
step "clippy" cargo clippy --workspace --all-targets -- -D warnings
step "fmt"    cargo fmt --all --check
if command -v cargo-deny >/dev/null 2>&1; then
  step "deny" cargo deny check
else
  echo "── deny"
  echo "   ⚠️  skipped — install with: brew install cargo-deny  (or cargo install cargo-deny --locked)"
fi

echo ""
if [ "$fail" -eq 0 ]; then
  echo "ALL GREEN ✅"
else
  echo "FAILURES ❌"
fi
exit "$fail"
