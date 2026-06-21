# PLAN: Port CLI-Anything "One-Shot CLI Generation Factory" to Rust

> Status: APPROVED-FOR-REVIEW (plan only — no implementation)
> Target repo: `/Users/j.leung/workspace/code/agentic-ai-workflow/CLI-Anything-rust`
> Reference repo (Python original): `/Users/j.leung/workspace/code/agentic-ai-workflow/CLI-Anything`
> Toolchain verified: cargo 1.89.0 / rustc 1.89.0. Real `inkscape` (`/opt/homebrew/bin/inkscape`) and `mmdc` (`~/.nvm/.../mmdc`) are installed locally, so E2E tests can invoke the real backends. `cargo-deny` is NOT installed yet (Phase A installs it).

---

## Confidence Assessment

Confidence Assessment Complete:
- All checklist items verified.
- Confidence ≥99%.
- The design is fully locked by the 12 decisions in the task brief; this plan sequences them, it does not re-open them.

What was verified before writing:
- The plan-master workflow (loaded from the archived skill library).
- `HARNESS.md` (7-phase methodology), `commands/cli-anything.md` (slash command), and the sibling commands present in the plugin (`refine.md`, `test.md`, `validate.md`, `list.md`).
- `skill_generator.py` (regex-over-Click approach we are replacing with a clap `Command`-tree walk).
- `repl_skin.py` (banner / prompt / success / error / warning / info / status / table / progress / help / goodbye + per-software accent colors + history file).
- `preview_bundle.py` + `guides/preview-methodology.md` (full bundle / session / trajectory model, sha256 cache key, `preview live status --json` with compact `trajectory_summary`).
- `guides/auto-save-dry-run.md` and `guides/session-locking.md` (core save semantics).
- The two proof CLIs' exact command surfaces, state shapes, undo/redo depth (inkscape = 50 levels), backends, and E2E verification (mermaid renders via HTTP to mermaid.ink and via local `mmdc`; inkscape exports via real `inkscape` subprocess; both verify magic bytes).

---

## 🧠 Problem Understanding

### Precise restatement
Port the CLI-Anything system — an "agent factory" that turns GUI/OSS software into stateful, agent-usable CLIs — from Python to Rust. The port covers two things, not 26 CLIs:

1. **The factory itself**: an agent "brain" (a Rust-adapted `HARNESS-rs.md` methodology + a Claude Code plugin with `/cli-anything-rs` slash commands) PLUS a deterministic Rust scaffolder (`cli-anything new`) that stamps boilerplate so the agent only writes domain logic.
2. **Two proof CLIs** built by that factory: `cli-anything-mermaid` and `cli-anything-inkscape`. Inkscape is also the security showcase (safe untrusted-XML parsing).

Everything depends on a shared library crate, `cli-anything-core`, which carries the full reusable substrate: session + undo/redo, project open/save with auto-save-on-close and `--dry-run`, a `--json` output envelope, a reedline-based REPL skin, the full preview/trajectory subsystem, security utilities, and the SKILL.md emitter that walks clap's `Command` tree.

### Scope (in)
- Single Cargo workspace with: `cli-anything-core` (lib), `cli-anything-new` (scaffolder bin), `cli-anything-mermaid` (bin), `cli-anything-inkscape` (bin).
- `plugin/` directory: Claude Code plugin (slash commands + `HARNESS-rs.md`).
- `templates/` embedded into the scaffolder via `minijinja`.
- Full preview subsystem in core from day one.
- Security deliverable: safe-by-default core + written checklist baked into `HARNESS-rs.md`.
- Local-only distribution (`cargo install --path`, workspace runs).
- Test rigor mirroring the original: unit (synthetic, no external deps), E2E (real `inkscape`/mermaid with output verification), and an installed-binary subprocess test.

### Non-goals (out)
- NOT a 26-CLI port. Only the factory + 2 proof CLIs.
- NOT publishing to crates.io (leave metadata ready; defer publish).
- NOT tokio/async in core (sync by default; `ureq` for blocking HTTP; `std::process::Command` for subprocess). Async only if a future CLI domain demands it.
- NOT ratatui (reedline is the line editor).
- NOT a re-litigation of any locked decision.
- NOT a `cli-hub`/consumer binary in this scope. Core emits the bundle/session/trajectory; the read-only consumer surface (`cli-hub previews ...`) is documented in SKILL/README text as the inspect side but is out of build scope for the proof. (Recorded as an assumption.)
- NOT a Python-compatible on-disk project format. We keep the same logical shape but Rust-native serde JSON (documented). Cross-language file interop is not required.

### Constraints
- Tech stack is fixed: **clap (derive)**, **reedline**, **ureq** (HTTP), **std::process::Command** (subprocess), **quick-xml** + **serde** (XML), **minijinja** (templates), **serde/serde_json** (state), **sha2** (fingerprints), **cargo-deny** (supply chain).
- `#![forbid(unsafe_code)]` in core and all CLIs.
- Sync only in core.
- Local-only proof; metadata ready but no publish.

---

## 🎯 Objectives

Definition of success:
1. `cargo build --workspace` and `cargo test --workspace` succeed (unit + E2E gated by real-tool availability — see Testing).
2. `cli-anything new --software <name>` stamps a compiling new CLI crate that depends on core, has a clap skeleton, a reedline REPL, `emit-skill`, and a test skeleton — with zero hand-edits required to compile.
3. `cli-anything-mermaid` and `cli-anything-inkscape` build, run as one-shot and REPL, support `--json` everywhere, auto-save on close with `--dry-run`, and produce verified output (mermaid: PNG/SVG magic bytes; inkscape: real-`inkscape` PNG export verified by magic bytes).
4. Each proof CLI's `emit-skill` produces a valid `SKILL.md` derived from the live clap `Command` tree (no regex).
5. The preview subsystem in core round-trips bundle/session/trajectory, caches by sha256, and `preview live status --json` returns a compact `trajectory_summary`.
6. `cargo deny check` passes; `#![forbid(unsafe_code)]` holds; the security checklist in `HARNESS-rs.md` is complete and each item maps to a concrete core API.
7. The Claude Code plugin loads, exposes `/cli-anything-rs` (+ adapted refine/test/validate/list), and `HARNESS-rs.md` describes the Rust-adapted 7 phases.

Quality / security / DX expectations:
- Errors are loud, structured, and actionable (agents must self-correct). Use `anyhow` for app-level error context in binaries, `thiserror` for typed errors in core.
- Idempotent where possible; introspection commands (`info`, `list`, `status`) present.
- No `unsafe`. Minimal, audited dependency tree enforced by `cargo-deny`.
- Subprocess only via `Command` with explicit args (never a shell string).
- Untrusted XML parsed with entity-expansion disabled + input-size limits.
- Path-traversal guards on every project load/save.

---

## 🧩 System / Codebase Context

### Reference files that define behavior to mirror
- `cli-anything-plugin/HARNESS.md` — the methodology being rewritten as `HARNESS-rs.md`.
- `cli-anything-plugin/commands/{cli-anything,refine,test,validate,list}.md` — slash commands to adapt.
- `cli-anything-plugin/skill_generator.py` — REPLACED by a clap `Command`-tree walk (`emit-skill`).
- `cli-anything-plugin/repl_skin.py` — mapped to a reedline-based `skin` module (banner/prompt/messages/table/progress/help/goodbye; per-software accent colors in `_ACCENT_COLORS`; history file at `~/.cli-anything-<software>/history`).
- `cli-anything-plugin/preview_bundle.py` — ported 1:1 (logically) into core's `preview` module (protocol strings `preview-bundle/v1`, `preview-trajectory/v1`; sha256 cache key; immutable bundle dir; mutable `session.json`; append-only `trajectory.json`; `summarize_trajectory`).
- `cli-anything-plugin/guides/{preview-methodology,auto-save-dry-run,session-locking}.md` — preview contract + save semantics.
- `mermaid/agent-harness/cli_anything/mermaid/**` and `inkscape/agent-harness/**` — the two proof CLIs' command surfaces, state shapes, backends, and tests (mapped in detail below).

### Runtime assumptions
- Local dev only. Real `inkscape` and `mmdc` present for E2E. mermaid also has an HTTP path (mermaid.ink / mermaid.live) like the Python original.
- On-disk project files are Rust serde JSON; logical shape matches the Python originals but byte-compatibility is not required.

### Conventions to establish
- Crate naming: `cli-anything-<software>`; binary name identical; package on crates.io reserved-but-not-published.
- One core lib crate; each CLI is a workspace member depending on it via path. (This replaces the Python "copy `repl_skin.py` into each package" pattern — Rust shares one crate instead of vendoring.)
- Every command supports `--json`. The `--json` envelope is a single documented struct (below).

---

## 🗂️ Proposed Cargo Workspace Layout

```
CLI-Anything-rust/
├── Cargo.toml                       # [workspace] members + shared [workspace.dependencies]
├── deny.toml                        # cargo-deny config (licenses, bans, advisories, sources)
├── rust-toolchain.toml              # pin stable (1.89 verified)
├── README.md                        # top-level: what this is, how to build/run/test
├── HARNESS-rs.md                    # the Rust-adapted 7-phase methodology (the agent "brain")
├── PLAN-cli-anything-rust.md        # this file
│
├── crates/
│   ├── cli-anything-core/           # shared library — the substrate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs               # #![forbid(unsafe_code)]; re-exports; prelude
│   │       ├── session.rs           # Session + undo/redo snapshot stack (~50 levels)
│   │       ├── project.rs           # open/save, auto-save-on-close, --dry-run, locking
│   │       ├── json_envelope.rs     # the --json output contract (Envelope<T>)
│   │       ├── skin/                # reedline-based REPL skin
│   │       │   ├── mod.rs           # Skin struct: banner/prompt/messages/table/progress
│   │       │   ├── colors.rs        # accent map + status colors (port of repl_skin.py)
│   │       │   └── repl.rs          # reedline wiring: history, prompt, line loop
│   │       ├── preview/             # full preview subsystem (port of preview_bundle.py)
│   │       │   ├── mod.rs
│   │       │   ├── fingerprint.rs   # sha256 data/file fingerprints, cache key
│   │       │   ├── bundle.rs        # manifest/summary structs, prepare/finalize, cache lookup
│   │       │   ├── session_head.rs  # mutable session.json head
│   │       │   ├── trajectory.rs    # append-only trajectory.json + summarize_trajectory
│   │       │   └── live_status.rs   # preview live status --json payload builder
│   │       ├── security/            # safe-by-default utilities
│   │       │   ├── mod.rs
│   │       │   ├── subprocess.rs    # run(cmd, args) wrapper — explicit args, timeout, no shell
│   │       │   ├── xml.rs           # bounded/entity-safe quick-xml reader
│   │       │   └── path_guard.rs    # path-traversal guard for load/save
│   │       ├── emit_skill.rs        # walk a clap Command tree -> SKILL.md
│   │       └── error.rs             # thiserror types for core
│   │
│   ├── cli-anything-new/            # the scaffolder (custom `cli-anything new` binary)
│   │   ├── Cargo.toml
│   │   ├── build.rs                 # (optional) embed templates via include_dir/RUST_EMBED-style
│   │   └── src/
│   │       ├── main.rs              # clap CLI: `new --software <name> [--accent ...] [--out ...]`
│   │       ├── render.rs            # minijinja env + template stamping
│   │       └── manifest.rs          # writes/edits workspace Cargo.toml to add the member
│   │
│   ├── cli-anything-mermaid/        # PROOF CLI #1
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs              # clap root + REPL default + emit-skill + auto-save
│   │       ├── cli.rs               # clap derive command tree
│   │       ├── core/
│   │       │   ├── project.rs       # diagram source + theme state
│   │       │   ├── diagram.rs       # set/show operations
│   │       │   └── export.rs        # render/share pipeline
│   │       ├── backend.rs           # mermaid backend: mmdc subprocess + HTTP (ureq) fallback
│   │       └── repl_cmds.rs         # REPL command dispatch
│   │
│   └── cli-anything-inkscape/       # PROOF CLI #2 (+ security showcase)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── cli.rs               # clap derive command tree (document/shape/text/style/...)
│           ├── core/
│           │   ├── document.rs      # SVG/XML document state model
│           │   ├── shapes.rs        # rect/circle/ellipse/line/polygon/path/star/text
│           │   ├── layers.rs        # layer add/remove/reorder/move
│           │   ├── style.rs         # parse/serialize style strings
│           │   ├── transform.rs     # translate/rotate/scale/skew
│           │   ├── gradient.rs
│           │   ├── svg.rs           # project -> SVG (quick-xml writer)
│           │   └── export.rs        # svg/png/pdf export
│           ├── backend.rs           # real `inkscape` subprocess export
│           └── repl_cmds.rs
│
├── templates/                       # scaffolder source templates (also embedded at build)
│   ├── Cargo.toml.j2
│   ├── main.rs.j2
│   ├── cli.rs.j2
│   ├── core_project.rs.j2
│   ├── backend.rs.j2
│   ├── repl_cmds.rs.j2
│   ├── tests_unit.rs.j2
│   ├── tests_e2e.rs.j2
│   └── README.md.j2
│
└── plugin/                          # Claude Code plugin (the agent layer)
    ├── .claude-plugin/plugin.json
    ├── commands/
    │   ├── cli-anything-rs.md       # the driver (adapts cli-anything.md to Rust phases)
    │   ├── refine.md
    │   ├── test.md
    │   ├── validate.md
    │   └── list.md
    └── HARNESS-rs.md                # symlink/copy of top-level HARNESS-rs.md (single source)
```

### Justification of each top-level piece
- **Single workspace**: one `cargo build --workspace`, shared dependency versions (`[workspace.dependencies]`), shared lockfile, atomic refactors across core + CLIs. This is the Rust-idiomatic replacement for the Python "vendor `repl_skin.py` into every package" pattern.
- **`cli-anything-core` (lib)**: the substrate. Everything reusable lives here exactly once so a generated CLI is "thin": clap surface + domain logic + a few core calls.
- **`cli-anything-new` (bin)**: deterministic scaffolder. Embeds templates so it works from any directory and from an installed binary. Self-contained (no `cargo-generate` dependency, per locked decision 8).
- **`cli-anything-mermaid` / `cli-anything-inkscape` (bins)**: the two proofs; workspace members so they exercise core directly.
- **`templates/`**: single source of truth for the boilerplate the scaffolder stamps; also embedded into the scaffolder binary (via `include_dir!`-style or a `build.rs` that bakes them in) so the installed `cli-anything new` needs no on-disk template dir.
- **`plugin/`**: ships the agent layer (slash commands + `HARNESS-rs.md`). First-class per locked decision 11.
- **`deny.toml` / `rust-toolchain.toml`**: supply-chain enforcement + reproducible toolchain.

---

## `cli-anything-core` Module Breakdown

### `session.rs` — Session + undo/redo
- `Session<S>` generic over a serde-serializable project state `S: Clone + Serialize + DeserializeOwned`.
- Fields: `state: Option<S>`, `project_path: Option<PathBuf>`, `modified: bool`, `undo_stack: Vec<S>`, `redo_stack: Vec<S>`, `max_undo: usize` (default 50, matching inkscape's `_undo_stack` cap).
- API: `snapshot(&mut self, description: Option<&str>)` (deep-copy via `Clone`, push to `undo_stack`, truncate to `max_undo` from the front, clear `redo_stack`); `undo()` / `redo()` (swap heads); `is_open()`, `mark_modified()`, `status()` (returns counts + path + modified for `session status --json`).
- Deep-copy snapshot = `Clone` of `S` (serde structs derive `Clone`); ~50-level cap, FIFO eviction of oldest, like the original.
- Mermaid's Python `Session` had no explicit cap; we standardize on 50 across both for consistency (documented assumption).

### `project.rs` — open/save + auto-save-on-close + `--dry-run`
- `open_project::<S>(path)` → path-guard, read, `serde_json::from_str`, return `S`.
- `save_project::<S>(path, &S)` → path-guard, atomic locked write.
- **Locking** (port of `session-locking.md`): `locked_save_json(path, &data)` opens with read-write (no truncate-on-open), acquires an exclusive advisory lock, then `set_len(0)` + write + flush inside the lock, then unlock. Use `fs2` crate's `FileExt::lock_exclusive`/`unlock` for cross-platform advisory locking (gracefully proceed unlocked if the FS rejects locks, matching the Python fallback). This is the one extra small dependency justified by the locking requirement; if `cargo-deny`/audit prefers fewer deps, fall back to a `libc::flock` call behind `#[cfg(unix)]` (the proof targets macOS/Linux). Decision recorded under Assumptions.
- **Auto-save-on-close** (port of `auto-save-dry-run.md`): an `AutoSaveGuard` RAII struct that, on `Drop`, saves iff `!repl_mode && !dry_run && session.modified && session.project_path.is_some()`. Binaries hold the guard for the lifetime of a one-shot command. REPL never installs the guard (it saves manually). `--dry-run` is accepted in REPL but ignored.
- `--dry-run` semantics table reproduced in `HARNESS-rs.md`.

### `json_envelope.rs` — the `--json` output contract
Single documented envelope so every command's machine output is uniform:
```rust
#[derive(Serialize)]
pub struct Envelope<T: Serialize> {
    pub ok: bool,                 // true on success, false on error
    pub action: String,           // e.g. "project.new", "export.render"
    pub data: Option<T>,          // command-specific payload (None on error)
    pub error: Option<ErrInfo>,   // populated on failure
    pub warnings: Vec<String>,    // non-fatal notes
}
#[derive(Serialize)]
pub struct ErrInfo { pub kind: String, pub message: String, pub hint: Option<String> }
```
- Helpers: `Envelope::ok(action, data)`, `Envelope::err(action, kind, message, hint)`.
- Human mode prints via the skin; `--json` prints `serde_json::to_string(&envelope)` to stdout, exit code 0/1.
- Documented in `HARNESS-rs.md` and each `SKILL.md`. (The Python originals returned bare dicts; we standardize on a wrapper to give agents a consistent `ok`/`error` contract — documented assumption, an improvement over the original.)

### `skin/` — reedline-based REPL skin (port of `repl_skin.py`)
- `colors.rs`: port `_ACCENT_COLORS` (gimp/blender/inkscape/audacity/libreoffice/obs/kdenlive/shotcut + default sky-blue), status colors (green/yellow/red/blue/magenta), brand cyan. Provide both ANSI-256 escape constants and the hex map (for reedline styled prompt). Use the `nu-ansi-term` crate (reedline already depends on it) for styling so we add no new color dependency.
- `mod.rs` `Skin`: `new(software, version)`; `print_banner()` (box-drawing banner with title `◆ cli-anything · <Name>`, version, skill path, tip); `success/error/warning/info/hint/section`; `status/status_block`; `progress(current,total,label)`; `table(headers, rows)`; `help(commands)`; `print_goodbye()`. Error goes to stderr (matching Python).
- `repl.rs`: build a reedline `Reedline` with a `FileBackedHistory` at `~/.cli-anything-<software>/history`, a custom `Prompt` impl that renders `◆ <software> [project*] ❯ ` with the accent color and `*` modified marker, history search, and (optionally) a `Completer` seeded from the command list. The REPL loop reads a line, shlex-splits it (use the `shlex` crate to mirror Python's `shlex` parsing), dispatches to the CLI's `repl_cmds`, and never auto-saves.
- Banner shows the absolute `SKILL.md` path so an agent can read it (mirrors the original's behavior); the path resolution prefers a repo-root `skills/<id>/SKILL.md` then a packaged copy — for the proof we point it at the crate-local generated `SKILL.md`.

### `preview/` — full preview subsystem (port of `preview_bundle.py` + methodology)
Protocol constants: `PROTOCOL_VERSION = "preview-bundle/v1"`, `TRAJECTORY_PROTOCOL_VERSION = "preview-trajectory/v1"`.
- `fingerprint.rs`: `hash_data(&T)` (canonical JSON via `serde_json` with sorted keys + compact separators, then sha256 hex), `fingerprint_data` → `"sha256:<hex>"`, `fingerprint_file(path)` → fingerprint of `{path,size,mtime_ns}`, and `build_cache_key(...)` over `{protocol_version, software, recipe, bundle_kind, source_fingerprint, options, harness_version}`. **Important parity note**: Python uses `json.dumps(sort_keys=True, separators=(",",":"))`. We must reproduce the canonical-form rule (sorted keys, no spaces) so fingerprints are stable; document that cross-language fingerprint equality is NOT a goal (Rust-only), but intra-Rust stability IS.
- `bundle.rs`: serde structs `Manifest`, `Summary`, `ArtifactRecord`; `bundle_root(software, recipe, project_path|root_dir)` (defaults to `<project_dir>/.cli-anything/previews/<software>/<recipe>` or `~/.cli-anything/previews/...`); `find_cached_manifest` (scan `manifest.json` files, match protocol+software+recipe+bundle_kind+status∈{ok,partial}+cache_key); `find_latest_manifest`; `prepare_bundle` (cache hit → return cached paths; miss → mint `bundle_id = <UTCstamp>_<cachekey8>_<recipe-slug>` and create `artifacts/`); `artifact_record(...)`; `finalize_bundle(...)` (write `summary.json` then `manifest.json`, attach `_manifest_path`/`_bundle_dir`/`_summary_path`). Bundle dirs are immutable once written.
- `session_head.rs`: mutable `session.json` head (current bundle id/paths, recipe, viewer hints, current step id, trajectory path).
- `trajectory.rs`: append-only `trajectory.json` with `steps[]` (each: `step_id`, `step_index`, `command`, `command_started_at`, `command_finished_at`, `publish_reason`, `source_fingerprint`, `bundle_id`, `bundle_dir`, `manifest_path`, `summary_path`, optional `stage_label`/`note`); `append_live_trajectory(...)`; `summarize_trajectory(recent_steps=3)` → compact summary.
- `live_status.rs`: build the `preview live status --json` payload (`status`, `active`, `_session_dir`, `_session_path`, `current_bundle_*`, `_trajectory_path`, `current_step_id`, `latest_command`, `latest_publish_reason`, `trajectory_summary`, viewer hints). This is the agent-cheap poll.
- Producer = the CLI (`preview capture/latest/recipes`, optional `diff` + `live start/push/status/stop`); consumer (`cli-hub previews ...`) is documented text only, not built (see Non-goals).

### `security/` — safe-by-default utilities
- `subprocess.rs`: `run(program: &str, args: &[&str], timeout: Duration) -> Result<Output>`. Always `Command::new(program).args(args)` — never a shell string, so no shell injection is possible. Resolve the program via a `which`-style PATH lookup helper (port of `shutil.which`); on missing binary return a typed error with install instructions. Enforce timeout via a wait-with-timeout (spawn + `wait_timeout` crate, or a thread + channel — `wait-timeout` is the minimal, audited choice; recorded under Assumptions). Capture stdout/stderr; on non-zero exit return the last N chars of stderr in the error.
- `xml.rs`: bounded, entity-safe reader built on `quick-xml`. Configure the reader to **not expand entities** (quick-xml does not perform DTD/entity expansion by default — this is the Rust analog of `defusedxml`'s protection; we make it explicit and add a guard that rejects any `DOCTYPE`/DTD or custom-entity declaration). Enforce an **input-size limit** (reject inputs over a configurable byte cap, default e.g. 25 MB) and a **max-depth / max-element-count** guard to stop billion-laughs-style amplification. Expose `read_svg_safely(bytes_or_path) -> Result<Dom>` for inkscape's import paths.
- `path_guard.rs`: `guard_project_path(base: Option<&Path>, candidate: &Path) -> Result<PathBuf>` — canonicalize, reject `..` escapes outside an allowed base (or outside CWD when no base), reject absolute-path traversal when a base is enforced, and normalize symlinks. Used by every `open_project`/`save_project`.

### `emit_skill.rs` — SKILL.md emitter that walks clap's Command tree
- Replaces `skill_generator.py`'s regex-over-Click approach with runtime introspection of clap.
- `emit_skill(cmd: &clap::Command, meta: SkillMeta) -> String`: walk `cmd.get_subcommands()` recursively; for each subcommand collect name, about/long_about, args (names, value names, help, required, defaults). Render a `SKILL.md` with YAML frontmatter (`name: cli-anything-<software>`, `description`), an installation/prereq section, a per-command-group table, agent-guidance (always use `--json`, check exit codes, parse the envelope, use absolute paths, verify outputs), and preview producer/consumer docs when the CLI has a `preview` group.
- Surfaced as a **hidden `emit-skill` subcommand** on each generated CLI (`#[command(hide = true)]`), so the agent/factory regenerates `SKILL.md` deterministically from the real parser: `cli-anything-<software> emit-skill -o SKILL.md`. This is the locked decision-2/9/11 mechanism and the key Rust-vs-Python divergence.
- The clap tree is the single source of truth → SKILL.md can never drift from the actual command surface (the regex approach could).

### `error.rs`
- `thiserror`-based `CoreError` (Io, Json, PathTraversal, XmlTooLarge, XmlForbiddenEntity, SubprocessNotFound, SubprocessFailed{code,stderr}, SubprocessTimeout, BundleCacheMiss, ...). Binaries map these into the `--json` `ErrInfo` envelope (kind/message/hint) so agents get structured failures.

---

## The Scaffolder (`cli-anything new`)

### Inputs
- `--software <name>` (required) — e.g. `mermaid`; slugged to crate name `cli-anything-<slug>` and binary name identical.
- `--accent <color-name|ansi256>` (optional) — REPL accent; defaults to core's default sky-blue if unknown, mirroring `repl_skin.py`.
- `--out <dir>` (optional) — workspace root; defaults to CWD (must contain the workspace `Cargo.toml`).
- `--description <str>` (optional) — used in Cargo.toml + SKILL frontmatter.
- `--with-preview` / `--no-preview` (optional, default on) — whether to stamp a `preview` command group stub.

### Template set it embeds (via `minijinja`)
From `templates/`: `Cargo.toml.j2`, `main.rs.j2`, `cli.rs.j2`, `core_project.rs.j2`, `backend.rs.j2`, `repl_cmds.rs.j2`, `tests_unit.rs.j2`, `tests_e2e.rs.j2`, `README.md.j2`. Templates are baked into the binary at build time so the installed scaffolder is self-contained.

### Files it stamps
For `cli-anything-<name>`:
1. `crates/cli-anything-<name>/Cargo.toml` — package metadata (name, version 0.1.0, description, crates.io metadata fields filled but `publish = false`), `#![forbid(unsafe_code)]`-ready, dependency on `cli-anything-core` via path + workspace deps (clap, serde, serde_json, anyhow).
2. `src/main.rs` — clap root with global `--json`, `--project`, `--dry-run`; REPL-as-default (no subcommand → enter REPL); hidden `emit-skill`; `AutoSaveGuard` wiring for one-shot commands.
3. `src/cli.rs` — clap derive skeleton with `project new/open/save/info` and `session status/undo/redo` already present (the universal surface), plus a TODO domain group stub.
4. `src/core/project.rs` — a starter project state struct (serde) + `open/save` calling core.
5. `src/backend.rs` — a backend stub with a `find_binary` + `run` example using `core::security::subprocess`.
6. `src/repl_cmds.rs` — REPL dispatch wired to the skin.
7. `tests/unit.rs` + `tests/e2e.rs` — test skeletons (unit = synthetic; e2e = real-backend placeholder with magic-byte assertion pattern).
8. `README.md` — install/run/test, prereqs.
9. Workspace `Cargo.toml` edit — append the new member to `[workspace].members` (handled by `manifest.rs`, idempotent; no-op if already present).

### How the agent invokes it then fills domain logic
1. Agent runs `cli-anything new --software <name> --accent <c> --description "<d>"`.
2. Scaffolder stamps the crate and registers the workspace member; result compiles immediately (`cargo build -p cli-anything-<name>` is green).
3. Agent (driven by `HARNESS-rs.md`) writes ONLY domain logic: the real command groups in `cli.rs`, the state operations in `core/*`, and the real `backend.rs` (subprocess/HTTP calls to the actual software).
4. Agent regenerates `SKILL.md` via `cli-anything-<name> emit-skill -o SKILL.md`.
5. Agent writes/extends `tests/unit.rs` + `tests/e2e.rs` per the test strategy.

---

## HARNESS-rs.md — the Rust-adapted 7-phase methodology

Rewrite `HARNESS.md` for Rust output. Keep the spine (analyze → design → implement → test → SKILL.md → publish) and the non-negotiable rule "use the real software, don't reimplement it." Call out every divergence:

- **Phase 1 — Codebase Analysis** (unchanged intent): identify backend engine, map GUI actions to API/CLI calls, identify the data model and any existing CLI (`mmdc`, `inkscape`, `melt`, etc.).
- **Phase 2 — CLI Architecture Design**: design command groups; **clap (derive), not Click**; REPL via **reedline, not prompt_toolkit**; **one shared `cli-anything-core` crate, not a vendored `repl_skin.py` copy**; plan the `--json` envelope; design state + undo/redo on top of `core::Session`.
- **Phase 3 — Implementation**: run `cli-anything new` (the **deterministic Rust scaffolder, not a manual directory layout**); fill domain logic; **`std::process::Command` for subprocess (sync), `ureq` for blocking HTTP (no tokio)**; auto-save + `--dry-run` via `core::AutoSaveGuard`; preview via `core::preview`.
- **Phase 3.5 — Security Checklist Phase (NEW, Rust-specific)**: a mandatory checklist (below) every CLI must satisfy before passing.
- **Phase 4 — Test Planning**: a `TEST.md`-equivalent section in the crate README or a `tests/PLAN.md`; same rigor.
- **Phase 5 — Test Implementation**: unit (synthetic, no external deps), E2E (real backend, output verification by magic bytes/format), installed-binary subprocess test; **`cargo test`, not pytest**; gate real-tool E2E on tool presence.
- **Phase 6 — SKILL.md Generation**: **`emit-skill` walks the clap `Command` tree, not regex over decorators**; deterministic, never drifts.
- **Phase 7 — Distribution**: **`cargo build`/`cargo install --path`, not pip/PyPI**; crate metadata ready, `publish = false`, crates.io deferred. Document the `cargo install --path crates/cli-anything-<name>` path.

### Security checklist (baked into HARNESS-rs.md Phase 3.5)
Each item maps to a concrete core API:
1. **No shell strings** — all subprocess via `core::security::subprocess::run(program, &args, timeout)` (explicit args). ☐
2. **Untrusted XML/file parsing bounded** — use `core::security::xml::read_svg_safely`: entity expansion off, DOCTYPE/DTD rejected, input-size limit, depth/element-count cap. ☐
3. **Path-traversal guards** — every load/save goes through `core::security::path_guard::guard_project_path`. ☐
4. **`#![forbid(unsafe_code)]`** at the top of `lib.rs` and every CLI `main.rs`. ☐
5. **Minimal audited dependency tree** — `cargo deny check` passes (advisories, bans, licenses, sources). ☐
6. **Subprocess binary resolution + clear install errors** — missing backend returns a typed error with install instructions, never a silent fallback. ☐
7. **Timeouts on all subprocess calls** — no unbounded waits. ☐

---

## The Two Proof CLIs as Concrete Build Targets

### Proof CLI #1 — `cli-anything-mermaid`
**State (`core/project.rs`)**: project = `{ code: String, mermaid: serde_json::Value (theme config, e.g. {"theme":"default"}), update_diagram: bool, rough: bool, pan_zoom: bool, grid: bool }`. Save format: `.mermaid.json` (serde JSON, indent 2). Built-in samples: `flowchart`, `sequence`, `er`.

**Command surface (clap)** — mirror the Python tree exactly:
- Global: `--json`, `--project <PATH>`, `--dry-run`.
- `project new [--sample flowchart] [--theme default] [-o <path>]`
- `project open <path>`
- `project save [path]`
- `project info`
- `project samples`
- `diagram set (--text <src> | --file <path>)`  (exactly one)
- `diagram show`
- `export render <output> [-f svg|png] [--overwrite]`
- `export share [--mode edit|view]`
- `session status | undo | redo`
- (default, no subcommand) → REPL
- hidden `emit-skill [-o <path>]`

**Backend (`backend.rs`)** — two render paths (the Python original used HTTP only; the locked decision wants `mmdc` CLI/HTTP):
- **Local `mmdc`** (preferred when present): write the diagram source to a temp `.mmd`, run `mmdc -i <in.mmd> -o <out.{svg|png}>` via `core::security::subprocess::run` (real `mmdc` confirmed installed locally). On success, the output file is the artifact.
- **HTTP fallback** (`ureq`, blocking): serialize state as `pako:<urlsafe-b64(zlib(compact-json))>` (port of `serialize_state`), GET `https://mermaid.ink/svg/<pako>` or `https://mermaid.ink/img/<pako>?type=png`; write the body. `export share` builds `https://mermaid.live/edit#<pako>` or `/view#<pako>` (no network).
- Selection: try `mmdc` first; fall back to HTTP; record `method` in the result. Env overrides `MERMAID_RENDERER_URL` / `MERMAID_LIVE_URL` honored like the original.
- zlib via the `flate2` crate (minimal, widely-audited) for the pako encoding; base64 via the `base64` crate.

**Output verification this CLI performs (and its E2E tests assert)**:
- PNG: first 4 bytes == `\x89PNG` (`[0x89,0x50,0x4E,0x47]`).
- SVG: body contains `<svg` within the first 200 bytes and is non-empty.
- File exists and size > 0; result envelope carries `output`, `format`, `method`, `file_size`, `url`.

### Proof CLI #2 — `cli-anything-inkscape` (+ SECURITY showcase)
**State (`core/document.rs`)**: project = `{ version, name, document:{width,height,units,view_box,background}, objects:[...], layers:[...], gradients:[...], metadata:{created,modified,software} }`. Object types: rect/circle/ellipse/line/polygon/path/star/text/image. Save format `.inkscape-cli.json`. Undo cap = 50.

**Command surface (clap)** — mirror the Python tree (large; full groups):
- Global: `--json`, `--project <PATH>`, `-s/--save`, `--dry-run`.
- `document new [-w 1920] [-h 1080] [-u px|mm|cm|in|pt|pc] [-bg #ffffff] [-n untitled] [-p <profile>] [-o <path>]`, `document open <path>`, `document save [path]`, `document info`, `document profiles`, `document canvas-size -w -h`, `document units <units>`, `document json`.
- `shape add-rect|add-circle|add-ellipse|add-line|add-polygon|add-path|add-star (...)`, `shape remove <i>`, `shape duplicate <i>`, `shape list`, `shape get <i>`.
- `text add (-t ...)`, `text set <i> <prop> <value>`, `text list`.
- `style set-fill|set-stroke|set-opacity|set <i> ...`, `style get <i>`, `style list-properties`.
- `transform translate|rotate|scale|skew-x|skew-y <i> ...`, `transform get <i>`, `transform clear <i>`.
- `layer add|remove|move-object|set|list|reorder|get (...)`.
- `path union|intersection|difference|exclusion <a> <b> [-n]`, `path convert <i>`, `path list-operations`.
- `gradient add-linear|add-radial (...)`, `gradient apply <g> <o> [-t fill|stroke]`, `gradient list`.
- `export png <out> [-w -h --dpi 96 -bg --overwrite]`, `export svg <out> [--overwrite]`, `export pdf <out> [--overwrite]`, `export presets`.
- `session status | undo | redo | history`.
- (default) → REPL; hidden `emit-skill`.

**SVG generation (`core/svg.rs`)**: build SVG with the `quick-xml` writer (no string concatenation), register SVG/Inkscape/Sodipodi/XLink namespaces, `<defs>` for gradients, one `<g inkscape:groupmode="layer">` per layer, element-per-object. **Import paths** (e.g. reading an existing SVG) go through `core::security::xml::read_svg_safely` (this is the security showcase: bounded, entity-safe parsing — the Rust analog of `defusedxml`).

**Backend (`backend.rs`)** — real `inkscape` subprocess (confirmed installed). Exact arg lists to mirror:
- PNG: `inkscape <svg_path> --export-filename=<out.png> --export-dpi=<dpi> [--export-width=<w>] [--export-height=<h>]`
- PDF: `inkscape <svg_path> --export-filename=<out.pdf>`
- EPS: `inkscape <svg_path> --export-filename=<out.eps>`
- Version probe: `inkscape --version`
- All via `core::security::subprocess::run` (explicit args, timeout 60s). Missing `inkscape` → typed error with install instructions (no silent Pillow-style fallback in the Rust port; the real software is the renderer per HARNESS rule).

**Output verification this CLI performs (and E2E asserts)**:
- SVG export: output is well-formed XML, root is `<svg>` in the SVG namespace, has width/height/viewBox, Inkscape namespaces present; file size > 0.
- PNG export (via real `inkscape`): first 8 bytes == PNG signature `\x89PNG\r\n\x1a\n`; file size > 0; (optionally) decode header to assert width/height match the export request.
- PDF export (via real `inkscape`): first 5 bytes == `%PDF-`; file size > 0.
- Document round-trip: create → add objects → save JSON → open → object count/contents preserved.

---

## 🧪 Testing & Validation Strategy (mirrors the original's rigor)

Three layers per CLI, plus workspace-level checks.

### 1. Unit tests (`tests/unit.rs`) — synthetic, no external deps
- Session undo/redo: snapshot → mutate → undo restores; redo re-applies; 50-level cap evicts oldest; `redo_stack` cleared on new snapshot.
- Project save/open round-trip via serde (temp files); path-guard rejects `..` escapes.
- `--json` envelope shape (`ok`/`action`/`data`/`error`).
- mermaid: `serialize_state` produces `pako:`-prefixed string; URL construction for render/share.
- inkscape: style parse/serialize; profile resolution; SVG element serialization (string contains expected tags/attrs); gradient defs; XML safety unit tests — feed a billion-laughs / DOCTYPE-bearing SVG and assert `read_svg_safely` rejects it; feed an oversized input and assert the size-limit error.
- preview (core): fingerprint determinism (same input → same `sha256:`), cache-key stability, `prepare_bundle` cache hit vs miss, `finalize_bundle` writes manifest+summary, `summarize_trajectory` compactness, `live status` payload fields.
- These run in CI with zero external tools. `cargo test --workspace` default path.

### 2. E2E tests (`tests/e2e.rs`) — real backend, output verification
- **mermaid**: render SVG and PNG via real `mmdc` AND via HTTP (mermaid.ink); assert magic bytes (`<svg`, `\x89PNG`), size > 0; print artifact paths. Requires `mmdc` (local) and network (HTTP path).
- **inkscape**: create a document with shapes/text/layers → export SVG (assert well-formed) → export PNG and PDF via real `inkscape` → assert PNG signature / `%PDF-` / size > 0; print artifact paths. Requires `inkscape`.
- **No graceful degradation**: when the real tool is required for a test, the test FAILS (not skips) if invoked in "force" mode. For local/CI convenience, gate these behind an env flag (default behavior documented below).

### 3. Installed-binary subprocess test
- Build/install the CLI (`cargo build -p ...` → use the target binary path, or `cargo install --path`), then invoke it as a child process for a full workflow: `project/document new` → mutate → `export` → verify output file. Mirrors the Python `_resolve_cli` + `CLI_ANYTHING_FORCE_INSTALLED=1` pattern. Provide a `CLI_ANYTHING_FORCE_INSTALLED=1` analog that requires the installed/target binary rather than a fallback.

### Gating which tests require real software (explicit)
- Unit tests: never require external software → always run.
- E2E real-backend tests: require `inkscape` / `mmdc` (and network for mermaid HTTP). Default `cargo test --workspace` runs them when the tools are detected; in a strict mode (`CLI_ANYTHING_FORCE_INSTALLED=1` / a `--features e2e` flag), absence is a hard failure. Document this in each crate README so a contributor without the tools isn't blocked, but a release run is.

### Workspace-level validation checkpoints
- `cargo build --workspace` green.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --all --check` clean.
- `cargo deny check` passes (advisories/bans/licenses/sources).
- `#![forbid(unsafe_code)]` present in `lib.rs` and every CLI `main.rs` (grep check).
- Each CLI's `emit-skill -o SKILL.md` produces a non-empty, frontmatter-valid SKILL.md whose command list matches the clap tree.

### Rollback / safety
- Each phase is an isolated, compiling milestone; if a phase regresses, the previous phase's tag/commit remains green. No destructive operations on user data (project saves are atomic + locked). The factory never writes outside the workspace except into `~/.cli-anything-*` (history) and `~/.cli-anything/previews` (preview bundles), both standard.

---

## 🔁 Execution Order (phased roadmap with definitions-of-done)

> Sequence chosen so each phase compiles and is independently verifiable. Core lands before the scaffolder (scaffolder templates depend on the core API surface); proofs land after the scaffolder so they're built the same way the factory will build future CLIs.

### Phase A — Workspace + skeleton + supply-chain
Tasks:
- [ ] `cargo init --lib` style: create workspace `Cargo.toml` with `members` and `[workspace.dependencies]` (clap, serde, serde_json, anyhow, thiserror, sha2, ureq, quick-xml, minijinja, flate2, base64, reedline, nu-ansi-term, shlex, fs2/wait-timeout — pin versions).
- [ ] `crates/cli-anything-core` lib crate with `#![forbid(unsafe_code)]`, empty module files, prelude re-exports.
- [ ] `rust-toolchain.toml` (stable), `deny.toml`, top-level `README.md` stub.
- [ ] Install `cargo-deny` (`cargo install cargo-deny`) and get `cargo deny check` green on the empty tree.
- [ ] CI-equivalent local check script: build + clippy + fmt + deny.
DoD: `cargo build --workspace`, `cargo clippy`, `cargo fmt --check`, `cargo deny check` all pass on the skeleton.

### Phase B — Core modules (incl. preview)
Tasks:
- [ ] `session.rs` (Session<S>, undo/redo, 50-cap) + unit tests.
- [ ] `project.rs` (open/save, locked atomic write, `AutoSaveGuard`, `--dry-run` semantics) + unit tests.
- [ ] `json_envelope.rs` (Envelope<T>, ErrInfo, helpers) + unit tests.
- [ ] `security/` (subprocess wrapper w/ timeout + PATH resolution; bounded entity-safe xml reader; path_guard) + unit tests (incl. billion-laughs / DOCTYPE / oversize rejection).
- [ ] `skin/` (colors, banner, messages, table, progress, help; reedline repl prompt + history) — smoke-tested via a tiny example.
- [ ] `preview/` (fingerprint, bundle, session_head, trajectory, live_status) full port + unit tests (cache hit/miss, fingerprint determinism, trajectory summary).
- [ ] `emit_skill.rs` (walk a clap Command tree → SKILL.md) + unit test against a synthetic clap command.
- [ ] `error.rs` (thiserror CoreError).
DoD: `cargo test -p cli-anything-core` green; all security/preview behaviors unit-covered; skin renders a banner in a manual smoke run.

### Phase C — Scaffolder (`cli-anything-new`) + templates
Tasks:
- [ ] `templates/*.j2` authored to produce a crate that uses the Phase-B core API.
- [ ] `cli-anything-new` binary: clap inputs, minijinja render, embed templates at build, `manifest.rs` workspace-member registration (idempotent).
- [ ] Generate a throwaway crate `cli-anything-smoke` via the scaffolder; assert it builds and its `emit-skill` runs; then remove it (or keep as a fixture under `tests/`).
DoD: `cli-anything new --software smoke` stamps a crate that compiles with no hand-edits, enters REPL, and `emit-skill` produces a valid SKILL.md.

### Phase D — mermaid proof
Tasks:
- [ ] Scaffold `cli-anything-mermaid` via the scaffolder, then fill domain logic: project/diagram/export/session command groups; `backend.rs` (mmdc subprocess + ureq HTTP fallback + share URL); pako serialization (flate2 + base64).
- [ ] Unit tests (serialize_state, URL building, session) + E2E (real `mmdc` and HTTP render → magic-byte verification) + installed-binary subprocess test.
- [ ] `emit-skill -o SKILL.md`; write crate README.
DoD: one-shot + REPL work; `--json` everywhere; SVG/PNG render verified by magic bytes via both `mmdc` and HTTP; auto-save + `--dry-run` correct; tests green with `mmdc` present.

### Phase E — inkscape proof + security showcase
Tasks:
- [ ] Scaffold `cli-anything-inkscape`; fill the full command surface (document/shape/text/style/transform/layer/path/gradient/export/session); `core/svg.rs` via quick-xml writer; SVG import via `read_svg_safely`.
- [ ] `backend.rs` real `inkscape` export (png/pdf/eps + version probe) with exact arg lists.
- [ ] Security showcase tests: untrusted-SVG rejection (DOCTYPE/entities/oversize), path-traversal guard, no shell string, timeouts.
- [ ] Unit + E2E (real `inkscape` PNG/PDF export → magic bytes; SVG well-formedness; document round-trip) + installed-binary subprocess test.
- [ ] `emit-skill -o SKILL.md`; crate README.
DoD: full surface works; real `inkscape` produces verified PNG/PDF; XML-safety tests pass; security checklist fully satisfied for this CLI.

### Phase F — Plugin + HARNESS-rs.md + slash commands
Tasks:
- [ ] Author `HARNESS-rs.md` (7 phases + Phase 3.5 security checklist; all Rust divergences called out).
- [ ] `plugin/.claude-plugin/plugin.json`; `plugin/commands/cli-anything-rs.md` (driver) + adapted `refine.md`, `test.md`, `validate.md`, `list.md`.
- [ ] Reference `HARNESS-rs.md` from the driver command (read-first, like the original).
- [ ] Top-level README: how the factory works end-to-end (scaffold → fill → emit-skill → test → install), and how to load the plugin.
DoD: plugin directory is valid; `/cli-anything-rs` is documented and points at `HARNESS-rs.md`; the two proof CLIs' `SKILL.md` files are consistent with their clap trees; full-workspace validation suite green.

### Cross-phase dependencies
- B depends on A. C depends on B (templates target B's API). D and E depend on C (built via the scaffolder) and B. F depends on D + E (documents the real proof CLIs) and references HARNESS-rs throughout.

---

## 🧼 Cleanup & Quality Checks
- Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` at the end of each phase.
- De-duplicate: anything used by both proof CLIs that is not yet in core gets promoted into `cli-anything-core` (the whole point of the shared crate).
- Naming consistency: crate = binary = `cli-anything-<software>`; envelope `action` strings use `group.command` dotted form.
- Docs: every crate has a README; each proof CLI ships a generated `SKILL.md`; `HARNESS-rs.md` and the top-level README explain the workflow; preview producer/consumer roles documented in README + SKILL.
- `cargo deny check` green; dependency tree reviewed for minimality (each added crate justified: clap, serde, serde_json, anyhow, thiserror, sha2, ureq, quick-xml, minijinja, reedline, nu-ansi-term, shlex, flate2, base64, plus one of fs2/libc for locking and wait-timeout for subprocess timeouts).

---

## 🤔 Assumptions
All originate from NON-BLOCKER defaults within the locked constraints; each is safe:
1. **`--json` envelope wrapper** (`{ok,action,data,error,warnings}`) is added even though the Python originals returned bare dicts. Safe: it's a strict superset that gives agents a uniform success/error contract; documented in HARNESS-rs and SKILL. (Improvement over the original, consistent with HARNESS's "fail loudly/clearly".)
2. **Undo cap = 50 for both proof CLIs.** Inkscape's Python original capped at 50; mermaid's did not cap. Standardizing on 50 is safe (bounded memory, matches the brief's "~50 levels like the original").
3. **mermaid render: try local `mmdc` first, then HTTP fallback.** The brief says "render via mermaid CLI/HTTP"; the Python original was HTTP-only. Local `mmdc` is confirmed installed, so we add it as the preferred path and keep HTTP for portability. Safe and strictly more capable.
4. **inkscape has no Pillow-style software fallback.** The Python original fell back to Pillow for PNG when inkscape was absent; the HARNESS non-negotiable is "use the real software, don't reimplement it." The Rust port requires real `inkscape` (confirmed installed) and errors loudly if missing. Safe and more correct per HARNESS.
5. **On-disk project files are Rust serde JSON; not byte-compatible with the Python files.** Cross-language file interop is not a stated requirement. Logical shape is preserved. Safe.
6. **`cli-hub previews ...` consumer is documented, not built.** The brief states "producer = the CLI; consumer = read-only inspection" and lists building the generator + 2 proof CLIs; a separate consumer binary is not in scope. Core emits the full bundle/session/trajectory so a future consumer can read it. Safe and explicit in Non-goals.
7. **Locking via `fs2` (advisory lock), or `#[cfg(unix)] libc::flock` if dependency minimality demands it.** macOS/Linux are the proof targets; both options reproduce the `session-locking.md` "lock then truncate inside the lock" pattern. Safe.
8. **Subprocess timeouts via `wait-timeout` crate.** Minimal, audited, avoids tokio (which the locked decision forbids in core). Safe.
9. **fingerprint canonical form** reproduces Python's sorted-keys/compact-separators rule for intra-Rust stability; cross-language fingerprint equality is explicitly NOT a goal. Safe.
10. **Plan file location.** The brief explicitly requested this file at `CLI-Anything-rust/PLAN-cli-anything-rust.md` rather than the skill's default `~/.agent-tools/output/plan-master/`. Honored as instructed.

---

## ✅ Completion Criteria
A reviewer / CI should verify:
1. `cargo build --workspace`, `cargo clippy ... -D warnings`, `cargo fmt --check`, `cargo deny check` all pass.
2. `cli-anything new --software <x>` stamps a crate that compiles with zero hand-edits, enters REPL, and runs `emit-skill`.
3. `cli-anything-mermaid`: one-shot + REPL + `--json` everywhere; SVG/PNG verified by magic bytes via `mmdc` and HTTP; `export share` URL correct; auto-save + `--dry-run` correct.
4. `cli-anything-inkscape`: full command surface; real `inkscape` PNG/PDF export verified by magic bytes; SVG well-formed; document round-trip; undo/redo with 50 cap; XML-safety + path-guard + no-shell-string + timeout all tested.
5. Each proof CLI's `emit-skill` SKILL.md matches its clap tree and is non-empty with valid frontmatter.
6. Core preview subsystem: fingerprint determinism, cache hit/miss, immutable bundles, mutable session head, append-only trajectory, and `preview live status --json` with compact `trajectory_summary` all unit-covered.
7. `#![forbid(unsafe_code)]` present in core + both CLIs; security checklist in `HARNESS-rs.md` fully checked, each item mapped to a core API.
8. `plugin/` is a valid Claude Code plugin exposing `/cli-anything-rs` (+ refine/test/validate/list) and referencing `HARNESS-rs.md`.
9. Local install works: `cargo install --path crates/cli-anything-mermaid` and `.../cli-anything-inkscape` put the binaries on PATH; crate metadata is publish-ready with `publish = false`.

---

## 📊 Confidence Declaration
I am ≥99% confident this plan can be executed without further clarification.
