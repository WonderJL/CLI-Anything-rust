---
name: cli-anything-mermaid
description: Author, render (mmdc or HTTP), and verify Mermaid diagrams.
---

# cli-anything-mermaid

Author, render (mmdc or HTTP), and verify Mermaid diagrams.

## Installation

```bash
cargo install --path crates/cli-anything-mermaid
```

Requires the real backend: mmdc (optional; HTTP fallback otherwise). Install with `npm install -g @mermaid-js/mermaid-cli`.

## Conventions

- Every command accepts `--json`. JSON output is a uniform envelope: `{ok, action, data, error, warnings}` (`ok=false` plus `error.kind`/`error.message`/`error.hint` on failure).
- Run with no subcommand to enter an interactive REPL.
- Global flags: `--project <path>`, `--dry-run`.

## Commands

### project

Project lifecycle

- `project new` — Create a new project from a sample
  - `--sample <SAMPLE>` — Built-in sample: flowchart | sequence | er
  - `--theme <THEME>` — Mermaid theme (default | dark | forest | neutral)
  - `--output <OUTPUT>` — Save the new project to this path immediately
- `project open` — Open an existing `.mermaid.json` project
  - `<PATH>` (required) — Path to the project JSON
- `project save` — Save the current project
  - `<PATH>` — Output path (defaults to the open project path)
- `project info` — Show project + session status
- `project samples` — List the built-in samples

### diagram

Edit the diagram source

- `diagram set` — Set the diagram source (provide exactly one of --text / --file)
  - `--text <TEXT>` — Inline diagram source
  - `--file <FILE>` — Read the diagram source from a file
- `diagram show` — Print the current diagram source

### export

Render and share the diagram

- `export render` — Render the diagram to an image (mmdc if present, else HTTP)
  - `<OUTPUT>` (required) — Output file path
  - `--format <FORMAT>` — Output format
  - `--overwrite` — Overwrite the output if it exists
- `export share` — Build a mermaid.live share URL (no network)
  - `--mode <MODE>` — Editor or read-only view

### preview

Capture immutable preview bundles + a live session for an agent to poll

- `preview capture` — Render the current diagram into an immutable preview bundle and advance the live session (reuses a cached bundle when the source is unchanged)
  - `--format <FORMAT>` — Format to render into the bundle
  - `--recipe <RECIPE>` — Recipe name — groups bundles and the live session
  - `--force` — Force a fresh bundle even if an identical one is cached
- `preview status` — Print the live preview status — the cheap agent poll (`--json`)
  - `--recipe <RECIPE>` — Recipe name to inspect
  - `--recent <RECENT>` — How many recent trajectory steps to include
- `preview list` — List the preview bundles for a recipe (newest first)
  - `--recipe <RECIPE>` — Recipe name

### session

Undo/redo session control

- `session status` — Show undo/redo status
- `session undo` — Undo the last change
- `session redo` — Redo the last undone change

## Agent guidance

- Always pass `--json` for machine-readable output.
- Check the exit code: `0` success, `1` error.
- On error, read `error.kind` (stable) and `error.hint`.
- Use absolute paths for `--project` and outputs.
- Verify produced files (e.g. magic bytes) rather than trusting exit code alone.

## Preview

- `preview capture` renders an honest, content-addressed bundle from the real backend and advances a live session (identical source reuses the cached bundle).
- Poll cheaply with `preview status --json` — a compact `trajectory_summary` plus the current bundle, without reading every step.
- Enumerate a recipe's bundles (newest first) with `preview list --json`.

