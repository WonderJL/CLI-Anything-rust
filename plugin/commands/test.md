# cli-anything-rs:test

**Trust the artifact, not the exit code.** Run a harness's full test suite —
offline units, gated real-backend E2E, and the quality gate — then report the
honest counts. A skipped E2E is never a pass.

## Usage

```
/cli-anything-rs:test [crate-name]
```

## What this does

1. Offline tests: `cargo test -p cli-anything-<name>` (or `--workspace` if no
   crate is given). These must pass with no network and no real backend.
2. E2E tests (need the real backend): `cargo test -p cli-anything-<name> -- --ignored`.
   Report clearly if a backend is missing — a gated/skipped E2E is NOT a pass.
3. Quality gate: `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all --check`, `cargo deny check`.
4. Report the real counts (passed / failed / ignored) and any backend that
   prevented an E2E from running. Never report success when tests failed or were
   silently skipped.
