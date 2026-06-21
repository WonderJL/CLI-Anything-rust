# CLI-Anything-rust

A Rust port of [CLI-Anything](https://github.com/HKUDS/CLI-Anything)'s **one-shot
CLI generation factory** — reproducing *how* it builds agent-native CLIs, with
Rust's memory safety, safe-by-default I/O, and single-binary distribution as the
motivation.

> Status: **complete proof** (Phases A–F). See [`PLAN-cli-anything-rust.md`](PLAN-cli-anything-rust.md)
> for the full design and [`HARNESS-rs.md`](HARNESS-rs.md) for the methodology.

## What this is

The "factory" has three layers, mirrored from the original:

1. **Agent brain** — the [`HARNESS-rs.md`](HARNESS-rs.md) methodology + a Claude
   Code plugin (`/cli-anything-rs`) that analyzes a target software's source and
   designs the command surface.
2. **Deterministic scaffolder** — `cli-anything-new --software <name>` stamps a
   compilable CLI crate (clap skeleton, reedline REPL, core wiring, `--json`
   envelope, auto-save, SKILL.md hook, tests).
3. **Shared substrate** — the [`cli-anything-core`](crates/cli-anything-core)
   crate: session/undo, the `--json` envelope, the reedline skin, the full
   preview/trajectory subsystem, and safe-by-default utilities (no-shell
   subprocess, entity-safe XML, path guards). `#![forbid(unsafe_code)]`.

Two **proof CLIs** validate the factory end-to-end:
- **`mermaid`** — authors a diagram, renders via local `mmdc` (HTTP fallback to
  mermaid.ink), verifies the PNG/SVG by magic bytes.
- **`inkscape`** — builds an SVG document model, exports via the real `inkscape`,
  and safely imports untrusted SVG (the **security showcase**: rejects
  DOCTYPE/billion-laughs/SSRF, accepts safe input).

## End-to-end: how the factory builds a CLI

```bash
# 1. Scaffold a compilable crate (the deterministic half).
cargo run -p cli-anything-new -- --software <name>

# 2. The agent (driven by HARNESS-rs.md / the /cli-anything-rs command) fills
#    ONLY the domain logic: src/domain/*, src/cli.rs, src/backend.rs, src/repl_cmds.rs

# 3. Regenerate the agent-discoverable skill from the live clap tree.
cargo run -p cli-anything-<name> -- emit-skill -o crates/cli-anything-<name>/SKILL.md

# 4. Test + install.
cargo test -p cli-anything-<name>                  # offline
cargo test -p cli-anything-<name> -- --ignored     # real backend (E2E)
cargo install --path crates/cli-anything-<name>
```

Every command supports `--json` and emits a uniform envelope
`{ok, action, data, error, warnings}` — see HARNESS-rs.md.

## Layout

```
crates/cli-anything-core/      # shared substrate (#![forbid(unsafe_code)])
crates/cli-anything-new/       # the scaffolder (+ embedded templates/)
crates/cli-anything-mermaid/   # proof CLI #1 — render + verify
crates/cli-anything-inkscape/  # proof CLI #2 — SVG model + export + security
plugin/                        # Claude Code plugin (/cli-anything-rs + refine/test/validate/list)
HARNESS-rs.md                  # the agent methodology
scripts/check.sh               # local gate: build + clippy + fmt + deny
```

## Develop

```bash
scripts/check.sh                                    # build + clippy -D warnings + fmt + cargo-deny
cargo test --workspace                              # offline tests
cargo test --workspace -- --ignored                 # E2E (needs real backends/network)
```

`cargo-deny` is required for the gate: `brew install cargo-deny` (or
`cargo install cargo-deny --locked`).

## Using the Claude Code plugin

Load the `plugin/` directory as a Claude Code plugin, then drive the factory:

```
/cli-anything-rs <software-path-or-repo>     # build a new harness
/cli-anything-rs:refine <crate> [focus]      # expand coverage
/cli-anything-rs:test <crate>                # run tests
/cli-anything-rs:validate <crate>            # check against HARNESS-rs.md
/cli-anything-rs:list                        # list harnesses
```

Distribution is local-only for the proof (`cargo install --path ...`); crate
metadata is publish-ready with `publish = false` until the API settles.

License: Apache-2.0.
