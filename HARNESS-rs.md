# HARNESS-rs — the standard playbook for building agent-native Rust CLIs

> **Today's software serves humans. Tomorrow's users will be agents.**

This is the Rust adaptation of CLI-Anything's `HARNESS.md` — the standard
operating procedure for turning a piece of real software into a
**production-ready, agent-native CLI** on top of the shared
[`cli-anything-core`](crates/cli-anything-core) crate. The goal is a thin,
structured interface *to* the real tool — never a dumbed-down reimplementation.
Follow it phase by phase; do not improvise.

## The one non-negotiable rule

**Use the real software. Do not reimplement it.** The generated CLI is a *thin
harness* that drives the real tool (e.g. `inkscape`, `mmdc`, `ffmpeg`) via the
safe subprocess helper and **verifies the real output**. If the backend is
missing, fail loudly with a typed error and install instructions — never a
silent fake-render fallback.

## Mental model: three layers

1. **Agent brain** — this document + the `/cli-anything-rs` slash command. The
   agent reads the target software's source, designs the command surface, and
   fills domain logic.
2. **Deterministic scaffolder** — `cli-anything-new --software <name>` stamps a
   compilable crate (clap skeleton, reedline REPL, `--json` envelope, auto-save,
   `emit-skill` hook). You never hand-write boilerplate.
3. **Shared substrate** — `cli-anything-core`: session/undo, the `--json`
   envelope, the reedline skin, the full preview/trajectory subsystem, and the
   safe-by-default utilities (no-shell subprocess, entity-safe XML, path guard).

## Rust divergences from the Python original (call these out)

| Concern | Python original | Rust harness |
|---|---|---|
| CLI parser | Click | **clap (derive)** |
| REPL | prompt_toolkit | **reedline** |
| Code reuse | copy `repl_skin.py` into each | **one shared `cli-anything-core` crate** |
| SKILL.md | regex over Click decorators | **walk the clap `Command` tree** (`emit-skill`, never drifts) |
| Async | — | **sync by default** (no tokio); blocking `ureq`/`std::process` |
| Packaging | pip / PEP 420 namespace | **cargo** (`cargo install --path`), crates.io deferred |
| Memory safety | — | **`#![forbid(unsafe_code)]`** in core + every CLI |

## The phases

### Phase 1 — Codebase analysis
Read the target software's source. Identify: the backend engine, the data model,
any existing CLI (`mmdc`, `inkscape`, `melt`, …), and how GUI actions map to
API/CLI calls. Decide what the agent-usable command surface should be.

### Phase 2 — CLI architecture design
Design command groups that mirror the software's domains. Plan the serde state
model (saved as JSON), the undo/redo on top of `core::Session`, and the `--json`
envelope payloads. REPL is the default (no subcommand → REPL).

### Phase 3 — Implementation
1. Run the scaffolder: `cli-anything-new --software <name> [--no-preview]`
   (or `cargo run -p cli-anything-new -- --software <name>`).
2. Fill **only** the domain logic:
   - `src/domain/*` — the state model + operations (serde structs).
   - `src/cli.rs` — the real clap command tree.
   - `src/backend.rs` — real subprocess/HTTP calls via `core::security::subprocess`.
   - `src/repl_cmds.rs` — dispatch each command to a state op or the backend.
3. Subprocess is **sync** (`std::process::Command` via `core`); blocking HTTP via
   `ureq`. No tokio.
4. Auto-save + `--dry-run` come free via `core::AutoSaveGuard`; one-shot handlers
   should call `guard.commit()` to surface save errors.

### Phase 3.5 — Security checklist (Rust-specific, MANDATORY)
Every CLI must satisfy all of these before it passes. Each maps to a core API:

- [ ] **No shell strings** — all subprocess via `core::security::subprocess::run(program, &args, timeout)`.
- [ ] **Untrusted XML/files bounded** — parse via `core::security::xml::read_svg_safely` (DOCTYPE/entity/SSRF rejection, size/depth caps).
- [ ] **Path-traversal guarded** — every load/save through `core::security::path_guard::guard_project_path` (used by `open_project`/`save_project`).
- [ ] **`#![forbid(unsafe_code)]`** at the top of `lib.rs`/`main.rs`.
- [ ] **Minimal audited deps** — `cargo deny check` passes (advisories, bans, licenses, sources).
- [ ] **Missing backend → typed error + install hint** (`require_binary`), never a silent fallback.
- [ ] **Timeouts on all subprocess calls** — no unbounded waits.

### Phase 4 — Test planning
Plan unit tests (synthetic, offline), E2E tests (real backend, output
verification), and the security tests (attack inputs must be rejected).

### Phase 5 — Test implementation
- `tests/unit.rs` — model round-trip, output-builders, verification helpers, and
  security accept/reject — **all offline**.
- `tests/e2e.rs` — drive the **real backend**, verify output by magic
  bytes/format. `#[ignore]` and gate on backend availability; run with
  `cargo test -- --ignored`.
- Never trust exit code alone — verify the produced artifact.

### Phase 6 — SKILL.md generation
Run `cli-anything-<software> emit-skill -o crates/cli-anything-<software>/SKILL.md`.
It walks the clap `Command` tree, so the skill never drifts from the real surface.

### Phase 7 — Distribution
`cargo build` / `cargo install --path crates/cli-anything-<software>`. Crate
metadata is publish-ready with `publish = false`; crates.io is deferred until the
API settles.

## The `--json` envelope contract

Every command under `--json` prints a uniform `Envelope`:

```json
{ "ok": true, "action": "group.command", "data": { }, "error": null, "warnings": [] }
```

On failure: `ok=false`, `data=null`, and `error = { kind, message, hint }` where
`kind` is a stable snake_case identifier. Exit code is `0` on success, `1` on
error. Errors go to stderr.

## Success criteria

1. `cargo build`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo deny check` all pass.
2. One-shot + REPL both work; `--json` everywhere; auto-save + `--dry-run` correct.
3. Real backend drives real output, verified by magic bytes/format (E2E).
4. Security checklist fully satisfied; attack inputs rejected by tests.
5. `emit-skill` produces a `SKILL.md` consistent with the clap tree.
