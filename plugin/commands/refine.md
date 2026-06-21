# cli-anything-rs:refine

Expand an existing Rust CLI harness's coverage toward the software's full
capabilities.

## Usage

```
/cli-anything-rs:refine <crate-name-or-path> [focus]
```

## What this does

1. Read `HARNESS-rs.md` and the target crate (`crates/cli-anything-<name>`).
2. Compare the software's real capabilities against the current command surface;
   identify gaps (optionally narrowed by `[focus]`, e.g. "gradients and filters").
3. Add new clap subcommands + domain ops + backend calls, following the same
   conventions (`--json` envelope, `core::Session` undo, safe subprocess/XML).
4. Re-run Phase 3.5 security checklist for any new backend/parsing paths.
5. Add unit + (gated) E2E tests for the new commands and verify output.
6. Regenerate `SKILL.md` (`emit-skill`).
7. Gate: `cargo build` + `clippy -D warnings` + `fmt --check` + `cargo deny check` green.

Do not break existing commands or the envelope contract.
