# cli-anything-rs

**Make any software agent-native.** Build a complete Rust CLI harness for a
piece of software — a thin, structured interface *to* the real tool (never a
reimplementation), on top of the shared `cli-anything-core` crate. The result
is one-shot + REPL, `--json` on every command, and a generated `SKILL.md` an
agent can discover.

## CRITICAL: read HARNESS-rs.md first

**Before doing anything, read `HARNESS-rs.md` at the cli-anything-rust repo
root.** It defines the methodology, the security checklist, and the Rust
conventions. Every phase below follows it. Do not improvise.

## Usage

```
/cli-anything-rs <software-path-or-repo>
```

`<software-path-or-repo>` is a local path to the software's source or a GitHub
URL (clone it first, then analyze the local copy). A bare name is not accepted —
you must analyze real source to design the command surface.

## What this does (follows HARNESS-rs.md)

### Phase 1 — Codebase analysis
Analyze the source: backend engine, data model, existing CLI (`mmdc`,
`inkscape`, …), and GUI→API/CLI mappings.

### Phase 2 — CLI architecture design
Design clap command groups mirroring the software's domains; plan the serde
state model, undo/redo on `core::Session`, and the `--json` envelope payloads.

### Phase 3 — Implementation
1. Scaffold: `cargo run -p cli-anything-new -- --software <name> [--no-preview]`
   (or the installed `cli-anything-new`). This stamps a compilable crate.
2. Fill ONLY the domain logic: `src/domain/*` (state + ops), `src/cli.rs` (real
   command tree), `src/backend.rs` (real subprocess/HTTP via
   `core::security::subprocess`), `src/repl_cmds.rs` (dispatch).
3. Sync only — `std::process::Command` and blocking `ureq`; no tokio.

### Phase 3.5 — Security checklist (MANDATORY)
Satisfy every item in the HARNESS-rs.md checklist: no shell strings;
`read_svg_safely` for untrusted XML; `guard_project_path` on all load/save;
`#![forbid(unsafe_code)]`; `cargo deny check` green; typed error + install hint
when the backend is missing; timeouts on every subprocess call.

### Phase 4–5 — Tests
`tests/unit.rs` (offline: model, builders, verify helpers, security
accept/reject) and `tests/e2e.rs` (`#[ignore]`, real backend, verify output by
magic bytes/format).

### Phase 6 — SKILL.md
`cli-anything-<name> emit-skill -o crates/cli-anything-<name>/SKILL.md` (walks
the clap tree — never drifts).

### Phase 7 — Distribution
`cargo install --path crates/cli-anything-<name>`. `publish = false` until ready.

## Success criteria

`cargo build` + `cargo clippy -- -D warnings` + `cargo fmt --check` +
`cargo deny check` all green; one-shot + REPL work; `--json` everywhere; real
backend verified; security checklist satisfied; `SKILL.md` generated.

## Related commands

- `/cli-anything-rs:refine <crate>` — expand an existing harness's coverage.
- `/cli-anything-rs:test <crate>` — run its tests (incl. `--ignored` E2E).
- `/cli-anything-rs:validate <crate>` — check it against HARNESS-rs.md.
- `/cli-anything-rs:list` — list the CLIs in the workspace.
